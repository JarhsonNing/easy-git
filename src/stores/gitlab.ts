import { ref } from "vue";
import {
  cloneGitLabProject,
  createGitLabPipeline,
  errorText,
  getGitLabProject,
  getJobTrace,
  getSettings,
  listGitLabBranches,
  listPipelineJobs,
  listPipelines,
  listRepositoryTree,
  readGitLabFile,
  syncGitLabFork,
} from "../lib/api";
import type {
  GitLabBranch,
  GitLabFileContent,
  GitLabJob,
  GitLabJobTrace,
  GitLabPipeline,
  GitLabProject,
  GitLabTreeEntry,
} from "../types";
import {
  activeInstanceId,
  instances,
  prepare as prepareMr,
  project as mrProject,
  projectError,
} from "./mr";
import { session } from "./repo";

export type GitLabTab = "code" | "pipelines";

export const projects = ref<GitLabProject[]>([]);
export const projectsLoading = ref(false);
export const projectsError = ref("");

export const selectedPath = ref<string | null>(null);
export const summary = ref<GitLabProject | null>(null);
export const summaryLoading = ref(false);
export const summaryError = ref("");

export const tab = ref<GitLabTab>("code");

export const branches = ref<GitLabBranch[]>([]);
export const branchesLoading = ref(false);
export const selectedRef = ref("");

export const tree = ref<Record<string, GitLabTreeEntry[]>>({});
export const expandedDirs = ref<Set<string>>(new Set());
export const treeLoading = ref(false);
export const treeError = ref("");
export const filePath = ref<string | null>(null);
export const fileContent = ref<GitLabFileContent | null>(null);
export const fileLoading = ref(false);
export const fileError = ref("");

export const pipelines = ref<GitLabPipeline[]>([]);
export const pipelinesLoading = ref(false);
export const pipelinesLoadingMore = ref(false);
export const pipelinesError = ref("");
export const pipelinesPage = ref(1);
export const pipelinesHasMore = ref(false);

export const selectedPipelineId = ref<number | null>(null);
export const jobs = ref<GitLabJob[]>([]);
export const jobsLoading = ref(false);
export const jobsError = ref("");
export const selectedJobId = ref<number | null>(null);
export const trace = ref<GitLabJobTrace | null>(null);
export const traceLoading = ref(false);
export const traceError = ref("");

export const syncBusy = ref(false);
export const syncError = ref("");
export const syncNotice = ref("");
export const cloneBusy = ref(false);
export const cloneError = ref("");
export const runBusy = ref(false);
export const runError = ref("");
export const runNotice = ref("");

let projectsToken = 0;
let treeToken = 0;
let fileToken = 0;
let pipelinesToken = 0;
let jobsToken = 0;
let traceToken = 0;

export const repoProjectPath = () => mrProject.value?.projectPath ?? null;

export function resetCodeState() {
  tree.value = {};
  expandedDirs.value = new Set();
  treeError.value = "";
  filePath.value = null;
  fileContent.value = null;
  fileError.value = "";
  treeToken += 1;
  fileToken += 1;
}

function resetPipelinesState() {
  pipelines.value = [];
  pipelinesPage.value = 1;
  pipelinesHasMore.value = false;
  pipelinesError.value = "";
  selectedPipelineId.value = null;
  jobs.value = [];
  jobsError.value = "";
  selectedJobId.value = null;
  trace.value = null;
  runError.value = "";
  runNotice.value = "";
  pipelinesToken += 1;
  jobsToken += 1;
  traceToken += 1;
}

async function loadScope(): Promise<void> {
  const token = ++projectsToken;
  projects.value = [];
  projectsError.value = "";
  projectsLoading.value = true;
  try {
    await prepareMr();
    if (token !== projectsToken) return;
    const instanceId = activeInstanceId.value;
    const currentPath = mrProject.value?.projectPath ?? null;
    if (!instanceId || !currentPath) {
      projectsError.value =
        projectError.value || "当前仓库没有对应的 GitLab 项目，换一个实例或到首页从 GitLab 克隆。";
      return;
    }

    const current = await getGitLabProject(instanceId, currentPath);
    if (token !== projectsToken) return;
    const rows = [current];
    if (current.forkedFromPath && current.forkedFromPath !== current.path) {
      try {
        rows.push(await getGitLabProject(instanceId, current.forkedFromPath));
      } catch {
        // 上游不在这台实例上，或令牌看不到它：此时只看自己的仓库
      }
    }
    if (token !== projectsToken) return;
    projects.value = rows;
    if (!rows.some((row) => row.path === selectedPath.value)) {
      await selectProject(current.path, current);
    }
  } catch (error) {
    if (token !== projectsToken) return;
    projectsError.value = errorText(error);
  } finally {
    if (token === projectsToken) projectsLoading.value = false;
  }
}

