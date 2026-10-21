<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import CanvasMenu from "./CanvasMenu.vue";
import Icon from "./Icon.vue";
import {
  amend,
  description,
  drafting,
  identity,
  reuseCommitMessage,
  stageAllAndCommit,
  stagedCount,
  stagedSummary,
  stopDraft,
  summary,
  commitStaged,
  writeDraft,
} from "../stores/workspace";
import { rows } from "../stores/repo";
import type { MenuItem } from "../types";

const props = defineProps<{
  branch: string;
  canPush: boolean;
  disabled?: boolean;
}>();

const emit = defineEmits<{
  (e: "push"): void;
}>();

const menu = ref<{ x: number; y: number } | null>(null);
const historyMenu = ref<{ x: number; y: number } | null>(null);
const summaryRef = ref<HTMLInputElement | null>(null);

const canCommit = computed(
  () => !props.disabled && (amend.value || stagedCount.value > 0) && summary.value.trim().length > 0,
);

const menuItems = computed<MenuItem[]>(() => [
  { id: "stage-commit", label: "暂存全部并提交", icon: "plus", disabled: props.disabled },
  { id: "commit-push", label: "提交并推送", icon: "upload", disabled: !props.canPush || !!props.disabled },
]);

const historyItems = computed<MenuItem[]>(() =>
  rows.value.slice(0, 12).map((row) => ({
    id: row.oid,
    label: `${row.shortId} · ${row.summary}`,
    icon: "commit",
  })),
);

async function onCommit() {
  if (!canCommit.value) return;
  await commitStaged();
}

function openMenu(event: MouseEvent) {
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  menu.value = { x: rect.right - 6, y: rect.bottom + 4 };
}

async function onMenuSelect(id: string) {
  menu.value = null;
  if (id === "stage-commit") {
    await stageAllAndCommit();
    return;
  }
  if (id === "commit-push") {
    const committed = await commitStaged();
    if (committed) emit("push");
  }
}

async function onDraft() {
  if (drafting.value) return;
  const written = await writeDraft();
  if (written && !summaryRef.value?.matches(":focus")) {
    await nextTick();
    summaryRef.value?.focus();
  }
}

function onDraftClick() {
  if (drafting.value) {
    stopDraft();
    return;
  }
  void onDraft();
}

function openHistory(event: MouseEvent) {
  if (!historyItems.value.length) return;
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  historyMenu.value = { x: rect.right - 6, y: rect.bottom + 4 };
}

async function onHistorySelect(id: string) {
  historyMenu.value = null;
  const filled = await reuseCommitMessage(id);
  if (filled && !summaryRef.value?.matches(":focus")) {
    await nextTick();
    summaryRef.value?.focus();
  }
}

function onKeydown(event: KeyboardEvent) {
  if (event.key !== "Enter" || !(event.ctrlKey || event.metaKey)) return;
  event.preventDefault();
  void onCommit();
}

onMounted(() => {
  window.addEventListener("keydown", onKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <div class="composer">
    <div class="composer-body">
      <div class="composer-head">
        <span class="section-label">
          {{ amend ? "改写最后一次提交" : `提交到 ${branch}` }}
        </span>
        <div class="composer-head-actions">
          <button
            type="button"
            class="button ghost tiny"
            :disabled="disabled || !historyItems.length"
            title="复用历史提交信息"
            @click="openHistory"
          >
            <Icon name="clock" :size="13" />
            历史
          </button>
          <button
            type="button"
            class="button ghost tiny"
            :disabled="disabled || (!drafting && stagedCount === 0)"
            :title="
              drafting
                ? '停止 AI 撰写'
                : stagedCount
                  ? '根据已暂存的差异撰写提交信息'
                  : '请先暂存变更'
            "
            @click="onDraftClick"
          >
            <span v-if="drafting" class="loading-orbit" aria-hidden="true"></span>
            <Icon v-else name="sparkles" :size="13" />
            {{ drafting ? "停止" : "AI 写" }}
          </button>
        </div>
      </div>

      <label class="field">
        <span>摘要</span>
        <input
          ref="summaryRef"
          v-model="summary"
          class="input"
          type="text"
          maxlength="72"
          :placeholder="amend ? '改写后的提交摘要' : '一句话说清这次改动'"
          @keydown.enter.prevent
        />
      </label>

      <label class="field">
        <span>详细描述（可选）</span>
        <textarea
          v-model="description"
          rows="4"
          placeholder="为什么这样改、有什么影响，留空也可以。"
        ></textarea>
      </label>

      <label class="check-row composer-amend">
        <input v-model="amend" type="checkbox" :disabled="disabled" />
        用这次提交改写最后一次（amend）
      </label>

      <div class="composer-stage-line">
        <span class="composer-count">
          已暂存 <strong>{{ stagedCount }}</strong> 个文件
        </span>
        <span class="diff-stats composer-stats">
          <span class="stat-add">+{{ stagedSummary.additions }}</span>
          <span class="stat-del">−{{ stagedSummary.deletions }}</span>
        </span>
      </div>

      <p v-if="!identity" class="composer-warn">
        <Icon name="alert" :size="13" />
        还没有配置 Git 用户身份，提交会失败。请先设置 user.name 与 user.email。
      </p>
      <p v-else class="composer-identity">提交者：{{ identity }}</p>
    </div>

    <div class="composer-foot">
      <div class="composer-split">
        <button
          type="button"
          class="button primary composer-main"
          :disabled="!canCommit"
          @click="onCommit"
        >
          <Icon name="commit" :size="14" />
          {{ amend ? "改写提交" : "提交" }}
        </button>
        <button
          type="button"
          class="button primary composer-more"
          title="更多提交方式"
          :disabled="!canCommit"
          @click="openMenu"
        >
          更多
          <Icon name="chevron-down" :size="13" />
        </button>
      </div>

      <p class="composer-hint">
        <kbd>Ctrl</kbd> + <kbd>Enter</kbd> 提交<span v-if="!stagedCount"> · 先在上方暂存文件</span>
      </p>
    </div>

    <CanvasMenu
      v-if="menu"
      :x="menu.x"
      :y="menu.y"
      :items="menuItems"
      @close="menu = null"
      @select="onMenuSelect"
    />
    <CanvasMenu
      v-if="historyMenu"
      :x="historyMenu.x"
      :y="historyMenu.y"
      :items="historyItems"
      @close="historyMenu = null"
      @select="onHistorySelect"
    />
  </div>
</template>
