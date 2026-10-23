<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import Icon from "./Icon.vue";
import type { ConflictBlock, ConflictFile, WorkingFile } from "../types";

const props = defineProps<{
  file: WorkingFile | null;
  conflict: ConflictFile | null;
  loading: boolean;
  error?: string;
  disabled?: boolean;
  backLabel?: string;
}>();

const emit = defineEmits<{
  (e: "back"): void;
  (e: "reload"): void;
  (e: "save", content: string): void;
  (e: "adopt-side", side: "ours" | "theirs"): void;
  (e: "mark-resolved"): void;
}>();

type ReferenceKey = "base" | "ours" | "theirs";

interface TextSegment {
  kind: "text";
  position: number;
  head: string;
  tail: string;
  hidden: number;
}

interface ConflictSegment {
  kind: "conflict";
  position: number;
  slot: number;
  block: ConflictBlock;
}

type Segment = TextSegment | ConflictSegment;

const CONTEXT_HEAD = 10;
const CONTEXT_TAIL = 6;

const resolutions = ref<string[]>([]);
const handled = ref<boolean[]>([]);
const expandedContext = ref<Set<number>>(new Set());
const referenceOpen = ref(false);
const referenceTab = ref<ReferenceKey>("ours");
const referenceBody = ref<HTMLElement | null>(null);

watch(referenceTab, () => {
  void nextTick(() => {
    if (referenceBody.value) referenceBody.value.scrollTop = 0;
  });
});

watch(
  () => props.conflict,
  (next) => {
    const count = next?.conflictCount ?? 0;
    resolutions.value = Array.from({ length: count }, () => "");
    handled.value = Array.from({ length: count }, () => false);
    expandedContext.value = new Set();
    referenceOpen.value = false;
    referenceTab.value = next?.basePresent
      ? "base"
      : next?.oursPresent
        ? "ours"
        : "theirs";
  },
  { immediate: true },
);

const stateLabel = computed(() => {
  const state = props.conflict?.state ?? "";
  if (state.startsWith("rebase")) return "变基中";
  if (state.startsWith("cherry-pick")) return "拣选中";
  if (state === "revert") return "还原中";
  if (state === "merge") return "合并中";
  return "解决冲突";
});

const sideNames = computed(() => {
  const state = props.conflict?.state ?? "";
  if (state.startsWith("rebase")) {
    return { ours: "变基目标", theirs: "正在应用的提交" };
  }
  if (state.startsWith("cherry-pick")) {
    return { ours: "当前分支", theirs: "正在拣选的提交" };
  }
  if (state === "revert") {
    return { ours: "当前分支", theirs: "正在还原的提交" };
  }
  return { ours: "当前分支", theirs: "传入变更" };
});

const segments = computed<Segment[]>(() => {
  const blocks = props.conflict?.blocks ?? [];
  return blocks.map((block, position) => {
    if (block.kind === "conflict") {
      return { kind: "conflict", position, slot: block.index - 1, block };
    }
    const lines = block.text === "" ? [] : block.text.split("\n");
    if (
      lines.length <= CONTEXT_HEAD + CONTEXT_TAIL ||
      expandedContext.value.has(position)
    ) {
      return { kind: "text", position, head: block.text, tail: "", hidden: 0 };
    }
    return {
      kind: "text",
      position,
      head: lines.slice(0, CONTEXT_HEAD).join("\n"),
      tail: lines.slice(-CONTEXT_TAIL).join("\n"),
      hidden: lines.length - CONTEXT_HEAD - CONTEXT_TAIL,
    };
  });
});

const handledCount = computed(() => handled.value.filter(Boolean).length);

const allHandled = computed(
  () =>
    !!props.conflict &&
    props.conflict.conflictCount > 0 &&
    handledCount.value === props.conflict.conflictCount,
);

const canSave = computed(
  () => !!props.conflict?.editable && allHandled.value && !props.disabled,
);

const canMarkResolved = computed(
  () => !!props.conflict && !props.conflict.hasMarkers && !props.disabled,
);

const fallbackTitle = computed(() => {
  const conflict = props.conflict;
  if (!conflict) return "";
  if (conflict.reason === "binary") return "二进制文件无法逐段合并";
  if (conflict.reason === "tooLarge") return "文件太大，没有展开逐段视图";
  if (conflict.reason === "missing") return "工作区里没有这个文件";
  if (conflict.truncated) return "冲突段太多，只解析了前面一部分";
  return "文件里已经没有冲突标记";
});

