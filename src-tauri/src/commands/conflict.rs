use std::fs;
use std::path::{Component, Path};

use git2::{IndexEntry, Repository};
use serde::Serialize;
use tauri::State;

use super::branch::{finish, reject_dash, run_git};
use super::repo::{OperationResult, RepoStore, lock_ops, read_status, with_open, workdir};

const CONTENT_LIMIT: usize = 1_000_000;
const SAVE_LIMIT: usize = 8_000_000;
const REFERENCE_LIMIT: usize = 400_000;
const BLOCK_LIMIT: usize = 200;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictBlock {
    pub kind: String,
    pub text: String,
    pub index: usize,
    pub ours: String,
    pub theirs: String,
    pub base: Option<String>,
    pub ours_label: String,
    pub theirs_label: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictFile {
    pub path: String,
    pub state: String,
    pub editable: bool,
    /// "binary" / "tooLarge" / "missing"；能编辑或已无标记时为 None
    pub reason: Option<String>,
    pub has_markers: bool,
    pub truncated: bool,
    pub newline: String,
    pub final_newline: bool,
    pub conflict_count: usize,
    pub blocks: Vec<ConflictBlock>,
    pub base: Option<String>,
    pub ours: Option<String>,
    pub theirs: Option<String>,
    pub base_present: bool,
    pub ours_present: bool,
    pub theirs_present: bool,
    pub reference_truncated: bool,
}

#[derive(Default)]
struct Stage {
    present: bool,
    binary: bool,
    bytes: Vec<u8>,
    text: Option<String>,
}

#[derive(Default)]
struct Stages {
    base: Stage,
    ours: Stage,
    theirs: Stage,
}

struct Parsed {
    blocks: Vec<ConflictBlock>,
    has_markers: bool,
    conflict_count: usize,
    truncated: bool,
    newline: String,
    final_newline: bool,
}

impl Parsed {
    fn empty() -> Self {
        Self {
            blocks: Vec::new(),
            has_markers: false,
            conflict_count: 0,
            truncated: false,
            newline: "lf".to_string(),
            final_newline: true,
        }
    }
}

struct FoundConflict<'a> {
    ours: Vec<&'a str>,
    base: Option<Vec<&'a str>>,
    theirs: Vec<&'a str>,
    ours_label: String,
    theirs_label: String,
    end: usize,
}

fn marker_label(line: &str, marker: &str) -> String {
    line.get(marker.len()..).unwrap_or("").trim().to_string()
}

fn find_conflict<'a>(lines: &[&'a str], start: usize) -> Option<FoundConflict<'a>> {
    let mut base_start = None;
    let mut separator = None;
    let mut index = start + 1;
    while index < lines.len() {
        let line = lines[index];
        if line == "=======" {
            separator = Some(index);
            break;
        }
        if line.starts_with("|||||||") && base_start.is_none() {
            base_start = Some(index);
        } else if line.starts_with(">>>>>>>") {
            // 先遇到收尾标记说明这一段标记不完整，按普通文本处理
            return None;
        }
        index += 1;
    }
    let separator = separator?;

    let mut end = None;
    let mut index = separator + 1;
    while index < lines.len() {
        if lines[index].starts_with(">>>>>>>") {
            end = Some(index);
            break;
        }
        index += 1;
    }
    let end = end?;

    let (ours_end, base) = match base_start {
        Some(at) if at < separator => (at, Some(lines[at + 1..separator].to_vec())),
        _ => (separator, None),
    };
    Some(FoundConflict {
        ours: lines[start + 1..ours_end].to_vec(),
        base,
        theirs: lines[separator + 1..end].to_vec(),
        ours_label: marker_label(lines[start], "<<<<<<<"),
        theirs_label: marker_label(lines[end], ">>>>>>>"),
        end,
    })
}

fn push_text(blocks: &mut Vec<ConflictBlock>, buffer: &mut Vec<&str>) {
    if buffer.is_empty() {
        return;
    }
    blocks.push(ConflictBlock {
        kind: "text".to_string(),
        text: buffer.join("\n"),
        index: 0,
        ours: String::new(),
        theirs: String::new(),
        base: None,
        ours_label: String::new(),
        theirs_label: String::new(),
    });
    buffer.clear();
}

