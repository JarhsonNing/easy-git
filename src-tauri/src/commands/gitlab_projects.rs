use serde::{Deserialize, Serialize};

use crate::commands::gitlab::{
    api_error, encode_component, null_str, parse_response, prepare, ApiContext,
};

const PER_PAGE: u32 = 30;
const FILE_LIMIT: usize = 256 * 1024;
const TRACE_LIMIT: usize = 256 * 1024;

fn next_page_header(response: &reqwest::Response) -> Option<bool> {
    response
        .headers()
        .get("x-next-page")
        .and_then(|value| value.to_str().ok())
        .map(|value| !value.trim().is_empty())
}

#[derive(Debug, Deserialize, Default)]
struct RawForkParent {
    #[serde(default, with = "null_str")]
    path_with_namespace: String,
    #[serde(default, with = "null_str")]
    web_url: String,
}

#[derive(Debug, Deserialize, Default)]
struct RawProject {
    #[serde(default)]
    id: u64,
    #[serde(default, with = "null_str")]
    name: String,
    #[serde(default, with = "null_str")]
    name_with_namespace: String,
    #[serde(default, with = "null_str")]
    path_with_namespace: String,
    #[serde(default, with = "null_str")]
    description: String,
    #[serde(default, with = "null_str")]
    web_url: String,
    #[serde(default, with = "null_str")]
    default_branch: String,
    #[serde(default, with = "null_str")]
    visibility: String,
    #[serde(default, with = "null_str")]
    last_activity_at: String,
    #[serde(default)]
    archived: bool,
    #[serde(default)]
    forked_from_project: Option<RawForkParent>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GitLabProject {
    pub id: u64,
    pub name: String,
    pub name_with_namespace: String,
    pub path: String,
    pub description: String,
    pub web_url: String,
    pub default_branch: String,
    pub visibility: String,
    pub last_activity_at: String,
    pub archived: bool,
    pub forked_from_path: Option<String>,
    pub forked_from_web_url: Option<String>,
}

impl From<RawProject> for GitLabProject {
    fn from(raw: RawProject) -> Self {
        let fork = raw.forked_from_project;
        Self {
            id: raw.id,
            name: raw.name,
            name_with_namespace: raw.name_with_namespace,
            path: raw.path_with_namespace,
            description: raw.description,
            web_url: raw.web_url,
            default_branch: raw.default_branch,
            visibility: raw.visibility,
            last_activity_at: raw.last_activity_at,
            archived: raw.archived,
            forked_from_path: fork.as_ref().map(|value| value.path_with_namespace.clone()),
            forked_from_web_url: fork.as_ref().map(|value| value.web_url.clone()),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitLabProjectPage {
    pub items: Vec<GitLabProject>,
    pub page: u32,
    pub has_more: bool,
}

#[tauri::command]
pub async fn list_gitlab_projects(
    instance_id: String,
    search: Option<String>,
    page: u32,
) -> Result<GitLabProjectPage, String> {
    let context = prepare(&instance_id)?;
    let mut params: Vec<(&str, String)> = vec![
        ("membership", "true".to_string()),
        ("order_by", "last_activity_at".to_string()),
        ("sort", "desc".to_string()),
        ("per_page", PER_PAGE.to_string()),
        ("page", page.max(1).to_string()),
    ];
    if let Some(term) = search.as_deref().map(str::trim).filter(|term| !term.is_empty()) {
        params.push(("search", term.to_string()));
    }

    let response = context
        .client
        .get(format!("{}/projects", context.base))
        .header("PRIVATE-TOKEN", &context.token)
        .query(&params)
        .send()
        .await
        .map_err(|e| format!("连接 GitLab 失败: {e}"))?;
    let has_more = next_page_header(&response);
    let raw = parse_response::<Vec<RawProject>>(response).await?;
    let has_more = has_more.unwrap_or(raw.len() >= PER_PAGE as usize);
    Ok(GitLabProjectPage {
        items: raw.into_iter().map(GitLabProject::from).collect(),
        page,
        has_more,
    })
}

#[tauri::command]
pub async fn get_gitlab_project(
    instance_id: String,
    project_path: String,
) -> Result<GitLabProject, String> {
    let context = prepare(&instance_id)?;
    fetch_project(&context, &project_path).await
}

async fn fetch_project(context: &ApiContext, project_path: &str) -> Result<GitLabProject, String> {
    let response = context
        .client
        .get(format!(
            "{}/projects/{}",
            context.base,
            encode_component(project_path)
        ))
        .header("PRIVATE-TOKEN", &context.token)
        .send()
        .await
        .map_err(|e| format!("连接 GitLab 失败: {e}"))?;
    let raw = parse_response::<RawProject>(response).await?;
    Ok(raw.into())
}

#[derive(Debug, Deserialize)]
struct RawTreeEntry {
    #[serde(default, with = "null_str")]
    name: String,
    #[serde(rename = "type", default, with = "null_str")]
    kind: String,
    #[serde(default, with = "null_str")]
    path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitLabTreeEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitLabTreePage {
    pub entries: Vec<GitLabTreeEntry>,
    pub has_more: bool,
}

#[tauri::command]
pub async fn list_repository_tree(
    instance_id: String,
    project_path: String,
    git_ref: Option<String>,
    path: Option<String>,
    page: u32,
) -> Result<GitLabTreePage, String> {
    let context = prepare(&instance_id)?;
    let mut params: Vec<(&str, String)> = vec![
        ("per_page", "100".to_string()),
        ("page", page.max(1).to_string()),
    ];
    if let Some(git_ref) = git_ref.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        params.push(("ref", git_ref.to_string()));
    }
    if let Some(path) = path.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        params.push(("path", path.to_string()));
    }

    let response = context
        .client
        .get(format!(
            "{}/projects/{}/repository/tree",
            context.base,
            encode_component(&project_path)
        ))
        .header("PRIVATE-TOKEN", &context.token)
        .query(&params)
        .send()
        .await
        .map_err(|e| format!("连接 GitLab 失败: {e}"))?;
    let has_more = next_page_header(&response);
    let raw = parse_response::<Vec<RawTreeEntry>>(response).await?;
    let has_more = has_more.unwrap_or(raw.len() >= 100);
    let mut entries: Vec<GitLabTreeEntry> = raw
        .into_iter()
        .map(|entry| GitLabTreeEntry {
            name: entry.name,
            is_dir: entry.kind == "tree",
            path: entry.path,
        })
        .collect();
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(GitLabTreePage { entries, has_more })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitLabFileContent {
    pub path: String,
    pub content: String,
    pub binary: bool,
    pub truncated: bool,
    pub size: usize,
}

#[tauri::command]
pub async fn read_gitlab_file(
    instance_id: String,
    project_path: String,
    file_path: String,
    git_ref: Option<String>,
) -> Result<GitLabFileContent, String> {
    let context = prepare(&instance_id)?;
    let mut params: Vec<(&str, String)> = Vec::new();
    if let Some(git_ref) = git_ref.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        params.push(("ref", git_ref.to_string()));
    }
    let response = context
        .client
        .get(format!(
            "{}/projects/{}/repository/files/{}/raw",
            context.base,
            encode_component(&project_path),
            encode_component(&file_path)
        ))
        .header("PRIVATE-TOKEN", &context.token)
        .query(&params)
        .send()
        .await
        .map_err(|e| format!("连接 GitLab 失败: {e}"))?;
    if !response.status().is_success() {
        return Err(api_error(response).await);
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("读取文件失败: {e}"))?;
    let size = bytes.len();
    match String::from_utf8(bytes.to_vec()) {
        Ok(text) => {
            let truncated = text.len() > FILE_LIMIT;
            let content = if truncated {
                text.chars().take(FILE_LIMIT).collect()
            } else {
                text
            };
            Ok(GitLabFileContent {
                path: file_path,
                content,
                binary: false,
                truncated,
                size,
            })
        }
        Err(_) => Ok(GitLabFileContent {
            path: file_path,
            content: String::new(),
            binary: true,
            truncated: false,
            size,
        }),
    }
}

#[derive(Debug, Deserialize, Default)]
struct RawPipeline {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    iid: u64,
    #[serde(default, with = "null_str")]
    status: String,
    #[serde(default, with = "null_str")]
    source: String,
    #[serde(default, rename = "ref", with = "null_str")]
    ref_name: String,
    #[serde(default, with = "null_str")]
    sha: String,
    #[serde(default, with = "null_str")]
    web_url: String,
    #[serde(default, with = "null_str")]
    created_at: String,
    #[serde(default, with = "null_str")]
    updated_at: String,
    #[serde(default)]
    duration: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitLabPipeline {
    pub id: u64,
    pub iid: u64,
    pub status: String,
    pub source: String,
    pub ref_name: String,
    pub sha: String,
    pub web_url: String,
    pub created_at: String,
    pub updated_at: String,
    pub duration: Option<u64>,
}

impl From<RawPipeline> for GitLabPipeline {
    fn from(raw: RawPipeline) -> Self {
        Self {
            id: raw.id,
            iid: raw.iid,
            status: raw.status,
            source: raw.source,
            ref_name: raw.ref_name,
            sha: raw.sha,
            web_url: raw.web_url,
            created_at: raw.created_at,
            updated_at: raw.updated_at,
            duration: raw.duration,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitLabPipelinePage {
    pub items: Vec<GitLabPipeline>,
    pub page: u32,
    pub has_more: bool,
}

#[tauri::command]
pub async fn list_pipelines(
    instance_id: String,
    project_path: String,
    page: u32,
) -> Result<GitLabPipelinePage, String> {
    let context = prepare(&instance_id)?;
    let response = context
        .client
        .get(format!(
            "{}/projects/{}/pipelines",
            context.base,
            encode_component(&project_path)
        ))
        .header("PRIVATE-TOKEN", &context.token)
        .query(&[
            ("per_page", PER_PAGE.to_string()),
            ("page", page.max(1).to_string()),
        ])
        .send()
        .await
        .map_err(|e| format!("连接 GitLab 失败: {e}"))?;
    let has_more = next_page_header(&response);
    let raw = parse_response::<Vec<RawPipeline>>(response).await?;
    let has_more = has_more.unwrap_or(raw.len() >= PER_PAGE as usize);
    Ok(GitLabPipelinePage {
        items: raw.into_iter().map(GitLabPipeline::from).collect(),
        page,
        has_more,
    })
}

/// GitLab 用 `POST /projects/:id/pipeline` 触发一条新流水线，只接受单个 `ref`。
#[tauri::command]
pub async fn create_gitlab_pipeline(
    instance_id: String,
    project_path: String,
    branch: String,
) -> Result<GitLabPipeline, String> {
    let branch = branch.trim();
    if branch.is_empty() {
        return Err("请选择要运行流水线的分支".to_string());
    }
    let context = prepare(&instance_id)?;
    let response = context
        .client
        .post(format!(
            "{}/projects/{}/pipeline",
            context.base,
            encode_component(&project_path)
        ))
        .header("PRIVATE-TOKEN", &context.token)
        .query(&[("ref", branch)])
        .send()
        .await
        .map_err(|e| format!("连接 GitLab 失败: {e}"))?;
    let status = response.status();
    if !status.is_success() {
        let detail = api_error(response).await;
        return Err(if status.as_u16() == 403 {
            format!("{detail}；创建流水线需要令牌具备 api 作用域")
        } else {
            detail
        });
    }
    let raw = parse_response::<RawPipeline>(response).await?;
    if raw.id == 0 {
        return Err("GitLab 没有返回新建流水线的标识".to_string());
    }
    Ok(GitLabPipeline::from(raw))
}

#[derive(Debug, Deserialize, Default)]
struct RawJobRunner {
    #[serde(default, with = "null_str")]
    description: String,
}

#[derive(Debug, Deserialize, Default)]
struct RawJob {
    #[serde(default)]
    id: u64,
    #[serde(default, with = "null_str")]
    name: String,
    #[serde(default, with = "null_str")]
    stage: String,
    #[serde(default, with = "null_str")]
    status: String,
    #[serde(default, with = "null_str")]
    web_url: String,
    #[serde(default, with = "null_str")]
    created_at: String,
    #[serde(default)]
    started_at: Option<String>,
    #[serde(default)]
    finished_at: Option<String>,
    #[serde(default)]
    duration: Option<u64>,
    #[serde(default)]
    runner: Option<RawJobRunner>,
    #[serde(default)]
    failure_reason: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitLabJob {
    pub id: u64,
    pub name: String,
    pub stage: String,
    pub status: String,
    pub web_url: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub duration: Option<u64>,
    pub runner: String,
    pub failure_reason: Option<String>,
}

#[tauri::command]
pub async fn list_pipeline_jobs(
    instance_id: String,
    project_path: String,
    pipeline_id: u64,
) -> Result<Vec<GitLabJob>, String> {
    let context = prepare(&instance_id)?;
    let response = context
        .client
        .get(format!(
            "{}/projects/{}/pipelines/{pipeline_id}/jobs",
            context.base,
            encode_component(&project_path)
        ))
        .header("PRIVATE-TOKEN", &context.token)
        .query(&[("per_page", "100")])
        .send()
        .await
        .map_err(|e| format!("连接 GitLab 失败: {e}"))?;
    let raw = parse_response::<Vec<RawJob>>(response).await?;
    Ok(raw
        .into_iter()
        .map(|job| GitLabJob {
            id: job.id,
            name: job.name,
            stage: job.stage,
            status: job.status,
            web_url: job.web_url,
            created_at: job.created_at,
            started_at: job.started_at,
            finished_at: job.finished_at,
            duration: job.duration,
            runner: job.runner.map(|runner| runner.description).unwrap_or_default(),
            failure_reason: job.failure_reason,
        })
        .collect())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitLabJobTrace {
    pub trace: String,
    pub truncated: bool,
    pub size: usize,
}

#[tauri::command]
pub async fn get_job_trace(
    instance_id: String,
    project_path: String,
    job_id: u64,
) -> Result<GitLabJobTrace, String> {
    let context = prepare(&instance_id)?;
    let response = context
        .client
        .get(format!(
            "{}/projects/{}/jobs/{job_id}/trace",
            context.base,
            encode_component(&project_path)
        ))
        .header("PRIVATE-TOKEN", &context.token)
        .send()
        .await
        .map_err(|e| format!("连接 GitLab 失败: {e}"))?;
    if response.status().as_u16() == 404 {
        return Ok(GitLabJobTrace {
            trace: String::new(),
            truncated: false,
            size: 0,
        });
    }
    if !response.status().is_success() {
        return Err(api_error(response).await);
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("读取日志失败: {e}"))?;
    let size = bytes.len();
    let text = String::from_utf8_lossy(&bytes).to_string();
    let truncated = text.len() > TRACE_LIMIT;
    let trace = if truncated {
        let skip = text.len() - TRACE_LIMIT;
        let tail = &text[skip..];
        format!("…{}", tail.trim_start_matches('\n'))
    } else {
        text
    };
    Ok(GitLabJobTrace {
        trace,
        truncated,
        size,
    })
}

#[derive(Debug, Serialize)]
struct SyncForkQuery<'a> {
    query: &'a str,
    variables: SyncForkVariables<'a>,
}

#[derive(Debug, Serialize)]
struct SyncForkVariables<'a> {
    project_path: &'a str,
    target_branch: &'a str,
}

#[derive(Debug, Deserialize)]
struct SyncForkPayload {
    #[serde(default)]
    errors: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SyncForkData {
    #[serde(default)]
    project_sync_fork: Option<SyncForkPayload>,
}

#[derive(Debug, Deserialize)]
struct SyncForkResponse {
    #[serde(default)]
    data: Option<SyncForkData>,
    #[serde(default)]
    errors: Vec<serde_json::Value>,
}

#[tauri::command]
pub async fn sync_gitlab_fork(
    instance_id: String,
    project_path: String,
    target_branch: String,
) -> Result<String, String> {
    let branch = target_branch.trim();
    if branch.is_empty() {
        return Err("请选择要同步的分支".to_string());
    }
    let context = prepare(&instance_id)?;
    let graphql = format!(
        "{}/api/graphql",
        context
            .base
            .strip_suffix("/api/v4")
            .unwrap_or(&context.base)
            .trim_end_matches('/')
    );
    let query = "mutation($projectPath: ID!, $targetBranch: String!) { projectSyncFork(input: {projectPath: $projectPath, targetBranch: $targetBranch}) { errors } }";
    let response = context
        .client
        .post(graphql)
        .header("Authorization", format!("Bearer {}", context.token))
        .json(&SyncForkQuery {
            query,
            variables: SyncForkVariables {
                project_path: &project_path,
                target_branch: branch,
            },
        })
        .send()
        .await
        .map_err(|e| format!("连接 GitLab 失败: {e}"))?;
    if !response.status().is_success() {
        return Err(api_error(response).await);
    }
    let body = parse_response::<SyncForkResponse>(response).await?;
    if !body.errors.is_empty() {
        return Err(format!(
            "同步复刻失败: {}",
            body
                .errors
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join("；")
        ));
    }
    let errors = body
        .data
        .and_then(|data| data.project_sync_fork)
        .map(|payload| payload.errors)
        .unwrap_or_default();
    if !errors.is_empty() {
        return Err(format!("同步复刻失败: {}", errors.join("；")));
    }
    Ok(format!("已提交同步请求：{branch} 正在同步上游最新提交"))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneResult {
    pub path: String,
    pub name: String,
}

fn sanitize_segment(value: &str) -> String {
    let cleaned: String = value
        .chars()
        .map(|ch| {
            if matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
                || ch.is_control()
            {
                '-'
            } else {
                ch
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').trim();
    if trimmed.is_empty() {
        "repo".to_string()
    } else {
        trimmed.to_string()
    }
}

#[tauri::command]
pub async fn clone_gitlab_project(
    instance_id: String,
    project_path: String,
    parent_dir: String,
) -> Result<CloneResult, String> {
    let context = prepare(&instance_id)?;
    let base = context
        .base
        .strip_suffix("/api/v4")
        .unwrap_or(&context.base)
        .trim_end_matches('/')
        .to_string();
    let clone_url = format!("{base}/{project_path}.git");
    let segment = project_path
        .rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or("repo");
    let target = std::path::Path::new(&parent_dir).join(sanitize_segment(segment));
    if target.exists()
        && std::fs::read_dir(&target)
            .map(|mut entries| entries.next().is_some())
            .unwrap_or(false)
    {
        return Err(format!(
            "目标目录 {} 已存在且不为空，请换一个位置。",
            target.to_string_lossy()
        ));
    }

    let token = context.token;
    let path = target.to_string_lossy().to_string();
    let name = target
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| segment.to_string());
    tauri::async_runtime::spawn_blocking(move || {
        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(move |_url, _username, _allowed| {
            git2::Cred::userpass_plaintext("oauth2", &token)
        });
        let mut fetch_options = git2::FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);
        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_options);
        builder
            .clone(&clone_url, &target)
            .map_err(|e| format!("克隆失败: {}", e.message()))?;
        Ok::<_, String>(())
    })
    .await
    .map_err(|e| format!("克隆任务失败: {e}"))??;

    Ok(CloneResult { path, name })
}

#[cfg(test)]
mod tests {
    use super::{
        sanitize_segment, GitLabPipeline, GitLabProject, RawPipeline, RawProject, RawTreeEntry,
    };

    #[test]
    fn project_list_tolerates_null_fields() {
        // 真实 GitLab 用 null 而不是省略键：没有描述、空仓库没有默认分支、不是复刻。
        let raw: Vec<RawProject> = serde_json::from_str(
            r#"[{
                "id": 7, "name": "demo", "name_with_namespace": "Group / demo",
                "path_with_namespace": "group/demo", "description": null,
                "web_url": "http://gl/group/demo", "default_branch": null,
                "visibility": "private", "last_activity_at": "2026-10-21T10:00:00.000Z",
                "archived": false, "forked_from_project": null, "owner": null
            }]"#,
        )
        .expect("含 null 的项目列表应能解析");
        let project = GitLabProject::from(raw.into_iter().next().unwrap());
        assert_eq!(project.description, "");
        assert_eq!(project.default_branch, "");
        assert_eq!(project.path, "group/demo");
        assert_eq!(project.forked_from_path, None);
    }

    #[test]
    fn pipeline_reads_ref_key() {
        let raw: RawPipeline =
            serde_json::from_str(r#"{"iid":3,"status":"running","ref":"feature/x"}"#).unwrap();
        let pipeline = GitLabPipeline::from(raw);
        assert_eq!(pipeline.ref_name, "feature/x");
    }

    #[test]
    fn tree_entry_reads_type_key() {
        let dir: RawTreeEntry =
            serde_json::from_str(r#"{"name":"src","type":"tree","path":"src"}"#).unwrap();
        assert_eq!(dir.kind, "tree");
        assert_eq!(dir.path, "src");
    }

    #[test]
    fn clone_folder_name_is_filesystem_safe() {
        assert_eq!(sanitize_segment("my:repo<>|"), "my-repo---");
        assert_eq!(sanitize_segment(".."), "repo");
        assert_eq!(sanitize_segment("demo.git"), "demo.git");
    }
}
