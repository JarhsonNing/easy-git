<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import Icon from "../components/Icon.vue";
import DeltaMark from "../components/DeltaMark.vue";
import BranchRail from "../components/BranchRail.vue";
import CanvasMenu from "../components/CanvasMenu.vue";
import ActionDialog from "../components/ActionDialog.vue";
import {
  abortOperation as abortGitOperation,
  cherryPick as cherryPickCommit,
  checkoutBranch as checkoutGitBranch,
  checkoutCommit as checkoutGitCommit,
  continueOperation as continueGitOperation,
  createBranch as createGitBranch,
  createTag as createGitTag,
  deleteBranch as deleteGitBranch,
  deleteTag as deleteGitTag,
  fetchRemote as fetchGitRemote,
  mergeBranch as mergeGitBranches,
  pullBranch as pullGitBranch,
  pushBranch as pushGitBranch,
  rebaseBranch as rebaseGitBranch,
  renameBranch as renameGitBranch,
  resetBranch as resetGitBranch,
  revertCommit as revertGitCommit,
} from "../lib/api";
import {
  clearNotice,
  detail,
  detailLoading,
  error as storeError,
  hasMore,
  loadMore,
  loading,
  loadingMore,
  notice,
  operation,
  refs,
  refreshRepo,
  rows,
  runOperation,
  selectBranch,
  selectCommit,
  selectOid,
  selectedBranch,
  selectedOid,
  session,
  setNotice,
  status,
} from "../stores/repo";
import { navigate } from "../stores/ui";
import type {
  ActionDialogConfig,
  BranchInfo,
  CanvasDragSource,
  CommitRow,
  DialogValues,
  MenuItem,
  OperationKind,
  RefLabel,
  ResetMode,
  TagInfo,
} from "../types";

const ROW_HEIGHT = 42;
const GRAPH_PADDING = 14;
const PALETTE = ["#1F6E6A", "#C9563C", "#B47A2C", "#765C88", "#4D7458", "#5C7890"];

const canvasRef = ref<HTMLCanvasElement | null>(null);
const scrollRef = ref<HTMLElement | null>(null);
const copied = ref(false);
const railOpen = ref(false);
const dragSource = ref<CanvasDragSource | null>(null);
const dropTarget = ref<string | null>(null);

type MenuContext =
  | { type: "branch"; branch: BranchInfo }
  | { type: "commit"; row: CommitRow }
  | { type: "tag"; tag: TagInfo };

interface MenuState {
  x: number;
  y: number;
  items: MenuItem[];
  context: MenuContext;
}

const menu = ref<MenuState | null>(null);
const dialog = ref<{
  config: ActionDialogConfig;
  run: (values: DialogValues) => Promise<void>;
} | null>(null);

const currentBranch = computed(() => refs.value?.local.find((branch) => branch.isCurrent) ?? null);
const headLabel = computed(() => {
  if (status.value?.detached) {
    return status.value.headOid ? `HEAD ${status.value.headOid.slice(0, 7)}` : "分离 HEAD";
  }
  return currentBranch.value?.name ?? status.value?.branch ?? session.value?.headBranch ?? "—";
});
const changes = computed(() => {
  if (!status.value) return 0;
  return status.value.staged + status.value.unstaged + status.value.untracked;
});
const operationStateKind = computed<OperationKind | null>(() =>
  notice.value?.state ? operationKind(notice.value.state) : null,
);
const detailRow = computed<CommitRow | null>(() => {
  const oid = detail.value?.oid ?? selectedOid.value;
  if (!oid) return null;
  const row = rows.value.find((item) => item.oid === oid);
  if (row) return row;
  if (!detail.value) return null;
  return {
    oid: detail.value.oid,
    shortId: detail.value.shortId,
    summary: detail.value.summary,
    authorName: detail.value.authorName,
    authoredAt: detail.value.authoredAt,
    lane: 0,
    parents: [],
    through: [],
    refs: [],
  };
});

const maxLane = computed(() =>
  rows.value.reduce((max, row) => {
    let next = Math.max(max, row.lane);
    for (const lane of row.through) next = Math.max(next, lane);
    for (const parent of row.parents) next = Math.max(next, parent.lane);
    return next;
  }, 0),
);

const laneWidth = computed(() => {
  const count = maxLane.value + 1;
  return Math.max(10, Math.min(18, Math.floor(120 / count)));
});

const graphWidth = computed(() => (maxLane.value + 1) * laneWidth.value + GRAPH_PADDING * 2);

function laneX(lane: number): number {
  return GRAPH_PADDING + lane * laneWidth.value + laneWidth.value / 2;
}

function laneColor(lane: number): string {
  return PALETTE[lane % PALETTE.length];
}

function withAlpha(hex: string, alpha: number): string {
  const value = hex.replace("#", "");
  const r = parseInt(value.slice(0, 2), 16);
  const g = parseInt(value.slice(2, 4), 16);
  const b = parseInt(value.slice(4, 6), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

function drawGraph() {
  const canvas = canvasRef.value;
  if (!canvas) return;
  const width = graphWidth.value;
  const height = Math.max(1, rows.value.length * ROW_HEIGHT);
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.round(width * dpr);
  canvas.height = Math.round(height * dpr);
  canvas.style.width = `${width}px`;
  canvas.style.height = `${height}px`;

  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, width, height);
  ctx.lineCap = "round";
  ctx.lineWidth = 2.2;

  for (let index = 0; index < rows.value.length; index += 1) {
    const row = rows.value[index];
    const centerY = index * ROW_HEIGHT + ROW_HEIGHT / 2;
    const nextY = Math.min(centerY + ROW_HEIGHT, height);
    const nodeX = laneX(row.lane);

    for (const lane of row.through) {
      ctx.strokeStyle = withAlpha(laneColor(lane), 0.65);
      ctx.beginPath();
      ctx.moveTo(laneX(lane), centerY);
      ctx.lineTo(laneX(lane), nextY);
      ctx.stroke();
    }

    for (const parent of row.parents) {
      const targetX = laneX(parent.lane);
      ctx.strokeStyle = withAlpha(laneColor(parent.lane), 0.9);
      ctx.beginPath();
      ctx.moveTo(nodeX, centerY);
      if (parent.lane === row.lane) {
        ctx.lineTo(targetX, nextY);
      } else {
        ctx.bezierCurveTo(
          nodeX,
          centerY + ROW_HEIGHT * 0.55,
          targetX,
          centerY + ROW_HEIGHT * 0.45,
          targetX,
          nextY,
        );
      }
      ctx.stroke();
    }

    const selected = selectedOid.value === row.oid;
    if (selected) {
      ctx.beginPath();
      ctx.arc(nodeX, centerY, 9, 0, Math.PI * 2);
      ctx.fillStyle = withAlpha(laneColor(row.lane), 0.2);
      ctx.fill();
    }
    ctx.beginPath();
    ctx.arc(nodeX, centerY, selected ? 5 : 4, 0, Math.PI * 2);
    ctx.fillStyle = laneColor(row.lane);
    ctx.fill();
    ctx.strokeStyle = "#FBF7EE";
    ctx.lineWidth = 1.8;
    ctx.stroke();
    ctx.lineWidth = 2.2;
  }
}

function scheduleDraw() {
  void nextTick(drawGraph);
}

watch([rows, selectedOid, graphWidth], scheduleDraw);
onMounted(scheduleDraw);

function visibleRefs(row: CommitRow): RefLabel[] {
  return row.refs.slice(0, 3);
}

function extraRefs(row: CommitRow): number {
  return Math.max(0, row.refs.length - 3);
}

function extraRefNames(row: CommitRow): string {
  return row.refs
    .slice(3)
    .map((label) => label.name)
    .join(" · ");
}

function branchForRef(label: RefLabel): BranchInfo | null {
  if (label.kind === "branch") {
    return refs.value?.local.find((branch) => branch.name === label.name) ?? null;
  }
  if (label.kind === "remote") {
    return refs.value?.remote.find((branch) => branch.name === label.name) ?? null;
  }
  return null;
}

function tagForRef(label: RefLabel): TagInfo | null {
  return refs.value?.tags.find((tag) => tag.name === label.name) ?? null;
}

const STATUS_TEXT: Record<string, string> = {
  added: "A",
  deleted: "D",
  modified: "M",
  renamed: "R",
  copied: "C",
  typechange: "T",
  conflicted: "U",
};

function statusText(value: string): string {
  return STATUS_TEXT[value] ?? "M";
}

function shortHash(hash: string): string {
  return hash.slice(0, 7);
}

function initial(name: string): string {
  return name.trim().slice(0, 1).toUpperCase() || "G";
}

const dateFormatter = new Intl.DateTimeFormat("zh-CN", {
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
});

function formatRelative(seconds: number): string {
  const diff = Math.floor(Date.now() / 1000) - seconds;
  if (diff < 60) return "刚刚";
  const minutes = Math.floor(diff / 60);
  if (minutes < 60) return `${minutes} 分钟前`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} 小时前`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days} 天前`;
  return dateFormatter.format(new Date(seconds * 1000));
}

