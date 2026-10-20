<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import Icon from "../components/Icon.vue";
import DeltaMark from "../components/DeltaMark.vue";
import { errorText } from "../lib/api";
import { session } from "../stores/repo";
import {
  actionBusy,
  actionError,
  activeInstanceId,
  branches,
  branchesError,
  branchesLoading,
  changes,
  createCurrentMR,
  detail,
  detailError,
  detailLoading,
  filter,
  hasMore,
  instances,
  items,
  listError,
  listLoading,
  listLoadingMore,
  loadBranches,
  loadList,
  mergeCurrent,
  notes,
  prepare,
  project,
  projectError,
  projectLoading,
  remoteInfo,
  refreshDetail,
  search,
  selectInstance,
  selectMergeRequest,
  selectedIid,
  toggleCurrentState,
} from "../stores/mr";
import { navigate } from "../stores/ui";
import type { MergeRequest, MergeRequestFile, MergeRequestFilter } from "../types";

const stateTabs: { key: MergeRequestFilter; label: string }[] = [
  { key: "opened", label: "打开" },
  { key: "merged", label: "已合并" },
  { key: "closed", label: "已关闭" },
  { key: "all", label: "全部" },
];

const refreshing = ref(false);
const searchInput = ref(search.value);
const detailTab = ref<"changes" | "notes">("changes");
const expandedFiles = ref<Set<string>>(new Set());

const mergeConfirm = ref(false);
const squash = ref(false);
const removeSourceBranch = ref(true);

const createOpen = ref(false);
const createSource = ref("");
const createTarget = ref("");
const createTitle = ref("");
const createDescription = ref("");
const createRemoveSource = ref(true);
const createError = ref("");

const creating = computed(() => actionBusy.value === "create");
const canMerge = computed(
  () => detail.value?.state === "opened" && !detail.value.draft && !detail.value.hasConflicts,
);
const currentMergeHint = computed(() =>
  detail.value?.state === "opened" ? mergeHint(detail.value) : null,
);

let searchTimer: number | undefined;

async function enter() {
  await prepare();
  if (project.value) await loadList(true);
}

function onEscape(event: KeyboardEvent) {
  if (event.key === "Escape" && createOpen.value) closeCreate();
}

onMounted(() => {
  void enter();
  window.addEventListener("keydown", onEscape);
});

watch(
  () => session.value?.path ?? null,
  (path) => {
    if (path) void enter();
  },
);

watch(selectedIid, () => {
  detailTab.value = "changes";
  mergeConfirm.value = false;
  actionError.value = "";
});

watch(changes, (value) => {
  const next = new Set<string>();
  if (value) {
    for (const file of value.files) {
      if (next.size >= 4) break;
      if (!file.tooLarge && file.diff.length <= 8000) next.add(fileKey(file));
    }
  }
  expandedFiles.value = next;
});

watch(searchInput, (value) => {
  window.clearTimeout(searchTimer);
  searchTimer = window.setTimeout(() => {
    const term = value.trim();
    if (search.value === term) return;
    search.value = term;
    if (project.value) void loadList(true);
  }, 350);
});

onBeforeUnmount(() => {
  window.clearTimeout(searchTimer);
  window.removeEventListener("keydown", onEscape);
});

async function refreshAll() {
  if (refreshing.value) return;
  refreshing.value = true;
  try {
    await prepare();
    if (project.value) {
      await loadList(true);
      if (selectedIid.value) await selectMergeRequest(selectedIid.value);
    }
  } finally {
    refreshing.value = false;
  }
}

function setFilter(next: MergeRequestFilter) {
  if (filter.value === next) return;
  filter.value = next;
  if (project.value) void loadList(true);
}

function onInstanceChange(event: Event) {
  const value = (event.target as HTMLSelectElement).value;
  if (value) void selectInstance(value);
}

function select(iid: number) {
  if (selectedIid.value === iid) return;
  void selectMergeRequest(iid);
}

const STATE_TEXT: Record<string, string> = {
  opened: "打开",
  merged: "已合并",
  closed: "已关闭",
  locked: "已锁定",
};

