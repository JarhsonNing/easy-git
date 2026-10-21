use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use git2::{
    BranchType, Delta, DiffFormat, DiffOptions, Oid, Patch, Repository, RepositoryState, Sort,
    Status, StatusOptions,
};
use serde::Serialize;
use tauri::State;

pub struct RepoStore {
    state: Mutex<Option<OpenRepo>>,
    ops: Mutex<()>,
}

impl Default for RepoStore {
    fn default() -> Self {
        Self {
            state: Mutex::new(None),
            ops: Mutex::new(()),
        }
    }
}

struct OpenRepo {
    repo: Repository,
}

pub(crate) fn with_open<T>(
    store: &RepoStore,
    run: impl FnOnce(&Repository) -> Result<T, String>,
) -> Result<T, String> {
    let guard = store.state.lock().map_err(|_| "仓库状态不可用".to_string())?;
    let open = guard.as_ref().ok_or_else(|| "尚未打开仓库".to_string())?;
    run(&open.repo)
}

pub(crate) fn lock_ops(store: &RepoStore) -> Result<MutexGuard<'_, ()>, String> {
    store
        .ops
        .lock()
        .map_err(|_| "仓库操作状态不可用".to_string())
}

pub(crate) fn workdir(store: &RepoStore) -> Result<PathBuf, String> {
    with_open(store, |repo| {
        if repo.is_bare() {
            return Err("裸仓库不支持工作区操作".to_string());
        }
        repo.workdir()
            .map(Path::to_path_buf)
            .ok_or_else(|| "无法定位仓库工作区".to_string())
    })
}

pub(crate) fn read_status(store: &RepoStore) -> Result<RepoStatus, String> {
    with_open(store, status_snapshot)
}

pub(crate) fn read_refs(store: &RepoStore) -> Result<RepoRefs, String> {
    with_open(store, refs_snapshot)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoSummary {
    pub name: String,
    pub path: String,
    pub head_branch: String,
    pub head_oid: Option<String>,
    pub remote_url: Option<String>,
}

#[tauri::command]
pub fn open_repository(store: State<'_, RepoStore>, path: String) -> Result<RepoSummary, String> {
    let repo = Repository::discover(Path::new(&path))
        .map_err(|e| format!("无法打开仓库: {}", e.message()))?;
    let summary = summarize(&repo, &path);

    let mut guard = store
        .state
        .lock()
        .map_err(|_| "仓库状态不可用".to_string())?;
    *guard = Some(OpenRepo { repo });
    Ok(summary)
}

#[tauri::command]
pub fn inspect_repository(path: String) -> Result<RepoSummary, String> {
    let repo = Repository::discover(Path::new(&path))
        .map_err(|e| format!("无法打开仓库: {}", e.message()))?;
    Ok(summarize(&repo, &path))
}

fn summarize(repo: &Repository, fallback_path: &str) -> RepoSummary {
    let root = repo
        .workdir()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| repo.path().to_path_buf());
    let name = root
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback_path.to_string());

    let (head_branch, head_oid) = match repo.head() {
        Ok(head) => {
            let branch = head
                .shorthand()
                .map(str::to_string)
                .unwrap_or_else(|_| "HEAD".to_string());
            (branch, head.target().map(|oid| oid.to_string()))
        }
        Err(_) => ("（无提交）".to_string(), None),
    };
    let remote_url = repo
        .find_remote("origin")
        .ok()
        .and_then(|remote| remote.url().ok().map(str::to_string));

    RepoSummary {
        name,
        path: trim_trailing_separators(&root.to_string_lossy()),
        head_branch,
        head_oid,
        remote_url,
    }
}