fn parse_markers(content: &str) -> Parsed {
    let crlf = content.contains("\r\n");
    let normalized = content.replace("\r\n", "\n");
    let final_newline = normalized.ends_with('\n');
    let mut lines: Vec<&str> = normalized.split('\n').collect();
    if final_newline {
        lines.pop();
    }

    let mut blocks = Vec::new();
    let mut buffer: Vec<&str> = Vec::new();
    let mut conflict_count = 0;
    let mut truncated = false;
    let mut index = 0;
    while index < lines.len() {
        if lines[index].starts_with("<<<<<<<") {
            if let Some(found) = find_conflict(&lines, index) {
                push_text(&mut blocks, &mut buffer);
                conflict_count += 1;
                if conflict_count > BLOCK_LIMIT {
                    conflict_count -= 1;
                    truncated = true;
                    break;
                }
                blocks.push(ConflictBlock {
                    kind: "conflict".to_string(),
                    text: String::new(),
                    index: conflict_count,
                    ours: found.ours.join("\n"),
                    base: found.base.map(|lines| lines.join("\n")),
                    theirs: found.theirs.join("\n"),
                    ours_label: found.ours_label,
                    theirs_label: found.theirs_label,
                });
                index = found.end + 1;
                continue;
            }
        }
        buffer.push(lines[index]);
        index += 1;
    }
    if !truncated {
        push_text(&mut blocks, &mut buffer);
    }

    let has_markers = conflict_count > 0;
    Parsed {
        blocks: if has_markers { blocks } else { Vec::new() },
        has_markers,
        conflict_count,
        truncated,
        newline: if crlf { "crlf".to_string() } else { "lf".to_string() },
        final_newline,
    }
}

fn stage_side(repo: &Repository, entry: Option<&IndexEntry>) -> Result<Stage, String> {
    let Some(entry) = entry else {
        return Ok(Stage::default());
    };
    let blob = repo
        .find_blob(entry.id)
        .map_err(|error| format!("读取冲突版本失败: {error}"))?;
    let bytes = blob.content().to_vec();
    let regular = entry.mode & 0o170000 == 0o100000;
    let text = if regular {
        std::str::from_utf8(&bytes)
            .ok()
            .filter(|value| !value.contains('\0'))
            .map(str::to_string)
    } else {
        None
    };
    Ok(Stage {
        present: true,
        binary: !regular || (text.is_none() && !bytes.is_empty()),
        bytes,
        text,
    })
}

fn read_stages(repo: &Repository, path: &str) -> Result<Option<Stages>, String> {
    let index = repo
        .index()
        .map_err(|error| format!("读取暂存区失败: {error}"))?;
    let conflicts = index
        .conflicts()
        .map_err(|error| format!("读取冲突列表失败: {error}"))?;
    for conflict in conflicts {
        let conflict = conflict.map_err(|error| format!("读取冲突条目失败: {error}"))?;
        let matches = conflict
            .ancestor
            .iter()
            .chain(conflict.our.iter())
            .chain(conflict.their.iter())
            .any(|entry| String::from_utf8_lossy(&entry.path) == path);
        if !matches {
            continue;
        }
        return Ok(Some(Stages {
            base: stage_side(repo, conflict.ancestor.as_ref())?,
            ours: stage_side(repo, conflict.our.as_ref())?,
            theirs: stage_side(repo, conflict.their.as_ref())?,
        }));
    }
    Ok(None)
}

fn validate_relative(path: &str) -> Result<(), String> {
    if path.trim().is_empty() || path.contains('\0') {
        return Err("文件路径不合法".to_string());
    }
    let mut has_name = false;
    for part in Path::new(path).components() {
        match part {
            Component::Normal(_) => has_name = true,
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("文件路径「{path}」不合法"));
            }
        }
    }
    if !has_name {
        return Err("文件路径不合法".to_string());
    }
    Ok(())
}

fn clip_reference(text: &str) -> (String, bool) {
    if text.chars().count() <= REFERENCE_LIMIT {
        return (text.to_string(), false);
    }
    (text.chars().take(REFERENCE_LIMIT).collect(), true)
}

fn reference(stage: &Stage) -> Option<String> {
    stage.text.as_deref().map(|text| clip_reference(text).0)
}

