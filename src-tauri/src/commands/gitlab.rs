use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::State;
use url::Url;

use crate::commands::repo::{self, RepoStore};
use crate::config::{
    self,
    schema::{GitLabInstance, ProjectEntry},
};

const PER_PAGE: u32 = 30;

fn truncate(value: &str, limit: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= limit {
        trimmed.to_string()
    } else {
        format!("{}…", trimmed.chars().take(limit).collect::<String>())
    }
}

/// GitLab 会用 `null` 表示“没有值”的字符串字段（项目描述、空仓库的默认分支、
/// 新增文件的 old_path、二进制文件的 diff、没有头像的用户……），
/// 而这些字段在界面里都当成空字符串处理。`#[serde(default)]` 只兜住“键不存在”，
/// 兜不住显式的 `null`，所以统一走这个反序列化。
pub(crate) mod null_str {
    use serde::{Deserializer, Deserialize};

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<String, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
    }
}

/// 同上，用于 GitLab 可能整体返回 `null` 的对象字段（例如已注销用户的 author）。
pub(crate) mod null_struct {
    use serde::de::DeserializeOwned;
    use serde::{Deserialize, Deserializer};

    pub(crate) fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: DeserializeOwned + Default,
        D: Deserializer<'de>,
    {
        Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
    }
}

pub(crate) fn encode_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn normalize_state(state: &str) -> Result<String, String> {
    let value = state.trim().to_lowercase();
    match value.as_str() {
        "opened" | "closed" | "merged" | "all" => Ok(value),
        _ => Err(format!("不支持的合并请求状态: {state}")),
    }
}

fn api_base(base_url: &str) -> Result<String, String> {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("请先在设置中填写 GitLab 实例地址".to_string());
    }
    let parsed = Url::parse(trimmed).map_err(|e| format!("GitLab 地址无效: {e}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("GitLab 地址必须以 http:// 或 https:// 开头".to_string());
    }
    if parsed.host_str().is_none() {
        return Err("GitLab 地址缺少主机名".to_string());
    }
    let base = trimmed
        .strip_suffix("/api/v4")
        .unwrap_or(trimmed)
        .trim_end_matches('/');
    Ok(format!("{base}/api/v4"))
}

struct RemoteParts {
    host: String,
    path: String,
}

fn normalize_remote_path(path: &str) -> Option<String> {
    let mut value = path.trim().trim_start_matches('/').trim_end_matches('/');
    if let Some(stripped) = value.strip_suffix(".git") {
        value = stripped;
    }
    let value = value.trim_end_matches('/');
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn parse_remote_url(raw: &str) -> Option<RemoteParts> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if raw.contains("://") {
        let parsed = Url::parse(raw).ok()?;
        if !matches!(parsed.scheme(), "http" | "https" | "ssh" | "git") {
            return None;
        }
        let host_name = parsed.host_str()?;
        let host = match parsed.port() {
            Some(port) => format!("{host_name}:{port}"),
            None => host_name.to_string(),
        };
        return Some(RemoteParts {
            host,
            path: normalize_remote_path(parsed.path())?,
        });
    }
    let without_user = raw.rsplit_once('@').map(|(_, rest)| rest).unwrap_or(raw);
    let (host, path) = without_user.split_once(':')?;
    if host.is_empty() || path.is_empty() {
        return None;
    }
    Some(RemoteParts {
        host: host.to_lowercase(),
        path: normalize_remote_path(path)?,
    })
}

fn project_path_from_remote(remote: &RemoteParts, base_url: &str) -> Result<String, String> {
    let parsed = Url::parse(base_url.trim())
        .map_err(|e| format!("GitLab 实例地址无效: {e}"))?;
    let prefix = parsed.path().trim_matches('/');
    let mut path = remote.path.clone();
    if !prefix.is_empty() {
        if path == prefix {
            return Err("远端地址指向 GitLab 实例根目录，无法确定项目".to_string());
        }
        if let Some(stripped) = path.strip_prefix(&format!("{prefix}/")) {
            path = stripped.to_string();
        }
    }
    if path.is_empty() {
        return Err("无法从远端地址解析 GitLab 项目路径".to_string());
    }
    Ok(path)
}

