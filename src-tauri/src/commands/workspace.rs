use std::collections::{HashMap, HashSet};
use std::path::Path;

use git2::{DiffFormat, DiffOptions, Repository, Status, StatusOptions};
use serde::Serialize;
use tauri::State;

use super::branch::{
    ensure_no_operation, finish, reject_dash, run_git, run_git_raw, run_git_stdin,
};
use super::repo::{lock_ops, with_open, workdir, OperationResult, RepoStore};

const FILE_LIMIT: usize = 400;
const DIFF_CHAR_LIMIT: usize = 80_000;
const UNTRACKED_BYTE_LIMIT: usize = 512_000;
const UNTRACKED_LINE_LIMIT: usize = 4_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingFile {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub stage: String,
    pub additions: usize,
    pub deletions: usize,
    pub hunkable: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingFiles {
    pub files: Vec<WorkingFile>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hunk {
    pub index: usize,
    pub header: String,
    pub additions: usize,
    pub deletions: usize,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub path: String,
    pub stage: String,
    pub hunkable: bool,
    pub binary: bool,
    pub truncated: bool,
    pub additions: usize,
    pub deletions: usize,
    pub hunks: Vec<Hunk>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StashEntry {
    pub reference: String,
    pub message: String,
    pub date: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitOutcome {
    pub message: String,
    pub output: String,
    pub oid: Option<String>,
    pub amended: bool,
}

fn normalize(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn count_lines(diff: &git2::Diff) -> Result<HashMap<String, (usize, usize)>, String> {
    let mut map: HashMap<String, (usize, usize)> = HashMap::new();
    diff.print(DiffFormat::Patch, |delta, _hunk, line| {
        let origin = line.origin();
        if origin != '+' && origin != '-' {
            return true;
        }
        let mut keys: Vec<String> = Vec::with_capacity(2);
        for path in [delta.new_file().path(), delta.old_file().path()]
            .into_iter()
            .flatten()
        {
            let key = normalize(path);
            if !key.is_empty() && !keys.contains(&key) {
                keys.push(key);
            }
        }
        for key in keys {
            let counters = map.entry(key).or_insert((0, 0));
            if origin == '+' {
                counters.0 += 1;
            } else {
                counters.1 += 1;
            }
        }
        true
    })
    .map_err(|error| format!("统计差异行数失败: {error}"))?;
    Ok(map)
}

fn side_counts(
    repo: &Repository,
) -> Result<(HashMap<String, (usize, usize)>, HashMap<String, (usize, usize)>), String> {
    let head_tree = match repo.head() {
        Ok(head) => head.peel_to_tree().ok(),
        Err(_) => None,
    };
    let index = repo
        .index()
        .map_err(|error| format!("读取暂存区失败: {error}"))?;

    let mut staged_options = DiffOptions::new();
    staged_options.include_typechange(true);
    let staged_diff = repo
        .diff_tree_to_index(head_tree.as_ref(), Some(&index), Some(&mut staged_options))
        .map_err(|error| format!("计算暂存区差异失败: {error}"))?;

    let mut work_options = DiffOptions::new();
    work_options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_typechange(true);
    let work_diff = repo
        .diff_index_to_workdir(Some(&index), Some(&mut work_options))
        .map_err(|error| format!("计算工作区差异失败: {error}"))?;

    Ok((count_lines(&staged_diff)?, count_lines(&work_diff)?))
}

fn status_options() -> StatusOptions {
    let mut options = StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .renames_head_to_index(true);
    options
}

/// Mutations only accept paths this command just reported, so a stale or crafted pathspec
/// never reaches a Git command.
fn reported_paths(repo: &Repository) -> Result<HashSet<String>, String> {
    let mut options = status_options();
    let statuses = repo
        .statuses(Some(&mut options))
        .map_err(|error| format!("读取工作区状态失败: {error}"))?;
    let mut set = HashSet::new();
    for entry in statuses.iter() {
        if let Ok(path) = entry.path() {
            set.insert(path.to_string());
        }
        if let Some(delta) = entry.head_to_index() {
            if let Some(old) = delta.old_file().path() {
                set.insert(normalize(old));
            }
        }
    }
    Ok(set)
}

fn ensure_reported(store: &RepoStore, paths: &[String]) -> Result<(), String> {
    let reported = with_open(store, reported_paths)?;
    for path in paths {
        reject_dash(path, "文件路径")?;
        if path.contains('\0') || path.contains("..") {
            return Err(format!("文件路径「{path}」不合法"));
        }
        if !reported.contains(path.as_str()) {
            return Err("变更清单已更新，请刷新后重试".to_string());
        }
    }
    Ok(())
}

fn ensure_path(store: &RepoStore, path: &str) -> Result<(), String> {
    ensure_reported(store, &[path.to_string()])
}

fn pathspec_args<'a>(base: &[&'a str], paths: &'a [String]) -> Vec<&'a str> {
    let mut args: Vec<&str> = base.to_vec();
    args.push("--");
    args.extend(paths.iter().map(|path| path.as_str()));
    args
}

fn working_snapshot(repo: &Repository) -> Result<WorkingFiles, String> {
    let (staged_counts, work_counts) = side_counts(repo)?;

    let mut options = status_options();
    let statuses = repo
        .statuses(Some(&mut options))
        .map_err(|error| format!("读取工作区状态失败: {error}"))?;

    let mut files: Vec<WorkingFile> = Vec::new();
    let mut truncated = false;

    for entry in statuses.iter() {
        if files.len() >= FILE_LIMIT {
            truncated = true;
            break;
        }
        let flags = entry.status();
        let path = match entry.path() {
            Ok(value) => value.to_string(),
            Err(_) => continue,
        };

        if flags.contains(Status::CONFLICTED) {
            files.push(WorkingFile {
                path,
                old_path: None,
                status: "conflicted".to_string(),
                stage: "conflicted".to_string(),
                additions: 0,
                deletions: 0,
                hunkable: false,
            });
            continue;
        }

        let index_kind = if flags.contains(Status::INDEX_NEW) {
            "added"
        } else if flags.contains(Status::INDEX_DELETED) {
            "deleted"
        } else if flags.contains(Status::INDEX_RENAMED) {
            "renamed"
        } else if flags.contains(Status::INDEX_TYPECHANGE) {
            "typechange"
        } else if flags.contains(Status::INDEX_MODIFIED) {
            "modified"
        } else {
            ""
        };

        if !index_kind.is_empty() {
            let (additions, deletions) = staged_counts.get(&path).copied().unwrap_or((0, 0));
            files.push(WorkingFile {
                path: path.clone(),
                old_path: entry
                    .head_to_index()
                    .and_then(|delta| delta.old_file().path().map(normalize)),
                status: index_kind.to_string(),
                stage: "staged".to_string(),
                additions,
                deletions,
                hunkable: matches!(index_kind, "added" | "modified"),
            });
            if files.len() >= FILE_LIMIT {
                truncated = true;
                break;
            }
        }

        if flags.contains(Status::WT_NEW) {
            let (additions, deletions) = work_counts.get(&path).copied().unwrap_or((0, 0));
            files.push(WorkingFile {
                path,
                old_path: None,
                status: "added".to_string(),
                stage: "untracked".to_string(),
                additions,
                deletions,
                hunkable: false,
            });
            continue;
        }

        let work_kind = if flags.contains(Status::WT_DELETED) {
            "deleted"
        } else if flags.contains(Status::WT_RENAMED) {
            "renamed"
        } else if flags.contains(Status::WT_TYPECHANGE) {
            "typechange"
        } else if flags.contains(Status::WT_MODIFIED) {
            "modified"
        } else {
            ""
        };

        if !work_kind.is_empty() {
            let (additions, deletions) = work_counts.get(&path).copied().unwrap_or((0, 0));
            files.push(WorkingFile {
                path,
                old_path: entry
                    .index_to_workdir()
                    .and_then(|delta| delta.old_file().path().map(normalize)),
                status: work_kind.to_string(),
                stage: "unstaged".to_string(),
                additions,
                deletions,
                hunkable: matches!(work_kind, "modified"),
            });
        }
    }

    Ok(WorkingFiles { files, truncated })
}

#[tauri::command]
pub fn get_working_files(store: State<'_, RepoStore>) -> Result<WorkingFiles, String> {
    with_open(&store, working_snapshot)
}

struct ParsedHunk {
    header: String,
    body: String,
    lines: Vec<DiffLine>,
    additions: usize,
    deletions: usize,
}

struct ParsedPatch {
    header: String,
    hunks: Vec<ParsedHunk>,
    binary: bool,
    truncated: bool,
}

fn line_kind(origin: u8) -> &'static str {
    match origin {
        b'+' => "add",
        b'-' => "del",
        b'\\' => "meta",
        _ => "context",
    }
}

fn parse_patch(text: &str) -> ParsedPatch {
    let bytes = text.as_bytes();
    let mut lines: Vec<usize> = Vec::new();
    let mut cursor = 0usize;
    for (position, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            lines.push(cursor);
            cursor = position + 1;
        }
    }
    if cursor < text.len() {
        lines.push(cursor);
    }
    let ends: Vec<usize> = lines
        .iter()
        .skip(1)
        .copied()
        .chain(std::iter::once(text.len()))
        .collect();

    let hunk_positions: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|&(index, start)| text[*start..ends[index]].starts_with("@@"))
        .map(|(index, _)| index)
        .collect();

    let binary = text.contains("GIT binary patch");
    if hunk_positions.is_empty() {
        return ParsedPatch {
            header: text.to_string(),
            hunks: Vec::new(),
            binary,
            truncated: false,
        };
    }

    let first = hunk_positions[0];
    let header = if first == 0 {
        String::new()
    } else {
        text[lines[0]..ends[first - 1]].to_string()
    };

    let mut hunks: Vec<ParsedHunk> = Vec::new();
    let mut consumed = 0usize;
    let mut truncated = false;
    for (position, hunk_index) in hunk_positions.iter().enumerate() {
        let start = lines[*hunk_index];
        let end = match hunk_positions.get(position + 1) {
            Some(next) => lines[*next],
            None => text.len(),
        };
        let body = text[start..end].to_string();
        let cost = body.chars().count();
        if !hunks.is_empty() && consumed + cost > DIFF_CHAR_LIMIT {
            truncated = true;
            break;
        }
        consumed += cost;

        let header_line = text[start..ends[*hunk_index]].trim_end().to_string();
        let remainder = &body[body.find('\n').map(|offset| offset + 1).unwrap_or(body.len())..];
        let mut parsed_lines: Vec<DiffLine> = Vec::new();
        let mut additions = 0;
        let mut deletions = 0;
        for raw in remainder.lines() {
            let kind = line_kind(raw.as_bytes().first().copied().unwrap_or(b' '));
            if kind == "add" {
                additions += 1;
            } else if kind == "del" {
                deletions += 1;
            }
            parsed_lines.push(DiffLine {
                kind: kind.to_string(),
                text: raw.chars().skip(1).collect(),
            });
        }
        hunks.push(ParsedHunk {
            header: header_line,
            body,
            lines: parsed_lines,
            additions,
            deletions,
        });
    }

    ParsedPatch {
        header,
        hunks,
        binary,
        truncated,
    }
}

fn to_file_diff(path: &str, stage: &str, parsed: ParsedPatch) -> FileDiff {
    let mut additions = 0;
    let mut deletions = 0;
    let hunks: Vec<Hunk> = parsed
        .hunks
        .into_iter()
        .enumerate()
        .map(|(index, hunk)| {
            additions += hunk.additions;
            deletions += hunk.deletions;
            Hunk {
                index,
                header: hunk.header,
                additions: hunk.additions,
                deletions: hunk.deletions,
                lines: hunk.lines,
            }
        })
        .collect();
    FileDiff {
        path: path.to_string(),
        stage: stage.to_string(),
        hunkable: matches!(stage, "staged" | "unstaged") && !parsed.binary && !parsed.truncated,
        binary: parsed.binary,
        truncated: parsed.truncated,
        additions,
        deletions,
        hunks,
    }
}

fn diff_text(dir: &Path, path: &str, staged: bool) -> Result<String, String> {
    let mut args: Vec<&str> = vec!["diff", "--no-ext-diff", "--unified=3"];
    if staged {
        args.push("--cached");
    }
    args.push("--");
    args.push(path);
    let outcome = run_git_raw(dir, &args)?;
    if !outcome.ok {
        return Err(outcome.error);
    }
    Ok(outcome.stdout)
}

fn untracked_diff(dir: &Path, path: &str) -> Result<FileDiff, String> {
    let root = dir
        .canonicalize()
        .map_err(|_| "无法定位仓库工作区".to_string())?;
    let target = dir
        .join(path)
        .canonicalize()
        .map_err(|_| format!("找不到文件「{path}」"))?;
    if !target.starts_with(&root) {
        return Err("文件路径不合法".to_string());
    }
    let meta = std::fs::metadata(&target).map_err(|_| format!("读取「{path}」失败"))?;
    if meta.len() as usize > UNTRACKED_BYTE_LIMIT {
        return Ok(FileDiff {
            path: path.to_string(),
            stage: "untracked".to_string(),
            hunkable: false,
            binary: false,
            truncated: true,
            additions: 0,
            deletions: 0,
            hunks: Vec::new(),
        });
    }

    let bytes = std::fs::read(&target).map_err(|_| format!("读取「{path}」失败"))?;
    if bytes.contains(&0) {
        return Ok(FileDiff {
            path: path.to_string(),
            stage: "untracked".to_string(),
            hunkable: false,
            binary: true,
            truncated: false,
            additions: 0,
            deletions: 0,
            hunks: Vec::new(),
        });
    }

    let text = String::from_utf8_lossy(&bytes).replace("\r\n", "\n");
    let mut lines: Vec<DiffLine> = Vec::new();
    let mut truncated = false;
    for raw in text.split('\n') {
        if lines.len() >= UNTRACKED_LINE_LIMIT {
            truncated = true;
            break;
        }
        lines.push(DiffLine {
            kind: "add".to_string(),
            text: raw.to_string(),
        });
    }
    if lines.last().map(|line| line.text.is_empty()).unwrap_or(false) {
        lines.pop();
    }
    let additions = lines.len();
    Ok(FileDiff {
        path: path.to_string(),
        stage: "untracked".to_string(),
        hunkable: false,
        binary: false,
        truncated,
        additions,
        deletions: 0,
        hunks: vec![Hunk {
            index: 0,
            header: format!("@@ -0,0 +1,{additions} @@"),
            additions,
            deletions: 0,
            lines,
        }],
    })
}

#[tauri::command]
pub fn get_file_diff(
    store: State<'_, RepoStore>,
    path: String,
    stage: String,
) -> Result<FileDiff, String> {
    reject_dash(&path, "文件路径")?;
    ensure_path(&store, &path)?;
    let dir = workdir(&store)?;
    match stage.as_str() {
        "untracked" => untracked_diff(&dir, &path),
        "staged" => {
            let text = diff_text(&dir, &path, true)?;
            Ok(to_file_diff(&path, "staged", parse_patch(&text)))
        }
        "unstaged" => {
            let text = diff_text(&dir, &path, false)?;
            Ok(to_file_diff(&path, "unstaged", parse_patch(&text)))
        }
        _ => Err("未知的差异来源".to_string()),
    }
}

#[tauri::command]
pub fn apply_hunk(
    store: State<'_, RepoStore>,
    path: String,
    stage: String,
    hunk_index: usize,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    reject_dash(&path, "文件路径")?;
    ensure_path(&store, &path)?;
    if stage != "staged" && stage != "unstaged" {
        return Err("该文件不支持按代码块暂存".to_string());
    }
    let dir = workdir(&store)?;

    // Built here so the hunk offsets always match the index as it stands right now.
    let staged = stage == "staged";
    let text = diff_text(&dir, &path, staged)?;
    let parsed = parse_patch(&text);
    let hunk = parsed
        .hunks
        .get(hunk_index)
        .ok_or_else(|| "该代码块已不在当前差异中，已为你刷新".to_string())?;
    let patch = format!("{}{}", parsed.header, hunk.body);

    let mut args: Vec<&str> = vec![
        "apply",
        "--cached",
        "--recount",
        "--whitespace=nowarn",
        "--unidiff-zero",
    ];
    if staged {
        args.push("--reverse");
    }
    let outcome = run_git_stdin(&dir, &args, patch.as_bytes())?;
    if !outcome.ok {
        return Err(format!("暂存该代码块失败：\n{}", outcome.text));
    }
    let message = if staged {
        "已取消暂存该代码块"
    } else {
        "已暂存该代码块"
    };
    finish(&store, outcome, message, false)
}

#[tauri::command]
pub fn stage_files(
    store: State<'_, RepoStore>,
    paths: Vec<String>,
) -> Result<OperationResult, String> {
    if paths.is_empty() {
        return Err("没有选择文件".to_string());
    }
    let _guard = lock_ops(&store)?;
    ensure_reported(&store, &paths)?;
    let dir = workdir(&store)?;
    let args = pathspec_args(&["add", "-A"], &paths);
    let outcome = run_git(&dir, &args)?;
    let message = format!("已暂存 {} 个文件", paths.len());
    finish(&store, outcome, message, false)
}

#[tauri::command]
pub fn unstage_files(
    store: State<'_, RepoStore>,
    paths: Vec<String>,
) -> Result<OperationResult, String> {
    if paths.is_empty() {
        return Err("没有选择文件".to_string());
    }
    let _guard = lock_ops(&store)?;
    ensure_reported(&store, &paths)?;
    let dir = workdir(&store)?;
    let args = pathspec_args(&["restore", "--staged"], &paths);
    let outcome = run_git(&dir, &args)?;
    let message = format!("已取消暂存 {} 个文件", paths.len());
    finish(&store, outcome, message, false)
}

#[tauri::command]
pub fn stage_all(store: State<'_, RepoStore>) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    let dir = workdir(&store)?;
    let outcome = run_git(&dir, &["add", "-A", "--", "."])?;
    finish(&store, outcome, "已暂存全部变更", false)
}

#[tauri::command]
pub fn unstage_all(store: State<'_, RepoStore>) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    let dir = workdir(&store)?;
    let outcome = run_git(&dir, &["restore", "--staged", "--", "."])?;
    finish(&store, outcome, "已取消暂存全部变更", false)
}

#[tauri::command]
pub fn discard_files(
    store: State<'_, RepoStore>,
    paths: Vec<String>,
    mode: String,
) -> Result<OperationResult, String> {
    if paths.is_empty() {
        return Err("没有选择文件".to_string());
    }
    let _guard = lock_ops(&store)?;
    ensure_no_operation(&store)?;
    ensure_reported(&store, &paths)?;
    let dir = workdir(&store)?;
    let args: Vec<&str> = match mode.as_str() {
        "head" => pathspec_args(&["restore", "--source=HEAD", "--staged", "--worktree"], &paths),
        "unstage" => pathspec_args(&["restore", "--staged"], &paths),
        "clean" => pathspec_args(&["clean", "-f", "-q"], &paths),
        _ => return Err("未知的丢弃方式".to_string()),
    };
    let outcome = run_git(&dir, &args)?;
    let message = format!("已丢弃 {} 个文件的更改", paths.len());
    finish(&store, outcome, message, false)
}

#[tauri::command]
pub fn commit_identity(store: State<'_, RepoStore>) -> Result<Option<String>, String> {
    let dir = workdir(&store)?;
    let outcome = run_git(&dir, &["var", "GIT_AUTHOR_IDENT"])?;
    if !outcome.ok || outcome.text.trim().is_empty() {
        return Ok(None);
    }
    // GIT_AUTHOR_IDENT is "Name <mail> <epoch> <tz>"; only the author label is useful here.
    let raw = outcome.text.trim();
    let mut parts: Vec<&str> = raw.split_whitespace().collect();
    if parts.len() > 2 {
        parts.truncate(parts.len() - 2);
        return Ok(Some(parts.join(" ")));
    }
    Ok(Some(raw.to_string()))
}

#[tauri::command]
pub fn commit_changes(
    store: State<'_, RepoStore>,
    summary: String,
    description: String,
    amend: bool,
) -> Result<CommitOutcome, String> {
    let _guard = lock_ops(&store)?;
    let status = ensure_no_operation(&store)?;
    let trimmed = summary.trim();
    if trimmed.is_empty() {
        return Err("请填写提交摘要".to_string());
    }
    if trimmed.contains('\n') || trimmed.contains('\r') || trimmed.contains('\0') {
        return Err("提交摘要只能是单行".to_string());
    }
    if description.contains('\0') {
        return Err("提交说明包含非法字符".to_string());
    }
    if status.staged == 0 && !amend {
        return Err("暂存区为空，请先选择要提交的变更".to_string());
    }
    if amend && status.head_oid.is_none() {
        return Err("仓库还没有提交可供修改".to_string());
    }

    let dir = workdir(&store)?;
    if !run_git(&dir, &["var", "GIT_AUTHOR_IDENT"])?.ok {
        return Err("未配置 Git 用户身份，请先设置 user.name 与 user.email 后再提交".to_string());
    }

    let mut args: Vec<String> = vec!["commit".to_string(), "-m".to_string(), trimmed.to_string()];
    let body = description.trim();
    if !body.is_empty() {
        args.push("-m".to_string());
        args.push(body.to_string());
    }
    if amend {
        args.push("--amend".to_string());
    }
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    let outcome = run_git(&dir, &borrowed)?;
    if !outcome.ok {
        return Err(if outcome.text.is_empty() {
            "提交失败，请检查仓库状态".to_string()
        } else {
            outcome.text
        });
    }

    let oid: Option<String> = with_open(&store, |repo| {
        Ok(repo
            .head()
            .ok()
            .and_then(|head| head.target())
            .map(|id| id.to_string()))
    })
    .unwrap_or(None);
    let short = oid
        .as_deref()
        .map(|value| value.chars().take(7).collect::<String>())
        .unwrap_or_default();
    let message = if amend {
        format!("已修改提交 {short}")
    } else {
        format!("已提交 {short}")
    };
    Ok(CommitOutcome {
        message,
        output: outcome.text,
        oid,
        amended: amend,
    })
}

fn stash_rows(text: &str) -> Vec<StashEntry> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split('\u{1f}');
            let reference = parts.next()?.trim();
            let message = parts.next().unwrap_or("WIP").trim();
            let date = parts.next().unwrap_or("").trim();
            if reference.is_empty() {
                return None;
            }
            Some(StashEntry {
                reference: reference.to_string(),
                message: if message.is_empty() {
                    "WIP".to_string()
                } else {
                    message.to_string()
                },
                date: date.to_string(),
            })
        })
        .collect()
}

