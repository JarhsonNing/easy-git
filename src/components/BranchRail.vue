<script setup lang="ts">
import { computed, ref } from "vue";
import Icon from "./Icon.vue";
import type { BranchInfo, BranchKind, RepoRefs, TagInfo } from "../types";

const props = defineProps<{
  refs: RepoRefs | null;
  selected: { name: string; kind: BranchKind } | null;
  disabled?: boolean;
}>();

const emit = defineEmits<{
  (e: "select-branch", branch: BranchInfo): void;
  (e: "select-tag", tag: TagInfo): void;
  (e: "menu", payload: { x: number; y: number; branch?: BranchInfo; tag?: TagInfo }): void;
  (e: "create"): void;
  (e: "fetch"): void;
  (e: "drag-start", branch: BranchInfo): void;
  (e: "drag-end"): void;
  (e: "drop-branch", branch: BranchInfo): void;
  (e: "close"): void;
}>();

const query = ref("");
const dropKey = ref<string | null>(null);

const needle = computed(() => query.value.trim().toLowerCase());

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
const total = computed(
  () => (props.refs?.local.length ?? 0) + (props.refs?.remote.length ?? 0) + (props.refs?.tags.length ?? 0),
);
const visibleTotal = computed(
  () => localBranches.value.length + remoteBranches.value.length + tags.value.length,
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
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", `${branch.kind}:${branch.name}`);
  }
  emit("drag-start", branch);
}

function onDragOver(event: DragEvent, branch: BranchInfo) {
  if (props.disabled || branch.kind !== "local") return;
  event.preventDefault();
  if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
  dropKey.value = keyOf(branch.kind, branch.name);
}

function onDragLeave(branch: BranchInfo) {
  if (dropKey.value === keyOf(branch.kind, branch.name)) {
    dropKey.value = null;
  }
}

function onDrop(event: DragEvent, branch: BranchInfo) {
  event.preventDefault();
  dropKey.value = null;
  if (props.disabled || branch.kind !== "local") return;
  emit("drop-branch", branch);
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
          class="icon-button rail-icon"
          title="获取所有远程更新"
          aria-label="获取所有远程更新"
          :disabled="disabled"
          @click="$emit('fetch')"
        >
          <Icon name="refresh" :size="14" />
        </button>
        <button
          type="button"
          class="icon-button rail-icon"
          title="新建分支"
          aria-label="新建分支"
          :disabled="disabled"
          @click="$emit('create')"
        >
          <Icon name="plus" :size="15" />
        </button>
        <button
          type="button"
          class="icon-button rail-icon rail-close"
          title="收起分支栏"
          aria-label="收起分支栏"
          @click="$emit('close')"
        >
          <Icon name="close" :size="14" />
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
      <section v-if="localBranches.length" class="rail-section">
        <div class="rail-section-head">
          <span>本地</span>
          <span>{{ localBranches.length }}</span>
        </div>
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
            @contextmenu="openMenu($event, { branch })"
            @dragstart="onDragStart($event, branch)"
            @dragend="$emit('drag-end')"
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
            aria-label="分支操作"
            @click="openMore($event, { branch })"
          >
            <Icon name="more" :size="14" />
          </button>
        </div>
      </section>

      <section v-if="remoteBranches.length" class="rail-section">
        <div class="rail-section-head">
          <span>远程</span>
          <span>{{ remoteBranches.length }}</span>
        </div>
        <div
          v-for="branch in remoteBranches"
          :key="keyOf(branch.kind, branch.name)"
          class="rail-row remote"
          :class="{ active: isSelected(branch.kind, branch.name) }"
        >
          <button
            type="button"
            class="rail-item"
            :draggable="!disabled"
            :title="branch.name"
            @click="$emit('select-branch', branch)"
            @contextmenu="openMenu($event, { branch })"
            @dragstart="onDragStart($event, branch)"
            @dragend="$emit('drag-end')"
          >
            <span class="rail-dot remote" aria-hidden="true"></span>
            <span class="rail-name">{{ branch.name }}</span>
          </button>
          <button
            type="button"
            class="rail-more"
            title="远程分支操作"
            aria-label="远程分支操作"
            @click="openMore($event, { branch })"
          >
            <Icon name="more" :size="14" />
          </button>
        </div>
      </section>

      <section v-if="tags.length" class="rail-section">
        <div class="rail-section-head">
          <span>标签</span>
          <span>{{ tags.length }}</span>
        </div>
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
            aria-label="标签操作"
            @click="openMore($event, { tag })"
          >
            <Icon name="more" :size="14" />
          </button>
        </div>
      </section>

      <p v-if="!visibleTotal && total" class="rail-empty">
        没有匹配「{{ query.trim() }}」的分支或标签
      </p>
      <p v-else-if="!total" class="rail-empty">这个仓库还没有分支或标签</p>
    </div>

    <p class="rail-hint">
      <Icon name="branch" :size="13" />
      拖拽分支到分支或提交上开始操作，也可以右键。
    </p>
  </aside>
</template>
