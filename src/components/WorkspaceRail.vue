<script setup lang="ts">
import { computed, ref } from "vue";
import Icon from "./Icon.vue";
import type { StashEntry, WorkingFile, WorkingStage } from "../types";

const props = defineProps<{
  files: WorkingFile[];
  stashes: StashEntry[];
  selected: { path: string; stage: WorkingStage } | null;
  truncated: boolean;
  loading?: boolean;
  disabled?: boolean;
}>();

const emit = defineEmits<{
  (e: "select", file: WorkingFile): void;
  (e: "toggle", file: WorkingFile): void;
  (e: "discard", file: WorkingFile): void;
  (e: "stage-all"): void;
  (e: "unstage-all"): void;
  (e: "stash-new"): void;
  (e: "stash-apply", entry: StashEntry): void;
  (e: "stash-pop", entry: StashEntry): void;
  (e: "stash-drop", entry: StashEntry): void;
  (e: "refresh"): void;
  (e: "close"): void;
}>();

const query = ref("");
const stashOpen = ref(true);

const STATUS_GLYPH: Record<string, string> = {
  added: "A",
  deleted: "D",
  modified: "M",
  renamed: "R",
  copied: "C",
  typechange: "T",
  conflicted: "U",
};

const needle = computed(() => query.value.trim().toLowerCase());

function matches(file: WorkingFile): boolean {
  if (!needle.value) return true;
  return (
    file.path.toLowerCase().includes(needle.value) ||
    (file.oldPath?.toLowerCase().includes(needle.value) ?? false)
  );
}

function group(stage: WorkingStage): WorkingFile[] {
  return props.files.filter((file) => file.stage === stage && matches(file));
}

const conflicted = computed(() => group("conflicted"));
const staged = computed(() => group("staged"));
const unstaged = computed(() => group("unstaged"));
const untracked = computed(() => group("untracked"));
const visible = computed(
  () => conflicted.value.length + staged.value.length + unstaged.value.length + untracked.value.length,
);

function isSelected(file: WorkingFile): boolean {
  return props.selected?.path === file.path && props.selected?.stage === file.stage;
}

function rowKey(file: WorkingFile): string {
  return `${file.stage}:${file.path}`;
}

function dirOf(path: string): string {
  const index = path.lastIndexOf("/");
  return index === -1 ? "" : path.slice(0, index + 1);
}

function baseOf(path: string): string {
  const index = path.lastIndexOf("/");
  return index === -1 ? path : path.slice(index + 1);
}

function fullLabel(file: WorkingFile): string {
  return file.oldPath ? `${file.oldPath} → ${file.path}` : file.path;
}

function stageTitle(file: WorkingFile): string {
  if (file.stage === "staged") return "取消暂存整个文件";
  if (file.stage === "conflicted") return "标记为已解决（暂存该文件）";
  return "暂存整个文件";
}

function discardTitle(file: WorkingFile): string {
  if (file.stage === "untracked") return "删除这个未跟踪文件";
  if (file.stage === "staged") return "取消暂存并丢弃工作区改动";
  return "丢弃工作区改动";
}

function stashIndex(entry: StashEntry): string {
  return entry.reference.replace(/^stash@\{/, "").replace(/\}$/, "");
}

function stashBaseMessage(message: string): string {
  const colon = message.indexOf(": ");
  return colon === -1 ? message : message.slice(colon + 2);
}
</script>

