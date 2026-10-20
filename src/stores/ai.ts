import { computed, ref } from "vue";
import { cancelAi, chatStream, errorText, getSettings } from "../lib/api";
import type { AiProfile, ChatMessage } from "../types";

export interface ChatEntry {
  id: string;
  role: "user" | "assistant";
  content: string;
  pending: boolean;
  stopped?: boolean;
  error?: string;
}

const SYSTEM_PROMPT = `你是 Easy Git 桌面应用内置的代码助手，服务对象是正在本地查看 Git 仓库的开发者。
请使用简体中文回答，除非用户明确要求其他语言。
回答要具体、可执行；引用代码时使用围栏代码块并标注语言。
如果提供的上下文不足以判断，请说明还缺少哪些信息，不要编造。
不要输出与问题无关的客套话。`;

export const profiles = ref<AiProfile[]>([]);
export const activeProfileId = ref<string | null>(null);
export const profilesLoading = ref(false);
export const profilesError = ref("");

export const messages = ref<ChatEntry[]>([]);
export const streaming = ref(false);
export const stopping = ref(false);
export const streamError = ref("");

export const currentProfile = computed(
  () => profiles.value.find((profile) => profile.id === activeProfileId.value) ?? null,
);

let counter = 0;
let stopRequested = false;

function nextId(): string {
  counter += 1;
  return `msg-${Date.now().toString(36)}-${counter}`;
}

export async function loadProfiles(): Promise<void> {
  profilesLoading.value = true;
  profilesError.value = "";
  try {
    const config = await getSettings();
    profiles.value = config.ai.profiles;
    const selected = config.ai.profiles.find((profile) => profile.id === activeProfileId.value);
    const active = config.ai.profiles.find((profile) => profile.id === config.ai.activeProfileId);
    activeProfileId.value =
      selected?.id ?? active?.id ?? config.ai.profiles[0]?.id ?? null;
  } catch (error) {
    profilesError.value = errorText(error);
  } finally {
    profilesLoading.value = false;
  }
}

export function clearChat() {
  if (streaming.value) return;
  messages.value = [];
  streamError.value = "";
}

export async function stopStreaming(): Promise<void> {
  if (!streaming.value || stopRequested) return;
  stopRequested = true;
  stopping.value = true;
  try {
    await cancelAi();
  } catch {
    stopping.value = false;
  }
}

export async function sendMessage(text: string, context: string): Promise<void> {
  const content = text.trim();
  if (!content || streaming.value) return;
  if (!activeProfileId.value) {
    streamError.value = "还没有可用的 AI 模型配置，请先在设置中添加模型。";
    return;
  }

  streamError.value = "";
  stopRequested = false;
  stopping.value = false;

  messages.value.push({ id: nextId(), role: "user", content, pending: false });

  const history: ChatMessage[] = messages.value
    .filter((entry) => !entry.error && !entry.pending && entry.content.trim())
    .slice(-16)
    .map((entry) => ({ role: entry.role, content: entry.content }));
  const system: ChatMessage = {
    role: "system",
    content: context
      ? `${SYSTEM_PROMPT}\n\n以下是与本次对话相关的本地仓库上下文：\n\n${context}`
      : SYSTEM_PROMPT,
  };

  messages.value.push({ id: nextId(), role: "assistant", content: "", pending: true });
  const target = messages.value[messages.value.length - 1];
  streaming.value = true;

  try {
    await chatStream(activeProfileId.value, [system, ...history], (event) => {
      if (event.type === "delta") {
        if (!stopRequested) target.content += event.text;
      } else if (event.cancelled) {
        target.stopped = true;
      }
    });
    if (stopRequested) target.stopped = true;
  } catch (error) {
    target.error = errorText(error);
    streamError.value = target.error;
  } finally {
    streaming.value = false;
    stopping.value = false;
    target.pending = false;
    if (!target.content && !target.error && !target.stopped) {
      target.content = "（模型没有返回内容）";
    }
  }
}