function formatDateTime(seconds: number): string {
  return new Date(seconds * 1000).toLocaleString("zh-CN");
}

function operationKind(state: string): OperationKind | null {
  if (state === "merge") return "merge";
  if (
    state === "rebase" ||
    state === "rebase-interactive" ||
    state === "rebase-merge" ||
    state === "apply-mailbox" ||
    state === "apply-mailbox-or-rebase"
  ) {
    return "rebase";
  }
  if (state === "cherry-pick" || state === "cherry-pick-sequence") return "cherry-pick";
  if (state === "revert" || state === "revert-sequence") return "revert";
  return null;
}

function stateLabel(state: string): string {
  switch (operationKind(state)) {
    case "merge":
      return "合并尚未完成";
    case "rebase":
      return "变基尚未完成";
    case "cherry-pick":
      return "拣选尚未完成";
    case "revert":
      return "还原尚未完成";
    default:
      return "仓库中有未完成的操作";
  }
}

async function copyHash(hash: string) {
  try {
    await navigator.clipboard.writeText(hash);
    copied.value = true;
    window.setTimeout(() => {
      copied.value = false;
    }, 1600);
  } catch {
    copied.value = false;
  }
}

async function copyText(text: string, message: string) {
  try {
    await navigator.clipboard.writeText(text);
    setNotice({ tone: "success", message });
  } catch {
    setNotice({ tone: "error", message: "复制失败，请手动选择文本复制。" });
  }
}

function revealOid(oid: string) {
  void nextTick(() => {
    const target = scrollRef.value?.querySelector<HTMLElement>(`[data-oid="${oid}"]`);
    target?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  });
}

function onSelect(row: CommitRow) {
  copied.value = false;
  void selectCommit(row);
}

async function focusBranch(branch: BranchInfo) {
  railOpen.value = false;
  copied.value = false;
  await selectBranch(branch);
  revealOid(branch.oid);
}

async function focusTag(tag: TagInfo) {
  railOpen.value = false;
  copied.value = false;
  await selectOid(tag.oid);
  revealOid(tag.oid);
}

async function focusCurrentBranch() {
  if (currentBranch.value) {
    await focusBranch(currentBranch.value);
  } else if (status.value?.headOid) {
    await selectOid(status.value.headOid);
  }
}

function onRefClick(row: CommitRow, label: RefLabel) {
  if (label.kind === "head") {
    if (currentBranch.value) {
      void focusBranch(currentBranch.value);
    } else {
      onSelect(row);
    }
    return;
  }
  if (label.kind === "tag") {
    const tag = tagForRef(label);
    if (tag) {
      void focusTag(tag);
    } else {
      onSelect(row);
    }
    return;
  }
  const branch = branchForRef(label);
  if (branch) {
    void focusBranch(branch);
  } else {
    onSelect(row);
  }
}

function resetDrag() {
  dragSource.value = null;
  dropTarget.value = null;
}

function onBranchDragStart(branch: BranchInfo) {
  dragSource.value = { type: "branch", branch };
  dropTarget.value = null;
}

function onRowDragStart(event: DragEvent, row: CommitRow) {
  if (operation.value) {
    event.preventDefault();
    return;
  }
  dragSource.value = {
    type: "commit",
    oid: row.oid,
    shortId: row.shortId,
    summary: row.summary,
  };
  dropTarget.value = null;
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", row.oid);
  }
}

function onRefDragStart(event: DragEvent, branch: BranchInfo) {
  event.stopPropagation();
  if (operation.value) {
    event.preventDefault();
    return;
  }
  dragSource.value = { type: "branch", branch };
  dropTarget.value = null;
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", `${branch.kind}:${branch.name}`);
  }
}

function refDragStart(event: DragEvent, label: RefLabel) {
  const branch = branchForRef(label);
  if (!branch) {
    event.preventDefault();
    return;
  }
  onRefDragStart(event, branch);
}

function refDragOver(event: DragEvent, label: RefLabel) {
  if (!dragSource.value) return;
  event.stopPropagation();
  const branch = branchForRef(label);
  if (branch?.kind === "local") {
    onBranchDragOver(event, branch);
  }
}

function onBranchDragOver(event: DragEvent, branch: BranchInfo) {
  if (!dragSource.value || branch.kind !== "local" || operation.value) return;
  event.preventDefault();
  event.stopPropagation();
  if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
  dropTarget.value = `branch:${branch.kind}:${branch.name}`;
}

function onRowDragOver(event: DragEvent, row: CommitRow) {
  const source = dragSource.value;
  if (!source || source.type !== "branch" || source.branch.oid === row.oid) return;
  event.preventDefault();
  if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
  dropTarget.value = `commit:${row.oid}`;
}

function onRowDragLeave(event: DragEvent, row: CommitRow) {
  const current = event.currentTarget as HTMLElement;
  if (current.contains(event.relatedTarget as Node | null)) return;
  if (dropTarget.value === `commit:${row.oid}`) dropTarget.value = null;
}

