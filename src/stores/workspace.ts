import { computed, ref } from "vue";
import {
  applyHunk,
  cancelAi,
  chatStream,
  commitChanges,
  commitIdentity,
  discardFiles,
  errorText,
  getFileDiff,
  getCommit,
  getRepoStatus,
  getWorkingDiff,
  getWorkingFiles,
  listStashes,
  stageAll as stageAllGit,
  stageFiles,
  stashApply,
  stashDrop,
  stashPush,
  unstageAll as unstageAllGit,
  unstageFiles,
} from "../lib/api";
import {
  operation,
  refreshRepo,
  runOperation,
  session,
  setNotice,
  status,
} from "./repo";
import type {
  ChatMessage,
  FileDiff,
  OperationResult,
  StashEntry,
  WorkingFile,
  WorkingStage,
  WorkingTarget,
} from "../types";

export const files = ref<WorkingFile[]>([]);
export const filesTruncated = ref(false);
export const stashes = ref<StashEntry[]>([]);
export const selected = ref<{ path: string; stage: WorkingStage } | null>(null);
export const fileDiff = ref<FileDiff | null>(null);
export const diffLoading = ref(false);
export const workspaceLoading = ref(false);
export const workspaceError = ref("");
export const summary = ref("");
export const description = ref("");
export const amend = ref(false);
export const drafting = ref(false);
export const identity = ref<string | null>(null);

let diffRequest = 0;

const DRAFT_SYSTEM = `你是 Easy Git 桌面应用内置的代码助手，正在为一次本地提交撰写提交信息。
使用简体中文。标题遵循 Conventional Commits 格式且不超过 72 个字符，正文用简短要点说明改动动机与影响。
只输出提交信息本身，不要解释，不要代码块，不要markdown 标记。`;

export const conflictedFiles = computed(() =>
  files.value.filter((file) => file.stage === "conflicted"),
);
export const stagedFiles = computed(() =>
  files.value.filter((file) => file.stage === "staged"),
);
export const unstagedFiles = computed(() =>
  files.value.filter((file) => file.stage === "unstaged"),
);
export const untrackedFiles = computed(() =>
  files.value.filter((file) => file.stage === "untracked"),
);
export const changeCount = computed(() => files.value.length);
export const stagedCount = computed(() => stagedFiles.value.length);

export const stagedSummary = computed(() => {
  let additions = 0;
  let deletions = 0;
  for (const file of stagedFiles.value) {
    additions += file.additions;
    deletions += file.deletions;
  }
  return { additions, deletions };
});

export function targetsOf(selection: WorkingFile[]): WorkingTarget[] {
  return selection.map((file) => ({
    path: file.path,
    oldPath: file.oldPath,
    status: file.status,
    stage: file.stage,
  }));
}

async function loadDiff(path: string | null, stage: WorkingStage | null): Promise<void> {
  const request = ++diffRequest;
  if (!path || !stage || stage === "conflicted") {
    fileDiff.value = null;
    diffLoading.value = false;
    return;
  }
  diffLoading.value = true;
  try {
    const result = await getFileDiff(path, stage);
    if (request === diffRequest) fileDiff.value = result;
  } catch (e) {
    if (request === diffRequest) {
      fileDiff.value = null;
      workspaceError.value = errorText(e);
    }
  } finally {
    if (request === diffRequest) diffLoading.value = false;
  }
}

async function reconcileSelection(): Promise<void> {
  const current = selected.value;
  if (!current) {
    if (fileDiff.value) await loadDiff(null, null);
    return;
  }
  const exact = files.value.find(
    (file) => file.path === current.path && file.stage === current.stage,
  );
  const fallback = exact ?? files.value.find((file) => file.path === current.path);
  if (!fallback) {
    selected.value = null;
    await loadDiff(null, null);
    return;
  }
  selected.value = { path: fallback.path, stage: fallback.stage };
  await loadDiff(fallback.path, fallback.stage);
}

