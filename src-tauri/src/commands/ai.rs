use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::ipc::Channel;
use tauri::State;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Default)]
pub struct AiState(Mutex<Option<Arc<AtomicBool>>>);

fn begin(state: &AiState) -> Result<Arc<AtomicBool>, String> {
    let flag = Arc::new(AtomicBool::new(false));
    let mut guard = state.0.lock().map_err(|_| "AI 状态不可用".to_string())?;
    if let Some(current) = guard.as_ref() {
        current.store(true, Ordering::Relaxed);
    }
    *guard = Some(flag.clone());
    Ok(flag)
}

fn finish(state: &AiState, flag: &Arc<AtomicBool>) {
    if let Ok(mut guard) = state.0.lock() {
        if guard
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, flag))
        {
            *guard = None;
        }
    }
}

#[tauri::command]
pub fn cancel_ai(state: State<'_, AiState>) {
    if let Ok(guard) = state.0.lock() {
        if let Some(flag) = guard.as_ref() {
            flag.store(true, Ordering::Relaxed);
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AiEvent {
    Delta { text: String },
    Done { cancelled: bool },
}

#[derive(Debug, Deserialize, Default)]
struct StreamDelta {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct StreamChoice {
    #[serde(default)]
    delta: StreamDelta,
    #[serde(default)]
    message: Option<StreamDelta>,
}

#[derive(Debug, Deserialize, Default)]
struct StreamError {
    #[serde(default)]
    message: String,
}

#[derive(Debug, Deserialize, Default)]
struct StreamChunk {
    #[serde(default)]
    choices: Vec<StreamChoice>,
    #[serde(default)]
    error: Option<StreamError>,
}

enum SseData {
    Done,
    Text(String),
    Ignore,
}

fn parse_sse_data(data: &str) -> Result<SseData, String> {
    if data == "[DONE]" {
        return Ok(SseData::Done);
    }
    let chunk: StreamChunk =
        serde_json::from_str(data).map_err(|e| format!("解析模型流失败: {e}"))?;
    if let Some(error) = chunk.error {
        if !error.message.is_empty() {
            return Err(format!("模型服务返回错误: {}", error.message));
        }
    }
    let mut text = String::new();
    for choice in chunk.choices {
        if let Some(content) = choice.delta.content {
            text.push_str(&content);
        }
        if let Some(content) = choice.message.and_then(|message| message.content) {
            text.push_str(&content);
        }
    }
    if text.is_empty() {
        Ok(SseData::Ignore)
    } else {
        Ok(SseData::Text(text))
    }
}

fn chat_endpoint(base_url: &str) -> Result<String, String> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err("请先在设置中填写模型服务地址".to_string());
    }
    let parsed = url::Url::parse(base).map_err(|e| format!("模型服务地址无效: {e}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("模型服务地址必须使用 http 或 https".to_string());
    }
    if base.ends_with("/chat/completions") {
        Ok(base.to_string())
    } else {
        Ok(format!("{base}/chat/completions"))
    }
}

fn resolve_profile(profile_id: Option<&str>) -> Result<crate::config::schema::AiProfile, String> {
    let settings = crate::config::get_settings()?;
    let active_id = settings.ai.active_profile_id.clone();
    let profiles = settings.ai.profiles;
    if profiles.is_empty() {
        return Err("还没有配置 AI 模型，请先在设置中添加模型".to_string());
    }
    let wanted = profile_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or(active_id);
    let profile = match wanted {
        Some(id) => profiles
            .into_iter()
            .find(|profile| profile.id == id)
            .ok_or_else(|| "找不到该 AI 模型配置，请刷新设置后重试".to_string())?,
        None => profiles.into_iter().next().expect("profiles is not empty"),
    };
    if profile.model.trim().is_empty() {
        return Err("模型 ID 为空，请先在设置中填写模型".to_string());
    }
    Ok(profile)
}

fn validate_messages(messages: &[ChatMessage]) -> Result<(), String> {
    if messages.is_empty() {
        return Err("消息不能为空".to_string());
    }
    if messages.len() > 60 {
        return Err("消息数量过多，请新建一次对话".to_string());
    }
    let mut total = 0usize;
    for message in messages {
        if !matches!(message.role.as_str(), "system" | "user" | "assistant") {
            return Err(format!("不支持的消息角色：{}", message.role));
        }
        total += message.content.chars().count();
    }
    if total > 200_000 {
        return Err("上下文超过 200k 字符，请移除部分上下文后重试".to_string());
    }
    Ok(())
}

fn truncate(body: &str, limit: usize) -> String {
    let trimmed = body.trim();
    if trimmed.chars().count() <= limit {
        trimmed.to_string()
    } else {
        format!("{}…", trimmed.chars().take(limit).collect::<String>())
    }
}

fn api_error(status: reqwest::StatusCode, body: &str) -> String {
    match status.as_u16() {
        401 | 403 => format!("模型服务鉴权失败（HTTP {status}），请检查 API Key"),
        404 => "模型服务接口不存在（HTTP 404），请确认服务地址以 /v1 结尾".to_string(),
        429 => "模型服务请求过于频繁（HTTP 429），请稍后重试".to_string(),
        _ => format!("模型服务返回 HTTP {status}: {}", truncate(body, 240)),
    }
}

async fn stream_completion(
    client: &reqwest::Client,
    endpoint: &str,
    token: Option<&str>,
    profile: &crate::config::schema::AiProfile,
    messages: &[ChatMessage],
    cancelled: &AtomicBool,
    mut on_delta: impl FnMut(String),
) -> Result<bool, String> {
    if cancelled.load(Ordering::Relaxed) {
        return Ok(true);
    }

    let payload = json!({
        "model": profile.model,
        "messages": messages,
        "stream": true,
        "temperature": profile.temperature.clamp(0.0, 2.0),
        "max_tokens": profile.max_tokens.max(1),
    });

    let mut request = client
        .post(endpoint)
        .header("Accept", "text/event-stream")
        .json(&payload);
    if let Some(token) = token.filter(|value| !value.trim().is_empty()) {
        request = request.bearer_auth(token.trim());
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("连接模型服务失败: {e}"))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(api_error(status, &body));
    }

    let streaming = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().contains("text/event-stream"));

    if !streaming {
        let body = response
            .text()
            .await
            .map_err(|e| format!("读取模型响应失败: {e}"))?;
        if let SseData::Text(text) = parse_sse_data(body.trim())? {
            on_delta(text);
        }
        return Ok(cancelled.load(Ordering::Relaxed));
    }

    let mut stream = response.bytes_stream();
    let mut buffer: Vec<u8> = Vec::new();

    'outer: while let Some(chunk) = stream.next().await {
        if cancelled.load(Ordering::Relaxed) {
            return Ok(true);
        }
        let chunk = chunk.map_err(|e| format!("读取模型流失败: {e}"))?;
        buffer.extend_from_slice(&chunk);

        while let Some(position) = buffer.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = buffer.drain(..=position).collect();
            let line = String::from_utf8_lossy(&line);
            let line = line.trim_end_matches('\n').trim_end_matches('\r');
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data.is_empty() {
                continue;
            }
            match parse_sse_data(data)? {
                SseData::Done => {
                    break 'outer;
                }
                SseData::Text(text) => on_delta(text),
                SseData::Ignore => {}
            }
        }
    }

    Ok(cancelled.load(Ordering::Relaxed))
}