fn list_snapshots(store: &RepoStore) -> Result<Vec<StashEntry>, String> {
    let dir = workdir(store)?;
    let outcome = run_git(&dir, &["stash", "list", "--format=%gd%x1f%gs%x1f%cI"])?;
    if !outcome.ok {
        return Ok(Vec::new());
    }
    Ok(stash_rows(&outcome.text))
}

#[tauri::command]
pub fn list_stashes(store: State<'_, RepoStore>) -> Result<Vec<StashEntry>, String> {
    list_snapshots(&store)
}

fn ensure_stash_reference(store: &RepoStore, reference: &str) -> Result<(), String> {
    reject_dash(reference, "贮藏引用")?;
    let inner = match reference
        .strip_prefix("stash@{")
        .and_then(|rest| rest.strip_suffix('}'))
    {
        Some(inner) => inner,
        None => return Err("贮藏引用格式不正确".to_string()),
    };
    if inner.is_empty() || !inner.chars().all(|character| character.is_ascii_digit()) {
        return Err("贮藏引用格式不正确".to_string());
    }
    let known = list_snapshots(store)?;
    if !known.iter().any(|entry| entry.reference == reference) {
        return Err("该贮藏已不存在，请刷新后重试".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn stash_push(
    store: State<'_, RepoStore>,
    message: String,
    include_untracked: bool,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    let status = ensure_no_operation(&store)?;
    if status.staged + status.unstaged + status.untracked == 0 {
        return Err("没有需要贮藏的变更".to_string());
    }
    let dir = workdir(&store)?;
    let note = message.trim();
    let mut args: Vec<String> = vec!["stash".to_string(), "push".to_string()];
    if include_untracked {
        args.push("--include-untracked".to_string());
    }
    args.push("-m".to_string());
    args.push(if note.is_empty() {
        format!("Easy Git · {}", status.branch)
    } else {
        note.to_string()
    });
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    let outcome = run_git(&dir, &borrowed)?;
    finish(&store, outcome, "已贮藏当前变更", false)
}

#[tauri::command]
pub fn stash_apply(
    store: State<'_, RepoStore>,
    reference: String,
    drop: bool,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    let dir = workdir(&store)?;
    ensure_stash_reference(&store, &reference)?;
    let verb = if drop { "pop" } else { "apply" };
    let outcome = run_git(&dir, &["stash", verb, reference.as_str()])?;
    let message = if drop {
        "已恢复并删除该贮藏"
    } else {
        "已恢复贮藏"
    };
    finish(&store, outcome, message, true)
}

#[tauri::command]
pub fn stash_drop(
    store: State<'_, RepoStore>,
    reference: String,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    let dir = workdir(&store)?;
    ensure_stash_reference(&store, &reference)?;
    let outcome = run_git(&dir, &["stash", "drop", reference.as_str()])?;
    finish(&store, outcome, "已删除该贮藏", false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_patch() -> &'static str {
        "diff --git a/app.js b/app.js\nindex 1111111..2222222 100644\n--- a/app.js\n+++ b/app.js\n@@ -1,3 +1,4 @@\n const a = 1;\n+const b = 2;\n const c = 3;\n@@ -20,3 +21,4 @@\n const x = 1;\n+const y = 2;\n const z = 3;\n"
    }

    #[test]
    fn splits_a_multi_hunk_patch_and_keeps_the_header() {
        let parsed = parse_patch(sample_patch());
        assert_eq!(parsed.hunks.len(), 2);
        assert!(!parsed.binary);
        assert!(parsed.header.starts_with("diff --git a/app.js b/app.js"));
        assert_eq!(parsed.hunks[0].additions, 1);
        assert_eq!(parsed.hunks[0].header, "@@ -1,3 +1,4 @@");

        let second = format!("{}{}", parsed.header, parsed.hunks[1].body);
        assert!(second.starts_with("diff --git"));
        assert!(second.contains("@@ -20,3 +21,4 @@"));
        assert!(!second.contains("@@ -1,3 +1,4 @@"));
    }

    #[test]
    fn classifies_added_deleted_and_no_newline_markers() {
        let parsed = parse_patch(
            "diff --git a/a b/a\n--- a/a\n+++ b/a\n@@ -1 +1 @@\n-old\n+new\n\\ No newline at end of file\n",
        );
        let kinds: Vec<&str> = parsed.hunks[0]
            .lines
            .iter()
            .map(|line| line.kind.as_str())
            .collect();
        assert_eq!(kinds, vec!["del", "add", "meta"]);
        assert_eq!(parsed.hunks[0].deletions, 1);
    }

    #[test]
    fn detects_binary_patches() {
        let parsed = parse_patch("diff --git a/a b/a\nindex 1..2\nGIT binary patch\nliteral 10\n");
        assert!(parsed.binary);
        assert!(parsed.hunks.is_empty());
    }

    #[test]
    fn parses_stash_rows_around_the_unit_separator() {
        let rows = stash_rows("stash@{0}\u{1f}WIP on main: abc subject\u{1f}2026-10-20T10:00:00+08:00\n");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].reference, "stash@{0}");
        assert!(rows[0].message.starts_with("WIP on main"));
    }

    #[test]
    fn staging_one_hunk_leaves_the_other_in_the_worktree() {
        let dir = std::env::temp_dir().join(format!(
            "easy-git-hunk-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");

        let identity = ["-c", "user.name=Test", "-c", "user.email=test@example.com"];
        assert!(run_git(&dir, &["init", "."]).expect("git").ok);

        let mut body = String::new();
        for index in 1..=30 {
            body.push_str(&format!("line {index}\n"));
        }
        std::fs::write(dir.join("app.js"), &body).expect("write file");
        assert!(run_git(&dir, &[&identity[..], &["add", "app.js"]].concat())
            .expect("git")
            .ok);
        assert!(run_git(
            &dir,
            &[&identity[..], &["commit", "-m", "base"]].concat()
        )
        .expect("git")
        .ok);

        let edited = body
            .replace("line 2\n", "line 2\ninserted top\n")
            .replace("line 25\n", "line 25\ninserted bottom\n");
        std::fs::write(dir.join("app.js"), &edited).expect("write edit");

        let text = diff_text(&dir, "app.js", false).expect("diff");
        let parsed = parse_patch(&text);
        assert_eq!(parsed.hunks.len(), 2, "expected two hunks in {text}");

        let patch = format!("{}{}", parsed.header, parsed.hunks[0].body);
        let applied = run_git_stdin(
            &dir,
            &["apply", "--cached", "--recount", "--whitespace=nowarn", "--unidiff-zero"],
            patch.as_bytes(),
        )
        .expect("apply");
        assert!(applied.ok, "apply failed: {}", applied.text);

        let staged = diff_text(&dir, "app.js", true).expect("staged diff");
        assert!(staged.contains("inserted top"));
        assert!(!staged.contains("inserted bottom"));

        let staged_parsed = parse_patch(&staged);
        assert_eq!(staged_parsed.hunks.len(), 1);
        let revert = format!("{}{}", staged_parsed.header, staged_parsed.hunks[0].body);
        let undone = run_git_stdin(
            &dir,
            &[
                "apply",
                "--cached",
                "--recount",
                "--whitespace=nowarn",
                "--unidiff-zero",
                "--reverse",
            ],
            revert.as_bytes(),
        )
        .expect("reverse apply");
        assert!(undone.ok, "reverse apply failed: {}", undone.text);
        assert!(diff_text(&dir, "app.js", true).expect("diff").is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pathspec_args_stage_unstage_and_discard_reach_the_file() {
        let dir = std::env::temp_dir().join(format!(
            "easy-git-pathspec-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");

        let identity = ["-c", "user.name=Test", "-c", "user.email=test@example.com"];
        assert!(run_git(&dir, &["init", "."]).expect("git").ok);
        std::fs::write(dir.join("app.js"), "one\ntwo\nthree\n").expect("write file");
        assert!(
            run_git(&dir, &[&identity[..], &["add", "app.js"]].concat())
                .expect("git")
                .ok
        );
        assert!(
            run_git(&dir, &[&identity[..], &["commit", "-m", "base"]].concat())
                .expect("git")
                .ok
        );
        std::fs::write(dir.join("app.js"), "one\ntwo\nthree\nfour\n").expect("edit file");
        std::fs::write(dir.join("scratch.txt"), "temp\n").expect("write untracked");

        let paths = vec!["app.js".to_string()];
        let staged = run_git(
            &dir,
            &pathspec_args(&["add", "-A"], &paths),
        )
        .expect("git");
        assert!(staged.ok, "stage failed: {}", staged.text);
        assert!(diff_text(&dir, "app.js", true).expect("diff").contains("+four"));

        let unstaged = run_git(
            &dir,
            &pathspec_args(&["restore", "--staged"], &paths),
        )
        .expect("git");
        assert!(unstaged.ok, "unstage failed: {}", unstaged.text);
        assert!(diff_text(&dir, "app.js", true).expect("diff").is_empty());
        assert!(diff_text(&dir, "app.js", false).expect("diff").contains("+four"));

        let dropped = run_git(
            &dir,
            &pathspec_args(&["restore", "--source=HEAD", "--staged", "--worktree"], &paths),
        )
        .expect("git");
        assert!(dropped.ok, "discard failed: {}", dropped.text);
        assert!(diff_text(&dir, "app.js", false).expect("diff").is_empty());

        let cleaned = run_git(
            &dir,
            &pathspec_args(&["clean", "-f", "-q"], &["scratch.txt".to_string()]),
        )
        .expect("git");
        assert!(cleaned.ok, "clean failed: {}", cleaned.text);
        assert!(!dir.join("scratch.txt").exists());

        let all = run_git(&dir, &["add", "-A", "--", "."]).expect("git");
        assert!(all.ok, "stage all failed: {}", all.text);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn counts_are_attributed_once_per_path() {
        let dir = std::env::temp_dir().join(format!(
            "easy-git-count-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");

        let identity = ["-c", "user.name=Test", "-c", "user.email=test@example.com"];
        assert!(run_git(&dir, &["init", "."]).expect("git").ok);
        std::fs::write(dir.join("a.js"), "one\ntwo\nthree\n").expect("write a");
        std::fs::write(dir.join("b.js"), "b1\nb2\nb3\nb4\n").expect("write b");
        assert!(
            run_git(&dir, &[&identity[..], &["add", "-A"]].concat())
                .expect("git")
                .ok
        );
        assert!(
            run_git(&dir, &[&identity[..], &["commit", "-m", "base"]].concat())
                .expect("git")
                .ok
        );

        std::fs::write(dir.join("a.js"), "one\nchanged\nthree\n").expect("edit a");
        assert!(
            run_git(&dir, &[&identity[..], &["mv", "b.js", "c.js"]].concat())
                .expect("git")
                .ok
        );

        let repo = Repository::open(&dir).expect("open repo");
        let (staged, work) = side_counts(&repo).expect("side counts");
        assert_eq!(work.get("a.js").copied(), Some((1, 1)));
        assert_eq!(staged.get("b.js").copied(), Some((0, 4)));
        assert_eq!(staged.get("c.js").copied(), Some((4, 0)));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
