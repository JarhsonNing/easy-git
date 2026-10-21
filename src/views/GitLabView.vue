<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import Icon from "../components/Icon.vue";
import DeltaMark from "../components/DeltaMark.vue";
import { errorText } from "../lib/api";
import { session, openRepo } from "../stores/repo";
import { addProject } from "../stores/projects";
import { navigate } from "../stores/ui";
import {
  activeInstanceId,
  instances,
} from "../stores/mr";
import {
  branches,
  branchesLoading,
  cloneBusy,
  cloneError,
  cloneTo,
  createPipeline,
  expandedDirs,
  fileContent,
  fileError,
  filePath,
  fileLoading,
  jobs,
  jobsError,
  jobsLoading,
  loadDir,
  loadPipelines,
  openFile,
  pipelines,
  pipelinesError,
  pipelinesHasMore,
  pipelinesLoading,
  pipelinesLoadingMore,
  prepare,
  projects,
  projectsError,
  projectsLoading,
  refreshJobs,
  refreshTrace,
  repoProjectPath,
  runBusy,
  runError,
  runNotice,
  selectInstance,
  selectJob,
  selectPipeline,
  selectProject,
  selectedJobId,
  selectedPath,
  selectedPipelineId,
  selectedRef,
  setRef,
  summary,
  summaryError,
  summaryLoading,
  syncBusy,
  syncError,
  syncFork,
  syncNotice,
  tab,
  toggleDir,
  trace,
  traceError,
  traceLoading,
  tree,
  treeError,
  treeLoading,
  type GitLabTab,
} from "../stores/gitlab";
import type { GitLabTreeEntry } from "../types";

const refreshing = ref(false);
const syncOpen = ref(false);
const syncBranch = ref("");
const openNotice = ref("");
const runBranch = ref("");

let noticeTimer: number | undefined;

const tabs: { key: GitLabTab; label: string }[] = [
  { key: "code", label: "代码" },
  { key: "pipelines", label: "流水线" },
];

const PIPELINE_TEXT: Record<string, string> = {
  created: "已创建",
  waiting_for_resource: "等待资源",
  preparing: "准备中",
  pending: "等待中",
  running: "运行中",
  success: "已通过",
  failed: "已失败",
  canceled: "已取消",
  skipped: "已跳过",
  manual: "手动",
  scheduled: "已排期",
};

const PIPELINE_CLASS: Record<string, string> = {
  success: "ok",
  failed: "fail",
  running: "run",
  canceled: "muted",
  skipped: "muted",
  created: "muted",
  pending: "run",
  preparing: "run",
  waiting_for_resource: "muted",
  manual: "muted",
  scheduled: "muted",
};

function statusText(status: string): string {
  return PIPELINE_TEXT[status] ?? status;
}

function statusClass(status: string): string {
  return PIPELINE_CLASS[status] ?? "muted";
}

const relativeFormatter = new Intl.DateTimeFormat("zh-CN", {
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
});

function relative(iso: string | null): string {
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
  return relativeFormatter.format(new Date(time));
}

function duration(seconds: number | null): string {
  if (seconds == null || seconds <= 0) return "";
  if (seconds < 60) return `${seconds} 秒`;
  const minutes = Math.floor(seconds / 60);
  const rest = seconds % 60;
  if (minutes < 60) return rest ? `${minutes} 分 ${rest} 秒` : `${minutes} 分钟`;
  const hours = Math.floor(minutes / 60);
  return `${hours} 小时 ${minutes % 60} 分`;
}

function shortSha(sha: string): string {
  return sha.slice(0, 8);
}

interface TreeRow {
  entry: GitLabTreeEntry;
  depth: number;
}

const flatTree = computed<TreeRow[]>(() => {
  const rows: TreeRow[] = [];
  const walk = (dir: string, depth: number) => {
    for (const entry of tree.value[dir] ?? []) {
      rows.push({ entry, depth });
      if (entry.isDir && expandedDirs.value.has(entry.path)) {
        walk(entry.path, depth + 1);
      }
    }
  };
  walk("", 0);
  return rows;
});

const fileLines = computed<string[]>(() => {
  const text = fileContent.value?.content ?? "";
  if (!text) return [];
  return text.replace(/\n$/, "").split(/\r?\n/);
});

