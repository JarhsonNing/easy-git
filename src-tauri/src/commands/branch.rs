use std::io::{ErrorKind, Write};
use std::path::Path;
use std::process::{Command, Output, Stdio};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use tauri::State;

use super::repo::{
    OperationResult, RepoStatus, RepoStore, lock_ops, read_refs, read_status, workdir,
};

pub(crate) const OUTPUT_LIMIT: usize = 6_000;

/// CREATE_NO_WINDOW: the GUI has no console, so Git must not allocate one.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub(crate) struct GitOutput {
    pub(crate) ok: bool,
    pub(crate) text: String,
}

fn redact_url_credentials(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut index = 0;
    while index < text.len() {
        if !text.is_char_boundary(index) {
            index += 1;
            continue;
        }
        if text[index..].starts_with("://") {
            result.push_str("://");
            index += 3;
            let start = index;
            while index < text.len()
                && text.is_char_boundary(index)
                && !text.as_bytes()[index].is_ascii_whitespace()
                && !matches!(text.as_bytes()[index], b'\'' | b'"' | b'<' | b'>')
            {
                index += 1;
            }
            let authority = &text[start..index];
            match authority.rfind('@') {
                Some(at) if at > 0 => {
                    result.push_str("***@");
                    result.push_str(&authority[at + 1..]);
                }
                _ => result.push_str(authority),
            }
            continue;
        }
        let length = text[index..]
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or(1);
        result.push_str(&text[index..index + length]);
        index += length;
    }
    result
}

fn clip_output(raw: &str) -> String {
    let clean = redact_url_credentials(raw.trim());
    let count = clean.chars().count();
    if count <= OUTPUT_LIMIT {
        clean
    } else {
        let tail: String = clean.chars().skip(count - OUTPUT_LIMIT).collect();
        format!("…（输出已截断）\n{tail}")
    }
}

fn not_found_hint(error: &std::io::Error) -> String {
    match error.kind() {
        ErrorKind::NotFound => "未找到 Git 命令，请先安装 Git 并确保它在系统 PATH 中。".to_string(),
        _ => format!("启动 Git 失败: {error}"),
    }
}

fn collect(output: Output) -> GitOutput {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut combined = String::new();
    if !stdout.trim().is_empty() {
        combined.push_str(stdout.trim_end());
    }
    if !stderr.trim().is_empty() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(stderr.trim_end());
    }

    GitOutput {
        ok: output.status.success(),
        text: clip_output(&combined),
    }
}

fn base_command(dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(dir)
        .args(["-c", "color.ui=false", "-c", "core.quotepath=false"])
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_EDITOR", "true")
        .env("GIT_MERGE_AUTOEDIT", "no")
        .env("GIT_PAGER", "cat")
        .env("GCM_INTERACTIVE", "Never");
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

pub(crate) fn run_git(dir: &Path, args: &[&str]) -> Result<GitOutput, String> {
    let output = base_command(dir, args)
        .output()
        .map_err(|error| not_found_hint(&error))?;
    Ok(collect(output))
}

pub(crate) struct RawOutput {
    pub(crate) ok: bool,
    /// Stdout verbatim: patch text keeps its final newline or Git calls it corrupt.
    pub(crate) stdout: String,
    pub(crate) error: String,
}

pub(crate) fn run_git_raw(dir: &Path, args: &[&str]) -> Result<RawOutput, String> {
    let output = base_command(dir, args)
        .output()
        .map_err(|error| not_found_hint(&error))?;
    Ok(RawOutput {
        ok: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        error: clip_output(&String::from_utf8_lossy(&output.stderr)),
    })
}

/// Runs Git with `input` fed to stdin, so patch text never has to touch the command line
/// or the filesystem.
pub(crate) fn run_git_stdin(dir: &Path, args: &[&str], input: &[u8]) -> Result<GitOutput, String> {
    let mut child = base_command(dir, args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| not_found_hint(&error))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input)
            .map_err(|error| format!("写入 Git 输入失败: {error}"))?;
        stdin.flush().ok();
    }

    let output = child
        .wait_with_output()
        .map_err(|error| format!("等待 Git 完成失败: {error}"))?;
    Ok(collect(output))
}

pub(crate) fn reject_dash(value: &str, label: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{label}不能为空"));
    }
    if value.starts_with('-') {
        return Err(format!("{label}不能以 - 开头"));
    }
    if value.contains('\0') || value.contains('\n') || value.contains('\r') {
        return Err(format!("{label}包含非法字符"));
    }
    Ok(())
}

