use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use serde::Serialize;

use crate::config::secrets;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const KEY_NAME_LIMIT: usize = 64;
const COMMENT_LIMIT: usize = 200;
const PASSPHRASE_LIMIT: usize = 1024;
const PASSPHRASE_ACCOUNT_PREFIX: &str = "ssh-pass:";
const OUTPUT_LIMIT: usize = 2_000;
const ALLOWED_KEY_TYPES: [&str; 3] = ["ssh-ed25519", "ecdsa-sha2-nistp256", "ssh-rsa"];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshKeyInfo {
    pub name: String,
    pub path: String,
    pub public_path: String,
    pub key_type: String,
    pub bits: u32,
    pub fingerprint: String,
    pub comment: String,
    pub public_key: String,
    pub encrypted: bool,
    pub has_passphrase: bool,
    pub modified_at: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshHostKey {
    pub key_type: String,
    pub fingerprint: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshTestResult {
    /// ok | unknownHost | hostKeyChanged | authFailed | unreachable | error
    pub status: String,
    pub message: String,
    pub host_keys: Vec<SshHostKey>,
}

struct ScannedHostKey {
    key_type: String,
    key: String,
    fingerprint: String,
}

struct KeyFingerprint {
    bits: u32,
    fingerprint: String,
    key_type: String,
    comment: String,
}

pub(crate) fn ssh_dir() -> Result<PathBuf, String> {
    #[cfg(debug_assertions)]
    if let Some(dir) = std::env::var_os("EASYGIT_SSH_DIR") {
        return Ok(PathBuf::from(dir));
    }
    dirs::home_dir()
        .map(|home| home.join(".ssh"))
        .ok_or_else(|| "无法定位用户主目录".to_string())
}

fn ensure_ssh_dir() -> Result<PathBuf, String> {
    let dir = ssh_dir()?;
    fs::create_dir_all(&dir).map_err(|error| format!("创建 SSH 目录失败: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    Ok(dir)
}

fn passphrase_account(name: &str) -> String {
    format!("{PASSPHRASE_ACCOUNT_PREFIX}{name}")
}

pub(crate) fn stored_passphrase(name: &str) -> Option<String> {
    secrets::read_secret(&passphrase_account(name))
        .ok()
        .flatten()
}

fn clip(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= OUTPUT_LIMIT {
        trimmed.to_string()
    } else {
        let mut clipped: String = trimmed.chars().take(OUTPUT_LIMIT).collect();
        clipped.push('…');
        clipped
    }
}

fn tool_output(tool: &str, args: &[&str], stdin: Option<&str>) -> Result<Output, String> {
    let mut command = Command::new(find_tool(tool));
    command
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("SSH_ASKPASS")
        .env_remove("SSH_ASKPASS_REQUIRE")
        .env_remove("EASYGIT_SSH_PASSPHRASE");
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    let mut child = command
        .spawn()
        .map_err(|error| tool_error(tool, &error))?;
    if let Some(input) = stdin {
        if let Some(mut handle) = child.stdin.take() {
            handle
                .write_all(input.as_bytes())
                .map_err(|error| format!("写入 {tool} 输入失败: {error}"))?;
            handle.write_all(b"\n").ok();
            handle.flush().ok();
        }
    }
    child
        .wait_with_output()
        .map_err(|error| format!("等待 {tool} 完成失败: {error}"))
}

fn tool_error(tool: &str, error: &std::io::Error) -> String {
    if error.kind() == std::io::ErrorKind::NotFound {
        format!("未找到 {tool}，请安装 OpenSSH 客户端或 Git for Windows。")
    } else {
        format!("启动 {tool} 失败: {error}")
    }
}

/// Looks on PATH first, then in the Git for Windows bundled OpenSSH directory, so a
/// packaged app started from Explorer does not depend on `Git\\usr\\bin` being on PATH.
pub(crate) fn find_tool(name: &str) -> PathBuf {
    let file_name = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };

    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(&file_name);
            if candidate.is_file() {
                return candidate;
            }
        }
    }

    if let Ok(output) = Command::new("git").args(["--exec-path"]).output() {
        if output.status.success() {
            let exec = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !exec.is_empty() {
                if let Some(root) = PathBuf::from(exec)
                    .parent()
                    .and_then(Path::parent)
                    .and_then(Path::parent)
                {
                    for candidate in [
                        root.join("usr").join("bin").join(&file_name),
                        root.join("bin").join(&file_name),
                    ] {
                        if candidate.is_file() {
                            return candidate;
                        }
                    }
                }
            }
        }
    }

    PathBuf::from(name)
}

fn command_quote(path: &Path) -> String {
    let text = path.to_string_lossy();
    let text = if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text.into_owned()
    };
    format!("'{}'", text.replace('\'', "'\\''"))
}

