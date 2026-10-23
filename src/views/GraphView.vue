<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import Icon from "../components/Icon.vue";
import DeltaMark from "../components/DeltaMark.vue";
import BranchRail from "../components/BranchRail.vue";
import CanvasMenu from "../components/CanvasMenu.vue";
import ActionDialog from "../components/ActionDialog.vue";
import CommitComposer from "../components/CommitComposer.vue";
import CommitDiffView from "../components/CommitDiffView.vue";
import ChangesPanel from "../components/ChangesPanel.vue";
import InteractiveRebaseDialog from "../components/InteractiveRebaseDialog.vue";
import MergeConflictEditor from "../components/MergeConflictEditor.vue";
import WorkingDiff from "../components/WorkingDiff.vue";
import {
  abortOperation as abortGitOperation,
  addRemote as addGitRemote,
  cherryPick as cherryPickCommit,
  cherryPickMany as cherryPickCommits,
  checkoutBranch as checkoutGitBranch,
  checkoutCommit as checkoutGitCommit,
  continueOperation as continueGitOperation,
  createBranch as createGitBranch,
  createTag as createGitTag,
  deleteBranch as deleteGitBranch,
  deleteRemoteBranch as deleteGitRemoteBranch,
  deleteRemoteTag as deleteGitRemoteTag,
  deleteTag as deleteGitTag,
  diffCommits as diffGitCommits,
  errorText,
  fastForwardTag as fastForwardGitTag,
  fetchRemote as fetchGitRemote,
  getCommitDiff,
  getRebasePlan,
  inspectUndo,
  mergeBranch as mergeGitBranches,
  mergeIntoTag as mergeIntoGitTag,
  pruneRemote as pruneGitRemote,
  pullBranch as pullGitBranch,
  pushBranch as pushGitBranch,
  pushTag as pushGitTag,
  rebaseBranch as rebaseGitBranch,
  removeRemote as removeGitRemote,
  renameBranch as renameGitBranch,
  renameRemote as renameGitRemote,
  resetBranch as resetGitBranch,
  revertCommit as revertGitCommit,
  runInteractiveRebase,
  setRemoteUrl as setGitRemoteUrl,
  undoLast as undoGitLast,
} from "../lib/api";
import {
  clearNotice,
  clearSelection,
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
  remotes,
  rows,
  runOperation,
  selectBranch,
  selectCommit,
  selectCommits,
  selectOid,
  selectWip,
  selectedBranch,
  selectedOid,
  selectedOids,
  session,
  setNotice,
  status,
  undoInfo,
  wipSelected,
} from "../stores/repo";
import { navigate } from "../stores/ui";
import { requestCreateMergeRequest } from "../stores/mr";
import { setDragGhost } from "../lib/dragGhost";
import {
  adoptConflictSide,
  applyHunkByIndex,
  applyStash,
  conflictError,
  conflictFile,
  conflictLoading,
  diffLoading,
  discardFile,
  discardMany,
  enterWorkspace,
  files as workingFiles,
  filesTruncated,
  fileDiff,
  markConflictResolved,
  refreshWorkspace,
  removeStash,
  resetWorkspace,
  reuseCommitMessage,
  saveConflictMerge,
  selectFile,
  selected as selectedFileKey,
  stageMany,
  stashEverything,
  stashes,
  toggleFile,
  unstageMany,
  workspaceLoading,
} from "../stores/workspace";
import type {
  ActionDialogConfig,
  BranchInfo,
  CanvasDragSource,
  CommitDiff,
  CommitRow,
  DialogValues,
  LaneParent,
  MenuItem,
  OperationKind,
  RebasePlan,
  RebaseStep,
  RefLabel,
  RemoteInfo,
  ResetMode,
  StashEntry,
  TagInfo,
  WorkingFile,
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
const filterQuery = ref("");
const anchorOid = ref<string | null>(null);
const selectedStash = ref<StashEntry | null>(null);
const centerMode = ref<"graph" | "working" | "commit" | "compare">("graph");
const commitDiff = ref<CommitDiff | null>(null);
const commitDiffLoading = ref(false);
const commitFilePath = ref<string | null>(null);
const compareDiff = ref<CommitDiff | null>(null);
const compareLoading = ref(false);
const comparePair = ref<{ from: string; to: string } | null>(null);
let commitDiffRequest = 0;
let compareRequest = 0;

type MenuContext =
  | { type: "branch"; branch: BranchInfo }
  | { type: "commit"; row: CommitRow }
  | { type: "tag"; tag: TagInfo }
  | { type: "stash"; stash: StashEntry }
  | { type: "remote"; remote: RemoteInfo };

interface MenuState {
  x: number;
  y: number;
  items: MenuItem[];
  context: MenuContext;
}

const menu = ref<MenuState | null>(null);
const hoverOid = ref<string | null>(null);
const dropLabel = ref<string | null>(null);
const railTarget = ref<BranchInfo | null>(null);
interface DropMenuState {
  x: number;
  y: number;
  heading: string;
  items: MenuItem[];
  run: (id: string) => void | Promise<void>;
}
const dropMenu = ref<DropMenuState | null>(null);
const dialog = ref<{
  config: ActionDialogConfig;
  run: (values: DialogValues) => Promise<void>;
} | null>(null);

interface RebaseDialogState {
  branch: string;
  base: string;
  baseLabel: string;
}

const rebaseDialog = ref<RebaseDialogState | null>(null);
const rebasePlan = ref<RebasePlan | null>(null);
const rebasePlanLoading = ref(false);
const rebasePlanError = ref("");
let rebasePlanRequest = 0;

const DOSSIER_PREFS_KEY = "easygit.dossier";
const DOSSIER_MIN_CHANGES = 283;
const DOSSIER_MIN_COMPOSER = 190;

interface DossierPrefs {
  changes?: boolean;
  composer?: boolean;
  ratio?: number;
}

function readDossierPrefs(): DossierPrefs {
  try {
    const raw = localStorage.getItem(DOSSIER_PREFS_KEY);
    const parsed = raw ? JSON.parse(raw) : null;
    return parsed && typeof parsed === "object" ? parsed : {};
  } catch {
    return {};
  }
}

const dossierPrefs = readDossierPrefs();
const changesCollapsed = ref(Boolean(dossierPrefs.changes));
const composerCollapsed = ref(Boolean(dossierPrefs.composer));
const changesHeight = ref<number | null>(null);
const dossierDragging = ref(false);
const dossierRef = ref<HTMLElement | null>(null);
let dossierRatio =
  typeof dossierPrefs.ratio === "number" && Number.isFinite(dossierPrefs.ratio)
    ? Math.min(0.8, Math.max(0.2, dossierPrefs.ratio))
    : null;

function persistDossierPrefs() {
  const el = dossierRef.value;
  if (changesHeight.value != null && el && el.clientHeight > 0) {
    dossierRatio = changesHeight.value / el.clientHeight;
  }
  try {
    localStorage.setItem(
      DOSSIER_PREFS_KEY,
      JSON.stringify({
        changes: changesCollapsed.value,
        composer: composerCollapsed.value,
        ratio: dossierRatio,
      }),
    );
  } catch {
    // storage unavailable; keep the in-memory state
  }
}

function clampDossierHeight(height: number): number {
  const el = dossierRef.value;
  if (!el) return Math.max(DOSSIER_MIN_CHANGES, height);
  const gripHeight = el.querySelector<HTMLElement>(".dossier-grip")?.offsetHeight ?? 0;
  const max = Math.max(
    DOSSIER_MIN_CHANGES,
    el.clientHeight - gripHeight - DOSSIER_MIN_COMPOSER,
  );
  return Math.min(max, Math.max(DOSSIER_MIN_CHANGES, Math.round(height)));
}

function currentChangesHeight(): number {
  const el = dossierRef.value?.querySelector<HTMLElement>(".wip-dossier-changes");
  return changesHeight.value ?? el?.getBoundingClientRect().height ?? DOSSIER_MIN_CHANGES;
}

function onDossierGripDown(event: PointerEvent) {
  if (event.button !== 0) return;
  const grip = event.currentTarget as HTMLElement;
  event.preventDefault();
  grip.setPointerCapture(event.pointerId);
  dossierDragging.value = true;
  document.body.classList.add("is-resizing-rows");
  const startY = event.clientY;
  const startHeight = currentChangesHeight();
  const onMove = (moveEvent: PointerEvent) => {
    changesHeight.value = clampDossierHeight(startHeight + moveEvent.clientY - startY);
  };
  const finish = (upEvent: PointerEvent) => {
    grip.removeEventListener("pointermove", onMove);
    grip.removeEventListener("pointerup", finish);
    grip.removeEventListener("pointercancel", finish);
    if (grip.hasPointerCapture(upEvent.pointerId)) grip.releasePointerCapture(upEvent.pointerId);
    dossierDragging.value = false;
    document.body.classList.remove("is-resizing-rows");
    persistDossierPrefs();
  };
  grip.addEventListener("pointermove", onMove);
  grip.addEventListener("pointerup", finish);
  grip.addEventListener("pointercancel", finish);
}

function onDossierGripKey(event: KeyboardEvent) {
  if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
  event.preventDefault();
  const step = event.shiftKey ? 48 : 16;
  const delta = event.key === "ArrowDown" ? step : -step;
  changesHeight.value = clampDossierHeight(currentChangesHeight() + delta);
  persistDossierPrefs();
}

function resetDossierSplit() {
  changesHeight.value = null;
  dossierRatio = null;
  persistDossierPrefs();
}

async function toggleChangesCollapse() {
  changesCollapsed.value = !changesCollapsed.value;
  if (changesCollapsed.value) {
    changesHeight.value = null;
  } else {
    await nextTick();
    restoreDossierRatio();
  }
  persistDossierPrefs();
}

async function toggleComposerCollapse() {
  composerCollapsed.value = !composerCollapsed.value;
  if (composerCollapsed.value) {
    changesHeight.value = null;
  } else {
    await nextTick();
    restoreDossierRatio();
  }
  persistDossierPrefs();
}

function restoreDossierRatio() {
  if (dossierRatio == null || changesCollapsed.value || composerCollapsed.value) return;
  const height = dossierRef.value?.clientHeight;
  if (height) changesHeight.value = clampDossierHeight(dossierRatio * height);
}

let dossierObserver: ResizeObserver | null = null;
watch(dossierRef, (el) => {
  dossierObserver?.disconnect();
  dossierObserver = null;
  if (!el) return;
  dossierObserver = new ResizeObserver(() => {
    if (changesHeight.value != null) {
      changesHeight.value = clampDossierHeight(changesHeight.value);
    }
  });
  dossierObserver.observe(el);
  restoreDossierRatio();
});

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
const dragHint = computed(() => {
  const source = dragSource.value;
  if (!source) return "";
  if (source.type === "commit") {
    const target = dropLabel.value ?? railTarget.value?.name;
    return target
      ? `松开：把 ${source.shortId} 拣选到 ${target}`
      : `把 ${source.shortId} 拖到本地分支上拣选`;
  }
  if (source.type === "tag") {
    const target = dropLabel.value ?? railTarget.value?.name;
    return target
      ? `松开：${source.tag.name} → ${target}，选择快进或合并`
      : `把标签 ${source.tag.name} 拖到本地分支上`;
  }
  const target = dropTarget.value;
  if (target?.startsWith("commit:")) {
    return `松开：把 ${source.branch.name} 重置或变基到 ${dropLabel.value}`;
  }
  if (target?.startsWith("branch:")) {
    if (target.startsWith("branch:remote:")) {
      return `松开：${source.branch.name} → ${dropLabel.value}，新建合并请求`;
    }
    return source.branch.kind === "local"
      ? `松开：${source.branch.name} → ${dropLabel.value}，选择合并、变基或新建合并请求`
      : `松开：${source.branch.name} → ${dropLabel.value}，选择合并或新建合并请求`;
  }
  if (railTarget.value) {
    if (railTarget.value.kind === "remote") {
      return `松开：${source.branch.name} → ${railTarget.value.name}，新建合并请求`;
    }
    return source.branch.kind === "local"
      ? `松开：${source.branch.name} → ${railTarget.value.name}，选择合并、变基或新建合并请求`
      : `松开：${source.branch.name} → ${railTarget.value.name}，选择合并或新建合并请求`;
  }
  return `把 ${source.branch.name} 拖到分支或提交上执行操作`;
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

const isFiltering = computed(() => filterQuery.value.trim().length > 0);

const visibleRows = computed<CommitRow[]>(() => {
  if (!isFiltering.value) return rows.value;
  return relayoutRows(rows.value, visibleMatches.value);
});

const visibleMatches = computed<CommitRow[]>(() => {
  const needle = filterQuery.value.trim().toLowerCase();
  if (!needle) return rows.value;
  return rows.value.filter(
    (row) =>
      row.summary.toLowerCase().includes(needle) ||
      row.authorName.toLowerCase().includes(needle) ||
      row.shortId.toLowerCase().includes(needle) ||
      row.refs.some((label) => label.name.toLowerCase().includes(needle)),
  );
});

function relayoutRows(all: CommitRow[], source: CommitRow[]): CommitRow[] {
  const byOid = new Map(all.map((row) => [row.oid, row]));
  const visible = new Set(source.map((row) => row.oid));
  const nearestMemo = new Map<string, string | null>();

  const nearestVisible = (oid: string): string | null => {
    if (visible.has(oid)) return oid;
    if (nearestMemo.has(oid)) return nearestMemo.get(oid) ?? null;
    const row = byOid.get(oid);
    if (!row) {
      nearestMemo.set(oid, null);
      return null;
    }
    let found: string | null = null;
    for (const parent of row.parents) {
      found = nearestVisible(parent.oid);
      if (found) break;
    }
    nearestMemo.set(oid, found);
    return found;
  };

  const lanes: (string | null)[] = [];
  const result: CommitRow[] = [];

  for (const row of source) {
    let lane = lanes.indexOf(row.oid);
    if (lane === -1) {
      lane = lanes.indexOf(null);
      if (lane === -1) {
        lane = lanes.length;
        lanes.push(null);
      }
    }

    const parentOids: string[] = [];
    for (const parent of row.parents) {
      const target = nearestVisible(parent.oid);
      if (target && target !== row.oid && !parentOids.includes(target)) {
        parentOids.push(target);
      }
    }

    const parents: LaneParent[] = [];
    parentOids.forEach((oid, position) => {
      let targetLane = lanes.indexOf(oid);
      if (position === 0) {
        targetLane = lane;
      } else if (targetLane === -1 || targetLane === lane) {
        targetLane = lanes.indexOf(null);
        if (targetLane === -1) {
          targetLane = lanes.length;
          lanes.push(null);
        }
      }
      lanes[targetLane] = oid;
      parents.push({ oid, lane: targetLane });
    });
    if (!parentOids.length) lanes[lane] = null;

    const through: number[] = [];
    for (let index = 0; index < lanes.length; index += 1) {
      if (
        lanes[index] !== null &&
        index !== lane &&
        !parents.some((parent) => parent.lane === index)
      ) {
        through.push(index);
      }
    }
    while (lanes.length && lanes[lanes.length - 1] === null) lanes.pop();

    result.push({ ...row, lane, parents, through });
  }

  return result;
}

const originalRowsByOid = computed(() => new Map(rows.value.map((row) => [row.oid, row])));

const maxLane = computed(() =>
  visibleRows.value.reduce((max, row) => {
    let next = Math.max(max, row.lane);
    for (const lane of row.through) next = Math.max(next, lane);
    for (const parent of row.parents) next = Math.max(next, parent.lane);
    return next;
  }, 0),
);

const laneWidth = computed(() => {
  const count = maxLane.value + 1;
  return Math.max(12, Math.min(22, Math.floor(140 / count)));
});

const graphWidth = computed(() =>
  Math.max(92, (maxLane.value + 1) * laneWidth.value + GRAPH_PADDING * 2),
);
const wipNodeLeft = computed(() => laneX(0) - 6.5);

function laneX(lane: number): number {
  return GRAPH_PADDING + lane * laneWidth.value + laneWidth.value / 2;
}

function laneColor(lane: number): string {
  return PALETTE[lane % PALETTE.length];
}

interface RowColorPlan {
  node: string;
  through: { lane: number; color: string }[];
  parents: string[];
}

const colorPlan = computed(() => {
  const plan = new Map<string, RowColorPlan>();
  const laneColors: (string | null)[] = [];
  let cursor = 0;

  const pickColor = () => {
    const active = new Set(laneColors.filter((color): color is string => color !== null));
    for (let offset = 0; offset < PALETTE.length; offset += 1) {
      const color = PALETTE[(cursor + offset) % PALETTE.length];
      if (!active.has(color)) {
        cursor = (cursor + offset + 1) % PALETTE.length;
        return color;
      }
    }
    const color = PALETTE[cursor % PALETTE.length];
    cursor = (cursor + 1) % PALETTE.length;
    return color;
  };
  const ensure = (lane: number) => {
    if (!laneColors[lane]) laneColors[lane] = pickColor();
    return laneColors[lane];
  };

  for (const row of visibleRows.value) {
    const through = row.through.map((lane) => ({ lane, color: ensure(lane) }));
    const node = ensure(row.lane);
    const parents = row.parents.map((parent, index) => {
      const existing = laneColors[parent.lane];
      if (existing) return index === 0 ? node : existing;
      const color = index === 0 ? node : pickColor();
      laneColors[parent.lane] = color;
      return color;
    });
    plan.set(row.oid, { node, through, parents });

    const keep = new Set<number>([...row.through, ...row.parents.map((parent) => parent.lane)]);
    for (let lane = 0; lane < laneColors.length; lane += 1) {
      if (!keep.has(lane)) laneColors[lane] = null;
    }
  }
  return plan;
});

function rowColor(row: CommitRow): string {
  return colorPlan.value.get(row.oid)?.node ?? laneColor(row.lane);
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
  const height = Math.max(1, visibleRows.value.length * ROW_HEIGHT);
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
  ctx.lineJoin = "round";

  const targetOid = dropTarget.value?.startsWith("commit:")
    ? dropTarget.value.slice("commit:".length)
    : null;
  const focusOid = hoverOid.value ?? targetOid;
  const focusRow = focusOid
    ? visibleRows.value.find((row) => row.oid === focusOid) ?? null
    : null;
  const focusLane = focusRow?.lane ?? null;
  const laneAlpha = (lane: number, alpha: number) => {
    if (focusLane === null) return alpha;
    return lane === focusLane ? Math.min(1, alpha + 0.2) : alpha * 0.24;
  };
  const laneStroke = (lane: number) => (focusLane !== null && lane === focusLane ? 3.4 : 2.8);

  for (let index = 0; index < visibleRows.value.length; index += 1) {
    const row = visibleRows.value[index];
    const centerY = index * ROW_HEIGHT + ROW_HEIGHT / 2;
    const nextY = Math.min(centerY + ROW_HEIGHT, height);
    const nodeX = laneX(row.lane);

    const plan = colorPlan.value.get(row.oid);
    for (let laneIndex = 0; laneIndex < row.through.length; laneIndex += 1) {
      const lane = row.through[laneIndex];
      ctx.lineWidth = laneStroke(lane);
      ctx.strokeStyle = withAlpha(
        plan?.through[laneIndex]?.color ?? laneColor(lane),
        laneAlpha(lane, 0.92),
      );
      ctx.beginPath();
      ctx.moveTo(laneX(lane), centerY);
      ctx.lineTo(laneX(lane), nextY);
      ctx.stroke();
    }

    for (let parentIndex = 0; parentIndex < row.parents.length; parentIndex += 1) {
      const parent = row.parents[parentIndex];
      const targetX = laneX(parent.lane);
      ctx.lineWidth = laneStroke(parent.lane);
      ctx.strokeStyle = withAlpha(
        plan?.parents[parentIndex] ?? laneColor(parent.lane),
        laneAlpha(parent.lane, 0.96),
      );
      ctx.beginPath();
      ctx.moveTo(nodeX, centerY);
      if (parent.lane === row.lane) {
        ctx.lineTo(targetX, nextY);
      } else {
        ctx.bezierCurveTo(
          nodeX,
          centerY + ROW_HEIGHT * 0.62,
          targetX,
          nextY - ROW_HEIGHT * 0.62,
          targetX,
          nextY,
        );
      }
      ctx.stroke();
    }

    const selected = selectedOids.value.includes(row.oid);
    const focused = row.oid === focusRow?.oid;
    const staged = dropTarget.value === `commit:${row.oid}`;
    const lane = plan?.node ?? laneColor(row.lane);
    const faded = focusLane !== null && row.lane !== focusLane && !selected && !staged;
    const parentCount = originalRowsByOid.value.get(row.oid)?.parents.length ?? row.parents.length;
    const merge = parentCount > 1;
    const root = parentCount === 0;

    ctx.globalAlpha = faded ? 0.42 : 1;

    if (selected || focused || staged) {
      ctx.beginPath();
      ctx.arc(nodeX, centerY, staged ? 14.5 : 13.5, 0, Math.PI * 2);
      ctx.fillStyle = withAlpha(lane, staged ? 0.24 : 0.15);
      ctx.fill();
    }

    if (root) {
      ctx.beginPath();
      ctx.arc(nodeX, centerY, 7.2, 0, Math.PI * 2);
      ctx.fillStyle = "#FBF7EE";
      ctx.fill();
      ctx.strokeStyle = lane;
      ctx.lineWidth = 2.6;
      ctx.stroke();
      ctx.beginPath();
      ctx.arc(nodeX, centerY, 2.2, 0, Math.PI * 2);
      ctx.fillStyle = lane;
      ctx.fill();
    } else {
      if (merge) {
        ctx.beginPath();
        ctx.arc(nodeX, centerY, 10.2, 0, Math.PI * 2);
        ctx.fillStyle = "#FBF7EE";
        ctx.fill();
        ctx.strokeStyle = lane;
        ctx.lineWidth = 2.4;
        ctx.stroke();
      }
      const radius = merge ? 7.4 : 8.4;
      ctx.beginPath();
      ctx.arc(nodeX, centerY, radius, 0, Math.PI * 2);
      ctx.fillStyle = lane;
      ctx.fill();
      if (!merge) {
        ctx.strokeStyle = "#FBF7EE";
        ctx.lineWidth = 2;
        ctx.stroke();
      }
      ctx.fillStyle = "#FBF7EE";
      ctx.font = `${merge ? 7.6 : 8.4}px "Public Sans", "Microsoft YaHei", "PingFang SC", sans-serif`;
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText(authorInitials(row.authorName), nodeX, centerY + 0.4);
    }

    if (staged) {
      ctx.beginPath();
      ctx.arc(nodeX, centerY, 15, 0, Math.PI * 2);
      ctx.strokeStyle = withAlpha("#1F6E6A", 0.85);
      ctx.lineWidth = 1.7;
      ctx.setLineDash([3, 3]);
      ctx.stroke();
      ctx.setLineDash([]);
    } else if (selected) {
      ctx.beginPath();
      ctx.arc(nodeX, centerY, 13, 0, Math.PI * 2);
      ctx.strokeStyle = withAlpha(lane, 0.55);
      ctx.lineWidth = 1.6;
      ctx.stroke();
    }

    ctx.globalAlpha = 1;
  }
}

function scheduleDraw() {
  void nextTick(drawGraph);
}

watch([visibleRows, selectedOids, graphWidth, hoverOid, dropTarget], scheduleDraw);
watch(centerMode, (mode) => {
  if (mode === "graph") scheduleDraw();
});
onMounted(scheduleDraw);

interface RefStackLine {
  key: "local" | "tracking";
  labels: RefLabel[];
  hidden: number;
  hiddenNames: string;
}

const REF_LINE_LIMIT = 1;

function refStackLine(key: RefStackLine["key"], labels: RefLabel[]): RefStackLine | null {
  if (!labels.length) return null;
  return {
    key,
    labels: labels.slice(0, REF_LINE_LIMIT),
    hidden: Math.max(0, labels.length - REF_LINE_LIMIT),
    hiddenNames: labels
      .slice(REF_LINE_LIMIT)
      .map((label) => label.name)
      .join(" · "),
  };
}

function rowRefStack(row: CommitRow): RefStackLine[] {
  const local: RefLabel[] = [];
  const tracking: RefLabel[] = [];
  let head: RefLabel | null = null;
  for (const label of row.refs) {
    if (label.kind === "head") {
      head = label;
    } else if (label.kind === "branch") {
      local.push(label);
    } else if (label.kind === "remote") {
      if (!label.name.endsWith("/HEAD")) tracking.push(label);
    } else if (label.kind === "tag") {
      tracking.push(label);
    }
  }
  if (head && !local.length) local.unshift(head);
  local.sort((a, b) => Number(isCurrentRef(b)) - Number(isCurrentRef(a)));
  const current = currentBranch.value;
  const upstream = current?.upstream ?? "";
  tracking.sort((a, b) => remotePriority(b, upstream, current?.name) - remotePriority(a, upstream, current?.name));
  return [refStackLine("local", local), refStackLine("tracking", tracking)].filter(
    (line): line is RefStackLine => line !== null,
  );
}

function isCurrentRef(label: RefLabel): boolean {
  return branchForRef(label)?.isCurrent ?? false;
}

function remotePriority(label: RefLabel, upstream: string, currentName?: string): number {
  if (label.kind !== "remote") return -1;
  if (upstream && label.name === upstream) return 2;
  if (currentName && label.name.endsWith(`/${currentName}`)) return 1;
  return 0;
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

function isRefDropTarget(label: RefLabel): boolean {
  const branch = branchForRef(label);
  return !!branch && dropTarget.value === `branch:${branch.kind}:${branch.name}`;
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

function authorInitials(name: string): string {
  const trimmed = name.trim();
  if (!trimmed) return "·";
  if (/^[\u3400-\u9fff]/.test(trimmed)) return trimmed.slice(0, 1);
  const parts = trimmed.split(/[\s._-]+/).filter(Boolean);
  if (parts.length >= 2) {
    return `${parts[0][0]}${parts[1][0]}`.toUpperCase();
  }
  return trimmed.slice(0, 2).toUpperCase();
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

function formatIsoDate(value: string): string {
  if (!value) return "—";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString("zh-CN");
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

const wipCount = computed(() => {
  const value = status.value;
  return value
    ? value.staged + value.unstaged + value.untracked + value.conflicted
    : workingFiles.value.length;
});

const workspaceStats = computed(() => {
  let additions = 0;
  let deletions = 0;
  for (const file of workingFiles.value) {
    additions += file.additions;
    deletions += file.deletions;
  }
  return { additions, deletions };
});

const selectedRows = computed(() =>
  rows.value.filter((row) => selectedOids.value.includes(row.oid)),
);

const selectedRowsOldestFirst = computed(() => [...selectedRows.value].reverse());

const commitDiffTitle = computed(
  () => detail.value?.summary ?? (selectedOid.value ? selectedOid.value.slice(0, 7) : "提交差异"),
);

const commitDiffSubtitle = computed(() =>
  detail.value
    ? `${detail.value.shortId} · ${formatDateTime(detail.value.authoredAt)}`
    : "",
);

const compareTitle = computed(() => {
  const pair = comparePair.value;
  if (!pair) return "提交对比";
  const from = rows.value.find((row) => row.oid === pair.from);
  const to = rows.value.find((row) => row.oid === pair.to);
  return `对比 ${from?.shortId ?? pair.from.slice(0, 7)} → ${to?.shortId ?? pair.to.slice(0, 7)}`;
});

const compareSubtitle = computed(() => {
  const pair = comparePair.value;
  if (!pair) return "";
  const from = rows.value.find((row) => row.oid === pair.from);
  const to = rows.value.find((row) => row.oid === pair.to);
  if (!from || !to) return "";
  return `${from.summary} → ${to.summary}`;
});

function resetCenter() {
  centerMode.value = "graph";
  commitDiff.value = null;
  commitDiffLoading.value = false;
  commitFilePath.value = null;
  compareDiff.value = null;
  compareLoading.value = false;
  comparePair.value = null;
  commitDiffRequest += 1;
  compareRequest += 1;
}

function focusWip() {
  railOpen.value = false;
  copied.value = false;
  selectedStash.value = null;
  anchorOid.value = null;
  resetCenter();
  selectWip();
  if (!workingFiles.value.length && !workspaceLoading.value) void refreshWorkspace();
}

function focusStash(entry: StashEntry) {
  railOpen.value = false;
  copied.value = false;
  anchorOid.value = null;
  resetCenter();
  clearSelection();
  selectedStash.value = entry;
}

function selectSingle(row: CommitRow) {
  copied.value = false;
  selectedStash.value = null;
  anchorOid.value = row.oid;
  resetCenter();
  void selectCommit(row);
}

function onSelect(row: CommitRow) {
  selectSingle(row);
}

function onRowClick(event: MouseEvent, row: CommitRow) {
  copied.value = false;
  selectedStash.value = null;
  resetCenter();
  const range = isFiltering.value ? visibleRows.value : rows.value;
  if (event.shiftKey && anchorOid.value) {
    const from = range.findIndex((item) => item.oid === anchorOid.value);
    const to = range.findIndex((item) => item.oid === row.oid);
    if (from !== -1 && to !== -1) {
      const [low, high] = from < to ? [from, to] : [to, from];
      void selectCommits(
        range.slice(low, high + 1).map((item) => item.oid),
        row.oid,
      );
      return;
    }
  }
  if (event.ctrlKey || event.metaKey) {
    const current = selectedOids.value;
    const next = current.includes(row.oid)
      ? current.filter((oid) => oid !== row.oid)
      : [...current, row.oid];
    anchorOid.value = row.oid;
    void selectCommits(next.length ? next : [row.oid], row.oid);
    return;
  }
  selectSingle(row);
}

async function focusBranch(branch: BranchInfo) {
  railOpen.value = false;
  copied.value = false;
  selectedStash.value = null;
  filterQuery.value = "";
  resetCenter();
  anchorOid.value = branch.oid;
  await selectBranch(branch);
  revealOid(branch.oid);
}

async function focusTag(tag: TagInfo) {
  railOpen.value = false;
  copied.value = false;
  selectedStash.value = null;
  filterQuery.value = "";
  resetCenter();
  anchorOid.value = tag.oid;
  await selectOid(tag.oid);
  revealOid(tag.oid);
}

async function openWorkingFile(file: WorkingFile) {
  selectedStash.value = null;
  selectWip();
  centerMode.value = "working";
  await selectFile(file);
}

async function loadCommitDiff(oid: string): Promise<CommitDiff | null> {
  const request = ++commitDiffRequest;
  commitDiffLoading.value = true;
  try {
    const result = await getCommitDiff(oid);
    if (request === commitDiffRequest) commitDiff.value = result;
    return request === commitDiffRequest ? result : null;
  } catch (e) {
    if (request === commitDiffRequest) {
      commitDiff.value = null;
      setNotice({ tone: "error", message: errorText(e) });
    }
    return null;
  } finally {
    if (request === commitDiffRequest) commitDiffLoading.value = false;
  }
}

async function openCommitDiff(path: string | null = null) {
  const oid = detail.value?.oid ?? selectedOid.value;
  if (!oid) return;
  commitFilePath.value = path;
  centerMode.value = "commit";
  if (commitDiff.value?.oid === oid) return;
  commitDiff.value = null;
  await loadCommitDiff(oid);
}

async function openCompare() {
  if (selectedOids.value.length !== 2) return;
  const positions = selectedOids.value
    .map((oid) => rows.value.findIndex((row) => row.oid === oid))
    .filter((index) => index >= 0)
    .sort((a, b) => a - b);
  if (positions.length !== 2) {
    setNotice({ tone: "error", message: "这两个提交不在当前已加载的历史里，请多加载一些提交后再对比。" });
    return;
  }
  const newer = rows.value[positions[0]];
  const older = rows.value[positions[1]];
  comparePair.value = { from: older.oid, to: newer.oid };
  centerMode.value = "compare";
  compareDiff.value = null;
  compareLoading.value = true;
  const request = ++compareRequest;
  try {
    const result = await diffGitCommits(older.oid, newer.oid);
    if (request === compareRequest) compareDiff.value = result;
  } catch (e) {
    if (request === compareRequest) {
      compareDiff.value = null;
      setNotice({ tone: "error", message: errorText(e) });
    }
  } finally {
    if (request === compareRequest) compareLoading.value = false;
  }
}

function copySelectedHashes() {
  if (!selectedRows.value.length) return;
  const hashes = selectedRows.value.map((row) => row.oid).join("\n");
  void copyText(hashes, `已复制 ${selectedRows.value.length} 个提交哈希`);
}

function clearMultiSelection() {
  const primary = selectedOid.value ?? selectedOids.value[0];
  const row = rows.value.find((item) => item.oid === primary);
  if (row) selectSingle(row);
  else focusWip();
}

async function onReuseMessage(oid: string) {
  const written = await reuseCommitMessage(oid);
  if (!written) return;
  focusWip();
  setNotice({ tone: "success", message: "已把提交信息填入提交框，按需修改后提交。" });
}

function onStageFiles(selection: WorkingFile[]) {
  void stageMany(selection);
}

function onUnstageFiles(selection: WorkingFile[]) {
  void unstageMany(selection);
}

function openDiscardMany(selection: WorkingFile[]) {
  if (!selection.length) return;
  const preview = selection
    .slice(0, 4)
    .map((file) => file.path)
    .join("、");
  showDialog(
    {
      title: `丢弃 ${selection.length} 个文件的更改`,
      description:
        selection.length > 4
          ? `${preview} 等文件的未提交改动会被还原，未跟踪文件会被删除。`
          : `${preview} 的未提交改动会被还原，未跟踪文件会被删除。`,
      confirmLabel: "丢弃更改",
      danger: true,
      fields: [
        {
          kind: "note",
          key: "note",
          danger: true,
          text: "这个操作不可撤销，且不会写入任何可以恢复的地方。确认后再继续。",
        },
      ],
    },
    async () => {
      await discardMany(selection);
    },
  );
}

function openStashMenuFromRail(payload: { x: number; y: number; stash: StashEntry }) {
  openStashMenu(payload.x, payload.y, payload.stash);
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
  dropLabel.value = null;
  railTarget.value = null;
}

function dragPoint(event: DragEvent): { x: number; y: number } {
  if (event.clientX || event.clientY) return { x: event.clientX, y: event.clientY };
  const rect = (event.currentTarget as HTMLElement | null)?.getBoundingClientRect();
  return rect
    ? { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 }
    : { x: 240, y: 240 };
}

function onBranchDragStart(branch: BranchInfo) {
  dragSource.value = { type: "branch", branch };
  dropTarget.value = null;
  dropLabel.value = null;
  menu.value = null;
}

function onTagDragStart(tag: TagInfo) {
  dragSource.value = { type: "tag", tag };
  dropTarget.value = null;
  dropLabel.value = null;
  railTarget.value = null;
  menu.value = null;
}

function onTagRefDragStart(event: DragEvent, tag: TagInfo) {
  event.stopPropagation();
  if (operation.value) {
    event.preventDefault();
    return;
  }
  onTagDragStart(tag);
  setDragGhost(event, tag.name, "tag");
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", `tag:${tag.name}`);
  }
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
  dropLabel.value = null;
  menu.value = null;
  setDragGhost(event, `${row.shortId} · ${row.summary}`, "commit");
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
  dropLabel.value = null;
  menu.value = null;
  setDragGhost(event, branch.name, branch.kind === "remote" ? "remote" : "branch");
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", `${branch.kind}:${branch.name}`);
  }
}

function refDragStart(event: DragEvent, label: RefLabel) {
  if (label.kind === "tag") {
    const tag = tagForRef(label);
    if (!tag) {
      event.preventDefault();
      return;
    }
    onTagRefDragStart(event, tag);
    return;
  }
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
  if (branch) onBranchDragOver(event, branch);
}

function refDragLeave(event: DragEvent, label: RefLabel) {
  const current = event.currentTarget as HTMLElement;
  if (current.contains(event.relatedTarget as Node | null)) return;
  const branch = branchForRef(label);
  if (branch && dropTarget.value === `branch:${branch.kind}:${branch.name}`) {
    dropTarget.value = null;
    dropLabel.value = null;
  }
}

function onBranchDragOver(event: DragEvent, branch: BranchInfo) {
  const source = dragSource.value;
  if (!source || operation.value) return;
  const key = `branch:${branch.kind}:${branch.name}`;
  if (source.type === "commit") {
    if (branch.kind !== "local") {
      if (dropTarget.value === key) {
        dropTarget.value = null;
        dropLabel.value = null;
      }
      return;
    }
  } else if (source.type === "tag") {
    if (branch.kind !== "local") {
      if (dropTarget.value === key) {
        dropTarget.value = null;
        dropLabel.value = null;
      }
      return;
    }
  } else if (source.branch.kind === branch.kind && source.branch.name === branch.name) {
    if (dropTarget.value === key) {
      dropTarget.value = null;
      dropLabel.value = null;
    }
    return;
  }
  event.preventDefault();
  event.stopPropagation();
  if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
  dropTarget.value = key;
  dropLabel.value = branch.name;
}

function onRowDragOver(event: DragEvent, row: CommitRow) {
  const source = dragSource.value;
  if (!source || source.type !== "branch" || source.branch.oid === row.oid) return;
  event.preventDefault();
  if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
  dropTarget.value = `commit:${row.oid}`;
  dropLabel.value = row.shortId;
}

function onRowDragLeave(event: DragEvent, row: CommitRow) {
  const current = event.currentTarget as HTMLElement;
  if (current.contains(event.relatedTarget as Node | null)) return;
  if (dropTarget.value === `commit:${row.oid}`) {
    dropTarget.value = null;
    dropLabel.value = null;
  }
}

function onScrollDragOver(event: DragEvent) {
  const element = scrollRef.value;
  if (!element || !dragSource.value) return;
  const rect = element.getBoundingClientRect();
  const edge = 52;
  if (event.clientY < rect.top + edge) element.scrollTop -= 18;
  else if (event.clientY > rect.bottom - edge) element.scrollTop += 18;
}

function openDropMenu(
  x: number,
  y: number,
  heading: string,
  items: MenuItem[],
  run: (id: string) => void | Promise<void>,
) {
  menu.value = null;
  dropMenu.value = { x, y, heading, items, run };
}

function onDropMenuSelect(id: string) {
  const state = dropMenu.value;
  dropMenu.value = null;
  if (!state) return;
  void state.run(id);
}

function handleBranchDrop(branch: BranchInfo, x: number, y: number) {
  const source = dragSource.value;
  resetDrag();
  if (!source) return;
  if (source.type === "commit") {
    if (branch.kind !== "local") {
      setNotice({ tone: "error", message: "请把提交拖到本地分支上。" });
      return;
    }
    openDropMenu(
      x,
      y,
      `${source.shortId} → ${branch.name}`,
      [
        {
          id: "cherry-pick",
          label: `拣选 ${source.shortId} 到 ${branch.name}`,
          hint: "把这次提交的改动应用到目标分支",
          icon: "git-merge",
        },
      ],
      () => openCherryPick(source.oid, source.shortId, branch.name),
    );
    return;
  }
  if (source.type === "tag") {
    if (branch.kind !== "local") {
      setNotice({ tone: "error", message: "请把标签拖到本地分支上。" });
      return;
    }
    openDropMenu(
      x,
      y,
      `${source.tag.name} → ${branch.name}`,
      [
        {
          id: "fast-forward-tag",
          label: `Fast-forward ${source.tag.name} to ${branch.name}`,
          hint: "把标签快进到分支的最新提交",
          icon: "arrow-right",
        },
        {
          id: "merge-into-tag",
          label: `Merge ${branch.name} into ${source.tag.name}`,
          hint: "创建合并提交并把标签移动到合并结果",
          icon: "git-merge",
        },
      ],
      (id) => {
        if (id === "fast-forward-tag") openTagFastForward(source.tag, branch);
        else if (id === "merge-into-tag") openTagMerge(source.tag, branch);
      },
    );
    return;
  }
  if (source.branch.kind === branch.kind && source.branch.name === branch.name) return;

  const sourceName = serverBranchName(source.branch);
  const targetName = serverBranchName(branch);
  if (sourceName === targetName) {
    setNotice({ tone: "error", message: "源分支与目标分支相同，无法新建合并请求。" });
    return;
  }

  const items: MenuItem[] = [
    {
      id: "merge-request",
      label: "新建合并请求",
      hint: `在 GitLab 上创建 ${sourceName} → ${targetName}`,
      icon: "git-compare",
    },
  ];
  if (branch.kind === "local") {
    items.push({
      id: "merge",
      label: `合并到 ${branch.name}`,
      hint: `保留 ${source.branch.name} 与 ${branch.name} 的历史`,
      icon: "git-merge",
    });
  }
  if (branch.kind === "local" && source.branch.kind === "local") {
    items.push({
      id: "rebase",
      label: `将 ${source.branch.name} 变基到 ${branch.name}`,
      hint: "把来源分支的提交重放到目标之上",
      icon: "branch",
    });
  }

  openDropMenu(x, y, `${source.branch.name} → ${branch.name}`, items, (id) => {
    if (id === "merge-request") {
      requestCreateMergeRequest(sourceName, targetName);
      navigate("mr");
    } else if (id === "merge") {
      openMerge(source.branch, branch.name);
    } else if (id === "rebase") {
      openRebase(source.branch.name, branch.name, branch.name);
    }
  });
}

function onRefDrop(event: DragEvent, label: RefLabel) {
  event.preventDefault();
  event.stopPropagation();
  const branch = branchForRef(label);
  const point = dragPoint(event);
  if (branch) handleBranchDrop(branch, point.x, point.y);
  else resetDrag();
}

function handleCommitDrop(row: CommitRow, x: number, y: number) {
  const source = dragSource.value;
  resetDrag();
  if (!source || source.type !== "branch") return;
  if (source.branch.kind !== "local") {
    setNotice({ tone: "error", message: "远程分支不能被移动，请先检出为本地分支。" });
    return;
  }
  if (source.branch.oid === row.oid) return;

  openDropMenu(
    x,
    y,
    `${source.branch.name} → ${row.shortId}`,
    [
      {
        id: "reset",
        label: `重置 ${source.branch.name} 到 ${row.shortId}…`,
        hint: "选择软、混合或硬重置",
        icon: "clock",
      },
      {
        id: "rebase",
        label: `将 ${source.branch.name} 变基到 ${row.shortId}…`,
        hint: "把分支上的提交重放到此提交之上",
        icon: "branch",
      },
    ],
    (id) => {
      if (id === "reset") openReset(source.branch.name, row.oid, row.shortId);
      else openRebase(source.branch.name, row.shortId, row.oid);
    },
  );
}

function onRowDrop(event: DragEvent, row: CommitRow) {
  event.preventDefault();
  event.stopPropagation();
  const point = dragPoint(event);
  handleCommitDrop(row, point.x, point.y);
}

function onRailDragOver(branch: BranchInfo | null) {
  railTarget.value = branch;
}

function onRailBranchDrop(payload: { branch: BranchInfo; x: number; y: number }) {
  handleBranchDrop(payload.branch, payload.x, payload.y);
}

function isDragSource(label: RefLabel): boolean {
  const source = dragSource.value;
  if (!source) return false;
  if (label.kind === "tag") {
    return source.type === "tag" && label.name === source.tag.name;
  }
  if (source.type !== "branch") return false;
  const branch = branchForRef(label);
  return (
    !!branch && branch.kind === source.branch.kind && branch.name === source.branch.name
  );
}

function isRowDragSource(row: CommitRow): boolean {
  const source = dragSource.value;
  return source?.type === "commit" && source.oid === row.oid;
}

function refTitle(label: RefLabel): string {
  if (label.kind === "branch") return `${label.name} · 双击切换`;
  if (label.kind === "remote") return `${label.name} · 双击检出`;
  return label.name;
}

async function onRefDoubleClick(label: RefLabel) {
  if (operation.value) return;
  const branch = branchForRef(label);
  if (!branch || branch.isCurrent) return;
  if (branch.kind === "remote") {
    const local = refs.value?.local.find((item) => item.name === serverBranchName(branch));
    if (local) {
      if (!local.isCurrent) await checkoutTo(local);
      return;
    }
  }
  await checkoutTo(branch);
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
    items: commitMenuItems(row),
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

function openStashMenu(x: number, y: number, stash: StashEntry) {
  menu.value = {
    x,
    y,
    items: stashMenuItems(),
    context: { type: "stash", stash },
  };
}

function openRemoteMenu(payload: { x: number; y: number; remote: RemoteInfo }) {
  menu.value = {
    x: payload.x,
    y: payload.y,
    items: remoteMenuItems(payload.remote),
    context: { type: "remote", remote: payload.remote },
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
  const localActions: MenuItem[] = local
    ? [
        { id: "rename", label: "重命名分支…", icon: "pencil" },
        {
          id: "delete",
          label: "删除分支…",
          icon: "trash",
          danger: true,
          disabled: branch.isCurrent,
        },
        { id: "push", label: "推送分支…", icon: "upload" },
      ]
    : [{ id: "delete-remote", label: "删除远端分支…", icon: "trash", danger: true }];
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
    {
      id: "interactive-rebase",
      label: "交互式变基…",
      icon: "list",
      hint: branch.upstream
        ? `从 ${branch.upstream} 之后开始整理`
        : "这个分支还没有上游分支",
      disabled: !local || !branch.upstream,
    },
    { separator: true },
    ...localActions,
    { id: "copy", label: "复制分支名", icon: "copy" },
  ];
}

function commitMenuItems(row: CommitRow): MenuItem[] {
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
    {
      id: "interactive-rebase",
      label: "整理此后的提交…",
      icon: "list",
      hint: current ? `按新顺序重写 ${current.name} 的历史` : "先在本地分支上操作",
      disabled: !current || row.oid === current.oid,
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
    { id: "push", label: "推送标签到远端…", icon: "upload" },
    {
      id: "delete-remote",
      label: "删除远端标签…",
      icon: "trash",
      danger: true,
    },
    { id: "delete", label: "删除标签…", icon: "trash", danger: true },
  ];
}

function stashMenuItems(): MenuItem[] {
  return [
    { id: "apply", label: "应用贮藏", icon: "download" },
    { id: "pop", label: "弹出贮藏", icon: "upload" },
    { separator: true },
    { id: "copy", label: "复制贮藏引用", icon: "copy" },
    { id: "delete", label: "删除贮藏…", icon: "trash", danger: true },
  ];
}

function remoteMenuItems(remote: RemoteInfo): MenuItem[] {
  return [
    { id: "fetch", label: "抓取更新", icon: "refresh" },
    { id: "prune", label: "清理失效分支引用", icon: "tree" },
    { separator: true },
    { id: "url", label: "设置远程地址…", icon: "pencil" },
    { id: "rename", label: "重命名远程…", icon: "branch" },
    { id: "copy-name", label: "复制远程名", icon: "copy" },
    { id: "copy-url", label: "复制抓取地址", icon: "copy" },
    { separator: true },
    { id: "remove", label: `移除远程 ${remote.name}…`, icon: "trash", danger: true },
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
  } else if (context.type === "tag") {
    void handleTagAction(id, context.tag);
  } else if (context.type === "remote") {
    void handleRemoteAction(id, context.remote);
  } else {
    void handleStashAction(id, context.stash);
  }
}

async function handleStashAction(id: string, stash: StashEntry) {
  switch (id) {
    case "apply":
    case "pop": {
      const ok = await applyStash(stash, id === "pop");
      if (ok && selectedStash.value?.reference === stash.reference) selectedStash.value = null;
      break;
    }
    case "delete":
      openStashDrop(stash);
      break;
    case "copy":
      await copyText(stash.reference, `已复制 ${stash.reference}`);
      break;
  }
}

async function handleRemoteAction(id: string, remote: RemoteInfo) {
  switch (id) {
    case "fetch":
      await runOperation(`正在从 ${remote.name} 获取更新…`, () =>
        fetchGitRemote(remote.name),
      );
      break;
    case "prune":
      await runOperation(`正在清理 ${remote.name} 的失效分支引用…`, () =>
        pruneGitRemote(remote.name),
      );
      break;
    case "url":
      openRemoteUrl(remote);
      break;
    case "rename":
      openRenameRemote(remote);
      break;
    case "copy-name":
      await copyText(remote.name, `已复制远程名 ${remote.name}`);
      break;
    case "copy-url":
      await copyText(remote.fetchUrl, "已复制远程抓取地址");
      break;
    case "remove":
      openRemoveRemote(remote);
      break;
  }
}

function openAddRemote() {
  showDialog(
    {
      title: "添加远程仓库",
      description: "连接一个 HTTPS、SSH 或本地仓库地址。",
      confirmLabel: "添加远程",
      fields: [
        {
          kind: "text",
          key: "name",
          label: "远程名",
          value: "origin",
          required: true,
          mono: true,
          help: "通常使用 origin；也可以添加 upstream 等第二个远程。",
        },
        {
          kind: "text",
          key: "url",
          label: "远程地址",
          placeholder: "git@gitlab.example.com:group/repo.git",
          required: true,
          mono: true,
          help: "为避免凭据泄露，HTTP 地址不能包含用户名或密码。",
        },
      ],
    },
    async (values) => {
      const name = textValue(values, "name");
      const url = textValue(values, "url");
      await runOperation("正在添加远程…", () => addGitRemote(name, url));
    },
  );
}

function openRemoteUrl(remote: RemoteInfo) {
  const distinctPush = remote.pushUrl && remote.pushUrl !== remote.fetchUrl;
  const help = remote.hasCredentials
    ? "已保存地址中的凭据不会回显。未修改的字段会保持原样；如需更换地址，请重新输入完整地址。"
    : "支持 https://、ssh://、git@host:path 与本地路径。HTTP 地址不能包含用户名或密码。";
  showDialog(
    {
      title: "设置远程地址",
      description: `更新远程仓库 ${remote.name} 的抓取地址。`,
      confirmLabel: "保存地址",
      fields: [
        {
          kind: "text",
          key: "fetchUrl",
          label: "抓取地址",
          value: remote.fetchUrl,
          required: true,
          mono: true,
          help,
        },
        {
          kind: "text",
          key: "pushUrl",
          label: "推送地址",
          value: distinctPush ? remote.pushUrl : "",
          placeholder: "留空则不修改",
          mono: true,
          help: "仅当推送地址与抓取地址不同时填写。",
        },
      ],
    },
    async (values) => {
      const fetchUrl = textValue(values, "fetchUrl");
      const pushUrl = textValue(values, "pushUrl");
      let attempted = false;
      if (fetchUrl !== remote.fetchUrl) {
        attempted = true;
        await runOperation("正在更新抓取地址…", () =>
          setGitRemoteUrl(remote.name, fetchUrl, false),
        );
      }
      if (pushUrl && pushUrl !== remote.pushUrl) {
        attempted = true;
        await runOperation("正在更新推送地址…", () =>
          setGitRemoteUrl(remote.name, pushUrl, true),
        );
      }
      if (!attempted) {
        setNotice({ tone: "success", message: "远程地址未修改。" });
      }
    },
  );
}

function openRenameRemote(remote: RemoteInfo) {
  showDialog(
    {
      title: "重命名远程",
      description: `修改远程 ${remote.name} 的名称，远程分支引用会一并更新。`,
      confirmLabel: "重命名",
      fields: [
        {
          kind: "text",
          key: "name",
          label: "新远程名",
          value: remote.name,
          required: true,
          mono: true,
        },
      ],
    },
    async (values) => {
      const name = textValue(values, "name");
      if (name === remote.name) return;
      await runOperation("正在重命名远程…", () =>
        renameGitRemote(remote.name, name),
      );
    },
  );
}

function openRemoveRemote(remote: RemoteInfo) {
  showDialog(
    {
      title: "移除远程仓库",
      description: `移除远程 ${remote.name}，本地分支不会被删除。`,
      confirmLabel: "移除远程",
      danger: true,
      fields: [
        {
          kind: "note",
          key: "note",
          text: `remote.${remote.name} 的配置和 ${remote.name}/… 远程分支引用会被删除。如果只是暂时断开，可以保留远程。`,
          danger: true,
        },
      ],
    },
    async () => {
      await runOperation("正在移除远程…", () => removeGitRemote(remote.name));
    },
  );
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
    case "interactive-rebase":
      if (branch.upstream) {
        void openInteractiveRebase(branch.name, branch.upstream, branch.upstream);
      }
      break;
    case "rename":
      openRenameBranch(branch);
      break;
    case "delete":
      openDeleteBranch(branch);
      break;
    case "delete-remote":
      openDeleteRemoteBranch(branch);
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
    case "interactive-rebase":
      if (current) {
        void openInteractiveRebase(current.name, row.oid, `提交 ${row.shortId}`);
      }
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
    case "push":
      openPushTag(tag);
      break;
    case "delete-remote":
      openDeleteRemoteTag(tag);
      break;
  }
}

function showDialog(
  config: ActionDialogConfig,
  run: (values: DialogValues) => Promise<void>,
) {
  dialog.value = { config, run };
}

async function loadRebasePlan() {
  const current = rebaseDialog.value;
  if (!current) return;
  const request = ++rebasePlanRequest;
  rebasePlan.value = null;
  rebasePlanError.value = "";
  rebasePlanLoading.value = true;
  try {
    const plan = await getRebasePlan(current.branch, current.base);
    if (request === rebasePlanRequest) rebasePlan.value = plan;
  } catch (e) {
    if (request === rebasePlanRequest) rebasePlanError.value = errorText(e);
  } finally {
    if (request === rebasePlanRequest) rebasePlanLoading.value = false;
  }
}

async function openInteractiveRebase(branch: string, base: string, baseLabel: string) {
  rebaseDialog.value = { branch, base, baseLabel };
  await loadRebasePlan();
}

function closeInteractiveRebase() {
  rebaseDialog.value = null;
  rebasePlan.value = null;
  rebasePlanError.value = "";
  rebasePlanLoading.value = false;
  rebasePlanRequest += 1;
}

async function startInteractiveRebase(steps: RebaseStep[]) {
  const current = rebaseDialog.value;
  if (!current) return;
  const { branch, base } = current;
  closeInteractiveRebase();
  await runOperation("正在交互式变基…", () =>
    runInteractiveRebase(branch, base, steps),
  );
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

function openTagFastForward(tag: TagInfo, branch: BranchInfo) {
  showDialog(
    {
      title: "快进标签",
      description: `把标签 ${tag.name} 移动到 ${branch.name} 的最新提交。只有标签提交是 ${branch.name} 的祖先时才能快进。`,
      confirmLabel: "快进标签",
      danger: true,
      fields: [
        {
          kind: "note",
          key: "warning",
          text: "标签引用会被移动，已经推送到远端的标签需要强制推送才能更新。",
          danger: true,
        },
      ],
    },
    async () => {
      await runOperation("正在快进标签…", () => fastForwardGitTag(tag.name, branch.name));
    },
  );
}

function openTagMerge(tag: TagInfo, branch: BranchInfo) {
  showDialog(
    {
      title: "合并到标签",
      description: `把 ${branch.name} 合并进标签 ${tag.name}，并将标签移动到合并后的提交。`,
      confirmLabel: "合并并移动标签",
      danger: true,
      fields: [
        {
          kind: "note",
          key: "warning",
          text: "合并会在临时工作区完成，不会切换当前分支；已经推送的标签需要强制推送才能更新。",
          danger: true,
        },
      ],
    },
    async () => {
      await runOperation("正在合并到标签…", () => mergeIntoGitTag(tag.name, branch.name));
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

function openCherryPickSelection() {
  const target = currentBranch.value;
  const commits = selectedRowsOldestFirst.value;
  if (!commits.length) return;
  if (!target) {
    setNotice({ tone: "error", message: "当前不在任何本地分支上，无法拣选提交。" });
    return;
  }
  const names = commits.map((row) => row.shortId).join("、");
  showDialog(
    {
      title: `拣选 ${commits.length} 个提交`,
      description: `按从旧到新的顺序，把 ${names} 依次拣选到 ${target.name}。`,
      confirmLabel: "开始拣选",
      fields: [
        {
          kind: "note",
          key: "order",
          text: "提交会按历史顺序应用。如果中途发生冲突，可以在图谱顶部继续或中止这次拣选。",
        },
      ],
    },
    async () => {
      await runOperation(`正在拣选 ${commits.length} 个提交…`, () =>
        cherryPickCommits(
          commits.map((row) => row.oid),
          target.name,
        ),
      );
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

function remoteOfBranch(name: string): string {
  const remotes = refs.value?.remotes ?? [];
  const match = remotes
    .filter((remote) => name.startsWith(`${remote}/`))
    .sort((a, b) => b.length - a.length)[0];
  return match ?? "";
}

function openDeleteRemoteBranch(branch: BranchInfo) {
  const remote = remoteOfBranch(branch.name);
  showDialog(
    {
      title: "删除远端分支",
      description: `从远程仓库删除 ${branch.name}。`,
      confirmLabel: "删除远端分支",
      danger: true,
      fields: [
        {
          kind: "note",
          key: "note",
          text: remote
            ? `这条命令会删除 ${remote} 上的分支 ${branch.name.slice(remote.length + 1)}；其他协作者下次抓取时它就会消失。本地分支不受影响。`
            : "其他协作者下次抓取时这条分支就会消失。本地分支不受影响。",
          danger: true,
        },
      ],
    },
    async () => {
      await runOperation("正在删除远端分支…", () =>
        deleteGitRemoteBranch(branch.name),
      );
    },
  );
}

function tagRemoteFields(): { remotes: string[]; defaultRemote: string } {
  const remotes = refs.value?.remotes ?? [];
  const upstreamRemote = currentBranch.value?.upstream?.split("/")[0];
  const defaultRemote =
    (upstreamRemote && remotes.includes(upstreamRemote) ? upstreamRemote : remotes[0]) ?? "";
  return { remotes, defaultRemote };
}

function openPushTag(tag: TagInfo) {
  const { remotes, defaultRemote } = tagRemoteFields();
  if (!remotes.length) {
    setNotice({ tone: "error", message: "这个仓库还没有配置远程仓库，无法推送标签。" });
    return;
  }
  showDialog(
    {
      title: "推送标签",
      description: `将标签 ${tag.name} 推送到远程仓库。`,
      confirmLabel: "推送标签",
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
          key: "force",
          label: "覆盖远程已有的同名标签",
          help: "只有标签需要改指向时才用；它会改写别人已经看到的标签。",
          danger: true,
        },
        {
          kind: "note",
          key: "note",
          text: "默认只推送这一个标签，不会顺带推送其他标签。",
        },
      ],
    },
    async (values) => {
      await runOperation("正在推送标签…", () =>
        pushGitTag(tag.name, textValue(values, "remote"), boolValue(values, "force")),
      );
    },
  );
}

function openDeleteRemoteTag(tag: TagInfo) {
  const { remotes, defaultRemote } = tagRemoteFields();
  if (!remotes.length) {
    setNotice({ tone: "error", message: "这个仓库还没有配置远程仓库，无法删除远端标签。" });
    return;
  }
  showDialog(
    {
      title: "删除远端标签",
      description: `删除远程仓库上的标签 ${tag.name}，本地标签会保留。`,
      confirmLabel: "删除远端标签",
      danger: true,
      fields: [
        {
          kind: "select",
          key: "remote",
          label: "远程",
          options: remotes.map((remote) => ({ value: remote, label: remote })),
          value: defaultRemote,
        },
        {
          kind: "note",
          key: "note",
          text: "删除后其他协作者不会再抓取这个标签；已经存在于他们本地的标签不会被撤回。",
          danger: true,
        },
      ],
    },
    async (values) => {
      await runOperation("正在删除远端标签…", () =>
        deleteGitRemoteTag(tag.name, textValue(values, "remote")),
      );
    },
  );
}

async function openUndo() {
  let info = undoInfo.value;
  try {
    info = await inspectUndo();
    undoInfo.value = info;
  } catch (e) {
    setNotice({ tone: "error", message: errorText(e) });
    return;
  }
  if (!info) {
    setNotice({ tone: "error", message: "没有可以撤销的操作。" });
    return;
  }
  showDialog(
    {
      title: "撤销上一步",
      description: `撤销${info.summary}。`,
      confirmLabel: "撤销",
      danger: info.destructive,
      fields: [
        {
          kind: "note",
          key: "note",
          text: info.detail,
          danger: info.destructive,
        },
      ],
    },
    async () => {
      await runOperation("正在撤销…", () => undoGitLast(), refreshAll);
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

function serverBranchName(branch: BranchInfo): string {
  if (branch.kind === "local") return branch.name;
  const remote = (refs.value?.remotes ?? [])
    .filter((name) => branch.name.startsWith(`${name}/`))
    .sort((a, b) => b.length - a.length)[0];
  return remote ? branch.name.slice(remote.length + 1) : branch.name;
}

async function checkoutTo(branch: BranchInfo) {
  if (branch.isCurrent || operation.value) return;
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

async function refreshAll() {
  await Promise.all([refreshRepo(), refreshWorkspace()]);
}

const AUTO_FETCH_INTERVAL = 120_000;
let autoFetchTimer: number | undefined;
let autoFetching = false;

function canAutoFetch(): boolean {
  return (
    !!session.value &&
    !loading.value &&
    !operation.value &&
    !workspaceLoading.value
  );
}

// GitKraken 式后台抓取：定时与窗口聚焦时静默更新，不打扰当前操作。
async function autoFetch() {
  if (autoFetching || !canAutoFetch()) return;
  autoFetching = true;
  try {
    await fetchGitRemote(null);
    if (canAutoFetch()) await refreshAll();
  } catch {
    // 离线或凭据缺失时静默跳过
  } finally {
    autoFetching = false;
  }
}

function onWindowVisible() {
  if (document.visibilityState === "visible") void autoFetch();
}

onMounted(() => {
  autoFetchTimer = window.setInterval(() => void autoFetch(), AUTO_FETCH_INTERVAL);
  document.addEventListener("visibilitychange", onWindowVisible);
});

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

const selectedFile = computed<WorkingFile | null>(() => {
  const key = selectedFileKey.value;
  if (!key) return null;
  return (
    workingFiles.value.find(
      (file) => file.path === key.path && file.stage === key.stage,
    ) ?? null
  );
});

watch(
  () => session.value?.path ?? "",
  (next, previous) => {
    if (next === previous) return;
    resetWorkspace();
    selectedStash.value = null;
    anchorOid.value = null;
    filterQuery.value = "";
    resetCenter();
    if (next) void enterWorkspace();
  },
  { immediate: true },
);

let workspaceTimer: number | undefined;

watch(
  () => {
    const value = status.value;
    if (!value) return "";
    return [
      value.branch,
      value.staged,
      value.unstaged,
      value.untracked,
      value.conflicted,
      value.state,
    ].join(":");
  },
  (next, previous) => {
    if (!session.value || next === previous) return;
    if (workspaceTimer !== undefined) window.clearTimeout(workspaceTimer);
    workspaceTimer = window.setTimeout(() => {
      workspaceTimer = undefined;
      if (session.value) void refreshWorkspace();
    }, 180);
  },
);

watch(selectedFileKey, (key) => {
  if (!key && centerMode.value === "working") resetCenter();
});

onBeforeUnmount(() => {
  if (workspaceTimer !== undefined) window.clearTimeout(workspaceTimer);
  if (autoFetchTimer !== undefined) window.clearInterval(autoFetchTimer);
  document.removeEventListener("visibilitychange", onWindowVisible);
  dossierObserver?.disconnect();
});

function openDiscard(file: WorkingFile) {
  const untracked = file.stage === "untracked";
  showDialog(
    {
      title: untracked ? "删除未跟踪文件" : "丢弃更改",
      description: untracked
        ? `将从磁盘删除 ${file.path}。这个文件还没有被 Git 记录，删掉后无法从仓库找回。`
        : `将 ${file.path} 的改动还原到 ${file.stage === "staged" ? "HEAD（同时取消暂存）" : "暂存区"}。未保存的改动会直接消失。`,
      confirmLabel: untracked ? "删除文件" : "丢弃更改",
      danger: true,
      fields: [
        {
          kind: "note",
          key: "note",
          danger: true,
          text: "这个操作不可撤销，且不会写入任何可以恢复的地方。确认后再继续。",
        },
      ],
    },
    async () => {
      await discardFile(file);
    },
  );
}

function openStash() {
  showDialog(
    {
      title: "贮藏变更",
      description: "把当前工作区的改动暂时收起来，让工作区回到干净状态。",
      confirmLabel: "贮藏",
      fields: [
        {
          kind: "text",
          key: "message",
          label: "说明",
          placeholder: "例如：切换分支前的临时保存",
          help: "留空则使用默认说明。",
        },
        {
          kind: "checkbox",
          key: "includeUntracked",
          label: "一起贮藏未跟踪文件",
          help: "未跟踪文件不会被 HEAD 记录，选上才会一起收起来。",
        },
      ],
    },
    async (values) => {
      await stashEverything(textValue(values, "message").trim(), boolValue(values, "includeUntracked"));
    },
  );
}

function openStashDrop(entry: StashEntry) {
  showDialog(
    {
      title: "删除贮藏",
      description: `将删除 ${entry.reference}：${entry.message}`,
      confirmLabel: "删除贮藏",
      danger: true,
      fields: [
        {
          kind: "note",
          key: "note",
          danger: true,
          text: "删除后这条贮藏很难再找回，请确认它已经不再需要。",
        },
      ],
    },
    async () => {
      const removed = await removeStash(entry);
      if (removed && selectedStash.value?.reference === entry.reference) {
        selectedStash.value = null;
      }
    },
  );
}

async function onStashApply(entry: StashEntry) {
  const ok = await applyStash(entry, false);
  if (ok && selectedStash.value?.reference === entry.reference) selectedStash.value = null;
}

async function onStashPop(entry: StashEntry) {
  const ok = await applyStash(entry, true);
  if (ok && selectedStash.value?.reference === entry.reference) selectedStash.value = null;
}

function onHunkApply(index: number) {
  void applyHunkByIndex(index);
}

function onWorkingDiscard() {
  if (selectedFile.value) openDiscard(selectedFile.value);
}

function onWorkingToggleSelected() {
  if (selectedFile.value) void toggleFile(selectedFile.value);
}

function onConflictReload() {
  if (selectedFile.value) void selectFile(selectedFile.value);
}

async function onConflictSave(content: string) {
  await saveConflictMerge(content);
}

function onConflictAdoptSide(side: "ours" | "theirs") {
  const file = selectedFile.value;
  if (!file) return;
  const label = side === "ours" ? "采用当前版本" : "采用传入版本";
  showDialog(
    {
      title: label,
      description: `把 ${file.path} 整个替换成所选一侧的版本，未保存的合并结果会一起丢失。`,
      confirmLabel: label,
      danger: true,
      fields: [
        {
          kind: "note",
          key: "note",
          text: "只影响这个文件，采用后会暂存并标记为已解决。",
          danger: true,
        },
      ],
    },
    async () => {
      await adoptConflictSide(side);
    },
  );
}

function onConflictMarkResolved() {
  void markConflictResolved();
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
              class="sync-button"
              title="获取所有远程更新"
              :disabled="!!operation || loading"
              @click="fetchAll"
            >
              <Icon name="refresh" :size="14" />
              获取
            </button>
            <button
              type="button"
              class="sync-button"
              title="拉取当前分支"
              :disabled="!!operation || loading || !currentBranch"
              @click="openPullCurrent"
            >
              <Icon name="download" :size="14" />
              拉取
            </button>
            <button
              type="button"
              class="sync-button"
              title="推送分支"
              :disabled="!!operation || loading || !currentBranch"
              @click="openPushCurrent"
            >
              <Icon name="upload" :size="14" />
              推送
            </button>
          </div>
          <button
            type="button"
            class="button ghost small"
            :disabled="!!operation || loading || !undoInfo"
            :title="undoInfo ? `撤销${undoInfo.summary}` : '没有可以撤销的操作'"
            @click="openUndo"
          >
            <Icon name="undo" :size="15" />
            撤销
          </button>
          <button type="button" class="button ghost small rail-toggle" @click="railOpen = true">
            <Icon name="branch" :size="15" />
            分支
          </button>
          <button
            type="button"
            class="button ghost small"
            :disabled="loading || !!operation"
            @click="refreshAll"
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
          <button type="button" class="button ghost tiny notice-close" title="关闭提示" @click="clearNotice">
            <Icon name="close" :size="13" />
            关闭
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
          :stashes="stashes"
          :selected-stash="selectedStash?.reference ?? null"
          :remotes="remotes"
          :disabled="!!operation"
          :drag-kind="
            dragSource
              ? dragSource.type === 'commit'
                ? 'commit'
                : dragSource.type === 'tag'
                  ? 'tag'
                  : dragSource.branch.kind
              : null
          "
          :class="{ 'rail-open': railOpen }"
          @select-branch="focusBranch"
          @select-tag="focusTag"
          @select-stash="focusStash"
          @stash-new="openStash"
          @stash-menu="openStashMenuFromRail"
          @remote-add="openAddRemote"
          @remote-menu="openRemoteMenu"
          @menu="openRailMenu"
          @create="openCreateBranch()"
          @fetch="fetchAll"
          @drag-start="onBranchDragStart"
          @tag-drag-start="onTagDragStart"
          @drag-over="onRailDragOver"
          @drag-end="resetDrag"
          @drop-branch="onRailBranchDrop"
          @checkout-branch="checkoutTo"
          @close="railOpen = false"
        />

        <section
          v-if="centerMode === 'graph'"
          class="surface commit-sheet"
          :class="{ dragging: !!dragSource }"
          aria-label="提交图谱"
        >
          <div class="commit-head">
            <span class="col-refs">分支 / 标签</span>
            <span class="col-graph" :style="{ width: `${graphWidth}px` }">图谱</span>
            <span class="col-summary">提交</span>
            <span class="col-author">作者</span>
            <span class="col-date">时间</span>
            <span class="col-sha">SHA</span>
          </div>

          <div class="commit-tools">
            <label class="commit-filter">
              <Icon name="search" :size="14" />
              <input
                v-model="filterQuery"
                type="search"
                placeholder="筛选提交：摘要、作者、哈希或分支"
                aria-label="筛选提交"
              />
              <button
                v-if="filterQuery"
                type="button"
                class="filter-clear"
                title="清除筛选"
                @click="filterQuery = ''"
              >
                <Icon name="close" :size="11" />
                清除
              </button>
            </label>
            <span v-if="isFiltering" class="filter-count">
              {{ visibleMatches.length }} / {{ rows.length }}
            </span>
            <span v-else class="commit-tools-hint">Ctrl / Shift 点选多个提交</span>
          </div>

          <div v-if="selectedOids.length > 1" class="selection-bar" role="status">
            <span class="selection-count">
              已选 <strong>{{ selectedOids.length }}</strong> 个提交
            </span>
            <div class="selection-actions">
              <button
                type="button"
                class="button ghost tiny"
                :disabled="selectedOids.length !== 2"
                :title="selectedOids.length === 2 ? '对比两个提交' : '选择恰好两个提交才能对比'"
                @click="openCompare"
              >
                <Icon name="git-compare" :size="13" />
                对比
              </button>
              <button
                type="button"
                class="button ghost tiny"
                :disabled="!currentBranch"
                @click="openCherryPickSelection"
              >
                <Icon name="git-merge" :size="13" />
                拣选
              </button>
              <button type="button" class="button ghost tiny" @click="copySelectedHashes">
                <Icon name="copy" :size="13" />
                复制哈希
              </button>
              <button
                type="button"
                class="button ghost tiny"
                title="清除多选"
                @click="clearMultiSelection"
              >
                <Icon name="close" :size="12" />
                清除选择
              </button>
            </div>
          </div>

          <div
            class="wip-row"
            :class="{ selected: wipSelected }"
            role="button"
            tabindex="0"
            :aria-pressed="wipSelected"
            title="查看工作区变更并提交"
            @click="focusWip"
            @keydown.enter.self.prevent="focusWip"
            @keydown.space.self.prevent="focusWip"
          >
            <span class="col-refs wip-refs" aria-hidden="true"></span>
            <span
              class="col-graph wip-graph"
              :style="{ width: `${graphWidth}px` }"
              aria-hidden="true"
            >
              <span class="wip-node" :style="{ left: `${wipNodeLeft}px` }"></span>
            </span>
            <span class="wip-main">
              <span class="wip-badge">WIP</span>
              <span class="wip-title">
                {{ wipCount ? `${wipCount} 个文件待提交` : "工作区是干净的" }}
              </span>
              <span class="wip-branch">
                <Icon name="branch" :size="12" />
                {{ headLabel }}
              </span>
            </span>
            <span class="wip-stats">
              <span v-if="workspaceStats.additions" class="stat-add">
                +{{ workspaceStats.additions }}
              </span>
              <span v-if="workspaceStats.deletions" class="stat-del">
                −{{ workspaceStats.deletions }}
              </span>
              <span class="wip-cta">
                查看变更
                <Icon name="chevron-right" :size="11" />
              </span>
            </span>
          </div>

          <div ref="scrollRef" class="commit-scroll" @dragover="onScrollDragOver">
            <div class="commit-rows">
              <canvas ref="canvasRef" class="graph-canvas"></canvas>
              <div
                v-for="row in visibleRows"
                :key="row.oid"
                class="commit-row"
                :class="{
                  selected: selectedOids.includes(row.oid),
                  'primary-selected': selectedOid === row.oid,
                  'drop-target': dropTarget === `commit:${row.oid}`,
                  'drag-source': isRowDragSource(row),
                }"
                :title="row.summary"
                :data-oid="row.oid"
                :draggable="!operation"
                role="button"
                tabindex="0"
                :aria-pressed="selectedOids.includes(row.oid)"
                @click="onRowClick($event, row)"
                @keydown.enter.self.prevent="selectSingle(row)"
                @keydown.space.self.prevent="selectSingle(row)"
                @mouseenter="hoverOid = row.oid"
                @mouseleave="hoverOid = hoverOid === row.oid ? null : hoverOid"
                @contextmenu="onRowContext($event, row)"
                @dragstart="onRowDragStart($event, row)"
                @dragend="resetDrag"
                @dragover="onRowDragOver($event, row)"
                @dragleave="onRowDragLeave($event, row)"
                @drop="onRowDrop($event, row)"
              >
                <span class="col-refs">
                  <span v-if="rowRefStack(row).length" class="row-refs" aria-label="分支与标签">
                    <span
                      v-for="line in rowRefStack(row)"
                      :key="line.key"
                      class="row-ref-line"
                      :data-line="line.key"
                    >
                      <button
                        v-for="label in line.labels"
                        :key="`${label.kind}-${label.name}`"
                        type="button"
                        class="ref"
                        :class="[
                          `ref-${label.kind}`,
                          {
                            'drop-target': isRefDropTarget(label),
                            'drag-source': isDragSource(label),
                          },
                        ]"
                        :title="refTitle(label)"
                        :draggable="
                          !operation &&
                          (label.kind === 'branch' ||
                            label.kind === 'remote' ||
                            label.kind === 'tag')
                        "
                        @click.stop="onRefClick(row, label)"
                        @dblclick.stop="onRefDoubleClick(label)"
                        @keydown.stop
                        @contextmenu="onRefContext($event, label)"
                        @dragstart="refDragStart($event, label)"
                        @dragend.stop="resetDrag"
                        @dragover="refDragOver($event, label)"
                        @dragleave="refDragLeave($event, label)"
                        @drop="onRefDrop($event, label)"
                      >
                        <Icon
                          v-if="label.kind === 'tag'"
                          name="tag"
                          :size="9"
                          class="ref-icon"
                        />
                        <Icon
                          v-else-if="label.kind === 'remote'"
                          name="cloud"
                          :size="9"
                          class="ref-icon"
                        />
                        <Icon
                          v-else-if="label.kind === 'head' || isCurrentRef(label)"
                          name="check"
                          :size="9"
                          class="ref-icon ref-current"
                        />
                        <Icon
                          v-else
                          name="branch"
                          :size="9"
                          class="ref-icon ref-icon-lane"
                          :style="{ color: rowColor(row) }"
                        />
                        <span class="ref-name">{{ label.name }}</span>
                      </button>
                      <span v-if="line.hidden" class="ref ref-more" :title="line.hiddenNames">
                        +{{ line.hidden }}
                      </span>
                    </span>
                  </span>
                </span>
                <span
                  class="col-graph"
                  :style="{ width: `${graphWidth}px` }"
                  aria-hidden="true"
                ></span>
                <span class="col-summary">
                  <span class="commit-summary">{{ row.summary }}</span>
                </span>
                <span class="col-author" :title="row.authorName">{{ row.authorName }}</span>
                <span class="col-date" :title="formatDateTime(row.authoredAt)">
                  {{ formatRelative(row.authoredAt) }}
                </span>
                <span class="col-sha" :title="row.oid">{{ row.shortId }}</span>
              </div>

              <div v-if="!visibleRows.length && isFiltering" class="commit-empty">
                <span class="empty-line" aria-hidden="true"></span>
                没有匹配「{{ filterQuery.trim() }}」的提交
                <button type="button" class="button ghost tiny" @click="filterQuery = ''">
                  清除筛选
                </button>
              </div>
              <div v-else-if="!visibleRows.length && !loading" class="commit-empty">
                <span class="empty-line" aria-hidden="true"></span>
                这个仓库还没有任何提交
              </div>
              <div v-else-if="!visibleRows.length && loading" class="commit-empty">
                正在读取提交历史…
              </div>
            </div>

            <div v-if="hasMore" class="load-more">
              <button type="button" class="button ghost small" :disabled="loadingMore" @click="loadMore">
                {{ loadingMore ? "加载中…" : "加载更多提交" }}
              </button>
            </div>
          </div>

          <div
            v-if="dragSource"
            class="drag-hint"
            :class="{ active: !!dropLabel }"
            aria-hidden="true"
          >
            <Icon
              :name="
                dragSource.type === 'branch'
                  ? 'git-merge'
                  : dragSource.type === 'tag'
                    ? 'tag'
                    : 'commit'
              "
              :size="14"
            />
            {{ dragHint }}
          </div>
        </section>

        <MergeConflictEditor
          v-else-if="centerMode === 'working' && selectedFile?.stage === 'conflicted'"
          :file="selectedFile"
          :conflict="conflictFile"
          :loading="conflictLoading"
          :error="conflictError"
          :disabled="!!operation"
          back-label="图谱"
          @back="resetCenter"
          @reload="onConflictReload"
          @save="onConflictSave"
          @adopt-side="onConflictAdoptSide"
          @mark-resolved="onConflictMarkResolved"
        />

        <WorkingDiff
          v-else-if="centerMode === 'working'"
          :diff="fileDiff"
          :file="selectedFile"
          :loading="diffLoading"
          :disabled="!!operation"
          back-label="图谱"
          @apply-hunk="onHunkApply"
          @toggle-file="onWorkingToggleSelected"
          @discard="onWorkingDiscard"
          @back="resetCenter"
        />

        <CommitDiffView
          v-else-if="centerMode === 'commit'"
          :diff="commitDiff"
          :title="commitDiffTitle"
          :subtitle="commitDiffSubtitle"
          :only-path="commitFilePath"
          :loading="commitDiffLoading"
          :empty-text="commitFilePath ? '这个文件没有可展示的差异' : '这个提交没有文本差异'"
          back-label="图谱"
          @back="resetCenter"
        >
          <template #actions>
            <button
              v-if="commitFilePath"
              type="button"
              class="button ghost tiny"
              @click="commitFilePath = null"
            >
              <Icon name="file" :size="13" />
              全部文件
            </button>
          </template>
        </CommitDiffView>

        <CommitDiffView
          v-else
          :diff="compareDiff"
          :title="compareTitle"
          :subtitle="compareSubtitle"
          :loading="compareLoading"
          empty-text="两个提交之间没有文本差异"
          back-label="图谱"
          @back="resetCenter"
        />

        <aside
          v-if="wipSelected"
          ref="dossierRef"
          class="surface inspector wip-dossier"
          :class="{
            'changes-collapsed': changesCollapsed,
            'composer-collapsed': composerCollapsed,
            'is-split': changesHeight != null && !changesCollapsed && !composerCollapsed,
            'is-resizing': dossierDragging,
          }"
          aria-label="工作区变更与提交"
        >
          <div
            class="wip-dossier-changes"
            :style="
              changesHeight != null && !changesCollapsed && !composerCollapsed
                ? { flex: `0 0 ${changesHeight}px` }
                : undefined
            "
          >
            <ChangesPanel
              :files="workingFiles"
              :selected="selectedFileKey"
              :truncated="filesTruncated"
              :loading="workspaceLoading"
              :disabled="!!operation"
              :collapsed="changesCollapsed"
              @select="openWorkingFile"
              @stage-files="onStageFiles"
              @unstage-files="onUnstageFiles"
              @discard-files="openDiscardMany"
              @stash="openStash"
              @refresh="refreshWorkspace"
              @toggle-collapse="toggleChangesCollapse"
            />
          </div>
          <div
            v-if="!changesCollapsed && !composerCollapsed"
            class="dossier-grip"
            role="separator"
            aria-orientation="horizontal"
            aria-label="拖动调整变更与提交面板的高度"
            :aria-valuemin="DOSSIER_MIN_CHANGES"
            :aria-valuenow="Math.round(currentChangesHeight())"
            tabindex="0"
            title="拖动调整高度，双击恢复均分"
            @pointerdown="onDossierGripDown"
            @keydown="onDossierGripKey"
            @dblclick="resetDossierSplit"
          ></div>
          <div class="wip-dossier-composer">
            <CommitComposer
              :branch="headLabel"
              :can-push="(refs?.remotes.length ?? 0) > 0"
              :disabled="!!operation || !!dialog"
              :collapsed="composerCollapsed"
              @push="openPushCurrent"
              @toggle-collapse="toggleComposerCollapse"
            />
          </div>
        </aside>

        <aside v-else-if="selectedStash" class="surface inspector" aria-label="贮藏详情">
          <div class="inspector-head">
            <span class="section-label">贮藏详情</span>
            <button
              type="button"
              class="button ghost tiny"
              @click="copyText(selectedStash.reference, `已复制 ${selectedStash.reference}`)"
            >
              <Icon name="copy" :size="14" />
              复制引用
            </button>
          </div>
          <div class="inspector-body">
            <div class="branch-identity">
              <span class="branch-emblem" aria-hidden="true">
                <Icon name="stash" :size="17" />
              </span>
              <div class="branch-identity-copy">
                <span class="branch-kind">贮藏</span>
                <h3>{{ selectedStash.message }}</h3>
              </div>
            </div>

            <dl class="branch-meta">
              <div>
                <dt>引用</dt>
                <dd>{{ selectedStash.reference }}</dd>
              </div>
              <div>
                <dt>时间</dt>
                <dd>{{ formatIsoDate(selectedStash.date) }}</dd>
              </div>
            </dl>

            <div class="branch-actions">
              <button
                type="button"
                class="button primary small"
                :disabled="!!operation"
                @click="onStashApply(selectedStash)"
              >
                <Icon name="download" :size="14" />
                应用
              </button>
              <button
                type="button"
                class="button ghost small"
                :disabled="!!operation"
                @click="onStashPop(selectedStash)"
              >
                <Icon name="upload" :size="14" />
                弹出
              </button>
              <button
                type="button"
                class="button ghost small danger-text"
                :disabled="!!operation"
                @click="openStashDrop(selectedStash)"
              >
                <Icon name="trash" :size="14" />
                删除
              </button>
            </div>

            <p class="rail-hint">
              <Icon name="stash" :size="13" />
              应用会把贮藏内容放回工作区；弹出在应用成功后会删除这条贮藏。
            </p>
          </div>
        </aside>

        <aside v-else-if="selectedOids.length > 1" class="surface inspector" aria-label="多选提交">
          <div class="inspector-head">
            <span class="section-label">已选择 {{ selectedOids.length }} 个提交</span>
            <button type="button" class="button ghost tiny" @click="copySelectedHashes">
              <Icon name="copy" :size="14" />
              复制哈希
            </button>
          </div>
          <div class="inspector-body">
            <p class="multi-hint">
              选两个提交可以查看它们之间的差异；也可以把这一串提交按从旧到新的顺序拣选到当前分支。
            </p>

            <div class="multi-actions">
              <button
                type="button"
                class="button primary small"
                :disabled="selectedOids.length !== 2"
                @click="openCompare"
              >
                <Icon name="git-compare" :size="14" />
                对比所选
              </button>
              <button
                type="button"
                class="button ghost small"
                :disabled="!currentBranch"
                @click="openCherryPickSelection"
              >
                <Icon name="git-merge" :size="14" />
                拣选到 {{ currentBranch?.name ?? "当前 HEAD" }}
              </button>
            </div>

            <div class="multi-list">
              <button
                v-for="row in selectedRows"
                :key="row.oid"
                type="button"
                class="multi-row"
                @click="selectSingle(row)"
              >
                <span class="multi-hash">{{ row.shortId }}</span>
                <span class="multi-summary">{{ row.summary }}</span>
              </button>
            </div>
          </div>
        </aside>

        <aside v-else class="surface inspector" aria-label="详情">
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
                <button
                  v-for="file in detail.files"
                  :key="file.path"
                  type="button"
                  class="file-row file-row-button"
                  :title="`查看 ${file.path} 的差异`"
                  @click="openCommitDiff(file.path)"
                >
                  <span class="file-status" :class="`status-${file.status}`">
                    {{ statusText(file.status) }}
                  </span>
                  <span class="file-path">{{ file.path }}</span>
                  <span class="file-stats">
                    <span class="stat-add">+{{ file.additions }}</span>
                    <span class="stat-del">−{{ file.deletions }}</span>
                  </span>
                </button>
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
                @click="onReuseMessage(detail.oid)"
              >
                <Icon name="message" :size="14" />
                复用信息
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
              <button
                v-for="file in detail.files"
                :key="file.path"
                type="button"
                class="file-row file-row-button"
                :title="`查看 ${file.path} 的差异`"
                @click="openCommitDiff(file.path)"
              >
                <span class="file-status" :class="`status-${file.status}`">
                  {{ statusText(file.status) }}
                </span>
                <span class="file-path">{{ file.path }}</span>
                <span class="file-stats">
                  <span class="stat-add">+{{ file.additions }}</span>
                  <span class="stat-del">−{{ file.deletions }}</span>
                </span>
              </button>
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
    <CanvasMenu
      v-if="dropMenu"
      :x="dropMenu.x"
      :y="dropMenu.y"
      :heading="dropMenu.heading"
      :items="dropMenu.items"
      @close="dropMenu = null"
      @select="onDropMenuSelect"
    />
    <InteractiveRebaseDialog
      v-if="rebaseDialog"
      :branch="rebaseDialog.branch"
      :base-label="rebaseDialog.baseLabel"
      :plan="rebasePlan"
      :loading="rebasePlanLoading"
      :error="rebasePlanError"
      @close="closeInteractiveRebase"
      @retry="loadRebasePlan"
      @submit="startInteractiveRebase"
    />
    <ActionDialog
      v-if="dialog"
      :config="dialog.config"
      @close="dialog = null"
      @submit="onDialogSubmit"
    />
  </div>
</template>
