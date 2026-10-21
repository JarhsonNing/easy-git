<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import Icon from "../components/Icon.vue";
import DeltaMark from "../components/DeltaMark.vue";
import { errorText, pickRepository } from "../lib/api";
import {
  addProject,
  cloneToProjectList,
  gitlabActiveId,
  gitlabCloning,
  gitlabError,
  gitlabHasMore,
  gitlabInstances,
  gitlabLoading,
  gitlabLoadingMore,
  gitlabProjects,
  gitlabSearch,
  loadGitlabProjects,
  loadLocalMatches,
  loadProjects,
  localMatchFor,
  projects,
  projectsError,
  projectsLoading,
  refreshGitlab,
  removeProject,
  sameProjectPath,
  selectGitlabInstance,
} from "../stores/projects";
import { error as repoError, loading as repoLoading, openRepo, session } from "../stores/repo";
import { navigate } from "../stores/ui";
import type { GitLabProject, ProjectEntry } from "../types";

type Source = "local" | "gitlab";

const source = ref<Source>("local");
const picking = ref(false);
const openingPath = ref("");
const removePath = ref("");
const removing = ref("");
const menuPath = ref("");
const actionError = ref("");
const notice = ref("");
const query = ref("");
const gitlabQuery = ref("");

let searchTimer = 0;

const relativeFormatter = new Intl.DateTimeFormat("zh-CN", {
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
});

const filteredProjects = computed(() => {
  const term = query.value.trim().toLowerCase();
  if (!term) return projects.value;
  return projects.value.filter(
    (project) =>
      project.name.toLowerCase().includes(term) || project.path.toLowerCase().includes(term),
  );
});

const openingProject = computed(
  () => projects.value.find((project) => project.path === openingPath.value) ?? null,
);

const searchTerm = computed({
  get: () => (source.value === "local" ? query.value : gitlabQuery.value),
  set: (value: string) => {
    if (source.value === "local") {
      query.value = value;
    } else {
      gitlabQuery.value = value;
    }
  },
});

const headLede = computed(() =>
  source.value === "local"
    ? "本地仓库都在这里。打开一个，继续今天的改动。"
    : "GitLab 上的项目。选一个克隆下来，就会加入本地列表并直接打开。",
);

const listBusy = computed(() =>
  source.value === "local"
    ? projectsLoading.value && !projects.value.length
    : gitlabLoading.value && !gitlabProjects.value.length,
);

const countText = computed(() => {
  if (source.value === "local") {
    return query.value.trim()
      ? `${filteredProjects.value.length} / ${projects.value.length} 个仓库`
      : `${projects.value.length} 个仓库`;
  }
  return `${gitlabProjects.value.length} 个项目`;
});

const refreshing = computed(() =>
  source.value === "local" ? projectsLoading.value : gitlabLoading.value,
);

onMounted(() => {
  void loadProjects();
  document.addEventListener("click", closeMenu);
});

onBeforeUnmount(() => {
  window.clearTimeout(searchTimer);
  document.removeEventListener("click", closeMenu);
});

watch(gitlabQuery, (value) => {
  window.clearTimeout(searchTimer);
  searchTimer = window.setTimeout(() => {
    const term = value.trim();
    if (gitlabSearch.value === term) return;
    gitlabSearch.value = term;
    void loadGitlabProjects(true);
  }, 350);
});

watch(
  () => projects.value.length,
  () => {
    if (gitlabInstances.value.length) void loadLocalMatches();
  },
);

function closeMenu() {
  menuPath.value = "";
}

function isCurrent(project: ProjectEntry): boolean {
  return Boolean(session.value && sameProjectPath(session.value.path, project.path));
}

function toggleMenu(project: ProjectEntry) {
  menuPath.value = menuPath.value === project.path ? "" : project.path;
  removePath.value = "";
}

async function setSource(next: Source): Promise<void> {
  if (source.value === next) return;
  source.value = next;
  actionError.value = "";
  notice.value = "";
  removePath.value = "";
  closeMenu();
  if (next !== "gitlab") return;
  if (!gitlabInstances.value.length) await loadProjects();
  if (gitlabInstances.value.length && !gitlabProjects.value.length) {
    await Promise.all([loadGitlabProjects(true), loadLocalMatches()]);
  }
}

function onInstanceChange(event: Event) {
  const value = (event.target as HTMLSelectElement).value;
  if (value) void selectGitlabInstance(value);
}

function relative(iso: string): string {
  if (!iso) return "";
  const time = new Date(iso).getTime();
  if (Number.isNaN(time)) return iso;
  const diff = Math.max(0, Math.floor((Date.now() - time) / 1000));
  if (diff < 60) return "刚刚";
  const minutes = Math.floor(diff / 60);
  if (minutes < 60) return `${minutes} 分钟前`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} 小时前`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days} 天前`;
  return `${relativeFormatter.format(new Date(time))} 活跃`;
}