function handleBranchDrop(branch: BranchInfo) {
  const source = dragSource.value;
  resetDrag();
  if (!source) return;
  if (source.type === "commit") {
    if (branch.kind !== "local") {
      setNotice({ tone: "error", message: "请把提交拖到本地分支上。" });
      return;
    }
    openCherryPick(source.oid, source.shortId, branch.name);
    return;
  }
  if (source.branch.kind === branch.kind && source.branch.name === branch.name) return;
  if (branch.kind !== "local") {
    setNotice({ tone: "error", message: "请把分支拖到本地分支上。" });
    return;
  }
  openDropBranch(source.branch, branch);
}

function onRefDrop(event: DragEvent, label: RefLabel) {
  event.preventDefault();
  event.stopPropagation();
  const branch = branchForRef(label);
  if (branch) handleBranchDrop(branch);
  else resetDrag();
}

function handleCommitDrop(row: CommitRow) {
  const source = dragSource.value;
  resetDrag();
  if (!source || source.type !== "branch") return;
  if (source.branch.kind !== "local") {
    setNotice({ tone: "error", message: "远程分支不能被移动，请先检出为本地分支。" });
    return;
  }
  if (source.branch.oid === row.oid) return;
  openDropBranchCommit(source.branch, row.oid, row.shortId);
}

function onRowDrop(event: DragEvent, row: CommitRow) {
  event.preventDefault();
  event.stopPropagation();
  handleCommitDrop(row);
}

function openRailMenu(payload: {
  x: number;
  y: number;
  branch?: BranchInfo;
  tag?: TagInfo;
}) {
  if (payload.branch) {
    openBranchMenu(payload.x, payload.y, payload.branch);
  } else if (payload.tag) {
    openTagMenu(payload.x, payload.y, payload.tag);
  }
}

function onRefContext(event: MouseEvent, label: RefLabel) {
  event.preventDefault();
  event.stopPropagation();
  if (label.kind === "head") {
    if (currentBranch.value) openBranchMenu(event.clientX, event.clientY, currentBranch.value);
    return;
  }
  if (label.kind === "tag") {
    const tag = tagForRef(label);
    if (tag) openTagMenu(event.clientX, event.clientY, tag);
    return;
  }
  const branch = branchForRef(label);
  if (branch) openBranchMenu(event.clientX, event.clientY, branch);
}

function onRowContext(event: MouseEvent, row: CommitRow) {
  event.preventDefault();
  openCommitMenu(event.clientX, event.clientY, row);
}

function openBranchMenu(x: number, y: number, branch: BranchInfo) {
  menu.value = {
    x,
    y,
    items: branchMenuItems(branch),
    context: { type: "branch", branch },
  };
}

function openCommitMenu(x: number, y: number, row: CommitRow) {
  menu.value = {
    x,
    y,
    items: commitMenuItems(),
    context: { type: "commit", row },
  };
}

function openTagMenu(x: number, y: number, tag: TagInfo) {
  menu.value = {
    x,
    y,
    items: tagMenuItems(),
    context: { type: "tag", tag },
  };
}

function openBranchMenuFromButton(event: MouseEvent, branch: BranchInfo) {
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  openBranchMenu(rect.right - 4, rect.bottom + 4, branch);
}

function openCommitMenuFromButton(event: MouseEvent) {
  const row = detailRow.value;
  if (!row) return;
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  openCommitMenu(rect.right - 4, rect.bottom + 4, row);
}

function branchMenuItems(branch: BranchInfo): MenuItem[] {
  const current = currentBranch.value;
  const local = branch.kind === "local";
  return [
    { id: "view", label: "查看分支详情", icon: "branch" },
    {
      id: "checkout",
      label: local ? "切换到该分支" : "检出为本地分支",
      icon: "arrow-right",
      disabled: branch.isCurrent,
    },
    { separator: true },
    { id: "create-branch", label: "从此分支新建…", icon: "plus" },
    { id: "create-tag", label: "在此创建标签…", icon: "tag" },
    {
      id: "merge",
      label: current ? `合并到 ${current.name}` : "合并到当前分支",
      icon: "git-merge",
      disabled: !current || branch.isCurrent,
    },
    {
      id: "rebase-current",
      label: current ? `将 ${current.name} 变基到此` : "将当前分支变基到此",
      icon: "branch",
      disabled: !current || branch.isCurrent,
    },
    { separator: true },
    { id: "rename", label: "重命名分支…", icon: "pencil", disabled: !local },
    {
      id: "delete",
      label: "删除分支…",
      icon: "trash",
      danger: true,
      disabled: !local || branch.isCurrent,
    },
    { id: "push", label: "推送分支…", icon: "upload", disabled: !local },
    { id: "copy", label: "复制分支名", icon: "copy" },
  ];
}

function commitMenuItems(): MenuItem[] {
  const current = currentBranch.value;
  return [
    { id: "view", label: "查看提交详情", icon: "commit" },
    { id: "create-branch", label: "在此创建分支…", icon: "plus" },
    { id: "create-tag", label: "在此创建标签…", icon: "tag" },
    {
      id: "cherry-pick",
      label: current ? `拣选到 ${current.name}` : "拣选到当前 HEAD",
      icon: "git-merge",
    },
    { id: "revert", label: "还原此提交", icon: "undo" },
    { separator: true },
    {
      id: "reset",
      label: current ? `将 ${current.name} 重置到此…` : "重置当前分支到此…",
      icon: "clock",
      disabled: !current,
    },
    {
      id: "rebase",
      label: current ? `将 ${current.name} 变基到此…` : "将当前分支变基到此…",
      icon: "branch",
      disabled: !current,
    },
    { separator: true },
    { id: "checkout", label: "签出此提交（分离 HEAD）", icon: "arrow-right" },
    { id: "copy", label: "复制提交哈希", icon: "copy" },
  ];
}

function tagMenuItems(): MenuItem[] {
  return [
    { id: "view", label: "查看标签提交", icon: "commit" },
    { id: "create-branch", label: "从此标签新建分支…", icon: "plus" },
    { id: "checkout", label: "签出标签（分离 HEAD）", icon: "arrow-right" },
    { separator: true },
    { id: "delete", label: "删除标签…", icon: "trash", danger: true },
  ];
}

function onMenuSelect(id: string) {
  const state = menu.value;
  menu.value = null;
  if (!state) return;
  const context = state.context;
  if (context.type === "branch") {
    void handleBranchAction(id, context.branch);
  } else if (context.type === "commit") {
    void handleCommitAction(id, context.row);
  } else {
    void handleTagAction(id, context.tag);
  }
}

async function handleBranchAction(id: string, branch: BranchInfo) {
  const current = currentBranch.value;
  switch (id) {
    case "view":
      await focusBranch(branch);
      break;
    case "checkout":
      await checkoutTo(branch);
      break;
    case "create-branch":
      openCreateBranch({ oid: branch.oid, label: branch.name });
      break;
    case "create-tag":
      openCreateTag(branch.oid, branch.name);
      break;
    case "merge":
      if (current) openMerge(branch, current.name);
      break;
    case "rebase-current":
      if (current) openRebase(current.name, branch.name, branch.name);
      break;
    case "rename":
      openRenameBranch(branch);
      break;
    case "delete":
      openDeleteBranch(branch);
      break;
    case "push":
      openPush(branch);
      break;
    case "copy":
      await copyText(branch.name, `已复制分支名 ${branch.name}`);
      break;
  }
}