const fallbackText = computed(() => {
  const conflict = props.conflict;
  if (!conflict) return "";
  if (conflict.reason === "binary") {
    return "整文件采用一侧，或在外部工具里处理完再回来标记。";
  }
  if (conflict.reason === "tooLarge") {
    return "超过 1 MB 的文件不在应用内展开。可以整文件采用一侧，或在外部编辑器处理后重新读取。";
  }
  if (conflict.reason === "missing") {
    return "有一侧删除了这个文件。采用删除的一侧会把它从工作区移除，采用另一侧会恢复文件内容。";
  }
  if (conflict.truncated) {
    return "建议在外部编辑器里处理完，再重新读取确认标记已经清理干净。";
  }
  return "可能在外部编辑器里已经处理完了。确认内容无误后标记为已解决，或重新读取再看一次。";
});

const referenceTabs = computed(() => {
  const conflict = props.conflict;
  return [
    {
      key: "base" as const,
      label: "基准",
      title: "共同祖先版本",
      present: conflict?.basePresent ?? false,
      text: conflict?.base ?? null,
    },
    {
      key: "ours" as const,
      label: "当前",
      title: sideNames.value.ours,
      present: conflict?.oursPresent ?? false,
      text: conflict?.ours ?? null,
    },
    {
      key: "theirs" as const,
      label: "传入",
      title: sideNames.value.theirs,
      present: conflict?.theirsPresent ?? false,
      text: conflict?.theirs ?? null,
    },
  ];
});

const activeReference = computed(
  () => referenceTabs.value.find((tab) => tab.key === referenceTab.value) ?? null,
);

function baseOf(path: string): string {
  const at = path.lastIndexOf("/");
  return at >= 0 ? path.slice(at + 1) : path;
}

function dirOf(path: string): string {
  const at = path.lastIndexOf("/");
  return at >= 0 ? path.slice(0, at) : "";
}

function lineCount(text: string): number {
  if (!text) return 0;
  return text.split("\n").length;
}

function toggleContext(position: number) {
  const next = new Set(expandedContext.value);
  if (next.has(position)) next.delete(position);
  else next.add(position);
  expandedContext.value = next;
}

function setResolution(slot: number, text: string) {
  const nextResolutions = [...resolutions.value];
  nextResolutions[slot] = text;
  resolutions.value = nextResolutions;
  const nextHandled = [...handled.value];
  nextHandled[slot] = true;
  handled.value = nextHandled;
}

function markHandled(slot: number) {
  if (handled.value[slot]) return;
  const next = [...handled.value];
  next[slot] = true;
  handled.value = next;
}

function takeBoth(block: ConflictBlock) {
  const text =
    block.ours && block.theirs
      ? `${block.ours}\n${block.theirs}`
      : block.ours || block.theirs;
  setResolution(block.index - 1, text);
}

function adoptAll(side: "ours" | "theirs") {
  if (!props.conflict) return;
  const nextResolutions = [...resolutions.value];
  const nextHandled = [...handled.value];
  for (const block of props.conflict.blocks) {
    if (block.kind !== "conflict") continue;
    const slot = block.index - 1;
    nextResolutions[slot] = side === "ours" ? block.ours : block.theirs;
    nextHandled[slot] = true;
  }
  resolutions.value = nextResolutions;
  handled.value = nextHandled;
}

function assemble(): string {
  const conflict = props.conflict;
  if (!conflict) return "";
  const lines: string[] = [];
  for (const block of conflict.blocks) {
    if (block.kind === "text") {
      lines.push(...block.text.split("\n"));
      continue;
    }
    const resolution = resolutions.value[block.index - 1] ?? "";
    if (resolution !== "") lines.push(...resolution.split("\n"));
  }
  if (!lines.length) return "";
  return lines.join("\n") + (conflict.finalNewline ? "\n" : "");
}

function save() {
  if (canSave.value) emit("save", assemble());
}
</script>

