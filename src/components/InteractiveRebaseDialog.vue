<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import Icon from "./Icon.vue";
import type { RebaseAction, RebaseCommit, RebasePlan, RebaseStep } from "../types";

interface RebaseRow extends RebaseCommit {
  action: RebaseAction;
  draft: string;
}

const props = defineProps<{
  branch: string;
  baseLabel: string;
  plan: RebasePlan | null;
  loading: boolean;
  error?: string;
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "retry"): void;
  (e: "submit", steps: RebaseStep[]): void;
}>();

const ACTIONS: Array<{ value: RebaseAction; label: string }> = [
  { value: "pick", label: "保留" },
  { value: "reword", label: "改写说明" },
  { value: "squash", label: "压缩合并" },
  { value: "fixup", label: "并入上一条" },
  { value: "drop", label: "删除" },
];

const rows = ref<RebaseRow[]>([]);
const dragIndex = ref<number | null>(null);
const dropIndex = ref<number | null>(null);
const titleId = `rebase-title-${Math.random().toString(36).slice(2, 9)}`;

watch(
  () => props.plan,
  (plan) => {
    rows.value = (plan?.commits ?? []).map((commit) => ({
      ...commit,
      action: "pick",
      draft: commit.message,
    }));
    dragIndex.value = null;
    dropIndex.value = null;
  },
  { immediate: true },
);

function normalize() {
  let kept = false;
  for (const row of rows.value) {
    if (row.action === "drop") continue;
    if (!kept && (row.action === "squash" || row.action === "fixup")) {
      row.action = "pick";
    }
    kept = true;
  }
}

function canFold(row: RebaseRow): boolean {
  const index = rows.value.indexOf(row);
  return rows.value.slice(0, index).some((item) => item.action !== "drop");
}

function setAction(row: RebaseRow, event: Event) {
  const value = (event.target as HTMLSelectElement).value as RebaseAction;
  row.action = value;
  if (value === "reword" && !row.draft.trim()) row.draft = row.message;
  normalize();
}

function moveRow(from: number, to: number) {
  if (from === to || from < 0 || to < 0 || to >= rows.value.length) return;
  const items = [...rows.value];
  const [item] = items.splice(from, 1);
  items.splice(to, 0, item);
  rows.value = items;
  normalize();
}

function startDrag(index: number, event: DragEvent) {
  const origin = event.target as HTMLElement | null;
  if (origin?.closest("select, textarea, button")) {
    event.preventDefault();
    return;
  }
  dragIndex.value = index;
  dropIndex.value = index;
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", String(index));
  }
}

function overRow(index: number, event: DragEvent) {
  if (dragIndex.value === null) return;
  event.preventDefault();
  if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
  dropIndex.value = index;
}

function dropRow(index: number) {
  const from = dragIndex.value;
  resetDrag();
  if (from !== null) moveRow(from, index);
}

function resetDrag() {
  dragIndex.value = null;
  dropIndex.value = null;
}

function rowKeydown(index: number, event: KeyboardEvent) {
  if (!event.altKey) return;
  if (event.key === "ArrowUp") {
    event.preventDefault();
    moveRow(index, index - 1);
  } else if (event.key === "ArrowDown") {
    event.preventDefault();
    moveRow(index, index + 1);
  }
}

function when(seconds: number): string {
  const diff = Math.floor(Date.now() / 1000) - seconds;
  if (diff < 60) return "刚刚";
  const minutes = Math.floor(diff / 60);
  if (minutes < 60) return `${minutes} 分钟前`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} 小时前`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days} 天前`;
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
  }).format(new Date(seconds * 1000));
}

function actionHint(row: RebaseRow): string {
  switch (row.action) {
    case "reword":
      return "会替换这个提交的说明，保留改动";
    case "squash":
      return "会并入上一条保留的提交，两段说明都会留下";
    case "fixup":
      return "会并入上一条保留的提交，这条说明会被丢弃";
    case "drop":
      return "会从历史里移除这个提交";
    default:
      return "";
  }
}

const keptCount = computed(() => rows.value.filter((row) => row.action !== "drop").length);
const foldedCount = computed(
  () => rows.value.filter((row) => row.action === "squash" || row.action === "fixup").length,
);
const rewordCount = computed(
  () => rows.value.filter((row) => row.action === "reword").length,
);
const droppedCount = computed(() => rows.value.filter((row) => row.action === "drop").length);
const emptyReword = computed(() =>
  rows.value.some((row) => row.action === "reword" && !row.draft.trim()),
);

const canSubmit = computed(
  () =>
    !!props.plan &&
    !props.loading &&
    !props.error &&
    keptCount.value > 0 &&
    !emptyReword.value,
);

function submit() {
  if (!canSubmit.value) return;
  emit(
    "submit",
    rows.value.map((row) => ({
      oid: row.oid,
      action: row.action,
      ...(row.action === "reword" ? { message: row.draft } : {}),
    })),
  );
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    event.preventDefault();
    emit("close");
  }
}