fn read_conflict_state(store: &RepoStore, path: &str) -> Result<Option<Stages>, String> {
    with_open(store, |repo| read_stages(repo, path))
}

#[tauri::command]
pub fn get_conflict(store: State<'_, RepoStore>, path: String) -> Result<ConflictFile, String> {
    reject_dash(&path, "文件路径")?;
    validate_relative(&path)?;
    let state = read_status(&store)?.state;
    let Some(stages) = read_conflict_state(&store, &path)? else {
        return Err("这个文件当前没有待解决的冲突".to_string());
    };

    let dir = workdir(&store)?;
    let target = dir.join(&path);
    let raw = match fs::read(&target) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("读取「{path}」失败: {error}")),
    };

    let mut reason = if stages.ours.binary || stages.theirs.binary || stages.base.binary {
        Some("binary".to_string())
    } else {
        None
    };
    let worktree_text = match raw {
        None => {
            reason.get_or_insert_with(|| "missing".to_string());
            None
        }
        Some(bytes) if bytes.len() > CONTENT_LIMIT => {
            reason.get_or_insert_with(|| "tooLarge".to_string());
            None
        }
        Some(bytes) => match std::str::from_utf8(&bytes) {
            Ok(text) if !text.contains('\0') => Some(text.to_string()),
            _ => {
                reason.get_or_insert_with(|| "binary".to_string());
                None
            }
        },
    };

    let parsed = worktree_text
        .as_deref()
        .map(parse_markers)
        .unwrap_or_else(Parsed::empty);
    let editable = reason.is_none() && parsed.has_markers && !parsed.truncated;
    let reference_truncated = [&stages.base, &stages.ours, &stages.theirs]
        .iter()
        .filter_map(|stage| stage.text.as_deref())
        .any(|text| text.chars().count() > REFERENCE_LIMIT);

    Ok(ConflictFile {
        path: path.clone(),
        state,
        editable,
        reason,
        has_markers: parsed.has_markers,
        truncated: parsed.truncated,
        newline: parsed.newline,
        final_newline: parsed.final_newline,
        conflict_count: parsed.conflict_count,
        blocks: if editable { parsed.blocks } else { Vec::new() },
        base: reference(&stages.base),
        ours: reference(&stages.ours),
        theirs: reference(&stages.theirs),
        base_present: stages.base.present,
        ours_present: stages.ours.present,
        theirs_present: stages.theirs.present,
        reference_truncated,
    })
}

fn write_worktree(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建目录失败: {error}"))?;
    }
    fs::write(path, bytes).map_err(|error| format!("写入文件失败: {error}"))
}

fn stage_path(store: &RepoStore, path: &str) -> Result<super::branch::GitOutput, String> {
    let dir = workdir(store)?;
    run_git(&dir, &["add", "-A", "--", path])
}

#[tauri::command]
pub fn save_conflict(
    store: State<'_, RepoStore>,
    path: String,
    content: String,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    reject_dash(&path, "文件路径")?;
    validate_relative(&path)?;
    if content.len() > SAVE_LIMIT {
        return Err("合并结果太大，请在外部编辑器中保存后回来标记为已解决。".to_string());
    }
    if read_conflict_state(&store, &path)?.is_none() {
        return Err("这个文件当前没有待解决的冲突".to_string());
    }

    let dir = workdir(&store)?;
    let target = dir.join(&path);
    let crlf = fs::read_to_string(&target)
        .map(|text| text.contains("\r\n"))
        .unwrap_or(false);
    let normalized = content.replace("\r\n", "\n");
    let bytes = if crlf {
        normalized.replace('\n', "\r\n").into_bytes()
    } else {
        normalized.into_bytes()
    };
    write_worktree(&target, &bytes)?;
    let outcome = stage_path(&store, &path)?;
    finish(&store, outcome, format!("已解决 {path}"), false)
}

