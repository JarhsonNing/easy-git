<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import Icon from "../components/Icon.vue";
import {
  clearSshPassphrase,
  deleteSecret,
  deleteSshKey,
  errorText,
  generateSshKey,
  getGitIdentity,
  getSettings,
  hasSecret,
  listSshKeys,
  saveGitIdentity,
  saveSettings,
  setSecret,
  setSshPassphrase,
  testAi,
  testGitLab,
  testSshConnection,
  trustSshHost,
} from "../lib/api";
import type {
  AiProfile,
  AppConfig,
  GitIdentity,
  GitLabInstance,
  SshKeyInfo,
  SshTestResult,
  SshTestStatus,
} from "../types";
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

const sshKeys = ref<SshKeyInfo[]>([]);
const sshKeysLoading = ref(true);
const sshKeysError = ref("");
const sshSectionNotice = ref<Notice | null>(null);
const sshNotices = reactive<Record<string, Notice>>({});
const sshBusy = reactive<Record<string, boolean>>({});
const sshExpanded = ref<string | null>(null);
const sshDeleteConfirm = ref<string | null>(null);
const sshPassphraseFor = ref<string | null>(null);
const sshPassphraseInput = ref("");
const sshFormOpen = ref(false);
const sshForm = reactive({ name: "", type: "ed25519", comment: "easy-git", passphrase: "" });
const sshFormBusy = ref(false);
const sshFormNotice = ref<Notice | null>(null);

const sshTestOpen = ref(false);
const sshHostInput = ref<HTMLInputElement | null>(null);
const sshTest = reactive({
  host: "",
  port: 22,
  user: "git",
  keyName: "",
  busy: false,
  error: "",
  result: null as SshTestResult | null,
  trustType: "",
  trustBusy: false,
  trustError: "",
});

const gitIdentity = ref<GitIdentity>({ name: null, email: null });
const identityForm = reactive({ name: "", email: "" });
const identityBusy = ref(false);
const identityNotice = ref<Notice | null>(null);

const identityConfigured = computed(
  () => Boolean(gitIdentity.value.name && gitIdentity.value.email),
);
const identityMissing = computed(() => {
  const name = identityForm.name.trim();
  const email = identityForm.email.trim();
  if (!name && !email) return "还没有配置提交身份，提交会被 Git 拒绝。";
  if (!name) return "还缺用户名：Git 需要用户名和邮箱同时存在。";
  if (!email) return "还缺邮箱：Git 需要用户名和邮箱同时存在。";
  return "";
});

async function loadGitIdentity() {
  try {
    const loaded = await getGitIdentity();
    gitIdentity.value = loaded;
    identityForm.name = loaded.name ?? "";
    identityForm.email = loaded.email ?? "";
  } catch (error) {
    identityNotice.value = { kind: "error", text: errorText(error) };
  }
}

async function saveIdentity() {
  if (identityBusy.value) return;
  identityNotice.value = null;
  identityBusy.value = true;
  try {
    const saved = await saveGitIdentity(identityForm.name, identityForm.email);
    gitIdentity.value = saved;
    identityForm.name = saved.name ?? "";
    identityForm.email = saved.email ?? "";
    identityNotice.value = {
      kind: "ok",
      text: "已写入全局 Git 配置，本机所有仓库都会使用这个提交身份。",
    };
  } catch (error) {
    identityNotice.value = { kind: "error", text: errorText(error) };
  } finally {
    identityBusy.value = false;
  }
}

const selectedSshKey = computed(() => {
  const name = config.value?.git.sshKeyName;
  if (!name) return null;
  return sshKeys.value.find((key) => key.name === name) ?? null;
});

const selectedSshKeyMissing = computed(
  () =>
    Boolean(config.value?.git.sshKeyName) &&
    !sshKeysLoading.value &&
    !sshKeysError.value &&
    !selectedSshKey.value,
);

const SSH_STATUS_LABEL: Record<SshTestStatus, string> = {
  ok: "连接成功",
  unknownHost: "主机密钥待确认",
  hostKeyChanged: "主机密钥已变更",
  authFailed: "认证失败",
  unreachable: "无法连接",
  error: "连接失败",
};

function sshResultTone(status: SshTestStatus): Notice["kind"] {
  if (status === "ok") return "ok";
  if (status === "unknownHost" || status === "hostKeyChanged") return "info";
  return "error";
}

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
  void loadSshKeys();
  void loadGitIdentity();
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

