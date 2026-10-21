export interface GitLabInstance {
  id: string;
  name: string;
  baseUrl: string;
}

export interface AiProfile {
  id: string;
  name: string;
  baseUrl: string;
  model: string;
  temperature: number;
  maxTokens: number;
}

export interface ProjectEntry {
  path: string;
  name: string;
}

export interface AppConfig {
  projects: ProjectEntry[];
  gitlab: {
    instances: GitLabInstance[];
  };
  ai: {
    activeProfileId: string | null;
    profiles: AiProfile[];
  };
  git: {
    pageSize: number;
    maxLanes: number;
  };
}

export interface GitLabUser {
  id: number;
  username: string;
  name: string;
  webUrl: string;
}

export interface AiModels {
  models: string[];
}

export type SecretKind = "gitlab" | "ai";

export function secretAccount(kind: SecretKind, id: string): string {
  return `${kind}:${id}`;
}

export interface RepoSummary {
  name: string;
  path: string;
  headBranch: string;
  headOid: string | null;
  remoteUrl: string | null;
}

export type RefKind = "head" | "branch" | "remote" | "tag";

export interface RefLabel {
  name: string;
  kind: RefKind;
}

export interface LaneParent {
  oid: string;
  lane: number;
}

export interface CommitRow {
  oid: string;
  shortId: string;
  summary: string;
  authorName: string;
  authoredAt: number;
  lane: number;
  parents: LaneParent[];
  through: number[];
  refs: RefLabel[];
}

export interface CommitPage {
  rows: CommitRow[];
  hasMore: boolean;
}

export interface CommitFile {
  path: string;
  status: string;
  additions: number;
  deletions: number;
}

export interface CommitDetail {
  oid: string;
  shortId: string;
  summary: string;
  message: string;
  authorName: string;
  authorEmail: string;
  authoredAt: number;
  committerName: string;
  committedAt: number;
  parents: string[];
  filesChanged: number;
  insertions: number;
  deletions: number;
  files: CommitFile[];
}

export interface RepoRemoteInfo {
  remoteUrl: string | null;
  host: string | null;
  path: string | null;
}

export interface GitLabProjectRef {
  remoteUrl: string;
  remoteHost: string;
  projectPath: string;
  webUrl: string;
}

export interface MergeRequest {
  id: number;
  iid: number;
  title: string;
  description: string;
  state: string;
  draft: boolean;
  sourceBranch: string;
  targetBranch: string;
  webUrl: string;
  createdAt: string;
  updatedAt: string;
  mergedAt: string | null;
  userNotesCount: number;
  labels: string[];
  authorName: string;
  authorUsername: string;
  authorAvatarUrl: string;
  hasConflicts: boolean;
  mergeStatus: string;
}

export interface MergeRequestPage {
  items: MergeRequest[];
  page: number;
  hasMore: boolean;
}

export interface MergeRequestFile {
  oldPath: string;
  newPath: string;
  diff: string;
  newFile: boolean;
  renamedFile: boolean;
  deletedFile: boolean;
  tooLarge: boolean;
}

export interface MergeRequestChanges {
  files: MergeRequestFile[];
  overflow: boolean;
}

export interface MergeRequestNote {
  id: number;
  body: string;
  authorName: string;
  authorUsername: string;
  createdAt: string;
  system: boolean;
  resolvable: boolean;
  resolved: boolean;
}

export interface GitLabBranch {
  name: string;
  isDefault: boolean;
  merged: boolean;
  protected: boolean;
}

export type MergeRequestFilter = "opened" | "merged" | "closed" | "all";

export interface GitLabProject {
  id: number;
  name: string;
  nameWithNamespace: string;
  path: string;
  description: string;
  webUrl: string;
  defaultBranch: string;
  visibility: string;
  lastActivityAt: string;
  archived: boolean;
  forkedFromPath: string | null;
  forkedFromWebUrl: string | null;
}

export interface LocalProjectMatch {
  projectPath: string;
  localPath: string;
  name: string;
}

export interface GitLabProjectPage {
  items: GitLabProject[];
  page: number;
  hasMore: boolean;
}

export interface GitLabTreeEntry {
  name: string;
  path: string;
  isDir: boolean;
}

export interface GitLabTreePage {
  entries: GitLabTreeEntry[];
  hasMore: boolean;
}

export interface GitLabFileContent {
  path: string;
  content: string;
  binary: boolean;
  truncated: boolean;
  size: number;
}

export interface GitLabPipeline {
  id: number;
  iid: number;
  status: string;
  source: string;
  refName: string;
  sha: string;
  webUrl: string;
  createdAt: string;
  updatedAt: string;
  duration: number | null;
}

export interface GitLabPipelinePage {
  items: GitLabPipeline[];
  page: number;
  hasMore: boolean;
}

export interface GitLabJob {
  id: number;
  name: string;
  stage: string;
  status: string;
  webUrl: string;
  createdAt: string;
  startedAt: string | null;
  finishedAt: string | null;
  duration: number | null;
  runner: string;
  failureReason: string | null;
}

export interface GitLabJobTrace {
  trace: string;
  truncated: boolean;
  size: number;
}