fn validate_branch_name(dir: &Path, name: &str) -> Result<(), String> {
    reject_dash(name, "分支名")?;
    if name == "HEAD" {
        return Err("不能使用 HEAD 作为分支名".to_string());
    }
    let outcome = run_git(dir, &["check-ref-format", "--branch", name])?;
    if !outcome.ok {
        return Err(format!("分支名「{name}」不符合 Git 命名规则"));
    }
    Ok(())
}

fn validate_tag_name(dir: &Path, name: &str) -> Result<(), String> {
    reject_dash(name, "标签名")?;
    let reference = format!("refs/tags/{name}");
    let outcome = run_git(dir, &["check-ref-format", reference.as_str()])?;
    if !outcome.ok {
        return Err(format!("标签名「{name}」不符合 Git 命名规则"));
    }
    Ok(())
}

fn ensure_oid(oid: &str) -> Result<(), String> {
    let value = oid.trim();
    if value.len() < 7
        || value.len() > 40
        || !value.chars().all(|character| character.is_ascii_hexdigit())
    {
        return Err("无效的提交哈希".to_string());
    }
    Ok(())
}

fn resolve_commit(dir: &Path, spec: &str, label: &str) -> Result<String, String> {
    reject_dash(spec, label)?;
    let revision = format!("{spec}^{{commit}}");
    let outcome = run_git(dir, &["rev-parse", "--verify", revision.as_str()])?;
    if !outcome.ok {
        return Err(format!("无法解析{label}「{spec}」"));
    }
    let oid = outcome
        .text
        .lines()
        .map(str::trim)
        .find(|line| line.len() == 40 && line.chars().all(|c| c.is_ascii_hexdigit()))
        .ok_or_else(|| format!("无法解析{label}「{spec}」"))?;
    Ok(oid.to_string())
}

fn ensure_local_branch(dir: &Path, name: &str) -> Result<(), String> {
    validate_branch_name(dir, name)?;
    let reference = format!("refs/heads/{name}");
    let outcome = run_git(
        dir,
        &["rev-parse", "--verify", "--quiet", reference.as_str()],
    )?;
    if !outcome.ok {
        return Err(format!("本地分支「{name}」不存在"));
    }
    Ok(())
}

pub(crate) fn ensure_remote(store: &RepoStore, remote: &str) -> Result<(), String> {
    reject_dash(remote, "远程名")?;
    let refs = read_refs(store)?;
    if !refs.remotes.iter().any(|name| name == remote) {
        return Err(format!("远程「{remote}」不存在"));
    }
    Ok(())
}

pub(crate) fn ensure_no_operation(store: &RepoStore) -> Result<RepoStatus, String> {
    let status = read_status(store)?;
    if status.state != "clean" {
        return Err(format!(
            "{}，请先继续或中止当前操作",
            state_label(&status.state)
        ));
    }
    Ok(status)
}

fn state_label(state: &str) -> &'static str {
    match state {
        "merge" => "合并尚未完成",
        "rebase"
        | "rebase-interactive"
        | "rebase-merge"
        | "apply-mailbox"
        | "apply-mailbox-or-rebase" => "变基尚未完成",
        "cherry-pick" | "cherry-pick-sequence" => "拣选尚未完成",
        "revert" | "revert-sequence" => "还原尚未完成",
        "bisect" => "二分查找正在进行",
        _ => "仓库中有未完成的操作",
    }
}

pub(crate) fn finish(
    store: &RepoStore,
    outcome: GitOutput,
    success: impl Into<String>,
    conflict_capable: bool,
) -> Result<OperationResult, String> {
    let status = read_status(store)?;
    if conflict_capable && (status.state != "clean" || status.conflicted > 0) {
        return Ok(OperationResult {
            message: format!("{}，请解决冲突后继续", state_label(&status.state)),
            output: outcome.text,
            conflicts: status.conflicts,
            state: status.state,
        });
    }
    if !outcome.ok {
        return Err(if outcome.text.is_empty() {
            "Git 操作失败，请检查仓库状态".to_string()
        } else {
            outcome.text
        });
    }
    Ok(OperationResult {
        message: success.into(),
        output: outcome.text,
        conflicts: Vec::new(),
        state: status.state,
    })
}

