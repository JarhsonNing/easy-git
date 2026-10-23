---
name: Easy Git
description: A local-first desktop Git client drawn as a river atlas, not an admin console.
colors:
  survey-paper: "#f5efe3"
  paper-fold: "#ebe1ce"
  fresh-sheet: "#fbf7ee"
  button-ivory: "#fffcf4"
  atlas-ink: "#20241f"
  soft-graphite: "#45493f"
  contour-gray: "#5e6157"
  hairline: "rgba(32, 36, 31, 0.15)"
  survey-rule: "rgba(32, 36, 31, 0.28)"
  channel-teal: "#1f6e6a"
  deep-channel: "#185753"
  attention-coral: "#c9563c"
  signal-coral: "#a3412a"
  field-ochre: "#b47a2c"
  deep-ochre: "#7a5415"
  moss-success: "#587a4b"
  deep-moss: "#2f6b4c"
  assistant-plum: "#765c88"
  deep-plum: "#5c466d"
  river-slate: "#5c7890"
typography:
  display:
    fontFamily: "Bricolage Grotesque, Microsoft YaHei, PingFang SC, Noto Sans CJK SC, sans-serif"
    fontSize: "42px"
    fontWeight: 550
    lineHeight: 1.2
    letterSpacing: "-0.03em"
  headline:
    fontFamily: "Bricolage Grotesque, Microsoft YaHei, PingFang SC, Noto Sans CJK SC, sans-serif"
    fontSize: "34px"
    fontWeight: 550
    lineHeight: 1.2
    letterSpacing: "-0.03em"
  title:
    fontFamily: "Bricolage Grotesque, Microsoft YaHei, PingFang SC, Noto Sans CJK SC, sans-serif"
    fontSize: "19px"
    fontWeight: 600
    lineHeight: 1.3
    letterSpacing: "-0.015em"
  body:
    fontFamily: "Public Sans, Microsoft YaHei, PingFang SC, Noto Sans CJK SC, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: "normal"
  label:
    fontFamily: "Public Sans, Microsoft YaHei, PingFang SC, Noto Sans CJK SC, sans-serif"
    fontSize: "11px"
    fontWeight: 700
    lineHeight: 1.4
    letterSpacing: "0.09em"
  code:
    fontFamily: "Cascadia Code, Cascadia Mono, JetBrains Mono, Consolas, Liberation Mono, monospace"
    fontSize: "11px"
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: "normal"
rounded:
  sm: "6px"
  md: "8px"
  field: "9px"
  lg: "10px"
  xl: "12px"
  surface: "14px"
  pill: "999px"
spacing:
  "2xs": "4px"
  xs: "6px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  xl: "24px"
  "2xl": "36px"
  "3xl": "56px"
components:
  button-primary:
    backgroundColor: "{colors.channel-teal}"
    textColor: "{colors.button-ivory}"
    rounded: "{rounded.lg}"
    padding: "8px 14px"
    height: "36px"
  button-primary-hover:
    backgroundColor: "{colors.deep-channel}"
    textColor: "{colors.button-ivory}"
    rounded: "{rounded.lg}"
    padding: "8px 14px"
    height: "36px"
  button-ghost:
    backgroundColor: "rgba(251, 247, 238, 0.35)"
    textColor: "{colors.soft-graphite}"
    rounded: "{rounded.lg}"
    padding: "8px 14px"
    height: "36px"
  button-danger:
    backgroundColor: "{colors.signal-coral}"
    textColor: "{colors.button-ivory}"
    rounded: "{rounded.lg}"
    padding: "8px 14px"
    height: "36px"
  button-danger-quiet:
    backgroundColor: "transparent"
    textColor: "{colors.signal-coral}"
    rounded: "{rounded.md}"
    padding: "6px 10px"
  nav-item-active:
    backgroundColor: "{colors.survey-paper}"
    textColor: "{colors.atlas-ink}"
    rounded: "{rounded.pill}"
    padding: "6px 13px"
  chip-tag:
    backgroundColor: "{colors.fresh-sheet}"
    textColor: "{colors.soft-graphite}"
    rounded: "{rounded.pill}"
    padding: "2px 8px"
  field-input:
    backgroundColor: "{colors.fresh-sheet}"
    textColor: "{colors.atlas-ink}"
    rounded: "{rounded.field}"
    padding: "8px 10px"
    height: "34px"
  quick-card:
    backgroundColor: "{colors.fresh-sheet}"
    textColor: "{colors.atlas-ink}"
    rounded: "{rounded.xl}"
    padding: "13px 14px"
---

# Design System: Easy Git

## Overview

**Creative North Star: "The River Atlas"**

