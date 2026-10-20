<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from "vue";
import Icon from "../components/Icon.vue";
import DeltaMark from "../components/DeltaMark.vue";
import { errorText, getCommitPatch, getWorkingDiff } from "../lib/api";
import {
  activeProfileId,
  clearChat,
  currentProfile,
  loadProfiles,
  messages,
  profiles,
  profilesError,
  profilesLoading,
  sendMessage,
  stopStreaming,
  stopping,
  streamError,
  streaming,
  type ChatEntry,
} from "../stores/ai";
import { detail, rows, selectedOid, session } from "../stores/repo";
import { navigate } from "../stores/ui";

const CONTEXT_LIMIT = 24_000;

const input = ref("");
const scrollRef = ref<HTMLElement | null>(null);
const contextLoading = ref(false);
const contextError = ref("");
const copiedId = ref("");
const nearBottom = ref(true);
const useRepoInfo = ref(true);
const useWorktree = ref(false);
const useCommit = ref(false);

type QuickKind = "commitMessage" | "explainWorktree" | "reviewWorktree" | "explainCommit";

const QUICK_PROMPTS: Record<QuickKind, string> = {
  commitMessage:
    "请根据当前工作区变更生成一条提交信息。使用 Conventional Commits 格式，标题不超过 72 个字符；如有必要，在正文中列出要点。只输出提交信息本身，不要添加解释或代码块。",
  explainWorktree:
    "请解释当前工作区变更：说明改动目的、关键实现方式，以及对现有功能可能产生的影响。",
  reviewWorktree:
    "请审查当前工作区变更，按严重程度列出潜在问题（正确性、安全性、性能、可维护性），并给出具体修改建议。若没有发现问题，请明确说明。",
  explainCommit: "请解释选中提交的目的、实现方式与潜在风险，并指出值得复查的代码位置。",
};

const quickActions: { kind: QuickKind; title: string; hint: string; icon: string }[] = [
  {
    kind: "commitMessage",
    title: "生成提交信息",
    hint: "根据工作区改动写一条清晰的提交信息",
    icon: "file",
  },
  {
    kind: "explainWorktree",
    title: "解释工作区变更",
    hint: "把未提交的改动讲清楚",
    icon: "branch",
  },
  {
    kind: "reviewWorktree",
    title: "审查工作区变更",
    hint: "从正确性、安全性、性能排查风险",
    icon: "search",
  },
  {
    kind: "explainCommit",
    title: "解释选中提交",
    hint: "结合图谱里选中的提交与补丁",
    icon: "git-merge",
  },
];

onMounted(() => {
  void loadProfiles();
});

watch(
  messages,
  () => {
    if (nearBottom.value) {
      void nextTick(() => {
        const element = scrollRef.value;
        if (element) element.scrollTop = element.scrollHeight;
      });
    }
  },
  { deep: true },
);

function onScroll() {
  const element = scrollRef.value;
  if (!element) return;
  nearBottom.value =
    element.scrollHeight - element.scrollTop - element.clientHeight < 80;
}

function clipPatch(patch: string): { text: string; clipped: boolean } {
  if (patch.length <= CONTEXT_LIMIT) return { text: patch, clipped: false };
  return {
    text: patch.slice(0, CONTEXT_LIMIT),
    clipped: true,
  };
}

async function buildContext(extra?: { worktree?: boolean; commit?: boolean }): Promise<string> {
  const parts: string[] = [];

  if (useRepoInfo.value && session.value) {
    const recent = rows.value
      .slice(0, 10)
      .map((row) => `${row.shortId} ${row.summary} — ${row.authorName}`)
      .join("\n");
    parts.push(
      [
        `【仓库】${session.value.name}`,
        `当前分支：${session.value.headBranch}`,
        `本地路径：${session.value.path}`,
        `远端：${session.value.remoteUrl ?? "无"}`,
        recent ? `最近提交：\n${recent}` : "",
      ]
        .filter(Boolean)
        .join("\n"),
    );
  }

  const includeWorktree = (useWorktree.value || extra?.worktree) && session.value;
  if (includeWorktree) {
    const diff = await getWorkingDiff(false);
    const clipped = clipPatch(diff.patch);
    parts.push(
      [
        `【当前工作区变更】${diff.filesChanged} 个文件，+${diff.insertions} / -${diff.deletions}${
          diff.truncated || clipped.clipped ? "（补丁已截断）" : ""
        }`,
        "```diff",
        clipped.text || "（没有未提交的变更）",
        "```",
      ].join("\n"),
    );
  }

  const includeCommit = (useCommit.value || extra?.commit) && selectedOid.value;
  if (includeCommit) {
    const oid = selectedOid.value as string;
    const patch = await getCommitPatch(oid);
    const clipped = clipPatch(patch.patch);
    const meta =
      detail.value && detail.value.oid === oid
        ? `${detail.value.shortId} ${detail.value.summary}\n${detail.value.message}`
        : oid;
    parts.push(
      [
        `【选中提交】${meta}`,
        `补丁${patch.truncated || clipped.clipped ? "（已截断）" : ""}：`,
        "```diff",
        clipped.text || "（该提交没有文件变更）",
        "```",
      ].join("\n"),
    );
  }

  return parts.join("\n\n");
}

