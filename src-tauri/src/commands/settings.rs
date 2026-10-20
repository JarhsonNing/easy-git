use serde::{Deserialize, Serialize};
use std::time::Duration;

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))
}

fn truncate(body: &str, limit: usize) -> String {
    let trimmed = body.trim();
    if trimmed.chars().count() <= limit {
        trimmed.to_string()
    } else {
        format!("{}…", trimmed.chars().take(limit).collect::<String>())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitLabUser {
    pub id: u64,
    pub username: String,
    pub name: String,
    #[serde(default)]
    pub web_url: String,
}

#[tauri::command]
pub async fn test_gitlab(
    base_url: String,
    token: Option<String>,
    account: Option<String>,
) -> Result<GitLabUser, String> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err("请先填写 GitLab 实例地址".to_string());
    }

    let token = match token.filter(|t| !t.trim().is_empty()) {
        Some(t) => t,
        None => match account {
            Some(acc) => crate::config::secrets::read_secret(&acc)?
                .ok_or_else(|| "未找到已保存的访问令牌".to_string())?,
            None => return Err("请先填写访问令牌".to_string()),
        },
    };

    let response = http_client()?
        .get(format!("{base}/api/v4/user"))
        .header("PRIVATE-TOKEN", token.trim())
        .send()
        .await
        .map_err(|e| format!("连接 GitLab 失败: {e}"))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "GitLab 返回 HTTP {status}: {}",
            truncate(&body, 200)
        ));
    }

    response
        .json::<GitLabUser>()
        .await
        .map_err(|e| format!("解析 GitLab 响应失败: {e}"))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiModels {
    pub models: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModel {
    id: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelList {
    #[serde(default)]
    data: Vec<OpenAiModel>,
}

#[tauri::command]
pub async fn test_ai(
    base_url: String,
    api_key: Option<String>,
    account: Option<String>,
) -> Result<AiModels, String> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err("请先填写模型服务地址".to_string());
    }

    let api_key = match api_key.filter(|k| !k.trim().is_empty()) {
        Some(k) => Some(k),
        None => match account {
            Some(acc) => crate::config::secrets::read_secret(&acc)?,
            None => None,
        },
    };

    let mut request = http_client()?.get(format!("{base}/models"));
    if let Some(key) = api_key {
        request = request.bearer_auth(key.trim());
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("连接模型服务失败: {e}"))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "模型服务返回 HTTP {status}: {}",
            truncate(&body, 200)
        ));
    }

    let list = response
        .json::<OpenAiModelList>()
        .await
        .map_err(|e| format!("解析模型列表失败: {e}"))?;

    Ok(AiModels {
        models: list.data.into_iter().map(|m| m.id).take(100).collect(),
    })
}