A repository is a watershed and its history is a river network; branches split and merge like tributaries. Easy Git is the atlas you work in: warm survey paper, an ink sky at the top, hairline survey rules, and generated delta line art that draws itself when a repository opens. The project index reads like an atlas index; a project opens into a working map of its own history. Nothing here is meant to feel like a dashboard, a control panel, or a backoffice.

Density is treated as legibility rather than authority. Most surfaces sit unframed on paper and are separated by 1px rules; contained cards exist only for self-contained groups such as a merge-request report or a settings block. The one ink field is the 64px app bar; everything below it stays paper, sheet, or paper-deep. Actions speak in teal, attention and destruction in coral, warnings and tags in ochre, success in moss, and AI in plum — each color carries a meaning, none is decoration.

The type pairing does the quiet work: Bricolage Grotesque gives Latin display, project names, and numerals a measured, cartographic character; Public Sans and Microsoft YaHei keep the Chinese interface plain and fast to read; mono is confined to hashes, paths, routes, and code. Motion is short and purposeful — a view settles in 0.22s, a sheet slides in 0.3s, the delta plate draws in under a second — and all of it yields to `prefers-reduced-motion`.

**Key Characteristics:**
- Warm survey paper field under a single ink app bar.
- Structure from hairline rules and paper tones, not panels or resting shadows.
- One teal action voice; semantic accents stay rare and meaningful.
- Generated delta line art as the identity, echoed by the commit graph.
- Tabular numerals and an 11px functional type floor.
- One 1180px column for index surfaces; work area plus one fixed rail or list pane inside a project, where the commit view's rail carries repository refs and the inspector follows the selection.
- Chinese-first copy with a Chinese-aware font stack, never Latin tracking on CJK.

### Mark & icon

The identity mark is the delta itself, drawn rather than typeset: one Survey Paper trunk enters from the lower left, meets a split node, and fans into three channels — terracotta upslope, sage straight out, ochre toward the sea. It is the commit graph reduced to its one gesture, and it uses the same channel colors the graph lanes use. `brand/easy-git-icon.svg` is the single source of truth; `npx tauri icon brand/easy-git-icon.svg -o src-tauri/icons` regenerates every desktop asset from it.

- **App icon:** a 1024² Atlas Ink plate at a 192px corner radius. Trunk at 98, channels at 76 / 70 / 74 with round caps, a split node at r=64 and three mouth dots at r=34 in Survey Paper. Weights are chosen so the fan still separates at 32px.
- **App bar mark:** the same geometry at 28×24, inline in `src/App.vue` and plateless because the bar is already ink. Channels even out to a single 2.2px stroke, the trunk holds at 2.6, and the mouth dots drop — at that size they read as breaks in the line rather than as nodes.

## Colors

A warm paper-and-ink atlas palette, with one cool working accent and four meaning-bound signal colors.

