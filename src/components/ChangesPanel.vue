<script setup lang="ts">
import { computed, ref, watch } from "vue";
import CanvasMenu from "./CanvasMenu.vue";
import Icon from "./Icon.vue";
import type { MenuItem, WorkingFile, WorkingStage } from "../types";

const props = defineProps<{
  files: WorkingFile[];
  selected: { path: string; stage: WorkingStage } | null;
  truncated?: boolean;
  loading?: boolean;
  disabled?: boolean;
  collapsed?: boolean;
}>();

const emit = defineEmits<{
  (e: "select", file: WorkingFile): void;
  (e: "stage-files", files: WorkingFile[]): void;
  (e: "unstage-files", files: WorkingFile[]): void;
  (e: "discard-files", files: WorkingFile[]): void;
  (e: "stash"): void;
  (e: "refresh"): void;
  (e: "toggle-collapse"): void;
}>();

type ViewMode = "path" | "tree";

interface FileRow {
  kind: "file";
  key: string;
  file: WorkingFile;
  depth: number;
  name: string;
  dir: string;
}

interface DirRow {
  kind: "dir";
  key: string;
  label: string;
  depth: number;
  files: WorkingFile[];
  additions: number;
  deletions: number;
}

type Row = FileRow | DirRow;

interface GroupView {
  key: "conflicts" | "unstaged" | "staged";
  label: string;
  files: WorkingFile[];
  rows: Row[];
  additions: number;
  deletions: number;
  stage: WorkingStage;
}

const STATUS_GLYPH: Record<string, string> = {
  added: "A",
  deleted: "D",
  modified: "M",
  renamed: "R",
  copied: "C",
  typechange: "T",
  conflicted: "U",
};

const viewMode = ref<ViewMode>("path");
const query = ref("");
const checked = ref<Set<string>>(new Set());
const folded = ref<Set<string>>(new Set());
const cardsClosed = ref<Set<string>>(new Set());
const anchor = ref<string | null>(null);
const menu = ref<{ x: number; y: number; item: WorkingFile } | null>(null);

function isCardOpen(key: string): boolean {
  return !cardsClosed.value.has(key);
}

function toggleCard(key: string) {
  const next = new Set(cardsClosed.value);
  if (next.has(key)) next.delete(key);
  else next.add(key);
  cardsClosed.value = next;
}

function fileKey(file: WorkingFile): string {
  return `${file.stage}:${file.path}`;
}

function sameFile(file: WorkingFile, selected: { path: string; stage: WorkingStage }): boolean {
  return file.path === selected.path && file.stage === selected.stage;
}

function baseOf(path: string): string {
  const index = path.lastIndexOf("/");
  return index === -1 ? path : path.slice(index + 1);
}

function dirOf(path: string): string {
  const index = path.lastIndexOf("/");
  return index === -1 ? "" : path.slice(0, index);
}

function folderKey(group: string, path: string): string {
  return `${group}:${path}`;
}

interface TreeNode {
  dirs: Map<string, TreeNode>;
  files: WorkingFile[];
}

function emptyNode(): TreeNode {
  return { dirs: new Map(), files: [] };
}