<template>
  <aside class="surface branch-rail workspace-rail" aria-label="工作区变更">
    <div class="rail-head">
      <div class="rail-heading">
        <span class="section-label">变更</span>
        <span class="rail-count">{{ files.length }}</span>
      </div>
      <div class="rail-head-actions">
        <button
          type="button"
          class="icon-button rail-icon"
          title="刷新工作区"
          aria-label="刷新工作区"
          :disabled="disabled"
          @click="$emit('refresh')"
        >
          <Icon name="refresh" :size="14" />
        </button>
        <button
          type="button"
          class="icon-button rail-icon"
          title="贮藏当前变更"
          aria-label="贮藏当前变更"
          :disabled="disabled || !files.length"
          @click="$emit('stash-new')"
        >
          <Icon name="stash" :size="15" />
        </button>
        <button
          type="button"
          class="icon-button rail-icon rail-close"
          title="收起变更栏"
          aria-label="收起变更栏"
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
        placeholder="筛选变更文件"
        aria-label="筛选变更文件"
      />
    </label>

    <div class="rail-scroll">
      <section v-if="conflicted.length" class="rail-section">
        <div class="rail-section-head">
          <span class="ws-group-title danger">冲突</span>
          <span class="ws-group-count">{{ conflicted.length }}</span>
        </div>
        <div
          v-for="file in conflicted"
          :key="rowKey(file)"
          class="rail-row ws-row"
          :class="{ active: isSelected(file) }"
        >
          <button
            type="button"
            class="rail-item"
            :title="file.path"
            @click="$emit('select', file)"
          >
            <span class="file-status status-conflicted">{{ STATUS_GLYPH.conflicted }}</span>
            <span class="ws-path">
              <span v-if="dirOf(file.path)" class="ws-dir">{{ dirOf(file.path) }}</span>
              <span class="ws-base">{{ baseOf(file.path) }}</span>
            </span>
          </button>
          <button
            type="button"
            class="rail-more"
            :title="stageTitle(file)"
            :disabled="disabled"
            @click="$emit('toggle', file)"
          >
            <Icon name="check" :size="14" />
          </button>
        </div>
        <p class="ws-group-note">在编辑器里解决后点勾，标记为已解决。</p>
      </section>

      <section v-if="staged.length" class="rail-section">
        <div class="rail-section-head">
          <span class="ws-group-title">已暂存</span>
          <span class="ws-group-right">
            <span class="ws-group-count">{{ staged.length }}</span>
            <button
              type="button"
              class="icon-button ws-group-action"
              title="取消暂存全部"
              aria-label="取消暂存全部"
              :disabled="disabled"
              @click="$emit('unstage-all')"
            >
              <Icon name="minus" :size="14" />
            </button>
          </span>
        </div>
        <div
          v-for="file in staged"
          :key="rowKey(file)"
          class="rail-row ws-row"
          :class="{ active: isSelected(file) }"
        >
          <button
            type="button"
            class="rail-item"
            :title="fullLabel(file)"
            @click="$emit('select', file)"
          >
            <span class="file-status" :class="`status-${file.status}`">
              {{ STATUS_GLYPH[file.status] ?? "M" }}
            </span>
            <span class="ws-path">
              <span v-if="dirOf(file.path)" class="ws-dir">{{ dirOf(file.path) }}</span>
              <span class="ws-base">{{ baseOf(file.path) }}</span>
            </span>
            <span class="file-stats">
              <span v-if="file.additions" class="stat-add">+{{ file.additions }}</span>
              <span v-if="file.deletions" class="stat-del">−{{ file.deletions }}</span>
            </span>
          </button>
          <button
            type="button"
            class="rail-more"
            title="取消暂存这个文件"
            aria-label="取消暂存这个文件"
            :disabled="disabled"
            @click="$emit('toggle', file)"
          >
            <Icon name="minus" :size="14" />
          </button>
          <button
            type="button"
            class="rail-more"
            :title="discardTitle(file)"
            aria-label="丢弃这个文件的更改"
            :disabled="disabled"
            @click="$emit('discard', file)"
          >
            <Icon name="trash" :size="14" />
          </button>
        </div>
      </section>

      <section v-if="unstaged.length" class="rail-section">
        <div class="rail-section-head">
          <span class="ws-group-title">未暂存</span>
          <span class="ws-group-right">
            <span class="ws-group-count">{{ unstaged.length }}</span>
            <button
              type="button"
              class="icon-button ws-group-action"
              title="暂存全部变更"
              aria-label="暂存全部变更"
              :disabled="disabled"
              @click="$emit('stage-all')"
            >
              <Icon name="plus" :size="14" />
            </button>
          </span>
        </div>
        <div
          v-for="file in unstaged"
          :key="rowKey(file)"
          class="rail-row ws-row"
          :class="{ active: isSelected(file) }"
        >
          <button
            type="button"
            class="rail-item"
            :title="fullLabel(file)"
            @click="$emit('select', file)"
          >
            <span class="file-status" :class="`status-${file.status}`">
              {{ STATUS_GLYPH[file.status] ?? "M" }}
            </span>
            <span class="ws-path">
              <span v-if="dirOf(file.path)" class="ws-dir">{{ dirOf(file.path) }}</span>
              <span class="ws-base">{{ baseOf(file.path) }}</span>
            </span>
            <span class="file-stats">
              <span v-if="file.additions" class="stat-add">+{{ file.additions }}</span>
              <span v-if="file.deletions" class="stat-del">−{{ file.deletions }}</span>
            </span>
          </button>
          <button
            type="button"
            class="rail-more"
            title="暂存这个文件"
            aria-label="暂存这个文件"
            :disabled="disabled"
            @click="$emit('toggle', file)"
          >
            <Icon name="plus" :size="14" />
          </button>
          <button
            type="button"
            class="rail-more"
            :title="discardTitle(file)"
            aria-label="丢弃这个文件的更改"
            :disabled="disabled"
            @click="$emit('discard', file)"
          >
            <Icon name="trash" :size="14" />
          </button>
        </div>
      </section>

      <section v-if="untracked.length" class="rail-section">
        <div class="rail-section-head">
          <span class="ws-group-title muted">未跟踪</span>
          <span class="ws-group-count">{{ untracked.length }}</span>
        </div>
        <div
          v-for="file in untracked"
          :key="rowKey(file)"
          class="rail-row ws-row"
          :class="{ active: isSelected(file) }"
        >
          <button
            type="button"
            class="rail-item"
            :title="file.path"
            @click="$emit('select', file)"
          >
            <span class="file-status" :class="`status-${file.status}`">
              {{ STATUS_GLYPH[file.status] ?? "A" }}
            </span>
            <span class="ws-path">
              <span v-if="dirOf(file.path)" class="ws-dir">{{ dirOf(file.path) }}</span>
              <span class="ws-base">{{ baseOf(file.path) }}</span>
            </span>
            <span class="file-stats">
              <span v-if="file.additions" class="stat-add">+{{ file.additions }}</span>
            </span>
          </button>
          <button
            type="button"
            class="rail-more"
            title="暂存这个文件"
            aria-label="暂存这个文件"
            :disabled="disabled"
            @click="$emit('toggle', file)"
          >
            <Icon name="plus" :size="14" />
          </button>
          <button
            type="button"
            class="rail-more"
            title="删除这个未跟踪文件"
            aria-label="删除这个未跟踪文件"
            :disabled="disabled"
            @click="$emit('discard', file)"
          >
            <Icon name="trash" :size="14" />
          </button>
        </div>
      </section>

      <p v-if="truncated" class="alert info ws-truncated">
        变更文件很多，这里只显示前 400 个。
      </p>
      <p v-else-if="!files.length && loading" class="rail-empty">正在读取工作区…</p>
      <p v-else-if="!files.length" class="rail-empty ws-clean">
        <Icon name="check" :size="14" />
        工作区很干净，没有待提交的变更
      </p>
      <p v-else-if="!visible" class="rail-empty">
        没有匹配「{{ query.trim() }}」的变更文件
      </p>

      <section v-if="stashes.length" class="rail-section">
        <button
          type="button"
          class="rail-section-head stash-head"
          :aria-expanded="stashOpen"
          @click="stashOpen = !stashOpen"
        >
          <span class="ws-group-title">贮藏</span>
          <span class="ws-group-right">
            <span class="ws-group-count">{{ stashes.length }}</span>
            <Icon
              name="chevron-down"
              :size="13"
              class="stash-chevron"
              :class="{ expanded: stashOpen }"
            />
          </span>
        </button>
        <div v-show="stashOpen" class="stash-list">
          <div v-for="entry in stashes" :key="entry.reference" class="rail-row stash-row">
            <button
              type="button"
              class="rail-item"
              :title="`${entry.reference} · ${entry.date}`"
              @click="$emit('stash-apply', entry)"
            >
              <span class="stash-ref">{{ stashIndex(entry) }}</span>
              <span class="ws-path">
                <span class="ws-base">{{ stashBaseMessage(entry.message) }}</span>
              </span>
            </button>
            <button
              type="button"
              class="rail-more"
              title="弹出这个贮藏（应用并删除）"
              aria-label="弹出这个贮藏"
              :disabled="disabled"
              @click="$emit('stash-pop', entry)"
            >
              <Icon name="download" :size="14" />
            </button>
            <button
              type="button"
              class="rail-more"
              title="删除这个贮藏"
              aria-label="删除这个贮藏"
              :disabled="disabled"
              @click="$emit('stash-drop', entry)"
            >
              <Icon name="trash" :size="14" />
            </button>
          </div>
        </div>
      </section>
    </div>

    <p class="rail-hint">
      <Icon name="file" :size="13" />
      点文件看差异，⊕ 暂存整个文件，差异里还能只暂存一个代码块。
    </p>
  </aside>
</template>
