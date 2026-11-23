# GitKraken 10.8.1 阅读记录与复读方法

本文记录本项目立项时对 GitKraken 10.8.1 做的一次静态逆向，包括**读到了什么**、**哪些没读到**，以及**别人怎么复现或继续往下读**。

结论先行：这次只做了静态证据（asar 解包 + Electron fuses + 主进程静态分析 + bundle 字符串/i18n 统计），**没有做运行时行为验证**，大 bundle 的完整语义图分析也因体积超限降级了。因此下文的功能表是「代码与文案中的出现频率」，只能当功能面清单用，不能当功能完整度或实现细节用。

## 一、分析对象

| 项目 | 值 |
|---|---|
| 版本 | GitKraken 10.8.1（Electron 应用） |
| 安装方式 | Squirrel 风格：`%LOCALAPPDATA%\gitkraken\app-10.8.1\`（同级有 `Update.exe`、`packages\`） |
| 应用包 | `resources\app.asar`，约 807 MB，36,372 个文件 |
| 原生模块 | `resources\app.asar.unpacked\node_modules\`：`nodegit.node`、`node-pty`（含 ConPTY）、`nsfw`、`native-keymap`、`find-git-repositories`、`windows-process-tree` |
| 内置 Git | asar 内自带完整 MinGit 发行版（`git/`、`git-lfs.zip`） |
| 本次分析日期 | 2026-10-20 |

## 二、读到的信息（静态证据）

### 2.1 总体架构

- **主进程** `src/main/static/main.bundle.js`：7.88 MB，约 1,998 个 webpack 模块。
- **渲染进程** `src/render/static/entryPoints/main/render.bundle.js`：10.06 MB，约 2,742 个 webpack 模块。
- 另有 requestHandler（约 1.7 MB）、worker、webview preload。
- **前端栈**：React 16.8.4 + Redux + redux-saga + Monaco Editor。
- **双 Git 引擎**：`@axosoft/nodegit`（libgit2）+ 内置 MinGit 二进制。
- **IPC**：主 bundle 中提取到 **167 个字面量通道名**，例如 `application:new-tab`、`application:show-settings`、`application:open-cli-install-page`。
- **依赖侧写**（`package.json`）：`@gitbeaker`（GitLab）、`@octokit`（GitHub）、`monaco-editor`、`node-pty`、`openai`、Sentry/OpenTelemetry 等。

### 2.2 功能域清单

命中数为字符串/文案出现次数，**不代表功能规模**，只用于判断该功能域是否存在。

| 功能域 | 证据（静态命中） |
|---|---|
| Git 操作引擎 | rebase 1,585、submodule 835、worktree 492、cherry-pick 133、stash 103；GitFlow 45 组 i18n |
| 提交图谱可视化 | `graphContainer` 32、`commitGraph`、GraphHeader；自研 `@gitkraken/gitkraken-components` |
| 托管平台集成 | GitHub（Octokit REST/GraphQL）、GitLab（`@gitbeaker`）、Bitbucket、Azure DevOps；PR 视图 80 组 i18n |
| Issue 追踪 | Jira、Trello、GitHub/GitLab/Bitbucket Issues；筛选器表单 108 组字符串 |
| 合并冲突解决 | `conflictDetection` 224、ConflictDetection 30 组 i18n、独立冲突检测资源目录 |
| Workspaces 团队协作 | Workspace 80 组、Organization 22 组、Profile 39 组；Cloud Patches 335 次命中 |
| Focus View 工作流 | 渲染层 1,968 次命中、148 组 i18n |
| Launchpad | 51 次命中（GitHub/GitLab 待办聚合面板） |
| AI 能力 | OpenAI `/v1/chat/completions`、AI 提交信息生成、Code Suggest 127 次命中、Copilot 相关字符串 |
| 终端与编辑器 | node-pty 内嵌终端（821 次命中）、Monaco（119 次命中）、外部编辑器集成 |
| 认证与安全 | SSH 223 次、GPG 1,017 次、Deep Link 35 组 i18n |
| CLI | 777 组 `Cli-*` 字符串；yargs + commander；CliClient 通过 `net` 与运行中的 App 通信 |
| 可观测性 | Sentry DSN、OpenTelemetry OTLP（`otel.gitkraken.com`） |
| 次要/受限模块 | Timeline 12 组、FileHistory 10 组、Insights 仅 9 次命中 |

### 2.3 Electron 安全姿态（REA fuses）

检测目标：`app-10.8.1\gitkraken.exe`。

| Fuse | 状态 |
|---|---|
| `RunAsNode` | enabled |
| `EnableNodeOptionsEnvironmentVariable` | enabled |
| `EnableNodeCliInspectArguments` | enabled |
| `EnableEmbeddedAsarIntegrityValidation` | disabled |
| `OnlyLoadAppFromAsar` | disabled |

含义：asar 完整性不校验，Node 调试入口未被关闭，`ELECTRON_RUN_AS_NODE`、`--inspect` 一类常规调试手段可用。另外 `src/main/static/startDevTools.js` 里存在 `/json/list` CDP 端点，属于开发调试通道。

### 2.4 行为层观察（UI 对照，非二进制证据）

这些是使用 GitKraken 时观察到的隐式优化，用于补本项目的行为差距，**没有从 bundle 中逐条核实**：

1. 切换分支 / 变基 / 拣选前自动 stash，完成后自动 pop（冲突则保留 stash）。
2. `pull --autostash`，拉取时带上本地未提交改动。
3. 后台定时 + 窗口聚焦时自动 fetch，不打扰用户。
4. reflog 驱动的撤销（checkout / merge / reset 一键回退）。
5. push 默认 `--set-upstream` + `--force-with-lease`。
6. checkout 远程分支时自动建跟踪分支。
7. stash push / apply / pop / drop 的完整入口。

## 三、没读到的部分

- **运行时行为**：没有实际启动 GitKraken 观察网络请求、进程调用、真实交互，2.4 之外的行为均未验证。
- **大 bundle 语义图**：REA 对 7.9 MB 压缩 bundle 做完整语义分析会超时（超出单文件预算），本次只拿到主进程 6 个文件的 38 项静态发现，渲染层靠字符串统计。
- **没有 source map**：变量名、模块边界大量丢失，只能按字符串与 i18n key 归类。
- **服务端**：Workspaces、Cloud Patches、Launchpad 等云端能力只看到客户端调用痕迹，服务端协议与实现完全未涉及。
- **许可与授权逻辑**：未分析 Pro 功能开关的实现细节。GitKraken 是 `UNLICENSED` 闭源产品，分析仅用于功能参考，**不要复用其代码、图标、文案或资源**。
- **其他语言包**：只提取了 `src/strings/en-us.json`，其余 locale 未看。

## 四、复现与继续阅读的方法

### 4.1 定位安装与结构

```bash
ls -d "$LOCALAPPDATA/gitkraken/app-"*                       # Squirrel 布局，目录名即版本
ls "$LOCALAPPDATA/gitkraken/app-<version>/resources"         # 应能看到 app.asar / app.asar.unpacked
```

也检查过 `C:\Program Files\GitKraken`、`C:\Program Files (x86)\GitKraken`、`%APPDATA%\GitKraken`，本机命中在 `%LOCALAPPDATA%`。

### 4.2 先用 REA 看 Electron 层

需要 REA MCP 已连接（`npx rea-agents@latest setup`）。关键调用：

```
mcp__rea__inspect_electron_fuses
  path: <install>/gitkraken.exe

