use std::path::PathBuf;

use serde::Serialize;

use super::branch::{run_git, run_git_raw};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitIdentity {
    pub name: Option<String>,
    pub email: Option<String>,
}

/// `git config --global` needs a working directory, not a repository.
fn neutral_dir() -> Result<PathBuf, String> {
    dirs::home_dir()
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| "无法定位用户主目录".to_string())
}

fn read_global(key: &str) -> Result<Option<String>, String> {
    let dir = neutral_dir()?;
    let output = run_git_raw(&dir, &["config", "--global", "--get", key])?;
    if output.ok {
        let value = output.stdout.trim();
        return Ok((!value.is_empty()).then(|| value.to_string()));
    }
    // Exit code 1 with no stderr means the key is simply unset.
    if output.error.trim().is_empty() {
        Ok(None)
    } else {
        Err(format!("读取全局 Git 配置失败：{}", output.error.trim()))
    }
}

fn write_global(key: &str, value: &str) -> Result<(), String> {
    let dir = neutral_dir()?;
    let output = run_git(&dir, &["config", "--global", "--replace-all", key, value])?;
    if output.ok {
        return Ok(());
    }
    Err(if output.text.trim().is_empty() {
        format!("写入全局 Git 配置 {key} 失败")
    } else {
        output.text.trim().to_string()
    })
}

fn validate_name(raw: &str) -> Result<String, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err("请填写 Git 用户名".to_string());
    }
    if value.chars().count() > 128 {
        return Err("Git 用户名不能超过 128 个字符".to_string());
    }
    if value.starts_with('-') {
        return Err("Git 用户名不能以 - 开头".to_string());
    }
    if value.contains('<') || value.contains('>') {
        return Err("Git 用户名不能包含 < 或 >".to_string());
    }
    if value.chars().any(char::is_control) {
        return Err("Git 用户名不能包含控制字符".to_string());
    }
    Ok(value.to_string())
}

fn validate_email(raw: &str) -> Result<String, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err("请填写 Git 邮箱".to_string());
    }
    if value.chars().count() > 254 {
        return Err("Git 邮箱不能超过 254 个字符".to_string());
    }
    if value.starts_with('-') {
        return Err("Git 邮箱不能以 - 开头".to_string());
    }
    if value.contains('<') || value.contains('>') {
        return Err("Git 邮箱不能包含 < 或 >".to_string());
    }
    if value.chars().any(char::is_control) || value.chars().any(char::is_whitespace) {
        return Err("Git 邮箱不能包含空格".to_string());
    }
    let mut parts = value.split('@');
    let local = parts.next().unwrap_or_default();
    let domain = parts.next().unwrap_or_default();
    if local.is_empty() || domain.is_empty() || parts.next().is_some() {
        return Err("Git 邮箱格式不正确，应形如 you@example.com".to_string());
    }
    Ok(value.to_string())
}

#[tauri::command]
pub fn get_git_identity() -> Result<GitIdentity, String> {
    Ok(GitIdentity {
        name: read_global("user.name")?,
        email: read_global("user.email")?,
    })
}

#[tauri::command]
pub fn save_git_identity(name: String, email: String) -> Result<GitIdentity, String> {
    let name = validate_name(&name)?;
    let email = validate_email(&email)?;
    write_global("user.name", &name)?;
    write_global("user.email", &email)?;
    Ok(GitIdentity {
        name: Some(name),
        email: Some(email),
    })
}

#[cfg(test)]
mod tests {
    use super::{validate_email, validate_name};

    #[test]
    fn accepts_common_identity() {
        assert_eq!(validate_name(" Ada Lovelace ").unwrap(), "Ada Lovelace");
        assert_eq!(validate_email("ada@example.com").unwrap(), "ada@example.com");
    }

    #[test]
    fn rejects_empty_values() {
        assert!(validate_name("   ").is_err());
        assert!(validate_email("").is_err());
    }

    #[test]
    fn rejects_angle_brackets_and_controls() {
        assert!(validate_name("Ada <ada>").is_err());
        assert!(validate_name("Ada\nLovelace").is_err());
        assert!(validate_email("ada@exam<ple>.com").is_err());
        assert!(validate_email("ada\t@example.com").is_err());
    }

    #[test]
    fn rejects_option_like_values() {
        assert!(validate_name("-ada").is_err());
        assert!(validate_email("-ada@example.com").is_err());
    }

    #[test]
    fn rejects_malformed_email() {
        assert!(validate_email("ada").is_err());
        assert!(validate_email("ada@").is_err());
        assert!(validate_email("@example.com").is_err());
        assert!(validate_email("ada@@example.com").is_err());
        assert!(validate_email("ada lovelace@example.com").is_err());
    }

    #[test]
    fn rejects_overlong_values() {
        assert!(validate_name(&"a".repeat(129)).is_err());
        assert!(validate_email(&format!("{}@example.com", "a".repeat(243))).is_err());
    }
}