fn askpass_script() -> Option<PathBuf> {
    static SCRIPT: OnceLock<Option<PathBuf>> = OnceLock::new();
    SCRIPT
        .get_or_init(|| {
            let path = std::env::temp_dir().join(if cfg!(windows) {
                format!("easy-git-askpass-{}.bat", std::process::id())
            } else {
                format!("easy-git-askpass-{}.sh", std::process::id())
            });
            let body = if cfg!(windows) {
                "@echo off\r\nsetlocal EnableDelayedExpansion\r\necho(!EASYGIT_SSH_PASSPHRASE!\r\n"
            } else {
                "#!/bin/sh\nprintf '%s\\n' \"$EASYGIT_SSH_PASSPHRASE\"\n"
            };
            fs::write(&path, body).ok()?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).ok()?;
            }
            Some(path)
        })
        .clone()
}

fn apply_askpass_env(command: &mut Command, passphrase: &str) -> Result<(), String> {
    let script = askpass_script().ok_or_else(|| "无法创建 SSH 口令辅助脚本".to_string())?;
    command
        .env("SSH_ASKPASS", script)
        .env("SSH_ASKPASS_REQUIRE", "force")
        .env("DISPLAY", ":0")
        .env("EASYGIT_SSH_PASSPHRASE", passphrase);
    Ok(())
}

pub(crate) fn apply_git_ssh(command: &mut Command) {
    let Ok(config) = crate::config::load_config() else {
        return;
    };
    let Some(name) = config
        .git
        .ssh_key_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let Ok(command_line) = git_ssh_command(name) else {
        return;
    };
    command.env("GIT_SSH_COMMAND", command_line);

    if let Some(passphrase) = stored_passphrase(name) {
        if apply_askpass_env(command, &passphrase).is_err() {
            // A missing helper is not fatal: SSH will fall back to its own prompt
            // and Git's non-interactive environment will surface the failure.
        }
    }
}

fn git_ssh_command(name: &str) -> Result<String, String> {
    let name = validate_key_name(name)?;
    let dir = ssh_dir()?;
    let key_path = dir.join(&name);
    if !key_path.is_file() {
        return Err(format!("找不到私钥 {name}"));
    }
    let known_hosts = dir.join("known_hosts");
    let ssh = find_tool("ssh");
    let mut value = format!(
        "{} -i {} -o IdentitiesOnly=yes -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile={}",
        command_quote(&ssh),
        command_quote(&key_path),
        command_quote(&known_hosts)
    );
    if stored_passphrase(&name).is_none() {
        value.push_str(" -o BatchMode=yes");
    }
    Ok(value)
}

