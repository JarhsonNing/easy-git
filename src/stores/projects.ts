import { ref } from "vue";
import { errorText, getSettings, inspectRepository, saveSettings } from "../lib/api";
import type { ProjectEntry } from "../types";
import { closeRepo, session } from "./repo";

export const projects = ref<ProjectEntry[]>([]);
export const projectsLoading = ref(false);
export const projectsError = ref("");

function normalizePath(value: string): string {
  return value.replace(/\\/g, "/").replace(/\/+$/, "");
}

export function sameProjectPath(a: string, b: string): boolean {
  return normalizePath(a) === normalizePath(b);
}

export async function loadProjects(): Promise<void> {
  projectsLoading.value = true;
  projectsError.value = "";
  try {
    const config = await getSettings();
    projects.value = config.projects ?? [];
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