export async function prepare(): Promise<void> {
  projectsError.value = "";
  if (!session.value) {
    projects.value = [];
    selectedPath.value = null;
    summary.value = null;
    return;
  }
  try {
    if (!instances.value.length) {
      const config = await getSettings();
      instances.value = config.gitlab.instances;
    }
    if (!instances.value.length) {
      projectsError.value = "尚未配置 GitLab 实例，请先在设置中添加站点地址与访问令牌。";
      return;
    }
    if (!activeInstanceId.value) activeInstanceId.value = instances.value[0].id;

    await loadScope();
  } catch (error) {
    projectsError.value = errorText(error);
  }
}

export async function selectInstance(instanceId: string): Promise<void> {
  if (activeInstanceId.value === instanceId) return;
  activeInstanceId.value = instanceId;
  selectedPath.value = null;
  summary.value = null;
  resetCodeState();
  resetPipelinesState();
  await loadScope();
}

export async function selectProject(path: string, preset?: GitLabProject): Promise<void> {
  if (selectedPath.value === path) return;
  const instanceId = activeInstanceId.value;
  if (!instanceId) return;
  selectedPath.value = path;
  summary.value = null;
  summaryError.value = "";
  branches.value = [];
  selectedRef.value = "";
  resetCodeState();
  resetPipelinesState();

  if (preset) {
    summary.value = preset;
  } else {
    summaryLoading.value = true;
    try {
      summary.value = await getGitLabProject(instanceId, path);
    } catch (error) {
      summaryError.value = errorText(error);
    } finally {
      summaryLoading.value = false;
    }
  }

  await Promise.all([loadBranchList(), loadPipelinesList(true)]);
  if (selectedRef.value) await loadDir("");
}

async function loadBranchList(): Promise<void> {
  const instanceId = activeInstanceId.value;
  const path = selectedPath.value;
  if (!instanceId || !path) return;
  branchesLoading.value = true;
  try {
    branches.value = await listGitLabBranches(instanceId, path);
    const fallback =
      branches.value.find((item) => item.isDefault)?.name ?? branches.value[0]?.name ?? "";
    selectedRef.value = fallback;
  } catch (error) {
    selectedRef.value = "";
    treeError.value = errorText(error);
  } finally {
    branchesLoading.value = false;
  }
}

export async function setRef(branch: string): Promise<void> {
  if (!branch || selectedRef.value === branch) return;
  selectedRef.value = branch;
  resetCodeState();
  await loadDir("");
}

export async function loadDir(path: string): Promise<void> {
  const instanceId = activeInstanceId.value;
  const projectPath = selectedPath.value;
  if (!instanceId || !projectPath || !selectedRef.value) return;
  const token = ++treeToken;
  if (!path) treeLoading.value = true;
  treeError.value = "";
  try {
    const page = await listRepositoryTree(instanceId, projectPath, selectedRef.value, path || null, 1);
    if (token !== treeToken) return;
    tree.value = { ...tree.value, [path]: page.entries };
  } catch (error) {
    if (token !== treeToken) return;
    treeError.value = errorText(error);
  } finally {
    if (token === treeToken && !path) treeLoading.value = false;
  }
}

export async function toggleDir(path: string): Promise<void> {
  const next = new Set(expandedDirs.value);
  if (next.has(path)) {
    next.delete(path);
  } else {
    next.add(path);
    if (!tree.value[path]) await loadDir(path);
  }
  expandedDirs.value = next;
}

export async function openFile(path: string): Promise<void> {
  const instanceId = activeInstanceId.value;
  const projectPath = selectedPath.value;
  if (!instanceId || !projectPath) return;
  if (filePath.value === path) return;
  filePath.value = path;
  fileContent.value = null;
  fileError.value = "";
  fileLoading.value = true;
  const token = ++fileToken;
  try {
    const result = await readGitLabFile(instanceId, projectPath, path, selectedRef.value || null);
    if (token !== fileToken) return;
    fileContent.value = result;
  } catch (error) {
    if (token !== fileToken) return;
    fileError.value = errorText(error);
  } finally {
    if (token === fileToken) fileLoading.value = false;
  }
}

async function loadPipelinesList(reset: boolean): Promise<void> {
  const instanceId = activeInstanceId.value;
  const projectPath = selectedPath.value;
  if (!instanceId || !projectPath) return;
  if (reset) {
    if (pipelinesLoading.value) return;
    pipelinesLoading.value = true;
    pipelinesPage.value = 1;
  } else {
    if (!pipelinesHasMore.value || pipelinesLoadingMore.value) return;
    pipelinesLoadingMore.value = true;
  }
  pipelinesError.value = "";
  const token = ++pipelinesToken;
  const target = reset ? 1 : pipelinesPage.value + 1;

  try {
    const result = await listPipelines(instanceId, projectPath, target);
    if (token !== pipelinesToken) return;
    if (reset) {
      pipelines.value = result.items;
    } else {
      const known = new Set(pipelines.value.map((item) => item.id));
      pipelines.value = pipelines.value.concat(
        result.items.filter((item) => !known.has(item.id)),
      );
    }
    pipelinesPage.value = result.page;
    pipelinesHasMore.value = result.hasMore;
  } catch (error) {
    if (token !== pipelinesToken) return;
    pipelinesError.value = errorText(error);
  } finally {
    if (token === pipelinesToken) {
      pipelinesLoading.value = false;
      pipelinesLoadingMore.value = false;
    }
  }
}