fn validate_key_name(raw: &str) -> Result<String, String> {
    let name = raw.trim();
    if name.is_empty() {
        return Err("密钥名称不能为空".to_string());
    }
    if name.chars().count() > KEY_NAME_LIMIT {
        return Err(format!("密钥名称不能超过 {KEY_NAME_LIMIT} 个字符"));
    }
    if name.starts_with('-') || name.starts_with('.') {
        return Err("密钥名称不能以 - 或 . 开头".to_string());
    }
    if name.contains("..")
        || name.contains('/')
        || name.contains('\\')
        || name.chars().any(|character| {
            !(character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-'))
        })
    {
        return Err("密钥名称只能包含字母、数字、点、下划线和连字符".to_string());
    }
    let lower = name.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "config" | "known_hosts" | "known_hosts.old" | "authorized_keys" | "authorized_keys2"
    ) {
        return Err("这个名称是 SSH 保留文件名，请换一个".to_string());
    }
    #[cfg(windows)]
    if matches!(
        lower.trim_end_matches('.'),
        "con" | "prn" | "aux" | "nul" | "com1" | "com2" | "com3" | "com4" | "com5" | "com6"
            | "com7" | "com8" | "com9" | "lpt1" | "lpt2" | "lpt3" | "lpt4" | "lpt5" | "lpt6"
            | "lpt7" | "lpt8" | "lpt9"
    ) {
        return Err("这个名称是 Windows 保留设备名，请换一个".to_string());
    }
    Ok(name.to_string())
}

fn validate_comment(raw: &str) -> Result<String, String> {
    let comment = raw.trim();
    if comment.chars().count() > COMMENT_LIMIT {
        return Err(format!("注释不能超过 {COMMENT_LIMIT} 个字符"));
    }
    if comment
        .chars()
        .any(|character| character.is_control())
    {
        return Err("注释不能包含控制字符".to_string());
    }
    Ok(if comment.is_empty() {
        "easy-git".to_string()
    } else {
        comment.to_string()
    })
}

fn validate_passphrase(raw: Option<String>) -> Result<Option<String>, String> {
    let Some(value) = raw else {
        return Ok(None);
    };
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > PASSPHRASE_LIMIT {
        return Err(format!("口令不能超过 {PASSPHRASE_LIMIT} 个字符"));
    }
    if value
        .chars()
        .any(|character| matches!(character, '\0' | '\n' | '\r'))
    {
        return Err("口令不能包含换行或空字符".to_string());
    }
    Ok(Some(value))
}

fn validate_host(raw: &str) -> Result<String, String> {
    let host = raw.trim();
    if host.is_empty() {
        return Err("主机地址不能为空".to_string());
    }
    if host.len() > 255 {
        return Err("主机地址过长".to_string());
    }
    if host.starts_with('-')
        || host.contains("://")
        || host.chars().any(|character| {
            character.is_whitespace()
                || character.is_control()
                || matches!(character, '/' | '\\' | '@' | '"' | '\'' | ';' | '&' | '|' | '<' | '>')
        })
    {
        return Err("主机地址包含非法字符".to_string());
    }
    Ok(host.to_string())
}

fn validate_user(raw: &str) -> Result<String, String> {
    let user = raw.trim();
    if user.is_empty() {
        return Err("用户名不能为空".to_string());
    }
    if user.len() > 64
        || user.chars().any(|character| {
            !(character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-'))
        })
    {
        return Err("用户名只能包含字母、数字、点、下划线和连字符".to_string());
    }
    Ok(user.to_string())
}

fn validate_port(port: u16) -> Result<u16, String> {
    if port == 0 {
        return Err("端口必须在 1–65535 之间".to_string());
    }
    Ok(port)
}

fn validate_key_type(raw: &str) -> Result<String, String> {
    let key_type = raw.trim();
    if !ALLOWED_KEY_TYPES.contains(&key_type) {
        return Err("不支持的主机密钥类型".to_string());
    }
    Ok(key_type.to_string())
}

fn key_paths(dir: &Path, name: &str) -> (PathBuf, PathBuf) {
    (dir.join(name), dir.join(format!("{name}.pub")))
}

fn looks_like_private_key(path: &Path) -> bool {
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(96)]);
    head.starts_with("-----BEGIN ") && head.lines().next().is_some_and(|line| line.contains("PRIVATE KEY"))
}

fn read_public_key(path: &Path) -> Result<String, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("读取公钥失败: {error}"))?;
    let line = raw
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .ok_or_else(|| "公钥文件为空".to_string())?;
    if !(line.starts_with("ssh-")
        || line.starts_with("ecdsa-")
        || line.starts_with("sk-"))
    {
        return Err("公钥文件格式无法识别".to_string());
    }
    Ok(line.to_string())
}