/// libgit2 把工作目录规范化成带结尾分隔符的形式，展示和写回配置时都要去掉。
fn trim_trailing_separators(value: &str) -> String {
    value.trim_end_matches(['/', '\\']).to_string()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitRow {
    pub oid: String,
    pub short_id: String,
    pub summary: String,
    pub author_name: String,
    pub authored_at: i64,
    pub lane: usize,
    pub parents: Vec<LaneParent>,
    pub through: Vec<usize>,
    pub refs: Vec<RefLabel>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaneParent {
    pub oid: String,
    pub lane: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefLabel {
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitPage {
    pub rows: Vec<CommitRow>,
    pub has_more: bool,
}

#[tauri::command]
pub fn load_graph(
    store: State<'_, RepoStore>,
    skip: usize,
    limit: usize,
) -> Result<CommitPage, String> {
    with_open(&store, |repo| build_page(repo, skip, limit.clamp(1, 2000)))
}

struct LaneFrame {
    lane: usize,
    parents: Vec<(Oid, usize)>,
    through: Vec<usize>,
}

fn assign_free(lanes: &mut Vec<Option<Oid>>, oid: Oid) -> usize {
    if let Some(index) = lanes.iter().position(Option::is_none) {
        lanes[index] = Some(oid);
        index
    } else {
        lanes.push(Some(oid));
        lanes.len() - 1
    }
}

fn layout_lanes(commits: &[(Oid, Vec<Oid>)]) -> Vec<LaneFrame> {
    let mut lanes: Vec<Option<Oid>> = Vec::new();
    let mut frames = Vec::with_capacity(commits.len());

    for (oid, parent_oids) in commits {
        let lane = match lanes.iter().position(|slot| *slot == Some(*oid)) {
            Some(index) => index,
            None => assign_free(&mut lanes, *oid),
        };

        let through = lanes
            .iter()
            .enumerate()
            .filter(|(index, slot)| *index != lane && slot.is_some())
            .map(|(index, _)| index)
            .collect();

        lanes[lane] = None;
        let mut parents = Vec::with_capacity(parent_oids.len());
        for (index, parent) in parent_oids.iter().enumerate() {
            if let Some(existing) = lanes.iter().position(|slot| *slot == Some(*parent)) {
                parents.push((*parent, existing));
            } else if index == 0 {
                lanes[lane] = Some(*parent);
                parents.push((*parent, lane));
            } else {
                let slot = assign_free(&mut lanes, *parent);
                parents.push((*parent, slot));
            }
        }

        frames.push(LaneFrame {
            lane,
            parents,
            through,
        });
    }

    frames
}

fn build_page(repo: &Repository, skip: usize, limit: usize) -> Result<CommitPage, String> {
    if repo.is_empty().unwrap_or(false) {
        return Ok(CommitPage {
            rows: Vec::new(),
            has_more: false,
        });
    }

    let target = skip.saturating_add(limit).saturating_add(1);
    let mut walk = repo
        .revwalk()
        .map_err(|e| format!("创建提交遍历失败: {e}"))?;
    let mut pushed = HashSet::new();
    let mut push_commit = |oid: Oid, walk: &mut git2::Revwalk| {
        if pushed.insert(oid) {
            let _ = walk.push(oid);
        }
    };

    if let Ok(branches) = repo.branches(None) {
        for (branch, _) in branches.flatten() {
            if let Ok(commit) = branch.get().peel_to_commit() {
                push_commit(commit.id(), &mut walk);
            }
        }
    }
    if let Ok(tags) = repo.tag_names(None) {
        for name in tags.iter().filter_map(|name| name.ok().flatten()) {
            if let Ok(object) = repo.revparse_single(&format!("refs/tags/{name}")) {
                if let Ok(commit) = object.peel_to_commit() {
                    push_commit(commit.id(), &mut walk);
                }
            }
        }
    }
    if let Ok(head) = repo.head() {
        if let Ok(commit) = head.peel_to_commit() {
            push_commit(commit.id(), &mut walk);
        }
    }

    walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)
        .map_err(|e| format!("设置提交排序失败: {e}"))?;

    let mut commits = Vec::new();
    for oid in walk.take(target) {
        let oid = oid.map_err(|e| format!("遍历提交失败: {e}"))?;
        let commit = repo
            .find_commit(oid)
            .map_err(|e| format!("读取提交 {oid} 失败: {e}"))?;
        let parents: Vec<Oid> = commit.parent_ids().collect();
        commits.push((commit, parents));
    }

    let order: Vec<(Oid, Vec<Oid>)> = commits
        .iter()
        .map(|(commit, parents)| (commit.id(), parents.clone()))
        .collect();
    let frames = layout_lanes(&order);
    let refs = collect_refs(repo);
    let has_more = commits.len() > skip.saturating_add(limit);

    let label_rank = |kind: &str| match kind {
        "head" => 0,
        "branch" => 1,
        "remote" => 2,
        "tag" => 3,
        _ => 4,
    };

    let mut rows = Vec::new();
    for ((commit, _), frame) in commits.iter().zip(frames).skip(skip).take(limit) {
        let mut labels = refs.get(&commit.id()).cloned().unwrap_or_default();
        labels.sort_by_key(|label| label_rank(&label.kind));
        let oid = commit.id().to_string();
        rows.push(CommitRow {
            short_id: oid.chars().take(7).collect(),
            oid,
            summary: commit
                .summary()
                .ok()
                .flatten()
                .map(str::to_string)
                .unwrap_or_else(|| "（无提交信息）".to_string()),
            author_name: commit
                .author()
                .name()
                .ok()
                .map(str::to_string)
                .unwrap_or_else(|| "未知作者".to_string()),
            authored_at: commit.author().when().seconds(),
            lane: frame.lane,
            parents: frame
                .parents
                .iter()
                .map(|(oid, lane)| LaneParent {
                    oid: oid.to_string(),
                    lane: *lane,
                })
                .collect(),
            through: frame.through,
            refs: labels,
        });
    }

    Ok(CommitPage { rows, has_more })
}

fn insert_ref(refs: &mut HashMap<Oid, Vec<RefLabel>>, oid: Oid, name: String, kind: &str) {
    let labels = refs.entry(oid).or_default();
    if labels
        .iter()
        .any(|label| label.name == name && label.kind == kind)
    {
        return;
    }
    labels.push(RefLabel {
        name,
        kind: kind.to_string(),
    });
}

fn collect_refs(repo: &Repository) -> HashMap<Oid, Vec<RefLabel>> {
    let mut refs = HashMap::new();

    if let Ok(branches) = repo.branches(Some(BranchType::Local)) {
        for (branch, _) in branches.flatten() {
            if let Some(name) = branch.name().ok().flatten() {
                if let Ok(commit) = branch.get().peel_to_commit() {
                    insert_ref(&mut refs, commit.id(), name.to_string(), "branch");
                }
            }
        }
    }

    if let Ok(branches) = repo.branches(Some(BranchType::Remote)) {
        for (branch, _) in branches.flatten() {
            if let Some(name) = branch.name().ok().flatten() {
                if let Ok(commit) = branch.get().peel_to_commit() {
                    insert_ref(&mut refs, commit.id(), name.to_string(), "remote");
                }
            }
        }
    }

    if let Ok(tags) = repo.tag_names(None) {
        for name in tags.iter().filter_map(|name| name.ok().flatten()) {
            if let Ok(object) = repo.revparse_single(&format!("refs/tags/{name}")) {
                if let Ok(commit) = object.peel_to_commit() {
                    insert_ref(&mut refs, commit.id(), name.to_string(), "tag");
                }
            }
        }
    }

    if let Ok(head) = repo.head() {
        if let Ok(commit) = head.peel_to_commit() {
            insert_ref(&mut refs, commit.id(), "HEAD".to_string(), "head");
        }
    }

    refs
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetail {
    pub oid: String,
    pub short_id: String,
    pub summary: String,
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    pub authored_at: i64,
    pub committer_name: String,
    pub committed_at: i64,
    pub parents: Vec<String>,
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
    pub files: Vec<CommitFile>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitFile {
    pub path: String,
    pub status: String,
    pub additions: usize,
    pub deletions: usize,
}

fn file_status(delta: Delta) -> &'static str {
    match delta {
        Delta::Added => "added",
        Delta::Deleted => "deleted",
        Delta::Modified => "modified",
        Delta::Renamed => "renamed",
        Delta::Copied => "copied",
        Delta::Typechange => "typechange",
        Delta::Conflicted => "conflicted",
        _ => "modified",
    }
}

#[tauri::command]
pub fn get_commit(store: State<'_, RepoStore>, oid: String) -> Result<CommitDetail, String> {
    with_open(&store, |repo| build_commit_detail(repo, &oid))
}

fn build_commit_detail(repo: &Repository, oid: &str) -> Result<CommitDetail, String> {
        let oid = Oid::from_str(oid).map_err(|e| format!("无效的提交哈希: {e}"))?;
        let commit = repo
            .find_commit(oid)
            .map_err(|e| format!("读取提交失败: {e}"))?;
        let tree = commit.tree().map_err(|e| format!("读取提交树失败: {e}"))?;
        let parent_tree = match commit.parent(0) {
            Ok(parent) => Some(
                parent
                    .tree()
                    .map_err(|e| format!("读取父提交树失败: {e}"))?,
            ),
            Err(_) => None,
        };

        let mut options = DiffOptions::new();
        options.include_typechange(true);
        let diff = repo
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut options))
            .map_err(|e| format!("计算变更失败: {e}"))?;
        let stats = diff.stats().map_err(|e| format!("统计算变更失败: {e}"))?;

        let mut files = Vec::new();
        for index in 0..diff.deltas().len() {
            if files.len() >= 300 {
                break;
            }
            let delta = diff
                .get_delta(index)
                .ok_or_else(|| "读取变更条目失败".to_string())?;
            let path = delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default();
            let (additions, deletions) = match Patch::from_diff(&diff, index) {
                Ok(Some(patch)) => match patch.line_stats() {
                    Ok((_, additions, deletions)) => (additions, deletions),
                    Err(_) => (0, 0),
                },
                _ => (0, 0),
            };
            files.push(CommitFile {
                path,
                status: file_status(delta.status()).to_string(),
                additions,
                deletions,
            });
        }

        let author = commit.author();
        let committer = commit.committer();
        let message = commit.message().unwrap_or_default().to_string();
        let summary = commit
            .summary()
            .ok()
            .flatten()
            .map(str::to_string)
            .unwrap_or_else(|| "（无提交信息）".to_string());
        let short_id = oid.to_string().chars().take(7).collect();

        Ok(CommitDetail {
            oid: oid.to_string(),
            short_id,
            summary,
            message,
            author_name: author
                .name()
                .ok()
                .map(str::to_string)
                .unwrap_or_else(|| "未知作者".to_string()),
            author_email: author.email().unwrap_or_default().to_string(),
            authored_at: author.when().seconds(),
            committer_name: committer
                .name()
                .ok()
                .map(str::to_string)
                .unwrap_or_else(|| "未知作者".to_string()),
            committed_at: committer.when().seconds(),
            parents: commit
                .parent_ids()
                .map(|parent| parent.to_string())
                .collect(),
            files_changed: stats.files_changed(),
            insertions: stats.insertions(),
            deletions: stats.deletions(),
            files,
        })
}

const PATCH_LIMIT: usize = 80_000;

fn collect_patch(diff: &git2::Diff) -> Result<String, String> {
    let mut patch = String::new();
    diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
        if matches!(line.origin(), '+' | '-' | ' ') {
            patch.push(line.origin());
        }
        patch.push_str(&String::from_utf8_lossy(line.content()));
        true
    })
    .map_err(|e| format!("生成补丁失败: {e}"))?;
    Ok(patch)
}

