import { ref } from "vue";
import {
  cloneGitLabProject,
  errorText,
  getSettings,
  inspectRepository,
  listGitLabProjects,
  listLocalProjectMatches,
  saveSettings,
} from "../lib/api";
import type {
  CloneResult,
  GitLabInstance,
  GitLabProject,
  LocalProjectMatch,
  ProjectEntry,
} from "../types";
import { closeRepo, session } from "./repo";

export const projects = ref<ProjectEntry[]>([]);
export const projectsLoading = ref(false);
export const projectsError = ref("");

export const gitlabInstances = ref<GitLabInstance[]>([]);
export const gitlabActiveId = ref<string | null>(null);
export const gitlabProjects = ref<GitLabProject[]>([]);
export const gitlabSearch = ref("");
export const gitlabLoading = ref(false);
export const gitlabLoadingMore = ref(false);
export const gitlabHasMore = ref(false);
export const gitlabPage = ref(1);
export const gitlabError = ref("");
export const gitlabCloning = ref("");
export const localMatches = ref<Record<string, LocalProjectMatch>>({});

let listToken = 0;
let matchToken = 0;

function normalizePath(value: string): string {
  return value.replace(/\\/g, "/").replace(/\/+$/, "");
}

export function sameProjectPath(a: string, b: string): boolean {
  return normalizePath(a) === normalizePath(b);
}

export function localMatchFor(projectPath: string): LocalProjectMatch | null {
  return localMatches.value[projectPath.toLowerCase()] ?? null;
}

export async function loadProjects(): Promise<void> {
  projectsLoading.value = true;
  projectsError.value = "";
  try {
    const config = await getSettings();
    projects.value = config.projects ?? [];
    gitlabInstances.value = config.gitlab.instances ?? [];
    if (!gitlabInstances.value.some((item) => item.id === gitlabActiveId.value)) {
      gitlabActiveId.value = gitlabInstances.value[0]?.id ?? null;
    }
  } catch (error) {
    projectsError.value = errorText(error);
  } finally {
    projectsLoading.value = false;
  }
}

export async function addProject(path: string): Promise<ProjectEntry> {
  const summary = await inspectRepository(path);
  const entry: ProjectEntry = { path: summary.path, name: summary.name };
  const config = await getSettings();
  const list = config.projects ?? [];
  const exists = list.some((item) => sameProjectPath(item.path, summary.path));
  const next = exists
    ? list.map((item) => (sameProjectPath(item.path, summary.path) ? entry : item))
    : [entry, ...list];
  config.projects = next;
  await saveSettings(config);
  projects.value = next;
  return entry;
}

export async function removeProject(path: string): Promise<void> {
  const config = await getSettings();
  const next = (config.projects ?? []).filter((item) => !sameProjectPath(item.path, path));
  config.projects = next;
  await saveSettings(config);
  projects.value = next;
  if (session.value && sameProjectPath(session.value.path, path)) {
    closeRepo();
  }
}

export async function loadGitlabProjects(reset = true): Promise<void> {
  const instanceId = gitlabActiveId.value;
  if (!instanceId) {
    gitlabProjects.value = [];
    gitlabHasMore.value = false;
    return;
  }
  if (!reset && (!gitlabHasMore.value || gitlabLoading.value)) return;
  gitlabError.value = "";
  const token = ++listToken;
  const target = reset ? 1 : gitlabPage.value + 1;
  gitlabLoading.value = true;
  if (!reset) gitlabLoadingMore.value = true;

  try {
    const result = await listGitLabProjects(instanceId, gitlabSearch.value.trim() || null, target);
    if (reset) {
      gitlabProjects.value = result.items;
    } else {
      const known = new Set(gitlabProjects.value.map((item) => item.path));
      gitlabProjects.value = gitlabProjects.value.concat(
        result.items.filter((item) => !known.has(item.path)),
      );
    }
    gitlabPage.value = result.page;
    gitlabHasMore.value = result.hasMore;
  } catch (error) {
    if (token === listToken) gitlabError.value = errorText(error);
  } finally {
    if (token === listToken) {
      gitlabLoading.value = false;
      gitlabLoadingMore.value = false;
    }
  }
}

export async function loadLocalMatches(): Promise<void> {
  const instanceId = gitlabActiveId.value;
  if (!instanceId) {
    localMatches.value = {};
    return;
  }
  const token = ++matchToken;
  try {
    const items = await listLocalProjectMatches(instanceId);
    const next: Record<string, LocalProjectMatch> = {};
    for (const item of items) next[item.projectPath.toLowerCase()] = item;
    if (token === matchToken) localMatches.value = next;
  } catch {
    if (token === matchToken) localMatches.value = {};
  }
}

export async function selectGitlabInstance(instanceId: string): Promise<void> {
  if (gitlabActiveId.value === instanceId) return;
  gitlabActiveId.value = instanceId;
  gitlabProjects.value = [];
  gitlabPage.value = 1;
  gitlabHasMore.value = false;
  await Promise.all([loadGitlabProjects(true), loadLocalMatches()]);
}

export async function refreshGitlab(): Promise<void> {
  await loadProjects();
  await Promise.all([loadGitlabProjects(true), loadLocalMatches()]);
}

export async function cloneToProjectList(
  projectPath: string,
  parentDir: string,
): Promise<CloneResult | null> {
  const instanceId = gitlabActiveId.value;
  if (!instanceId || gitlabCloning.value) return null;
  gitlabCloning.value = projectPath;
  try {
    const result = await cloneGitLabProject(instanceId, projectPath, parentDir);
    try {
      await addProject(result.path);
    } catch (error) {
      throw new Error(`已克隆到 ${result.path}，但加入项目列表失败：${errorText(error)}`);
    }
    await loadLocalMatches();
    return result;
  } finally {
    gitlabCloning.value = "";
  }
}
