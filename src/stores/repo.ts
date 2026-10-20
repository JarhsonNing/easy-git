import { ref } from "vue";
import {
  errorText,
  getCommit,
  getRepoRefs,
  getRepoStatus,
  getSettings,
  loadGraph,
  openRepository,
} from "../lib/api";
import type {
  BranchInfo,
  CommitDetail,
  CommitRow,
  Notice,
  OperationResult,
  RepoRefs,
  RepoStatus,
  RepoSummary,
} from "../types";

export const session = ref<RepoSummary | null>(null);
export const rows = ref<CommitRow[]>([]);
export const hasMore = ref(false);
export const loading = ref(false);
export const loadingMore = ref(false);
export const error = ref("");
export const selectedOid = ref<string | null>(null);
export const selectedBranch = ref<BranchInfo | null>(null);
export const detail = ref<CommitDetail | null>(null);
export const detailLoading = ref(false);
export const refs = ref<RepoRefs | null>(null);
export const status = ref<RepoStatus | null>(null);
export const operation = ref<string | null>(null);
export const notice = ref<Notice | null>(null);

let pageSize = 500;
let detailRequest = 0;
let noticeTimer: number | undefined;

async function readPageSize() {
  try {
    const config = await getSettings();
    pageSize = Math.max(50, config.git.pageSize || 500);
  } catch {
    pageSize = 500;
  }
}

async function loadInitialSnapshot(): Promise<RepoRefs | null> {
  const [page, refsResult, statusResult] = await Promise.all([
    loadGraph(0, pageSize),
    getRepoRefs(),
    getRepoStatus(),
  ]);
  rows.value = page.rows;
  hasMore.value = page.hasMore;
  refs.value = refsResult;
  status.value = statusResult;
  if (session.value && statusResult.branch) {
    session.value.headBranch = statusResult.branch;
  }
  return refsResult;
}

async function loadDetail(oid: string) {
  const request = ++detailRequest;
  detailLoading.value = true;
  try {
    const result = await getCommit(oid);
    if (request === detailRequest) {
      detail.value = result;
    }
  } catch (e) {
    if (request === detailRequest) {
      detail.value = null;
      error.value = errorText(e);
    }
  } finally {
    if (request === detailRequest) {
      detailLoading.value = false;
    }
  }
}

export async function openRepo(path: string): Promise<boolean> {
  if (loading.value) return false;
  error.value = "";
  loading.value = true;
  session.value = null;
  rows.value = [];
  hasMore.value = false;
  selectedOid.value = null;
  selectedBranch.value = null;
  detail.value = null;
  detailLoading.value = false;
  refs.value = null;
  status.value = null;
  clearNotice();
  try {
    await readPageSize();
    session.value = await openRepository(path);
    const snapshotRefs = await loadInitialSnapshot();
    const current = snapshotRefs?.local.find((branch) => branch.isCurrent) ?? null;
    if (current) {
      selectedBranch.value = current;
      selectedOid.value = current.oid;
      await loadDetail(current.oid);
    }
    return true;
  } catch (e) {
    error.value = errorText(e);
    return false;
  } finally {
    loading.value = false;
  }
}

export function closeRepo() {
  session.value = null;
  rows.value = [];
  hasMore.value = false;
  error.value = "";
  selectedOid.value = null;
  selectedBranch.value = null;
  detail.value = null;
  detailLoading.value = false;
  refs.value = null;
  status.value = null;
  operation.value = null;
  loading.value = false;
  loadingMore.value = false;
  clearNotice();
}

async function reconcileSelection() {
  if (selectedBranch.value) {
    const previous = selectedBranch.value;
    const pool =
      previous.kind === "local"
        ? (refs.value?.local ?? [])
        : (refs.value?.remote ?? []);
    const next = pool.find((branch) => branch.name === previous.name) ?? null;
    if (!next) {
      const current = refs.value?.local.find((branch) => branch.isCurrent) ?? null;
      if (current) {
        await selectBranch(current);
      } else {
        selectedBranch.value = null;
        selectedOid.value = null;
        detail.value = null;
      }
      return;
    }
    selectedBranch.value = next;
    if (selectedOid.value !== next.oid || detail.value?.oid !== next.oid) {
      selectedOid.value = next.oid;
      detail.value = null;
      await loadDetail(next.oid);
    }
    return;
  }

  const oid = selectedOid.value;
  if (!oid) return;
  if (!rows.value.some((row) => row.oid === oid)) {
    selectedOid.value = null;
    detail.value = null;
    return;
  }
  if (!detail.value || detail.value.oid !== oid) {
    detail.value = null;
    await loadDetail(oid);
  }
}

export async function refreshRepo() {
  if (!session.value || loading.value) return;
  error.value = "";
  loading.value = true;
  try {
    await readPageSize();
    await loadInitialSnapshot();
    await reconcileSelection();
  } catch (e) {
    error.value = errorText(e);
  } finally {
    loading.value = false;
  }
}

export async function loadMore() {
  if (!session.value || loading.value || loadingMore.value || !hasMore.value) return;
  error.value = "";
  loadingMore.value = true;
  try {
    const page = await loadGraph(rows.value.length, pageSize);
    const known = new Set(rows.value.map((row) => row.oid));
    rows.value = rows.value.concat(page.rows.filter((row) => !known.has(row.oid)));
    hasMore.value = page.hasMore;
  } catch (e) {
    error.value = errorText(e);
  } finally {
    loadingMore.value = false;
  }
}

export async function selectCommit(row: CommitRow) {
  selectedBranch.value = null;
  selectedOid.value = row.oid;
  if (detail.value?.oid === row.oid) return;
  detail.value = null;
  await loadDetail(row.oid);
}

export async function selectBranch(branch: BranchInfo) {
  selectedBranch.value = branch;
  selectedOid.value = branch.oid;
  if (detail.value?.oid === branch.oid) return;
  detail.value = null;
  await loadDetail(branch.oid);
}

export async function selectOid(oid: string) {
  selectedBranch.value = null;
  selectedOid.value = oid;
  if (detail.value?.oid === oid) return;
  detail.value = null;
  await loadDetail(oid);
}

export function setNotice(next: Notice | null) {
  if (noticeTimer !== undefined) {
    window.clearTimeout(noticeTimer);
    noticeTimer = undefined;
  }
  notice.value = next;
  if (next?.tone === "success") {
    noticeTimer = window.setTimeout(() => {
      notice.value = null;
      noticeTimer = undefined;
    }, 6000);
  }
}

export function clearNotice() {
  setNotice(null);
}

export async function runOperation(
  label: string,
  action: () => Promise<OperationResult>,
): Promise<boolean> {
  if (operation.value) return false;
  operation.value = label;
  error.value = "";
  try {
    const result = await action();
    const conflicting = result.state !== "clean" || result.conflicts.length > 0;
    setNotice(
      conflicting
        ? {
            tone: "conflict",
            message: result.message,
            detail: result.output,
            conflicts: result.conflicts,
            state: result.state,
          }
        : {
            tone: "success",
            message: result.message,
            detail: result.output,
          },
    );
    await refreshRepo();
    return true;
  } catch (e) {
    setNotice({ tone: "error", message: errorText(e) });
    await refreshRepo();
    return false;
  } finally {
    operation.value = null;
  }
}