fn truncate_patch(patch: String) -> (String, bool) {
    if patch.chars().count() <= PATCH_LIMIT {
        (patch, false)
    } else {
        (patch.chars().take(PATCH_LIMIT).collect(), true)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingDiff {
    pub patch: String,
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
    pub truncated: bool,
}

fn build_working_diff(repo: &Repository, staged: bool) -> Result<WorkingDiff, String> {
    let head_tree = match repo.head() {
        Ok(head) => head.peel_to_tree().ok(),
        Err(_) => None,
    };
    let mut options = DiffOptions::new();
    options.include_typechange(true);
    options.context_lines(3);
    let diff = if staged {
        let index = repo
            .index()
            .map_err(|e| format!("读取暂存区失败: {e}"))?;
        repo.diff_tree_to_index(head_tree.as_ref(), Some(&index), Some(&mut options))
    } else {
        repo.diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut options))
    }
    .map_err(|e| format!("计算工作区差异失败: {e}"))?;
    let stats = diff.stats().map_err(|e| format!("统计工作区差异失败: {e}"))?;
    let (patch, truncated) = truncate_patch(collect_patch(&diff)?);
    Ok(WorkingDiff {
        patch,
        files_changed: stats.files_changed(),
        insertions: stats.insertions(),
        deletions: stats.deletions(),
        truncated,
    })
}