#[tauri::command]
pub fn checkout_branch(
    store: State<'_, RepoStore>,
    name: String,
    kind: String,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    ensure_no_operation(&store)?;
    let dir = workdir(&store)?;

    if kind == "remote" {
        validate_branch_name(&dir, &name).map_err(|_| format!("远程分支「{name}」名称异常"))?;
        let reference = format!("refs/remotes/{name}");
        let exists = run_git(
            &dir,
            &["rev-parse", "--verify", "--quiet", reference.as_str()],
        )?;
        if !exists.ok {
            return Err(format!("远程分支「{name}」不存在"));
        }
        let local = name
            .split_once('/')
            .map(|(_, rest)| rest)
            .filter(|rest| !rest.is_empty())
            .ok_or_else(|| format!("无法从「{name}」推导本地分支名"))?;
        validate_branch_name(&dir, local)?;
        let local_reference = format!("refs/heads/{local}");
        let local_exists = run_git(
            &dir,
            &["rev-parse", "--verify", "--quiet", local_reference.as_str()],
        )?;
        if local_exists.ok {
            return Err(format!("本地已存在分支「{local}」，请直接切换该分支"));
        }
        let outcome = run_git(&dir, &["checkout", "-b", local, "--track", name.as_str()])?;
        finish(
            &store,
            outcome,
            format!("已基于 {name} 创建并切换到 {local}"),
            false,
        )
    } else {
        ensure_local_branch(&dir, &name)?;
        let outcome = run_git(&dir, &["checkout", name.as_str()])?;
        finish(&store, outcome, format!("已切换到分支 {name}"), false)
    }
}

#[tauri::command]
pub fn checkout_commit(
    store: State<'_, RepoStore>,
    oid: String,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    ensure_no_operation(&store)?;
    ensure_oid(&oid)?;
    let dir = workdir(&store)?;
    let target = resolve_commit(&dir, &oid, "提交")?;
    let outcome = run_git(&dir, &["checkout", "--detach", target.as_str()])?;
    let short: String = target.chars().take(7).collect();
    finish(
        &store,
        outcome,
        format!("已切换到提交 {short}（分离 HEAD）"),
        false,
    )
}

#[tauri::command]
pub fn create_branch(
    store: State<'_, RepoStore>,
    name: String,
    start_point: Option<String>,
    checkout: bool,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    let dir = workdir(&store)?;
    validate_branch_name(&dir, &name)?;
    let existing = format!("refs/heads/{name}");
    if run_git(
        &dir,
        &["rev-parse", "--verify", "--quiet", existing.as_str()],
    )?
    .ok
    {
        return Err(format!("分支「{name}」已存在"));
    }
    let start = match start_point.filter(|value| !value.trim().is_empty()) {
        Some(value) => resolve_commit(&dir, &value, "起点")?,
        None => resolve_commit(&dir, "HEAD", "当前提交")?,
    };

    let outcome = if checkout {
        run_git(&dir, &["checkout", "-b", name.as_str(), start.as_str()])?
    } else {
        run_git(&dir, &["branch", name.as_str(), start.as_str()])?
    };
    let success = if checkout {
        format!("已创建并切换到分支 {name}")
    } else {
        format!("已创建分支 {name}")
    };
    finish(&store, outcome, success, false)
}

#[tauri::command]
pub fn rename_branch(
    store: State<'_, RepoStore>,
    old_name: String,
    new_name: String,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    let dir = workdir(&store)?;
    ensure_local_branch(&dir, &old_name)?;
    validate_branch_name(&dir, &new_name)?;
    if old_name == new_name {
        return Err("新分支名与原名相同".to_string());
    }
    let existing = format!("refs/heads/{new_name}");
    if run_git(
        &dir,
        &["rev-parse", "--verify", "--quiet", existing.as_str()],
    )?
    .ok
    {
        return Err(format!("分支「{new_name}」已存在"));
    }
    let outcome = run_git(
        &dir,
        &["branch", "-m", old_name.as_str(), new_name.as_str()],
    )?;
    finish(
        &store,
        outcome,
        format!("已将分支 {old_name} 重命名为 {new_name}"),
        false,
    )
}

#[tauri::command]
pub fn delete_branch(
    store: State<'_, RepoStore>,
    name: String,
    force: bool,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    let dir = workdir(&store)?;
    ensure_local_branch(&dir, &name)?;
    let status = read_status(&store)?;
    if !status.detached && status.branch == name {
        return Err(format!("不能删除当前所在的分支「{name}」"));
    }
    let flag = if force { "-D" } else { "-d" };
    let outcome = run_git(&dir, &["branch", flag, name.as_str()])?;
    finish(&store, outcome, format!("已删除分支 {name}"), false)
}