#[tauri::command]
pub async fn chat_stream(
    state: State<'_, AiState>,
    profile_id: Option<String>,
    messages: Vec<ChatMessage>,
    on_event: Channel<AiEvent>,
) -> Result<(), String> {
    let profile = resolve_profile(profile_id.as_deref())?;
    validate_messages(&messages)?;
    let endpoint = chat_endpoint(&profile.base_url)?;

    let flag = begin(&state)?;
    let outcome = async {
        let token = crate::config::secrets::read_secret(&format!("ai:{}", profile.id))?
            .filter(|value| !value.trim().is_empty());
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(300))
            .user_agent("easy-git/0.1")
            .build()
            .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;
        stream_completion(
            &client,
            &endpoint,
            token.as_deref(),
            &profile,
            &messages,
            &flag,
            |text| {
                let _ = on_event.send(AiEvent::Delta { text });
            },
        )
        .await
    }
    .await;
    finish(&state, &flag);

    let cancelled = outcome?;
    let _ = on_event.send(AiEvent::Done { cancelled });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn builds_chat_endpoints() {
        assert_eq!(
            chat_endpoint("http://127.0.0.1:11434/v1/").unwrap(),
            "http://127.0.0.1:11434/v1/chat/completions"
        );
        assert_eq!(
            chat_endpoint("https://api.example.com/v1/chat/completions").unwrap(),
            "https://api.example.com/v1/chat/completions"
        );
        assert!(chat_endpoint("ftp://example.com").is_err());
        assert!(chat_endpoint("  ").is_err());
    }

    #[test]
    fn parses_stream_events() {
        let text = parse_sse_data(r#"{"choices":[{"delta":{"content":"你好"}}]}"#).unwrap();
        assert!(matches!(text, SseData::Text(value) if value == "你好"));
        assert!(matches!(
            parse_sse_data(r#"{"choices":[{"delta":{"content":""}}]}"#).unwrap(),
            SseData::Ignore
        ));
        assert!(matches!(parse_sse_data("[DONE]").unwrap(), SseData::Done));
        assert!(parse_sse_data(r#"{"error":{"message":"bad key"}}"#).is_err());
        assert!(parse_sse_data("not-json").is_err());
    }

    #[test]
    fn validates_messages() {
        assert!(validate_messages(&[]).is_err());
        assert!(validate_messages(&[ChatMessage {
            role: "tool".to_string(),
            content: "x".to_string(),
        }])
        .is_err());
        assert!(validate_messages(&[ChatMessage {
            role: "user".to_string(),
            content: "hi".to_string(),
        }])
        .is_ok());
    }

    #[tokio::test]
    async fn streams_sse_content_from_an_openai_compatible_server() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind server");
        let address = listener.local_addr().expect("address");
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().expect("accept");
            let mut request = [0u8; 8192];
            let _ = socket.read(&mut request);
            let body = concat!(
                "data: {\"choices\":[{\"delta\":{\"content\":\"你\"}}]}\n\n",
                "data: {\"choices\":[{\"delta\":{\"content\":\"好\"}}]}\n\n",
                "data: [DONE]\n\n"
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.as_bytes().len(),
                body
            );
            socket.write_all(response.as_bytes()).expect("write response");
        });

        let profile = crate::config::schema::AiProfile {
            id: "test".to_string(),
            name: "test".to_string(),
            base_url: format!("http://{address}/v1"),
            model: "test-model".to_string(),
            temperature: 0.2,
            max_tokens: 128,
        };
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: "hello".to_string(),
        }];
        let cancelled = AtomicBool::new(false);
        let mut chunks = Vec::new();
        let client = reqwest::Client::new();
        let outcome = stream_completion(
            &client,
            &chat_endpoint(&profile.base_url).unwrap(),
            None,
            &profile,
            &messages,
            &cancelled,
            |text| chunks.push(text),
        )
        .await
        .expect("stream");

        assert!(!outcome);
        assert_eq!(chunks, vec!["你".to_string(), "好".to_string()]);
        server.join().expect("server thread");
    }
}