<template>
  <section class="surface diff-sheet merge-sheet" aria-label="冲突合并">
    <div v-if="backLabel" class="diff-backbar">
      <button type="button" class="button ghost tiny" @click="emit('back')">
        <Icon name="arrow-left" :size="13" />
        返回{{ backLabel }}
      </button>
    </div>

    <div v-if="file" class="diff-pane-head merge-head">
      <span class="conflict-seal" aria-hidden="true">
        <Icon name="alert" :size="13" />
      </span>
      <div class="diff-pane-name">
        <strong>{{ baseOf(file.path) }}</strong>
        <small v-if="dirOf(file.path)">{{ dirOf(file.path) }}</small>
      </div>
      <span class="stage-tag stage-conflicted">{{ stateLabel }}</span>
      <button
        type="button"
        class="button ghost tiny"
        :disabled="!conflict"
        :aria-expanded="referenceOpen"
        @click="referenceOpen = !referenceOpen"
      >
        <Icon name="file" :size="13" />
        参考版本
        <Icon
          :name="referenceOpen ? 'chevron-down' : 'chevron-right'"
          :size="11"
        />
      </button>
    </div>

    <div v-if="conflict && referenceOpen" class="merge-reference">
      <div class="reference-tabs" role="tablist" aria-label="冲突参考版本">
        <button
          v-for="tab in referenceTabs"
          :key="tab.key"
          type="button"
          role="tab"
          class="reference-tab"
          :class="{ active: referenceTab === tab.key }"
          :aria-selected="referenceTab === tab.key"
          :disabled="!tab.present"
          :title="tab.present ? tab.title : `${tab.title}（这一侧没有文件）`"
          @click="referenceTab = tab.key"
        >
          {{ tab.label }}
        </button>
      </div>
      <pre
        v-if="activeReference && activeReference.text !== null"
        ref="referenceBody"
        class="reference-body"
      >{{ activeReference.text || " " }}</pre>
      <p v-else class="reference-empty">这个版本里没有这个文件。</p>
      <p v-if="conflict.referenceTruncated" class="reference-note">
        参考版本太长，只显示了前面一部分。
      </p>
    </div>

    <div class="diff-pane-body merge-body">
      <div v-if="loading" class="diff-loading">
        <span class="loading-orbit" aria-hidden="true"></span>
        正在读取冲突…
      </div>
      <div v-else-if="error" class="merge-error">
        <p>{{ error }}</p>
        <button type="button" class="button ghost tiny" @click="emit('reload')">
          <Icon name="refresh" :size="12" />
          重新读取
        </button>
      </div>

      <template v-else-if="conflict">
        <template v-if="conflict.editable">
          <div class="merge-intro">
            <span class="merge-count">{{ conflict.conflictCount }} 处冲突</span>
            <span>逐段采用一侧、两段都要，或直接改写合并结果。</span>
          </div>

          <template v-for="segment in segments" :key="`${segment.kind}-${segment.position}`">
            <div
              v-if="segment.kind === 'text'"
              class="merge-context"
              :class="{ blank: !segment.head && !segment.tail }"
            >
              <pre v-if="segment.head" class="merge-context-pre">{{ segment.head }}</pre>
              <button
                v-if="segment.hidden"
                type="button"
                class="merge-context-more"
                @click="toggleContext(segment.position)"
              >
                <Icon name="chevron-down" :size="11" />
                展开 {{ segment.hidden }} 行未改动的上下文
              </button>
              <pre v-if="segment.tail" class="merge-context-pre">{{ segment.tail }}</pre>
            </div>

            <article
              v-else
              class="conflict-card"
              :class="{ handled: handled[segment.slot] }"
            >
              <header class="conflict-card-head">
                <span class="conflict-number">冲突 {{ segment.block.index }}</span>
                <span class="conflict-state">
                  <Icon
                    :name="handled[segment.slot] ? 'check' : 'alert'"
                    :size="11"
                  />
                  {{ handled[segment.slot] ? "已处理" : "待处理" }}
                </span>
              </header>

              <div class="conflict-sides">
                <section class="conflict-side side-ours">
                  <header class="conflict-side-head">
                    <span class="side-chip">当前</span>
                    <span class="side-name">{{ sideNames.ours }}</span>
                    <span v-if="segment.block.oursLabel" class="side-ref">{{
                      segment.block.oursLabel
                    }}</span>
                  </header>
                  <pre
                    class="conflict-side-body"
                    :class="{ empty: !segment.block.ours }"
                  >{{ segment.block.ours || "（这一侧为空）" }}</pre>
                  <button
                    type="button"
                    class="side-pick"
                    :disabled="disabled"
                    @click="setResolution(segment.slot, segment.block.ours)"
                  >
                    <Icon name="chevron-right" :size="11" />
                    采用当前
                  </button>
                </section>

                <section class="conflict-side side-theirs">
                  <header class="conflict-side-head">
                    <span class="side-chip">传入</span>
                    <span class="side-name">{{ sideNames.theirs }}</span>
                    <span v-if="segment.block.theirsLabel" class="side-ref">{{
                      segment.block.theirsLabel
                    }}</span>
                  </header>
                  <pre
                    class="conflict-side-body"
                    :class="{ empty: !segment.block.theirs }"
                  >{{ segment.block.theirs || "（这一侧为空）" }}</pre>
                  <button
                    type="button"
                    class="side-pick"
                    :disabled="disabled"
                    @click="setResolution(segment.slot, segment.block.theirs)"
                  >
                    <Icon name="chevron-right" :size="11" />
                    采用传入
                  </button>
                </section>
              </div>

              <div class="conflict-result">
                <header class="conflict-result-head">
                  <label :for="`merge-result-${segment.position}`">合并结果</label>
                  <span class="result-lines">
                    {{ lineCount(resolutions[segment.slot] ?? "") }} 行
                  </span>
                  <button
                    v-if="segment.block.base !== null"
                    type="button"
                    class="result-adopt"
                    :disabled="disabled"
                    title="采用共同祖先版本"
                    @click="setResolution(segment.slot, segment.block.base ?? '')"
                  >
                    采用基准
                  </button>
                  <button
                    type="button"
                    class="result-adopt"
                    :disabled="disabled"
                    title="先放当前版本，再放传入版本"
                    @click="takeBoth(segment.block)"
                  >
                    两者都要
                  </button>
                </header>
                <textarea
                  :id="`merge-result-${segment.position}`"
                  v-model="resolutions[segment.slot]"
                  class="conflict-result-input"
                  spellcheck="false"
                  :disabled="disabled"
                  placeholder="可以直接在这里写合并后的内容，留空表示两边都不要。"
                  @input="markHandled(segment.slot)"
                ></textarea>
              </div>
            </article>
          </template>

          <p v-if="!conflict.finalNewline" class="merge-eof-note">
            原文件末尾没有换行，保存时会保持这一状态。
          </p>
        </template>

        <div v-else class="merge-fallback">
          <span class="merge-fallback-mark" aria-hidden="true">
            <Icon
              :name="conflict.reason === 'missing' ? 'alert' : 'file'"
              :size="24"
            />
          </span>
          <h3>{{ fallbackTitle }}</h3>
          <p class="merge-fallback-text">{{ fallbackText }}</p>

          <div class="fallback-sides">
            <section class="fallback-side">
              <header>
                <span class="side-chip">当前</span>
                <strong>{{ sideNames.ours }}</strong>
              </header>
              <p>{{ conflict.oursPresent ? "这一侧有文件内容" : "这一侧删除了文件" }}</p>
              <button
                type="button"
                class="button ghost tiny"
                :disabled="disabled"
                @click="emit('adopt-side', 'ours')"
              >
                {{ conflict.oursPresent ? "采用当前版本" : "采用删除" }}
              </button>
            </section>
            <section class="fallback-side">
              <header>
                <span class="side-chip">传入</span>
                <strong>{{ sideNames.theirs }}</strong>
              </header>
              <p>{{ conflict.theirsPresent ? "这一侧有文件内容" : "这一侧删除了文件" }}</p>
              <button
                type="button"
                class="button ghost tiny"
                :disabled="disabled"
                @click="emit('adopt-side', 'theirs')"
              >
                {{ conflict.theirsPresent ? "采用传入版本" : "采用删除" }}
              </button>
            </section>
          </div>

          <div class="fallback-actions">
            <button
              type="button"
              class="button ghost tiny"
              :disabled="disabled"
              @click="emit('reload')"
            >
              <Icon name="refresh" :size="12" />
              重新读取
            </button>
            <button
              v-if="canMarkResolved"
              type="button"
              class="button primary tiny"
              :disabled="disabled"
              title="直接采用工作区里现在的文件内容（或删除）并标记为已解决"
              @click="emit('mark-resolved')"
            >
              <Icon name="check" :size="12" />
              标记为已解决
            </button>
          </div>
        </div>
      </template>
    </div>

    <footer v-if="conflict?.editable && !loading" class="merge-footer">
      <span class="merge-progress" :class="{ done: allHandled }">
        <Icon :name="allHandled ? 'check' : 'alert'" :size="12" />
        已处理 {{ handledCount }} / {{ conflict.conflictCount }}
      </span>
      <div class="merge-footer-actions">
        <button
          type="button"
          class="button ghost tiny"
          :disabled="disabled"
          @click="adoptAll('ours')"
        >
          全部采用当前
        </button>
        <button
          type="button"
          class="button ghost tiny"
          :disabled="disabled"
          @click="adoptAll('theirs')"
        >
          全部采用传入
        </button>
        <button
          type="button"
          class="button primary small"
          :disabled="!canSave"
          @click="save"
        >
          <Icon name="check" :size="13" />
          保存并标记为已解决
        </button>
      </div>
    </footer>
  </section>
</template>