#[tauri::command]
pub fn get_working_diff(store: State<'_, RepoStore>, staged: bool) -> Result<WorkingDiff, String> {
    with_open(&store, |repo| build_working_diff(repo, staged))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitPatch {
    pub patch: String,
    pub truncated: bool,
}

fn build_commit_patch(repo: &Repository, oid: &str) -> Result<CommitPatch, String> {
    let oid = Oid::from_str(oid).map_err(|e| format!("无效的提交哈希: {e}"))?;
    let commit = repo
        .find_commit(oid)
        .map_err(|e| format!("读取提交失败: {e}"))?;
    let tree = commit.tree().map_err(|e| format!("读取提交树失败: {e}"))?;
    let parent_tree = match commit.parent(0) {
        Ok(parent) => Some(
            parent
                .tree()
                .map_err(|e| format!("读取父提交树失败: {e}"))?,
        ),
        Err(_) => None,
    };
    let mut options = DiffOptions::new();
    options.include_typechange(true);
    options.context_lines(3);
    let diff = repo
        .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut options))
        .map_err(|e| format!("计算提交差异失败: {e}"))?;
    let (patch, truncated) = truncate_patch(collect_patch(&diff)?);
    Ok(CommitPatch { patch, truncated })
}

#[tauri::command]
pub fn get_commit_patch(store: State<'_, RepoStore>, oid: String) -> Result<CommitPatch, String> {
    with_open(&store, |repo| build_commit_patch(repo, &oid))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    pub name: String,
    pub kind: String,
    pub oid: String,
    pub short_id: String,
    pub summary: String,
    pub updated_at: i64,
    pub is_current: bool,
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagInfo {
    pub name: String,
    pub oid: String,
    pub short_id: String,
    pub summary: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoRefs {
    pub local: Vec<BranchInfo>,
    pub remote: Vec<BranchInfo>,
    pub tags: Vec<TagInfo>,
    pub remotes: Vec<String>,
}

fn short_oid(oid: Oid) -> String {
    oid.to_string().chars().take(7).collect()
}

fn refs_snapshot(repo: &Repository) -> Result<RepoRefs, String> {
    let mut local = Vec::new();
    let mut remote = Vec::new();

    if let Ok(branches) = repo.branches(None) {
        for (branch, branch_type) in branches.flatten() {
            let Some(name) = branch.name().ok().flatten().map(str::to_string) else {
                continue;
            };
            if branch_type == BranchType::Remote && name.ends_with("/HEAD") {
                continue;
            }
            let Ok(commit) = branch.get().peel_to_commit() else {
                continue;
            };

            let mut upstream = None;
            let mut ahead = 0;
            let mut behind = 0;
            if branch_type == BranchType::Local {
                if let Ok(up) = branch.upstream() {
                    upstream = up.name().ok().flatten().map(str::to_string);
                    if let Ok(up_commit) = up.get().peel_to_commit() {
                        if let Ok((ahead_count, behind_count)) =
                            repo.graph_ahead_behind(commit.id(), up_commit.id())
                        {
                            ahead = ahead_count;
                            behind = behind_count;
                        }
                    }
                }
            }

            let info = BranchInfo {
                name,
                kind: if branch_type == BranchType::Local {
                    "local"
                } else {
                    "remote"
                }
                .to_string(),
                oid: commit.id().to_string(),
                short_id: short_oid(commit.id()),
                summary: commit
                    .summary()
                    .ok()
                    .flatten()
                    .unwrap_or("（无提交信息）")
                    .to_string(),
                updated_at: commit.time().seconds(),
                is_current: branch.is_head(),
                upstream,
                ahead,
                behind,
            };
            if branch_type == BranchType::Local {
                local.push(info);
            } else {
                remote.push(info);
            }
        }
    }

    local.sort_by(|a, b| {
        b.is_current
            .cmp(&a.is_current)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    remote.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    let mut tags = Vec::new();
    if let Ok(names) = repo.tag_names(None) {
        for name in names.iter().filter_map(|name| name.ok().flatten()) {
            let Ok(object) = repo.revparse_single(&format!("refs/tags/{name}")) else {
                continue;
            };
            let Ok(commit) = object.peel_to_commit() else {
                continue;
            };
            let updated_at = object
                .as_tag()
                .and_then(|tag| tag.tagger().map(|sig| sig.when().seconds()))
                .unwrap_or_else(|| commit.time().seconds());
            tags.push(TagInfo {
                name: name.to_string(),
                oid: commit.id().to_string(),
                short_id: short_oid(commit.id()),
                summary: commit
                    .summary()
                    .ok()
                    .flatten()
                    .unwrap_or("（无提交信息）")
                    .to_string(),
                updated_at,
            });
        }
    }
    tags.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    let remotes = repo
        .remotes()
        .map(|names| {
            names
                .iter()
                .filter_map(|name| name.ok().flatten())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    Ok(RepoRefs {
        local,
        remote,
        tags,
        remotes,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatus {
    pub branch: String,
    pub head_oid: Option<String>,
    pub detached: bool,
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub staged: usize,
    pub unstaged: usize,
    pub untracked: usize,
    pub conflicted: usize,
    pub state: String,
    pub conflicts: Vec<String>,
}

fn repo_state_name(state: RepositoryState) -> &'static str {
    match state {
        RepositoryState::Clean => "clean",
        RepositoryState::Merge => "merge",
        RepositoryState::Revert => "revert",
        RepositoryState::RevertSequence => "revert-sequence",
        RepositoryState::CherryPick => "cherry-pick",
        RepositoryState::CherryPickSequence => "cherry-pick-sequence",
        RepositoryState::Bisect => "bisect",
        RepositoryState::Rebase => "rebase",
        RepositoryState::RebaseInteractive => "rebase-interactive",
        RepositoryState::RebaseMerge => "rebase-merge",
        RepositoryState::ApplyMailbox => "apply-mailbox",
        RepositoryState::ApplyMailboxOrRebase => "apply-mailbox-or-rebase",
    }
}

fn status_snapshot(repo: &Repository) -> Result<RepoStatus, String> {
    let detached = repo.head_detached().unwrap_or(false);
    let head = repo.head().ok();
    let head_oid = head.as_ref().and_then(|head| head.target()).map(|oid| oid.to_string());
    let branch = if detached {
        "HEAD".to_string()
    } else {
        head.as_ref()
            .and_then(|head| head.shorthand().ok().map(str::to_string))
            .unwrap_or_else(|| "（无提交）".to_string())
    };

    let mut upstream = None;
    let mut ahead = 0;
    let mut behind = 0;
    if !detached {
        if let Ok(branch_ref) = repo.find_branch(&branch, BranchType::Local) {
            if let Ok(up) = branch_ref.upstream() {
                upstream = up.name().ok().flatten().map(str::to_string);
                if let (Some(local_oid), Ok(up_commit)) =
                    (branch_ref.get().target(), up.get().peel_to_commit())
                {
                    if let Ok((ahead_count, behind_count)) =
                        repo.graph_ahead_behind(local_oid, up_commit.id())
                    {
                        ahead = ahead_count;
                        behind = behind_count;
                    }
                }
            }
        }
    }

    let mut options = StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true);
    let statuses = repo
        .statuses(Some(&mut options))
        .map_err(|e| format!("读取工作区状态失败: {e}"))?;

    let mut staged = 0;
    let mut unstaged = 0;
    let mut untracked = 0;
    let mut conflicted = 0;
    for entry in statuses.iter() {
        let value = entry.status();
        if value.intersects(
            Status::INDEX_NEW
                | Status::INDEX_MODIFIED
                | Status::INDEX_DELETED
                | Status::INDEX_RENAMED
                | Status::INDEX_TYPECHANGE,
        ) {
            staged += 1;
        }
        if value.intersects(
            Status::WT_MODIFIED
                | Status::WT_DELETED
                | Status::WT_RENAMED
                | Status::WT_TYPECHANGE,
        ) {
            unstaged += 1;
        }
        if value.contains(Status::WT_NEW) {
            untracked += 1;
        }
        if value.contains(Status::CONFLICTED) {
            conflicted += 1;
        }
    }

    let mut conflicts = Vec::new();
    if let Ok(index) = repo.index() {
        if index.has_conflicts() {
            if let Ok(entries) = index.conflicts() {
                for entry in entries.flatten() {
                    let path = entry
                        .our
                        .as_ref()
                        .or(entry.their.as_ref())
                        .or(entry.ancestor.as_ref())
                        .and_then(|entry| String::from_utf8(entry.path.clone()).ok());
                    if let Some(path) = path {
                        if !conflicts.contains(&path) {
                            conflicts.push(path);
                        }
                        if conflicts.len() >= 30 {
                            break;
                        }
                    }
                }
            }
        }
    }

    Ok(RepoStatus {
        branch,
        head_oid,
        detached,
        upstream,
        ahead,
        behind,
        staged,
        unstaged,
        untracked,
        conflicted,
        state: repo_state_name(repo.state()).to_string(),
        conflicts,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationResult {
    pub message: String,
    pub output: String,
    pub conflicts: Vec<String>,
    pub state: String,
}

#[tauri::command]
pub fn get_repo_refs(store: State<'_, RepoStore>) -> Result<RepoRefs, String> {
    read_refs(&store)
}

#[tauri::command]
pub fn get_repo_status(store: State<'_, RepoStore>) -> Result<RepoStatus, String> {
    read_status(&store)
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Commit, Signature};

    fn oid(value: u8) -> Oid {
        Oid::from_bytes(&[value; 20]).expect("valid oid")
    }

    #[test]
    fn linear_history_uses_single_lane() {
        let a = oid(1);
        let b = oid(2);
        let c = oid(3);
        let frames = layout_lanes(&[(a, vec![b]), (b, vec![c]), (c, vec![])]);

        assert_eq!(frames.iter().map(|frame| frame.lane).collect::<Vec<_>>(), vec![0, 0, 0]);
        assert_eq!(frames[0].parents, vec![(b, 0)]);
        assert!(frames.iter().all(|frame| frame.through.is_empty()));
    }

    #[test]
    fn merge_schedules_second_parent_in_new_lane() {
        let merge = oid(10);
        let main = oid(11);
        let side = oid(12);
        let base = oid(13);
        let frames = layout_lanes(&[
            (merge, vec![main, side]),
            (main, vec![base]),
            (side, vec![base]),
            (base, vec![]),
        ]);

        assert_eq!(frames[0].lane, 0);
        assert_eq!(frames[0].parents, vec![(main, 0), (side, 1)]);
        assert_eq!(frames[1].lane, 0);
        assert_eq!(frames[1].through, vec![1]);
        assert_eq!(frames[2].lane, 1);
        assert_eq!(frames[2].parents, vec![(base, 0)]);
        assert_eq!(frames[2].through, vec![0]);
        assert_eq!(frames[3].lane, 0);
        assert!(frames[3].through.is_empty());
    }

    #[test]
    fn shared_parent_does_not_duplicate_lane() {
        let a = oid(21);
        let b = oid(22);
        let c = oid(23);
        let frames = layout_lanes(&[
            (a, vec![b, c]),
            (b, vec![c]),
            (c, vec![]),
        ]);

        assert_eq!(frames[0].parents, vec![(b, 0), (c, 1)]);
        assert_eq!(frames[1].lane, 0);
        assert_eq!(frames[1].parents, vec![(c, 1)]);
        assert_eq!(frames[1].through, vec![1]);
        assert_eq!(frames[2].lane, 1);
    }

    #[test]
    fn freed_lane_is_reused() {
        let a = oid(31);
        let b = oid(32);
        let c = oid(33);
        let d = oid(34);
        let e = oid(35);
        let frames = layout_lanes(&[
            (a, vec![b, c]),
            (b, vec![]),
            (c, vec![]),
            (d, vec![e]),
            (e, vec![]),
        ]);

        assert_eq!(frames[0].parents, vec![(b, 0), (c, 1)]);
        assert_eq!(frames[2].lane, 1);
        assert_eq!(frames[3].lane, 0);
        assert_eq!(frames[3].parents, vec![(e, 0)]);
        assert!(frames[3].through.is_empty());
    }

    fn make_repo(tag: &str) -> (Repository, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "easy-git-repo-test-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let repo = Repository::init(&dir).expect("init repo");
        (repo, dir)
    }

    fn write_commit(
        repo: &Repository,
        sig: &Signature,
        message: &str,
        content: Option<&str>,
        parents: &[&Commit],
    ) -> Oid {
        let mut index = repo.index().expect("open index");
        if let Some(content) = content {
            std::fs::write(repo.workdir().unwrap().join("file.txt"), content).expect("write file");
            index.add_path(std::path::Path::new("file.txt")).unwrap();
        }
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), sig, sig, message, &tree, parents)
            .expect("commit")
    }

    #[test]
    fn builds_graph_from_a_real_repository() {
        use git2::build::CheckoutBuilder;

        let (repo, _dir) = make_repo("graph");
        let sig = Signature::now("Alice", "alice@example.com").unwrap();

        let base = write_commit(&repo, &sig, "base", None, &[]);
        let base_commit = repo.find_commit(base).unwrap();
        let main_ref = repo.head().unwrap().name().unwrap().to_string();
        repo.branch("side", &base_commit, false).unwrap();

        let main_oid = write_commit(&repo, &sig, "main work", Some("one\nmain\n"), &[&base_commit]);
        repo.set_head("refs/heads/side").unwrap();
        repo.checkout_head(Some(CheckoutBuilder::new().force()))
            .unwrap();
        let side_oid = write_commit(&repo, &sig, "side work", Some("one\nside\n"), &[&base_commit]);
        repo.set_head(&main_ref).unwrap();

        let main_commit = repo.find_commit(main_oid).unwrap();
        let side_commit = repo.find_commit(side_oid).unwrap();
        let side_tree = side_commit.tree().unwrap();
        let merge = repo
            .commit(
                Some("HEAD"),
                &sig,
                &sig,
                "merge side",
                &side_tree,
                &[&main_commit, &side_commit],
            )
            .unwrap();
        let merge_object = repo.find_object(merge, None).unwrap();
        repo.tag_lightweight("v1.0", &merge_object, false).unwrap();

        let page = build_page(&repo, 0, 10).unwrap();
        assert_eq!(page.rows.len(), 4);
        assert!(!page.has_more);

        let top = &page.rows[0];
        assert_eq!(top.summary, "merge side");
        assert_eq!(top.lane, 0);
        assert_eq!(top.parents.len(), 2);
        let mut parent_lanes: Vec<usize> = top.parents.iter().map(|p| p.lane).collect();
        parent_lanes.sort();
        assert_eq!(parent_lanes, vec![0, 1]);
        assert!(top
            .refs
            .iter()
            .any(|label| label.kind == "head" && label.name == "HEAD"));
        assert!(top.refs.iter().any(|label| label.kind == "tag" && label.name == "v1.0"));
        assert!(top.refs.iter().any(|label| label.kind == "branch"));
        assert!(page
            .rows
            .iter()
            .any(|row| row.summary == "side work"
                && row.refs.iter().any(|label| label.name == "side")));
        assert_eq!(page.rows.last().unwrap().summary, "base");

        let first_page = build_page(&repo, 0, 2).unwrap();
        assert_eq!(first_page.rows.len(), 2);
        assert!(first_page.has_more);
        let second_page = build_page(&repo, 2, 2).unwrap();
        assert_eq!(second_page.rows.len(), 2);
        assert!(!second_page.has_more);
        let mut oids: Vec<String> = first_page
            .rows
            .iter()
            .chain(second_page.rows.iter())
            .map(|row| row.oid.clone())
            .collect();
        oids.sort();
        oids.dedup();
        assert_eq!(oids.len(), 4);

        let detail = build_commit_detail(&repo, &merge.to_string()).unwrap();
        assert_eq!(detail.summary, "merge side");
        assert_eq!(detail.parents.len(), 2);
        assert_eq!(detail.files_changed, 1);
        assert_eq!(detail.files[0].path, "file.txt");
        assert_eq!(detail.files[0].status, "modified");
        assert!(detail.insertions >= 1);
        assert!(detail.deletions >= 1);

        let base_detail = build_commit_detail(&repo, &base.to_string()).unwrap();
        assert!(base_detail.parents.is_empty());
        assert_eq!(base_detail.files_changed, 0);
    }

    #[test]
    fn builds_working_and_commit_patches() {
        let (repo, dir) = make_repo("patch");
        let sig = Signature::now("Alice", "alice@example.com").unwrap();

        let first = write_commit(&repo, &sig, "first", Some("hello\n"), &[]);
        let first_commit = repo.find_commit(first).unwrap();
        write_commit(&repo, &sig, "second", Some("hello\nworld\n"), &[&first_commit]);

        std::fs::write(dir.join("file.txt"), "hello\nworld\nagain\n").expect("write file");

        let working = build_working_diff(&repo, false).unwrap();
        assert_eq!(working.files_changed, 1);
        assert_eq!(working.insertions, 1);
        assert!(working.patch.contains("+again"));
        assert!(!working.truncated);

        let head = repo.head().unwrap().peel_to_commit().unwrap();
        let patch = build_commit_patch(&repo, &head.id().to_string()).unwrap();
        assert!(patch.patch.contains("+world"));
        assert!(!patch.truncated);

        std::fs::write(dir.join("file.txt"), "hello\nstaged\n").expect("write file");
        let mut index = repo.index().unwrap();
        index
            .add_path(std::path::Path::new("file.txt"))
            .expect("stage file");
        index.write().unwrap();
        let staged = build_working_diff(&repo, true).unwrap();
        assert!(staged.patch.contains("+staged"));

        let invalid = build_commit_patch(&repo, "not-an-oid");
        assert!(invalid.is_err());
    }
}