function gitlabMeta(project: GitLabProject): string {
  const visibility =
    project.visibility === "private" ? "私有" : project.visibility === "internal" ? "内部" : "公开";
  const branch = project.defaultBranch || "空仓库";
  const active = project.lastActivityAt ? relative(project.lastActivityAt) : "";
  return [`${visibility} · ${branch}`, active].filter(Boolean).join("，");
}

function gitlabActionLabel(project: GitLabProject): string {
  if (gitlabCloning.value === project.path) return "克隆中…";
  return localMatchFor(project.path) ? "打开" : "克隆并添加";
}

async function refresh(): Promise<void> {
  actionError.value = "";
  notice.value = "";
  if (source.value === "local") {
    await loadProjects();
    return;
  }
  await refreshGitlab();
}

async function addFromPicker() {
  if (picking.value || repoLoading.value) return;
  actionError.value = "";
  notice.value = "";
  removePath.value = "";
  picking.value = true;
  try {
    const path = await pickRepository();
    if (!path) return;
    const entry = await addProject(path);
    notice.value = `已添加「${entry.name}」，随时可以从这里打开。`;
  } catch (error) {
    actionError.value = errorText(error);
  } finally {
    picking.value = false;
  }
}

function wait(milliseconds: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, milliseconds);
  });
}

async function openLocal(path: string): Promise<boolean> {
  if (repoLoading.value || openingPath.value) return false;
  actionError.value = "";
  openingPath.value = path;
  const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const [opened] = await Promise.all([openRepo(path), wait(reduced ? 0 : 480)]);
  if (!opened && repoError.value) actionError.value = repoError.value;
  openingPath.value = "";
  return opened;
}

async function openProject(project: ProjectEntry) {
  if (repoLoading.value || openingPath.value) return;
  notice.value = "";
  removePath.value = "";
  closeMenu();
  if (await openLocal(project.path)) {
    navigate("graph");
  }
}

async function activateGitlabProject(project: GitLabProject) {
  if (repoLoading.value || openingPath.value || gitlabCloning.value) return;
  actionError.value = "";
  notice.value = "";
  closeMenu();

  const match = localMatchFor(project.path);
  if (match) {
    if (await openLocal(match.localPath)) navigate("graph");
    return;
  }

  const parent = await openDialog({
    directory: true,
    multiple: false,
    title: "选择克隆存放目录",
  });
  if (typeof parent !== "string" || !parent) return;

  try {
    const result = await cloneToProjectList(project.path, parent);
    if (!result) return;
    notice.value = `已克隆「${result.name}」并加入项目列表。`;
    if (await openLocal(result.path)) navigate("graph");
  } catch (error) {
    actionError.value = errorText(error);
  }
}

function clearSearch() {
  if (source.value === "local") {
    query.value = "";
    return;
  }
  gitlabQuery.value = "";
}

function askRemove(project: ProjectEntry) {
  if (removing.value) return;
  actionError.value = "";
  notice.value = "";
  closeMenu();
  removePath.value = project.path;
}

async function confirmRemove(project: ProjectEntry) {
  if (removing.value) return;
  removing.value = project.path;
  actionError.value = "";
  try {
    await removeProject(project.path);
    removePath.value = "";
    notice.value = `已从列表移除「${project.name}」，磁盘上的仓库文件没有被删除。`;
  } catch (error) {
    actionError.value = errorText(error);
  } finally {
    removing.value = "";
  }
}
</script>