#[tauri::command]
pub fn merge_branch(
    store: State<'_, RepoStore>,
    source: String,
    target: String,
    no_ff: bool,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    ensure_no_operation(&store)?;
    let dir = workdir(&store)?;
    ensure_local_branch(&dir, &target)?;
    if source == target {
        return Err("不能将分支合并到它自己".to_string());
    }
    resolve_commit(&dir, &source, "来源")?;

    let checkout = run_git(&dir, &["checkout", target.as_str()])?;
    if !checkout.ok {
        return Err(checkout.text);
    }

    let outcome = if no_ff {
        run_git(&dir, &["merge", "--no-ff", "--no-edit", source.as_str()])?
    } else {
        run_git(&dir, &["merge", "--no-edit", source.as_str()])?
    };
    finish(
        &store,
        outcome,
        format!("已将 {source} 合并到 {target}"),
        true,
    )
}

#[tauri::command]
pub fn rebase_branch(
    store: State<'_, RepoStore>,
    source: String,
    onto: String,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    ensure_no_operation(&store)?;
    let dir = workdir(&store)?;
    ensure_local_branch(&dir, &source)?;
    let target = resolve_commit(&dir, &onto, "目标")?;

    let checkout = run_git(&dir, &["checkout", source.as_str()])?;
    if !checkout.ok {
        return Err(checkout.text);
    }
    let outcome = run_git(&dir, &["rebase", target.as_str()])?;
    finish(
        &store,
        outcome,
        format!("已将 {source} 变基到 {onto}"),
        true,
    )
}

#[tauri::command]
pub fn cherry_pick(
    store: State<'_, RepoStore>,
    oid: String,
    target: Option<String>,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    ensure_no_operation(&store)?;
    ensure_oid(&oid)?;
    let dir = workdir(&store)?;
    let commit = resolve_commit(&dir, &oid, "提交")?;
    let target_name = target.filter(|value| !value.trim().is_empty());
    if let Some(name) = target_name.as_deref() {
        ensure_local_branch(&dir, name)?;
        let checkout = run_git(&dir, &["checkout", name])?;
        if !checkout.ok {
            return Err(checkout.text);
        }
    }
    let outcome = run_git(&dir, &["cherry-pick", commit.as_str()])?;
    let short: String = commit.chars().take(7).collect();
    let success = match target_name {
        Some(name) => format!("已将提交 {short} 拣选到 {name}"),
        None => format!("已拣选提交 {short}"),
    };
    finish(&store, outcome, success, true)
}

#[tauri::command]
pub fn cherry_pick_many(
    store: State<'_, RepoStore>,
    oids: Vec<String>,
    target: Option<String>,
) -> Result<OperationResult, String> {
    if oids.is_empty() {
        return Err("没有选择提交".to_string());
    }
    if oids.len() > 100 {
        return Err("一次最多拣选 100 个提交".to_string());
    }
    let _guard = lock_ops(&store)?;
    ensure_no_operation(&store)?;
    let dir = workdir(&store)?;
    let mut commits: Vec<String> = Vec::new();
    for oid in &oids {
        ensure_oid(oid)?;
        let commit = resolve_commit(&dir, oid, "提交")?;
        if !commits.contains(&commit) {
            commits.push(commit);
        }
    }
    let target_name = target.filter(|value| !value.trim().is_empty());
    if let Some(name) = target_name.as_deref() {
        ensure_local_branch(&dir, name)?;
        let checkout = run_git(&dir, &["checkout", name])?;
        if !checkout.ok {
            return Err(checkout.text);
        }
    }
    let mut args: Vec<&str> = vec!["cherry-pick"];
    args.extend(commits.iter().map(String::as_str));
    let outcome = run_git(&dir, &args)?;
    let success = match target_name {
        Some(name) => format!("已将 {} 个提交拣选到 {name}", commits.len()),
        None => format!("已拣选 {} 个提交", commits.len()),
    };
    finish(&store, outcome, success, true)
}

