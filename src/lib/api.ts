import { Channel, invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type {
  AiEvent,
  AiModels,
  AppConfig,
  ChatMessage,
  CommitDetail,
  CommitOutcome,
  CommitPage,
  CommitPatch,
  DiscardMode,
  FileDiff,
  GitLabBranch,
  GitLabProjectRef,
  GitLabUser,
  MergeRequest,
  MergeRequestChanges,
  MergeRequestFilter,
  MergeRequestNote,
  MergeRequestPage,
  OperationKind,
  OperationResult,
  RepoRefs,
  RepoRemoteInfo,
  RepoStatus,
  RepoSummary,
  ResetMode,
  StashEntry,
  WorkingDiff,
  WorkingFiles,
  WorkingStage,
} from "../types";

export function getSettings(): Promise<AppConfig> {
  return invoke<AppConfig>("get_settings");
}

export function saveSettings(config: AppConfig): Promise<void> {
  return invoke<void>("save_settings", { config });
}

export function setSecret(account: string, value: string): Promise<void> {
  return invoke<void>("set_secret", { account, value });
}

export function hasSecret(account: string): Promise<boolean> {
  return invoke<boolean>("has_secret", { account });
}

export function deleteSecret(account: string): Promise<void> {
  return invoke<void>("delete_secret", { account });
}

export function testGitLab(
  baseUrl: string,
  token: string | null,
  account: string | null,
): Promise<GitLabUser> {
  return invoke<GitLabUser>("test_gitlab", { baseUrl, token, account });
}

export function testAi(
  baseUrl: string,
  apiKey: string | null,
  account: string | null,
): Promise<AiModels> {
  return invoke<AiModels>("test_ai", { baseUrl, apiKey, account });
}

export function openRepository(path: string): Promise<RepoSummary> {
  return invoke<RepoSummary>("open_repository", { path });
}

export function inspectRepository(path: string): Promise<RepoSummary> {
  return invoke<RepoSummary>("inspect_repository", { path });
}

export function loadGraph(skip: number, limit: number): Promise<CommitPage> {
  return invoke<CommitPage>("load_graph", { skip, limit });
}

export function getCommit(oid: string): Promise<CommitDetail> {
  return invoke<CommitDetail>("get_commit", { oid });
}

export function getWorkingDiff(staged: boolean): Promise<WorkingDiff> {
  return invoke<WorkingDiff>("get_working_diff", { staged });
}

export function getCommitPatch(oid: string): Promise<CommitPatch> {
  return invoke<CommitPatch>("get_commit_patch", { oid });
}

export function getRepoRefs(): Promise<RepoRefs> {
  return invoke<RepoRefs>("get_repo_refs");
}

export function getRepoStatus(): Promise<RepoStatus> {
  return invoke<RepoStatus>("get_repo_status");
}

export function checkoutBranch(name: string, kind: string): Promise<OperationResult> {
  return invoke<OperationResult>("checkout_branch", { name, kind });
}

export function checkoutCommit(oid: string): Promise<OperationResult> {
  return invoke<OperationResult>("checkout_commit", { oid });
}

export function createBranch(
  name: string,
  startPoint: string | null,
  checkout: boolean,
): Promise<OperationResult> {
  return invoke<OperationResult>("create_branch", { name, startPoint, checkout });
}

export function renameBranch(oldName: string, newName: string): Promise<OperationResult> {
  return invoke<OperationResult>("rename_branch", { oldName, newName });
}

export function deleteBranch(name: string, force: boolean): Promise<OperationResult> {
  return invoke<OperationResult>("delete_branch", { name, force });
}

export function mergeBranch(
  source: string,
  target: string,
  noFf: boolean,
): Promise<OperationResult> {
  return invoke<OperationResult>("merge_branch", { source, target, noFf });
}

export function rebaseBranch(source: string, onto: string): Promise<OperationResult> {
  return invoke<OperationResult>("rebase_branch", { source, onto });
}

export function cherryPick(oid: string, target: string | null): Promise<OperationResult> {
  return invoke<OperationResult>("cherry_pick", { oid, target });
}

export function revertCommit(oid: string): Promise<OperationResult> {
  return invoke<OperationResult>("revert_commit", { oid });
}

export function resetBranch(
  branch: string,
  oid: string,
  mode: ResetMode,
): Promise<OperationResult> {
  return invoke<OperationResult>("reset_branch", { branch, oid, mode });
}

export function createTag(
  name: string,
  oid: string,
  message: string | null,
): Promise<OperationResult> {
  return invoke<OperationResult>("create_tag", { name, oid, message });
}

export function deleteTag(name: string): Promise<OperationResult> {
  return invoke<OperationResult>("delete_tag", { name });
}

export function fetchRemote(remote: string | null): Promise<OperationResult> {
  return invoke<OperationResult>("fetch_remote", { remote });
}

export function pullBranch(
  remote: string | null,
  branch: string | null,
): Promise<OperationResult> {
  return invoke<OperationResult>("pull_branch", { remote, branch });
}

export function pushBranch(
  branch: string,
  remote: string,
  setUpstream: boolean,
  forceWithLease: boolean,
): Promise<OperationResult> {
  return invoke<OperationResult>("push_branch", {
    branch,
    remote,
    setUpstream,
    forceWithLease,
  });
}

export function continueOperation(kind: OperationKind): Promise<OperationResult> {
  return invoke<OperationResult>("continue_operation", { kind });
}

export function abortOperation(kind: OperationKind): Promise<OperationResult> {
  return invoke<OperationResult>("abort_operation", { kind });
}

export function getWorkingFiles(): Promise<WorkingFiles> {
  return invoke<WorkingFiles>("get_working_files");
}

export function getFileDiff(path: string, stage: WorkingStage): Promise<FileDiff> {
  return invoke<FileDiff>("get_file_diff", { path, stage });
}

export function applyHunk(
  path: string,
  stage: WorkingStage,
  hunkIndex: number,
): Promise<OperationResult> {
  return invoke<OperationResult>("apply_hunk", { path, stage, hunkIndex });
}

export function stageFiles(paths: string[]): Promise<OperationResult> {
  return invoke<OperationResult>("stage_files", { paths });
}

export function unstageFiles(paths: string[]): Promise<OperationResult> {
  return invoke<OperationResult>("unstage_files", { paths });
}

export function stageAll(): Promise<OperationResult> {
  return invoke<OperationResult>("stage_all");
}

export function unstageAll(): Promise<OperationResult> {
  return invoke<OperationResult>("unstage_all");
}

export function discardFiles(paths: string[], mode: DiscardMode): Promise<OperationResult> {
  return invoke<OperationResult>("discard_files", { paths, mode });
}

export function commitIdentity(): Promise<string | null> {
  return invoke<string | null>("commit_identity");
}

export function commitChanges(
  summary: string,
  description: string,
  amend: boolean,
): Promise<CommitOutcome> {
  return invoke<CommitOutcome>("commit_changes", { summary, description, amend });
}

export function listStashes(): Promise<StashEntry[]> {
  return invoke<StashEntry[]>("list_stashes");
}

export function stashPush(message: string, includeUntracked: boolean): Promise<OperationResult> {
  return invoke<OperationResult>("stash_push", { message, includeUntracked });
}

export function stashApply(reference: string, drop: boolean): Promise<OperationResult> {
  return invoke<OperationResult>("stash_apply", { reference, drop });
}

export function stashDrop(reference: string): Promise<OperationResult> {
  return invoke<OperationResult>("stash_drop", { reference });
}

export function chatStream(
  profileId: string | null,
  messages: ChatMessage[],
  onEvent: (event: AiEvent) => void,
): Promise<void> {
  const channel = new Channel<AiEvent>();
  channel.onmessage = onEvent;
  return invoke<void>("chat_stream", { profileId, messages, onEvent: channel });
}

export function cancelAi(): Promise<void> {
  return invoke<void>("cancel_ai");
}

export async function pickRepository(): Promise<string | null> {
  const selected = await openDialog({
    directory: true,
    multiple: false,
    title: "选择 Git 仓库目录",
  });
  return typeof selected === "string" ? selected : null;
}

export function getRepoRemote(): Promise<RepoRemoteInfo> {
  return invoke<RepoRemoteInfo>("repo_remote_info");
}

export function resolveGitLabProject(instanceId: string): Promise<GitLabProjectRef> {
  return invoke<GitLabProjectRef>("resolve_gitlab_project", { instanceId });
}

export function listMergeRequests(
  instanceId: string,
  projectPath: string,
  state: MergeRequestFilter,
  page: number,
  search: string | null,
): Promise<MergeRequestPage> {
  return invoke<MergeRequestPage>("list_merge_requests", {
    instanceId,
    projectPath,
    state,
    page,
    search,
  });
}

export function getMergeRequest(
  instanceId: string,
  projectPath: string,
  iid: number,
): Promise<MergeRequest> {
  return invoke<MergeRequest>("get_merge_request", { instanceId, projectPath, iid });
}

export function listMergeRequestChanges(
  instanceId: string,
  projectPath: string,
  iid: number,
): Promise<MergeRequestChanges> {
  return invoke<MergeRequestChanges>("list_merge_request_changes", {
    instanceId,
    projectPath,
    iid,
  });
}

export function listMergeRequestNotes(
  instanceId: string,
  projectPath: string,
  iid: number,
): Promise<MergeRequestNote[]> {
  return invoke<MergeRequestNote[]>("list_merge_request_notes", {
    instanceId,
    projectPath,
    iid,
  });
}

export function listGitLabBranches(
  instanceId: string,
  projectPath: string,
): Promise<GitLabBranch[]> {
  return invoke<GitLabBranch[]>("list_gitlab_branches", { instanceId, projectPath });
}

export function createMergeRequest(
  instanceId: string,
  projectPath: string,
  sourceBranch: string,
  targetBranch: string,
  title: string,
  description: string | null,
  removeSourceBranch: boolean | null,
): Promise<MergeRequest> {
  return invoke<MergeRequest>("create_merge_request", {
    instanceId,
    projectPath,
    sourceBranch,
    targetBranch,
    title,
    description,
    removeSourceBranch,
  });
}

export function mergeMergeRequest(
  instanceId: string,
  projectPath: string,
  iid: number,
  squash: boolean | null,
  removeSourceBranch: boolean | null,
): Promise<MergeRequest> {
  return invoke<MergeRequest>("merge_merge_request", {
    instanceId,
    projectPath,
    iid,
    squash,
    removeSourceBranch,
  });
}

export function setMergeRequestState(
  instanceId: string,
  projectPath: string,
  iid: number,
  event: "close" | "reopen",
): Promise<MergeRequest> {
  return invoke<MergeRequest>("set_merge_request_state", {
    instanceId,
    projectPath,
    iid,
    event,
  });
}

export function errorText(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}