function buildRows(
  groupKey: string,
  files: WorkingFile[],
  mode: ViewMode,
  closed: Set<string>,
): Row[] {
  if (mode === "path") {
    return files.map((file) => ({
      kind: "file",
      key: fileKey(file),
      file,
      depth: 0,
      name: baseOf(file.path),
      dir: dirOf(file.path),
    }));
  }

  const root: TreeNode = emptyNode();
  for (const file of files) {
    const segments = file.path.split("/");
    let node = root;
    for (const segment of segments.slice(0, -1)) {
      let child = node.dirs.get(segment);
      if (!child) {
        child = emptyNode();
        node.dirs.set(segment, child);
      }
      node = child;
    }
    node.files.push(file);
  }

  const rows: Row[] = [];
  const walk = (node: TreeNode, prefix: string, depth: number) => {
    const dirs = Array.from(node.dirs.entries()).sort(([a], [b]) => a.localeCompare(b));
    for (const [name, child] of dirs) {
      const path = prefix ? `${prefix}/${name}` : name;
      const key = folderKey(groupKey, path);
      const filesInside = collectFiles(child);
      rows.push({
        kind: "dir",
        key,
        label: name,
        depth,
        files: filesInside,
        additions: filesInside.reduce((sum, file) => sum + file.additions, 0),
        deletions: filesInside.reduce((sum, file) => sum + file.deletions, 0),
      });
      if (!closed.has(key)) walk(child, path, depth + 1);
    }
    const sorted = [...node.files].sort((a, b) => baseOf(a.path).localeCompare(baseOf(b.path)));
    for (const file of sorted) {
      rows.push({
        kind: "file",
        key: fileKey(file),
        file,
        depth,
        name: baseOf(file.path),
        dir: "",
      });
    }
  };
  walk(root, "", 0);
  return rows;
}

function collectFiles(node: TreeNode): WorkingFile[] {
  const files = [...node.files];
  for (const child of node.dirs.values()) files.push(...collectFiles(child));
  return files;
}

const filtered = computed(() => {
  const needle = query.value.trim().toLowerCase();
  if (!needle) return props.files;
  return props.files.filter((file) => file.path.toLowerCase().includes(needle));
});

const groups = computed<GroupView[]>(() => {
  const definitions: {
    key: GroupView["key"];
    label: string;
    stage: WorkingStage;
    matches: (file: WorkingFile) => boolean;
  }[] = [
    {
      key: "conflicts",
      label: "冲突",
      stage: "conflicted",
      matches: (file) => file.stage === "conflicted",
    },
    {
      key: "unstaged",
      label: "未暂存",
      stage: "unstaged",
      matches: (file) => file.stage === "unstaged" || file.stage === "untracked",
    },
    {
      key: "staged",
      label: "已暂存",
      stage: "staged",
      matches: (file) => file.stage === "staged",
    },
  ];
  const result: GroupView[] = [];
  for (const definition of definitions) {
    const files = filtered.value
      .filter(definition.matches)
      .sort((a, b) => a.path.localeCompare(b.path));
    if (!files.length && definition.key === "conflicts") continue;
    result.push({
      key: definition.key,
      label: definition.label,
      files,
      rows: buildRows(definition.key, files, viewMode.value, folded.value),
      additions: files.reduce((sum, file) => sum + file.additions, 0),
      deletions: files.reduce((sum, file) => sum + file.deletions, 0),
      stage: definition.stage,
    });
  }
  return result;
});

const hasMatches = computed(() => groups.value.some((group) => group.files.length));

const orderedFiles = computed(() => groups.value.flatMap((group) => group.files));

const checkedFiles = computed(() =>
  orderedFiles.value.filter((file) => checked.value.has(fileKey(file))),
);

const stageable = computed(() =>
  checkedFiles.value.filter(
    (file) => file.stage === "unstaged" || file.stage === "untracked",
  ),
);

const unstageable = computed(() =>
  checkedFiles.value.filter((file) => file.stage === "staged"),
);

const discardable = computed(() =>
  checkedFiles.value.filter((file) => file.stage !== "conflicted"),
);

watch(
  () => props.files,
  (next) => {
    const valid = new Set(next.map(fileKey));
    const kept = new Set(Array.from(checked.value).filter((key) => valid.has(key)));
    if (kept.size !== checked.value.size) checked.value = kept;
    if (anchor.value && !valid.has(anchor.value)) anchor.value = null;
  },
);

watch(query, () => {
  checked.value = new Set();
  anchor.value = null;
});

function isChecked(file: WorkingFile): boolean {
  return checked.value.has(fileKey(file));
}

function setChecked(next: Set<string>) {
  checked.value = next;
}

