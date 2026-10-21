use std::path::Path;

use serde::Serialize;
use tauri::State;

use super::branch::{ensure_remote, finish, reject_dash, run_git};
use super::repo::{OperationResult, RepoStore, lock_ops, read_refs, with_open, workdir};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInfo {
    pub name: String,
    pub fetch_url: String,
    pub push_url: String,
    pub has_credentials: bool,
}

fn scheme_and_authority(url: &str) -> Option<(String, usize, usize)> {
    let scheme_end = url.find("://")?;
    let start = scheme_end + 3;
    let end = url[start..]
        .find(['/', '?', '#'])
        .map(|index| start + index)
        .unwrap_or(url.len());
    Some((url[..scheme_end].to_ascii_lowercase(), start, end))
}

fn url_has_credentials(url: &str) -> bool {
    if let Some((scheme, start, end)) = scheme_and_authority(url) {
        let authority = &url[start..end];
        if let Some(at) = authority.rfind('@') {
            return match scheme.as_str() {
                "http" | "https" => true,
                _ => authority[..at].contains(':'),
            };
        }
        return false;
    }
    if let Some(at) = url.rfind('@') {
        let before = &url[..at];
        return before.contains(':') && !before.contains('/') && !before.contains('\\');
    }
    false
}

fn redact_remote_url(url: &str) -> String {
    if let Some((scheme, start, end)) = scheme_and_authority(url) {
        let authority = &url[start..end];
        if let Some(at) = authority.rfind('@') {
            let sensitive = match scheme.as_str() {
                "http" | "https" => true,
                _ => authority[..at].contains(':'),
            };
            if sensitive {
                return format!(
                    "{}***@{}{}",
                    &url[..start],
                    &authority[at + 1..],
                    &url[end..]
                );
            }
        }
        return url.to_string();
    }
    if let Some(at) = url.rfind('@') {
        let before = &url[..at];
        if before.contains(':') && !before.contains('/') && !before.contains('\\') {
            return format!("***@{}", &url[at + 1..]);
        }
    }
    url.to_string()
}

fn looks_like_windows_path(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() > 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn looks_like_local_path(value: &str) -> bool {
    value.starts_with('/')
        || value.starts_with("./")
        || value.starts_with("../")
        || value.starts_with("~/")
        || value.starts_with("\\\\")
        || looks_like_windows_path(value)
        || value.contains('/')
        || value.contains('\\')
}

fn validate_remote_url(raw: &str) -> Result<(), String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err("远程地址不能为空".to_string());
    }
    if value.len() > 2048 {
        return Err("远程地址过长".to_string());
    }
    if value.starts_with('-') {
        return Err("远程地址不能以 - 开头".to_string());
    }
    if value
        .chars()
        .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err("远程地址包含空白或控制字符".to_string());
    }
    if !value.contains("://") && value.contains("::") {
        return Err("不支持 ext:: 等扩展传输协议".to_string());
    }

    if let Some((scheme, start, end)) = scheme_and_authority(value) {
        let authority = &value[start..end];
        match scheme.as_str() {
            "http" | "https" => {
                if authority.contains('@') {
                    return Err(
                        "HTTP 远程地址不能包含用户名或密码，请使用凭据管理器或 SSH 地址"
                            .to_string(),
                    );
                }
                if authority.is_empty() {
                    return Err("远程地址缺少主机名".to_string());
                }
            }
            "ssh" => {
                let host = authority.rsplit('@').next().unwrap_or(authority);
                if host.is_empty() {
                    return Err("远程地址缺少主机名".to_string());
                }
                if let Some(at) = authority.rfind('@') {
                    if authority[..at].contains(':') {
                        return Err("远程地址不能包含明文密码，请使用 SSH 密钥".to_string());
                    }
                }
            }
            "git" => {
                if authority.contains('@') {
                    return Err("git:// 远程地址不能包含凭据".to_string());
                }
                if authority.is_empty() {
                    return Err("远程地址缺少主机名".to_string());
                }
            }
            "file" => {
                let path = &value[end..];
                if authority.is_empty() && path.is_empty() {
                    return Err("file 地址缺少路径".to_string());
                }
            }
            _ => return Err(format!("不支持的远程协议「{scheme}」")),
        }
        return Ok(());
    }

    // scp 风格：user@host:path
    if let Some(colon) = value.find(':') {
        let host_part = &value[..colon];
        let host = host_part.rsplit('@').next().unwrap_or(host_part);
        if !host_part.contains('/') && !host_part.contains('\\') && !host.is_empty() && colon > 1 {
            return Ok(());
        }
    }

    if looks_like_local_path(value) {
        return Ok(());
    }
    Err("无法识别的远程地址，请使用 HTTPS、SSH 或本地仓库路径".to_string())
}