<template>
  <div class="projects-page">
    <header class="page-head">
      <div class="page-heading">
        <h1>项目</h1>
        <p>{{ headLede }}</p>
      </div>
      <div class="page-actions">
        <button
          type="button"
          class="button ghost"
          :disabled="refreshing || picking"
          @click="refresh"
        >
          <Icon name="refresh" :size="16" />
          {{ refreshing ? "刷新中…" : "刷新" }}
        </button>
        <button
          v-if="source === 'local'"
          type="button"
          class="button primary"
          :disabled="picking || repoLoading"
          @click="addFromPicker"
        >
          <Icon name="plus" :size="17" />
          {{ picking ? "读取仓库…" : "添加项目" }}
        </button>
      </div>
    </header>

    <p v-if="notice" class="alert success" role="status">{{ notice }}</p>
    <p v-if="actionError || projectsError" class="alert error" role="alert">
      {{ actionError || projectsError }}
    </p>

    <div v-if="listBusy" class="index-loading" role="status">
      <span class="loading-orbit" aria-hidden="true"></span>
      <span>{{ source === "local" ? "正在读取本地项目…" : "正在读取 GitLab 项目…" }}</span>
    </div>

    <template v-else>
      <div class="index-bar">
        <div class="segmented" role="tablist" aria-label="项目来源">
          <button
            type="button"
            class="segment"
            :class="{ active: source === 'local' }"
            role="tab"
            :aria-selected="source === 'local'"
            @click="setSource('local')"
          >
            本地
          </button>
          <button
            type="button"
            class="segment"
            :class="{ active: source === 'gitlab' }"
            role="tab"
            :aria-selected="source === 'gitlab'"
            @click="setSource('gitlab')"
          >
            GitLab
          </button>
        </div>
        <select
          v-if="source === 'gitlab' && gitlabInstances.length"
          class="input compact instance-select"
          :value="gitlabActiveId"
          title="选择 GitLab 实例"
          aria-label="选择 GitLab 实例"
          @change="onInstanceChange"
        >
          <option v-for="instance in gitlabInstances" :key="instance.id" :value="instance.id">
            {{ instance.name }}
          </option>
        </select>
        <label class="search-field">
          <Icon name="search" :size="16" />
          <input
            v-model="searchTerm"
            type="search"
            :placeholder="source === 'local' ? '搜索名称或路径' : '在 GitLab 上搜索项目'"
            :aria-label="source === 'local' ? '搜索项目' : '搜索 GitLab 项目'"
          />
          <button
            v-if="searchTerm"
            type="button"
            class="search-clear"
            aria-label="清除搜索"
            @click="clearSearch"
          >
            <Icon name="close" :size="14" />
          </button>
        </label>
        <span class="index-count">{{ countText }}</span>
      </div>

      <section v-if="source === 'local' && !projects.length" class="welcome">
        <div class="welcome-copy">
          <h2>把第一个仓库放进来</h2>
          <p>
            选择一个本地 Git 仓库。Easy Git 只读取本机数据，项目会留在这个列表里，
            下次打开就能直接继续。
          </p>
          <div class="page-actions">
            <button type="button" class="button primary" :disabled="picking" @click="addFromPicker">
              <Icon name="plus" :size="17" />
              {{ picking ? "读取仓库…" : "添加本地仓库" }}
            </button>
            <button type="button" class="button ghost" @click="setSource('gitlab')">
              <Icon name="cloud" :size="16" />
              从 GitLab 克隆
            </button>
          </div>
          <p class="welcome-note">
            <Icon name="lock" :size="14" />
            仓库路径保存在本机；凭据只写入系统钥匙串。
          </p>
        </div>
        <div class="welcome-art" aria-hidden="true">
          <DeltaMark seed="easy-git-welcome" animated />
          <span>从一条支流开始</span>
        </div>
      </section>

      <template v-else-if="source === 'local'">
        <div v-if="!filteredProjects.length" class="no-results">
          <p>没有匹配「{{ query }}」的项目。</p>
          <button type="button" class="text-link" @click="query = ''">清除搜索</button>
        </div>

        <div v-else class="project-list">
          <article
            v-for="project in filteredProjects"
            :key="project.path"
            class="project-row"
            :class="{ current: isCurrent(project), confirming: removePath === project.path }"
          >
            <button
              type="button"
              class="project-open"
              :disabled="repoLoading"
              :aria-label="`打开 ${project.name}`"
              @click="openProject(project)"
            >
              <span class="project-art" aria-hidden="true">
                <DeltaMark :seed="project.path" />
              </span>
              <span class="project-info">
                <span class="project-title-line">
                  <span class="project-name">{{ project.name }}</span>
                  <span v-if="isCurrent(project)" class="tag current-tag">
                    <span class="status-dot"></span>
                    正在查看
                  </span>
                </span>
                <span class="project-path" :title="project.path">{{ project.path }}</span>
                <span class="project-meta">
                  <template v-if="isCurrent(project)">
                    <Icon name="branch" :size="13" />
                    {{ session?.headBranch }}
                  </template>
                  <template v-else>
                    <Icon name="folder" :size="13" />
                    本地 Git 仓库
                  </template>
                </span>
              </span>
              <span class="project-open-mark" aria-hidden="true">
                <Icon name="arrow-up-right" :size="18" />
              </span>
            </button>

            <div class="project-menu-wrap">
              <button
                type="button"
                class="project-menu-trigger"
                :aria-expanded="menuPath === project.path"
                :aria-label="`${project.name} 的更多操作`"
                @click.stop="toggleMenu(project)"
              >
                <Icon name="more" :size="18" />
              </button>
              <Transition name="menu">
                <div v-if="menuPath === project.path" class="project-menu" @click.stop>
                  <button type="button" class="menu-item danger" @click="askRemove(project)">
                    <Icon name="close" :size="15" />
                    从项目列表移除
                  </button>
                </div>
              </Transition>
            </div>

            <div v-if="removePath === project.path" class="project-confirm" @click.stop>
              <div>
                <strong>从列表移除「{{ project.name }}」？</strong>
                <span>只是忘记这个路径，磁盘上的仓库文件不会被删除。</span>
              </div>
              <div class="page-actions">
                <button
                  type="button"
                  class="button danger-quiet small"
                  :disabled="Boolean(removing)"
                  @click="confirmRemove(project)"
                >
                  {{ removing === project.path ? "移除中…" : "确认移除" }}
                </button>
                <button
                  type="button"
                  class="button ghost small"
                  :disabled="Boolean(removing)"
                  @click="removePath = ''"
                >
                  取消
                </button>
              </div>
            </div>
          </article>
        </div>
      </template>

      <section v-else-if="!gitlabInstances.length" class="welcome">
        <div class="welcome-copy">
          <h2>接上团队的 GitLab</h2>
          <p>
            填好自建 GitLab 的站点地址与访问令牌，这里就能列出你参与的仓库，
            点一下便克隆到本机并加入项目列表。
          </p>
          <div class="page-actions">
            <button type="button" class="button primary" @click="navigate('settings')">
              前往设置
            </button>
          </div>
          <p class="welcome-note">
            <Icon name="lock" :size="14" />
            令牌只写入系统钥匙串，不会存进配置文件。
          </p>
        </div>
        <div class="welcome-art" aria-hidden="true">
          <DeltaMark seed="easy-git-gitlab" animated />
          <span>从团队的项目开始</span>
        </div>
      </section>

      <template v-else>
        <p v-if="gitlabError" class="alert error" role="alert">{{ gitlabError }}</p>

        <div v-if="!gitlabProjects.length" class="no-results">
          <p>
            {{
              gitlabSearch
                ? `没有匹配「${gitlabSearch}」的项目。`
                : gitlabError
                  ? "这个实例暂时读不到项目。"
                  : "这个实例上还没有你参与的仓库。"
            }}
          </p>
          <button v-if="gitlabSearch" type="button" class="text-link" @click="gitlabQuery = ''">
            清除搜索
          </button>
        </div>

        <div v-else class="project-list">
          <article
            v-for="project in gitlabProjects"
            :key="project.path"
            class="project-row gitlab-row"
            :class="{
              cloning: gitlabCloning === project.path,
              local: Boolean(localMatchFor(project.path)),
            }"
          >
            <button
              type="button"
              class="project-open"
              :disabled="Boolean(gitlabCloning) || repoLoading"
              :aria-label="`${localMatchFor(project.path) ? '打开' : '克隆并添加'} ${project.name}`"
              @click="activateGitlabProject(project)"
            >
              <span class="project-art" aria-hidden="true">
                <DeltaMark :seed="`gitlab:${project.path}`" />
              </span>
              <span class="project-info">
                <span class="project-title-line">
                  <span class="project-name">{{ project.name }}</span>
                  <span v-if="localMatchFor(project.path)" class="tag current-tag">
                    <span class="status-dot"></span>
                    已在本机
                  </span>
                  <span v-else-if="project.archived" class="tag tag-muted">已归档</span>
                  <span v-else-if="project.forkedFromPath" class="tag">复刻</span>
                </span>
                <span class="project-path" :title="project.path">{{ project.path }}</span>
                <span class="project-meta">
                  <Icon name="clock" :size="13" />
                  {{ gitlabMeta(project) }}
                </span>
              </span>
              <span class="project-open-mark">
                <Icon
                  :name="localMatchFor(project.path) ? 'arrow-up-right' : 'download'"
                  :size="18"
                  aria-hidden="true"
                />
                <span class="project-open-text">{{ gitlabActionLabel(project) }}</span>
              </span>
            </button>
          </article>
        </div>

        <div v-if="gitlabHasMore" class="load-more">
          <button
            type="button"
            class="button ghost"
            :disabled="gitlabLoadingMore"
            @click="loadGitlabProjects(false)"
          >
            {{ gitlabLoadingMore ? "加载中…" : "加载更多" }}
          </button>
        </div>
      </template>
    </template>

    <Transition name="veil">
      <div v-if="openingProject" class="opening-veil" role="status" aria-live="polite">
        <div class="opening-art" aria-hidden="true">
          <DeltaMark :seed="openingProject.path" animated />
        </div>
        <strong>{{ openingProject.name }}</strong>
        <span class="opening-path">{{ openingProject.path }}</span>
        <span class="opening-status">正在打开仓库…</span>
        <span class="opening-track" aria-hidden="true"><i></i></span>
      </div>
    </Transition>
  </div>
</template>