async function loadSshKeys(showSpinner = true) {
  if (showSpinner) sshKeysLoading.value = true;
  sshKeysError.value = "";
  try {
    sshKeys.value = await listSshKeys();
  } catch (error) {
    sshKeysError.value = errorText(error);
  } finally {
    sshKeysLoading.value = false;
  }
}

function suggestSshName(type: string): string {
  const base = type === "rsa" ? "id_rsa" : "id_ed25519";
  const used = new Set(sshKeys.value.map((key) => key.name));
  if (!used.has(base)) return base;
  if (!used.has(`${base}_easygit`)) return `${base}_easygit`;
  for (let index = 2; index < 100; index += 1) {
    const candidate = `${base}_${index}`;
    if (!used.has(candidate)) return candidate;
  }
  return `${base}_${Date.now().toString(36)}`;
}

function toggleSshForm() {
  if (!sshFormOpen.value) {
    sshForm.name = suggestSshName("ed25519");
    sshForm.type = "ed25519";
    sshForm.comment = "easy-git";
    sshForm.passphrase = "";
    sshFormNotice.value = null;
  }
  sshFormOpen.value = !sshFormOpen.value;
}

function onSshTypeChange() {
  sshForm.name = suggestSshName(sshForm.type);
}

async function createSshKey() {
  if (!config.value || sshFormBusy.value) return;
  const name = sshForm.name.trim();
  if (!name) {
    sshFormNotice.value = { kind: "error", text: "请填写密钥名称" };
    return;
  }
  sshFormBusy.value = true;
  sshFormNotice.value = null;
  try {
    const key = await generateSshKey(
      name,
      sshForm.type,
      sshForm.comment,
      sshForm.passphrase || null,
    );
    sshForm.passphrase = "";
    sshFormOpen.value = false;
    await loadSshKeys(false);
    config.value.git.sshKeyName = key.name;
    sshExpanded.value = key.name;
    sshNotices[key.name] = {
      kind: "ok",
      text: "密钥已生成，并选为 Git 使用的密钥。点击底部「保存配置」后生效。",
    };
  } catch (error) {
    sshFormNotice.value = { kind: "error", text: errorText(error) };
  } finally {
    sshFormBusy.value = false;
  }
}

function isSelectedSshKey(key: SshKeyInfo): boolean {
  return config.value?.git.sshKeyName === key.name;
}

function selectSshKey(key: SshKeyInfo) {
  if (!config.value) return;
  config.value.git.sshKeyName = key.name;
  sshNotices[key.name] = {
    kind: "info",
    text: "已选为 Git 使用的密钥。点击底部「保存配置」后生效。",
  };
}

function useDefaultSsh() {
  if (!config.value) return;
  config.value.git.sshKeyName = null;
  sshSectionNotice.value = {
    kind: "info",
    text: "已改用 SSH 默认配置。点击底部「保存配置」后生效。",
  };
}

async function copySshPublicKey(key: SshKeyInfo) {
  try {
    await navigator.clipboard.writeText(key.publicKey);
    sshNotices[key.name] = {
      kind: "ok",
      text: "公钥已复制，可粘贴到 GitLab 的 SSH 密钥页。",
    };
  } catch {
    sshNotices[key.name] = { kind: "error", text: "复制失败，请展开公钥手动选择文本。" };
  }
}

function toggleSshPublicKey(key: SshKeyInfo) {
  sshExpanded.value = sshExpanded.value === key.name ? null : key.name;
}

function startSshPassphrase(key: SshKeyInfo) {
  sshPassphraseFor.value = key.name;
  sshPassphraseInput.value = "";
  sshDeleteConfirm.value = null;
}

async function saveSshPassphrase(key: SshKeyInfo) {
  if (sshBusy[key.name]) return;
  if (!sshPassphraseInput.value) {
    sshNotices[key.name] = { kind: "error", text: "请填写密钥口令" };
    return;
  }
  sshBusy[key.name] = true;
  try {
    await setSshPassphrase(key.name, sshPassphraseInput.value);
    sshPassphraseFor.value = null;
    sshPassphraseInput.value = "";
    await loadSshKeys(false);
    sshNotices[key.name] = {
      kind: "ok",
      text: "口令已保存到系统钥匙串，Git 连接时会自动提供给 ssh。",
    };
  } catch (error) {
    sshNotices[key.name] = { kind: "error", text: errorText(error) };
  } finally {
    sshBusy[key.name] = false;
  }
}

