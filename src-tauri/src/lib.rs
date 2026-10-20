mod commands;
mod config;

use commands::ai::AiState;
use commands::repo::RepoStore;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(RepoStore::default())
        .manage(AiState::default())
        .invoke_handler(tauri::generate_handler![
            config::get_settings,
            config::save_settings,
            config::secrets::set_secret,
            config::secrets::has_secret,
            config::secrets::delete_secret,
            commands::settings::test_gitlab,
            commands::settings::test_ai,
            commands::repo::open_repository,
            commands::repo::inspect_repository,
            commands::repo::load_graph,
            commands::repo::get_commit,
            commands::repo::get_working_diff,
            commands::repo::get_commit_patch,
            commands::repo::get_repo_refs,
            commands::repo::get_repo_status,
            commands::branch::checkout_branch,
            commands::branch::checkout_commit,
            commands::branch::create_branch,
            commands::branch::rename_branch,
            commands::branch::delete_branch,
            commands::branch::merge_branch,
            commands::branch::rebase_branch,
            commands::branch::cherry_pick,
            commands::branch::revert_commit,
            commands::branch::reset_branch,
            commands::branch::create_tag,
            commands::branch::delete_tag,
            commands::branch::fetch_remote,
            commands::branch::pull_branch,
            commands::branch::push_branch,
            commands::branch::continue_operation,
            commands::branch::abort_operation,
            commands::workspace::get_working_files,
            commands::workspace::get_file_diff,
            commands::workspace::apply_hunk,
            commands::workspace::stage_files,
            commands::workspace::unstage_files,
            commands::workspace::stage_all,
            commands::workspace::unstage_all,
            commands::workspace::discard_files,
            commands::workspace::commit_identity,
            commands::workspace::commit_changes,
            commands::workspace::list_stashes,
            commands::workspace::stash_push,
            commands::workspace::stash_apply,
            commands::workspace::stash_drop,
            commands::ai::chat_stream,
            commands::ai::cancel_ai,
            commands::gitlab::repo_remote_info,
            commands::gitlab::resolve_gitlab_project,
            commands::gitlab::list_merge_requests,
            commands::gitlab::get_merge_request,
            commands::gitlab::list_merge_request_changes,
            commands::gitlab::list_merge_request_notes,
            commands::gitlab::list_gitlab_branches,
            commands::gitlab::create_merge_request,
            commands::gitlab::merge_merge_request,
            commands::gitlab::set_merge_request_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