mcp__rea__analyze_javascript_application
  input_path: <install>/resources/app.asar
  format: "asar"
  source_paths: ["src/main/static/startMainProcess.js", "src/main/static/mode.js",
                 "src/main/static/CliClient.js", "src/main/static/cli.js",
                 "src/main/static/startDevTools.js", "src/sharedModules/static/parseGitKrakenUri.js"]
  detail: "summary"
  max_heap_mb: 2048

mcp__rea__inspect_analysis_view
  source: { kind: "retained-evidence", evidence_id: "ev_..." }   # 上一步返回
  view: { kind: "javascript-facts", where: { paths: [...] },
          include_available_property_keys: true, limit: 64 }
```

`javascript-facts` 视图里可以直接按 kind 过滤 `endpoint`、`browser-window`、IPC 等事实，这是拿到 167 个 IPC 通道名与调试端点的方式。

**坑**：大 bundle 别指望一次跑完。对 `main.bundle.js` 用 `detail: "summary"` + `analysis_timeout_ms: 280000` + `max_semantic_nodes: 300000` 仍可能超时/降级，正确做法是主进程小文件走 REA、大 bundle 走下面的字符串统计。

### 4.3 读 asar：头部格式或 `@electron/asar`

asar 格式：16 字节头 → offset 12 处是 `uint32LE` 的 JSON 头长度 → JSON 目录从 offset 16 开始 → 文件数据位于 `16 + headerSize + file.offset`。

手写解析可以拿到目录树与 `package.json`，但**文件内容和 offset 用现成库更稳**：

```bash
mkdir gk-analysis && cd gk-analysis
npm init -y && npm install @electron/asar
```

```js
const asar = require("@electron/asar");
const a = "<install>/resources/app.asar";