fn origin_url(repo: &git2::Repository) -> Option<String> {
    repo.find_remote("origin")
        .ok()
        .and_then(|remote| remote.url().ok().map(str::to_string))
        .or_else(|| {
            let names = repo.remotes().ok()?;
            let mut fallback = None;
            for name in names.iter().filter_map(|item| item.ok().flatten()) {
                if let Ok(remote) = repo.find_remote(name) {
                    if let Ok(url) = remote.url() {
                        fallback = Some(url.to_string());
                        break;
                    }
                }
            }
            fallback
        })
}

fn find_instance(instance_id: &str) -> Result<GitLabInstance, String> {
    let settings = config::get_settings()?;
    settings
        .gitlab
        .instances
        .into_iter()
        .find(|instance| instance.id == instance_id)
        .ok_or_else(|| "找不到该 GitLab 实例，请先在设置中重新保存".to_string())
}

pub(crate) struct ApiContext {
    pub(crate) client: reqwest::Client,
    pub(crate) base: String,
    pub(crate) token: String,
}

pub(crate) fn prepare(instance_id: &str) -> Result<ApiContext, String> {
    let instance = find_instance(instance_id)?;
    let base = api_base(&instance.base_url)?;
    let token = crate::config::secrets::read_secret(&format!("gitlab:{}", instance.id))?
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "未找到 GitLab 访问令牌，请先在设置中保存该实例的令牌".to_string())?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(40))
        .user_agent("easy-git/0.1")
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;
    Ok(ApiContext {
        client,
        base,
        token,
    })
}

pub(crate) async fn api_error(response: reqwest::Response) -> String {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let detail = truncate(&body, 240);
    match status.as_u16() {
        401 => "GitLab 认证失败，请检查访问令牌是否有效或已过期".to_string(),
        403 => format!("GitLab 权限不足（HTTP 403）：{detail}"),
        404 => "GitLab 项目或合并请求不存在，或当前令牌无权访问（HTTP 404）".to_string(),
        _ => format!("GitLab 返回 HTTP {status}：{detail}"),
    }
}

pub(crate) async fn parse_response<T: DeserializeOwned>(response: reqwest::Response) -> Result<T, String> {
    if !response.status().is_success() {
        return Err(api_error(response).await);
    }
    let body = response
        .text()
        .await
        .map_err(|e| format!("读取 GitLab 响应失败: {e}"))?;
    serde_json::from_str::<T>(&body).map_err(|e| decode_error(&body, &e))
}