### Primary
- **Channel Teal** (#1f6e6a): the single action voice. Primary buttons, active navigation icon, focus rings, selected commit tint, open merge-request state, connection dots, link emphasis. Its rarity is what makes an action obvious.
- **Deep Channel** (#185753): hover and pressed state for teal actions, inline text links, and stronger emphasis on paper.

### Secondary
- **Attention Coral** (#c9563c): attention and deletion — destructive buttons, negative diff numbers, error-adjacent marks.
- **Signal Coral** (#a3412a): the deeper destructive fill used for confirm actions and quiet destructive text.

### Tertiary
- **Field Ochre** (#b47a2c): warnings, draft tags, and informational alerts.
- **Deep Ochre** (#7a5415): ochre text on tinted backgrounds.
- **Moss Success** (#587a4b): success marks and completed states.
- **Deep Moss** (#2f6b4c): success text and icons on tinted fields.
- **Assistant Plum** (#765c88): AI only — avatar, model mark, and AI-context accents.
- **Deep Plum** (#5c466d): plum text and icons on AI-tinted fields.
- **River Slate** (#5c7890): a sixth channel color reserved for the delta art and graph; never an action.

### Neutral
- **Survey Paper** (#f5efe3): the application field, sheets, and the active app-nav pill.
- **Paper Fold** (#ebe1ce): insets, hover fills, quiet chips, and the bottom of the tonal stack.
- **Fresh Sheet** (#fbf7ee): contained surfaces, menus, and the fill of fields at rest.
- **Button Ivory** (#fffcf4): text on teal and coral fills; never a page background.
- **Atlas Ink** (#20241f): the app bar fill, primary text, avatars, and the darker end of the delta ink.
- **Soft Graphite** (#45493f): secondary text and ghost-button labels.
- **Contour Gray** (#5e6157): metadata, placeholders, and helper copy.
- **Hairline** (rgba(32, 36, 31, 0.15)): every interior rule and resting border.
- **Survey Rule** (rgba(32, 36, 31, 0.28)): page-level rules, field borders, and structural boundaries.

### Named Rules
**The Ink Sky Rule.** Atlas Ink fills the 64px app bar and nothing else of scale. Every other large surface stays paper, sheet, or paper-deep; if a screen feels unanchored, add a rule or a heading, not a dark panel.

**The One Signal Rule.** Teal is for the one primary action in a region. Coral, ochre, moss, and plum are semantic and may be absent from a screen entirely. None of them is decorative.

## Typography

**Display Font:** Bricolage Grotesque (with Microsoft YaHei, PingFang SC, Noto Sans CJK SC)
**Body Font:** Public Sans (with Microsoft YaHei, PingFang SC, Noto Sans CJK SC)
**Label/Mono Font:** Cascadia Code (with Cascadia Mono, JetBrains Mono, Consolas)

**Character:** Bricolage's slightly irregular grotesque gives Latin names and numerals a surveyor's hand; Public Sans and YaHei keep the Chinese interface quiet and immediate. Mono appears only where a machine wrote the string.

### Hierarchy
- **Display** (550, 42px, 1.2, −0.03em): index page titles such as 项目 and 设置; 38px below 1080px. One per view.
- **Headline** (550, 34px, 1.2, −0.03em): empty-state and AI greeting headings; 30px below 1180px, 25px at 700px height or shorter.
- **Title** (550–600, 18–24px, 1.2–1.3): the workspace view title at 24px over a 12px context line, project names at 19px, sheet titles and merge-request detail titles at 23px, the browse detail head at 18px.
- **Body** (400, 12.5–14px, 1.5–1.8): the working interface; long explanatory copy sits at 12.5px/1.75 and is capped near 34–56ch. Base UI is 14px, but most operational text is 12.5–13px because desktop scanning tolerates compactness.
- **Label** (700, 11px, 1.4, 0.09em): section labels and table headers, uppercase only when the text is Latin. Chinese labels keep normal case.
- **Code** (400, 11px mono, tabular): hashes, paths, branch routes, file paths, model addresses, and code. Never prose.

### Named Rules
**The 11px Floor Rule.** No functional label, table header, badge, or metadata drops below 11px; prose stays at 12px or larger. Reduce content before shrinking type.

**The Chinese Pair Rule.** Bricolage Grotesque carries Latin display, project names, and numerals only. Chinese always falls through to Microsoft YaHei or PingFang in the same stack and never receives Latin uppercase tracking.

## Layout

The app is a single vertical shell: a 64px ink app bar over a scrolling main region. The bar is a three-column grid (1fr / auto / 1fr) with 22px insets: brand, back control, project name, and branch pill on the left; a centered pill navigation for 提交图谱 / 合并请求 / 代码库 / AI 助手; settings on the right. There is no sidebar and no persistent toolrail outside the app bar.

Index surfaces — project list and settings — use one column of `min(1180px, 100%)` with 42px / 36px / 56px padding, a 42px page title, a short lede capped at 54ch, and a page rule beneath. Project rows are full-width, 88px minimum, with an 84×52 delta plate, a flexible name/path column, and a circular open control. The search line is a 320px pill on the left with a tabular repository count on the right.

Project workspaces skip the index page heading and open instead on a workspace bar: a 24px view title over one 12px context line, with labeled controls aligned to its bottom edge — the GitLab instance select, 刷新, or the view's own actions. The commit view is a three-column atlas: a 236px refs rail, the 1fr work sheet, and a 348px dossier, 16px apart; table headers are 34px, commit rows 42px. The rail holds only repository refs — local and remote branches, tags, and stashes — and never changes modes. Above the commit table sits a filter line (摘要 / 作者 / 哈希 / 分支) and, when several commits are checked, a selection bar with 对比 / 拣选 / 复制哈希; a dashed `WIP` row stays pinned at the top of the sheet and is the only entry point to uncommitted work. The workspace bar's 获取 / 拉取 / 推送 cluster is joined by 撤销, a reflog-backed single-step undo: it stays disabled unless the latest HEAD move is reversible (commit, amend, checkout, merge, rebase, pull, reset, cherry-pick, revert), names that action in its tooltip, and confirms through the standard dialog with the exact target — a commit undo resets softly so the work returns staged, a checkout undo returns to the previous branch, and a hard undo that would discard tracked changes is refused with the reason instead of executed. Selection drives the dossier: WIP opens working changes plus the composer, a stash opens stash detail, a branch or tag opens ref detail, one commit opens commit detail, and several commits open a selection dossier with 对比所选 and 拣选. Clicking a working file or a commit file replaces the sheet with a diff carrying a 返回图谱 backbar; comparing two commits replaces it with a from→to report. The merge-request view is a 320–380px list plus a flexible report pane; the list narrows to 350px at 1180px and 320px at 1080px. AI is a centered workspace up to 1120px with an 820px conversation column. Settings is a 200px sticky index plus a form column on paper.

The 代码库 view sits under that bar as a two-column field: a `minmax(300px, 360px)` project rail and a 1fr detail sheet, 16px apart. The rail is a search-headed list of hairline rows — 13px name over a mono path, a 10.5px mono relative time pinned at the right edge, and tags for 当前仓库 / 复刻 / 已归档 — and it pages through a quiet 加载更多 control. The detail sheet is a single flex column that fills the field: project head, then the 代码 / 流水线 tab line, then the pane, with head, dock, tabs and alerts held fixed while only the pane scrolls. 代码 splits a `minmax(220px, 280px)` tree from the file viewer: the tree head carries the branch select, rows indent 14px per level with an ochre chevron for directories, and the viewer keeps a 48px right-aligned tabular line-number gutter beside 11.5px mono text, naming binary content and the 256 KB read cap in place of the file. 流水线 splits a `minmax(260px, 320px)` list from the job pane: each pipeline row is a status dot, mono branch, short SHA, and duration, and the pane groups jobs by stage over a full-height trace. Below 1180px the rail tightens to 320px, the tree to 220px, and the pipeline list to 260px; below 1080px the rail goes to 280px and both duration columns are dropped rather than truncated.

Below 1180px the refs rail becomes a drawer: it slides over the work area from the left at 264px with a veil, opened by the 分支 ghost button in the toolbar and closed by its own circular control or the veil. It never covers the app bar.

Breakpoints are 1340px, 1180px, 1080px, and 980px; a max-height pass at 700px compresses the AI empty state. The supported window runs from 1000×640 to 1280×820 and wider; the SHA column drops below 1340px, author below 1180px, and date below 1080px, so the commit summary keeps its reading width before the work area is allowed to feel cramped.

The rhythm is 4/6/8/12/16/24/36/56. Panel insets are 22–24px, row gaps 10–15px, and section separation 24–36px. Density is welcome; crowding is not.

### Named Rules
**The One Column Rule.** Index and settings surfaces use a single 1180px column. Project workspaces split into work area plus at most two fixed rails (a domain rail and one inspector). No app-navigation sidebar and no widget grid; the domain rail carries repository content only — branches, remotes, tags, and stashes; working changes belong to the WIP dossier — never pages or app modes.

## Elevation & Depth

The system is flat at rest. Depth comes from two paper tones and hairlines: survey paper is the field, fresh sheet is a contained surface, paper fold is an inset or hover fill. Resting content never floats.

Shadows exist only for things that genuinely overlay the field: the project menu, the sheet, and transient popovers. The sheet uses a directional shadow that reads as a page sliding over the atlas. Focus is a colored event, not an elevation: fields gain a 3px teal halo at 11% and a stronger teal border; every focusable control keeps a 2px teal outline with a 2px offset (paper on the ink bar).

### Shadow Vocabulary
- **Floating Menu** (`0 18px 38px -24px rgba(32, 36, 31, 0.55), 0 3px 10px rgba(32, 36, 31, 0.08)`): project overflow menu and small popovers.
- **Sheet Edge** (`-22px 0 60px -34px rgba(32, 36, 31, 0.55)`): the right-side sheet panel only.
- **Local Lift** (`0 1px 2px rgba(32, 36, 31, 0.09)`): a single-pixel lift for small raised controls where a hairline alone would disappear.

### Named Rules
**The Floating-Only Rule.** Shadows belong to overlays. Resting cards, rows, and panels are separated by hairlines and paper tones; if content needs emphasis, change its type or add a rule, not a shadow.

## Shapes

The form language is drafting-instrument geometry: plates and marks at 6px, working controls at 8–10px, contained surfaces at 12–14px, and pills or true circles at 999px. Nothing is softer than 14px except pills; there are no blob cards and no mixed radii within one element.

Circles mark openings, returns, avatars, menu triggers, and status dots. Pills carry navigation tabs, filters, search, branch refs, and tags. Hairline rules and 1px borders do the structural work; containers earn a border only when they group a self-contained block. The delta plate is a clipped 84×52 landscape with a 6px corner — a map inset, not a thumbnail card.

### Named Rules
**The Quiet Corner Rule.** Use 6–10px for controls and plates, 12–14px for surfaces, and 999px for pills and circles. Never exceed 14px except for a pill.

## Components

### Buttons
- **Shape:** 10px radius, 36px minimum height, 8×14px padding, 12.5px/600 label; small is 32px/12px and tiny is 26px/11px with an 8px radius.
- **Primary:** Channel Teal fill with Button Ivory text; hover deepens to Deep Channel. One primary action per region.
- **Hover / Focus:** color transitions at 0.16s; 2px teal focus outline with 2px offset, paper outline on the ink bar.
- **Secondary / Ghost:** Survey Rule border over translucent sheet; hover moves to fresh sheet and Atlas Ink text.
- **Danger:** Signal Coral fill for the confirming action; quiet destructive text is Signal Coral on transparency with a 9% coral wash on hover.

### Chips
- **Style:** 999px pill, 1px hairline border, translucent fresh sheet, 11px Contour or Graphite text. Branch refs use mono inside the same pill.
- **State:** active context chips switch to an 8% teal wash with a teal border and Deep Channel text; the “current project” tag follows the same treatment.

### Cards / Containers
- **Corner Style:** 12px for quick actions and menus, 14px for contained report surfaces.
- **Background:** Fresh Sheet at 92% over Survey Paper, or Paper Fold for insets.
- **Shadow Strategy:** flat at rest; see Elevation & Depth.
- **Border:** 1px hairline; the page-level rule uses Survey Rule.
- **Internal Padding:** 13–15px for compact cards, 20–24px for report panels.

### Inputs / Fields
- **Style:** 9px radius, Survey Rule border, translucent fresh sheet, 10px label at 600 with 0.03em tracking, 12.5px value text. Search is the one 999px pill field at 38px height.
- **Focus:** teal border at 55% plus a 3px 11% teal halo; the field fills to Fresh Sheet.
- **Error / Disabled:** disabled controls drop to 42% opacity; errors use coral text and a coral hairline rather than a filled red field.

### Navigation
- **App bar:** 64px Atlas Ink, fixed; brand at 18px display weight, project name at 13px/550 with a mono branch pill beneath, centered pill nav with 12px/550 items, active item on Survey Paper with a teal icon, settings as a circular icon button.
- **In-view:** tabs are underlined text or pill filters, never a second bar. The settings index is a vertical list with counts that becomes horizontal pills below 980px.
- **No sidebars for navigation.** The project context always shows where you are and offers one circular return control. The one exception is the commit view's domain rail: it is a map legend for the repository, not a navigation surface — it lists branches, remotes, tags, and stashes, never pages or app modes. Uncommitted work is reached through the pinned WIP row, not a mode switch.

### Delta Plate (signature)
A generated river delta drawn from the repository path in six channel colors (teal, coral, ochre, moss, plum, slate) over dotted survey contours. It animates once when a project opens: contours fade in, channels draw in 0.95s with staggered delays, joints and endpoints settle, and a slow current pulses along one channel. The same branching language becomes the commit graph, so the identity is functional, not ornamental.

### Project Row (signature)
A full-width row on paper, separated by hairlines: 84×52 delta plate, 19px display name with a teal “正在查看” tag, mono path at 11px, branch metadata at 11px, and a 34px circle that fills teal and lifts one pixel on hover. Hover warms the row rather than raising it.

### Project Index Sources (signature)
The 项目 index is one list with two sources, switched by a 本地 / GitLab segmented pill in the index bar — the same pill language the merge-request states use, with the selected half lifted onto a sheet and the other left transparent so the choice is legible without reading it. Everything downstream of the switch follows the source: the lede rewrites itself (本地 promises the repositories already on this machine; GitLab promises 选一个克隆下来，就会加入本地列表并直接打开), the search field's placeholder changes from 搜索名称或路径 to 在 GitLab 上搜索项目 and debounces 350ms into a server-side query, the instance select only exists on the GitLab side, and the tabular count reads 个仓库 versus 个项目. GitLab rows reuse the project row's grid with two deliberate differences: the delta plate wears a dashed survey frame while the repository is not on this machine — the same dashed language the graph uses for “not yet here” — and it turns solid once a local clone is matched, and the right-hand circle becomes a labeled pill, 克隆并添加 or 打开, with a download glyph versus an out-arrow so the two verbs never rely on color. A 已在本机 teal tag, plus 复刻 and 已归档 in the quiet chip, sit beside the name. 克隆并添加 always asks the native directory picker, never a silent default; on confirm it clones, adds the repository to the local list, refreshes the match table so sibling rows flip to 打开 without a reload, and opens the graph. If the clone lands but the list write fails, the error names the path it cloned to instead of pretending nothing happened. Rows already local skip the picker entirely and open straight through. With no instance configured the page shows guidance and 前往设置, and 前往设置 is the only action — never an error where a next step will do. Below 1080px the search field drops to its own full-width line and the count floats right, keeping the pill, the instance select and the count on one row without clipping the 克隆并添加 label.

### Commit Row (signature)
A 42px row laid out as six columns: a 128px 分支 / 标签 stack, a dedicated lane canvas (92–140px wide), the flexible commit summary, a 100px author, a 72px tabular time, and a 62px mono SHA. The stack sits before the graph: local branches on its first line, remote branches and tags on its second, one chip per line with a dashed +N for the rest (current branch and its upstream lead); both local and remote refs stay visible at the front of the row, and HEAD folds into the current branch instead of repeating it. Each branch chip's glyph takes its row's line color; the checked-out branch swaps that glyph for a teal check. The canvas speaks GitKraken's language: circular author-initial nodes on colored swimlanes, a paper outer ring around merge commits, a hollow ring around the root, cubic merge curves, and a lane color that is re-assigned whenever a lane is freed and reused, so separate histories never read as one continuous river; a parent edge joins its target lane in the child line's color. Hover is a 4.5% teal wash; selection is 8.5% teal with a filled node, and the primary row carries a 2px teal bar at the sheet's left edge. The selected commit opens in the dossier, not in a modal. Above the table a pinned dashed-node `WIP` row aligns its node to lane zero and its label to the summary column, showing the working-file count, current branch, and +n −n; it is the only entry to uncommitted work. Ctrl/Shift click builds a multi-selection, and a selection bar offers 对比 (exactly two), 拣选, and 复制哈希. The filter line recomputes lanes over the matched rows only, remapping each hidden ancestor to its nearest visible parent so the river stays legible. Rows and ref chips are drag sources and drop targets in both the sheet and the rail: a grab cursor and a compact torn-off label follow the pointer, the source dims, non-targets recede, and the hovered lane brightens while the rest of the river fades; the sheet's floating pill names the exact drop action, and dragging near the top or bottom edge scrolls the list. A valid branch target takes a teal ring (remote targets accept merge requests only), a commit row takes a teal band. Dropping opens a pointer-anchored action menu headed source → target, with one-line hints: branch → local branch offers 新建合并请求 / 合并 / 变基 (rebase only when both ends are local), branch → remote offers only the merge request, branch → commit offers reset / rebase, and commit → local branch offers cherry-pick. Double-clicking a branch chip or rail row checks it out; a remote branch without a local counterpart creates a tracking branch first. Hovering any commit row focuses its lane, dimming the others so one river can be traced.

### Branch Rail (signature)
A 236px paper column of the repository's own refs: filter pill, hairline sections (远程仓库 / 本地 / 远程分支 / 标签 / 贮藏) with mono counts, 7px hollow lane dots that fill teal for the checked-out branch, mono 11.5px ref names, a teal “当前” badge, and ↑/↓ tabular sync marks. Each section is a disclosure: its chevron + label + count toggles the rows with a short fade-slide, the collapsed set persists in local storage, and filtering temporarily opens matching sections before restoring the stored layout. 远程仓库 is always present: each remote is a cloud-marked row with its redacted fetch URL on a 9.5px mono second line, a lock mark when credentials are hidden, and an always-available add control in the section head; clicking or right-clicking the row opens 抓取更新 / 清理失效分支引用 / 设置远程地址 / 重命名远程 / 复制 / 移除. The stash section lists `stash@{n}` entries with their message and ends with a quiet add control; selecting one opens stash detail. Row overflow triggers are labeled — 管理 for remotes, 更多 for branches, remote branches, tags, and stashes — quiet at rest and stronger on hover; stash and remote rows also open a menu. The footer is a 11px hint line above a hairline. Drag a branch onto a branch or commit to start merge / rebase / reset / cherry-pick, or onto another branch to open a prefilled merge request; right-click opens the canvas menu. A remote branch's menu holds only what a remote branch can do — 删除远端分支 replaces the local 重命名 / 删除 / 推送 trio rather than sitting greyed-out beside it — and a tag's menu carries 推送标签到远端 and 删除远端标签 next to the local delete, each confirmed in a dialog that names the remote and states that the local copy survives.

### Working Changes, Dossiers & Commit Composer (signature)
The WIP dossier is the uncommitted counterpart of commit detail, in the same paper-and-hairline language. Its head reads 变更 with a tabular count and offers labeled 刷新 / 贮藏 controls; the head actions never shrink, so both stay visible however long the filter text or file paths grow. A compact toolbar filters filenames, offers a labeled 清除 while filtering, and switches between a labeled 列表 and 目录 view. Uncommitted work is split into two persistent cards — 未暂存 (untracked folded in) and 已暂存 — with a coral 冲突 card only when conflicts exist. Each card has a hairline header with a tri-state checkbox, label, tabular count, +n −n, and a labeled bulk action (全部暂存 / 取消暂存 / 逐个合并); cards size to their content and scroll their own bodies, and an empty card keeps its frame with a short empty line so the two-part model is always visible. Tree folders are rows of their own with a chevron, a tri-state checkbox over every file below them, and rolled-up +n −n; checking or unchecking a folder drives every descendant while keeping the manual per-file selection intact when the view is switched. Each file row carries a checkbox, a signal-colored status glyph (teal staged, ochre unstaged, moss untracked, coral conflicted), a filename over a muted directory, +n −n counts that match `git diff --numstat` exactly, and always-visible labeled actions — 暂存 / 取消暂存 and 丢弃, with a conflicted row swapping the first for 合并 — so nothing needs a hover tooltip to be understood. Click, Ctrl-click, and Shift-click manage the checked set; any checked set raises a floating batch bar with 暂存 / 取消暂存 / 丢弃 / 清除. Right-click repeats those actions plus 复制路径.

The center sheet shows one working file's diff, headed by its status and a 返回图谱 backbar. Each hunk has a hairline head carrying its `@@` range, the function context, its own +n −n, and a labeled 暂存本块 / 取消本块 control. Files that cannot be split (untracked, renamed, deleted) render the control disabled with the reason in its tooltip, and the pane head offers the whole-file action instead. A conflicted file replaces the diff with the merge editor.

The merge editor is a three-way sheet headed by a coral seal, the file path, an operation-aware state tag (合并中 / 变基中 / 拣选中 / 还原中), and a 参考版本 disclosure: its 基准 / 当前 / 传入 tabs read the index stages, disable a side that has no file, scroll back to the top on every switch, and note when a long reference is truncated. Editable conflicts stack as context and cards: unchanged context is trimmed to ten head and six tail lines behind a quiet 展开 N 行未改动的上下文 control, and each 冲突 N card holds a teal 当前 plate and an ochre 传入 plate — labeled by operation, 变基目标 / 正在应用的提交 for rebase and their cherry-pick and revert counterparts — each with its own 采用当前 / 采用传入, above a 合并结果 textarea where an intentionally empty resolution means both sides are dropped. 两者都要 concatenates the two sides, 采用基准 returns to the common ancestor, 全部采用当前 / 全部采用传入 fill every card, and the footer's 已处理 n/N counter gates 保存并标记为已解决 until each conflict has been touched. Saving reassembles the untouched context verbatim, keeps CRLF when the worktree file used it, stages the file so it leaves the 冲突 card, and returns the pane to the normal diff. A conflict git cannot split — binary, too large, missing one side, truncated, or marker-free — falls back to whole-side cards with 采用当前版本 / 采用传入版本 behind a danger confirmation, 重新读取, and 标记为已解决 only when no markers remain.

The WIP dossier ends in the composer, split into a scrolling body and a pinned footer. The body holds 摘要 (72-character cap), 详细描述, the amend checkbox, a hairline line reading 已暂存 N 个文件 with tabular +n −n, and the 提交者 identity line; the footer always keeps the split commit control visible — one teal voice for 提交 with a chevron cell joined at the same radius offering 暂存全部并提交 and 提交并推送 — above the `Ctrl+Enter` hint. Both halves disable together while the summary is empty. 历史 opens the last twelve commit messages for one-click reuse into 摘要 and body; AI 写 commits streams through the same assistant path, and the button becomes 停止 while drafting. Credentials never leave the keychain.

The other dossiers share the same head-and-body frame: a stash shows 引用 / 时间 with 应用 / 弹出 / 删除; a branch shows upstream, ahead/behind, its tip commit, and clickable tip files; a commit shows hash, author, time, parents, message, and a clickable file list whose diff head offers 全部文件. With several commits selected the dossier becomes a selection list with 对比所选 and 拣选到 <branch>; compare opens a from→to sheet, and cherry-pick applies from oldest to newest, reporting conflicts through the usual banner.

### Canvas Menu & Action Dialog
Destructive or parameterized git operations run through two overlay primitives: a floating menu (Floating Menu shadow, 12px corners, 14px icons, coral for danger rows) and a centered action dialog on an ink veil at 34% (14–16px corners, parameter fields reuse the input system, radio choice cards highlight with a teal border, danger confirmations use Signal Coral). The operation result returns as a banner between toolbar and graph: moss for success, coral for errors, and an ochre conflict state that lists conflicting paths as mono chips and offers 继续 / 中止 inline.

### Operation Notice (signature)
A 12px-radius banner replacing alerts for git mutations. Success is a 9% moss wash, conflict is a 9% ochre wash with a file-chip row and a collapsed “查看 Git 输出” detail; continue/abort actions sit at the right edge. It never blocks the graph.

### Merge Request Report (signature)
A field-report layout: a 320–380px list with state dots, mono IIDs, 13px titles, and mono branch routes; a flexible detail pane with title, route, actions, description, and file/comment tabs. Destructive merge actions are confirmed in a dock; creation uses a right sheet. The list and report scroll independently, never the page.

### GitLab Browse (signature)
The instance side of the atlas, drawn in the same paper-and-hairline language as the merge-request report. Its left rail is deliberately not a browser: entering from the graph scopes it to the repository the origin remote resolves to, plus at most the upstream project it was forked from, so the pane is headed 仓库 with a right-aligned 11px state line that reads 当前仓库与它的上游, 只有当前仓库 while the fork's parent is unreachable, 正在定位当前仓库… while resolving, and 没有可浏览的仓库 when the instance has no match. There is no search field and no 加载更多 here — instance-wide browsing and 克隆并添加 belong to the 项目 index — and the empty rail offers 前往设置 or 去项目首页浏览 GitLab instead of an error. Rows hover at a 4% teal wash and select at 8.5% with a teal-deep name, the same values the commit rows use, so a chosen repository reads identically anywhere in the app; the current repository wears the teal 当前仓库 tag and its parent 上游源仓库, so the two rows are distinguishable without reading the paths. The detail head carries an 18px/600 name over a mono `group/repo` path, a 60ch description, and an ochre-marked 复刻自 line whose upstream path is a link. Right-aligned actions keep one teal voice — 克隆到本地 — beside the quiet ghosts 在 GitLab 打开 and, only for a fork, 同步复刻. Clone opens the native directory picker, clones over HTTPS with the PAT supplied from the keychain through libgit2's credential callback so the token never enters a URL or a command line, and lands the result in the project list and the graph; a clone that succeeds but cannot be added reports the failure and keeps the local path rather than pretending nothing happened. 同步复刻 expands an inline 12px-radius dock on a paper-fold wash — the upstream path in the sentence, a branch select, 确认同步, 取消 — and the dock never claims the fork is up to date: GitLab's sync is asynchronous, so the action returns 已提交同步请求 and closes, leaving the result in the transient success line between the workspace bar and the field. Triggering a build is a run bar, not a dialog: one 运行分支 select and one teal 运行流水线 with a play glyph on the same paper-fold wash above the pipeline list, defaulting to the branch the code tab is reading and following it across project switches, because GitLab takes a single `ref` and a parameter dock for one field would only add a click. The bar answers in its own line — 已在 main 上创建流水线 #14, or GitLab's own words with 创建流水线需要令牌具备 api 作用域 appended when a 403 says the token is scoped too narrowly — and the new row is inserted at the top of the list and opened for you rather than waiting on the next refresh. While GitLab is still creating the jobs, the job pane says 这条流水线还在创建作业。 and offers 刷新作业; the log's 刷新 stays disabled until a job is actually selected, so no control promises a refresh it cannot perform. Pipeline status is one 7px dot in the meaning-bound palette — moss for success, coral for failure, teal for running, muted for skipped or cancelled — and a running build keeps the dot pulsing at 1.6s so live work is legible without reading the label; the failed job in an opened pipeline is selected for you, and the trace reports its own 256 KB tail cap in the head metadata. Binary and oversized files are named as facts inside the file pane instead of failing silently. With no instance configured, the list shows guidance and a 前往设置 control rather than an error.

## Do's and Don'ts

### Do:
- **Do** keep Survey Paper as the page field and reserve Atlas Ink for the 64px app bar.
- **Do** build hierarchy with 1px hairlines and the paper → sheet → paper-fold tonal stack.
- **Do** keep functional type at 11px or larger and prose at 12px or larger; use tabular numerals for hashes, dates, counts, and diffs.
- **Do** give each region one teal primary action and leave destructive actions in coral.
- **Do** set Chinese in the UI stack (Public Sans → YaHei/PingFang) and reserve Bricolage for Latin display and numerals.
- **Do** animate the delta draw and sheet transitions at 0.16–0.3s and honor `prefers-reduced-motion`.
- **Do** let lists scroll inside their own region while the app bar and rails stay fixed.

### Don't:
- **Don't** introduce a navigation sidebar, gray admin panels, a KPI card grid, or dashboard chrome anywhere in the app. The commit view's branch rail is repository content, not navigation — never hang pages, app modes, or settings off it.
- **Don't** use coral, ochre, moss, plum, or slate as decoration; they are meaning-bound.
- **Don't** put shadows on resting content or round any working surface beyond 14px.
- **Don't** set Chinese in Bricolage or mono, or apply uppercase Latin tracking to CJK.
- **Don't** shrink functional labels below 11px or body copy below 12px to win density; cut content instead.
- **Don't** use pure white page backgrounds; Fresh Sheet and Button Ivory are the lightest acceptable surfaces.
- **Don't** add a second dark surface or a second accent voice to a screen that already has one.