async function forgetSshPassphrase(key: SshKeyInfo) {
  if (sshBusy[key.name]) return;
  sshBusy[key.name] = true;
  try {
    await clearSshPassphrase(key.name);
    await loadSshKeys(false);
    sshNotices[key.name] = {
      kind: "info",
      text: "已从钥匙串移除口令。如果密钥仍受口令保护，需要重新保存口令或使用 ssh-agent。",
    };
  } catch (error) {
    sshNotices[key.name] = { kind: "error", text: errorText(error) };
  } finally {
    sshBusy[key.name] = false;
  }
}

async function removeSshKey(key: SshKeyInfo) {
  if (sshBusy[key.name]) return;
  sshBusy[key.name] = true;
  try {
    await deleteSshKey(key.name);
    if (config.value?.git.sshKeyName === key.name) {
      config.value.git.sshKeyName = null;
    }
    sshDeleteConfirm.value = null;
    if (sshExpanded.value === key.name) sshExpanded.value = null;
    if (sshPassphraseFor.value === key.name) sshPassphraseFor.value = null;
    delete sshNotices[key.name];
    await loadSshKeys(false);
    sshSectionNotice.value = {
      kind: "ok",
      text: `已删除密钥 ${key.name} 及其公钥文件。`,
    };
  } catch (error) {
    sshNotices[key.name] = { kind: "error", text: errorText(error) };
  } finally {
    sshBusy[key.name] = false;
  }
}

function keyTypeLabel(key: SshKeyInfo): string {
  const type = key.keyType.toUpperCase();
  if (type.includes("ED25519")) return "Ed25519";
  if (type.includes("RSA")) return "RSA";
  if (type.includes("ECDSA")) return "ECDSA";
  return key.keyType || "SSH";
}

async function openGitLabSshKeys(instance: GitLabInstance) {
  const base = instance.baseUrl.trim().replace(/\/+$/, "");
  if (!base) {
    sshSectionNotice.value = { kind: "error", text: "这个 GitLab 实例还没有填写地址" };
    return;
  }
  const key = selectedSshKey.value ?? sshKeys.value[0];
  if (key) {
    try {
      await navigator.clipboard.writeText(key.publicKey);
      sshSectionNotice.value = {
        kind: "ok",
        text: `已复制 ${key.name} 的公钥，在 GitLab 的 SSH 密钥页粘贴即可。`,
      };
    } catch {
      // Opening the page is still useful when the clipboard is unavailable.
    }
  }
  try {
    await openUrl(`${base}/-/profile/keys`);
  } catch (error) {
    sshSectionNotice.value = { kind: "error", text: errorText(error) };
  }
}

function openSshTest(key?: SshKeyInfo) {
  let host = "";
  const instance = config.value?.gitlab.instances[0];
  if (instance) {
    try {
      host = new URL(instance.baseUrl).hostname;
    } catch {
      host = "";
    }
  }
  sshTest.host = host;
  sshTest.port = 22;
  sshTest.user = "git";
  sshTest.keyName = key?.name ?? config.value?.git.sshKeyName ?? "";
  sshTest.busy = false;
  sshTest.error = "";
  sshTest.result = null;
  sshTest.trustType = "";
  sshTest.trustBusy = false;
  sshTest.trustError = "";
  sshTestOpen.value = true;
  void nextTick(() => sshHostInput.value?.focus());
}

function closeSshTest() {
  if (sshTest.busy) return;
  sshTestOpen.value = false;
}

async function runSshTest() {
  if (sshTest.busy) return;
  const host = sshTest.host.trim();
  if (!host) {
    sshTest.error = "请填写主机地址";
    return;
  }
  const port = Number(sshTest.port);
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    sshTest.error = "端口必须在 1–65535 之间";
    return;
  }
  sshTest.busy = true;
  sshTest.error = "";
  sshTest.trustError = "";
  sshTest.result = null;
  try {
    const result = await testSshConnection(
      host,
      port,
      sshTest.user.trim(),
      sshTest.keyName || null,
    );
    sshTest.result = result;
    sshTest.trustType = result.hostKeys[0]?.keyType ?? "";
  } catch (error) {
    sshTest.error = errorText(error);
  } finally {
    sshTest.busy = false;
  }
}