fn decode_error(body: &str, error: &serde_json::Error) -> String {
    let head = truncate(body, 160);
    if !head.starts_with('{') && !head.starts_with('[') {
        return format!("GitLab 返回的不是 JSON，可能被登录页或代理拦截：{head}");
    }
    format!("解析 GitLab 响应失败：{error}")
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoRemoteInfo {
    pub remote_url: Option<String>,
    pub host: Option<String>,
    pub path: Option<String>,
}

#[tauri::command]
pub fn repo_remote_info(store: State<'_, RepoStore>) -> Result<RepoRemoteInfo, String> {
    let remote_url = repo::with_open(&store, |repo| Ok::<_, String>(origin_url(repo)))?;
    let parts = remote_url.as_deref().and_then(parse_remote_url);
    Ok(RepoRemoteInfo {
        remote_url,
        host: parts.as_ref().map(|parts| parts.host.clone()),
        path: parts.map(|parts| parts.path),
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitLabProjectRef {
    pub remote_url: String,
    pub remote_host: String,
    pub project_path: String,
    pub web_url: String,
}

#[tauri::command]
pub fn resolve_gitlab_project(
    store: State<'_, RepoStore>,
    instance_id: String,
) -> Result<GitLabProjectRef, String> {
    let instance = find_instance(&instance_id)?;
    let remote_url = repo::with_open(&store, |repo| Ok::<_, String>(origin_url(repo)))?
        .ok_or_else(|| "当前仓库没有 origin 远端，无法定位 GitLab 项目".to_string())?;
    let parts = parse_remote_url(&remote_url)
        .ok_or_else(|| format!("无法解析远端地址：{remote_url}"))?;
    let project_path = project_path_from_remote(&parts, &instance.base_url)?;
    let web_url = format!(
        "{}/{}",
        instance.base_url.trim().trim_end_matches('/'),
        project_path
    );
    Ok(GitLabProjectRef {
        remote_url,
        remote_host: parts.host,
        project_path,
        web_url,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalProjectMatch {
    pub project_path: String,
    pub local_path: String,
    pub name: String,
}

fn instance_host(base_url: &str) -> Option<String> {
    let parsed = Url::parse(base_url.trim()).ok()?;
    let host = parsed.host_str()?.to_lowercase();
    Some(match parsed.port() {
        Some(port) => format!("{host}:{port}"),
        None => host,
    })
}

/// 首页用它判断线上项目是否已经在本机：把已添加项目的 origin 解析成该实例下的项目路径。
/// 仓库被移动、删除或没有远端时跳过，不影响其余项目。
fn collect_local_matches(entries: &[ProjectEntry], base_url: &str) -> Vec<LocalProjectMatch> {
    let host = match instance_host(base_url) {
        Some(host) => host,
        None => return Vec::new(),
    };
    let mut matched = Vec::new();
    for entry in entries {
        let Ok(repo) = git2::Repository::open(&entry.path) else {
            continue;
        };
        let Some(remote) = origin_url(&repo).as_deref().and_then(parse_remote_url) else {
            continue;
        };
        if remote.host != host {
            continue;
        }
        let Ok(project_path) = project_path_from_remote(&remote, base_url) else {
            continue;
        };
        matched.push(LocalProjectMatch {
            project_path,
            local_path: entry.path.clone(),
            name: entry.name.clone(),
        });
    }
    matched
}

#[tauri::command]
pub async fn list_local_project_matches(
    instance_id: String,
) -> Result<Vec<LocalProjectMatch>, String> {
    let instance = find_instance(&instance_id)?;
    let entries = config::get_settings()?.projects;
    tauri::async_runtime::spawn_blocking(move || collect_local_matches(&entries, &instance.base_url))
        .await
        .map_err(|e| format!("读取本地仓库失败: {e}"))
}

#[derive(Debug, Deserialize, Default)]
struct RawUser {
    #[serde(default, with = "null_str")]
    name: String,
    #[serde(default, with = "null_str")]
    username: String,
    #[serde(default, with = "null_str")]
    avatar_url: String,
}

#[derive(Debug, Deserialize, Default)]
struct RawMergeRequest {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    iid: u64,
    #[serde(default, with = "null_str")]
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default, with = "null_str")]
    state: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    work_in_progress: bool,
    #[serde(default, with = "null_str")]
    source_branch: String,
    #[serde(default, with = "null_str")]
    target_branch: String,
    #[serde(default, with = "null_str")]
    web_url: String,
    #[serde(default, with = "null_str")]
    created_at: String,
    #[serde(default, with = "null_str")]
    updated_at: String,
    #[serde(default)]
    merged_at: Option<String>,
    #[serde(default)]
    user_notes_count: u64,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default, with = "null_struct")]
    author: RawUser,
    #[serde(default)]
    has_conflicts: bool,
    #[serde(default)]
    detailed_merge_status: Option<String>,
    #[serde(default)]
    merge_status: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeRequest {
    pub id: u64,
    pub iid: u64,
    pub title: String,
    pub description: String,
    pub state: String,
    pub draft: bool,
    pub source_branch: String,
    pub target_branch: String,
    pub web_url: String,
    pub created_at: String,
    pub updated_at: String,
    pub merged_at: Option<String>,
    pub user_notes_count: u64,
    pub labels: Vec<String>,
    pub author_name: String,
    pub author_username: String,
    pub author_avatar_url: String,
    pub has_conflicts: bool,
    pub merge_status: String,
}

impl From<RawMergeRequest> for MergeRequest {
    fn from(raw: RawMergeRequest) -> Self {
        Self {
            id: raw.id,
            iid: raw.iid,
            title: raw.title,
            description: raw.description.unwrap_or_default(),
            state: raw.state,
            draft: raw.draft || raw.work_in_progress,
            source_branch: raw.source_branch,
            target_branch: raw.target_branch,
            web_url: raw.web_url,
            created_at: raw.created_at,
            updated_at: raw.updated_at,
            merged_at: raw.merged_at,
            user_notes_count: raw.user_notes_count,
            labels: raw.labels,
            author_name: raw.author.name,
            author_username: raw.author.username,
            author_avatar_url: raw.author.avatar_url,
            has_conflicts: raw.has_conflicts,
            merge_status: raw
                .detailed_merge_status
                .or(raw.merge_status)
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeRequestPage {
    pub items: Vec<MergeRequest>,
    pub page: u32,
    pub has_more: bool,
}

async fn fetch_mr_page(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    project_path: &str,
    state: &str,
    search: Option<&str>,
    page: u32,
    per_page: u32,
) -> Result<MergeRequestPage, String> {
    let state = normalize_state(state)?;
    let mut params: Vec<(&str, String)> = vec![
        ("scope", "all".to_string()),
        ("order_by", "updated_at".to_string()),
        ("sort", "desc".to_string()),
        ("per_page", per_page.clamp(1, 100).to_string()),
        ("page", page.max(1).to_string()),
    ];
    if state != "all" {
        params.push(("state", state));
    }
    if let Some(term) = search.map(str::trim).filter(|term| !term.is_empty()) {
        params.push(("search", term.to_string()));
    }

    let response = client
        .get(format!(
            "{base}/projects/{}/merge_requests",
            encode_component(project_path)
        ))
        .header("PRIVATE-TOKEN", token)
        .query(&params)
        .send()
        .await
        .map_err(|e| format!("连接 GitLab 失败: {e}"))?;

    if !response.status().is_success() {
        return Err(api_error(response).await);
    }
    let has_next = response
        .headers()
        .get("x-next-page")
        .and_then(|value| value.to_str().ok())
        .map(|value| !value.trim().is_empty());
    let raw = response
        .json::<Vec<RawMergeRequest>>()
        .await
        .map_err(|e| format!("解析合并请求列表失败: {e}"))?;
    let has_more = has_next.unwrap_or(raw.len() >= per_page as usize);
    Ok(MergeRequestPage {
        items: raw.into_iter().map(MergeRequest::from).collect(),
        page,
        has_more,
    })
}

#[tauri::command]
pub async fn list_merge_requests(
    instance_id: String,
    project_path: String,
    state: String,
    page: u32,
    search: Option<String>,
) -> Result<MergeRequestPage, String> {
    let context = prepare(&instance_id)?;
    fetch_mr_page(
        &context.client,
        &context.base,
        &context.token,
        &project_path,
        &state,
        search.as_deref(),
        page,
        PER_PAGE,
    )
    .await
}

#[tauri::command]
pub async fn get_merge_request(
    instance_id: String,
    project_path: String,
    iid: u64,
) -> Result<MergeRequest, String> {
    let context = prepare(&instance_id)?;
    let response = context
        .client
        .get(format!(
            "{}/projects/{}/merge_requests/{iid}",
            context.base,
            encode_component(&project_path)
        ))
        .header("PRIVATE-TOKEN", context.token)
        .send()
        .await
        .map_err(|e| format!("连接 GitLab 失败: {e}"))?;
    let raw = parse_response::<RawMergeRequest>(response).await?;
    Ok(raw.into())
}

#[derive(Debug, Deserialize, Default)]
struct RawChange {
    #[serde(default, with = "null_str")]
    old_path: String,
    #[serde(default, with = "null_str")]
    new_path: String,
    #[serde(default, with = "null_str")]
    diff: String,
    #[serde(default)]
    new_file: bool,
    #[serde(default)]
    renamed_file: bool,
    #[serde(default)]
    deleted_file: bool,
    #[serde(default)]
    too_large: bool,
}

#[derive(Debug, Deserialize, Default)]
struct RawChanges {
    #[serde(default)]
    changes: Vec<RawChange>,
    #[serde(default)]
    overflow: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeRequestFile {
    pub old_path: String,
    pub new_path: String,
    pub diff: String,
    pub new_file: bool,
    pub renamed_file: bool,
    pub deleted_file: bool,
    pub too_large: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeRequestChanges {
    pub files: Vec<MergeRequestFile>,
    pub overflow: bool,
}

#[tauri::command]
pub async fn list_merge_request_changes(
    instance_id: String,
    project_path: String,
    iid: u64,
) -> Result<MergeRequestChanges, String> {
    let context = prepare(&instance_id)?;
    let response = context
        .client
        .get(format!(
            "{}/projects/{}/merge_requests/{iid}/changes",
            context.base,
            encode_component(&project_path)
        ))
        .header("PRIVATE-TOKEN", context.token)
        .send()
        .await
        .map_err(|e| format!("连接 GitLab 失败: {e}"))?;
    let raw = parse_response::<RawChanges>(response).await?;
    let mut overflow = raw.overflow;
    let mut files = Vec::new();
    for change in raw.changes {
        if files.len() >= 300 {
            overflow = true;
            break;
        }
        files.push(MergeRequestFile {
            old_path: change.old_path,
            new_path: change.new_path,
            diff: change.diff,
            new_file: change.new_file,
            renamed_file: change.renamed_file,
            deleted_file: change.deleted_file,
            too_large: change.too_large,
        });
    }
    Ok(MergeRequestChanges { files, overflow })
}

#[derive(Debug, Deserialize, Default)]
struct RawNote {
    #[serde(default)]
    id: u64,
    #[serde(default, with = "null_str")]
    body: String,
    #[serde(default, with = "null_struct")]
    author: RawUser,
    #[serde(default, with = "null_str")]
    created_at: String,
    #[serde(default)]
    system: bool,
    #[serde(default)]
    resolvable: bool,
    #[serde(default)]
    resolved: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeRequestNote {
    pub id: u64,
    pub body: String,
    pub author_name: String,
    pub author_username: String,
    pub created_at: String,
    pub system: bool,
    pub resolvable: bool,
    pub resolved: bool,
}

#[tauri::command]
pub async fn list_merge_request_notes(
    instance_id: String,
    project_path: String,
    iid: u64,
) -> Result<Vec<MergeRequestNote>, String> {
    let context = prepare(&instance_id)?;
    let response = context
        .client
        .get(format!(
            "{}/projects/{}/merge_requests/{iid}/notes",
            context.base,
            encode_component(&project_path)
        ))
        .header("PRIVATE-TOKEN", context.token)
        .query(&[
            ("sort", "asc"),
            ("order_by", "created_at"),
            ("per_page", "100"),
        ])
        .send()
        .await
        .map_err(|e| format!("连接 GitLab 失败: {e}"))?;
    let raw = parse_response::<Vec<RawNote>>(response).await?;
    Ok(raw
        .into_iter()
        .map(|note| MergeRequestNote {
            id: note.id,
            body: note.body,
            author_name: note.author.name,
            author_username: note.author.username,
            created_at: note.created_at,
            system: note.system,
            resolvable: note.resolvable,
            resolved: note.resolved,
        })
        .collect())
}

#[derive(Debug, Deserialize, Default)]
struct RawBranch {
    #[serde(default, with = "null_str")]
    name: String,
    #[serde(default, rename = "default")]
    is_default: bool,
    #[serde(default)]
    merged: bool,
    #[serde(default)]
    protected: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitLabBranch {
    pub name: String,
    pub is_default: bool,
    pub merged: bool,
    pub protected: bool,
}

async fn fetch_branches(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    project_path: &str,
) -> Result<Vec<GitLabBranch>, String> {
    // 该端点只接受 search/regex 等参数，传 order_by/sort 会被 GitLab 以 HTTP 400 拒绝
    let response = client
        .get(format!(
            "{base}/projects/{}/repository/branches",
            encode_component(project_path)
        ))
        .header("PRIVATE-TOKEN", token)
        .query(&[("per_page", "100")])
        .send()
        .await
        .map_err(|e| format!("连接 GitLab 失败: {e}"))?;
    let raw = parse_response::<Vec<RawBranch>>(response).await?;
    Ok(raw
        .into_iter()
        .map(|branch| GitLabBranch {
            name: branch.name,
            is_default: branch.is_default,
            merged: branch.merged,
            protected: branch.protected,
        })
        .collect())
}

#[tauri::command]
pub async fn list_gitlab_branches(
    instance_id: String,
    project_path: String,
) -> Result<Vec<GitLabBranch>, String> {
    let context = prepare(&instance_id)?;
    fetch_branches(&context.client, &context.base, &context.token, &project_path).await
}

#[derive(Debug, Serialize)]
struct CreateMergeRequestPayload<'a> {
    source_branch: &'a str,
    target_branch: &'a str,
    title: &'a str,
    description: &'a str,
    remove_source_branch: bool,
}

#[tauri::command]
pub async fn create_merge_request(
    instance_id: String,
    project_path: String,
    source_branch: String,
    target_branch: String,
    title: String,
    description: Option<String>,
    remove_source_branch: Option<bool>,
) -> Result<MergeRequest, String> {
    let source = source_branch.trim();
    let target = target_branch.trim();
    let title = title.trim();
    if source.is_empty() || target.is_empty() {
        return Err("请选择源分支和目标分支".to_string());
    }
    if source == target {
        return Err("源分支和目标分支不能相同".to_string());
    }
    if title.is_empty() {
        return Err("请填写合并请求标题".to_string());
    }
    let description = description.unwrap_or_default();
    let context = prepare(&instance_id)?;
    let response = context
        .client
        .post(format!(
            "{}/projects/{}/merge_requests",
            context.base,
            encode_component(&project_path)
        ))
        .header("PRIVATE-TOKEN", context.token)
        .json(&CreateMergeRequestPayload {
            source_branch: source,
            target_branch: target,
            title,
            description: description.trim(),
            remove_source_branch: remove_source_branch.unwrap_or(false),
        })
        .send()
        .await
        .map_err(|e| format!("连接 GitLab 失败: {e}"))?;
    let raw = parse_response::<RawMergeRequest>(response).await?;
    Ok(raw.into())
}

#[derive(Debug, Serialize)]
struct MergePayload {
    squash: bool,
    should_remove_source_branch: bool,
}

#[tauri::command]
pub async fn merge_merge_request(
    instance_id: String,
    project_path: String,
    iid: u64,
    squash: Option<bool>,
    remove_source_branch: Option<bool>,
) -> Result<MergeRequest, String> {
    let context = prepare(&instance_id)?;
    let response = context
        .client
        .put(format!(
            "{}/projects/{}/merge_requests/{iid}/merge",
            context.base,
            encode_component(&project_path)
        ))
        .header("PRIVATE-TOKEN", context.token)
        .json(&MergePayload {
            squash: squash.unwrap_or(false),
            should_remove_source_branch: remove_source_branch.unwrap_or(false),
        })
        .send()
        .await
        .map_err(|e| format!("连接 GitLab 失败: {e}"))?;
    let raw = parse_response::<RawMergeRequest>(response).await?;
    Ok(raw.into())
}

#[derive(Debug, Serialize)]
struct StatePayload<'a> {
    state_event: &'a str,
}

#[tauri::command]
pub async fn set_merge_request_state(
    instance_id: String,
    project_path: String,
    iid: u64,
    event: String,
) -> Result<MergeRequest, String> {
    let event = event.trim().to_lowercase();
    if !matches!(event.as_str(), "close" | "reopen") {
        return Err(format!("不支持的状态操作: {event}"));
    }
    let context = prepare(&instance_id)?;
    let response = context
        .client
        .put(format!(
            "{}/projects/{}/merge_requests/{iid}",
            context.base,
            encode_component(&project_path)
        ))
        .header("PRIVATE-TOKEN", context.token)
        .json(&StatePayload {
            state_event: &event,
        })
        .send()
        .await
        .map_err(|e| format!("连接 GitLab 失败: {e}"))?;
    let raw = parse_response::<RawMergeRequest>(response).await?;
    Ok(raw.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn parses_common_gitlab_remotes() {
        let ssh = parse_remote_url("git@gitlab.example.com:team/sub/repo.git").unwrap();
        assert_eq!(ssh.host, "gitlab.example.com");
        assert_eq!(ssh.path, "team/sub/repo");

        let https = parse_remote_url("https://gitlab.example.com/team/repo.git").unwrap();
        assert_eq!(https.host, "gitlab.example.com");
        assert_eq!(https.path, "team/repo");

        let with_port = parse_remote_url("ssh://git@gitlab.example.com:2222/team/repo.git").unwrap();
        assert_eq!(with_port.host, "gitlab.example.com:2222");
        assert_eq!(with_port.path, "team/repo");

        assert!(parse_remote_url("not a url").is_none());
    }

    #[test]
    fn strips_self_hosted_subpath() {
        let remote = parse_remote_url("https://example.com/gitlab/team/repo.git").unwrap();
        assert_eq!(
            project_path_from_remote(&remote, "https://example.com/gitlab").unwrap(),
            "team/repo"
        );
        let ssh = parse_remote_url("git@example.com:team/repo.git").unwrap();
        assert_eq!(
            project_path_from_remote(&ssh, "https://example.com/gitlab").unwrap(),
            "team/repo"
        );
    }

    #[test]
    fn builds_api_base() {
        assert_eq!(
            api_base("https://gitlab.example.com/").unwrap(),
            "https://gitlab.example.com/api/v4"
        );
        assert_eq!(
            api_base("https://example.com/gitlab/api/v4/").unwrap(),
            "https://example.com/gitlab/api/v4"
        );
        assert!(api_base("ftp://example.com").is_err());
    }

    #[test]
    fn encodes_project_path() {
        assert_eq!(encode_component("group/sub/repo"), "group%2Fsub%2Frepo");
        assert_eq!(encode_component("team repo"), "team%20repo");
    }

    #[test]
    fn maps_draft_and_merge_status() {
        let raw: RawMergeRequest = serde_json::from_str(
            r#"{
                "id": 11, "iid": 7, "title": "Draft: graph",
                "state": "opened", "work_in_progress": true,
                "source_branch": "feature", "target_branch": "main",
                "author": {"name": "Lin Wei", "username": "lin"},
                "merge_status": "can_be_merged",
                "detailed_merge_status": "mergeable"
            }"#,
        )
        .unwrap();
        let mr = MergeRequest::from(raw);
        assert!(mr.draft);
        assert_eq!(mr.merge_status, "mergeable");
        assert_eq!(mr.author_name, "Lin Wei");
        assert_eq!(mr.description, "");
    }

    #[test]
    fn tolerates_null_strings_and_objects() {
        // GitLab 用 null 表示“没有值”：没头像的用户、被注销的 MR 作者、新增文件的 old_path、二进制文件的 diff。
        let mr: RawMergeRequest = serde_json::from_str(
            r#"{
                "iid": 8, "title": "Graph", "state": "opened",
                "source_branch": "a", "target_branch": "main",
                "web_url": "https://gl/team/repo/-/merge_requests/8",
                "author": {"name": "Lin", "username": "lin", "avatar_url": null},
                "merged_at": null, "labels": []
            }"#,
        )
        .expect("avatar_url 为 null 时应能解析");
        assert_eq!(MergeRequest::from(mr).author_avatar_url, "");

        let note: RawNote = serde_json::from_str(
            r#"{"id": 3, "body": "ok", "author": null, "created_at": "2026-10-21T10:00:00Z"}"#,
        )
        .expect("author 为 null 时应能解析");
        assert_eq!(note.author.username, "");

        let change: RawChange = serde_json::from_str(
            r#"{"old_path": null, "new_path": "assets/logo.png", "diff": null, "new_file": true}"#,
        )
        .expect("新增文件与二进制 diff 应能解析");
        assert_eq!(change.old_path, "");
        assert_eq!(change.diff, "");
    }

    #[test]
    fn matches_local_repos_to_gitlab_projects() {
        let root = std::env::temp_dir().join(format!("easy-git-local-match-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create temp dir");

        let entry = |name: &str, url: &str| {
            let dir = root.join(name);
            std::fs::create_dir_all(&dir).expect("create repo dir");
            let repo = git2::Repository::init(&dir).expect("init repo");
            if !url.is_empty() {
                repo.remote("origin", url).expect("add origin");
            }
            ProjectEntry {
                path: dir.to_string_lossy().to_string(),
                name: name.to_string(),
            }
        };

        let entries = vec![
            entry("same-host", "http://gitlab.test:8888/team/demo.git"),
            entry("under-subpath", "http://gitlab.test:8888/gitlab/team/second.git"),
            entry("other-host", "https://gitlab.com/team/demo.git"),
            entry("no-remote", ""),
            ProjectEntry {
                path: root.join("moved-away").to_string_lossy().to_string(),
                name: "moved-away".to_string(),
            },
        ];

        let matched = collect_local_matches(&entries, "http://gitlab.test:8888");
        let names = matched
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["same-host", "under-subpath"], "只应命中同一主机的仓库");
        let paths = matched
            .iter()
            .map(|m| m.project_path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, ["team/demo", "gitlab/team/second"]);

        let subpath = collect_local_matches(&entries, "http://gitlab.test:8888/gitlab");
        let names = subpath
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["same-host", "under-subpath"], "同主机即命中：SSH 风格的远端不带子路径前缀");
        let paths = subpath
            .iter()
            .map(|m| m.project_path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, ["team/demo", "team/second"], "带子路径的实例应剥掉前缀");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn fetches_merge_request_page_over_http() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0u8; 8192];
            let read = stream.read(&mut buffer).unwrap();
            sender
                .send(String::from_utf8_lossy(&buffer[..read]).to_string())
                .unwrap();
            let body = r#"[{"id":11,"iid":7,"title":"Graph","state":"opened","source_branch":"feature","target_branch":"main","web_url":"https://gitlab.example.com/team/repo/-/merge_requests/7","author":{"name":"Lin Wei","username":"lin"}}]"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nX-Next-Page: 2\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
        });

        let client = reqwest::Client::new();
        let page = fetch_mr_page(
            &client,
            &format!("http://{address}"),
            "secret-token",
            "team/repo",
            "opened",
            Some("graph"),
            1,
            30,
        )
        .await
        .unwrap();

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].iid, 7);
        assert!(page.has_more);

        let request = receiver.recv().unwrap();
        let lower = request.to_lowercase();
        assert!(request.contains("GET /projects/team%2Frepo/merge_requests?"));
        assert!(request.contains("state=opened"));
        assert!(request.contains("search=graph"));
        assert!(lower.contains("private-token: secret-token"));
    }

    #[tokio::test]
    async fn fetches_branches_without_unsupported_sort_params() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0u8; 8192];
            let read = stream.read(&mut buffer).unwrap();
            sender
                .send(String::from_utf8_lossy(&buffer[..read]).to_string())
                .unwrap();
            let body = r#"[{"name":"main","default":true,"merged":false,"protected":true},{"name":"feature","default":false,"merged":false,"protected":false}]"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
        });

        let client = reqwest::Client::new();
        let branches = fetch_branches(
            &client,
            &format!("http://{address}"),
            "secret-token",
            "team/repo",
        )
        .await
        .unwrap();

        assert_eq!(branches.len(), 2);
        assert_eq!(branches[0].name, "main");
        assert!(branches[0].is_default);

        let request = receiver.recv().unwrap();
        assert!(request.contains("GET /projects/team%2Frepo/repository/branches?per_page=100"));
        assert!(!request.contains("sort="));
        assert!(!request.contains("order_by="));
    }
}