#[tauri::command]
pub fn resolve_conflict_side(
    store: State<'_, RepoStore>,
    path: String,
    side: String,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    reject_dash(&path, "文件路径")?;
    validate_relative(&path)?;
    let (chosen, label) = match side.as_str() {
        "ours" => ("ours", "当前版本"),
        "theirs" => ("theirs", "传入版本"),
        _ => return Err("未知的冲突版本".to_string()),
    };
    let Some(stages) = read_conflict_state(&store, &path)? else {
        return Err("这个文件当前没有待解决的冲突".to_string());
    };
    let stage = if chosen == "ours" {
        &stages.ours
    } else {
        &stages.theirs
    };

    let dir = workdir(&store)?;
    let target = dir.join(&path);
    if stage.present {
        write_worktree(&target, &stage.bytes)?;
    } else {
        // 删除/修改冲突里被删掉的一侧：文件不存在才是采用它的正确结果
        match fs::remove_file(&target) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("删除「{path}」失败: {error}")),
        }
    }
    let outcome = stage_path(&store, &path)?;
    finish(&store, outcome, format!("已采用{label}解决 {path}"), false)
}

#[tauri::command]
pub fn mark_conflict_resolved(
    store: State<'_, RepoStore>,
    path: String,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    reject_dash(&path, "文件路径")?;
    validate_relative(&path)?;
    if read_conflict_state(&store, &path)?.is_none() {
        return Err("这个文件当前没有待解决的冲突".to_string());
    }
    let outcome = stage_path(&store, &path)?;
    finish(&store, outcome, format!("已解决 {path}"), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conflicts(parsed: &Parsed) -> Vec<&ConflictBlock> {
        parsed
            .blocks
            .iter()
            .filter(|block| block.kind == "conflict")
            .collect()
    }

    #[test]
    fn parses_merge_style_markers_into_text_and_conflicts() {
        let text = "top\n<<<<<<< HEAD\nours one\nours two\n=======\ntheirs one\n>>>>>>> feature\nbottom\n";
        let parsed = parse_markers(text);
        assert!(parsed.has_markers);
        assert_eq!(parsed.conflict_count, 1);
        assert_eq!(parsed.newline, "lf");
        assert!(parsed.final_newline);
        assert_eq!(parsed.blocks.len(), 3);
        assert_eq!(parsed.blocks[0].kind, "text");
        assert_eq!(parsed.blocks[0].text, "top");
        let conflict = conflicts(&parsed)[0];
        assert_eq!(conflict.index, 1);
        assert_eq!(conflict.ours, "ours one\nours two");
        assert_eq!(conflict.theirs, "theirs one");
        assert_eq!(conflict.base, None);
        assert_eq!(conflict.ours_label, "HEAD");
        assert_eq!(conflict.theirs_label, "feature");
        assert_eq!(parsed.blocks[2].text, "bottom");
    }

    #[test]
    fn parses_diff3_base_sections_and_crlf() {
        let text = "a\r\n<<<<<<< HEAD\r\nours\r\n||||||| merged common ancestors\r\nbase\r\n=======\r\ntheirs\r\n>>>>>>> topic\r\n";
        let parsed = parse_markers(text);
        assert_eq!(parsed.newline, "crlf");
        assert!(parsed.final_newline);
        let conflict = conflicts(&parsed)[0];
        assert_eq!(conflict.base.as_deref(), Some("base"));
        assert_eq!(conflict.ours, "ours");
        assert_eq!(conflict.theirs, "theirs");
    }

    #[test]
    fn keeps_half_open_markers_as_plain_text() {
        let text = "<<<<<<< HEAD\nours\nno closing marker\n";
        let parsed = parse_markers(text);
        assert!(!parsed.has_markers);
        assert_eq!(parsed.conflict_count, 0);
        assert!(parsed.blocks.is_empty());
    }

    #[test]
    fn counts_multiple_conflicts_and_preserves_blank_lines() {
        let text = "<<<<<<< a\n1\n=======\n2\n>>>>>>> b\n\n<<<<<<< a\n3\n=======\n4\n>>>>>>> b\n";
        let parsed = parse_markers(text);
        assert_eq!(parsed.conflict_count, 2);
        assert_eq!(conflicts(&parsed)[1].index, 2);
        assert_eq!(parsed.blocks[1].kind, "text");
        assert_eq!(parsed.blocks[1].text, "");
    }

    #[test]
    fn rejects_parent_directory_segments() {
        assert!(validate_relative("src/main.rs").is_ok());
        assert!(validate_relative("./src/main.rs").is_ok());
        assert!(validate_relative("../outside.txt").is_err());
        assert!(validate_relative("C:/windows.txt").is_err());
        assert!(validate_relative("").is_err());
    }
}