const fileExtension = computed(() => {
  const name = filePath.value ?? "";
  const dot = name.lastIndexOf(".");
  return dot >= 0 ? name.slice(dot + 1).toLowerCase() : "";
});

const isFork = computed(() => Boolean(summary.value?.forkedFromPath));

const scopeHint = computed(() => {
  if (projects.value.length) {
    return projects.value.length > 1 ? "当前仓库与它的上游" : "只有当前仓库";
  }
  return projectsLoading.value ? "正在定位当前仓库…" : "没有可浏览的仓库";
});

const stages = computed(() => {
  const groups = new Map<string, typeof jobs.value>();
  for (const job of jobs.value) {
    const list = groups.get(job.stage) ?? [];
    list.push(job);
    groups.set(job.stage, list);
  }
  return Array.from(groups.entries());
});

async function enter() {
  await prepare();
}

onMounted(() => {
  void enter();
});

watch(
  () => session.value?.path ?? null,
  (path) => {
    if (path) void enter();
  },
);

watch(
  () => summary.value?.defaultBranch,
  (branch) => {
    if (branch) syncBranch.value = branch;
  },
);

watch(selectedPath, () => {
  runBranch.value = "";
});

watch(selectedRef, (branch) => {
  if (branch) runBranch.value = branch;
});

onBeforeUnmount(() => {
  window.clearTimeout(noticeTimer);
});

async function refreshAll() {
  if (refreshing.value) return;
  refreshing.value = true;
  try {
    await prepare();
    if (selectedPath.value) {
      await Promise.all([loadDir(""), loadPipelines(true)]);
    }
  } finally {
    refreshing.value = false;
  }
}

function onInstanceChange(event: Event) {
  const value = (event.target as HTMLSelectElement).value;
  if (value) void selectInstance(value);
}

function setTab(next: GitLabTab) {
  tab.value = next;
}

async function onRowClick(entry: GitLabTreeEntry) {
  if (entry.isDir) {
    await toggleDir(entry.path);
  } else {
    await openFile(entry.path);
  }
}

async function confirmSync() {
  if (!syncBranch.value) return;
  const done = await syncFork(syncBranch.value);
  if (!done) return;
  showOpenNotice(syncNotice.value || `已提交同步请求：${syncBranch.value}`);
  syncOpen.value = false;
}

function openSyncDock() {
  syncError.value = "";
  syncNotice.value = "";
  if (!syncBranch.value) {
    syncBranch.value = selectedRef.value || summary.value?.defaultBranch || "";
  }
  syncOpen.value = true;
}

async function confirmRun() {
  await createPipeline(runBranch.value);
}

const JOB_PENDING = new Set([
  "created",
  "waiting_for_resource",
  "preparing",
  "pending",
  "scheduled",
]);

const jobsPending = computed(() => {
  const item = pipelines.value.find((row) => row.id === selectedPipelineId.value);
  return Boolean(item && JOB_PENDING.has(item.status));
});

async function openInGitLab() {
  if (!summary.value?.webUrl) return;
  try {
    await openUrl(summary.value.webUrl);
  } catch (error) {
    syncError.value = errorText(error);
  }
}

async function startClone() {
  if (cloneBusy.value) return;
  cloneError.value = "";
  const parent = await openDialog({
    directory: true,
    multiple: false,
    title: "选择克隆存放目录",
  });
  if (typeof parent !== "string" || !parent) return;
  const result = await cloneTo(parent);
  if (!result) return;
  try {
    await addProject(result.path);
    const opened = await openRepo(result.path);
    if (opened) {
      showOpenNotice(`已克隆并打开 ${result.name}`);
      navigate("graph");
    } else {
      showOpenNotice(`已克隆到 ${result.path}`);
    }
  } catch (error) {
    cloneError.value = `克隆成功，但加入项目列表失败：${errorText(error)}`;
  }
}

function showOpenNotice(message: string) {
  openNotice.value = message;
  window.clearTimeout(noticeTimer);
  noticeTimer = window.setTimeout(() => {
    openNotice.value = "";
  }, 6000);
}
</script>

