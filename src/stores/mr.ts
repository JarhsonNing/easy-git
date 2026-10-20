import { ref } from "vue";
import {
  createMergeRequest,
  errorText,
  getMergeRequest,
  getRepoRemote,
  getSettings,
  listGitLabBranches,
  listMergeRequestChanges,
  listMergeRequestNotes,
  listMergeRequests,
  mergeMergeRequest,
  resolveGitLabProject,
  setMergeRequestState,
} from "../lib/api";
import type {
  GitLabBranch,
  GitLabInstance,
  GitLabProjectRef,
  MergeRequest,
  MergeRequestChanges,
  MergeRequestFilter,
  MergeRequestNote,
  RepoRemoteInfo,
} from "../types";
import { session } from "./repo";

export const instances = ref<GitLabInstance[]>([]);
export const activeInstanceId = ref<string | null>(null);
export const remoteInfo = ref<RepoRemoteInfo | null>(null);
export const project = ref<GitLabProjectRef | null>(null);
export const projectLoading = ref(false);
export const projectError = ref("");

export const filter = ref<MergeRequestFilter>("opened");
export const search = ref("");
export const items = ref<MergeRequest[]>([]);
export const page = ref(1);
export const hasMore = ref(false);
export const listLoading = ref(false);
export const listLoadingMore = ref(false);
export const listError = ref("");

export const selectedIid = ref<number | null>(null);
export const detail = ref<MergeRequest | null>(null);
export const changes = ref<MergeRequestChanges | null>(null);
export const notes = ref<MergeRequestNote[]>([]);
export const detailLoading = ref(false);
export const detailError = ref("");

export const actionBusy = ref<"merge" | "close" | "reopen" | "create" | "">("");
export const actionError = ref("");

export const branches = ref<GitLabBranch[]>([]);
export const branchesLoading = ref(false);
export const branchesError = ref("");

let prepareToken = 0;
let listToken = 0;
let detailToken = 0;
let lastRepoPath: string | null = null;

export const activeInstance = () => instances.value.find((item) => item.id === activeInstanceId.value) ?? null;

export const canBrowse = () => Boolean(session.value && project.value && activeInstanceId.value);

function resetList() {
  items.value = [];
  page.value = 1;
  hasMore.value = false;
  listError.value = "";
  listToken += 1;
}

function resetDetail() {
  selectedIid.value = null;
  detail.value = null;
  changes.value = null;
  notes.value = [];
  detailError.value = "";
  detailToken += 1;
}

function resetProject() {
  project.value = null;
  remoteInfo.value = null;
  resetList();
  resetDetail();
}

function hostOf(baseUrl: string): string | null {
  try {
    return new URL(baseUrl.trim()).hostname.toLowerCase();
  } catch {
    return null;
  }
}

function matchInstance(host: string | null): GitLabInstance | null {
  if (!host) return null;
  return instances.value.find((instance) => hostOf(instance.baseUrl) === host) ?? null;
}

export async function prepare(): Promise<void> {
  const token = ++prepareToken;
  projectError.value = "";
  projectLoading.value = true;

  if (!session.value) {
    projectLoading.value = false;
    resetProject();
    return;
  }

  try {
    const config = await getSettings();
    if (token !== prepareToken) return;
    instances.value = config.gitlab.instances;
    if (!instances.value.length) {
      resetProject();
      projectError.value = "尚未配置 GitLab 实例，请先在设置中添加站点地址与访问令牌。";
      return;
    }

    remoteInfo.value = await getRepoRemote();
    if (token !== prepareToken) return;
    const remote = remoteInfo.value;
    if (!remote?.remoteUrl) {
      resetProject();
      projectError.value = "当前仓库没有 origin 远端，无法定位 GitLab 项目。";
      return;
    }

    const repoChanged = lastRepoPath !== session.value.path;
    const matched = matchInstance(remote.host);
    let current = instances.value.find((item) => item.id === activeInstanceId.value) ?? null;
    if (!current) current = matched ?? instances.value[0];
    if (repoChanged && matched) current = matched;
    activeInstanceId.value = current.id;
    lastRepoPath = session.value.path;

    project.value = await resolveGitLabProject(current.id);
    if (token !== prepareToken) return;
    if (!project.value) return;
    if (remote.host && project.value.remoteHost !== remote.host) {
      projectError.value = `远端主机 ${remote.host} 与所选实例不匹配，请检查设置中的站点地址。`;
    }
  } catch (error) {
    if (token !== prepareToken) return;
    project.value = null;
    resetList();
    resetDetail();
    projectError.value = errorText(error);
  } finally {
    if (token === prepareToken) projectLoading.value = false;
  }
}

export async function selectInstance(instanceId: string): Promise<void> {
  if (activeInstanceId.value === instanceId && project.value) return;
  activeInstanceId.value = instanceId;
  resetList();
  resetDetail();
  await prepare();
  if (project.value && !projectError.value) await loadList(true);
}

