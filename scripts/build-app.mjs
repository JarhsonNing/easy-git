#!/usr/bin/env node
import { spawn } from "node:child_process";
import { access, readdir, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const cli = path.join(root, "node_modules", "@tauri-apps", "cli", "tauri.js");
const args = process.argv.slice(2);
const noBundle = args.includes("--no-bundle");
const releaseDir = path.join(root, "src-tauri", "target", "release");

function formatDuration(ms) {
  const totalSeconds = Math.round(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return minutes > 0 ? `${minutes} 分 ${seconds} 秒` : `${seconds} 秒`;
}

function runBuild() {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [cli, "build", ...args], {
      cwd: root,
      stdio: "inherit",
    });
    child.on("error", reject);
    child.on("exit", (code, signal) => {
      if (code === 0) resolve();
      else reject(new Error(`tauri build 失败（${signal ?? `退出码 ${code}`}）`));
    });
  });
}

async function collectArtifacts(dir) {
  const found = [];
  let entries;
  try {
    entries = await readdir(dir, { withFileTypes: true });
  } catch {
    return found;
  }
  for (const entry of entries) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) found.push(...(await collectArtifacts(full)));
    else if (/\.(msi|exe)$/i.test(entry.name)) found.push(full);
  }
  return found;
}

async function describe(file) {
  const info = await stat(file);
  return `${path.relative(root, file)}  ${(info.size / 1024 / 1024).toFixed(1)} MB`;
}

try {
  await access(cli);
} catch {
  console.error("[build-app] 没找到本地 Tauri CLI，请先运行 npm install。");
  process.exit(1);
}

const startedAt = Date.now();
console.log(`[build-app] 开始构建：tauri build ${args.join(" ")}`.trim());

try {
  await runBuild();
} catch (error) {
  console.error(`[build-app] ${error instanceof Error ? error.message : error}`);
  process.exit(1);
}

const sections = [];

if (!noBundle) {
  const installers = (await collectArtifacts(path.join(releaseDir, "bundle"))).sort();
  if (installers.length > 0) {
    sections.push(["安装包", await Promise.all(installers.map(describe))]);
  } else {
    sections.push(["安装包", ["未找到安装包，检查 tauri.conf.json 里的 bundle 配置。"]]);
  }
}

const appExe = path.join(releaseDir, "easy-git.exe");
try {
  await access(appExe);
  sections.push(["可执行文件", [await describe(appExe)]]);
} catch {
  // --no-bundle 时 exe 一定存在；缺失说明构建未产出，交给上面的退出码体现
}

console.log(`\n[build-app] 构建完成，用时 ${formatDuration(Date.now() - startedAt)}`);
for (const [title, lines] of sections) {
  console.log(`\n  ${title}:`);
  for (const line of lines) console.log(`    ${line}`);
}