async function gatherContext(extra?: {
  worktree?: boolean;
  commit?: boolean;
}): Promise<string | null> {
  if (streaming.value) return null;
  if (!activeProfileId.value) {
    streamError.value = "还没有可用的 AI 模型配置，请先在设置中添加模型。";
    return null;
  }
  contextError.value = "";
  contextLoading.value = true;
  try {
    return await buildContext(extra);
  } catch (error) {
    contextError.value = errorText(error);
    return null;
  } finally {
    contextLoading.value = false;
  }
}

async function submitInput() {
  const text = input.value.trim();
  if (!text || streaming.value) return;
  const context = await gatherContext();
  if (context === null) return;
  input.value = "";
  await sendMessage(text, context);
}

async function runQuick(kind: QuickKind) {
  if (streaming.value) return;
  if (kind !== "explainCommit" && !session.value) {
    contextError.value = "请先在提交图谱中打开仓库，再使用工作区上下文。";
    return;
  }
  if (kind === "explainCommit" && !selectedOid.value) {
    contextError.value = "请先在提交图谱中选择一个提交。";
    return;
  }
  const extra =
    kind === "explainCommit"
      ? { commit: true }
      : { worktree: true };
  const context = await gatherContext(extra);
  if (context === null) return;
  await sendMessage(QUICK_PROMPTS[kind], context);
}

async function stop() {
  await stopStreaming();
}

async function copyMessage(entry: ChatEntry) {
  try {
    await navigator.clipboard.writeText(entry.content);
    copiedId.value = entry.id;
    window.setTimeout(() => {
      if (copiedId.value === entry.id) copiedId.value = "";
    }, 1500);
  } catch {
    copiedId.value = "";
  }
}

interface ContentBlock {
  kind: "text" | "code";
  lang: string;
  text: string;
}