async function handleCommitAction(id: string, row: CommitRow) {
  const current = currentBranch.value;
  switch (id) {
    case "view":
      onSelect(row);
      break;
    case "create-branch":
      openCreateBranch({ oid: row.oid, label: `提交 ${row.shortId}` });
      break;
    case "create-tag":
      openCreateTag(row.oid, row.shortId);
      break;
    case "cherry-pick":
      openCherryPick(row.oid, row.shortId, current?.name ?? null);
      break;
    case "revert":
      openRevert(row.oid, row.shortId);
      break;
    case "reset":
      if (current) openReset(current.name, row.oid, row.shortId);
      break;
    case "rebase":
      if (current) openRebase(current.name, `提交 ${row.shortId}`, row.oid);
      break;
    case "checkout":
      openCheckoutCommit(row.oid, row.shortId);
      break;
    case "copy":
      await copyHash(row.oid);
      setNotice({ tone: "success", message: `已复制提交哈希 ${row.shortId}` });
      break;
  }
}

async function handleTagAction(id: string, tag: TagInfo) {
  switch (id) {
    case "view":
      await focusTag(tag);
      break;
    case "create-branch":
      openCreateBranch({ oid: tag.oid, label: `标签 ${tag.name}` });
      break;
    case "checkout":
      openCheckoutCommit(tag.oid, tag.name);
      break;
    case "delete":
      openDeleteTag(tag);
      break;
  }
}

function showDialog(
  config: ActionDialogConfig,
  run: (values: DialogValues) => Promise<void>,
) {
  dialog.value = { config, run };
}

async function onDialogSubmit(values: DialogValues) {
  const current = dialog.value;
  if (!current) return;
  dialog.value = null;
  await current.run(values);
}

function textValue(values: DialogValues, key: string): string {
  const value = values[key];
  return typeof value === "string" ? value : "";
}

function boolValue(values: DialogValues, key: string): boolean {
  return values[key] === true;
}

function openCreateBranch(start?: { oid?: string; label?: string }) {
  showDialog(
    {
      title: "新建分支",
      description: start?.label
        ? `从 ${start.label} 开始创建一个新的本地分支。`
        : "从当前 HEAD 开始创建一个新的本地分支。",
      confirmLabel: "创建分支",
      fields: [
        {
          kind: "text",
          key: "name",
          label: "分支名",
          placeholder: "feature/graph-drag",
          required: true,
          mono: true,
          help: "可以使用 / 分层，例如 feature/graph-drag。",
        },
        {
          kind: "checkbox",
          key: "checkout",
          label: "创建后切换到此分支",
          value: true,
        },
      ],
    },
    async (values) => {
      const name = textValue(values, "name").trim();
      const checkout = boolValue(values, "checkout");
      const ok = await runOperation("正在创建分支…", () =>
        createGitBranch(name, start?.oid ?? null, checkout),
      );
      if (ok && checkout) {
        const branch = refs.value?.local.find((item) => item.name === name);
        if (branch) await selectBranch(branch);
      }
    },
  );
}

function openRenameBranch(branch: BranchInfo) {
  const wasSelected =
    selectedBranch.value?.kind === branch.kind && selectedBranch.value.name === branch.name;
  showDialog(
    {
      title: "重命名分支",
      description: `修改本地分支 ${branch.name} 的名称。`,
      confirmLabel: "重命名",
      fields: [
        {
          kind: "text",
          key: "name",
          label: "新分支名",
          value: branch.name,
          required: true,
          mono: true,
        },
      ],
    },
    async (values) => {
      const name = textValue(values, "name").trim();
      if (name === branch.name) return;
      const ok = await runOperation("正在重命名分支…", () =>
        renameGitBranch(branch.name, name),
      );
      if (ok) {
        const next = refs.value?.local.find((item) => item.name === name);
        if (next && (wasSelected || next.isCurrent)) await selectBranch(next);
      }
    },
  );
}

function openDeleteBranch(branch: BranchInfo) {
  showDialog(
    {
      title: "删除分支",
      description: `删除本地分支 ${branch.name}。当前分支不能删除。`,
      confirmLabel: "删除分支",
      danger: true,
      fields: [
        {
          kind: "checkbox",
          key: "force",
          label: "强制删除",
          help: "未合并的提交会被丢弃，无法从分支列表恢复。",
          danger: true,
        },
      ],
    },
    async (values) => {
      await runOperation("正在删除分支…", () =>
        deleteGitBranch(branch.name, boolValue(values, "force")),
      );
    },
  );
}

function openMerge(source: BranchInfo, target: string) {
  showDialog(
    {
      title: "合并分支",
      description: `将 ${source.name} 合并到本地分支 ${target}。如果目标不是当前分支，会先切换到目标。`,
      confirmLabel: "开始合并",
      fields: [
        {
          kind: "checkbox",
          key: "noFf",
          label: "始终创建合并提交（--no-ff）",
          help: "不勾选时，可以直接快进的历史会保持线性。",
        },
      ],
    },
    async (values) => {
      await runOperation("正在合并分支…", () =>
        mergeGitBranches(source.name, target, boolValue(values, "noFf")),
      );
    },
  );
}

function openRebase(source: string, ontoLabel: string, onto: string) {
  showDialog(
    {
      title: "变基分支",
      description: `将 ${source} 的提交重放到 ${ontoLabel} 之上。`,
      confirmLabel: "开始变基",
      fields: [
        {
          kind: "note",
          key: "warning",
          text: "变基会改写分支历史。已经推送过的分支在变基后需要强制推送，请确认没有其他人正在使用它。",
        },
      ],
    },
    async () => {
      await runOperation("正在变基…", () => rebaseGitBranch(source, onto));
    },
  );
}

function openReset(branch: string, oid: string, short: string) {
  const isCurrent = status.value?.branch === branch && !status.value?.detached;
  showDialog(
    {
      title: "重置分支",
      description: `把 ${branch} 的引用移动到提交 ${short}。`,
      confirmLabel: "重置分支",
      danger: true,
      fields: [
        {
          kind: "radio",
          key: "mode",
          label: "重置方式",
          value: isCurrent ? "mixed" : "hard",
          options: isCurrent
            ? [
                {
                  value: "soft",
                  label: "软重置",
                  help: "移动分支，改动保留在暂存区。",
                },
                {
                  value: "mixed",
                  label: "混合重置",
                  help: "移动分支和暂存区，改动保留在工作区。",
                },
                {
                  value: "hard",
                  label: "硬重置",
                  help: "移动分支并丢弃工作区与暂存区的改动。",
                },
              ]
            : [
                {
                  value: "hard",
                  label: "移动分支引用",
                  help: "目标不是当前分支，只会移动引用，不影响工作区。",
                },
              ],
        },
        {
          kind: "note",
          key: "warning",
          text: isCurrent
            ? "硬重置会永久丢弃目标提交之后未提交的改动。"
            : "当前工作区不会被修改。",
          danger: isCurrent,
        },
      ],
    },
    async (values) => {
      const mode = (textValue(values, "mode") || "mixed") as ResetMode;
      await runOperation("正在重置分支…", () => resetGitBranch(branch, oid, mode));
    },
  );
}