fn parse_fingerprint(text: &str) -> Option<KeyFingerprint> {
    let line = text.lines().map(str::trim).find(|line| !line.is_empty())?;
    let mut fields = line.split_whitespace();
    let bits = fields.next().and_then(|value| value.parse().ok()).unwrap_or(0);
    let fingerprint = fields.next()?.to_string();
    let rest: Vec<&str> = fields.collect();
    let (key_type, comment_fields) = match rest.last() {
        Some(last) if last.starts_with('(') && last.ends_with(')') => (
            last.trim_matches(|character| character == '(' || character == ')')
                .to_string(),
            &rest[..rest.len() - 1],
        ),
        _ => (String::new(), &rest[..]),
    };
    Some(KeyFingerprint {
        bits,
        fingerprint,
        key_type,
        comment: comment_fields.join(" "),
    })
}

fn fingerprint_of(public_path: &Path) -> Result<KeyFingerprint, String> {
    let path = public_path.to_string_lossy().to_string();
    let output = tool_output("ssh-keygen", &["-l", "-E", "sha256", "-f", &path], None)?;
    if !output.status.success() {
        return Err(clip(&String::from_utf8_lossy(&output.stderr)));
    }
    parse_fingerprint(&String::from_utf8_lossy(&output.stdout))
        .ok_or_else(|| "无法解析公钥指纹".to_string())
}

fn key_is_encrypted(private_path: &Path) -> bool {
    let path = private_path.to_string_lossy().to_string();
    match tool_output("ssh-keygen", &["-y", "-P", "", "-f", &path], None) {
        Ok(output) => !output.status.success(),
        Err(_) => true,
    }
}

fn key_name_type(public_key: &str) -> String {
    public_key
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_string()
}