#[tauri::command]
pub fn revert_commit(store: State<'_, RepoStore>, oid: String) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    ensure_no_operation(&store)?;
    ensure_oid(&oid)?;
    let dir = workdir(&store)?;
    let commit = resolve_commit(&dir, &oid, "提交")?;
    let outcome = run_git(&dir, &["revert", "--no-edit", commit.as_str()])?;
    let short: String = commit.chars().take(7).collect();
    finish(&store, outcome, format!("已还原提交 {short}"), true)
}

#[tauri::command]
pub fn reset_branch(
    store: State<'_, RepoStore>,
    branch: String,
    oid: String,
    mode: String,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    ensure_no_operation(&store)?;
    ensure_oid(&oid)?;
    let dir = workdir(&store)?;
    ensure_local_branch(&dir, &branch)?;
    let target = resolve_commit(&dir, &oid, "提交")?;
    let mode = match mode.as_str() {
        "soft" => "soft",
        "mixed" => "mixed",
        "hard" => "hard",
        _ => return Err("未知的重置模式".to_string()),
    };

    let status = read_status(&store)?;
    let is_current = !status.detached && status.branch == branch;
    if !is_current && mode != "hard" {
        return Err("非当前分支只支持硬重置".to_string());
    }

    let outcome = if is_current {
        let flag = format!("--{mode}");
        run_git(&dir, &["reset", flag.as_str(), target.as_str()])?
    } else {
        run_git(&dir, &["branch", "-f", branch.as_str(), target.as_str()])?
    };
    let short: String = target.chars().take(7).collect();
    let label = match mode {
        "soft" => "软重置",
        "mixed" => "混合重置",
        _ => "硬重置",
    };
    finish(
        &store,
        outcome,
        format!("已将 {branch} {label}到 {short}"),
        false,
    )
}

#[tauri::command]
pub fn create_tag(
    store: State<'_, RepoStore>,
    name: String,
    oid: String,
    message: Option<String>,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    ensure_oid(&oid)?;
    let dir = workdir(&store)?;
    validate_tag_name(&dir, &name)?;
    let existing = format!("refs/tags/{name}");
    if run_git(
        &dir,
        &["rev-parse", "--verify", "--quiet", existing.as_str()],
    )?
    .ok
    {
        return Err(format!("标签「{name}」已存在"));
    }
    let target = resolve_commit(&dir, &oid, "提交")?;
    let trimmed = message.unwrap_or_default();
    let trimmed = trimmed.trim();
    let outcome = if trimmed.is_empty() {
        run_git(&dir, &["tag", name.as_str(), target.as_str()])?
    } else {
        run_git(
            &dir,
            &["tag", "-a", name.as_str(), "-m", trimmed, target.as_str()],
        )?
    };
    finish(&store, outcome, format!("已创建标签 {name}"), false)
}

#[tauri::command]
pub fn delete_tag(store: State<'_, RepoStore>, name: String) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    let dir = workdir(&store)?;
    validate_tag_name(&dir, &name)?;
    let reference = format!("refs/tags/{name}");
    if !run_git(
        &dir,
        &["rev-parse", "--verify", "--quiet", reference.as_str()],
    )?
    .ok
    {
        return Err(format!("标签「{name}」不存在"));
    }
    let outcome = run_git(&dir, &["tag", "-d", name.as_str()])?;
    finish(&store, outcome, format!("已删除标签 {name}"), false)
}

#[tauri::command]
pub fn fetch_remote(
    store: State<'_, RepoStore>,
    remote: Option<String>,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    let dir = workdir(&store)?;
    let outcome = match remote.filter(|value| !value.trim().is_empty()) {
        Some(name) => {
            ensure_remote(&store, &name)?;
            let result = run_git(&dir, &["fetch", "--prune", name.as_str()])?;
            (result, format!("已从 {name} 获取最新提交"))
        }
        None => {
            let result = run_git(&dir, &["fetch", "--all", "--prune"])?;
            (result, "已从所有远程获取最新提交".to_string())
        }
    };
    finish(&store, outcome.0, outcome.1, false)
}