async function trustHostAndRetry() {
  if (!sshTest.result || !sshTest.trustType || sshTest.trustBusy) return;
  sshTest.trustBusy = true;
  sshTest.trustError = "";
  try {
    await trustSshHost(sshTest.host.trim(), Number(sshTest.port), sshTest.trustType);
    await runSshTest();
  } catch (error) {
    sshTest.trustError = errorText(error);
  } finally {
    sshTest.trustBusy = false;
  }
}

function onGlobalKeydown(event: KeyboardEvent) {
  if (event.key === "Escape" && sshTestOpen.value) closeSshTest();
}

onMounted(() => {
  window.addEventListener("keydown", onGlobalKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onGlobalKeydown);
});

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
  if (
    config.value.git.sshKeyName &&
    !sshKeysLoading.value &&
    !sshKeysError.value &&
    !sshKeys.value.some((key) => key.name === config.value?.git.sshKeyName)
  ) {
    config.value.git.sshKeyName = null;
  }

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
        <p>设置提交身份、连接 GitLab、配置本地模型，以及调整图谱读取偏好。</p>
      </div>
      <span class="keychain-note">
        <Icon name="lock" :size="14" />
        凭据保存在系统钥匙串
      </span>
    </header>

    <div class="settings-layout">
      <aside class="settings-index" aria-label="设置分区">
        <span class="section-label">设置项</span>
        <button type="button" class="index-link" @click="scrollToSection('identity-settings')">
          <Icon name="user" :size="15" />
          提交身份
        </button>
        <button type="button" class="index-link" @click="scrollToSection('gitlab-settings')">
          <Icon name="folder" :size="15" />
          GitLab 实例
          <span>{{ config.gitlab.instances.length }}</span>
        </button>
        <button type="button" class="index-link" @click="scrollToSection('ssh-settings')">
          <Icon name="key" :size="15" />
          SSH 密钥
          <span>{{ sshKeys.length }}</span>
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
        <section id="identity-settings" class="settings-section">
          <div class="settings-section-head">
            <div>
              <h2>提交身份</h2>
              <p>Git 用这里的用户名和邮箱标记每一次提交。未配置时提交会被 Git 拒绝。</p>
            </div>
            <span v-if="identityConfigured" class="tag success">
              <span class="status-dot"></span>
              已配置
            </span>
            <span v-else class="tag muted">未配置</span>
          </div>

          <article class="settings-item">
            <div class="form-grid two">
              <label class="field">
                <span>用户名</span>
                <input
                  v-model="identityForm.name"
                  placeholder="Ada Lovelace"
                  autocomplete="name"
                  spellcheck="false"
                />
              </label>
              <label class="field">
                <span>邮箱</span>
                <input
                  v-model="identityForm.email"
                  type="email"
                  placeholder="ada@example.com"
                  autocomplete="email"
                  spellcheck="false"
                  @keydown.enter.prevent="saveIdentity"
                />
              </label>
            </div>

            <p v-if="identityMissing" class="alert info">{{ identityMissing }}</p>

            <footer class="settings-item-foot">
              <button
                type="button"
                class="button primary small"
                :disabled="identityBusy"
                @click="saveIdentity"
              >
                {{ identityBusy ? "保存中…" : "保存身份" }}
              </button>
              <span>
                写入全局 Git 配置（~/.gitconfig），对本机所有仓库生效；仓库内单独设置的身份优先。
              </span>
            </footer>

            <p v-if="identityNotice" class="alert" :class="identityNotice.kind">
              {{ identityNotice.text }}
            </p>
          </article>
        </section>

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

        <section id="ssh-settings" class="settings-section">
          <div class="settings-section-head">
            <div>
              <h2>SSH 密钥</h2>
              <p>
                生成、查看与测试本机的 SSH 密钥。选中的密钥会用于所有 Git 抓取、拉取与推送；
                口令保存在系统钥匙串，不写入配置文件。
              </p>
            </div>
            <div class="settings-section-actions">
              <button
                v-if="config.git.sshKeyName"
                type="button"
                class="button ghost small"
                @click="useDefaultSsh"
              >
                改用 SSH 默认配置
              </button>
              <button type="button" class="button ghost small" @click="toggleSshForm">
                <Icon :name="sshFormOpen ? 'close' : 'plus'" :size="15" />
                {{ sshFormOpen ? "收起生成表单" : "生成密钥" }}
              </button>
            </div>
          </div>

          <p v-if="sshSectionNotice" class="alert" :class="sshSectionNotice.kind">
            {{ sshSectionNotice.text }}
          </p>

          <p v-if="selectedSshKeyMissing" class="alert info">
            配置里选中的密钥「{{ config.git.sshKeyName }}」不在 SSH 目录中。Git 操作会回退到
            SSH 默认配置，请重新选择或生成密钥。
          </p>

          <article v-if="sshFormOpen" class="settings-item ssh-generate">
            <header class="settings-item-head">
              <span class="settings-item-mark ssh" aria-hidden="true">
                <Icon name="key" :size="17" />
              </span>
              <div class="settings-item-title">
                <strong>生成新的 SSH 密钥</strong>
                <span>写入 ~/.ssh，同名文件不会被覆盖</span>
              </div>
              <button
                type="button"
                class="icon-button"
                aria-label="关闭生成表单"
                @click="sshFormOpen = false"
              >
                <Icon name="close" :size="15" />
              </button>
            </header>

            <div class="form-grid">
              <label class="field">
                <span>名称</span>
                <input v-model="sshForm.name" placeholder="id_ed25519" spellcheck="false" />
              </label>
              <label class="field">
                <span>类型</span>
                <select v-model="sshForm.type" @change="onSshTypeChange">
                  <option value="ed25519">Ed25519（推荐）</option>
                  <option value="rsa">RSA 4096</option>
                </select>
              </label>
              <label class="field">
                <span>注释</span>
                <input v-model="sshForm.comment" placeholder="easy-git" spellcheck="false" />
              </label>
              <label class="field">
                <span>口令（可选）</span>
                <input
                  v-model="sshForm.passphrase"
                  type="password"
                  autocomplete="new-password"
                  placeholder="留空表示无口令"
                />
              </label>
            </div>

            <p class="ssh-form-note">
              Ed25519 兼容性最好；RSA 固定 4096 位。口令只保存在系统钥匙串，Git
              连接时由应用自动提供给 ssh。
            </p>

            <footer class="settings-item-foot">
              <button
                type="button"
                class="button primary small"
                :disabled="sshFormBusy"
                @click="createSshKey"
              >
                {{ sshFormBusy ? "生成中…" : "生成密钥" }}
              </button>
              <span>生成后会自动选为 Git 使用的密钥。</span>
            </footer>

            <p v-if="sshFormNotice" class="alert" :class="sshFormNotice.kind">
              {{ sshFormNotice.text }}
            </p>
          </article>

          <div v-if="sshKeysLoading" class="settings-empty" role="status">
            <span class="loading-orbit" aria-hidden="true"></span>
            <div>
              <strong>正在读取 SSH 目录</strong>
              <p>枚举 ~/.ssh 下的私钥与对应公钥。</p>
            </div>
          </div>

          <div v-else-if="sshKeysError" class="settings-empty ssh-empty-error">
            <div class="empty-icon"><Icon name="alert" :size="20" /></div>
            <div>
              <strong>无法读取 SSH 密钥</strong>
              <p>{{ sshKeysError }}</p>
            </div>
            <button type="button" class="button ghost small" @click="loadSshKeys()">
              <Icon name="refresh" :size="13" />
              重试
            </button>
          </div>

          <div v-else-if="!sshKeys.length" class="settings-empty">
            <div class="empty-icon"><Icon name="key" :size="20" /></div>
            <div>
              <strong>还没有 SSH 密钥</strong>
              <p>生成一个 Ed25519 密钥，把公钥粘贴到 GitLab，然后选用它。</p>
            </div>
            <button type="button" class="button primary small" @click="toggleSshForm">
              生成第一个密钥
            </button>
          </div>

          <template v-else>
            <article
              v-for="key in sshKeys"
              :key="key.name"
              class="settings-item ssh-key-card"
              :class="{ 'ssh-key-current': isSelectedSshKey(key) }"
            >
            <header class="settings-item-head">
              <span class="settings-item-mark ssh" aria-hidden="true">
                <Icon name="key" :size="17" />
              </span>
              <div class="settings-item-title">
                <strong>{{ key.name }}</strong>
                <span>{{ key.path }}</span>
              </div>
              <span v-if="isSelectedSshKey(key)" class="tag success">
                <span class="status-dot"></span>
                Git 使用中
              </span>
              <span class="tag muted">{{ keyTypeLabel(key) }} {{ key.bits || "" }}</span>
              <span v-if="key.encrypted" class="tag warn">
                <Icon name="lock" :size="11" />
                {{ key.hasPassphrase ? "口令已保存" : "有口令" }}
              </span>
              <span v-else class="tag muted">无口令</span>
            </header>

            <div class="ssh-key-meta">
              <p>
                <span>指纹</span>
                <code>{{ key.fingerprint || "—" }}</code>
              </p>
              <p>
                <span>注释</span>
                <code>{{ key.comment || "（无）" }}</code>
              </p>
            </div>

            <p
              v-if="isSelectedSshKey(key) && key.encrypted && !key.hasPassphrase"
              class="alert info ssh-key-warning"
            >
              这个密钥有口令，但应用还没有保存它。Git 连接会因无法输入口令而失败；请保存口令，
              或先把密钥加入 ssh-agent。
            </p>

            <pre v-if="sshExpanded === key.name" class="ssh-public-key" spellcheck="false">{{
              key.publicKey
            }}</pre>

            <div v-if="sshPassphraseFor === key.name" class="ssh-inline-form">
              <label class="field">
                <span>密钥口令</span>
                <input
                  v-model="sshPassphraseInput"
                  type="password"
                  autocomplete="new-password"
                  placeholder="输入这个私钥的口令"
                  @keydown.enter.prevent="saveSshPassphrase(key)"
                />
              </label>
              <button
                type="button"
                class="button primary tiny"
                :disabled="sshBusy[key.name] || !sshPassphraseInput"
                @click="saveSshPassphrase(key)"
              >
                {{ sshBusy[key.name] ? "保存中…" : "保存口令" }}
              </button>
              <button type="button" class="button ghost tiny" @click="sshPassphraseFor = null">
                取消
              </button>
            </div>

            <div v-if="sshDeleteConfirm === key.name" class="ssh-delete-confirm">
              <p>
                删除 <strong>{{ key.name }}</strong> 的私钥与公钥文件？这个操作无法撤销。
              </p>
              <button
                type="button"
                class="button danger tiny"
                :disabled="sshBusy[key.name]"
                @click="removeSshKey(key)"
              >
                {{ sshBusy[key.name] ? "删除中…" : "确认删除" }}
              </button>
              <button type="button" class="button ghost tiny" @click="sshDeleteConfirm = null">
                取消
              </button>
            </div>

            <footer class="settings-item-foot ssh-key-actions">
              <button type="button" class="button ghost tiny" @click="copySshPublicKey(key)">
                <Icon name="copy" :size="12" />
                复制公钥
              </button>
              <button type="button" class="button ghost tiny" @click="toggleSshPublicKey(key)">
                {{ sshExpanded === key.name ? "收起公钥" : "查看公钥" }}
              </button>
              <button type="button" class="button ghost tiny" @click="openSshTest(key)">
                <Icon name="external" :size="12" />
                测试连接
              </button>
              <button
                v-if="!isSelectedSshKey(key)"
                type="button"
                class="button ghost tiny"
                @click="selectSshKey(key)"
              >
                设为 Git 密钥
              </button>
              <button
                v-else
                type="button"
                class="button ghost tiny"
                @click="useDefaultSsh"
              >
                取消使用
              </button>
              <button
                v-if="key.encrypted && !key.hasPassphrase"
                type="button"
                class="button ghost tiny"
                @click="startSshPassphrase(key)"
              >
                保存口令
              </button>
              <button
                v-if="key.hasPassphrase"
                type="button"
                class="button ghost tiny"
                @click="forgetSshPassphrase(key)"
              >
                清除口令
              </button>
              <button
                type="button"
                class="button danger-quiet tiny"
                @click="sshDeleteConfirm = key.name"
              >
                删除
              </button>
            </footer>

            <p v-if="sshNotices[key.name]" class="alert" :class="sshNotices[key.name].kind">
              {{ sshNotices[key.name].text }}
            </p>
            </article>
          </template>

          <div v-if="sshKeys.length && config.gitlab.instances.length" class="ssh-gitlab-links">
            <span>复制公钥后，到 GitLab 的 SSH 密钥页粘贴：</span>
            <button
              v-for="instance in config.gitlab.instances"
              :key="instance.id"
              type="button"
              class="button ghost tiny"
              @click="openGitLabSshKeys(instance)"
            >
              <Icon name="external" :size="12" />
              {{ instance.name || instance.baseUrl || "GitLab" }}
            </button>
          </div>
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

  <Teleport to="body">
    <div v-if="sshTestOpen" class="dialog-layer" @pointerdown.self="closeSshTest">
      <section class="action-dialog ssh-test-dialog" role="dialog" aria-modal="true" aria-label="测试 SSH 连接">
        <header class="dialog-head">
          <div>
            <h2>测试 SSH 连接</h2>
            <p>用选中的私钥连接主机，并在首次连接时核对服务器指纹。</p>
          </div>
          <button type="button" class="icon-button dialog-close" aria-label="关闭" @click="closeSshTest">
            <Icon name="close" :size="15" />
          </button>
        </header>

        <div class="dialog-body">
          <div class="form-grid two">
            <label class="field">
              <span>主机</span>
              <input
                ref="sshHostInput"
                v-model="sshTest.host"
                placeholder="gitlab.example.com"
                autocomplete="off"
                spellcheck="false"
                @keydown.enter.prevent="runSshTest"
              />
            </label>
            <label class="field">
              <span>端口</span>
              <input v-model.number="sshTest.port" type="number" min="1" max="65535" />
            </label>
            <label class="field">
              <span>用户名</span>
              <input v-model="sshTest.user" placeholder="git" autocomplete="off" spellcheck="false" />
            </label>
            <label class="field">
              <span>私钥</span>
              <select v-model="sshTest.keyName">
                <option value="">使用 SSH 默认配置</option>
                <option v-for="key in sshKeys" :key="key.name" :value="key.name">
                  {{ key.name }}
                </option>
              </select>
            </label>
          </div>

          <p v-if="sshTest.error" class="alert error">{{ sshTest.error }}</p>

          <template v-if="sshTest.result">
            <p class="alert" :class="sshResultTone(sshTest.result.status)">
              {{ SSH_STATUS_LABEL[sshTest.result.status] }}
            </p>
            <pre v-if="sshTest.result.message" class="ssh-test-output">{{ sshTest.result.message }}</pre>

            <div
              v-if="sshTest.result.status === 'unknownHost' && sshTest.result.hostKeys.length"
              class="ssh-hostkey-panel"
            >
              <strong>这是第一次连接这台主机</strong>
              <p>
                请确认下面的指纹与服务器管理员提供的一致，再决定是否信任。信任后会写入
                ~/.ssh/known_hosts 并重新测试。
              </p>
              <div class="ssh-hostkey-list">
                <label
                  v-for="entry in sshTest.result.hostKeys"
                  :key="entry.keyType"
                  class="ssh-hostkey-option"
                  :class="{ checked: sshTest.trustType === entry.keyType }"
                >
                  <input v-model="sshTest.trustType" type="radio" :value="entry.keyType" />
                  <span>
                    <strong>{{ entry.keyType }}</strong>
                    <code>{{ entry.fingerprint }}</code>
                  </span>
                </label>
              </div>
              <p v-if="sshTest.trustError" class="alert error">{{ sshTest.trustError }}</p>
              <button
                type="button"
                class="button primary small"
                :disabled="sshTest.trustBusy || !sshTest.trustType"
                @click="trustHostAndRetry"
              >
                {{ sshTest.trustBusy ? "写入中…" : "信任并重新测试" }}
              </button>
            </div>

            <div
              v-else-if="sshTest.result.status === 'hostKeyChanged'"
              class="ssh-hostkey-panel danger"
            >
              <strong>服务器出示的密钥与之前不同</strong>
              <p>
                可能是服务器重装或更换了密钥，也可能是中间人攻击。应用不会自动覆盖旧记录；
                请先核对下面的新指纹，必要时手动编辑 ~/.ssh/known_hosts。
              </p>
              <div class="ssh-hostkey-list">
                <div v-for="entry in sshTest.result.hostKeys" :key="entry.keyType" class="ssh-hostkey-option">
                  <span>
                    <strong>{{ entry.keyType }}</strong>
                    <code>{{ entry.fingerprint }}</code>
                  </span>
                </div>
              </div>
            </div>
          </template>
        </div>

        <footer class="dialog-foot">
          <button type="button" class="button ghost" @click="closeSshTest">关闭</button>
          <button
            type="button"
            class="button primary"
            :disabled="sshTest.busy"
            @click="runSshTest"
          >
            {{ sshTest.busy ? "测试中…" : "测试连接" }}
          </button>
        </footer>
      </section>
    </div>
  </Teleport>
</template>