fn inspect_key(dir: &Path, name: &str, private_path: &Path) -> Result<SshKeyInfo, String> {
    let (_, public_path) = key_paths(dir, name);
    let public_key = read_public_key(&public_path)?;
    let fingerprint = fingerprint_of(&public_path)?;
    let modified_at = fs::metadata(private_path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let key_type = if fingerprint.key_type.is_empty() {
        key_name_type(&public_key)
    } else {
        fingerprint.key_type.clone()
    };
    let comment = if fingerprint.comment.is_empty() {
        public_key
            .split_whitespace()
            .nth(2)
            .unwrap_or("")
            .to_string()
    } else {
        fingerprint.comment.clone()
    };
    Ok(SshKeyInfo {
        name: name.to_string(),
        path: private_path.to_string_lossy().to_string(),
        public_path: public_path.to_string_lossy().to_string(),
        key_type,
        bits: fingerprint.bits,
        fingerprint: fingerprint.fingerprint,
        comment,
        public_key,
        encrypted: key_is_encrypted(private_path),
        has_passphrase: stored_passphrase(name).is_some(),
        modified_at,
    })
}

#[tauri::command]
pub fn list_ssh_keys() -> Result<Vec<SshKeyInfo>, String> {
    let dir = ssh_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let entries = fs::read_dir(&dir).map_err(|error| format!("读取 SSH 目录失败: {error}"))?;
    let mut keys = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !metadata.file_type().is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.ends_with(".pub") || validate_key_name(name).is_err() {
            continue;
        }
        if !looks_like_private_key(&path) {
            continue;
        }
        if let Ok(info) = inspect_key(&dir, name, &path) {
            keys.push(info);
        }
    }
    keys.sort_by(|left, right| {
        right
            .modified_at
            .cmp(&left.modified_at)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(keys)
}

#[tauri::command]
pub fn generate_ssh_key(
    name: String,
    key_type: String,
    comment: String,
    passphrase: Option<String>,
) -> Result<SshKeyInfo, String> {
    let name = validate_key_name(&name)?;
    let kind = match key_type.trim().to_ascii_lowercase().as_str() {
        "ed25519" => "ed25519",
        "rsa" => "rsa",
        _ => return Err("密钥类型只支持 Ed25519 或 RSA".to_string()),
    };
    let comment = validate_comment(&comment)?;
    let passphrase = validate_passphrase(passphrase)?;
    let dir = ensure_ssh_dir()?;
    let (private_path, public_path) = key_paths(&dir, &name);
    if private_path.exists() || public_path.exists() {
        return Err(format!("密钥「{name}」已存在，请换一个名称"));
    }

    let mut command = Command::new(find_tool("ssh-keygen"));
    command
        .args(["-q", "-t", kind, "-f"])
        .arg(&private_path)
        .args(["-C", &comment])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if kind == "rsa" {
        command.args(["-b", "4096"]);
    }
    match passphrase.as_deref() {
        Some(value) => {
            apply_askpass_env(&mut command, value)?;
        }
        None => {
            command.args(["-N", ""]);
        }
    }
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    let output = command
        .output()
        .map_err(|error| tool_error("ssh-keygen", &error))?;
    if !output.status.success() {
        let _ = fs::remove_file(&private_path);
        let _ = fs::remove_file(&public_path);
        let detail = clip(&String::from_utf8_lossy(&output.stderr));
        return Err(if detail.is_empty() {
            "生成 SSH 密钥失败".to_string()
        } else {
            format!("生成 SSH 密钥失败: {detail}")
        });
    }
    if !public_path.is_file() {
        let _ = fs::remove_file(&private_path);
        return Err("ssh-keygen 没有生成公钥文件".to_string());
    }

    if let Some(value) = passphrase {
        if secrets::set_secret(passphrase_account(&name), value).is_err() {
            let _ = fs::remove_file(&private_path);
            let _ = fs::remove_file(&public_path);
            return Err("密钥已生成，但口令无法写入系统钥匙串，已删除该密钥".to_string());
        }
    }

    inspect_key(&dir, &name, &private_path)
}

#[tauri::command]
pub fn set_ssh_passphrase(name: String, passphrase: String) -> Result<(), String> {
    let name = validate_key_name(&name)?;
    let value = validate_passphrase(Some(passphrase))?
        .ok_or_else(|| "口令不能为空".to_string())?;
    let dir = ssh_dir()?;
    let (private_path, _) = key_paths(&dir, &name);
    if !private_path.is_file() {
        return Err(format!("找不到私钥 {name}"));
    }
    if !key_is_encrypted(&private_path) {
        return Err("这个密钥没有口令，无需保存".to_string());
    }

    let path = private_path.to_string_lossy().to_string();
    let mut command = Command::new(find_tool("ssh-keygen"));
    command
        .args(["-y", "-f", &path])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    apply_askpass_env(&mut command, &value)?;
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let output = command
        .output()
        .map_err(|error| tool_error("ssh-keygen", &error))?;
    if !output.status.success() {
        return Err("口令不正确，无法解密这个密钥".to_string());
    }

    secrets::set_secret(passphrase_account(&name), value)
}

#[tauri::command]
pub fn clear_ssh_passphrase(name: String) -> Result<(), String> {
    let name = validate_key_name(&name)?;
    secrets::delete_secret(passphrase_account(&name))
}

#[tauri::command]
pub fn delete_ssh_key(name: String) -> Result<(), String> {
    let name = validate_key_name(&name)?;
    let dir = ssh_dir()?;
    let (private_path, public_path) = key_paths(&dir, &name);
    for path in [&private_path, &public_path] {
        if let Ok(metadata) = fs::symlink_metadata(path) {
            if metadata.file_type().is_symlink() {
                return Err("拒绝删除符号链接形式的密钥文件".to_string());
            }
        }
    }
    if !private_path.exists() && !public_path.exists() {
        return Err(format!("找不到密钥 {name}"));
    }
    if let Err(error) = fs::remove_file(&private_path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            return Err(format!("删除私钥失败: {error}"));
        }
    }
    if let Err(error) = fs::remove_file(&public_path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            return Err(format!("删除公钥失败: {error}"));
        }
    }
    let _ = secrets::delete_secret(passphrase_account(&name));

    if let Ok(mut config) = crate::config::load_config() {
        if config.git.ssh_key_name.as_deref() == Some(name.as_str()) {
            config.git.ssh_key_name = None;
            let _ = crate::config::save_config(&config);
        }
    }
    Ok(())
}