fn validate_remote_name(dir: &Path, name: &str) -> Result<(), String> {
    reject_dash(name, "远程名")?;
    if name.len() > 100 {
        return Err("远程名过长".to_string());
    }
    if name.trim() != name || name.chars().any(char::is_whitespace) {
        return Err("远程名不能包含空白字符".to_string());
    }
    let reference = format!("refs/remotes/{name}");
    let outcome = run_git(dir, &["check-ref-format", reference.as_str()])?;
    if !outcome.ok {
        return Err(format!("远程名「{name}」不符合 Git 命名规则"));
    }
    Ok(())
}

fn ensure_remote_absent(store: &RepoStore, name: &str) -> Result<(), String> {
    if read_refs(store)?.remotes.iter().any(|item| item == name) {
        return Err(format!("远程「{name}」已存在"));
    }
    Ok(())
}

#[tauri::command]
pub fn list_remotes(store: State<'_, RepoStore>) -> Result<Vec<RemoteInfo>, String> {
    with_open(&store, |repo| {
        let names = repo
            .remotes()
            .map_err(|error| format!("无法读取远程列表: {}", error.message()))?;
        let mut result = Vec::new();
        for name in names.iter().filter_map(|name| name.ok().flatten()) {
            let remote = repo
                .find_remote(name)
                .map_err(|error| format!("无法读取远程「{name}」: {}", error.message()))?;
            let fetch_raw = remote.url().unwrap_or_default().to_string();
            let push_raw = remote
                .pushurl()
                .ok()
                .flatten()
                .unwrap_or(fetch_raw.as_str())
                .to_string();
            result.push(RemoteInfo {
                name: name.to_string(),
                fetch_url: redact_remote_url(&fetch_raw),
                push_url: redact_remote_url(&push_raw),
                has_credentials: url_has_credentials(&fetch_raw) || url_has_credentials(&push_raw),
            });
        }
        Ok(result)
    })
}

#[tauri::command]
pub fn add_remote(
    store: State<'_, RepoStore>,
    name: String,
    url: String,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    let dir = workdir(&store)?;
    let name = name.trim().to_string();
    validate_remote_name(&dir, &name)?;
    ensure_remote_absent(&store, &name)?;
    validate_remote_url(&url)?;
    let outcome = run_git(&dir, &["remote", "add", name.as_str(), url.trim()])?;
    finish(&store, outcome, format!("已添加远程 {name}"), false)
}

#[tauri::command]
pub fn remove_remote(store: State<'_, RepoStore>, name: String) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    ensure_remote(&store, &name)?;
    let dir = workdir(&store)?;
    let outcome = run_git(&dir, &["remote", "remove", name.as_str()])?;
    finish(&store, outcome, format!("已移除远程 {name}"), false)
}

