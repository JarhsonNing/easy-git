use std::path::Path;

use serde::Serialize;
use tauri::State;

use super::branch::{ensure_no_operation, finish, run_git};
use super::repo::{OperationResult, RepoStore, lock_ops, read_status, workdir};

const HEAD_FORMAT: &str = "--format=%H%x1f%gs";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoInfo {
    pub summary: String,
    pub detail: String,
    pub destructive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Restore {
    /// reset --soft：撤销提交时把改动留在暂存区
    Soft,
    /// checkout：撤销一次分支切换
    Checkout,
    /// reset --hard：恢复分支指针，工作区必须干净
    Hard,
}

struct Plan {
    summary: String,
    detail: String,
    restore: Restore,
    target: String,
}

fn short(oid: &str) -> String {
    oid.chars().take(7).collect()
}

/// reset 的 reflog 会写进完整的四十二位哈希，展示时收短；HEAD~2 这类写法原样保留。
fn compact_target(target: &str) -> String {
    let looks_like_oid =
        target.len() >= 7 && target.chars().all(|c| c.is_ascii_hexdigit());
    if looks_like_oid {
        short(target)
    } else {
        target.to_string()
    }
}

fn split_reflog(line: &str) -> Option<(&str, &str)> {
    let (oid, subject) = line.split_once('\u{1f}')?;
    (!oid.is_empty() && !subject.is_empty()).then_some((oid, subject))
}

fn checkout_move(subject: &str) -> Option<(&str, &str)> {
    let rest = subject.strip_prefix("checkout: moving from ")?;
    let (from, to) = rest.split_once(" to ")?;
    (!from.is_empty() && !to.is_empty()).then_some((from, to))
}

fn to_plan(
    dir: &Path,
    subject: &str,
    head_oid: &str,
    previous_oid: &str,
) -> Result<Option<Plan>, String> {
    let head = short(head_oid);
    let previous = short(previous_oid);
    let subject = subject.trim();

    if subject.starts_with("commit (amend)") {
        return Ok(Some(Plan {
            summary: format!("上次的提交修正（{head}）"),
            detail: format!("分支退回修正前的 {previous}，被修正掉的改动回到暂存区，不会丢失。"),
            restore: Restore::Soft,
            target: previous_oid.to_string(),
        }));
    }
    if subject.starts_with("commit") {
        return Ok(Some(Plan {
            summary: format!("提交 {head}"),
            detail: format!("分支退回 {previous}，这次提交的改动回到暂存区，不会丢失。"),
            restore: Restore::Soft,
            target: previous_oid.to_string(),
        }));
    }
    if let Some((from, to)) = checkout_move(subject) {
        return Ok(Some(Plan {
            summary: format!("切换到 {to}"),
            detail: format!("把 HEAD 切回 {from}，工作区里未提交的改动会一起带过去。"),
            restore: Restore::Checkout,
            target: from.to_string(),
        }));
    }
    if subject.starts_with("merge ") {
        return Ok(Some(Plan {
            summary: format!("合并（{}）", subject.trim_start_matches("merge ").split(':').next().unwrap_or("").trim()),
            detail: format!("分支退回合并前的 {previous}，这次合并带进来的提交不会从仓库里消失。"),
            restore: Restore::Hard,
            target: previous_oid.to_string(),
        }));
    }
    if subject.starts_with("rebase") {
        // 变基的 reflog 上一条是回放过程，不是原点，只有 ORIG_HEAD 可靠
        let origin = run_git(dir, &["rev-parse", "--verify", "--quiet", "ORIG_HEAD"])?;
        let target = origin.text.trim().to_string();
        if !origin.ok || target.is_empty() {
            return Ok(None);
        }
        return Ok(Some(Plan {
            summary: "变基".to_string(),
            detail: format!("分支退回变基前的 {}，变基产生的提交仍然留在 reflog 里。", short(&target)),
            restore: Restore::Hard,
            target,
        }));
    }
    if subject.starts_with("pull") {
        return Ok(Some(Plan {
            summary: "拉取".to_string(),
            detail: format!("分支退回拉取前的 {previous}，从远端带进来的提交仍然留在远程跟踪分支上。"),
            restore: Restore::Hard,
            target: previous_oid.to_string(),
        }));
    }
    if subject.starts_with("reset: moving to") {
        let moved_to = subject
            .trim_start_matches("reset: moving to")
            .trim()
            .to_string();
        return Ok(Some(Plan {
            summary: format!("重置到 {}", compact_target(&moved_to)),
            detail: format!("分支退回重置前的 {previous}，被重置掉的提交会重新回到分支上。"),
            restore: Restore::Hard,
            target: previous_oid.to_string(),
        }));
    }
    if subject.starts_with("cherry-pick") {
        return Ok(Some(Plan {
            summary: format!("拣选 {head}"),
            detail: format!("分支退回拣选前的 {previous}。"),
            restore: Restore::Hard,
            target: previous_oid.to_string(),
        }));
    }
    if subject.starts_with("revert") {
        return Ok(Some(Plan {
            summary: format!("还原 {head}"),
            detail: format!("分支退回还原前的 {previous}。"),
            restore: Restore::Hard,
            target: previous_oid.to_string(),
        }));
    }
    Ok(None)
}

fn read_plan(store: &RepoStore) -> Result<Option<Plan>, String> {
    let dir = workdir(store)?;
    let outcome = run_git(&dir, &["reflog", "show", HEAD_FORMAT, "-n", "2", "HEAD"])?;
    if !outcome.ok {
        return Ok(None);
    }
    let mut lines = outcome.text.lines().filter(|line| !line.trim().is_empty());
    let (Some(current), Some(previous)) = (lines.next(), lines.next()) else {
        return Ok(None);
    };
    let Some((head_oid, subject)) = split_reflog(current) else {
        return Ok(None);
    };
    let Some((previous_oid, _)) = split_reflog(previous) else {
        return Ok(None);
    };
    to_plan(&dir, subject, head_oid, previous_oid)
}

#[tauri::command]
pub fn inspect_undo(store: State<'_, RepoStore>) -> Result<Option<UndoInfo>, String> {
    Ok(read_plan(&store)?.map(|plan| UndoInfo {
        summary: plan.summary,
        detail: plan.detail,
        destructive: plan.restore == Restore::Hard,
    }))
}

#[tauri::command]
pub fn undo_last(store: State<'_, RepoStore>) -> Result<OperationResult, String> {
    let _guard = lock_ops(&store)?;
    ensure_no_operation(&store)?;
    let plan = read_plan(&store)?.ok_or_else(|| "没有可以撤销的操作".to_string())?;
    let dir = workdir(&store)?;
    let action = match plan.restore {
        Restore::Soft => {
            run_git(&dir, &["reset", "--soft", plan.target.as_str()])?
        }
        Restore::Checkout => {
            run_git(&dir, &["checkout", plan.target.as_str()])?
        }
        Restore::Hard => {
            // reset --hard 只丢已跟踪的改动；未跟踪文件不在撤销范围内，
            // 真会撞上时 Git 自己会拒绝并给出文件名
            let status = read_status(&store)?;
            if status.staged + status.unstaged + status.conflicted > 0 {
                return Err(
                    "工作区还有未提交的改动，撤销会覆盖它们。请先提交或贮藏后再撤销。"
                        .to_string(),
                );
            }
            run_git(&dir, &["reset", "--hard", plan.target.as_str()])?
        }
    };
    finish(&store, action, format!("已撤销{}", plan.summary), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan_for(subject: &str) -> Option<Plan> {
        to_plan(Path::new("."), subject, "aaaaaaa1111", "bbbbbbb2222").ok().flatten()
    }

    #[test]
    fn commit_undo_is_soft_so_work_is_kept() {
        let plan = plan_for("commit: add graph lane filter").expect("plan");
        assert_eq!(plan.restore, Restore::Soft);
        assert_eq!(plan.target, "bbbbbbb2222");
        assert!(plan.summary.contains("aaaaaaa"));
    }

    #[test]
    fn amend_undo_is_soft_and_named_differently() {
        let plan = plan_for("commit (amend): tweak").expect("plan");
        assert_eq!(plan.restore, Restore::Soft);
        assert!(plan.summary.contains("修正"));
    }

    #[test]
    fn checkout_undo_returns_to_the_previous_ref_name() {
        let plan = plan_for("checkout: moving from main to feature/x").expect("plan");
        assert_eq!(plan.restore, Restore::Checkout);
        assert_eq!(plan.target, "main");
        assert!(plan.summary.contains("feature/x"));
    }

    #[test]
    fn merge_and_pull_undo_reset_hard_to_previous_head() {
        for subject in ["merge feature: Merge made by the 'ort' strategy.", "pull: Fast-forward"] {
            let plan = plan_for(subject).expect("plan");
            assert_eq!(plan.restore, Restore::Hard);
            assert_eq!(plan.target, "bbbbbbb2222");
        }
    }

    #[test]
    fn reset_undo_points_back_before_the_reset() {
        let plan = plan_for("reset: moving to HEAD~2").expect("plan");
        assert_eq!(plan.restore, Restore::Hard);
        assert_eq!(plan.target, "bbbbbbb2222");
        assert!(plan.summary.contains("HEAD~2"));
    }

    #[test]
    fn reset_summary_shortens_a_full_oid_but_keeps_symbolic_names() {
        let oid = plan_for("reset: moving to 1a28df5aae89320ac17c33c069df77d6be430f0a").expect("plan");
        assert!(oid.summary.contains("1a28df5"));
        assert!(!oid.summary.contains("aae8932"));
        assert_eq!(oid.target, "bbbbbbb2222");
        let symbolic = plan_for("reset: moving to origin/main").expect("plan");
        assert!(symbolic.summary.contains("origin/main"));
    }

    #[test]
    fn unknown_reflog_subjects_are_not_offered_as_undo() {
        for subject in ["clone: from somewhere", "branch: Created from main", "notes: x"] {
            assert!(plan_for(subject).is_none());
        }
    }
}