fn parse_public_line(line: &str) -> Option<(String, String)> {
    if line.starts_with('#') {
        return None;
    }
    let fields: Vec<&str> = line.split_whitespace().collect();
    let index = fields.iter().position(|field| {
        field.starts_with("ssh-") || field.starts_with("ecdsa-") || field.starts_with("sk-")
    })?;
    let key_type = fields.get(index)?.to_string();
    let key = fields.get(index + 1)?.to_string();
    Some((key_type, key))
}

fn fingerprint_of_line(line: &str) -> Option<String> {
    let output = tool_output("ssh-keygen", &["-l", "-E", "sha256", "-f", "-"], Some(line)).ok()?;
    if !output.status.success() {
        return None;
    }
    parse_fingerprint(&String::from_utf8_lossy(&output.stdout)).map(|value| value.fingerprint)
}

fn known_hosts_path() -> Result<PathBuf, String> {
    Ok(ssh_dir()?.join("known_hosts"))
}

fn known_hosts_lookup(host: &str, port: u16) -> String {
    if port == 22 {
        host.to_string()
    } else {
        format!("[{host}]:{port}")
    }
}

fn known_host_entries(host: &str, port: u16) -> Result<Vec<(String, String)>, String> {
    let known_hosts = known_hosts_path()?;
    if !known_hosts.is_file() {
        return Ok(Vec::new());
    }
    let lookup = known_hosts_lookup(host, port);
    let output = tool_output(
        "ssh-keygen",
        &["-F", &lookup, "-f", &known_hosts.to_string_lossy()],
        None,
    )?;
    if !output.status.success() {
        return Ok(Vec::new());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .filter_map(parse_public_line)
        .collect())
}

fn scan_host_keys(host: &str, port: u16) -> Result<Vec<ScannedHostKey>, String> {
    let port_text = port.to_string();
    let types = ALLOWED_KEY_TYPES.join(",");
    let output = tool_output(
        "ssh-keyscan",
        &["-T", "8", "-p", &port_text, "-t", &types, host],
        None,
    )?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut keys: Vec<ScannedHostKey> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let Some((key_type, key)) = parse_public_line(trimmed) else {
            continue;
        };
        if !ALLOWED_KEY_TYPES.contains(&key_type.as_str()) {
            continue;
        }
        if keys.iter().any(|entry| entry.key_type == key_type) {
            continue;
        }
        let Some(fingerprint) = fingerprint_of_line(trimmed) else {
            continue;
        };
        keys.push(ScannedHostKey {
            key_type,
            key,
            fingerprint,
        });
        if keys.len() >= ALLOWED_KEY_TYPES.len() {
            break;
        }
    }
    if keys.is_empty() && !output.status.success() {
        let detail = clip(&String::from_utf8_lossy(&output.stderr));
        return Err(if detail.is_empty() {
            "无法从服务器读取主机密钥".to_string()
        } else {
            format!("无法从服务器读取主机密钥: {detail}")
        });
    }
    Ok(keys)
}

fn host_key_list(keys: &[ScannedHostKey]) -> Vec<SshHostKey> {
    keys.iter()
        .map(|entry| SshHostKey {
            key_type: entry.key_type.clone(),
            fingerprint: entry.fingerprint.clone(),
        })
        .collect()
}