export async function loadList(reset = true): Promise<void> {
  if (!project.value || !activeInstanceId.value) return;
  if (reset) {
    page.value = 1;
    listLoading.value = true;
  } else {
    if (!hasMore.value || listLoadingMore.value) return;
    listLoadingMore.value = true;
  }
  listError.value = "";
  const token = ++listToken;
  const target = reset ? 1 : page.value + 1;

  try {
    const result = await listMergeRequests(
      activeInstanceId.value,
      project.value.projectPath,
      filter.value,
      target,
      search.value.trim() || null,
    );
    if (token !== listToken) return;
    if (reset) {
      items.value = result.items;
    } else {
      const known = new Set(items.value.map((item) => item.iid));
      items.value = items.value.concat(result.items.filter((item) => !known.has(item.iid)));
    }
    page.value = result.page;
    hasMore.value = result.hasMore;
  } catch (error) {
    if (token !== listToken) return;
    listError.value = errorText(error);
  } finally {
    if (token === listToken) {
      listLoading.value = false;
      listLoadingMore.value = false;
    }
  }
}

export async function selectMergeRequest(iid: number): Promise<void> {
  if (!project.value || !activeInstanceId.value) return;
  selectedIid.value = iid;
  const preview = items.value.find((item) => item.iid === iid) ?? null;
  detail.value = preview;
  changes.value = null;
  notes.value = [];
  detailError.value = "";
  detailLoading.value = true;
  const token = ++detailToken;
  const instanceId = activeInstanceId.value;
  const path = project.value.projectPath;

  const [mrResult, changesResult, notesResult] = await Promise.allSettled([
    getMergeRequest(instanceId, path, iid),
    listMergeRequestChanges(instanceId, path, iid),
    listMergeRequestNotes(instanceId, path, iid),
  ]);
  if (token !== detailToken || selectedIid.value !== iid) return;

  const failures: string[] = [];
  if (mrResult.status === "fulfilled") {
    detail.value = mrResult.value;
    syncItem(mrResult.value);
  } else {
    failures.push(errorText(mrResult.reason));
  }
  if (changesResult.status === "fulfilled") {
    changes.value = changesResult.value;
  } else {
    failures.push(errorText(changesResult.reason));
  }
  if (notesResult.status === "fulfilled") {
    notes.value = notesResult.value;
  } else {
    failures.push(errorText(notesResult.reason));
  }
  if (failures.length) detailError.value = failures.join("；");
  detailLoading.value = false;
}

export async function refreshDetail(): Promise<void> {
  if (selectedIid.value) await selectMergeRequest(selectedIid.value);
}

function syncItem(updated: MergeRequest) {
  const index = items.value.findIndex((item) => item.iid === updated.iid);
  if (index >= 0) items.value[index] = updated;
}

export async function mergeCurrent(options: { squash: boolean; removeSourceBranch: boolean }): Promise<boolean> {
  if (!project.value || !activeInstanceId.value || !selectedIid.value || actionBusy.value) return false;
  actionBusy.value = "merge";
  actionError.value = "";
  try {
    const updated = await mergeMergeRequest(
      activeInstanceId.value,
      project.value.projectPath,
      selectedIid.value,
      options.squash,
      options.removeSourceBranch,
    );
    applyAction(updated);
    return true;
  } catch (error) {
    actionError.value = errorText(error);
    return false;
  } finally {
    actionBusy.value = "";
  }
}

export async function toggleCurrentState(): Promise<void> {
  if (!project.value || !activeInstanceId.value || !detail.value || actionBusy.value) return;
  const event = detail.value.state === "closed" ? "reopen" : "close";
  if (event === "close" && detail.value.state !== "opened") return;
  actionBusy.value = event;
  actionError.value = "";
  try {
    const updated = await setMergeRequestState(
      activeInstanceId.value,
      project.value.projectPath,
      detail.value.iid,
      event,
    );
    applyAction(updated);
    if (filter.value !== "all" && updated.state !== filter.value) {
      await loadList(true);
    }
  } catch (error) {
    actionError.value = errorText(error);
  } finally {
    actionBusy.value = "";
  }
}

function applyAction(updated: MergeRequest) {
  detail.value = updated;
  syncItem(updated);
}

export async function loadBranches(force = false): Promise<void> {
  if (!project.value || !activeInstanceId.value) return;
  if (branches.value.length && !force) return;
  branchesLoading.value = true;
  branchesError.value = "";
  try {
    branches.value = await listGitLabBranches(activeInstanceId.value, project.value.projectPath);
  } catch (error) {
    branchesError.value = errorText(error);
  } finally {
    branchesLoading.value = false;
  }
}

export async function createCurrentMR(input: {
  sourceBranch: string;
  targetBranch: string;
  title: string;
  description: string;
  removeSourceBranch: boolean;
}): Promise<MergeRequest | null> {
  if (!project.value || !activeInstanceId.value || actionBusy.value) return null;
  actionBusy.value = "create";
  actionError.value = "";
  try {
    const created = await createMergeRequest(
      activeInstanceId.value,
      project.value.projectPath,
      input.sourceBranch,
      input.targetBranch,
      input.title,
      input.description || null,
      input.removeSourceBranch,
    );
    filter.value = "opened";
    search.value = "";
    await loadList(true);
    if (!items.value.some((item) => item.iid === created.iid)) {
      items.value = [created, ...items.value];
    }
    await selectMergeRequest(created.iid);
    return created;
  } catch (error) {
    actionError.value = errorText(error);
    return null;
  } finally {
    actionBusy.value = "";
  }
}
