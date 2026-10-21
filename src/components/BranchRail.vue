<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Icon from "./Icon.vue";
import { setDragGhost } from "../lib/dragGhost";
import type { BranchInfo, BranchKind, RemoteInfo, RepoRefs, StashEntry, TagInfo } from "../types";

const props = defineProps<{
  refs: RepoRefs | null;
  selected: { name: string; kind: BranchKind } | null;
  stashes?: StashEntry[];
  selectedStash?: string | null;
  remotes?: RemoteInfo[];
  disabled?: boolean;
  dragKind?: "commit" | BranchKind | null;
}>();

const emit = defineEmits<{
  (e: "select-branch", branch: BranchInfo): void;
  (e: "select-tag", tag: TagInfo): void;
  (e: "select-stash", stash: StashEntry): void;
  (e: "stash-new"): void;
  (e: "stash-menu", payload: { x: number; y: number; stash: StashEntry }): void;
  (e: "remote-add"): void;
  (e: "remote-menu", payload: { x: number; y: number; remote: RemoteInfo }): void;
  (e: "menu", payload: { x: number; y: number; branch?: BranchInfo; tag?: TagInfo }): void;
  (e: "create"): void;
  (e: "fetch"): void;
  (e: "drag-start", branch: BranchInfo): void;
  (e: "drag-over", branch: BranchInfo | null): void;
  (e: "drag-end"): void;
  (e: "drop-branch", payload: { branch: BranchInfo; x: number; y: number }): void;
  (e: "checkout-branch", branch: BranchInfo): void;
  (e: "close"): void;
}>();

const query = ref("");
const dropKey = ref<string | null>(null);
const dragged = ref<BranchInfo | null>(null);

const needle = computed(() => query.value.trim().toLowerCase());

type RailSection = "remotes" | "local" | "remoteBranches" | "tags" | "stashes";

const RAIL_COLLAPSE_KEY = "easygit.rail-collapsed";

function readCollapsed(): Partial<Record<RailSection, boolean>> {
  try {
    const raw = localStorage.getItem(RAIL_COLLAPSE_KEY);
    const parsed = raw ? JSON.parse(raw) : null;
    return parsed && typeof parsed === "object" ? parsed : {};
  } catch {
    return {};
  }
}

const collapsed = ref<Partial<Record<RailSection, boolean>>>(readCollapsed());
const searchCollapsed = ref<Partial<Record<RailSection, boolean>>>({});

function isOpen(section: RailSection): boolean {
  return needle.value ? !searchCollapsed.value[section] : !collapsed.value[section];
}

function toggleSection(section: RailSection) {
  if (needle.value) {
    searchCollapsed.value = {
      ...searchCollapsed.value,
      [section]: !searchCollapsed.value[section],
    };
    return;
  }
  collapsed.value = {
    ...collapsed.value,
    [section]: !collapsed.value[section],
  };
  try {
    localStorage.setItem(RAIL_COLLAPSE_KEY, JSON.stringify(collapsed.value));
  } catch {
    // storage unavailable; keep the in-memory state
  }
}

watch(needle, (value) => {
  if (value) searchCollapsed.value = {};
});

function matches(name: string): boolean {
  return !needle.value || name.toLowerCase().includes(needle.value);
}

const localBranches = computed(() =>
  (props.refs?.local ?? []).filter((branch) => matches(branch.name)),
);
const remoteBranches = computed(() =>
  (props.refs?.remote ?? []).filter((branch) => matches(branch.name)),
);
const tags = computed(() => (props.refs?.tags ?? []).filter((tag) => matches(tag.name)));
const remoteList = computed(() =>
  (props.remotes ?? []).filter(
    (remote) => matches(remote.name) || matches(remote.fetchUrl),
  ),
);
const stashList = computed(() =>
  (props.stashes ?? []).filter((stash) => matches(stash.message)),
);
const total = computed(
  () => (props.refs?.local.length ?? 0) + (props.refs?.remote.length ?? 0) + (props.refs?.tags.length ?? 0),
);
const visibleTotal = computed(
  () =>
    localBranches.value.length +
    remoteBranches.value.length +
    tags.value.length +
    remoteList.value.length +
    stashList.value.length,
);

function keyOf(kind: BranchKind, name: string): string {
  return `${kind}:${name}`;
}

function isSelected(kind: BranchKind, name: string): boolean {
  return props.selected?.kind === kind && props.selected.name === name;
}

function syncTitle(branch: BranchInfo): string {
  const parts = [];
  if (branch.upstream) parts.push(`上游 ${branch.upstream}`);
  if (branch.ahead) parts.push(`领先 ${branch.ahead}`);
  if (branch.behind) parts.push(`落后 ${branch.behind}`);
  return parts.join(" · ");
}