#[tauri::command]
pub fn pull_branch(
    store: State<'_, RepoStore>,
    remote: Option<String>,
    branch: Option<String>,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    ensure_no_operation(&store)?;
    let dir = workdir(&store)?;
    let status = read_status(&store)?;
    if status.detached {
        return Err("当前处于分离 HEAD 状态，无法拉取".to_string());
    }

    let remote = remote.filter(|value| !value.trim().is_empty());
    let branch = branch.filter(|value| !value.trim().is_empty());
    let outcome = match (remote.as_deref(), branch.as_deref()) {
        (Some(remote), Some(branch)) => {
            ensure_remote(&store, remote)?;
            validate_branch_name(&dir, branch)?;
            run_git(&dir, &["pull", "--no-edit", remote, branch])?
        }
        (Some(remote), None) => {
            ensure_remote(&store, remote)?;
            run_git(&dir, &["pull", "--no-edit", remote])?
        }
        _ => run_git(&dir, &["pull", "--no-edit"])?,
    };
    let target = branch.unwrap_or(status.branch);
    finish(&store, outcome, format!("已拉取 {target} 的最新提交"), true)
}

#[tauri::command]
pub fn push_branch(
    store: State<'_, RepoStore>,
    branch: String,
    remote: String,
    set_upstream: bool,
    force_with_lease: bool,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    let dir = workdir(&store)?;
    ensure_local_branch(&dir, &branch)?;
    ensure_remote(&store, &remote)?;
    let refspec = format!("{branch}:{branch}");
    let mut args = vec!["push"];
    if set_upstream {
        args.push("--set-upstream");
    }
    if force_with_lease {
        args.push("--force-with-lease");
    }
    args.push(remote.as_str());
    args.push(refspec.as_str());
    let outcome = run_git(&dir, &args)?;
    let suffix = if force_with_lease {
        "（--force-with-lease）"
    } else {
        ""
    };
    finish(
        &store,
        outcome,
        format!("已将 {branch} 推送到 {remote}{suffix}"),
        false,
    )
}

#[tauri::command]
pub fn continue_operation(
    store: State<'_, RepoStore>,
    kind: String,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    let dir = workdir(&store)?;
    let (args, success, conflict_capable) = match kind.as_str() {
        "merge" => (vec!["merge", "--continue"], "合并已完成", true),
        "rebase" => (vec!["rebase", "--continue"], "变基已完成", true),
        "cherry-pick" => (vec!["cherry-pick", "--continue"], "拣选已完成", true),
        "revert" => (vec!["revert", "--continue"], "还原已完成", true),
        _ => return Err("未知的待继续操作".to_string()),
    };
    let outcome = run_git(&dir, &args)?;
    finish(&store, outcome, success, conflict_capable)
}

#[tauri::command]
pub fn abort_operation(
    store: State<'_, RepoStore>,
    kind: String,
) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    let dir = workdir(&store)?;
    let (args, success) = match kind.as_str() {
        "merge" => (vec!["merge", "--abort"], "已中止合并"),
        "rebase" => (vec!["rebase", "--abort"], "已中止变基"),
        "cherry-pick" => (vec!["cherry-pick", "--abort"], "已中止拣选"),
        "revert" => (vec!["revert", "--abort"], "已中止还原"),
        _ => return Err("未知的待中止操作".to_string()),
    };
    let outcome = run_git(&dir, &args)?;
    finish(&store, outcome, success, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_url_credentials() {
        assert_eq!(
            redact_url_credentials(
                "fatal: unable to access 'https://user:secret@gitlab.example.com/g/repo.git/'"
            ),
            "fatal: unable to access 'https://***@gitlab.example.com/g/repo.git/'"
        );
        assert_eq!(
            redact_url_credentials("ssh://git@example.com/repo.git"),
            "ssh://***@example.com/repo.git"
        );
        assert_eq!(
            redact_url_credentials("https://example.com/repo.git"),
            "https://example.com/repo.git"
        );
        assert_eq!(redact_url_credentials("没有网址"), "没有网址");
    }

    #[test]
    fn clips_long_output_from_the_tail() {
        let raw = format!("{}{}", "start\n", "x".repeat(OUTPUT_LIMIT + 120));
        let clipped = clip_output(&raw);
        assert!(clipped.chars().count() <= OUTPUT_LIMIT + 16);
        assert!(clipped.starts_with("…（输出已截断）"));
    }

    #[test]
    fn rejects_dangerous_names() {
        assert!(reject_dash("-u", "分支名").is_err());
        assert!(reject_dash("", "分支名").is_err());
        assert!(reject_dash("a\nb", "分支名").is_err());
        assert!(reject_dash("feature/graph", "分支名").is_ok());
        assert!(ensure_oid("abc1234").is_ok());
        assert!(ensure_oid("-abc1234").is_err());
        assert!(ensure_oid("not-a-sha").is_err());
    }
}