function openCherryPick(oid: string, short: string, target: string | null) {
  const targetLabel = target ?? "当前 HEAD";
  const switching = target !== null && target !== currentBranch.value?.name;
  showDialog(
    {
      title: "拣选提交",
      description: `将提交 ${short} 的改动应用到 ${targetLabel}。`,
      confirmLabel: "拣选提交",
      fields: switching
        ? [
            {
              kind: "note",
              key: "switch",
              text: `目标不是当前分支，操作前会先切换到 ${targetLabel}。`,
            },
          ]
        : [],
    },
    async () => {
      await runOperation("正在拣选提交…", () => cherryPickCommit(oid, target));
    },
  );
}

function openRevert(oid: string, short: string) {
  showDialog(
    {
      title: "还原提交",
      description: `创建一个新提交，撤销 ${short} 的改动。`,
      confirmLabel: "创建还原提交",
      fields: [
        {
          kind: "note",
          key: "note",
          text: "还原只会撤销这一个提交的改动，之后的历史保持不变。",
        },
      ],
    },
    async () => {
      await runOperation("正在还原提交…", () => revertGitCommit(oid));
    },
  );
}

function openCreateTag(oid: string, label: string) {
  showDialog(
    {
      title: "创建标签",
      description: `标签将指向 ${label}。`,
      confirmLabel: "创建标签",
      fields: [
        {
          kind: "text",
          key: "name",
          label: "标签名",
          placeholder: "v1.2.0",
          required: true,
          mono: true,
        },
        {
          kind: "textarea",
          key: "message",
          label: "说明",
          placeholder: "填写后会创建附注标签，留空则创建轻量标签。",
        },
      ],
    },
    async (values) => {
      const message = textValue(values, "message").trim();
      await runOperation("正在创建标签…", () =>
        createGitTag(textValue(values, "name").trim(), oid, message || null),
      );
    },
  );
}

function openDeleteTag(tag: TagInfo) {
  showDialog(
    {
      title: "删除标签",
      description: `删除标签 ${tag.name}。指向的提交不会受到影响。`,
      confirmLabel: "删除标签",
      danger: true,
      fields: [],
    },
    async () => {
      await runOperation("正在删除标签…", () => deleteGitTag(tag.name));
    },
  );
}

function openPush(branch: BranchInfo) {
  const remotes = refs.value?.remotes ?? [];
  if (!remotes.length) {
    setNotice({ tone: "error", message: "这个仓库还没有配置远程仓库，无法推送。" });
    return;
  }
  const upstreamRemote = branch.upstream?.split("/")[0];
  const defaultRemote =
    (upstreamRemote && remotes.includes(upstreamRemote) ? upstreamRemote : remotes[0]) ?? "";
  showDialog(
    {
      title: "推送分支",
      description: `将本地分支 ${branch.name} 推送到远程仓库。`,
      confirmLabel: "推送",
      fields: [
        {
          kind: "select",
          key: "remote",
          label: "远程",
          options: remotes.map((remote) => ({ value: remote, label: remote })),
          value: defaultRemote,
        },
        {
          kind: "checkbox",
          key: "setUpstream",
          label: "设置为上游分支",
          value: !branch.upstream,
          help: "下次拉取和推送会自动使用这个远程分支。",
        },
        {
          kind: "checkbox",
          key: "forceWithLease",
          label: "使用 --force-with-lease",
          help: "仅在远程分支没有新提交时覆盖它。",
          danger: true,
        },
        {
          kind: "note",
          key: "note",
          text: "如果推送被拒绝，请先获取远程更新；不要在没有确认的情况下强制覆盖。",
        },
      ],
    },
    async (values) => {
      const remote = textValue(values, "remote");
      await runOperation("正在推送…", () =>
        pushGitBranch(
          branch.name,
          remote,
          boolValue(values, "setUpstream"),
          boolValue(values, "forceWithLease"),
        ),
      );
    },
  );
}

function openPull(branch: BranchInfo) {
  const remotes = refs.value?.remotes ?? [];
  if (!remotes.length) {
    setNotice({ tone: "error", message: "这个仓库还没有配置远程仓库，无法拉取。" });
    return;
  }
  const upstreamRemote = branch.upstream?.split("/")[0];
  const defaultRemote =
    (upstreamRemote && remotes.includes(upstreamRemote) ? upstreamRemote : remotes[0]) ?? "";
  showDialog(
    {
      title: "拉取更新",
      description: `把远程提交拉取并合并到 ${branch.name}。`,
      confirmLabel: "拉取",
      fields: [
        {
          kind: "select",
          key: "remote",
          label: "远程",
          options: remotes.map((remote) => ({ value: remote, label: remote })),
          value: defaultRemote,
        },
        {
          kind: "text",
          key: "branch",
          label: "远程分支",
          value: branch.name,
          mono: true,
          required: true,
          help: "默认拉取同名分支。",
        },
        {
          kind: "note",
          key: "note",
          text: "拉取后如果产生冲突，可以在图谱顶部继续或中止这次操作。",
        },
      ],
    },
    async (values) => {
      const remote = textValue(values, "remote");
      const remoteBranch = textValue(values, "branch").trim();
      await runOperation("正在拉取…", () => pullGitBranch(remote, remoteBranch || null));
    },
  );
}

function openCheckoutCommit(oid: string, short: string) {
  showDialog(
    {
      title: "签出提交",
      description: `将 HEAD 指向 ${short}。`,
      confirmLabel: "签出提交",
      fields: [
        {
          kind: "note",
          key: "detached",
          text: "这会进入分离 HEAD 状态。在这里创建的新提交不会属于任何分支，除非你新建一个分支。",
        },
      ],
    },
    async () => {
      await runOperation("正在签出提交…", () => checkoutGitCommit(oid));
    },
  );
}

function openDropBranch(source: BranchInfo, target: BranchInfo) {
  if (source.kind === "remote") {
    showDialog(
      {
        title: "合并远程分支",
        description: `将 ${source.name} 合并到 ${target.name}。`,
        confirmLabel: "开始合并",
        fields: [
          {
            kind: "note",
            key: "note",
            text: "远程分支不能被移动，这里只会把它的提交合并到本地目标分支。",
          },
        ],
      },
      async () => {
        await runOperation("正在合并分支…", () =>
          mergeGitBranches(source.name, target.name, false),
        );
      },
    );
    return;
  }
  showDialog(
    {
      title: "连接两个分支",
      description: `选择如何把 ${source.name} 放到 ${target.name} 上。`,
      confirmLabel: "继续",
      fields: [
        {
          kind: "radio",
          key: "action",
          label: "操作",
          value: "merge",
          options: [
            {
              value: "merge",
              label: `将 ${source.name} 合并到 ${target.name}`,
              help: "保留两条历史。会切换到目标分支后执行。",
            },
            {
              value: "rebase",
              label: `将 ${source.name} 变基到 ${target.name}`,
              help: "把来源分支的提交重放到目标之上。会切换到来源分支。",
            },
          ],
        },
        {
          kind: "note",
          key: "note",
          text: "工作区有未提交改动时，Git 可能会拒绝执行。",
        },
      ],
    },
    async (values) => {
      if (textValue(values, "action") === "rebase") {
        await runOperation("正在变基…", () =>
          rebaseGitBranch(source.name, target.name),
        );
      } else {
        await runOperation("正在合并分支…", () =>
          mergeGitBranches(source.name, target.name, false),
        );
      }
    },
  );
}

