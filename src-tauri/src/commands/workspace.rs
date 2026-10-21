use std::collections::{HashMap, HashSet};
use std::path::Path;

use git2::{DiffFormat, DiffOptions, Oid, Repository, Status, StatusOptions};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::branch::{
    GitOutput, ensure_no_operation, finish, reject_dash, run_git, run_git_raw, run_git_stdin,
};
use super::repo::{OperationResult, RepoStore, lock_ops, with_open, workdir};

const FILE_LIMIT: usize = 400;
const DIFF_CHAR_LIMIT: usize = 80_000;
const COMMIT_FILE_LIMIT: usize = 300;
const COMMIT_DIFF_LIMIT: usize = 600_000;
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingTarget {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub stage: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitFileDiff {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub additions: usize,
    pub deletions: usize,
    pub binary: bool,
    pub truncated: bool,
    pub hunks: Vec<Hunk>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDiff {
    pub oid: String,
    pub files: Vec<CommitFileDiff>,
    pub additions: usize,
    pub deletions: usize,
    pub truncated: bool,
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
) -> Result<
    (
        HashMap<String, (usize, usize)>,
        HashMap<String, (usize, usize)>,
    ),
    String,
> {
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
        if let Some(delta) = entry.index_to_workdir() {
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

fn push_unique(paths: &mut Vec<String>, value: &str) {
    if !paths.iter().any(|existing| existing == value) {
        paths.push(value.to_string());
    }
}

/// Validates the entries the UI selected and flattens them into the pathspecs Git needs.
/// Old paths are included so renames are applied to both sides of the move.
fn collect_target_paths(
    store: &RepoStore,
    entries: &[WorkingTarget],
    allowed: &[&str],
) -> Result<Vec<String>, String> {
    if entries.is_empty() {
        return Err("没有选择文件".to_string());
    }
    let reported = with_open(store, reported_paths)?;
    let mut paths: Vec<String> = Vec::new();
    for entry in entries {
        if !allowed.contains(&entry.stage.as_str()) {
            return Err("该文件当前不支持这个操作，请刷新后重试".to_string());
        }
        for value in [Some(&entry.path), entry.old_path.as_ref()]
            .into_iter()
            .flatten()
        {
            reject_dash(value, "文件路径")?;
            if value.contains('\0') || value.contains("..") {
                return Err(format!("文件路径「{value}」不合法"));
            }
            if !reported.contains(value.as_str()) {
                return Err("变更清单已更新，请刷新后重试".to_string());
            }
            push_unique(&mut paths, value);
        }
    }
    Ok(paths)
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

    let binary = text.contains("GIT binary patch")
        || text
            .lines()
            .any(|line| line.starts_with("Binary files ") || line.starts_with("Files "));
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
        let remainder = &body[body
            .find('\n')
            .map(|offset| offset + 1)
            .unwrap_or(body.len())..];
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

struct ParsedFilePatch {
    path: String,
    old_path: Option<String>,
    status: String,
    diff: ParsedPatch,
}

fn split_patch_chunks(text: &str) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut start: Option<usize> = None;
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        if line.starts_with("diff --git ") || line.starts_with("diff --cc ") {
            if let Some(begin) = start {
                chunks.push(&text[begin..offset]);
            }
            start = Some(offset);
        }
        offset += line.len();
    }
    if let Some(begin) = start {
        chunks.push(&text[begin..]);
    }
    chunks
}

fn header_path(chunk: &str, marker: &str) -> Option<String> {
    for line in chunk.lines() {
        if let Some(rest) = line.strip_prefix(marker) {
            if rest == "/dev/null" {
                return None;
            }
            let path = rest
                .strip_prefix("a/")
                .or_else(|| rest.strip_prefix("b/"))
                .unwrap_or(rest);
            return Some(path.to_string());
        }
    }
    None
}

fn metadata_path(chunk: &str, marker: &str) -> Option<String> {
    chunk
        .lines()
        .find_map(|line| line.strip_prefix(marker))
        .map(|value| value.trim().to_string())
}

fn file_status(chunk: &str) -> &'static str {
    if chunk.lines().any(|line| line.starts_with("new file mode ")) {
        "added"
    } else if chunk
        .lines()
        .any(|line| line.starts_with("deleted file mode "))
    {
        "deleted"
    } else if chunk.lines().any(|line| line.starts_with("rename from ")) {
        "renamed"
    } else if chunk.lines().any(|line| line.starts_with("copy from ")) {
        "copied"
    } else if chunk.lines().any(|line| line.starts_with("old mode ")) {
        "typechange"
    } else {
        "modified"
    }
}

fn parse_file_patches(text: &str) -> (Vec<ParsedFilePatch>, bool) {
    let mut files = Vec::new();
    let mut truncated = false;
    let mut budget = 0usize;
    for chunk in split_patch_chunks(text) {
        let cost = chunk.chars().count();
        if files.len() >= COMMIT_FILE_LIMIT
            || (!files.is_empty() && budget + cost > COMMIT_DIFF_LIMIT)
        {
            truncated = true;
            break;
        }
        budget += cost;
        let status = file_status(chunk);
        let new_path = header_path(chunk, "+++ ");
        let old_path = header_path(chunk, "--- ");
        let renamed_to = metadata_path(chunk, "rename to ");
        let renamed_from = metadata_path(chunk, "rename from ");
        let path = renamed_to
            .or_else(|| new_path)
            .or_else(|| old_path.clone())
            .or_else(|| {
                chunk
                    .lines()
                    .next()
                    .and_then(|line| line.rsplit_once(" b/").map(|(_, value)| value.to_string()))
            })
            .unwrap_or_default();
        if path.is_empty() {
            continue;
        }
        let old = renamed_from.or(old_path).filter(|value| *value != path);
        files.push(ParsedFilePatch {
            path,
            old_path: old,
            status: status.to_string(),
            diff: parse_patch(chunk),
        });
    }
    (files, truncated)
}

fn build_commit_diff(oid: &str, text: &str) -> CommitDiff {
    let (parsed, truncated) = parse_file_patches(text);
    let mut additions = 0usize;
    let mut deletions = 0usize;
    let mut files = Vec::with_capacity(parsed.len());
    for file in parsed {
        let file_additions = file
            .diff
            .hunks
            .iter()
            .map(|hunk| hunk.additions)
            .sum::<usize>();
        let file_deletions = file
            .diff
            .hunks
            .iter()
            .map(|hunk| hunk.deletions)
            .sum::<usize>();
        additions += file_additions;
        deletions += file_deletions;
        files.push(CommitFileDiff {
            path: file.path,
            old_path: file.old_path,
            status: file.status,
            additions: file_additions,
            deletions: file_deletions,
            binary: file.diff.binary,
            truncated: file.diff.truncated,
            hunks: file
                .diff
                .hunks
                .into_iter()
                .enumerate()
                .map(|(index, hunk)| Hunk {
                    index,
                    header: hunk.header,
                    additions: hunk.additions,
                    deletions: hunk.deletions,
                    lines: hunk.lines,
                })
                .collect(),
        });
    }
    CommitDiff {
        oid: oid.to_string(),
        files,
        additions,
        deletions,
        truncated,
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
    if lines
        .last()
        .map(|line| line.text.is_empty())
        .unwrap_or(false)
    {
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
pub fn get_commit_diff(store: State<'_, RepoStore>, oid: String) -> Result<CommitDiff, String> {
    let (resolved, first_parent) = with_open(&store, |repo| {
        let oid = Oid::from_str(&oid).map_err(|error| format!("无效的提交哈希: {error}"))?;
        let commit = repo
            .find_commit(oid)
            .map_err(|error| format!("找不到提交: {error}"))?;
        let parent = if commit.parent_count() > 1 {
            Some(
                commit
                    .parent_id(0)
                    .map_err(|error| format!("读取父提交失败: {error}"))?
                    .to_string(),
            )
        } else {
            None
        };
        Ok((oid.to_string(), parent))
    })?;
    let dir = workdir(&store)?;
    let text = match first_parent {
        Some(parent) => run_git_raw(
            &dir,
            &[
                "diff",
                "--no-ext-diff",
                "--no-color",
                "--unified=3",
                "-M",
                parent.as_str(),
                resolved.as_str(),
            ],
        )?,
        None => run_git_raw(
            &dir,
            &[
                "show",
                "--format=",
                "--no-ext-diff",
                "--no-color",
                "--unified=3",
                "-M",
                resolved.as_str(),
            ],
        )?,
    };
    if !text.ok {
        return Err(if text.error.is_empty() {
            "读取提交差异失败".to_string()
        } else {
            text.error
        });
    }
    Ok(build_commit_diff(&resolved, &text.stdout))
}

#[tauri::command]
pub fn diff_commits(
    store: State<'_, RepoStore>,
    from: String,
    to: String,
) -> Result<CommitDiff, String> {
    for value in [&from, &to] {
        Oid::from_str(value).map_err(|error| format!("无效的提交哈希: {error}"))?;
    }
    let dir = workdir(&store)?;
    let text = run_git_raw(
        &dir,
        &[
            "diff",
            "--no-ext-diff",
            "--no-color",
            "--unified=3",
            "-M",
            from.as_str(),
            to.as_str(),
        ],
    )?;
    if !text.ok {
        return Err(if text.error.is_empty() {
            "对比提交失败".to_string()
        } else {
            text.error
        });
    }
    Ok(build_commit_diff(&format!("{from}..{to}"), &text.stdout))
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
    entries: Vec<WorkingTarget>,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    let paths = collect_target_paths(&store, &entries, &["staged", "unstaged", "untracked"])?;
    let dir = workdir(&store)?;
    let args = pathspec_args(&["add", "-A"], &paths);
    let outcome = run_git(&dir, &args)?;
    let message = format!("已暂存 {} 个文件", entries.len());
    finish(&store, outcome, message, false)
}

#[tauri::command]
pub fn unstage_files(
    store: State<'_, RepoStore>,
    entries: Vec<WorkingTarget>,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    let paths = collect_target_paths(&store, &entries, &["staged"])?;
    let dir = workdir(&store)?;
    // `reset` also works before the first commit, where `restore --staged` cannot resolve HEAD.
    let args = pathspec_args(&["reset", "-q"], &paths);
    let outcome = run_git(&dir, &args)?;
    let message = format!("已取消暂存 {} 个文件", entries.len());
    finish(&store, outcome, message, false)
}

#[tauri::command]
pub fn stage_all(store: State<'_, RepoStore>) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    let snapshot = with_open(&store, working_snapshot)?;
    let mut paths: Vec<String> = Vec::new();
    for file in &snapshot.files {
        if file.stage == "conflicted" {
            continue;
        }
        push_unique(&mut paths, &file.path);
        if let Some(old) = &file.old_path {
            push_unique(&mut paths, old);
        }
    }
    if paths.is_empty() {
        return Err("没有可暂存的变更".to_string());
    }
    let dir = workdir(&store)?;
    let args = pathspec_args(&["add", "-A"], &paths);
    let outcome = run_git(&dir, &args)?;
    finish(&store, outcome, "已暂存全部变更", false)
}

#[tauri::command]
pub fn unstage_all(store: State<'_, RepoStore>) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    let snapshot = with_open(&store, working_snapshot)?;
    let mut paths: Vec<String> = Vec::new();
    for file in snapshot.files.iter().filter(|file| file.stage == "staged") {
        push_unique(&mut paths, &file.path);
        if let Some(old) = &file.old_path {
            push_unique(&mut paths, old);
        }
    }
    if paths.is_empty() {
        return Err("暂存区已经是空的".to_string());
    }
    let dir = workdir(&store)?;
    let args = pathspec_args(&["reset", "-q"], &paths);
    let outcome = run_git(&dir, &args)?;
    finish(&store, outcome, "已取消暂存全部变更", false)
}

struct DiscardPlan {
    reset: Vec<String>,
    head: Vec<String>,
    work: Vec<String>,
    clean: Vec<String>,
}

fn discard_plan(entries: &[WorkingTarget]) -> Result<DiscardPlan, String> {
    let mut plan = DiscardPlan {
        reset: Vec::new(),
        head: Vec::new(),
        work: Vec::new(),
        clean: Vec::new(),
    };

    for entry in entries {
        match (entry.stage.as_str(), entry.status.as_str()) {
            ("conflicted", _) => {
                return Err("冲突文件请先解决冲突，或中止当前操作".to_string());
            }
            ("staged", "added") => {
                // A brand-new staged file has no HEAD version, so unstage and remove it.
                push_unique(&mut plan.reset, &entry.path);
                push_unique(&mut plan.clean, &entry.path);
            }
            ("staged", _) => {
                push_unique(&mut plan.head, &entry.path);
                if let Some(old) = &entry.old_path {
                    push_unique(&mut plan.head, old);
                }
            }
            ("unstaged", "renamed") => {
                match &entry.old_path {
                    Some(old) => push_unique(&mut plan.work, old),
                    None => push_unique(&mut plan.work, &entry.path),
                }
                // The new side of a worktree rename is still untracked; drop it too.
                push_unique(&mut plan.clean, &entry.path);
            }
            ("unstaged", _) => {
                push_unique(&mut plan.work, &entry.path);
                if let Some(old) = &entry.old_path {
                    push_unique(&mut plan.work, old);
                }
            }
            ("untracked", _) => push_unique(&mut plan.clean, &entry.path),
            _ => return Err("该文件当前不支持丢弃，请刷新后重试".to_string()),
        }
    }

    // Restoring a path and then cleaning it is redundant, and clean covers the
    // staged-added case where the path is no longer in the index.
    plan.work.retain(|path| !plan.clean.contains(path));
    plan.head.retain(|path| !plan.reset.contains(path));
    Ok(plan)
}

#[tauri::command]
pub fn discard_files(
    store: State<'_, RepoStore>,
    entries: Vec<WorkingTarget>,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    ensure_no_operation(&store)?;
    collect_target_paths(
        &store,
        &entries,
        &["staged", "unstaged", "untracked", "conflicted"],
    )?;
    let dir = workdir(&store)?;
    let plan = discard_plan(&entries)?;

    let mut last = GitOutput {
        ok: true,
        text: String::new(),
    };
    let mut apply = |args: Vec<&str>| -> Result<(), String> {
        let outcome = run_git(&dir, &args)?;
        if !outcome.ok {
            return Err(if outcome.text.is_empty() {
                "丢弃更改失败，请检查仓库状态".to_string()
            } else {
                outcome.text
            });
        }
        last = outcome;
        Ok(())
    };

    if !plan.reset.is_empty() {
        apply(pathspec_args(&["reset", "-q"], &plan.reset))?;
    }
    if !plan.head.is_empty() {
        apply(pathspec_args(
            &["restore", "--source=HEAD", "--staged", "--worktree"],
            &plan.head,
        ))?;
    }
    if !plan.work.is_empty() {
        apply(pathspec_args(&["restore", "--worktree"], &plan.work))?;
    }
    if !plan.clean.is_empty() {
        apply(pathspec_args(&["clean", "-f", "-q"], &plan.clean))?;
    }

    let message = format!("已丢弃 {} 个文件的更改", entries.len());
    finish(&store, last, message, false)
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
        let rows =
            stash_rows("stash@{0}\u{1f}WIP on main: abc subject\u{1f}2026-10-20T10:00:00+08:00\n");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].reference, "stash@{0}");
        assert!(rows[0].message.starts_with("WIP on main"));
    }

    #[test]
    fn staging_one_hunk_leaves_the_other_in_the_worktree() {
        let dir = std::env::temp_dir().join(format!("easy-git-hunk-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");

        let identity = ["-c", "user.name=Test", "-c", "user.email=test@example.com"];
        assert!(run_git(&dir, &["init", "."]).expect("git").ok);

        let mut body = String::new();
        for index in 1..=30 {
            body.push_str(&format!("line {index}\n"));
        }
        std::fs::write(dir.join("app.js"), &body).expect("write file");
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
            &[
                "apply",
                "--cached",
                "--recount",
                "--whitespace=nowarn",
                "--unidiff-zero",
            ],
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
        let dir =
            std::env::temp_dir().join(format!("easy-git-pathspec-test-{}", std::process::id()));
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
        let staged = run_git(&dir, &pathspec_args(&["add", "-A"], &paths)).expect("git");
        assert!(staged.ok, "stage failed: {}", staged.text);
        assert!(
            diff_text(&dir, "app.js", true)
                .expect("diff")
                .contains("+four")
        );

        let unstaged =
            run_git(&dir, &pathspec_args(&["restore", "--staged"], &paths)).expect("git");
        assert!(unstaged.ok, "unstage failed: {}", unstaged.text);
        assert!(diff_text(&dir, "app.js", true).expect("diff").is_empty());
        assert!(
            diff_text(&dir, "app.js", false)
                .expect("diff")
                .contains("+four")
        );

        let dropped = run_git(
            &dir,
            &pathspec_args(
                &["restore", "--source=HEAD", "--staged", "--worktree"],
                &paths,
            ),
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
        let dir = std::env::temp_dir().join(format!("easy-git-count-test-{}", std::process::id()));
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

    #[test]
    fn parses_a_multi_file_commit_patch() {
        let text = concat!(
            "diff --git a/app.js b/app.js\n",
            "index 1111111..2222222 100644\n",
            "--- a/app.js\n",
            "+++ b/app.js\n",
            "@@ -1 +1,2 @@\n",
            " a\n",
            "+b\n",
            "diff --git a/old.txt b/new name.txt\n",
            "similarity index 100%\n",
            "rename from old.txt\n",
            "rename to new name.txt\n",
            "diff --git a/gone.txt b/gone.txt\n",
            "deleted file mode 100644\n",
            "index 1111111..0000000\n",
            "--- a/gone.txt\n",
            "+++ /dev/null\n",
            "@@ -1 +0,0 @@\n",
            "-bye\n",
            "diff --git a/logo.png b/logo.png\n",
            "index 1111111..2222222 100644\n",
            "Binary files a/logo.png and b/logo.png differ\n",
            "diff --git a/added.txt b/added.txt\n",
            "new file mode 100644\n",
            "index 0000000..2222222\n",
            "--- /dev/null\n",
            "+++ b/added.txt\n",
            "@@ -0,0 +1 @@\n",
            "+hi\n",
        );
        let diff = build_commit_diff("abc", text);
        assert_eq!(diff.files.len(), 5);
        assert_eq!(diff.additions, 2);
        assert_eq!(diff.deletions, 1);
        assert!(!diff.truncated);

        let app = diff
            .files
            .iter()
            .find(|file| file.path == "app.js")
            .unwrap();
        assert_eq!(app.status, "modified");
        assert_eq!(app.additions, 1);
        assert!(!app.binary);

        let renamed = diff
            .files
            .iter()
            .find(|file| file.path == "new name.txt")
            .unwrap();
        assert_eq!(renamed.status, "renamed");
        assert_eq!(renamed.old_path.as_deref(), Some("old.txt"));
        assert!(renamed.hunks.is_empty());

        let removed = diff
            .files
            .iter()
            .find(|file| file.path == "gone.txt")
            .unwrap();
        assert_eq!(removed.status, "deleted");
        assert_eq!(removed.deletions, 1);

        let binary = diff
            .files
            .iter()
            .find(|file| file.path == "logo.png")
            .unwrap();
        assert!(binary.binary);
        assert!(binary.hunks.is_empty());

        let added = diff
            .files
            .iter()
            .find(|file| file.path == "added.txt")
            .unwrap();
        assert_eq!(added.status, "added");
        assert_eq!(added.additions, 1);
    }

    #[test]
    fn discard_plan_routes_each_entry_to_the_right_command() {
        let entries = vec![
            WorkingTarget {
                path: "new.txt".to_string(),
                old_path: None,
                status: "added".to_string(),
                stage: "staged".to_string(),
            },
            WorkingTarget {
                path: "new.txt".to_string(),
                old_path: None,
                status: "modified".to_string(),
                stage: "unstaged".to_string(),
            },
            WorkingTarget {
                path: "gone.txt".to_string(),
                old_path: None,
                status: "deleted".to_string(),
                stage: "staged".to_string(),
            },
            WorkingTarget {
                path: "work.txt".to_string(),
                old_path: None,
                status: "modified".to_string(),
                stage: "unstaged".to_string(),
            },
            WorkingTarget {
                path: "tmp.txt".to_string(),
                old_path: None,
                status: "added".to_string(),
                stage: "untracked".to_string(),
            },
        ];
        let plan = discard_plan(&entries).expect("plan");
        assert_eq!(plan.reset, vec!["new.txt"]);
        assert_eq!(plan.clean, vec!["new.txt", "tmp.txt"]);
        assert_eq!(plan.head, vec!["gone.txt"]);
        assert_eq!(plan.work, vec!["work.txt"]);

        let conflict = vec![WorkingTarget {
            path: "conflict.txt".to_string(),
            old_path: None,
            status: "modified".to_string(),
            stage: "conflicted".to_string(),
        }];
        assert!(discard_plan(&conflict).is_err());
    }

    #[test]
    fn reset_unstages_before_the_first_commit() {
        let dir = std::env::temp_dir().join(format!("easy-git-unborn-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        assert!(run_git(&dir, &["init", "."]).expect("git").ok);
        std::fs::write(dir.join("first.txt"), "hello\n").expect("write file");
        assert!(
            run_git(&dir, &["add", "-A", "--", "first.txt"])
                .expect("git")
                .ok
        );
        assert!(
            diff_text(&dir, "first.txt", true)
                .expect("diff")
                .contains("+hello")
        );

        let outcome = run_git(&dir, &["reset", "-q", "--", "first.txt"]).expect("git");
        assert!(outcome.ok, "unstage failed: {}", outcome.text);
        assert!(diff_text(&dir, "first.txt", true).expect("diff").is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
