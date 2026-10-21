<script setup lang="ts">
import Icon from "./Icon.vue";
import type { FileDiff, WorkingFile } from "../types";

defineProps<{
  diff: FileDiff | null;
  file: WorkingFile | null;
  loading?: boolean;
  disabled?: boolean;
  backLabel?: string;
}>();

defineEmits<{
  (e: "apply-hunk", index: number): void;
  (e: "toggle-file"): void;
  (e: "discard"): void;
  (e: "back"): void;
}>();

const STAGE_LABEL: Record<string, string> = {
  staged: "已暂存",
  unstaged: "未暂存",
  untracked: "未跟踪",
  conflicted: "冲突中",
};

const STATUS_GLYPH: Record<string, string> = {
  added: "A",
  deleted: "D",
  modified: "M",
  renamed: "R",
  copied: "C",
  typechange: "T",
  conflicted: "U",
};

function dirOf(path: string): string {
  const index = path.lastIndexOf("/");
  return index === -1 ? "" : path.slice(0, index + 1);
}

function baseOf(path: string): string {
  const index = path.lastIndexOf("/");
  return index === -1 ? path : path.slice(index + 1);
}

function toggleLabel(kind: string): string {
  if (kind === "staged") return "取消暂存";
  if (kind === "untracked") return "跟踪并暂存";
  return "暂存文件";
}
</script>

<template>
  <section class="surface diff-sheet" aria-label="文件差异">
    <div v-if="backLabel" class="diff-backbar">
      <button type="button" class="button ghost tiny" @click="$emit('back')">
        <Icon name="arrow-left" :size="13" />
        返回{{ backLabel }}
      </button>
    </div>
    <div v-if="!file && !loading" class="diff-empty">
      <div class="inspector-mark" aria-hidden="true">
        <Icon name="file" :size="26" />
      </div>
      <p>从左边挑一个变更文件</p>
      <span>这里会显示它与暂存区或 HEAD 的差异，可以整文件暂存，也可以只暂存一个代码块。</span>
    </div>

    <div v-else-if="file?.stage === 'conflicted'" class="diff-empty">
      <div class="inspector-mark conflict-mark" aria-hidden="true">
        <Icon name="alert" :size="26" />
      </div>
      <p>{{ file.path }}</p>
      <span>
        这个文件还有冲突标记。请在编辑器里解决冲突，保存后回到左侧点勾把它标记为已解决，再继续合并。
      </span>
    </div>

    <template v-else>
      <div v-if="file" class="diff-pane-head">
        <span class="file-status" :class="`status-${file.status}`">
          {{ STATUS_GLYPH[file.status] ?? "M" }}
        </span>
        <div class="diff-pane-name">
          <strong>{{ baseOf(file.path) }}</strong>
          <small v-if="dirOf(file.path)">{{ dirOf(file.path) }}</small>
        </div>
        <span class="stage-tag" :class="`stage-${file.stage}`">
          {{ STAGE_LABEL[file.stage] ?? file.stage }}
        </span>
        <div class="diff-pane-actions">
          <button
            type="button"
            class="button ghost tiny"
            :disabled="disabled"
            @click="$emit('toggle-file')"
          >
            <Icon :name="file.stage === 'staged' ? 'minus' : 'plus'" :size="13" />
            {{ toggleLabel(file.stage) }}
          </button>
          <button
            type="button"
            class="button ghost tiny danger-text"
            :disabled="disabled"
            @click="$emit('discard')"
          >
            <Icon name="trash" :size="13" />
            丢弃
          </button>
        </div>
      </div>

      <div class="diff-pane-body">
        <div v-if="loading" class="diff-loading">
          <span class="loading-orbit" aria-hidden="true"></span>
          正在读取差异…
        </div>

        <template v-else-if="diff">
          <div class="diff-stats pane-stats">
            <span class="stat-files">{{ diff.hunks.length }} 个代码块</span>
            <span class="stat-add">+{{ diff.additions }}</span>
            <span class="stat-del">−{{ diff.deletions }}</span>
          </div>

          <p v-if="diff.binary" class="alert info">这是一个二进制文件，没有文本差异可以展示。</p>

          <div v-for="hunk in diff.hunks" :key="hunk.index" class="hunk">
            <div class="hunk-head">
              <span class="hunk-label" :title="hunk.header">{{ hunk.header }}</span>
              <span class="hunk-stats">
                <span v-if="hunk.additions" class="stat-add">+{{ hunk.additions }}</span>
                <span v-if="hunk.deletions" class="stat-del">−{{ hunk.deletions }}</span>
              </span>
              <button
                type="button"
                class="hunk-stage"
                :class="{ reverse: diff.stage === 'staged' }"
                :disabled="!diff.hunkable || disabled"
                :title="
                  diff.hunkable
                    ? diff.stage === 'staged'
                      ? '把这个代码块退回工作区'
                      : '只暂存这个代码块'
                    : '这个文件不支持按代码块暂存，请整文件暂存'
                "
                @click="$emit('apply-hunk', hunk.index)"
              >
                <Icon :name="diff.stage === 'staged' ? 'minus' : 'plus'" :size="12" />
                {{ diff.stage === "staged" ? "取消本块" : "暂存本块" }}
              </button>
            </div>
            <pre class="diff-pre"><span
              v-for="(line, index) in hunk.lines"
              :key="index"
              class="diff-line"
              :class="`diff-${line.kind}`"
            >{{ line.text || " " }}</span></pre>
          </div>

          <p v-if="diff.truncated" class="alert info">
            这个文件的差异很长，只显示了前面的代码块，其余部分请在编辑器中查看。
          </p>
          <div v-else-if="!diff.hunks.length" class="file-empty">没有文本差异</div>
        </template>

        <div v-else class="file-empty">读取差异失败，请刷新工作区后重试。</div>
      </div>
    </template>
  </section>
</template>