#[tauri::command]
pub fn rename_remote(
    store: State<'_, RepoStore>,
    old_name: String,
    new_name: String,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    ensure_remote(&store, &old_name)?;
    let dir = workdir(&store)?;
    let new_name = new_name.trim().to_string();
    validate_remote_name(&dir, &new_name)?;
    if new_name == old_name {
        return Err("新远程名与原名相同".to_string());
    }
    ensure_remote_absent(&store, &new_name)?;
    let outcome = run_git(
        &dir,
        &["remote", "rename", old_name.as_str(), new_name.as_str()],
    )?;
    finish(
        &store,
        outcome,
        format!("已将远程 {old_name} 重命名为 {new_name}"),
        false,
    )
}

#[tauri::command]
pub fn set_remote_url(
    store: State<'_, RepoStore>,
    name: String,
    url: String,
    push: bool,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    ensure_remote(&store, &name)?;
    validate_remote_url(&url)?;
    let dir = workdir(&store)?;
    let mut args = vec!["remote", "set-url"];
    if push {
        args.push("--push");
    }
    args.push(name.as_str());
    args.push(url.trim());
    let outcome = run_git(&dir, &args)?;
    let label = if push { "推送地址" } else { "抓取地址" };
    finish(
        &store,
        outcome,
        format!("已更新远程 {name} 的{label}"),
        false,
    )
}

#[tauri::command]
pub fn prune_remote(store: State<'_, RepoStore>, name: String) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    ensure_remote(&store, &name)?;
    let dir = workdir(&store)?;
    let outcome = run_git(&dir, &["remote", "prune", name.as_str()])?;
    finish(
        &store,
        outcome,
        format!("已清理 {name} 中失效的远程分支引用"),
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_common_remote_urls() {
        for url in [
            "https://gitlab.example.com/group/repo.git",
            "https://example.com/repo",
            "ssh://git@example.com:2222/group/repo.git",
            "ssh://example.com/repo.git",
            "git@github.com:group/repo.git",
            "git://example.com/repo.git",
            "file:///srv/git/repo.git",
            "/srv/git/repo.git",
            "./repo.git",
            "C:\\repos\\origin.git",
            "C:/repos/origin.git",
        ] {
            assert!(validate_remote_url(url).is_ok(), "should accept {url}");
        }
    }

    #[test]
    fn rejects_unsafe_remote_urls() {
        for url in [
            "",
            "-u",
            "https://user:secret@example.com/repo.git",
            "https://token@example.com/repo.git",
            "ssh://git:secret@example.com/repo.git",
            "ext::sh -c calc",
            "ext::./payload",
            "ftp://example.com/repo.git",
            "ssh:///repo.git",
            "https://exa mple.com/repo.git",
            "repo.git",
        ] {
            assert!(validate_remote_url(url).is_err(), "should reject {url}");
        }
    }

    #[test]
    fn redacts_remote_credentials_but_keeps_usernames() {
        assert_eq!(
            redact_remote_url("https://user:secret@example.com/repo.git"),
            "https://***@example.com/repo.git"
        );
        assert_eq!(
            redact_remote_url("https://token@example.com/repo.git"),
            "https://***@example.com/repo.git"
        );
        assert_eq!(
            redact_remote_url("ssh://git@example.com/repo.git"),
            "ssh://git@example.com/repo.git"
        );
        assert_eq!(
            redact_remote_url("ssh://git:secret@example.com/repo.git"),
            "ssh://***@example.com/repo.git"
        );
        assert_eq!(
            redact_remote_url("git@github.com:group/repo.git"),
            "git@github.com:group/repo.git"
        );
        assert_eq!(
            redact_remote_url("https://example.com/group/repo.git"),
            "https://example.com/group/repo.git"
        );
    }

    #[test]
    fn detects_embedded_credentials() {
        assert!(url_has_credentials(
            "https://user:secret@example.com/repo.git"
        ));
        assert!(url_has_credentials("https://token@example.com/repo.git"));
        assert!(url_has_credentials("ssh://git:secret@example.com/repo.git"));
        assert!(!url_has_credentials("ssh://git@example.com/repo.git"));
        assert!(!url_has_credentials("git@github.com:group/repo.git"));
        assert!(!url_has_credentials("https://example.com/group/repo.git"));
    }
}