function toggleCheck(file: WorkingFile, extend = false) {
  const key = fileKey(file);
  const next = new Set(checked.value);
  if (next.has(key)) {
    next.delete(key);
  } else {
    next.add(key);
    if (extend && anchor.value) {
      const from = orderedFiles.value.findIndex((item) => fileKey(item) === anchor.value);
      const to = orderedFiles.value.findIndex((item) => fileKey(item) === key);
      if (from !== -1 && to !== -1) {
        const [low, high] = from < to ? [from, to] : [to, from];
        for (let index = low; index <= high; index += 1) {
          next.add(fileKey(orderedFiles.value[index]));
        }
      }
    }
  }
  if (!next.has(key)) {
    if (anchor.value === key) anchor.value = null;
  } else {
    anchor.value = key;
  }
  setChecked(next);
}

function selectGroup(group: GroupView) {
  const keys = group.files.map(fileKey);
  const every = keys.every((key) => checked.value.has(key));
  const next = new Set(checked.value);
  for (const key of keys) {
    if (every) next.delete(key);
    else next.add(key);
  }
  setChecked(next);
}

function groupChecked(group: GroupView): boolean {
  return group.files.length > 0 && group.files.every((file) => checked.value.has(fileKey(file)));
}

function groupPartial(group: GroupView): boolean {
  return !groupChecked(group) && group.files.some((file) => checked.value.has(fileKey(file)));
}

function selectDir(row: DirRow) {
  const keys = row.files.map(fileKey);
  const every = keys.every((key) => checked.value.has(key));
  const next = new Set(checked.value);
  for (const key of keys) {
    if (every) next.delete(key);
    else next.add(key);
  }
  setChecked(next);
}

function toggleFolder(row: DirRow) {
  const next = new Set(folded.value);
  if (next.has(row.key)) next.delete(row.key);
  else next.add(row.key);
  folded.value = next;
}

function isCollapsed(row: DirRow): boolean {
  return folded.value.has(row.key);
}

function onRowClick(event: KeyboardEvent | MouseEvent, file: WorkingFile) {
  if (event.shiftKey) {
    event.preventDefault();
    toggleCheck(file, true);
    return;
  }
  if (event.metaKey || event.ctrlKey) {
    event.preventDefault();
    toggleCheck(file);
    return;
  }
  const key = fileKey(file);
  setChecked(new Set([key]));
  anchor.value = key;
  emit("select", file);
}

function toggleFile(file: WorkingFile) {
  if (props.disabled) return;
  if (file.stage === "conflicted") emit("select", file);
  else if (file.stage === "staged") emit("unstage-files", [file]);
  else emit("stage-files", [file]);
}

function discardFile(file: WorkingFile) {
  if (props.disabled || file.stage === "conflicted") return;
  emit("discard-files", [file]);
}

function stageGroup(group: GroupView) {
  if (props.disabled) return;
  if (group.key === "conflicts") {
    const first = group.files[0];
    if (first) emit("select", first);
    return;
  }
  if (group.stage === "staged") emit("unstage-files", group.files);
  else emit("stage-files", group.files);
}

function cardActionLabel(group: GroupView): string {
  if (group.key === "conflicts") return "逐个合并";
  return group.stage === "staged" ? "取消暂存" : "全部暂存";
}

function cardActionIcon(group: GroupView): string {
  if (group.key === "conflicts") return "git-merge";
  return group.stage === "staged" ? "minus" : "plus";
}

function cardEmptyText(group: GroupView): string {
  if (query.value.trim()) return "没有匹配的文件";
  if (group.key === "staged") return "还没有已暂存的文件";
  return "没有未暂存的改动";
}

function rowToggleLabel(file: WorkingFile): string {
  if (file.stage === "staged") return "取消暂存";
  if (file.stage === "conflicted") return "合并";
  return "暂存";
}

function rowToggleTitle(file: WorkingFile): string {
  if (file.stage === "staged") return "取消暂存";
  if (file.stage === "conflicted") return "打开三方合并编辑器";
  return "暂存文件";
}