export async function loadPipelines(reset = true): Promise<void> {
  await loadPipelinesList(reset);
}

export async function selectPipeline(id: number, force = false): Promise<void> {
  const instanceId = activeInstanceId.value;
  const projectPath = selectedPath.value;
  if (!instanceId || !projectPath) return;
  if (selectedPipelineId.value === id && !force) return;
  selectedPipelineId.value = id;
  jobs.value = [];
  jobsError.value = "";
  selectedJobId.value = null;
  trace.value = null;
  jobsLoading.value = true;
  const token = ++jobsToken;
  try {
    const result = await listPipelineJobs(instanceId, projectPath, id);
    if (token !== jobsToken) return;
    jobs.value = result;
    const first = result.find((job) => job.status === "failed") ?? result[0];
    if (first) await selectJob(first.id);
  } catch (error) {
    if (token !== jobsToken) return;
    jobsError.value = errorText(error);
  } finally {
    if (token === jobsToken) jobsLoading.value = false;
  }
}

export async function selectJob(id: number): Promise<void> {
  const instanceId = activeInstanceId.value;
  const projectPath = selectedPath.value;
  if (!instanceId || !projectPath) return;
  selectedJobId.value = id;
  trace.value = null;
  traceError.value = "";
  traceLoading.value = true;
  const token = ++traceToken;
  try {
    const result = await getJobTrace(instanceId, projectPath, id);
    if (token !== traceToken) return;
    trace.value = result;
  } catch (error) {
    if (token !== traceToken) return;
    traceError.value = errorText(error);
  } finally {
    if (token === traceToken) traceLoading.value = false;
  }
}

export async function refreshTrace(): Promise<void> {
  if (!selectedJobId.value) return;
  await selectJob(selectedJobId.value);
}

export async function refreshJobs(): Promise<void> {
  if (!selectedPipelineId.value) return;
  await selectPipeline(selectedPipelineId.value, true);
}

export async function createPipeline(branch: string): Promise<GitLabPipeline | null> {
  const instanceId = activeInstanceId.value;
  const projectPath = selectedPath.value;
  if (!instanceId || !projectPath || runBusy.value) return null;
  const ref = branch.trim();
  if (!ref) {
    runError.value = "请选择要运行流水线的分支";
    return null;
  }
  runBusy.value = true;
  runError.value = "";
  runNotice.value = "";
  try {
    const created = await createGitLabPipeline(instanceId, projectPath, ref);
    // 先本地插入，保证新建的这条立刻可见，即使这次刷新撞上还在途的列表请求
    if (!pipelines.value.some((row) => row.id === created.id)) {
      pipelines.value = [created, ...pipelines.value];
    }
    await loadPipelinesList(true);
    await selectPipeline(created.id);
    runNotice.value = `已在 ${ref} 上创建流水线 #${created.iid}`;
    return created;
  } catch (error) {
    runError.value = errorText(error);
    return null;
  } finally {
    runBusy.value = false;
  }
}

export async function syncFork(targetBranch: string): Promise<boolean> {
  const instanceId = activeInstanceId.value;
  const projectPath = selectedPath.value;
  if (!instanceId || !projectPath || syncBusy.value) return false;
  syncBusy.value = true;
  syncError.value = "";
  syncNotice.value = "";
  try {
    syncNotice.value = await syncGitLabFork(instanceId, projectPath, targetBranch);
    await Promise.all([refreshSummary(), loadPipelinesList(true)]);
    return true;
  } catch (error) {
    syncError.value = errorText(error);
    return false;
  } finally {
    syncBusy.value = false;
  }
}

export async function refreshSummary(): Promise<void> {
  const instanceId = activeInstanceId.value;
  const projectPath = selectedPath.value;
  if (!instanceId || !projectPath) return;
  try {
    summary.value = await getGitLabProject(instanceId, projectPath);
  } catch {
    // 保留上一次的摘要
  }
}

export async function cloneTo(parentDir: string): Promise<{ path: string; name: string } | null> {
  const instanceId = activeInstanceId.value;
  const projectPath = selectedPath.value;
  if (!instanceId || !projectPath || cloneBusy.value) return null;
  cloneBusy.value = true;
  cloneError.value = "";
  try {
    return await cloneGitLabProject(instanceId, projectPath, parentDir);
  } catch (error) {
    cloneError.value = errorText(error);
    return null;
  } finally {
    cloneBusy.value = false;
  }
}