function onDragStart(event: DragEvent, branch: BranchInfo) {
  if (props.disabled) {
    event.preventDefault();
    return;
  }
  dropKey.value = null;
  dragged.value = branch;
  setDragGhost(event, branch.name, branch.kind === "remote" ? "remote" : "branch");
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", `${branch.kind}:${branch.name}`);
  }
  emit("drag-start", branch);
}

function onDragEnd() {
  dragged.value = null;
  dropKey.value = null;
  emit("drag-over", null);
  emit("drag-end");
}

function onDragOver(event: DragEvent, branch: BranchInfo) {
  if (props.disabled) return;
  const kind = dragged.value?.kind ?? props.dragKind ?? null;
  const name = dragged.value?.name ?? null;
  if (kind === "commit" && branch.kind !== "local") {
    dropKey.value = null;
    emit("drag-over", null);
    return;
  }
  if (kind === branch.kind && name === branch.name) {
    dropKey.value = null;
    emit("drag-over", null);
    return;
  }
  event.preventDefault();
  if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
  dropKey.value = keyOf(branch.kind, branch.name);
  emit("drag-over", branch);
}

function onDragLeave(branch: BranchInfo) {
  if (dropKey.value === keyOf(branch.kind, branch.name)) {
    dropKey.value = null;
    emit("drag-over", null);
  }
}

function onDrop(event: DragEvent, branch: BranchInfo) {
  event.preventDefault();
  dropKey.value = null;
  emit("drag-over", null);
  if (props.disabled) return;
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  emit("drop-branch", {
    branch,
    x: event.clientX || rect.left + rect.width / 2,
    y: event.clientY || rect.top + rect.height / 2,
  });
}

function openMenu(event: MouseEvent, payload: { branch?: BranchInfo; tag?: TagInfo }) {
  event.preventDefault();
  event.stopPropagation();
  emit("menu", { x: event.clientX, y: event.clientY, ...payload });
}

function openMore(event: MouseEvent, payload: { branch?: BranchInfo; tag?: TagInfo }) {
  event.preventDefault();
  event.stopPropagation();
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  emit("menu", { x: rect.right - 4, y: rect.bottom + 4, ...payload });
}

function openStashMore(event: MouseEvent, stash: StashEntry) {
  event.preventDefault();
  event.stopPropagation();
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  emit("stash-menu", { x: rect.right - 4, y: rect.bottom + 4, stash });
}

function openRemoteMenu(event: MouseEvent, remote: RemoteInfo) {
  event.preventDefault();
  event.stopPropagation();
  emit("remote-menu", { x: event.clientX, y: event.clientY, remote });
}

function openRemoteMore(event: MouseEvent, remote: RemoteInfo) {
  event.preventDefault();
  event.stopPropagation();
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  emit("remote-menu", { x: rect.right - 4, y: rect.bottom + 4, remote });
}
</script>