function stageChecked() {
  if (props.disabled) return;
  emit("stage-files", stageable.value);
}

function unstageChecked() {
  if (props.disabled) return;
  emit("unstage-files", unstageable.value);
}

function discardChecked() {
  if (props.disabled || !discardable.value.length) return;
  emit("discard-files", discardable.value);
}

function clearChecked() {
  setChecked(new Set());
  anchor.value = null;
}

function openMenu(event: MouseEvent, file: WorkingFile) {
  event.preventDefault();
  event.stopPropagation();
  if (props.disabled) return;
  if (!checked.value.has(fileKey(file))) {
    setChecked(new Set([fileKey(file)]));
    anchor.value = fileKey(file);
  }
  menu.value = { x: event.clientX, y: event.clientY, item: file };
}

const menuItems = computed<MenuItem[]>(() => {
  const files = checkedFiles.value.length
    ? checkedFiles.value
    : menu.value
      ? [menu.value.item]
      : [];
  if (!files.length) return [];
  const count = files.length > 1 ? ` ${files.length} 个文件` : "";
  const staged = files.every((file) => file.stage === "staged");
  const conflicted = files.some((file) => file.stage === "conflicted");
  return [
    {
      id: "stage",
      label: staged ? `取消暂存${count}` : `暂存${count || "文件"}`,
      icon: staged ? "minus" : "plus",
    },
    {
      id: "discard",
      label: `丢弃${count || "文件"}的更改`,
      icon: "trash",
      danger: true,
      disabled: conflicted,
    },
    { separator: true },
    { id: "copy", label: "复制文件路径", icon: "copy", disabled: files.length !== 1 },
  ];
});

function onMenuSelect(id: string) {
  const state = menu.value;
  menu.value = null;
  if (!state) return;
  const files = checkedFiles.value.length ? checkedFiles.value : [state.item];
  if (id === "stage") {
    if (files.every((file) => file.stage === "staged")) emit("unstage-files", files);
    else emit("stage-files", files.filter((file) => file.stage !== "staged"));
    return;
  }
  if (id === "discard") {
    emit("discard-files", files.filter((file) => file.stage !== "conflicted"));
    return;
  }
  if (id === "copy") {
    void navigator.clipboard.writeText(state.item.path);
  }
}
</script>