<template>
  <div class="workspace gitlab-workspace">
    <div v-if="!session" class="workspace-empty">
      <div class="empty-panel">
        <div class="empty-art" aria-hidden="true">
          <DeltaMark seed="gitlab-browse-empty" animated />
        </div>
        <h2>请先选择一个项目</h2>
        <p>打开仓库后，可以在这里浏览 GitLab 上的项目、代码与流水线，也可以把项目克隆到本地。</p>
        <button type="button" class="button primary" @click="navigate('projects')">
          返回项目列表
        </button>
      </div>
    </div>

    <template v-else>
      <div class="workspace-bar">
        <div class="workspace-heading">
          <h2>代码库</h2>
          <p v-if="summary">{{ summary.nameWithNamespace }}</p>
          <p v-else>浏览 GitLab 实例上的项目、代码与流水线</p>
        </div>
        <div class="workspace-actions">
          <select
            v-if="instances.length"
            class="input compact instance-select"
            :value="activeInstanceId"
            title="选择 GitLab 实例"
            @change="onInstanceChange"
          >
            <option v-for="instance in instances" :key="instance.id" :value="instance.id">
              {{ instance.name }}
            </option>
          </select>
          <button type="button" class="button ghost small" :disabled="refreshing" @click="refreshAll">
            <Icon name="refresh" :size="15" />
            {{ refreshing ? "刷新中…" : "刷新" }}
          </button>
        </div>
      </div>

      <p v-if="projectsError && !projects.length && instances.length" class="alert error" role="alert">
        {{ projectsError }}
      </p>
      <p v-if="openNotice" class="alert success" role="status">{{ openNotice }}</p>

      <div class="gl-layout">
        <section class="surface gl-list" aria-label="当前仓库与上游">
          <div class="gl-list-head">
            <span class="gl-list-title">仓库</span>
            <span class="gl-list-hint">{{ scopeHint }}</span>
          </div>

          <div class="gl-scroll">
            <button
              v-for="item in projects"
              :key="item.path"
              type="button"
              class="gl-project"
              :class="{ selected: selectedPath === item.path }"
              @click="selectProject(item.path)"
            >
              <span class="gl-project-top">
                <span class="gl-project-name">{{ item.name }}</span>
                <span class="gl-project-time">{{ relative(item.lastActivityAt) }}</span>
              </span>
              <span class="gl-project-path">{{ item.path }}</span>
              <span v-if="item.path === repoProjectPath()" class="tag tag-current">当前仓库</span>
              <span v-else class="tag">上游源仓库</span>
              <span v-if="item.archived" class="tag tag-muted">已归档</span>
            </button>

            <div v-if="projectsLoading && !projects.length" class="list-state">
              正在定位当前仓库…
            </div>
            <div v-else-if="!projects.length" class="list-state gl-setup-hint">
              <p>{{ projectsError || "当前仓库没有对应的 GitLab 项目。" }}</p>
              <button
                v-if="!instances.length"
                type="button"
                class="button ghost small"
                @click="navigate('settings')"
              >
                前往设置
              </button>
              <button
                v-else
                type="button"
                class="button ghost small"
                @click="navigate('projects')"
              >
                去项目首页浏览 GitLab
              </button>
            </div>
          </div>
        </section>

        <section class="surface gl-detail" aria-label="GitLab 项目详情">
          <div v-if="!selectedPath" class="inspector-empty">
            <div class="inspector-mark" aria-hidden="true">
              <DeltaMark seed="select-gitlab-project" />
            </div>
            <p>选一个项目</p>
            <span>代码、流水线和克隆操作会在这里展开。</span>
          </div>

          <div v-else class="gl-detail-scroll">
            <div v-if="summaryLoading" class="list-state">正在读取项目…</div>
            <p v-else-if="summaryError" class="alert error" role="alert">{{ summaryError }}</p>

            <template v-if="summary">
              <header class="gl-head">
                <div class="gl-head-main">
                  <h3>{{ summary.name }}</h3>
                  <p class="gl-head-path">{{ summary.nameWithNamespace }}</p>
                  <p v-if="summary.description" class="gl-head-desc">{{ summary.description }}</p>
                  <p v-if="summary.forkedFromPath" class="gl-head-fork">
                    <Icon name="git-compare" :size="13" />
                    复刻自
                    <a
                      v-if="summary.forkedFromWebUrl"
                      :href="summary.forkedFromWebUrl"
                      target="_blank"
                      rel="noreferrer"
                      >{{ summary.forkedFromPath }}</a
                    >
                    <template v-else>{{ summary.forkedFromPath }}</template>
                  </p>
                </div>
                <div class="gl-head-actions">
                  <button type="button" class="button ghost small" @click="openInGitLab">
                    <Icon name="external" :size="14" />
                    在 GitLab 打开
                  </button>
                  <button
                    v-if="isFork"
                    type="button"
                    class="button ghost small"
                    :disabled="syncBusy"
                    @click="openSyncDock"
                  >
                    <Icon name="refresh" :size="14" />
                    同步复刻
                  </button>
                  <button
                    type="button"
                    class="button primary small"
                    :disabled="cloneBusy"
                    @click="startClone"
                  >
                    <Icon name="download" :size="15" />
                    {{ cloneBusy ? "克隆中…" : "克隆到本地" }}
                  </button>
                </div>
              </header>

              <div v-if="syncOpen" class="sync-dock">
                <div class="sync-dock-main">
                  <strong>同步复刻</strong>
                  <span>
                    把上游 {{ summary.forkedFromPath }} 的最新提交同步到本复刻的分支。
                  </span>
                </div>
                <div class="sync-dock-controls">
                  <select v-model="syncBranch" class="input compact" :disabled="syncBusy">
                    <option
                      v-for="branch in branches"
                      :key="branch.name"
                      :value="branch.name"
                    >
                      {{ branch.name }}
                    </option>
                  </select>
                  <button
                    type="button"
                    class="button primary small"
                    :disabled="syncBusy || !syncBranch"
                    @click="confirmSync"
                  >
                    {{ syncBusy ? "同步中…" : "确认同步" }}
                  </button>
                  <button
                    type="button"
                    class="button ghost small"
                    :disabled="syncBusy"
                    @click="syncOpen = false"
                  >
                    取消
                  </button>
                </div>
                <p v-if="syncNotice" class="alert success" role="status">{{ syncNotice }}</p>
                <p v-if="syncError" class="alert error" role="alert">{{ syncError }}</p>
              </div>

              <p v-if="cloneError" class="alert error" role="alert">{{ cloneError }}</p>

              <div class="tabs" role="tablist" aria-label="项目内容">
                <button
                  v-for="item in tabs"
                  :key="item.key"
                  type="button"
                  class="tab"
                  :class="{ active: tab === item.key }"
                  role="tab"
                  :aria-selected="tab === item.key"
                  @click="setTab(item.key)"
                >
                  {{ item.label }}
                </button>
              </div>

              <div v-show="tab === 'code'" class="gl-code">
                <div class="gl-tree">
                  <div class="gl-tree-head">
                    <select
                      class="input compact"
                      :value="selectedRef"
                      :disabled="branchesLoading"
                      title="选择分支"
                      @change="setRef(($event.target as HTMLSelectElement).value)"
                    >
                      <option v-if="!branches.length" value="" disabled>
                        {{ branchesLoading ? "读取分支…" : "没有分支" }}
                      </option>
                      <option v-for="branch in branches" :key="branch.name" :value="branch.name">
                        {{ branch.name }}{{ branch.isDefault ? "（默认）" : "" }}
                      </option>
                    </select>
                  </div>
                  <div class="gl-tree-body">
                    <div v-if="treeLoading" class="list-state">正在读取目录…</div>
                    <template v-else>
                      <button
                        v-for="row in flatTree"
                        :key="row.entry.path"
                        type="button"
                        class="gl-tree-row"
                        :class="{
                          selected: filePath === row.entry.path,
                          dir: row.entry.isDir,
                        }"
                        :style="{ paddingLeft: `${10 + row.depth * 14}px` }"
                        :title="row.entry.path"
                        @click="onRowClick(row.entry)"
                      >
                        <Icon
                          v-if="row.entry.isDir"
                          name="chevron-right"
                          :size="12"
                          class="gl-tree-chevron"
                          :class="{ open: expandedDirs.has(row.entry.path) }"
                        />
                        <Icon v-else name="file" :size="13" class="gl-tree-icon" />
                        <span class="gl-tree-name">{{ row.entry.name }}</span>
                      </button>
                      <div v-if="!flatTree.length" class="list-state">
                        {{ treeError || "这个分支是空的" }}
                      </div>
                    </template>
                    <p v-if="treeError && flatTree.length" class="alert error gl-inline-error">
                      {{ treeError }}
                    </p>
                  </div>
                </div>

                <div class="gl-file">
                  <div v-if="!filePath" class="inspector-empty">
                    <div class="inspector-mark" aria-hidden="true">
                      <DeltaMark seed="select-gitlab-file" />
                    </div>
                    <p>选一个文件</p>
                    <span>文件内容会按所选分支显示。</span>
                  </div>
                  <template v-else>
                    <div class="gl-file-head">
                      <Icon name="file" :size="14" />
                      <span class="gl-file-path" :title="filePath">{{ filePath }}</span>
                      <span v-if="fileContent" class="gl-file-meta">
                        {{ fileExtension ? `${fileExtension} · ` : "" }}{{ fileContent.size }} 字节
                      </span>
                    </div>
                    <div class="gl-file-body">
                      <div v-if="fileLoading" class="list-state">正在读取文件…</div>
                      <p v-else-if="fileError" class="alert error" role="alert">{{ fileError }}</p>
                      <div v-else-if="fileContent?.binary" class="file-empty">
                        这是二进制文件，无法直接显示。
                      </div>
                      <p v-else-if="fileContent?.truncated" class="alert info">
                        文件过大，仅显示前 256 KB。
                      </p>
                      <pre v-if="fileContent && !fileContent.binary" class="gl-file-pre"><code><span
                        v-for="(line, index) in fileLines"
                        :key="index"
                        class="gl-file-line"
                      ><span class="gl-line-no">{{ index + 1 }}</span><span class="gl-line-text">{{ line || " " }}</span></span></code></pre>
                    </div>
                  </template>
                </div>
              </div>

              <div v-show="tab === 'pipelines'" class="gl-run">
                <div class="gl-run-controls">
                  <label class="gl-run-label" for="gl-run-branch">运行分支</label>
                  <select
                    id="gl-run-branch"
                    v-model="runBranch"
                    class="input compact"
                    :disabled="runBusy || !branches.length"
                    title="选择要运行流水线的分支"
                  >
                    <option v-if="!branches.length" value="" disabled>
                      {{ branchesLoading ? "读取分支…" : "没有分支" }}
                    </option>
                    <option v-for="branch in branches" :key="branch.name" :value="branch.name">
                      {{ branch.name }}
                    </option>
                  </select>
                  <button
                    type="button"
                    class="button primary small"
                    :disabled="runBusy || !runBranch"
                    @click="confirmRun"
                  >
                    <Icon name="play" :size="13" />
                    {{ runBusy ? "创建中…" : "运行流水线" }}
                  </button>
                </div>
                <p v-if="runNotice" class="alert success" role="status">{{ runNotice }}</p>
                <p v-else-if="runError" class="alert error" role="alert">{{ runError }}</p>
              </div>

              <div v-show="tab === 'pipelines'" class="gl-pipes">
                <div class="gl-pipe-list">
                  <div class="gl-pipe-scroll">
                    <button
                      v-for="item in pipelines"
                      :key="item.id"
                      type="button"
                      class="gl-pipeline"
                      :class="{ selected: selectedPipelineId === item.id }"
                      @click="selectPipeline(item.id)"
                    >
                      <span class="gl-pipeline-top">
                        <span class="gl-status" :class="`gl-status-${statusClass(item.status)}`">
                          <span class="gl-status-dot"></span>
                          {{ statusText(item.status) }}
                        </span>
                        <span class="gl-pipeline-time">{{ relative(item.updatedAt) }}</span>
                      </span>
                      <span class="gl-pipeline-branch">
                        <Icon name="branch" :size="12" />
                        {{ item.refName }}
                      </span>
                      <span class="gl-pipeline-sha" :title="item.sha">
                        #{{ item.iid }} · {{ shortSha(item.sha) }}
                      </span>
                      <span v-if="item.duration" class="gl-pipeline-duration">
                        {{ duration(item.duration) }}
                      </span>
                    </button>

                    <div v-if="pipelinesLoading" class="list-state">正在读取流水线…</div>
                    <div v-else-if="!pipelines.length" class="list-state">
                      {{ pipelinesError || "还没有流水线" }}
                    </div>
                    <p v-if="pipelinesError" class="alert error gl-inline-error">
                      {{ pipelinesError }}
                    </p>

                    <div v-if="pipelinesHasMore" class="load-more">
                      <button
                        type="button"
                        class="button ghost small"
                        :disabled="pipelinesLoadingMore"
                        @click="loadPipelines(false)"
                      >
                        {{ pipelinesLoadingMore ? "加载中…" : "加载更多" }}
                      </button>
                    </div>
                  </div>
                </div>

                <div class="gl-pipe-detail">
                  <div v-if="!selectedPipelineId" class="inspector-empty">
                    <div class="inspector-mark" aria-hidden="true">
                      <DeltaMark seed="select-pipeline" />
                    </div>
                    <p>选一条流水线</p>
                    <span>作业和运行日志会在这里展开。</span>
                  </div>
                  <template v-else>
                    <div class="gl-jobs">
                      <div v-if="jobsLoading" class="list-state">正在读取作业…</div>
                      <p v-else-if="jobsError" class="alert error" role="alert">{{ jobsError }}</p>
                      <template v-else>
                        <div v-for="[stage, stageJobs] in stages" :key="stage" class="gl-stage">
                          <span class="gl-stage-name">{{ stage }}</span>
                          <button
                            v-for="job in stageJobs"
                            :key="job.id"
                            type="button"
                            class="gl-job"
                            :class="{ selected: selectedJobId === job.id }"
                            @click="selectJob(job.id)"
                          >
                            <span class="gl-status" :class="`gl-status-${statusClass(job.status)}`">
                              <span class="gl-status-dot"></span>
                              {{ statusText(job.status) }}
                            </span>
                            <span class="gl-job-name">{{ job.name }}</span>
                            <span class="gl-job-duration">{{ duration(job.duration) }}</span>
                          </button>
                        </div>
                        <div v-if="!jobs.length" class="list-state gl-jobs-empty">
                          <span>{{
                            jobsPending
                              ? "这条流水线还在创建作业。"
                              : "这条流水线没有作业"
                          }}</span>
                          <button
                            v-if="jobsPending"
                            type="button"
                            class="button ghost tiny"
                            :disabled="jobsLoading"
                            @click="refreshJobs"
                          >
                            <Icon name="refresh" :size="13" />
                            {{ jobsLoading ? "读取中…" : "刷新作业" }}
                          </button>
                        </div>
                      </template>
                    </div>

                    <div class="gl-trace">
                      <div class="gl-trace-head">
                        <strong>
                          {{ jobs.find((job) => job.id === selectedJobId)?.name ?? "作业日志" }}
                        </strong>
                        <span v-if="trace?.size" class="gl-trace-meta">
                          {{ trace.truncated ? "仅显示末尾 256 KB · " : "" }}{{ trace.size }} 字节
                        </span>
                        <button
                          type="button"
                          class="button ghost tiny"
                          :disabled="traceLoading || !selectedJobId"
                          @click="refreshTrace"
                        >
                          <Icon name="refresh" :size="13" />
                          {{ traceLoading ? "读取中…" : "刷新" }}
                        </button>
                      </div>
                      <div class="gl-trace-body">
                        <div v-if="traceLoading" class="list-state">正在读取日志…</div>
                        <p v-else-if="traceError" class="alert error" role="alert">
                          {{ traceError }}
                        </p>
                        <div v-else-if="!trace?.trace" class="file-empty">
                          这条作业还没有日志。
                        </div>
                        <pre v-else class="gl-trace-pre">{{ trace.trace }}</pre>
                      </div>
                    </div>
                  </template>
                </div>
              </div>
            </template>
          </div>
        </section>
      </div>
    </template>
  </div>
</template>