function contentBlocks(content: string): ContentBlock[] {
  const blocks: ContentBlock[] = [];
  const pattern = /```([^\n`]*)\n([\s\S]*?)(?:```|$)/g;
  let cursor = 0;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(content)) !== null) {
    if (match.index > cursor) {
      const text = content.slice(cursor, match.index).replace(/^\n+|\n+$/g, "");
      if (text) blocks.push({ kind: "text", lang: "", text });
    }
    blocks.push({
      kind: "code",
      lang: match[1].trim(),
      text: match[2].replace(/\n$/, ""),
    });
    cursor = match.index + match[0].length;
  }
  if (cursor < content.length) {
    const text = content.slice(cursor).replace(/^\n+|\n+$/g, "");
    if (text) blocks.push({ kind: "text", lang: "", text });
  }
  return blocks;
}

function quickDisabled(kind: QuickKind): boolean {
  if (streaming.value || !activeProfileId.value) return true;
  if (kind === "explainCommit") return !selectedOid.value;
  return !session.value;
}
</script>

<template>
  <div class="workspace ai-workspace">
    <h1 class="sr-only">AI 助手</h1>
    <div class="ai-topbar">
      <div class="ai-model">
        <span class="section-label">模型</span>
        <select
          v-if="profiles.length"
          v-model="activeProfileId"
          class="input compact"
          :disabled="profilesLoading || streaming"
          title="选择模型配置"
        >
          <option v-for="profile in profiles" :key="profile.id" :value="profile.id">
            {{ profile.name }} · {{ profile.model }}
          </option>
        </select>
        <span v-else class="tag muted">{{ profilesLoading ? "读取中…" : "未配置模型" }}</span>
        <span v-if="currentProfile" class="model-address">{{ currentProfile.baseUrl }}</span>
      </div>
      <div class="workspace-actions">
        <button
          type="button"
          class="button ghost small"
          :disabled="streaming || !messages.length"
          @click="clearChat"
        >
          <Icon name="plus" :size="15" />
          新对话
        </button>
        <button type="button" class="button ghost small" @click="navigate('settings')">
          <Icon name="settings" :size="15" />
          模型设置
        </button>
      </div>
    </div>

    <p v-if="profilesError" class="alert error" role="alert">{{ profilesError }}</p>
    <p v-if="streamError" class="alert error" role="alert">{{ streamError }}</p>
    <p v-if="contextError" class="alert error" role="alert">{{ contextError }}</p>

    <div ref="scrollRef" class="ai-chat" @scroll="onScroll">
      <div v-if="!messages.length" class="ai-empty">
        <div class="ai-empty-art" aria-hidden="true">
          <DeltaMark seed="ai-assistant" animated />
        </div>
        <h2>今天想一起看点什么？</h2>
        <p>
          结合当前仓库的提交历史与工作区变更，生成提交信息、解释 diff、辅助代码审查。
          请求直接发送到你在设置中配置的模型服务。
        </p>

        <div class="quick-grid">
          <button
            v-for="action in quickActions"
            :key="action.kind"
            type="button"
            class="quick-card"
            :disabled="quickDisabled(action.kind)"
            @click="runQuick(action.kind)"
          >
            <span class="quick-icon"><Icon :name="action.icon" :size="18" /></span>
            <span class="quick-copy">
              <strong>{{ action.title }}</strong>
              <small>{{ action.hint }}</small>
            </span>
            <Icon name="arrow-up-right" :size="16" class="quick-arrow" />
          </button>
        </div>

        <p v-if="!profiles.length && !profilesLoading" class="empty-config">
          先前往设置添加本地 Ollama 或任意 OpenAI 兼容服务。
          <button type="button" class="text-link" @click="navigate('settings')">打开设置</button>
        </p>
        <p v-else-if="!session" class="muted-copy">
          进入一个项目后，就能使用工作区与提交上下文。
        </p>
      </div>

      <div v-else class="ai-messages">
        <article
          v-for="entry in messages"
          :key="entry.id"
          class="ai-message"
          :class="entry.role"
        >
          <div class="ai-avatar" :class="entry.role">
            <Icon v-if="entry.role === 'assistant'" name="sparkles" :size="15" />
            <template v-else>你</template>
          </div>
          <div class="ai-message-body">
            <div class="ai-message-head">
              <strong>{{ entry.role === "user" ? "你" : currentProfile?.name || "AI 助手" }}</strong>
              <span v-if="entry.pending" class="ai-typing">
                {{ stopping ? "正在停止…" : "生成中…" }}
              </span>
              <span v-if="entry.stopped" class="tag">已停止</span>
              <button
                v-if="entry.role === 'assistant' && entry.content"
                type="button"
                class="button ghost tiny ai-copy"
                @click="copyMessage(entry)"
              >
                <Icon name="copy" :size="13" />
                {{ copiedId === entry.id ? "已复制" : "复制" }}
              </button>
            </div>

            <div v-if="entry.content" class="ai-content" :class="entry.role">
              <template v-for="(block, index) in contentBlocks(entry.content)" :key="index">
                <div v-if="block.kind === 'text' && block.text.trim()" class="ai-text">
                  {{ block.text }}
                </div>
                <div v-else-if="block.kind === 'code'" class="ai-code-block">
                  <div class="ai-code-head">
                    <span>{{ block.lang || "code" }}</span>
                  </div>
                  <pre class="ai-code"><code>{{ block.text }}</code></pre>
                </div>
              </template>
              <span v-if="entry.pending && !entry.stopped" class="ai-cursor"></span>
            </div>

            <p v-if="entry.error" class="alert error">{{ entry.error }}</p>
          </div>
        </article>
      </div>
    </div>

    <div class="ai-composer-wrap">
      <div class="ai-composer">
        <div class="ai-context-row">
          <span class="context-label">上下文</span>
          <label
            class="context-chip"
            :class="{ active: useRepoInfo, disabled: !session }"
            title="包含仓库名称、分支与最近提交"
          >
            <input v-model="useRepoInfo" type="checkbox" :disabled="!session" />
            <Icon name="check" :size="12" class="context-check" />
            仓库信息
          </label>
          <label
            class="context-chip"
            :class="{ active: useWorktree, disabled: !session }"
            title="包含尚未提交的变更补丁"
          >
            <input v-model="useWorktree" type="checkbox" :disabled="!session" />
            <Icon name="check" :size="12" class="context-check" />
            工作区变更
          </label>
          <label
            class="context-chip"
            :class="{ active: useCommit, disabled: !selectedOid }"
            title="包含提交图谱中选中的提交补丁"
          >
            <input v-model="useCommit" type="checkbox" :disabled="!selectedOid" />
            <Icon name="check" :size="12" class="context-check" />
            选中提交
            <code v-if="selectedOid">{{ selectedOid.slice(0, 7) }}</code>
          </label>
          <span v-if="contextLoading" class="context-loading">正在读取上下文…</span>
        </div>

        <div class="ai-input-row">
          <textarea
            v-model="input"
            class="ai-input"
            rows="2"
            placeholder="询问代码、生成提交信息或粘贴一段 diff…"
            :disabled="!activeProfileId"
            @keydown.enter.exact.prevent="submitInput"
          ></textarea>
          <button
            v-if="streaming"
            type="button"
            class="send-button stop"
            aria-label="停止生成"
            :disabled="stopping"
            @click="stop"
          >
            <Icon name="stop" :size="17" />
          </button>
          <button
            v-else
            type="button"
            class="send-button"
            aria-label="发送"
            :disabled="!input.trim() || !activeProfileId || contextLoading"
            @click="submitInput"
          >
            <Icon name="arrow-up-right" :size="19" />
          </button>
        </div>

        <div class="ai-composer-foot">
          <span class="muted-copy">
            Enter 发送 · Shift+Enter 换行
            <span v-if="!activeProfileId && !profilesLoading"> · 请先配置模型</span>
          </span>
          <span v-if="streaming" class="generating-note">正在生成回答</span>
        </div>
      </div>
    </div>
  </div>
</template>