export interface CloneResult {
  path: string;
  name: string;
}

export interface WorkingDiff {
  patch: string;
  filesChanged: number;
  insertions: number;
  deletions: number;
  truncated: boolean;
}

export interface CommitPatch {
  patch: string;
  truncated: boolean;
}

export type BranchKind = "local" | "remote";

export interface BranchInfo {
  name: string;
  kind: BranchKind;
  oid: string;
  shortId: string;
  summary: string;
  updatedAt: number;
  isCurrent: boolean;
  upstream: string | null;
  ahead: number;
  behind: number;
}

export interface TagInfo {
  name: string;
  oid: string;
  shortId: string;
  summary: string;
  updatedAt: number;
}

export interface RepoRefs {
  local: BranchInfo[];
  remote: BranchInfo[];
  tags: TagInfo[];
  remotes: string[];
}

export interface RemoteInfo {
  name: string;
  fetchUrl: string;
  pushUrl: string;
  hasCredentials: boolean;
}

export interface RepoStatus {
  branch: string;
  headOid: string | null;
  detached: boolean;
  upstream: string | null;
  ahead: number;
  behind: number;
  staged: number;
  unstaged: number;
  untracked: number;
  conflicted: number;
  state: string;
  conflicts: string[];
}

export interface OperationResult {
  message: string;
  output: string;
  conflicts: string[];
  state: string;
}

export type ResetMode = "soft" | "mixed" | "hard";
export type OperationKind = "merge" | "rebase" | "cherry-pick" | "revert";

export interface Notice {
  tone: "success" | "error" | "conflict";
  message: string;
  detail?: string;
  conflicts?: string[];
  state?: string;
}

export interface MenuItem {
  id?: string;
  label?: string;
  hint?: string;
  icon?: string;
  danger?: boolean;
  disabled?: boolean;
  separator?: boolean;
}

export interface DialogOption {
  value: string;
  label: string;
  help?: string;
}

export type DialogField =
  | {
      kind: "text";
      key: string;
      label: string;
      value?: string;
      placeholder?: string;
      required?: boolean;
      mono?: boolean;
      help?: string;
    }
  | {
      kind: "textarea";
      key: string;
      label: string;
      value?: string;
      placeholder?: string;
      help?: string;
    }
  | {
      kind: "select";
      key: string;
      label: string;
      options: DialogOption[];
      value: string;
      help?: string;
    }
  | {
      kind: "checkbox";
      key: string;
      label: string;
      value?: boolean;
      help?: string;
      danger?: boolean;
    }
  | {
      kind: "radio";
      key: string;
      label?: string;
      options: DialogOption[];
      value: string;
      help?: string;
    }
  | {
      kind: "note";
      key: string;
      label?: string;
      text: string;
      danger?: boolean;
    };

export interface ActionDialogConfig {
  title: string;
  description?: string;
  confirmLabel?: string;
  danger?: boolean;
  fields: DialogField[];
}

export type DialogValues = Record<string, string | boolean>;

export type CanvasDragSource =
  | { type: "branch"; branch: BranchInfo }
  | { type: "commit"; oid: string; shortId: string; summary: string };

export type ChatRole = "system" | "user" | "assistant";

export interface ChatMessage {
  role: ChatRole;
  content: string;
}

export type AiEvent =
  | { type: "delta"; text: string }
  | { type: "done"; cancelled: boolean };

export type WorkingStage = "staged" | "unstaged" | "untracked" | "conflicted";
export type WorkingStatus =
  | "added"
  | "deleted"
  | "renamed"
  | "typechange"
  | "modified"
  | "conflicted";
export type DiffLineKind = "add" | "del" | "meta" | "context";

export interface WorkingFile {
  path: string;
  oldPath: string | null;
  status: WorkingStatus;
  stage: WorkingStage;
  additions: number;
  deletions: number;
  hunkable: boolean;
}

export interface WorkingTarget {
  path: string;
  oldPath: string | null;
  status: WorkingStatus;
  stage: WorkingStage;
}

export interface WorkingFiles {
  files: WorkingFile[];
  truncated: boolean;
}

export interface DiffLine {
  kind: DiffLineKind;
  text: string;
}

export interface Hunk {
  index: number;
  header: string;
  additions: number;
  deletions: number;
  lines: DiffLine[];
}

export interface FileDiff {
  path: string;
  stage: WorkingStage;
  hunkable: boolean;
  binary: boolean;
  truncated: boolean;
  additions: number;
  deletions: number;
  hunks: Hunk[];
}

export interface CommitFileDiff {
  path: string;
  oldPath: string | null;
  status: WorkingStatus;
  additions: number;
  deletions: number;
  binary: boolean;
  truncated: boolean;
  hunks: Hunk[];
}

export interface CommitDiff {
  oid: string;
  files: CommitFileDiff[];
  additions: number;
  deletions: number;
  truncated: boolean;
}

export interface StashEntry {
  reference: string;
  message: string;
  date: string;
}

export interface CommitOutcome {
  message: string;
  output: string;
  oid: string | null;
  amended: boolean;
}