#[tauri::command]
pub fn trust_ssh_host(host: String, port: u16, key_type: String) -> Result<SshHostKey, String> {
    let host = validate_host(&host)?;
    let port = validate_port(port)?;
    let key_type = validate_key_type(&key_type)?;
    let scanned = scan_host_keys(&host, port)?;
    let selected = scanned
        .iter()
        .find(|entry| entry.key_type == key_type)
        .ok_or_else(|| format!("服务器没有提供 {key_type} 类型的主机密钥"))?;

    let entries = known_host_entries(&host, port)?;
    if let Some((_, existing_key)) = entries.iter().find(|(kind, _)| kind == &key_type) {
        if existing_key == &selected.key {
            return Ok(SshHostKey {
                key_type: selected.key_type.clone(),
                fingerprint: selected.fingerprint.clone(),
            });
        }
        let existing_fingerprint = fingerprint_of_line(&format!(
            "{} {}",
            key_type, existing_key
        ))
        .unwrap_or_else(|| "未知".to_string());
        return Err(format!(
            "known_hosts 中已有该主机的 {key_type} 密钥，但与服务器现在提供的指纹不一致（现有 {existing_fingerprint}，当前 {}）。出于安全考虑没有覆盖，请先核对服务器身份并手动处理 known_hosts 中的旧记录。",
            selected.fingerprint
        ));
    }

    let dir = ensure_ssh_dir()?;
    let known_hosts = dir.join("known_hosts");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&known_hosts)
        .map_err(|error| format!("写入 known_hosts 失败: {error}"))?;
    writeln!(file, "{} {} {}", known_hosts_lookup(&host, port), selected.key_type, selected.key)
        .map_err(|error| format!("写入 known_hosts 失败: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&known_hosts, fs::Permissions::from_mode(0o600));
    }

    Ok(SshHostKey {
        key_type: selected.key_type.clone(),
        fingerprint: selected.fingerprint.clone(),
    })
}

fn classify_ssh_failure(lower: &str, known_hosts_empty: bool) -> &'static str {
    if lower.contains("host key verification failed") {
        if known_hosts_empty {
            "unknownHost"
        } else {
            "hostKeyChanged"
        }
    } else if lower.contains("permission denied")
        || lower.contains("too many authentication failures")
        || lower.contains("no supported authentication methods")
    {
        "authFailed"
    } else if lower.contains("connection refused")
        || lower.contains("connection timed out")
        || lower.contains("operation timed out")
        || lower.contains("could not resolve hostname")
        || lower.contains("name or service not known")
        || lower.contains("network is unreachable")
        || lower.contains("no route to host")
        || lower.contains("connection closed")
    {
        "unreachable"
    } else {
        "error"
    }
}

fn ssh_identity(key_name: Option<&str>) -> Result<Option<(PathBuf, Option<String>)>, String> {
    let Some(raw) = key_name.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let name = validate_key_name(raw)?;
    let dir = ssh_dir()?;
    let path = dir.join(&name);
    if !path.is_file() {
        return Err(format!("找不到私钥 {name}"));
    }
    Ok(Some((path, stored_passphrase(&name))))
}