<template>
  <aside class="surface branch-rail" aria-label="分支与标签">
    <div class="rail-head">
      <div class="rail-heading">
        <span class="section-label">分支</span>
        <span class="rail-count">{{ total }}</span>
      </div>
      <div class="rail-head-actions">
        <button
          type="button"
          class="button ghost tiny rail-action"
          title="获取所有远程更新"
          :disabled="disabled"
          @click="$emit('fetch')"
        >
          <Icon name="refresh" :size="13" />
          刷新
        </button>
        <button
          type="button"
          class="button ghost tiny rail-action"
          title="新建分支"
          :disabled="disabled"
          @click="$emit('create')"
        >
          <Icon name="plus" :size="13" />
          新建
        </button>
        <button
          type="button"
          class="button ghost tiny rail-action rail-close"
          title="收起分支栏"
          @click="$emit('close')"
        >
          <Icon name="close" :size="13" />
          收起
        </button>
      </div>
    </div>

    <label class="rail-search">
      <Icon name="search" :size="14" />
      <input
        v-model="query"
        type="search"
        placeholder="筛选分支与标签"
        aria-label="筛选分支与标签"
      />
    </label>

    <div class="rail-scroll">
      <section v-if="remoteList.length || !needle" class="rail-section rail-remotes">
        <div class="rail-section-head">
          <button
            type="button"
            class="rail-section-toggle"
            :aria-expanded="isOpen('remotes')"
            aria-controls="rail-section-remotes"
            @click="toggleSection('remotes')"
          >
            <Icon
              class="rail-chevron"
              :name="isOpen('remotes') ? 'chevron-down' : 'chevron-right'"
              :size="11"
            />
            <span>远程仓库</span>
          </button>
          <span class="rail-section-actions">
            <span>{{ remoteList.length }}</span>
            <button
              type="button"
              class="rail-add"
              title="添加远程仓库"
              :disabled="disabled"
              @click="$emit('remote-add')"
            >
              <Icon name="plus" :size="12" />
              新建
            </button>
          </span>
        </div>
        <div v-show="isOpen('remotes')" id="rail-section-remotes" class="rail-rows">
          <div
            v-for="remote in remoteList"
            :key="`remote:${remote.name}`"
            class="rail-row remote-host"
          >
            <button
              type="button"
              class="rail-item"
              :title="`${remote.name}\n${remote.fetchUrl || '未设置地址'}\n点击管理远程`"
              @click="openRemoteMenu($event, remote)"
              @contextmenu="openRemoteMenu($event, remote)"
            >
              <span class="rail-remote-icon" aria-hidden="true">
                <Icon name="cloud" :size="13" />
              </span>
              <span class="rail-remote-copy">
                <span class="rail-remote-name">
                  {{ remote.name }}
                  <span v-if="remote.hasCredentials" class="rail-lock" title="地址包含已隐藏的凭据">
                    <Icon name="lock" :size="10" />
                  </span>
                </span>
                <span class="rail-url">{{ remote.fetchUrl || "未设置地址" }}</span>
              </span>
            </button>
            <button
              type="button"
              class="rail-more"
              title="远程操作"
              @click="openRemoteMore($event, remote)"
            >
              <Icon name="more" :size="12" />
              管理
            </button>
          </div>
          <p v-if="!remoteList.length" class="rail-empty rail-empty-inline">
            还没有配置远程仓库
          </p>
        </div>
      </section>

      <section v-if="localBranches.length" class="rail-section">
        <div class="rail-section-head">
          <button
            type="button"
            class="rail-section-toggle"
            :aria-expanded="isOpen('local')"
            aria-controls="rail-section-local"
            @click="toggleSection('local')"
          >
            <Icon
              class="rail-chevron"
              :name="isOpen('local') ? 'chevron-down' : 'chevron-right'"
              :size="11"
            />
            <span>本地</span>
          </button>
          <span>{{ localBranches.length }}</span>
        </div>
        <div v-show="isOpen('local')" id="rail-section-local" class="rail-rows">
          <div
            v-for="branch in localBranches"
            :key="keyOf(branch.kind, branch.name)"
            class="rail-row"
            :class="{
              current: branch.isCurrent,
              active: isSelected(branch.kind, branch.name),
              drop: dropKey === keyOf(branch.kind, branch.name),
            }"
            @dragover="onDragOver($event, branch)"
            @dragleave="onDragLeave(branch)"
            @drop="onDrop($event, branch)"
          >
            <button
              type="button"
              class="rail-item"
              :draggable="!disabled"
              :title="branch.name"
              :aria-current="branch.isCurrent ? 'true' : undefined"
              @click="$emit('select-branch', branch)"
              @dblclick="$emit('checkout-branch', branch)"
              @contextmenu="openMenu($event, { branch })"
              @dragstart="onDragStart($event, branch)"
              @dragend="onDragEnd"
            >
              <span class="rail-dot" :class="{ current: branch.isCurrent }" aria-hidden="true"></span>
              <span class="rail-name">{{ branch.name }}</span>
              <span v-if="branch.isCurrent" class="rail-badge">当前</span>
              <span
                v-else-if="branch.ahead || branch.behind"
                class="rail-sync"
                :title="syncTitle(branch)"
              >
                <span v-if="branch.ahead">↑{{ branch.ahead }}</span>
                <span v-if="branch.behind">↓{{ branch.behind }}</span>
              </span>
            </button>
            <button
              type="button"
              class="rail-more"
              title="分支操作"
              @click="openMore($event, { branch })"
            >
              <Icon name="more" :size="12" />
              更多
            </button>
          </div>
        </div>
      </section>

      <section v-if="remoteBranches.length" class="rail-section">
        <div class="rail-section-head">
          <button
            type="button"
            class="rail-section-toggle"
            :aria-expanded="isOpen('remoteBranches')"
            aria-controls="rail-section-remote-branches"
            @click="toggleSection('remoteBranches')"
          >
            <Icon
              class="rail-chevron"
              :name="isOpen('remoteBranches') ? 'chevron-down' : 'chevron-right'"
              :size="11"
            />
            <span>远程分支</span>
          </button>
          <span>{{ remoteBranches.length }}</span>
        </div>
        <div v-show="isOpen('remoteBranches')" id="rail-section-remote-branches" class="rail-rows">
          <div
            v-for="branch in remoteBranches"
            :key="keyOf(branch.kind, branch.name)"
            class="rail-row remote"
            :class="{
              active: isSelected(branch.kind, branch.name),
              drop: dropKey === keyOf(branch.kind, branch.name),
            }"
            @dragover="onDragOver($event, branch)"
            @dragleave="onDragLeave(branch)"
            @drop="onDrop($event, branch)"
          >
            <button
              type="button"
              class="rail-item"
              :draggable="!disabled"
              :title="branch.name"
              @click="$emit('select-branch', branch)"
              @dblclick="$emit('checkout-branch', branch)"
              @contextmenu="openMenu($event, { branch })"
              @dragstart="onDragStart($event, branch)"
              @dragend="onDragEnd"
            >
              <span class="rail-dot remote" aria-hidden="true"></span>
              <span class="rail-name">{{ branch.name }}</span>
            </button>
            <button
              type="button"
              class="rail-more"
              title="远程分支操作"
              @click="openMore($event, { branch })"
            >
              <Icon name="more" :size="12" />
              更多
            </button>
          </div>
        </div>
      </section>

      <section v-if="tags.length" class="rail-section">
        <div class="rail-section-head">
          <button
            type="button"
            class="rail-section-toggle"
            :aria-expanded="isOpen('tags')"
            aria-controls="rail-section-tags"
            @click="toggleSection('tags')"
          >
            <Icon
              class="rail-chevron"
              :name="isOpen('tags') ? 'chevron-down' : 'chevron-right'"
              :size="11"
            />
            <span>标签</span>
          </button>
          <span>{{ tags.length }}</span>
        </div>
        <div v-show="isOpen('tags')" id="rail-section-tags" class="rail-rows">
          <div
            v-for="tag in tags"
            :key="`tag:${tag.name}`"
            class="rail-row tag"
          >
            <button
              type="button"
              class="rail-item"
              :title="tag.name"
              @click="$emit('select-tag', tag)"
              @contextmenu="openMenu($event, { tag })"
            >
              <span class="rail-tag-icon" aria-hidden="true">
                <Icon name="tag" :size="12" />
              </span>
              <span class="rail-name">{{ tag.name }}</span>
            </button>
            <button
              type="button"
              class="rail-more"
              title="标签操作"
              @click="openMore($event, { tag })"
            >
              <Icon name="more" :size="12" />
              更多
            </button>
          </div>
        </div>
      </section>

      <section class="rail-section">
        <div class="rail-section-head">
          <button
            type="button"
            class="rail-section-toggle"
            :aria-expanded="isOpen('stashes')"
            aria-controls="rail-section-stashes"
            @click="toggleSection('stashes')"
          >
            <Icon
              class="rail-chevron"
              :name="isOpen('stashes') ? 'chevron-down' : 'chevron-right'"
              :size="11"
            />
            <span>贮藏</span>
          </button>
          <span class="rail-section-actions">
            <span>{{ stashList.length }}</span>
            <button
              type="button"
              class="rail-add"
              title="贮藏当前变更"
              :disabled="disabled"
              @click="$emit('stash-new')"
            >
              <Icon name="plus" :size="12" />
              新建
            </button>
          </span>
        </div>
        <div v-show="isOpen('stashes')" id="rail-section-stashes" class="rail-rows">
          <div
            v-for="stash in stashList"
            :key="stash.reference"
            class="rail-row stash"
            :class="{ active: selectedStash === stash.reference }"
          >
            <button
              type="button"
              class="rail-item"
              :title="`${stash.reference} · ${stash.message}`"
              @click="$emit('select-stash', stash)"
              @contextmenu="openStashMore($event, stash)"
            >
              <span class="rail-stash-icon" aria-hidden="true">
                <Icon name="stash" :size="12" />
              </span>
              <span class="rail-name">{{ stash.message }}</span>
            </button>
            <button
              type="button"
              class="rail-more"
              title="贮藏操作"
              @click="openStashMore($event, stash)"
            >
              <Icon name="more" :size="12" />
              更多
            </button>
          </div>
          <p v-if="!stashList.length" class="rail-empty rail-empty-inline">
            {{ query.trim() ? "没有匹配的贮藏" : "没有贮藏" }}
          </p>
        </div>
      </section>

      <p v-if="!visibleTotal && (total || remotes?.length)" class="rail-empty">
        没有匹配「{{ query.trim() }}」的分支、标签或远程
      </p>
      <p v-else-if="!total && !remotes?.length" class="rail-empty">这个仓库还没有分支或标签</p>
    </div>

    <p class="rail-hint">
      <Icon name="branch" :size="13" />
      拖拽分支到分支或提交上开始操作，也可以右键。
    </p>
  </aside>
</template>