export async function refreshWorkspace(): Promise<void> {
  if (!session.value) return;
  workspaceLoading.value = true;
  workspaceError.value = "";
  try {
    const [nextStatus, working, stashList] = await Promise.all([
      getRepoStatus(),
      getWorkingFiles(),
      listStashes(),
    ]);
    status.value = nextStatus;
    files.value = working.files;
    filesTruncated.value = working.truncated;
    stashes.value = stashList;
    await reconcileSelection();
  } catch (e) {
    workspaceError.value = errorText(e);
  } finally {
    workspaceLoading.value = false;
  }
}

async function loadIdentity(): Promise<void> {
  if (!session.value) return;
  try {
    identity.value = await commitIdentity();
  } catch {
    identity.value = null;
  }
}

export async function enterWorkspace(): Promise<void> {
  await Promise.all([refreshWorkspace(), loadIdentity()]);
}

export function resetWorkspace() {
  files.value = [];
  filesTruncated.value = false;
  stashes.value = [];
  selected.value = null;
  fileDiff.value = null;
  diffLoading.value = false;
  workspaceLoading.value = false;
  workspaceError.value = "";
  summary.value = "";
  description.value = "";
  amend.value = false;
  drafting.value = false;
  identity.value = null;
  diffRequest += 1;
}

export async function selectFile(file: WorkingFile): Promise<void> {
  selected.value = { path: file.path, stage: file.stage };
  await loadDiff(file.path, file.stage);
}

export async function reuseCommitMessage(oid: string): Promise<boolean> {
  try {
    const detail = await getCommit(oid);
    const lines = detail.message.replace(/\r\n/g, "\n").split("\n");
    const title = (lines.shift() ?? "").trim() || detail.summary;
    summary.value = title.slice(0, 72);
    description.value = lines.join("\n").trim();
    return true;
  } catch (e) {
    setNotice({ tone: "error", message: errorText(e) });
    return false;
  }
}

function runWorkspace(
  label: string,
  action: () => Promise<OperationResult>,
): Promise<boolean> {
  return runOperation(label, action, refreshWorkspace);
}

export function stageFile(file: WorkingFile): Promise<boolean> {
  const label =
    file.stage === "conflicted" ? "正在标记为已解决…" : `正在暂存 ${file.path}…`;
  return runWorkspace(label, () => stageFiles(targetsOf([file])));
}

export function unstageFile(file: WorkingFile): Promise<boolean> {
  return runWorkspace(`正在取消暂存 ${file.path}…`, () => unstageFiles(targetsOf([file])));
}

export function toggleFile(file: WorkingFile): Promise<boolean> {
  return file.stage === "staged" ? unstageFile(file) : stageFile(file);
}

export function stageMany(selection: WorkingFile[]): Promise<boolean> {
  if (!selection.length) return Promise.resolve(false);
  return runWorkspace(`正在暂存 ${selection.length} 个文件…`, () =>
    stageFiles(targetsOf(selection)),
  );
}

export function unstageMany(selection: WorkingFile[]): Promise<boolean> {
  if (!selection.length) return Promise.resolve(false);
  return runWorkspace(`正在取消暂存 ${selection.length} 个文件…`, () =>
    unstageFiles(targetsOf(selection)),
  );
}

export function discardMany(selection: WorkingFile[]): Promise<boolean> {
  if (!selection.length) return Promise.resolve(false);
  return runWorkspace(`正在丢弃 ${selection.length} 个文件的更改…`, () =>
    discardFiles(targetsOf(selection)),
  );
}

export function stageEverything(): Promise<boolean> {
  return runWorkspace("正在暂存全部变更…", () => stageAllGit());
}

export function unstageEverything(): Promise<boolean> {
  return runWorkspace("正在取消全部暂存…", () => unstageAllGit());
}