onMounted(() => window.addEventListener("keydown", onKeydown));
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <Teleport to="body">
    <div class="dialog-layer" @pointerdown.self="emit('close')">
      <section
        class="action-dialog rebase-dialog"
        role="dialog"
        aria-modal="true"
        :aria-labelledby="titleId"
        @pointerdown.stop
      >
        <header class="dialog-head rebase-head">
          <span class="rebase-seal" aria-hidden="true">
            <Icon name="branch" :size="15" />
          </span>
          <div class="rebase-head-copy">
            <p class="rebase-kicker">交互式变基</p>
            <h2 :id="titleId">整理 {{ plan?.branch ?? branch }} 的提交</h2>
            <p v-if="plan" class="rebase-lede">
              从 <code>{{ plan.base.shortId }}</code> 之后开始，共
              {{ plan.commits.length }} 个提交。拖动行调整顺序，或改写、压缩、删除每一步。
            </p>
            <p v-else class="rebase-lede">
              从 {{ baseLabel }} 之后开始整理提交。
            </p>
          </div>
          <button
            type="button"
            class="icon-button dialog-close"
            aria-label="关闭"
            @click="emit('close')"
          >
            <Icon name="close" :size="15" />
          </button>
        </header>

        <div class="dialog-body rebase-body">
          <div v-if="loading" class="rebase-state">
            <span class="loading-orbit" aria-hidden="true"></span>
            正在读取提交…
          </div>

          <div v-else-if="error" class="rebase-state rebase-state-error">
            <p>{{ error }}</p>
            <button type="button" class="button ghost tiny" @click="emit('retry')">
              <Icon name="refresh" :size="12" />
              重新读取
            </button>
          </div>

          <template v-else-if="plan">
            <div class="rebase-note">
              <Icon name="alert" :size="13" />
              <span>变基会改写提交哈希。已经推送过的分支，完成后需要强制推送。</span>
            </div>

            <div class="rebase-list-head" aria-hidden="true">
              <span></span>
              <span>顺序</span>
              <span>提交与说明</span>
              <span>动作</span>
            </div>

            <div class="rebase-list">
              <div
                v-for="(row, index) in rows"
                :key="row.oid"
                class="rebase-row"
                :class="{
                  dropped: row.action === 'drop',
                  folded: row.action === 'squash' || row.action === 'fixup',
                  reworded: row.action === 'reword',
                  dragging: dragIndex === index,
                  'drop-target': dragIndex !== null && dropIndex === index && dragIndex !== index,
                }"
                :tabindex="0"
                :aria-label="`第 ${index + 1} 个提交：${row.subject}`"
                draggable="true"
                @dragstart="startDrag(index, $event)"
                @dragover="overRow(index, $event)"
                @drop.prevent="dropRow(index)"
                @dragend="resetDrag"
                @keydown="rowKeydown(index, $event)"
              >
                <span class="rebase-grip" title="拖动排序，聚焦后可用 Alt+↑/↓" aria-hidden="true">
                  <Icon name="grip" :size="14" />
                </span>
                <span class="rebase-order">{{ index + 1 }}</span>
                <div class="rebase-copy">
                  <div class="rebase-subject">{{ row.subject }}</div>
                  <div class="rebase-meta">
                    <span class="mono">{{ row.shortId }}</span>
                    <span>{{ row.author }}</span>
                    <span>{{ when(row.authoredAt) }}</span>
                  </div>
                  <p v-if="actionHint(row)" class="rebase-hint">{{ actionHint(row) }}</p>
                  <textarea
                    v-if="row.action === 'reword'"
                    v-model="row.draft"
                    class="rebase-message"
                    rows="3"
                    spellcheck="false"
                    aria-label="改写后的提交说明"
                  ></textarea>
                </div>
                <label class="rebase-action">
                  <span class="sr-only">提交动作</span>
                  <select :value="row.action" @change="setAction(row, $event)">
                    <option
                      v-for="option in ACTIONS"
                      :key="option.value"
                      :value="option.value"
                      :disabled="
                        !canFold(row) &&
                        (option.value === 'squash' || option.value === 'fixup')
                      "
                    >
                      {{ option.label }}
                    </option>
                  </select>
                </label>
              </div>
            </div>
          </template>
        </div>

        <footer class="dialog-foot rebase-foot">
          <div class="rebase-tally">
            <span>保留 <strong>{{ keptCount }}</strong></span>
            <span v-if="foldedCount">合并 <strong>{{ foldedCount }}</strong></span>
            <span v-if="rewordCount">改写 <strong>{{ rewordCount }}</strong></span>
            <span v-if="droppedCount" class="drop">删除 <strong>{{ droppedCount }}</strong></span>
            <small v-if="emptyReword" class="rebase-warning">改写后的说明不能为空</small>
            <small v-else-if="plan && keptCount === 0" class="rebase-warning">
              至少要保留一个提交
            </small>
            <small v-else>已推送的分支在改写后需要强制推送</small>
          </div>
          <div class="rebase-foot-actions">
            <button type="button" class="button ghost" @click="emit('close')">取消</button>
            <button
              type="button"
              class="button primary"
              :disabled="!canSubmit"
              @click="submit"
            >
              开始变基
            </button>
          </div>
        </footer>
      </section>
    </div>
  </Teleport>
</template>