// asar 内部路径用反斜杠，Windows 上必须这么写
const files = {
  "package.json": "package.json",
  "src\\main\\static\\main.bundle.js": "main.bundle.js",
  "src\\render\\static\\entryPoints\\main\\render.bundle.js": "render.bundle.js",
  "src\\strings\\en-us.json": "en-us.json",
};
for (const [src, dst] of Object.entries(files)) {
  require("fs").writeFileSync(dst, asar.extractFile(a, src));
}

// 找不到路径时先 listPackage 再模糊匹配
console.log(asar.listPackage(a).filter(p => p.includes("render.bundle")));
```

本次提取到的四个文件：`package.json`、`main.bundle.js`（7.88 MB）、`render.bundle.js`（10.06 MB）、`en-us.json`。

### 4.4 无 source map 时的静态统计

没有语义图，就用可复现的机械统计（当初用的脚本思路）：

1. **webpack 模块数**：正则匹配 `数字:(参数)=>{` 形式的模块定义。
2. **字符串字面量集合**：抽出 2–200 字符的 `"..."` / `'...'`，去重后按关键词计数（`rebase`、`submodule`、`ConflictDetection`…）。
3. **i18n 功能面盘点**：读 `en-us.json`，按 key 的第一段前缀（`Launchpad`、`CloudPatch`、`GitFlow`…）做直方图——这是判断「某功能是否存在」最可靠的信号，比裸字符串噪音小。
4. **依赖侧写**：对 `package.json` 的 `dependencies` 做交集，确认 nodegit、`@gitbeaker`、`node-pty`、monaco、openai 等的存在与版本。
5. **原生模块**：`find app.asar.unpacked -iname "*.node"`，确认 libgit2 / pty / watcher 的实现来源。

### 4.5 想读到本文没读的部分

- **运行时**：fuses 允许 `ELECTRON_RUN_AS_NODE` 与 `--inspect`，`startDevTools.js` 暴露 `/json/list`，可用 CDP 观察渲染层；抓包观察 GitLab/GitHub 请求与 AI 端点。
- **完整解包**：`npx @electron/asar extract <asar> <dir>`（807 MB，注意磁盘与杀软耗时）。
- **更多语言包**：`src/strings/` 下其他 locale，对齐功能文案。
- **云端协议**：Workspaces / Cloud Patches 需要账号与抓包，注意不要触碰他人数据。
- **许可边界**：只记录行为与结构，不复用代码与资产。

## 五、对本项目的结论

GitKraken 的核心是一条「Git 双引擎 + 自研提交图谱 + 多平台 PR/Issue 集成 + 团队协作 + AI 辅助 + 内嵌终端」的桌面工作流，最重的自研部分在提交图谱渲染、冲突检测/解决、协作模型与 provider 抽象层。

Easy Git 只取其中与自身范围重合的部分（提交图谱、GitLab MR、AI 辅助、日志/详情），交互参考其行为，实现全部自写；架构、设计与范围分别见 [DESIGN.md](DESIGN.md)、[PRODUCT.md](PRODUCT.md)。