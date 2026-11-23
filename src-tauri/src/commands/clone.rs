use std::path::PathBuf;

use super::branch::run_git;
use super::gitlab_projects::CloneResult;

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

fn is_scp_like(url: &str) -> bool {
    let Some((user_host, path)) = url.split_once(':') else {
        return false;
    };
    if path.is_empty() || user_host.contains('/') {
        return false;
    }
    let Some((user, host)) = user_host.split_once('@') else {
        return false;
    };
    !user.is_empty() && !host.is_empty()
}

fn authority_has_password(url: &str) -> bool {
    let Some((_, rest)) = url.split_once("://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let Some((userinfo, _host)) = authority.rsplit_once('@') else {
        return false;
    };
    userinfo.contains(':')
}

fn validate_url(raw: &str) -> Result<String, String> {
    let url = raw.trim();
    if url.is_empty() {
        return Err("仓库地址不能为空。".to_string());
    }
    if url.starts_with('-') {
        return Err("仓库地址不能以 - 开头。".to_string());
    }
    if url.chars().any(|ch| ch.is_whitespace() || ch.is_control()) {
        return Err("仓库地址不能包含空白或控制字符。".to_string());
    }
    let lowered = url.to_ascii_lowercase();
    let supported = ["http://", "https://", "ssh://", "git://"]
        .iter()
        .any(|prefix| lowered.starts_with(prefix));
    if !supported && !is_scp_like(url) {
        return Err("只支持 http(s)://、ssh://、git:// 或 git@host:path 形式的仓库地址。".to_string());
    }
    if authority_has_password(url) {
        return Err(
            "地址里不要包含密码或令牌；私有仓库请使用 GitLab 面板，令牌会保存在系统钥匙串里。"
                .to_string(),
        );
    }
    Ok(url.to_string())
}

fn repository_name(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    let path = if let Some((_, rest)) = trimmed.split_once("://") {
        rest.split_once('/').map(|(_, path)| path).unwrap_or("")
    } else {
        trimmed
            .split_once(':')
            .map(|(_, path)| path)
            .unwrap_or(trimmed)
    };
    let tail = path.rsplit('/').next().unwrap_or(path);
    let tail = tail.split(['?', '#']).next().unwrap_or(tail);
    let tail = tail.strip_suffix(".git").unwrap_or(tail);
    sanitize_segment(tail)
}

#[tauri::command]
pub async fn clone_repository(url: String, parent_dir: String) -> Result<CloneResult, String> {
    let url = validate_url(&url)?;
    let parent = PathBuf::from(parent_dir.trim());
    if !parent.is_dir() {
        return Err(format!(
            "克隆目录 {} 不存在或不是文件夹。",
            parent.to_string_lossy()
        ));
    }
    let name = repository_name(&url);
    let target = parent.join(&name);
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

    let path = target.to_string_lossy().to_string();
    let display_name = target
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| name.clone());
    let display_path = path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let output = run_git(&parent, &["clone", "--", &url, &path])?;
        if !output.ok {
            let reason = if output.text.is_empty() {
                "Git 没有给出原因".to_string()
            } else {
                output.text
            };
            return Err(format!("克隆失败：{reason}"));
        }
        Ok::<_, String>(())
    })
    .await
    .map_err(|error| format!("克隆任务失败: {error}"))??;

    Ok(CloneResult {
        path: display_path,
        name: display_name,
    })
}

#[cfg(test)]
mod tests {
    use super::{authority_has_password, repository_name, validate_url};

    #[test]
    fn accepts_common_repository_urls() {
        for url in [
            "https://github.com/owner/repo.git",
            "http://gl.local/group/repo",
            "ssh://git@host:2222/group/repo.git",
            "git@github.com:owner/repo.git",
            "git://127.0.0.1:9418/repo.git",
        ] {
            assert!(validate_url(url).is_ok(), "{url}");
        }
    }

    #[test]
    fn rejects_unsupported_or_dangerous_urls() {
        assert!(validate_url("").is_err());
        assert!(validate_url("   ").is_err());
        assert!(validate_url("--upload-pack=calc").is_err());
        assert!(validate_url("file:///C:/repo").is_err());
        assert!(validate_url("C:\\repo").is_err());
        assert!(validate_url("https://user:token@host/repo.git").is_err());
        assert!(validate_url("https://host/re po.git").is_err());
    }

    #[test]
    fn derives_a_safe_target_name() {
        assert_eq!(repository_name("https://github.com/owner/repo.git"), "repo");
        assert_eq!(repository_name("git@github.com:owner/repo.git/"), "repo");
        assert_eq!(repository_name("https://host/group/my:repo"), "my-repo");
        assert_eq!(repository_name(".."), "repo");
    }

    #[test]
    fn userinfo_without_password_is_allowed() {
        assert!(!authority_has_password("ssh://git@host:2222/group/repo.git"));
        assert!(!authority_has_password("https://host:8443/group/repo.git"));
    }
}
