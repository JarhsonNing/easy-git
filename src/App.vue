<script setup lang="ts">
import { computed, type Component } from "vue";
import GraphView from "./views/GraphView.vue";
import MrView from "./views/MrView.vue";
import GitLabView from "./views/GitLabView.vue";
import AiView from "./views/AiView.vue";
import SettingsView from "./views/SettingsView.vue";
import ProjectsView from "./views/ProjectsView.vue";
import Icon from "./components/Icon.vue";
import { session } from "./stores/repo";
import { activeView, navigate, type ViewKey } from "./stores/ui";

interface ProjectNavEntry {
  key: ViewKey;
  label: string;
  icon: string;
}

const projectViews: ProjectNavEntry[] = [
  { key: "graph", label: "提交图谱", icon: "branch" },
  { key: "mr", label: "合并请求", icon: "git-merge" },
  { key: "gitlab", label: "代码库", icon: "cloud" },
  { key: "ai", label: "AI 助手", icon: "sparkles" },
];

const viewComponents: Record<ViewKey, Component> = {
  projects: ProjectsView,
  graph: GraphView,
  mr: MrView,
  gitlab: GitLabView,
  ai: AiView,
  settings: SettingsView,
};

const currentComponent = computed(() => viewComponents[activeView.value]);
const inProject = computed(
  () => Boolean(session.value) && projectViews.some((view) => view.key === activeView.value),
);

const locationLabel = computed(() => {
  if (inProject.value) return session.value?.name ?? "项目";
  if (activeView.value === "settings") return "设置";
  return "项目";
});
</script>

<template>
  <div class="app">
    <header class="appbar">
      <div class="appbar-left">
        <button type="button" class="brand" title="返回项目列表" @click="navigate('projects')">
          <span class="brand-mark" aria-hidden="true">
            <svg viewBox="0 0 28 24" fill="none">
              <path d="M11 12.8C13.3 9.8 17 5.5 22.3 2.6" stroke="#D98F6C" stroke-width="2.2" stroke-linecap="round" />
              <path d="M11 12.8C15.4 11.7 20.2 11.2 25.6 11.3" stroke="#9FC4B8" stroke-width="2.2" stroke-linecap="round" />
              <path d="M11 12.8C14.2 15.9 17.7 19.1 22.6 21.5" stroke="#E0B25F" stroke-width="2.2" stroke-linecap="round" />
              <path d="M2.8 17.4C5.8 13.9 8.9 12.8 11 12.8" stroke="#F5EFE3" stroke-width="2.6" stroke-linecap="round" />
              <circle cx="11" cy="12.8" r="1.35" fill="#F5EFE3" />
            </svg>
          </span>
          <span class="brand-name">easy git</span>
        </button>

        <span class="appbar-rule" aria-hidden="true"></span>

        <template v-if="inProject">
          <button
            type="button"
            class="appbar-back"
            title="返回项目列表"
            @click="navigate('projects')"
          >
            <Icon name="arrow-left" :size="15" />
            返回
          </button>
          <div class="project-context">
            <strong>{{ locationLabel }}</strong>
            <span class="branch-pill">
              <Icon name="branch" :size="12" />
              {{ session?.headBranch }}
            </span>
          </div>
        </template>
        <span v-else class="location-label">{{ locationLabel }}</span>
      </div>

      <nav v-if="inProject" class="app-nav" aria-label="项目视图">
        <button
          v-for="view in projectViews"
          :key="view.key"
          type="button"
          class="app-nav-item"
          :class="{ active: activeView === view.key }"
          :aria-current="activeView === view.key ? 'page' : undefined"
          @click="navigate(view.key)"
        >
          <Icon :name="view.icon" :size="15" />
          <span>{{ view.label }}</span>
        </button>
      </nav>

      <div class="appbar-right">
        <button
          v-if="activeView === 'settings' && session"
          type="button"
          class="appbar-text-button"
          @click="navigate('graph')"
        >
          回到项目
        </button>
        <button
          type="button"
          class="appbar-icon"
          :class="{ active: activeView === 'settings' }"
          title="应用设置"
          @click="navigate('settings')"
        >
          <Icon name="settings" :size="16" />
          设置
        </button>
      </div>
    </header>

    <main class="app-main" :class="{ 'app-main-fill': inProject }">
      <Transition name="view" mode="out-in">
        <component :is="currentComponent" :key="activeView" />
      </Transition>
    </main>
  </div>
</template>