function stateText(state: string): string {
  return STATE_TEXT[state] ?? state;
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

function mergeHint(mr: MergeRequest): { text: string; kind: string } | null {
  if (mr.draft) return { text: "草稿状态，暂不可合并", kind: "warn" };
  if (mr.hasConflicts || mr.mergeStatus === "conflict" || mr.mergeStatus === "cannot_be_merged") {
    return { text: "存在合并冲突", kind: "warn" };
  }
  if (mr.mergeStatus === "mergeable") return { text: "可合并", kind: "ok" };
  if (mr.mergeStatus === "ci_must_pass") return { text: "等待流水线通过", kind: "warn" };
  if (mr.mergeStatus === "discussions_not_resolved") return { text: "有未解决的讨论", kind: "warn" };
  if (mr.mergeStatus === "not_approved") return { text: "等待审批", kind: "warn" };
  if (mr.mergeStatus === "checking" || mr.mergeStatus === "unchecked") {
    return { text: "正在检查可合并性", kind: "muted" };
  }
  return null;
}

interface DiffLine {
  kind: "add" | "del" | "hunk" | "meta" | "context";
  text: string;
}

const diffCache = new WeakMap<MergeRequestFile, DiffLine[]>();

function diffLines(file: MergeRequestFile): DiffLine[] {
  const cached = diffCache.get(file);
  if (cached) return cached;
  const raw = file.diff.replace(/\n$/, "");
  const lines = (raw ? raw.split(/\r?\n/) : []).map((text) => {
    let kind: DiffLine["kind"] = "context";
    if (text.startsWith("@@")) kind = "hunk";
    else if (text.startsWith("+++") || text.startsWith("---")) kind = "meta";
    else if (text.startsWith("+")) kind = "add";
    else if (text.startsWith("-")) kind = "del";
    else if (text.startsWith("\\")) kind = "meta";
    return { kind, text };
  });
  diffCache.set(file, lines);
  return lines;
}

function fileKey(file: MergeRequestFile): string {
  return `${file.oldPath}->${file.newPath}`;
}

function fileStatus(file: MergeRequestFile): string {
  if (file.newFile) return "A";
  if (file.deletedFile) return "D";
  if (file.renamedFile) return "R";
  return "M";
}

function statusClass(file: MergeRequestFile): string {
  if (file.newFile) return "status-added";
  if (file.deletedFile) return "status-deleted";
  if (file.renamedFile) return "status-renamed";
  return "status-modified";
}

function fileStats(file: MergeRequestFile): { additions: number; deletions: number } {
  let additions = 0;
  let deletions = 0;
  for (const line of diffLines(file)) {
    if (line.kind === "add") additions += 1;
    else if (line.kind === "del") deletions += 1;
  }
  return { additions, deletions };
}

function expanded(file: MergeRequestFile): boolean {
  return expandedFiles.value.has(fileKey(file));
}

function toggleFile(file: MergeRequestFile) {
  const next = new Set(expandedFiles.value);
  const key = fileKey(file);
  if (next.has(key)) next.delete(key);
  else next.add(key);
  expandedFiles.value = next;
}

function filePath(file: MergeRequestFile): string {
  return file.renamedFile ? `${file.oldPath} → ${file.newPath}` : file.newPath;
}

function initial(name: string): string {
  return name.trim().slice(0, 1).toUpperCase() || "G";
}

async function openInGitLab() {
  if (!detail.value?.webUrl) return;
  try {
    await openUrl(detail.value.webUrl);
  } catch (error) {
    detailError.value = errorText(error);
  }
}

async function confirmMerge() {
  if (!detail.value) return;
  const merged = await mergeCurrent({
    squash: squash.value,
    removeSourceBranch: removeSourceBranch.value,
  });
  if (merged) {
    mergeConfirm.value = false;
    await selectMergeRequest(detail.value.iid);
  }
}

async function toggleState() {
  if (!detail.value) return;
  await toggleCurrentState();
}

async function openCreate() {
  createError.value = "";
  actionError.value = "";
  createOpen.value = true;
  createDescription.value = "";
  await loadBranches();
  const list = branches.value;
  const defaultBranch = list.find((branch) => branch.isDefault)?.name ?? list[0]?.name ?? "";
  createTarget.value = defaultBranch;
  const head = session.value?.headBranch ?? "";
  createSource.value = list.some((branch) => branch.name === head) ? head : "";
  createTitle.value = head;
  createRemoveSource.value = true;
  if (!list.length && branchesError.value) createError.value = branchesError.value;
}

function closeCreate() {
  if (creating.value) return;
  createOpen.value = false;
}

async function submitCreate() {
  if (!createSource.value || !createTarget.value) {
    createError.value = "请选择源分支和目标分支";
    return;
  }
  if (createSource.value === createTarget.value) {
    createError.value = "源分支与目标分支不能相同";
    return;
  }
  if (!createTitle.value.trim()) {
    createError.value = "请填写合并请求标题";
    return;
  }
  createError.value = "";
  const created = await createCurrentMR({
    sourceBranch: createSource.value,
    targetBranch: createTarget.value,
    title: createTitle.value.trim(),
    description: createDescription.value.trim(),
    removeSourceBranch: createRemoveSource.value,
  });
  if (created) {
    createOpen.value = false;
  } else if (actionError.value) {
    createError.value = actionError.value;
  }
}
</script>

<template>
  <div class="workspace mr-workspace">
    <div v-if="!session" class="workspace-empty">
      <div class="empty-panel">
        <div class="empty-art" aria-hidden="true">
          <DeltaMark seed="merge-requests-empty" animated />
        </div>
        <h2>请先选择一个项目</h2>
        <p>
          打开仓库后，合并请求会从 origin 远端识别 GitLab 项目，并使用设置中的实例与系统钥匙串凭据访问。
        </p>
        <button type="button" class="button primary" @click="navigate('projects')">
          返回项目列表
        </button>
      </div>
    </div>

    <template v-else>
      <div class="workspace-bar">
        <div class="workspace-heading">
          <h2>合并请求</h2>
          <p v-if="project">{{ project.projectPath }}</p>
          <p v-else-if="projectLoading">正在匹配 GitLab 项目…</p>
          <p v-else>从仓库的 origin 远端识别 GitLab 项目</p>
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
          <button type="button" class="button primary small" :disabled="!project" @click="openCreate">
            <Icon name="plus" :size="16" />
            新建合并请求
          </button>
        </div>
      </div>

      <p v-if="projectError && project" class="alert error" role="alert">{{ projectError }}</p>

      <div v-if="project" class="mr-layout">
        <section class="surface mr-list" aria-label="合并请求列表">
          <div class="mr-list-head">
            <div class="segmented" role="tablist" aria-label="合并请求状态">
              <button
                v-for="tab in stateTabs"
                :key="tab.key"
                type="button"
                class="segment"
                :class="{ active: filter === tab.key }"
                role="tab"
                :aria-selected="filter === tab.key"
                @click="setFilter(tab.key)"
              >
                {{ tab.label }}
              </button>
            </div>
            <label class="search-field compact">
              <Icon name="search" :size="15" />
              <input v-model="searchInput" type="search" placeholder="搜索标题或描述" />
            </label>
          </div>

          <div class="mr-scroll">
            <button
              v-for="item in items"
              :key="item.iid"
              type="button"
              class="mr-row"
              :class="{ selected: selectedIid === item.iid }"
              @click="select(item.iid)"
            >
              <span class="mr-row-top">
                <span class="state-tag" :class="`state-${item.state}`">
                  <span class="state-dot"></span>
                  {{ stateText(item.state) }}
                </span>
                <span class="mr-iid">!{{ item.iid }}</span>
                <span v-if="item.draft" class="tag">草稿</span>
                <span class="mr-row-time">{{ relative(item.updatedAt) }}</span>
              </span>
              <span class="mr-title">{{ item.title }}</span>
              <span class="mr-row-meta">
                <span class="mr-branch" :title="`${item.sourceBranch} → ${item.targetBranch}`">
                  <Icon name="branch" :size="12" />
                  {{ item.sourceBranch }}
                  <Icon name="arrow-right" :size="11" />
                  {{ item.targetBranch }}
                </span>
                <span class="mr-author">{{ item.authorName }}</span>
                <span v-if="item.userNotesCount" class="mr-note-count">
                  <Icon name="message" :size="12" />
                  {{ item.userNotesCount }}
                </span>
              </span>
            </button>

            <div v-if="listLoading" class="list-state">正在读取合并请求…</div>
            <div v-else-if="!items.length" class="list-state">
              {{ listError ? "读取失败" : "没有符合条件的合并请求" }}
            </div>
            <p v-if="listError" class="alert error mr-list-error">{{ listError }}</p>

            <div v-if="hasMore" class="load-more">
              <button
                type="button"
                class="button ghost small"
                :disabled="listLoadingMore"
                @click="loadList(false)"
              >
                {{ listLoadingMore ? "加载中…" : "加载更多" }}
              </button>
            </div>
          </div>
        </section>

        <section class="surface mr-detail" aria-label="合并请求详情">
          <div v-if="!selectedIid" class="inspector-empty">
            <div class="inspector-mark" aria-hidden="true">
              <DeltaMark seed="select-merge-request" />
            </div>
            <p>选一个合并请求</p>
            <span>描述、变更和讨论会在这里展开。</span>
          </div>

          <div v-else class="mr-detail-scroll">
            <template v-if="detail">
              <header class="mr-detail-head">
                <div class="mr-detail-badges">
                  <span class="state-tag" :class="`state-${detail.state}`">
                    <span class="state-dot"></span>
                    {{ stateText(detail.state) }}
                  </span>
                  <span class="mr-iid">!{{ detail.iid }}</span>
                  <span v-if="detail.draft" class="tag">草稿</span>
                  <span v-if="currentMergeHint" class="tag" :class="`tag-${currentMergeHint.kind}`">
                    {{ currentMergeHint.text }}
                  </span>
                </div>

                <h3>{{ detail.title }}</h3>

                <div class="mr-route">
                  <span class="branch-chip">
                    <Icon name="branch" :size="12" />
                    {{ detail.sourceBranch }}
                  </span>
                  <Icon name="arrow-right" :size="14" />
                  <span class="branch-chip">
                    <Icon name="branch" :size="12" />
                    {{ detail.targetBranch }}
                  </span>
                  <span class="mr-byline">
                    {{ detail.authorName }} 提交于 {{ relative(detail.createdAt) }}
                  </span>
                </div>

                <div class="mr-actions">
                  <button
                    v-if="detail.state === 'opened'"
                    type="button"
                    class="button primary small"
                    :disabled="!canMerge || actionBusy !== ''"
                    @click="mergeConfirm = !mergeConfirm"
                  >
                    <Icon name="git-merge" :size="15" />
                    合并
                  </button>
                  <button
                    v-if="detail.state === 'opened'"
                    type="button"
                    class="button danger-quiet small"
                    :disabled="actionBusy !== ''"
                    @click="toggleState"
                  >
                    {{ actionBusy === "close" ? "关闭中…" : "关闭" }}
                  </button>
                  <button
                    v-if="detail.state === 'closed'"
                    type="button"
                    class="button ghost small"
                    :disabled="actionBusy !== ''"
                    @click="toggleState"
                  >
                    {{ actionBusy === "reopen" ? "打开中…" : "重新打开" }}
                  </button>
                  <button type="button" class="button ghost small" @click="openInGitLab">
                    <Icon name="external" :size="14" />
                    在 GitLab 打开
                  </button>
                  <button
                    type="button"
                    class="button ghost small"
                    :disabled="detailLoading"
                    @click="refreshDetail"
                  >
                    <Icon name="refresh" :size="14" />
                    刷新
                  </button>
                </div>
              </header>

              <div v-if="mergeConfirm" class="merge-dock">
                <div class="merge-dock-head">
                  <strong>合并这个请求</strong>
                  <span>合并前确认以下选项。</span>
                </div>
                <div class="merge-options">
                  <label><input v-model="squash" type="checkbox" /> 压缩提交（squash）</label>
                  <label><input v-model="removeSourceBranch" type="checkbox" /> 合并后删除源分支</label>
                </div>
                <div class="merge-dock-actions">
                  <button
                    type="button"
                    class="button primary small"
                    :disabled="actionBusy !== ''"
                    @click="confirmMerge"
                  >
                    {{ actionBusy === "merge" ? "合并中…" : "确认合并" }}
                  </button>
                  <button type="button" class="button ghost small" @click="mergeConfirm = false">
                    取消
                  </button>
                </div>
              </div>

              <p v-if="actionError" class="alert error" role="alert">{{ actionError }}</p>
              <p v-if="detailError" class="alert error" role="alert">{{ detailError }}</p>

              <div v-if="detail.labels.length" class="chips">
                <span v-for="label in detail.labels" :key="label" class="chip">{{ label }}</span>
              </div>

              <section class="mr-section">
                <span class="section-label">描述</span>
                <pre v-if="detail.description.trim()" class="mr-description">{{ detail.description }}</pre>
                <p v-else class="muted-copy">没有填写描述。</p>
              </section>

              <div class="tabs" role="tablist" aria-label="合并请求内容">
                <button
                  type="button"
                  class="tab"
                  :class="{ active: detailTab === 'changes' }"
                  role="tab"
                  :aria-selected="detailTab === 'changes'"
                  @click="detailTab = 'changes'"
                >
                  文件变更 <span class="tab-count">{{ changes?.files.length ?? 0 }}</span>
                </button>
                <button
                  type="button"
                  class="tab"
                  :class="{ active: detailTab === 'notes' }"
                  role="tab"
                  :aria-selected="detailTab === 'notes'"
                  @click="detailTab = 'notes'"
                >
                  评论 <span class="tab-count">{{ notes.length }}</span>
                </button>
              </div>

              <div v-show="detailTab === 'changes'" class="mr-changes">
                <div v-if="detailLoading && !changes" class="list-state">正在读取文件变更…</div>
                <p v-if="changes?.overflow" class="alert info">
                  变更文件过多，仅显示前 300 个。
                </p>
                <div v-for="file in changes?.files" :key="fileKey(file)" class="diff-file">
                  <button type="button" class="diff-file-head" @click="toggleFile(file)">
                    <Icon
                      name="chevron-down"
                      :size="14"
                      class="diff-chevron"
                      :class="{ expanded: expanded(file) }"
                    />
                    <span class="file-status" :class="statusClass(file)">{{ fileStatus(file) }}</span>
                    <span class="file-path" :title="filePath(file)">{{ filePath(file) }}</span>
                    <span class="file-stats">
                      <span class="stat-add">+{{ fileStats(file).additions }}</span>
                      <span class="stat-del">−{{ fileStats(file).deletions }}</span>
                    </span>
                  </button>
                  <div v-if="expanded(file)" class="diff-body">
                    <div v-if="file.tooLarge" class="file-empty">该文件变更过大，请前往 GitLab 查看。</div>
                    <div v-else-if="!file.diff.trim()" class="file-empty">没有文本差异</div>
                    <pre v-else class="diff-pre"><span
                      v-for="(line, index) in diffLines(file)"
                      :key="index"
                      class="diff-line"
                      :class="`diff-${line.kind}`"
                    >{{ line.text || " " }}</span></pre>
                  </div>
                </div>
                <div v-if="changes && !changes.files.length && !detailLoading" class="list-state">
                  这个合并请求没有文件变更
                </div>
              </div>

              <div v-show="detailTab === 'notes'" class="mr-notes">
                <div v-if="detailLoading && !notes.length" class="list-state">正在读取评论…</div>
                <article v-for="note in notes" :key="note.id" class="note" :class="{ system: note.system }">
                  <span class="avatar small">{{ initial(note.authorName) }}</span>
                  <div class="note-main">
                    <div class="note-head">
                      <strong>{{ note.authorName }}</strong>
                      <span v-if="note.authorUsername" class="note-handle">@{{ note.authorUsername }}</span>
                      <span>{{ relative(note.createdAt) }}</span>
                      <span v-if="note.system" class="tag">系统</span>
                    </div>
                    <pre class="note-body">{{ note.body }}</pre>
                  </div>
                </article>
                <div v-if="!notes.length && !detailLoading" class="list-state">还没有评论</div>
              </div>
            </template>

            <div v-else-if="detailLoading" class="inspector-empty">
              <span class="loading-orbit" aria-hidden="true"></span>
              <p>正在读取合并请求…</p>
            </div>
          </div>
        </section>
      </div>

      <div v-else class="workspace-empty">
        <div class="empty-panel">
          <div class="empty-art" aria-hidden="true">
            <DeltaMark :seed="remoteInfo?.path || 'gitlab-project'" animated />
          </div>
          <h2>{{ projectLoading ? "正在匹配 GitLab 项目…" : "还不能读取合并请求" }}</h2>
          <p>{{ projectError || "请先配置 GitLab 实例，并确保仓库具有 origin 远端。" }}</p>
          <div class="page-actions">
            <button
              v-if="!projectLoading && instances.length && remoteInfo?.remoteUrl"
              type="button"
              class="button primary"
              @click="refreshAll"
            >
              重试
            </button>
            <button type="button" class="button ghost" @click="navigate('settings')">
              打开设置
            </button>
          </div>
        </div>
      </div>
    </template>

    <Transition name="sheet">
      <div v-if="createOpen" class="sheet-backdrop" @click.self="closeCreate">
        <aside class="sheet" role="dialog" aria-modal="true" aria-labelledby="create-mr-title">
          <header class="sheet-head">
            <div>
              <h3 id="create-mr-title">描述这次改动</h3>
              <p>{{ project?.projectPath }}</p>
            </div>
            <button type="button" class="icon-button" aria-label="关闭" @click="closeCreate">
              <Icon name="close" :size="18" />
            </button>
          </header>

          <div class="sheet-body">
            <div class="form-grid">
              <div class="field">
                <label>源分支</label>
                <select v-model="createSource" :disabled="creating || branchesLoading">
                  <option value="" disabled>选择源分支</option>
                  <option v-for="branch in branches" :key="branch.name" :value="branch.name">
                    {{ branch.name }}{{ branch.protected ? "（受保护）" : "" }}
                  </option>
                </select>
              </div>
              <div class="field">
                <label>目标分支</label>
                <select v-model="createTarget" :disabled="creating || branchesLoading">
                  <option value="" disabled>选择目标分支</option>
                  <option v-for="branch in branches" :key="branch.name" :value="branch.name">
                    {{ branch.name }}{{ branch.isDefault ? "（默认）" : "" }}
                  </option>
                </select>
              </div>
            </div>

            <div class="field">
              <label>标题</label>
              <input v-model="createTitle" :disabled="creating" placeholder="合并请求标题" />
            </div>

            <div class="field">
              <label>描述（可选）</label>
              <textarea
                v-model="createDescription"
                :disabled="creating"
                rows="6"
                placeholder="说明这次变更的内容、动机与验证方式"
              ></textarea>
            </div>

            <label class="check-row">
              <input v-model="createRemoveSource" type="checkbox" :disabled="creating" />
              合并后删除源分支
            </label>

            <p v-if="branchesLoading" class="muted-copy">正在读取远端分支…</p>
            <p v-if="createError || actionError" class="alert error" role="alert">
              {{ createError || actionError }}
            </p>
          </div>

          <footer class="sheet-foot">
            <button type="button" class="button ghost" :disabled="creating" @click="closeCreate">
              取消
            </button>
            <button type="button" class="button primary" :disabled="creating" @click="submitCreate">
              {{ creating ? "创建中…" : "创建合并请求" }}
            </button>
          </footer>
        </aside>
      </div>
    </Transition>
  </div>
</template>