<template>
  <div class="changes-panel">
    <div class="changes-head">
      <button
        type="button"
        class="changes-heading panel-collapse"
        :aria-expanded="!collapsed"
        :title="collapsed ? '展开变更' : '折叠变更'"
        @click="emit('toggle-collapse')"
      >
        <Icon :name="collapsed ? 'chevron-right' : 'chevron-down'" :size="13" />
        <span class="section-label">变更</span>
        <span class="rail-count">{{ files.length }}</span>
        <span v-if="truncated" class="changes-truncated">仅显示前 {{ files.length }} 个</span>
      </button>
      <div class="changes-head-actions">
        <button
          type="button"
          class="button ghost tiny"
          title="刷新变更"
          :disabled="disabled || loading"
          @click="$emit('refresh')"
        >
          <Icon name="refresh" :size="13" />
          刷新
        </button>
        <button
          type="button"
          class="button ghost tiny"
          title="贮藏当前全部变更"
          :disabled="disabled"
          @click="$emit('stash')"
        >
          <Icon name="stash" :size="13" />
          贮藏
        </button>
      </div>
    </div>

    <template v-if="!collapsed">
    <div class="changes-tools">
      <label class="changes-filter">
        <Icon name="search" :size="13" />
        <input v-model="query" type="search" placeholder="筛选文件" aria-label="筛选文件" />
        <button
          v-if="query"
          type="button"
          class="filter-clear"
          title="清除文件筛选"
          @click.prevent.stop="query = ''"
        >
          <Icon name="close" :size="11" />
          清除
        </button>
      </label>
      <div class="view-toggle" role="group" aria-label="文件排列方式">
        <button
          type="button"
          :class="{ active: viewMode === 'path' }"
          title="按路径显示"
          @click="viewMode = 'path'"
        >
          <Icon name="list" :size="13" />
          列表
        </button>
        <button
          type="button"
          :class="{ active: viewMode === 'tree' }"
          title="按目录树显示"
          @click="viewMode = 'tree'"
        >
          <Icon name="tree" :size="13" />
          目录
        </button>
      </div>
    </div>

    <div v-if="loading && !files.length" class="changes-empty">
      <span class="loading-orbit" aria-hidden="true"></span>
      正在读取工作区…
    </div>

    <div v-else-if="query && !hasMatches" class="changes-empty">
      <Icon name="search" :size="20" />
      <p>没有匹配「{{ query.trim() }}」的文件</p>
      <button type="button" class="button ghost tiny" @click="query = ''">清除筛选</button>
    </div>

    <div v-else class="changes-cards">
      <section
        v-for="group in groups"
        :key="group.key"
        class="change-card"
        :class="[`card-${group.key}`, { 'is-collapsed': !isCardOpen(group.key) }]"
        :aria-label="`${group.label}文件`"
      >
        <header class="change-card-head">
          <button
            type="button"
            class="card-chevron"
            :aria-expanded="isCardOpen(group.key)"
            :title="isCardOpen(group.key) ? `折叠${group.label}` : `展开${group.label}`"
            @click.stop="toggleCard(group.key)"
          >
            <Icon
              :name="isCardOpen(group.key) ? 'chevron-down' : 'chevron-right'"
              :size="12"
            />
          </button>
          <label class="group-check" @click.stop>
            <input
              type="checkbox"
              :checked="groupChecked(group)"
              :indeterminate="groupPartial(group)"
              :disabled="disabled || !group.files.length"
              @change="selectGroup(group)"
            />
          </label>
          <span class="group-label">{{ group.label }}</span>
          <span class="group-count">{{ group.files.length }}</span>
          <span class="group-stats">
            <span v-if="group.additions" class="stat-add">+{{ group.additions }}</span>
            <span v-if="group.deletions" class="stat-del">−{{ group.deletions }}</span>
          </span>
          <button
            v-if="group.files.length"
            type="button"
            class="card-action"
            :disabled="disabled"
            :title="cardActionLabel(group)"
            @click.stop="stageGroup(group)"
          >
            <Icon :name="cardActionIcon(group)" :size="12" />
            {{ cardActionLabel(group) }}
          </button>
        </header>

        <div v-show="isCardOpen(group.key)" class="change-card-body">
          <div
            v-for="row in group.rows"
            :key="row.key"
            class="change-row"
            :class="{
              active: row.kind === 'file' && selected && sameFile(row.file, selected),
              checked: row.kind === 'file' && isChecked(row.file),
              folder: row.kind === 'dir',
            }"
            :style="{ paddingLeft: `${8 + row.depth * 14}px` }"
            :title="row.kind === 'file' ? row.file.path : row.label"
            :role="row.kind === 'file' ? 'button' : undefined"
            :tabindex="row.kind === 'file' ? 0 : undefined"
            @click="
              row.kind === 'file'
                ? onRowClick($event, row.file)
                : toggleFolder(row)
            "
            @keydown.enter.self.prevent="
              row.kind === 'file' ? onRowClick($event, row.file) : undefined
            "
            @contextmenu="row.kind === 'file' ? openMenu($event, row.file) : undefined"
          >
            <template v-if="row.kind === 'dir'">
              <button
                type="button"
                class="folder-chevron"
                :aria-label="isCollapsed(row) ? '展开目录' : '收起目录'"
                @click.stop="toggleFolder(row)"
              >
                <Icon
                  :name="isCollapsed(row) ? 'chevron-right' : 'chevron-down'"
                  :size="12"
                />
              </button>
              <label class="row-check" @click.stop>
                <input
                  type="checkbox"
                  :checked="row.files.every((file) => checked.has(fileKey(file)))"
                  :indeterminate="
                    !row.files.every((file) => checked.has(fileKey(file))) &&
                    row.files.some((file) => checked.has(fileKey(file)))
                  "
                  @change="selectDir(row)"
                />
              </label>
              <span class="folder-icon" aria-hidden="true"><Icon name="folder" :size="13" /></span>
              <span class="change-name">
                <span class="change-name-label">{{ row.label }}</span>
                <span class="change-dir">{{ row.files.length }}</span>
              </span>
              <span class="file-stats">
                <span v-if="row.additions" class="stat-add">+{{ row.additions }}</span>
                <span v-if="row.deletions" class="stat-del">−{{ row.deletions }}</span>
              </span>
            </template>

            <template v-else>
              <label class="row-check" @click.stop>
                <input
                  type="checkbox"
                  :checked="isChecked(row.file)"
                  @change="toggleCheck(row.file, ($event as MouseEvent).shiftKey)"
                />
              </label>
              <span class="file-status" :class="`status-${row.file.status}`">
                {{ STATUS_GLYPH[row.file.status] ?? "M" }}
              </span>
              <span class="change-name">
                <span
                  class="change-name-label"
                  :class="{ deleted: row.file.status === 'deleted' }"
                >{{ row.name }}</span>
                <span v-if="row.dir" class="change-dir">{{ row.dir }}</span>
              </span>
              <span class="file-stats">
                <span v-if="row.file.additions" class="stat-add">+{{ row.file.additions }}</span>
                <span v-if="row.file.deletions" class="stat-del">−{{ row.file.deletions }}</span>
              </span>
              <span class="change-actions">
                <button
                  type="button"
                  class="row-action"
                  :disabled="disabled"
                  :title="rowToggleTitle(row.file)"
                  @click.stop="toggleFile(row.file)"
                >
                  <Icon
                    :name="
                      row.file.stage === 'staged'
                        ? 'minus'
                        : row.file.stage === 'conflicted'
                          ? 'git-merge'
                          : 'plus'
                    "
                    :size="12"
                  />
                  {{ rowToggleLabel(row.file) }}
                </button>
                <button
                  type="button"
                  class="row-action danger-text"
                  :disabled="disabled || row.file.stage === 'conflicted'"
                  title="丢弃更改"
                  @click.stop="discardFile(row.file)"
                >
                  <Icon name="trash" :size="12" />
                  丢弃
                </button>
              </span>
            </template>
          </div>

          <p v-if="!group.files.length" class="change-card-empty">
            {{ cardEmptyText(group) }}
          </p>
        </div>
      </section>
    </div>

    <div v-if="checkedFiles.length" class="change-batch" role="status">
      <span class="batch-count">{{ checkedFiles.length }} 个已选</span>
      <div class="batch-actions">
        <button
          v-if="stageable.length"
          type="button"
          class="button ghost tiny"
          :disabled="disabled"
          @click="stageChecked"
        >
          <Icon name="plus" :size="12" />
          暂存
        </button>
        <button
          v-if="unstageable.length"
          type="button"
          class="button ghost tiny"
          :disabled="disabled"
          @click="unstageChecked"
        >
          <Icon name="minus" :size="12" />
          取消暂存
        </button>
        <button
          type="button"
          class="button ghost tiny danger-text"
          :disabled="disabled || !discardable.length"
          @click="discardChecked"
        >
          <Icon name="trash" :size="12" />
          丢弃
        </button>
        <button
          type="button"
          class="button ghost tiny"
          title="清空选择"
          @click="clearChecked"
        >
          <Icon name="close" :size="12" />
          清除
        </button>
      </div>
    </div>
    </template>

    <CanvasMenu
      v-if="menu"
      :x="menu.x"
      :y="menu.y"
      :items="menuItems"
      @close="menu = null"
      @select="onMenuSelect"
    />
  </div>
</template>
