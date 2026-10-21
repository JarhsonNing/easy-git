<script setup lang="ts">
import { computed } from "vue";
import Icon from "./Icon.vue";
import type { CommitDiff, CommitFileDiff } from "../types";

const props = defineProps<{
  diff: CommitDiff | null;
  title: string;
  subtitle?: string;
  onlyPath?: string | null;
  loading?: boolean;
  emptyText?: string;
  backLabel?: string;
}>();

defineEmits<{
  (e: "back"): void;
}>();

const STATUS_GLYPH: Record<string, string> = {
  added: "A",
  deleted: "D",
  modified: "M",
  renamed: "R",
  copied: "C",
  typechange: "T",
  conflicted: "U",
};

const visibleFiles = computed<CommitFileDiff[]>(() => {
  if (!props.diff) return [];
  if (!props.onlyPath) return props.diff.files;
  return props.diff.files.filter((file) => file.path === props.onlyPath);
});

function baseOf(path: string): string {
  const index = path.lastIndexOf("/");
  return index === -1 ? path : path.slice(index + 1);
}

function dirOf(path: string): string {
  const index = path.lastIndexOf("/");
  return index === -1 ? "" : path.slice(0, index + 1);
}
</script>

<template>
  <section class="surface diff-sheet commit-diff-sheet" aria-label="提交差异">
    <div v-if="backLabel" class="diff-backbar">
      <button type="button" class="button ghost tiny" @click="$emit('back')">
        <Icon name="arrow-left" :size="13" />
        返回{{ backLabel }}
      </button>
    </div>
    <div class="diff-pane-head commit-diff-head">
      <div class="diff-pane-name">
        <strong>{{ title }}</strong>
        <small v-if="subtitle">{{ subtitle }}</small>
      </div>
      <span v-if="diff" class="diff-stats pane-stats">
        <span class="stat-files">{{ diff.files.length }} 个文件</span>
        <span class="stat-add">+{{ diff.additions }}</span>
        <span class="stat-del">−{{ diff.deletions }}</span>
      </span>
      <slot name="actions"></slot>
    </div>

    <div class="diff-pane-body commit-diff-body">
      <div v-if="loading" class="diff-loading">
        <span class="loading-orbit" aria-hidden="true"></span>
        正在读取差异…
      </div>

      <div v-else-if="!diff" class="diff-empty">
        <div class="inspector-mark" aria-hidden="true">
          <Icon name="file" :size="26" />
        </div>
        <p>{{ emptyText ?? "选择一个文件查看差异" }}</p>
      </div>

      <template v-else-if="visibleFiles.length">
        <article v-for="file in visibleFiles" :key="file.path" class="commit-file-diff">
          <header class="commit-file-head">
            <span class="file-status" :class="`status-${file.status}`">
              {{ STATUS_GLYPH[file.status] ?? "M" }}
            </span>
            <span class="commit-file-path">
              <strong>{{ baseOf(file.path) }}</strong>
              <small v-if="dirOf(file.path)">{{ dirOf(file.path) }}</small>
            </span>
            <span
              v-if="file.oldPath && file.oldPath !== file.path"
              class="commit-file-old"
              :title="file.oldPath"
            >← {{ file.oldPath }}</span>
            <span class="file-stats">
              <span v-if="file.additions" class="stat-add">+{{ file.additions }}</span>
              <span v-if="file.deletions" class="stat-del">−{{ file.deletions }}</span>
            </span>
          </header>

          <p v-if="file.binary" class="alert info">这是一个二进制文件，没有文本差异可以展示。</p>
          <template v-else>
            <div v-for="hunk in file.hunks" :key="hunk.index" class="hunk">
              <div class="hunk-head">
                <span class="hunk-label" :title="hunk.header">{{ hunk.header }}</span>
                <span class="hunk-stats">
                  <span v-if="hunk.additions" class="stat-add">+{{ hunk.additions }}</span>
                  <span v-if="hunk.deletions" class="stat-del">−{{ hunk.deletions }}</span>
                </span>
              </div>
              <pre class="diff-pre"><span
                v-for="(line, index) in hunk.lines"
                :key="index"
                class="diff-line"
                :class="`diff-${line.kind}`"
              >{{ line.text || " " }}</span></pre>
            </div>
            <p v-if="!file.hunks.length" class="file-empty">没有文本差异</p>
            <p v-if="file.truncated" class="alert info">这个文件的差异很长，只显示了前面的部分。</p>
          </template>
        </article>

        <p v-if="diff.truncated" class="alert info">
          变更文件太多或差异过长，只显示了前面的一部分。
        </p>
      </template>

      <div v-else class="diff-empty">
        <div class="inspector-mark" aria-hidden="true">
          <Icon name="file" :size="26" />
        </div>
        <p>{{ emptyText ?? "这个提交没有文本差异" }}</p>
      </div>
    </div>
  </section>
</template>
