<script setup lang="ts">
import { onMounted, reactive, ref } from "vue";
import Icon from "../components/Icon.vue";
import {
  deleteSecret,
  errorText,
  getSettings,
  hasSecret,
  saveSettings,
  setSecret,
  testAi,
  testGitLab,
} from "../lib/api";
import type { AiProfile, AppConfig, GitLabInstance } from "../types";
import { secretAccount } from "../types";

interface Notice {
  kind: "ok" | "error" | "info";
  text: string;
}

const config = ref<AppConfig | null>(null);
const loading = ref(true);
const loadError = ref("");
const saving = ref(false);
const tokens = reactive<Record<string, string>>({});
const savedSecrets = reactive<Record<string, boolean>>({});
const notices = reactive<Record<string, Notice>>({});
const busy = reactive<Record<string, boolean>>({});
const saveNotice = ref<Notice | null>(null);
const pendingDeletes = new Set<string>();

function newId(): string {
  return `cfg-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

function gitlabAccount(instance: GitLabInstance): string {
  return secretAccount("gitlab", instance.id);
}

function aiAccount(profile: AiProfile): string {
  return secretAccount("ai", profile.id);
}

function scrollToSection(id: string) {
  const element = document.getElementById(id);
  if (!element) return;
  const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  element.scrollIntoView({ behavior: reduced ? "auto" : "smooth", block: "start" });
}

onMounted(async () => {
  try {
    const loaded = await getSettings();
    config.value = loaded;
    const accounts = [
      ...loaded.gitlab.instances.map((instance) => gitlabAccount(instance)),
      ...loaded.ai.profiles.map((profile) => aiAccount(profile)),
    ];
    const results = await Promise.all(
      accounts.map(async (account) => [account, await hasSecret(account)] as const),
    );
    for (const [account, exists] of results) savedSecrets[account] = exists;
  } catch (error) {
    loadError.value = `${errorText(error)}。请通过 npm run tauri dev 在 Tauri 中运行应用。`;
  } finally {
    loading.value = false;
  }
});

function addInstance() {
  config.value?.gitlab.instances.push({
    id: newId(),
    name: "GitLab",
    baseUrl: "https://gitlab.example.com",
  });
}

function removeInstance(index: number) {
  if (!config.value) return;
  const instance = config.value.gitlab.instances[index];
  const account = gitlabAccount(instance);
  if (savedSecrets[account]) pendingDeletes.add(account);
  delete savedSecrets[account];
  delete tokens[account];
  delete notices[account];
  config.value.gitlab.instances.splice(index, 1);
}

function addProfile() {
  config.value?.ai.profiles.push({
    id: newId(),
    name: "本地模型",
    baseUrl: "http://127.0.0.1:11434/v1",
    model: "qwen2.5-coder:7b",
    temperature: 0.2,
    maxTokens: 2048,
  });
}

function removeProfile(index: number) {
  if (!config.value) return;
  const profile = config.value.ai.profiles[index];
  const account = aiAccount(profile);
  if (savedSecrets[account]) pendingDeletes.add(account);
  delete savedSecrets[account];
  delete tokens[account];
  delete notices[account];
  config.value.ai.profiles.splice(index, 1);
  if (config.value.ai.activeProfileId === profile.id) {
    config.value.ai.activeProfileId = config.value.ai.profiles[0]?.id ?? null;
  }
}

function setActive(profile: AiProfile) {
  if (!config.value) return;
  config.value.ai.activeProfileId = profile.id;
}

async function runGitLabTest(instance: GitLabInstance) {
  const account = gitlabAccount(instance);
  const baseUrl = instance.baseUrl.trim();
  if (!baseUrl) {
    notices[account] = { kind: "error", text: "请先填写 GitLab 站点地址" };
    return;
  }

  const token = (tokens[account] ?? "").trim();
  const fallbackAccount = !token && savedSecrets[account] ? account : null;
  if (!token && !fallbackAccount) {
    notices[account] = { kind: "error", text: "请填写私人访问令牌，或先保存一次凭据" };
    return;
  }

  busy[account] = true;
  notices[account] = { kind: "info", text: "正在验证…" };
  try {
    const user = await testGitLab(baseUrl, token || null, fallbackAccount);
    notices[account] = {
      kind: "ok",
      text: `连接成功：@${user.username}${user.name ? `（${user.name}）` : ""}`,
    };
  } catch (error) {
    notices[account] = { kind: "error", text: errorText(error) };
  } finally {
    busy[account] = false;
  }
}

async function runAiTest(profile: AiProfile) {
  const account = aiAccount(profile);
  const baseUrl = profile.baseUrl.trim();
  if (!baseUrl) {
    notices[account] = { kind: "error", text: "请先填写模型服务地址" };
    return;
  }

  const key = (tokens[account] ?? "").trim();
  const fallbackAccount = !key && savedSecrets[account] ? account : null;

  busy[account] = true;
  notices[account] = { kind: "info", text: "正在验证…" };
  try {
    const result = await testAi(baseUrl, key || null, fallbackAccount);
    if (!result.models.length) {
      notices[account] = { kind: "ok", text: "连接成功，服务未返回模型列表" };
    } else {
      const preview = result.models.slice(0, 4).join("、");
      const more = result.models.length > 4 ? ` 等 ${result.models.length} 个` : "";
      notices[account] = { kind: "ok", text: `连接成功，可用模型：${preview}${more}` };
    }
  } catch (error) {
    notices[account] = { kind: "error", text: errorText(error) };
  } finally {
    busy[account] = false;
  }
}

async function persist() {
  if (!config.value || saving.value) return;

  const missing = config.value.gitlab.instances.find((instance) => !instance.baseUrl.trim());
  if (missing) {
    saveNotice.value = { kind: "error", text: "存在未填写地址的 GitLab 实例" };
    return;
  }

  for (const profile of config.value.ai.profiles) {
    profile.temperature = Number.isFinite(profile.temperature) ? profile.temperature : 0.2;
    profile.maxTokens =
      Number.isFinite(profile.maxTokens) && profile.maxTokens > 0
        ? Math.round(profile.maxTokens)
        : 2048;
  }
  config.value.git.pageSize = Math.max(50, Math.round(config.value.git.pageSize) || 500);
  config.value.git.maxLanes = Math.max(4, Math.round(config.value.git.maxLanes) || 32);

  saving.value = true;
  saveNotice.value = null;
  let wroteSecret = false;
  try {
    await saveSettings(config.value);
    for (const [account, raw] of Object.entries(tokens)) {
      const value = raw.trim();
      if (!value) continue;
      await setSecret(account, value);
      savedSecrets[account] = true;
      tokens[account] = "";
      wroteSecret = true;
    }
    for (const account of [...pendingDeletes]) {
      await deleteSecret(account);
      pendingDeletes.delete(account);
    }
    saveNotice.value = {
      kind: "ok",
      text: wroteSecret ? "配置已保存，凭据已写入系统钥匙串" : "配置已保存",
    };
  } catch (error) {
    saveNotice.value = { kind: "error", text: errorText(error) };
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <div v-if="loading" class="settings-page settings-loading" role="status">
    <span class="loading-orbit" aria-hidden="true"></span>
    <span>正在读取配置…</span>
  </div>

  <div v-else-if="loadError" class="settings-page">
    <div class="empty-panel">
      <div class="empty-icon"><Icon name="alert" :size="22" /></div>
      <h2>无法加载配置</h2>
      <p>{{ loadError }}</p>
    </div>
  </div>

  <div v-else-if="config" class="settings-page">
    <header class="page-head">
      <div class="page-heading">
        <h1>设置</h1>
        <p>连接 GitLab、配置本地模型，以及调整图谱读取偏好。</p>
      </div>
      <span class="keychain-note">
        <Icon name="lock" :size="14" />
        凭据保存在系统钥匙串
      </span>
    </header>

    <div class="settings-layout">
      <aside class="settings-index" aria-label="设置分区">
        <span class="section-label">设置项</span>
        <button type="button" class="index-link" @click="scrollToSection('gitlab-settings')">
          <Icon name="folder" :size="15" />
          GitLab 实例
          <span>{{ config.gitlab.instances.length }}</span>
        </button>
        <button type="button" class="index-link" @click="scrollToSection('ai-settings')">
          <Icon name="sparkles" :size="15" />
          AI 模型
          <span>{{ config.ai.profiles.length }}</span>
        </button>
        <button type="button" class="index-link" @click="scrollToSection('graph-settings')">
          <Icon name="branch" :size="15" />
          图谱偏好
        </button>
        <p>非敏感配置保存在本机，令牌与 API Key 不写入配置文件。</p>
      </aside>

      <div class="settings-content">
        <section id="gitlab-settings" class="settings-section">
          <div class="settings-section-head">
            <div>
              <h2>GitLab 实例</h2>
              <p>支持 gitlab.com 与自建实例。填写站点根地址即可，请求会自动走 /api/v4。</p>
            </div>
            <button type="button" class="button ghost small" @click="addInstance">
              <Icon name="plus" :size="15" />
              添加实例
            </button>
          </div>

          <div v-if="!config.gitlab.instances.length" class="settings-empty">
            <div class="empty-icon"><Icon name="folder" :size="20" /></div>
            <div>
              <strong>还没有 GitLab 实例</strong>
              <p>添加一个实例后，就可以在合并请求页浏览和管理 MR。</p>
            </div>
            <button type="button" class="button primary small" @click="addInstance">
              添加第一个实例
            </button>
          </div>

          <article
            v-for="(instance, index) in config.gitlab.instances"
            :key="instance.id"
            class="settings-item"
          >
            <header class="settings-item-head">
              <span class="settings-item-mark gitlab" aria-hidden="true">
                <Icon name="folder" :size="17" />
              </span>
              <div class="settings-item-title">
                <strong>{{ instance.name || "未命名实例" }}</strong>
                <span>{{ instance.baseUrl || "尚未填写地址" }}</span>
              </div>
              <span v-if="savedSecrets[gitlabAccount(instance)]" class="tag success">
                <span class="status-dot"></span>
                令牌已保存
              </span>
              <span v-else class="tag muted">无令牌</span>
              <button
                type="button"
                class="button danger-quiet tiny"
                @click="removeInstance(index)"
              >
                移除
              </button>
            </header>

            <div class="form-grid">
              <label class="field">
                <span>名称</span>
                <input v-model="instance.name" placeholder="公司 GitLab" />
              </label>
              <label class="field">
                <span>站点地址</span>
                <input v-model="instance.baseUrl" placeholder="https://gitlab.example.com" />
              </label>
              <label class="field">
                <span>私人访问令牌</span>
                <input
                  v-model="tokens[gitlabAccount(instance)]"
                  type="password"
                  autocomplete="new-password"
                  placeholder="glpat-…（留空则使用已保存凭据）"
                />
              </label>
            </div>

            <footer class="settings-item-foot">
              <button
                type="button"
                class="button ghost small"
                :disabled="busy[gitlabAccount(instance)]"
                @click="runGitLabTest(instance)"
              >
                {{ busy[gitlabAccount(instance)] ? "验证中…" : "测试连接" }}
              </button>
              <span>需要 read_api 权限，令牌仅保存在系统钥匙串。</span>
            </footer>

            <p
              v-if="notices[gitlabAccount(instance)]"
              class="alert"
              :class="notices[gitlabAccount(instance)].kind"
            >
              {{ notices[gitlabAccount(instance)].text }}
            </p>
          </article>
        </section>

        <section id="ai-settings" class="settings-section">
          <div class="settings-section-head">
            <div>
              <h2>AI 模型</h2>
              <p>
                使用 OpenAI 兼容协议，支持 Ollama、LM Studio、llama.cpp、vLLM 等本地服务；
                云端服务同样兼容，API Key 可选。
              </p>
            </div>
            <button type="button" class="button ghost small" @click="addProfile">
              <Icon name="plus" :size="15" />
              添加模型
            </button>
          </div>

          <div v-if="!config.ai.profiles.length" class="settings-empty">
            <div class="empty-icon"><Icon name="sparkles" :size="20" /></div>
            <div>
              <strong>还没有 AI 模型配置</strong>
              <p>默认推荐本地 Ollama：http://127.0.0.1:11434/v1。</p>
            </div>
            <button type="button" class="button primary small" @click="addProfile">
              添加第一个模型
            </button>
          </div>

          <article v-for="(profile, index) in config.ai.profiles" :key="profile.id" class="settings-item">
            <header class="settings-item-head">
              <span class="settings-item-mark ai" aria-hidden="true">
                <Icon name="sparkles" :size="17" />
              </span>
              <div class="settings-item-title">
                <strong>{{ profile.name || "未命名模型" }}</strong>
                <span>{{ profile.model || "尚未填写模型 ID" }}</span>
              </div>
              <span v-if="config.ai.activeProfileId === profile.id" class="tag success">
                <span class="status-dot"></span>
                当前使用
              </span>
              <button v-else type="button" class="button ghost tiny" @click="setActive(profile)">
                设为当前
              </button>
              <button type="button" class="button danger-quiet tiny" @click="removeProfile(index)">
                移除
              </button>
            </header>

            <div class="form-grid">
              <label class="field">
                <span>名称</span>
                <input v-model="profile.name" placeholder="Ollama (本地)" />
              </label>
              <label class="field">
                <span>服务地址</span>
                <input v-model="profile.baseUrl" placeholder="http://127.0.0.1:11434/v1" />
              </label>
              <label class="field">
                <span>模型 ID</span>
                <input v-model="profile.model" placeholder="qwen2.5-coder:7b" />
              </label>
              <label class="field">
                <span>温度</span>
                <input v-model.number="profile.temperature" type="number" min="0" max="2" step="0.1" />
              </label>
              <label class="field">
                <span>最大 Token</span>
                <input v-model.number="profile.maxTokens" type="number" min="256" step="256" />
              </label>
              <label class="field">
                <span>API Key（可选）</span>
                <input
                  v-model="tokens[aiAccount(profile)]"
                  type="password"
                  autocomplete="new-password"
                  placeholder="本地服务通常留空"
                />
              </label>
            </div>

            <footer class="settings-item-foot">
              <button
                type="button"
                class="button ghost small"
                :disabled="busy[aiAccount(profile)]"
                @click="runAiTest(profile)"
              >
                {{ busy[aiAccount(profile)] ? "验证中…" : "测试连接" }}
              </button>
              <span>
                {{
                  savedSecrets[aiAccount(profile)]
                    ? "已保存 API Key，留空将复用。"
                    : "通过 GET /models 验证服务可用性。"
                }}
              </span>
            </footer>

            <p
              v-if="notices[aiAccount(profile)]"
              class="alert"
              :class="notices[aiAccount(profile)].kind"
            >
              {{ notices[aiAccount(profile)].text }}
            </p>
          </article>
        </section>

        <section id="graph-settings" class="settings-section">
          <div class="settings-section-head">
            <div>
              <h2>图谱偏好</h2>
              <p>控制提交图谱的分页与泳道上限，在打开仓库后生效。</p>
            </div>
          </div>

          <div class="settings-item">
            <div class="form-grid two">
              <label class="field">
                <span>每页提交数</span>
                <input v-model.number="config.git.pageSize" type="number" min="50" step="50" />
              </label>
              <label class="field">
                <span>最大泳道数</span>
                <input v-model.number="config.git.maxLanes" type="number" min="4" max="64" />
              </label>
            </div>
            <footer class="settings-item-foot">
              <span>提交越多，首次读取越慢；泳道上限用于控制图谱的横向宽度。</span>
            </footer>
          </div>
        </section>

        <div class="save-bar">
          <button type="button" class="button primary" :disabled="saving" @click="persist">
            {{ saving ? "保存中…" : "保存配置" }}
          </button>
          <span v-if="saveNotice" class="alert" :class="saveNotice.kind">{{ saveNotice.text }}</span>
          <span v-else class="muted-copy">保存后立即生效。</span>
        </div>
      </div>
    </div>
  </div>
</template>