function openDropBranchCommit(branch: BranchInfo, oid: string, short: string) {
  const isCurrent = branch.isCurrent;
  showDialog(
    {
      title: "把分支放到提交上",
      description: `让 ${branch.name} 指向或基于提交 ${short}。`,
      confirmLabel: "继续",
      fields: [
        {
          kind: "radio",
          key: "action",
          label: "操作",
          value: "reset",
          options: [
            {
              value: "reset",
              label: `将 ${branch.name} 重置到 ${short}`,
              help: isCurrent
                ? "当前分支会被移动，按下面选择的方式处理改动。"
                : "只移动分支引用，不影响当前工作区。",
            },
            {
              value: "rebase",
              label: `将 ${branch.name} 变基到 ${short}`,
              help: "把分支上的提交重放到这个提交之上，历史会改写。",
            },
          ],
        },
        {
          kind: "select",
          key: "mode",
          label: "重置方式",
          value: isCurrent ? "mixed" : "hard",
          options: isCurrent
            ? [
                { value: "soft", label: "软重置 · 改动进入暂存区" },
                { value: "mixed", label: "混合重置 · 改动留在工作区" },
                { value: "hard", label: "硬重置 · 丢弃改动" },
              ]
            : [{ value: "hard", label: "移动分支引用" }],
          help: "仅在选择重置时使用。",
        },
      ],
    },
    async (values) => {
      if (textValue(values, "action") === "rebase") {
        await runOperation("正在变基…", () => rebaseGitBranch(branch.name, oid));
      } else {
        const mode = (textValue(values, "mode") || "hard") as ResetMode;
        await runOperation("正在重置分支…", () => resetGitBranch(branch.name, oid, mode));
      }
    },
  );
}

async function checkoutTo(branch: BranchInfo) {
  const ok = await runOperation(`正在切换到 ${branch.name}…`, () =>
    checkoutGitBranch(branch.name, branch.kind),
  );
  if (ok) {
    railOpen.value = false;
    const current = refs.value?.local.find((item) => item.isCurrent);
    if (current) await selectBranch(current);
  }
}

function fetchAll() {
  if (operation.value) return;
  void runOperation("正在获取远程更新…", () => fetchGitRemote(null));
}

function openPushCurrent() {
  if (currentBranch.value) openPush(currentBranch.value);
}

function openPullCurrent() {
  if (currentBranch.value) openPull(currentBranch.value);
}

async function continueCurrent(kind: OperationKind) {
  const labels: Record<OperationKind, string> = {
    merge: "正在继续合并…",
    rebase: "正在继续变基…",
    "cherry-pick": "正在继续拣选…",
    revert: "正在继续还原…",
  };
  await runOperation(labels[kind], () => continueGitOperation(kind));
}

async function abortCurrent(kind: OperationKind) {
  await runOperation("正在中止操作…", () => abortGitOperation(kind));
}

function stateKindLabel(kind: OperationKind): string {
  return { merge: "合并", rebase: "变基", "cherry-pick": "拣选", revert: "还原" }[kind];
}
</script>

