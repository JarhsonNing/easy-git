<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import Icon from "../components/Icon.vue";
import DeltaMark from "../components/DeltaMark.vue";
import { errorText, pickRepository } from "../lib/api";
import {
  addProject,
  loadProjects,
  projects,
  projectsError,
  projectsLoading,
  removeProject,
  sameProjectPath,
} from "../stores/projects";
import { error as repoError, loading as repoLoading, openRepo, session } from "../stores/repo";
import { navigate } from "../stores/ui";
import type { ProjectEntry } from "../types";

const picking = ref(false);
const openingPath = ref("");
const removePath = ref("");
const removing = ref("");
const menuPath = ref("");
const actionError = ref("");
const notice = ref("");
const query = ref("");

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

onMounted(() => {
  void loadProjects();
  document.addEventListener("click", closeMenu);
});

onBeforeUnmount(() => {
  document.removeEventListener("click", closeMenu);
});

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

async function openProject(project: ProjectEntry) {
  if (repoLoading.value || openingPath.value) return;
  actionError.value = "";
  notice.value = "";
  removePath.value = "";
  closeMenu();
  openingPath.value = project.path;
  const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const [opened] = await Promise.all([
    openRepo(project.path),
    wait(reduced ? 0 : 480),
  ]);
  if (opened) {
    navigate("graph");
  } else if (repoError.value) {
    actionError.value = repoError.value;
  }
  openingPath.value = "";
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
        <p>本地仓库都在这里。打开一个，继续今天的改动。</p>
      </div>
      <div class="page-actions">
        <button
          type="button"
          class="button ghost"
          :disabled="projectsLoading || picking"
          @click="loadProjects"
        >
          <Icon name="refresh" :size="16" />
          {{ projectsLoading ? "刷新中…" : "刷新" }}
        </button>
        <button
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

    <div v-if="projectsLoading && !projects.length" class="index-loading" role="status">
      <span class="loading-orbit" aria-hidden="true"></span>
      <span>正在读取本地项目…</span>
    </div>

    <section v-else-if="!projects.length" class="welcome">
      <div class="welcome-copy">
        <h2>把第一个仓库放进来</h2>
        <p>
          选择一个本地 Git 仓库。Easy Git 只读取本机数据，项目会留在这个列表里，
          下次打开就能直接继续。
        </p>
        <div class="page-actions">
          <button
            type="button"
            class="button primary"
            :disabled="picking"
            @click="addFromPicker"
          >
            <Icon name="plus" :size="17" />
            {{ picking ? "读取仓库…" : "添加本地仓库" }}
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

    <template v-else>
      <div class="index-bar">
        <label class="search-field">
          <Icon name="search" :size="16" />
          <input v-model="query" type="search" placeholder="搜索名称或路径" aria-label="搜索项目" />
          <button
            v-if="query"
            type="button"
            class="search-clear"
            aria-label="清除搜索"
            @click="query = ''"
          >
            <Icon name="close" :size="14" />
          </button>
        </label>
        <span class="index-count">
          {{ query ? `${filteredProjects.length} / ${projects.length}` : projects.length }} 个仓库
        </span>
      </div>

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