export function applyHunkByIndex(hunkIndex: number): Promise<boolean> {
  const current = selected.value;
  if (!current || current.stage === "untracked" || current.stage === "conflicted") {
    return Promise.resolve(false);
  }
  const label = current.stage === "staged" ? "正在取消暂存该代码块…" : "正在暂存该代码块…";
  return runWorkspace(label, () => applyHunk(current.path, current.stage, hunkIndex));
}

export function discardFile(file: WorkingFile): Promise<boolean> {
  return runWorkspace(`正在丢弃 ${file.path} 的更改…`, () => discardFiles(targetsOf([file])));
}

export function stashEverything(message: string, includeUntracked: boolean): Promise<boolean> {
  return runWorkspace("正在贮藏变更…", () => stashPush(message, includeUntracked));
}

export function applyStash(entry: StashEntry, drop: boolean): Promise<boolean> {
  return runWorkspace(drop ? "正在弹出贮藏…" : "正在应用贮藏…", () =>
    stashApply(entry.reference, drop),
  );
}

export function removeStash(entry: StashEntry): Promise<boolean> {
  return runWorkspace("正在删除贮藏…", () => stashDrop(entry.reference));
}

function splitDraft(text: string): { title: string; body: string } {
  const cleaned = text
    .replace(/^\s*```[a-zA-Z]*\s*/, "")
    .replace(/\s*```\s*$/, "")
    .trim();
  const lines = cleaned
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
  const title = (lines.shift() ?? "").slice(0, 72);
  const body = lines.join("\n").trim();
  return { title, body };
}

export async function writeDraft(): Promise<boolean> {
  if (drafting.value || operation.value) return false;
  if (!stagedCount.value) {
    setNotice({ tone: "error", message: "请先暂存要提交的变更，再让 AI 撰写提交信息。" });
    return false;
  }
  drafting.value = true;
  workspaceError.value = "";
  try {
    const working = await getWorkingDiff(true);
    const context = working.truncated
      ? `${working.patch}\n\n（差异过长已被截断）`
      : working.patch;
    const messages: ChatMessage[] = [
      { role: "system", content: DRAFT_SYSTEM },
      { role: "user", content: `请根据以下已暂存的差异撰写提交信息：\n\n${context}` },
    ];
    let text = "";
    let cancelled = false;
    await chatStream(null, messages, (event) => {
      if (event.type === "delta") text += event.text;
      else if (event.cancelled) cancelled = true;
    });
    if (cancelled) return false;
    const draft = splitDraft(text);
    if (!draft.title) {
      setNotice({ tone: "error", message: "模型没有返回可用的提交信息。" });
      return false;
    }
    summary.value = draft.title;
    description.value = draft.body;
    return true;
  } catch (e) {
    setNotice({ tone: "error", message: errorText(e) });
    return false;
  } finally {
    drafting.value = false;
  }
}

export function stopDraft(): void {
  if (!drafting.value) return;
  void cancelAi();
}

export async function commitStaged(): Promise<boolean> {
  if (operation.value) return false;
  const title = summary.value.trim();
  if (!title) {
    setNotice({ tone: "error", message: "请先填写提交摘要。" });
    return false;
  }
  operation.value = amend.value ? "正在改写最后一次提交…" : "正在提交…";
  workspaceError.value = "";
  try {
    const outcome = await commitChanges(title, description.value, amend.value);
    setNotice({ tone: "success", message: outcome.message, detail: outcome.output });
    summary.value = "";
    description.value = "";
    amend.value = false;
    await refreshWorkspace();
    await refreshRepo();
    return true;
  } catch (e) {
    setNotice({ tone: "error", message: errorText(e) });
    await refreshWorkspace();
    return false;
  } finally {
    operation.value = null;
  }
}

export async function stageAllAndCommit(): Promise<boolean> {
  const staged = await stageEverything();
  if (!staged) return false;
  return commitStaged();
}