<template>
  <div class="workspace">
    <div v-if="!session" class="workspace-empty">
      <div class="empty-panel">
        <div class="empty-art" aria-hidden="true">
          <DeltaMark seed="commits-empty" animated />
        </div>
        <h2>请先选择一个项目</h2>
        <p>回到项目列表，打开一个本地 Git 仓库，提交历史和分支流向会在这里展开。</p>
        <button type="button" class="button primary" @click="navigate('projects')">
          返回项目列表
        </button>
      </div>
    </div>

    <template v-else>
      <div class="workspace-bar">
        <div class="workspace-heading">
          <h2>提交历史</h2>
          <p>
            {{ rows.length }} 个提交<span v-if="hasMore"> · 可继续加载</span>
            <span v-if="refs"> · {{ refs.local.length }} 个本地分支</span>
            <span v-if="changes"> · {{ changes }} 个未提交变更</span>
            <span v-if="status?.conflicted"> · {{ status.conflicted }} 个冲突</span>
            <span v-if="storeError"> · 读取遇到问题</span>
          </p>
        </div>
        <div class="workspace-actions">
          <span v-if="operation" class="op-pill" aria-live="polite">
            <span class="loading-orbit" aria-hidden="true"></span>
            {{ operation }}
          </span>
          <button
            type="button"
            class="branch-chip"
            :title="status?.upstream ? `上游 ${status.upstream}` : '当前分支'"
            @click="focusCurrentBranch"
          >
            <Icon name="branch" :size="13" />
            <span class="branch-chip-name">{{ headLabel }}</span>
            <span v-if="status?.ahead" class="chip-sync">↑{{ status.ahead }}</span>
            <span v-if="status?.behind" class="chip-sync">↓{{ status.behind }}</span>
          </button>
          <div class="sync-cluster" role="group" aria-label="远程同步">
            <button
              type="button"
              class="icon-button sync-button"
              title="获取所有远程更新"
              aria-label="获取所有远程更新"
              :disabled="!!operation || loading"
              @click="fetchAll"
            >
              <Icon name="refresh" :size="15" />
            </button>
            <button
              type="button"
              class="icon-button sync-button"
              title="拉取当前分支"
              aria-label="拉取当前分支"
              :disabled="!!operation || loading || !currentBranch"
              @click="openPullCurrent"
            >
              <Icon name="download" :size="15" />
            </button>
            <button
              type="button"
              class="icon-button sync-button"
              title="推送分支"
              aria-label="推送分支"
              :disabled="!!operation || loading || !currentBranch"
              @click="openPushCurrent"
            >
              <Icon name="upload" :size="15" />
            </button>
          </div>
          <button type="button" class="button ghost small rail-toggle" @click="railOpen = true">
            <Icon name="branch" :size="15" />
            分支
          </button>
          <button
            type="button"
            class="button ghost small"
            :disabled="loading || !!operation"
            @click="refreshRepo"
          >
            <Icon name="refresh" :size="15" />
            {{ loading ? "刷新中…" : "刷新" }}
          </button>
        </div>
      </div>

      <p v-if="storeError" class="alert error" role="alert">{{ storeError }}</p>

      <div v-if="notice" class="op-notice" :class="`tone-${notice.tone}`" role="status">
        <div class="op-notice-main">
          <span class="op-notice-icon" aria-hidden="true">
            <Icon
              :name="notice.tone === 'success' ? 'check' : notice.tone === 'conflict' ? 'alert' : 'alert'"
              :size="16"
            />
          </span>
          <div class="op-notice-copy">
            <strong>{{ notice.message }}</strong>
            <p v-if="notice.tone === 'conflict'">
              {{ stateLabel(notice.state ?? "") }}。在编辑器中解决冲突后回到这里继续，也可以中止本次操作。
            </p>
            <div v-if="notice.conflicts?.length" class="conflict-chips">
              <span v-for="path in notice.conflicts" :key="path" class="conflict-chip">{{ path }}</span>
            </div>
            <details v-if="notice.detail" class="op-detail">
              <summary>查看 Git 输出</summary>
              <pre>{{ notice.detail }}</pre>
            </details>
          </div>
        </div>
        <div class="op-notice-actions">
          <template v-if="notice.tone === 'conflict' && operationStateKind">
            <button
              type="button"
              class="button primary small"
              :disabled="!!operation"
              @click="continueCurrent(operationStateKind)"
            >
              <Icon name="play" :size="13" />
              继续{{ stateKindLabel(operationStateKind) }}
            </button>
            <button
              type="button"
              class="button ghost small"
              :disabled="!!operation"
              @click="abortCurrent(operationStateKind)"
            >
              中止
            </button>
          </template>
          <button type="button" class="icon-button notice-close" title="关闭提示" @click="clearNotice">
            <Icon name="close" :size="15" />
          </button>
        </div>
      </div>

      <div class="graph-layout">
        <div v-if="railOpen" class="rail-scrim" @click="railOpen = false"></div>
        <BranchRail
          :refs="refs"
          :selected="
            selectedBranch
              ? { name: selectedBranch.name, kind: selectedBranch.kind }
              : null
          "
          :disabled="!!operation"
          :class="{ 'rail-open': railOpen }"
          @select-branch="focusBranch"
          @select-tag="focusTag"
          @menu="openRailMenu"
          @create="openCreateBranch()"
          @fetch="fetchAll"
          @drag-start="onBranchDragStart"
          @drag-end="resetDrag"
          @drop-branch="handleBranchDrop"
          @close="railOpen = false"
        />

        <section class="surface commit-sheet" aria-label="提交图谱">
          <div class="commit-head">
            <span class="col-graph" :style="{ width: `${graphWidth + 8}px` }">图谱</span>
            <span class="col-summary">提交</span>
            <span class="col-author">作者</span>
            <span class="col-date">时间</span>
          </div>

          <div ref="scrollRef" class="commit-scroll">
            <div class="commit-rows">
              <canvas ref="canvasRef" class="graph-canvas"></canvas>
              <div
                v-for="row in rows"
                :key="row.oid"
                class="commit-row"
                :class="{
                  selected: selectedOid === row.oid,
                  'drop-target': dropTarget === `commit:${row.oid}`,
                }"
                :style="{ paddingLeft: `${graphWidth + 24}px` }"
                :title="row.summary"
                :data-oid="row.oid"
                :draggable="!operation"
                role="button"
                tabindex="0"
                :aria-pressed="selectedOid === row.oid"
                @click="onSelect(row)"
                @keydown.enter.self.prevent="onSelect(row)"
                @keydown.space.self.prevent="onSelect(row)"
                @contextmenu="onRowContext($event, row)"
                @dragstart="onRowDragStart($event, row)"
                @dragend="resetDrag"
                @dragover="onRowDragOver($event, row)"
                @dragleave="onRowDragLeave($event, row)"
                @drop="onRowDrop($event, row)"
              >
                <span class="col-summary">
                  <span class="commit-summary">{{ row.summary }}</span>
                  <span class="refs">
                    <button
                      v-for="label in visibleRefs(row)"
                      :key="`${label.kind}-${label.name}`"
                      type="button"
                      class="ref"
                      :class="[
                        `ref-${label.kind}`,
                        {
                          'drop-target':
                            label.kind !== 'tag' &&
                            label.kind !== 'head' &&
                            dropTarget === `branch:${label.kind}:${label.name}`,
                        },
                      ]"
                      :title="label.name"
                      :draggable="
                        !operation && (label.kind === 'branch' || label.kind === 'remote')
                      "
                      @click.stop="onRefClick(row, label)"
                      @keydown.stop
                      @contextmenu="onRefContext($event, label)"
                      @dragstart="refDragStart($event, label)"
                      @dragend.stop="resetDrag"
                      @dragover="refDragOver($event, label)"
                      @drop="onRefDrop($event, label)"
                    >
                      {{ label.name }}
                    </button>
                    <span
                      v-if="extraRefs(row)"
                      class="ref ref-more"
                      :title="extraRefNames(row)"
                    >
                      +{{ extraRefs(row) }}
                    </span>
                  </span>
                </span>
                <span class="col-author" :title="row.authorName">{{ row.authorName }}</span>
                <span class="col-date" :title="formatDateTime(row.authoredAt)">
                  {{ formatRelative(row.authoredAt) }}
                </span>
              </div>

              <div v-if="!rows.length && !loading" class="commit-empty">
                <span class="empty-line" aria-hidden="true"></span>
                这个仓库还没有任何提交
              </div>
              <div v-else-if="!rows.length && loading" class="commit-empty">
                正在读取提交历史…
              </div>
            </div>

            <div v-if="hasMore" class="load-more">
              <button type="button" class="button ghost small" :disabled="loadingMore" @click="loadMore">
                {{ loadingMore ? "加载中…" : "加载更多提交" }}
              </button>
            </div>
          </div>

          <div v-if="dragSource" class="drag-hint" aria-hidden="true">
            <Icon
              :name="dragSource.type === 'branch' ? 'git-merge' : 'commit'"
              :size="14"
            />
            <template v-if="dragSource.type === 'branch'">
              把 {{ dragSource.branch.name }} 放到分支上合并 / 变基，放到提交上重置 / 变基
            </template>
            <template v-else>
              把提交 {{ dragSource.shortId }} 放到本地分支上拣选
            </template>
          </div>
        </section>

        <aside class="surface inspector" aria-label="详情">
          <div class="inspector-head">
            <span class="section-label">{{ selectedBranch ? "分支详情" : "提交详情" }}</span>
            <button
              v-if="selectedBranch"
              type="button"
              class="button ghost tiny"
              @click="copyText(selectedBranch.name, `已复制分支名 ${selectedBranch.name}`)"
            >
              <Icon name="copy" :size="14" />
              复制分支名
            </button>
            <button
              v-else-if="detail"
              type="button"
              class="button ghost tiny"
              @click="copyHash(detail.oid)"
            >
              <Icon name="copy" :size="14" />
              {{ copied ? "已复制" : "复制哈希" }}
            </button>
          </div>

          <div v-if="selectedBranch" class="inspector-body branch-inspector">
            <div class="branch-identity">
              <span class="branch-emblem" aria-hidden="true">
                <Icon :name="selectedBranch.kind === 'local' ? 'branch' : 'cloud'" :size="17" />
              </span>
              <div class="branch-identity-copy">
                <span class="branch-kind">
                  {{ selectedBranch.kind === "local" ? "本地分支" : "远程分支" }}
                  <template v-if="selectedBranch.isCurrent"> · 当前检出</template>
                </span>
                <h3>{{ selectedBranch.name }}</h3>
              </div>
            </div>

            <dl class="branch-meta">
              <div>
                <dt>上游</dt>
                <dd>{{ selectedBranch.upstream ?? "未设置" }}</dd>
              </div>
              <div>
                <dt>领先 / 落后</dt>
                <dd>
                  <span class="stat-add">↑{{ selectedBranch.ahead }}</span>
                  <span class="stat-del">↓{{ selectedBranch.behind }}</span>
                </dd>
              </div>
              <div>
                <dt>最新提交</dt>
                <dd>{{ selectedBranch.shortId }}</dd>
              </div>
              <div>
                <dt>更新时间</dt>
                <dd>{{ formatRelative(selectedBranch.updatedAt) }}</dd>
              </div>
            </dl>

            <div class="branch-actions">
              <button
                v-if="!selectedBranch.isCurrent"
                type="button"
                class="button primary small"
                :disabled="!!operation"
                @click="checkoutTo(selectedBranch)"
              >
                <Icon name="arrow-right" :size="14" />
                {{ selectedBranch.kind === "local" ? "切换分支" : "检出分支" }}
              </button>
              <button
                v-if="selectedBranch.kind === 'local'"
                type="button"
                class="button ghost small"
                :disabled="!!operation"
                @click="openPush(selectedBranch)"
              >
                <Icon name="upload" :size="14" />
                推送
              </button>
              <button
                type="button"
                class="button ghost small"
                :disabled="!!operation"
                @click="openCreateBranch({ oid: selectedBranch.oid, label: selectedBranch.name })"
              >
                <Icon name="plus" :size="14" />
                新建分支
              </button>
              <button
                type="button"
                class="button ghost small"
                :disabled="!!operation"
                @click="openBranchMenuFromButton($event, selectedBranch)"
              >
                更多
              </button>
            </div>

            <div class="inspector-section-head split">
              <span class="section-label">最新提交</span>
              <span class="tip-hash">{{ selectedBranch.shortId }}</span>
            </div>

            <div v-if="detailLoading" class="detail-inline-loading">
              <span class="loading-orbit" aria-hidden="true"></span>
              正在读取提交…
            </div>
            <template v-else-if="detail">
              <h4 class="tip-summary">{{ detail.summary }}</h4>
              <p class="tip-meta">
                {{ detail.authorName }} · {{ formatDateTime(detail.authoredAt) }}
              </p>
              <div class="diff-stats">
                <span class="stat-files">{{ detail.filesChanged }} 个文件</span>
                <span class="stat-add">+{{ detail.insertions }}</span>
                <span class="stat-del">−{{ detail.deletions }}</span>
              </div>
              <pre v-if="detail.message.trim()" class="commit-message">{{ detail.message }}</pre>
              <div class="file-list">
                <div v-for="file in detail.files" :key="file.path" class="file-row">
                  <span class="file-status" :class="`status-${file.status}`">
                    {{ statusText(file.status) }}
                  </span>
                  <span class="file-path" :title="file.path">{{ file.path }}</span>
                  <span class="file-stats">
                    <span class="stat-add">+{{ file.additions }}</span>
                    <span class="stat-del">−{{ file.deletions }}</span>
                  </span>
                </div>
                <div v-if="!detail.files.length" class="file-empty">没有文件变更</div>
              </div>
            </template>
          </div>

          <div v-else-if="!selectedOid" class="inspector-empty">
            <div class="inspector-mark" aria-hidden="true">
              <DeltaMark seed="select-commit" />
            </div>
            <p>选一个提交或分支</p>
            <span>从左侧拖一个分支过来，合并、变基和重置都会在这里留下结果。</span>
          </div>

          <div v-else-if="detailLoading" class="inspector-empty" aria-live="polite">
            <span class="loading-orbit" aria-hidden="true"></span>
            <p>正在读取提交…</p>
          </div>

          <div v-else-if="detail" class="inspector-body">
            <span class="commit-hash">{{ detail.shortId }}</span>
            <h3>{{ detail.summary }}</h3>

            <div class="author-line">
              <span class="avatar">{{ initial(detail.authorName) }}</span>
              <span class="author-copy">
                <strong>{{ detail.authorName }}</strong>
                <span :title="detail.authorEmail">{{ detail.authorEmail }}</span>
              </span>
            </div>

            <div class="commit-actions">
              <button
                type="button"
                class="button ghost small"
                :disabled="!!operation"
                @click="openCreateBranch({ oid: detail.oid, label: detail.shortId })"
              >
                <Icon name="plus" :size="14" />
                新建分支
              </button>
              <button
                type="button"
                class="button ghost small"
                :disabled="!!operation"
                @click="openCreateTag(detail.oid, detail.shortId)"
              >
                <Icon name="tag" :size="14" />
                打标签
              </button>
              <button
                type="button"
                class="button ghost small"
                :disabled="!!operation"
                @click="openCherryPick(detail.oid, detail.shortId, currentBranch?.name ?? null)"
              >
                <Icon name="git-merge" :size="14" />
                拣选
              </button>
              <button
                type="button"
                class="button ghost small"
                :disabled="!!operation"
                @click="openCommitMenuFromButton($event)"
              >
                更多
              </button>
            </div>

            <dl class="meta-grid">
              <div>
                <dt>提交时间</dt>
                <dd>{{ formatDateTime(detail.authoredAt) }}</dd>
              </div>
              <div v-if="detail.parents.length">
                <dt>父提交</dt>
                <dd>{{ detail.parents.map(shortHash).join(" · ") }}</dd>
              </div>
            </dl>

            <pre v-if="detail.message.trim()" class="commit-message">{{ detail.message }}</pre>

            <div class="diff-stats">
              <span class="stat-files">{{ detail.filesChanged }} 个文件</span>
              <span class="stat-add">+{{ detail.insertions }}</span>
              <span class="stat-del">−{{ detail.deletions }}</span>
            </div>

            <div class="inspector-section-head">
              <span class="section-label">变更文件</span>
            </div>
            <div class="file-list">
              <div v-for="file in detail.files" :key="file.path" class="file-row">
                <span class="file-status" :class="`status-${file.status}`">
                  {{ statusText(file.status) }}
                </span>
                <span class="file-path" :title="file.path">{{ file.path }}</span>
                <span class="file-stats">
                  <span class="stat-add">+{{ file.additions }}</span>
                  <span class="stat-del">−{{ file.deletions }}</span>
                </span>
              </div>
              <div v-if="!detail.files.length" class="file-empty">没有文件变更</div>
            </div>
          </div>
        </aside>
      </div>
    </template>

    <CanvasMenu
      v-if="menu"
      :x="menu.x"
      :y="menu.y"
      :items="menu.items"
      @close="menu = null"
      @select="onMenuSelect"
    />
    <ActionDialog
      v-if="dialog"
      :config="dialog.config"
      @close="dialog = null"
      @submit="onDialogSubmit"
    />
  </div>
</template>