#[tauri::command]
pub fn test_ssh_connection(
    host: String,
    port: u16,
    user: String,
    key_name: Option<String>,
) -> Result<SshTestResult, String> {
    let host = validate_host(&host)?;
    let port = validate_port(port)?;
    let user = validate_user(&user)?;
    let identity = ssh_identity(key_name.as_deref())?;
    let known_hosts = known_hosts_path()?;
    let known_entries = known_host_entries(&host, port)?;

    let port_text = port.to_string();
    let known_text = known_hosts.to_string_lossy().to_string();
    let mut command = Command::new(find_tool("ssh"));
    command
        .arg("-T")
        .arg("-p")
        .arg(&port_text)
        .arg("-o")
        .arg("ConnectTimeout=10")
        .arg("-o")
        .arg("StrictHostKeyChecking=yes")
        .arg("-o")
        .arg(format!("UserKnownHostsFile={known_text}"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some((path, passphrase)) = &identity {
        command.arg("-i").arg(path).arg("-o").arg("IdentitiesOnly=yes");
        if let Some(value) = passphrase {
            apply_askpass_env(&mut command, value)?;
        } else {
            command.arg("-o").arg("BatchMode=yes");
        }
    } else {
        command.arg("-o").arg("BatchMode=yes");
    }
    command.arg(format!("{user}@{host}"));
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    let output = command
        .output()
        .map_err(|error| tool_error("ssh", &error))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let combined = [stdout.as_str(), stderr.as_str()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let lower = combined.to_ascii_lowercase();

    if output.status.success() || lower.contains("successfully authenticated") {
        return Ok(SshTestResult {
            status: "ok".to_string(),
            message: if combined.is_empty() {
                "SSH 连接成功".to_string()
            } else {
                clip(&combined)
            },
            host_keys: Vec::new(),
        });
    }

    let status = classify_ssh_failure(&lower, known_entries.is_empty());

    let mut host_keys = Vec::new();
    let mut message = if combined.is_empty() {
        format!("ssh 退出码 {}", output.status.code().unwrap_or(-1))
    } else {
        clip(&combined)
    };
    if status == "unknownHost" || status == "hostKeyChanged" {
        match scan_host_keys(&host, port) {
            Ok(keys) => host_keys = host_key_list(&keys),
            Err(error) => message.push_str(&format!("\n{error}")),
        }
        if status == "hostKeyChanged" {
            message.push_str(
                "\n本地 known_hosts 已有这个主机的记录，但密钥与服务器现在提供的不一致。请核对指纹后再决定是否更新。",
            );
        }
    }

    Ok(SshTestResult {
        status: status.to_string(),
        message,
        host_keys,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_key_names() {
        assert!(validate_key_name("id_ed25519").is_ok());
        assert!(validate_key_name("work-key.2026").is_ok());
        for name in [
            "",
            "-id",
            ".hidden",
            "../escape",
            "dir/key",
            "dir\\key",
            "key name",
            "config",
            "known_hosts",
        ] {
            assert!(validate_key_name(name).is_err(), "should reject {name}");
        }
    }

    #[test]
    fn validates_hosts_users_and_ports() {
        for host in ["gitlab.example.com", "172.20.46.71", "::1", "[::1]"] {
            assert!(validate_host(host).is_ok(), "should accept {host}");
        }
        for host in ["", "-oProxyCommand=calc", "ssh://host", "host name", "host;rm"] {
            assert!(validate_host(host).is_err(), "should reject {host}");
        }
        assert!(validate_user("git").is_ok());
        assert!(validate_user("name@host").is_err());
        assert!(validate_port(22).is_ok());
        assert!(validate_port(0).is_err());
    }

    #[test]
    fn parses_public_key_lines() {
        let line = "gitlab.example.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExample";
        assert_eq!(
            parse_public_line(line),
            Some((
                "ssh-ed25519".to_string(),
                "AAAAC3NzaC1lZDI1NTE5AAAAIExample".to_string()
            ))
        );
        assert_eq!(
            parse_public_line("@cert-authority example.com ssh-rsa AAAA"),
            Some(("ssh-rsa".to_string(), "AAAA".to_string()))
        );
        assert_eq!(parse_public_line("# comment"), None);
    }

    #[test]
    fn parses_ssh_keygen_fingerprint() {
        let parsed = parse_fingerprint(
            "256 SHA256:abcDEF123 user@example.com (ED25519)\n",
        )
        .unwrap();
        assert_eq!(parsed.bits, 256);
        assert_eq!(parsed.fingerprint, "SHA256:abcDEF123");
        assert_eq!(parsed.key_type, "ED25519");
        assert_eq!(parsed.comment, "user@example.com");
    }

    #[test]
    fn classifies_ssh_failures() {
        assert_eq!(
            classify_ssh_failure("host key verification failed.", true),
            "unknownHost"
        );
        assert_eq!(
            classify_ssh_failure("host key verification failed.", false),
            "hostKeyChanged"
        );
        assert_eq!(
            classify_ssh_failure("git@example.com: permission denied (publickey).", true),
            "authFailed"
        );
        assert_eq!(
            classify_ssh_failure(
                "ssh: connect to host example.com port 22: connection refused",
                true
            ),
            "unreachable"
        );
        assert_eq!(
            classify_ssh_failure("could not resolve hostname example.com", true),
            "unreachable"
        );
        assert_eq!(classify_ssh_failure("unexpected failure", true), "error");
    }
}
