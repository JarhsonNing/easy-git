<script setup lang="ts">
import { computed, type Component } from "vue";
import GraphView from "./views/GraphView.vue";
import MrView from "./views/MrView.vue";
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
  { key: "ai", label: "AI 助手", icon: "sparkles" },
];

const viewComponents: Record<ViewKey, Component> = {
  projects: ProjectsView,
  graph: GraphView,
  mr: MrView,
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
              <path d="M1 15C7 15 9 14 13 10C16 7 20 5 27 4" stroke="#F5EFE3" stroke-width="2" stroke-linecap="round" />
              <path d="M1 12C8 12 11 10 15 7C19 4 22 3 27 8" stroke="#D98F6C" stroke-width="1.6" stroke-linecap="round" opacity=".9" />
              <path d="M1 18C8 18 11 17 16 15C20 13 23 13 27 14" stroke="#9FC4B8" stroke-width="1.6" stroke-linecap="round" opacity=".9" />
              <circle cx="27" cy="4" r="2.2" fill="#F5EFE3" />
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
            aria-label="返回项目列表"
            @click="navigate('projects')"
          >
            <Icon name="arrow-left" :size="17" />
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
          aria-label="应用设置"
          @click="navigate('settings')"
        >
          <Icon name="settings" :size="18" />
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
