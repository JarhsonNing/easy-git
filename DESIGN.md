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
- One 1180px column for index surfaces; work area plus a fixed inspector rail inside a project, with one switch choosing whether that project reads as history or as working changes.
- Chinese-first copy with a Chinese-aware font stack, never Latin tracking on CJK.

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
- **Display** (550, 42px, 1.2, −0.03em): page titles such as 项目 and 提交历史; 38px below 1080px. One per view.
- **Headline** (550, 34px, 1.2, −0.03em): empty-state and AI greeting headings; 30px below 1180px, 25px at 700px height or shorter.
- **Title** (550–600, 19–23px, 1.2–1.3): project names at 19px, sheet titles and merge-request detail titles at 23px.
- **Body** (400, 12.5–14px, 1.5–1.8): the working interface; long explanatory copy sits at 12.5px/1.75 and is capped near 34–56ch. Base UI is 14px, but most operational text is 12.5–13px because desktop scanning tolerates compactness.
- **Label** (700, 11px, 1.4, 0.09em): section labels and table headers, uppercase only when the text is Latin. Chinese labels keep normal case.
- **Code** (400, 11px mono, tabular): hashes, paths, branch routes, file paths, model addresses, and code. Never prose.

### Named Rules
**The 11px Floor Rule.** No functional label, table header, badge, or metadata drops below 11px; prose stays at 12px or larger. Reduce content before shrinking type.

**The Chinese Pair Rule.** Bricolage Grotesque carries Latin display, project names, and numerals only. Chinese always falls through to Microsoft YaHei or PingFang in the same stack and never receives Latin uppercase tracking.

## Layout

The app is a single vertical shell: a 64px ink app bar over a scrolling main region. The bar is a three-column grid (1fr / auto / 1fr) with 22px insets: brand, back control, project name, and branch pill on the left; a centered pill navigation for 提交图谱 / 合并请求 / AI 助手; settings on the right. There is no sidebar and no persistent toolrail outside the app bar.

Index surfaces — project list and settings — use one column of `min(1180px, 100%)` with 42px / 36px / 56px padding, a 42px page title, a short lede capped at 54ch, and a page rule beneath. Project rows are full-width, 88px minimum, with an 84×52 delta plate, a flexible name/path column, and a circular open control. The search line is a 320px pill on the left with a tabular repository count on the right.

Project workspaces skip the page heading and go straight to work. The commit view is a three-column atlas: a 236px domain rail, the 1fr work sheet, and a 348px inspector, 16px apart; table headers are 34px, commit rows 42px. One segmented switch in the toolbar (图谱 / 变更, with a tabular change count) drives all three columns at once: in 图谱 the rail lists branches, the sheet lists commits, and the inspector holds the branch or commit dossier; in 变更 the rail lists working files, the sheet becomes the file's hunk-level diff, and the inspector becomes the commit composer. The same switch is the only place the mode is set — no second toggle lives in the rail or the inspector. The merge-request view is a 320–380px list plus a flexible report pane; the list narrows to 350px at 1180px and 320px at 1080px. AI is a centered workspace up to 1120px with an 820px conversation column. Settings is a 200px sticky index plus a form column on paper.

Below 1180px the domain rail becomes a drawer: it slides over the work area from the left at 264px with a veil, opened by a ghost button in the toolbar whose label matches the current mode (“分支” or “变更”) and closed by its own circular control or the veil. It never covers the app bar.

Breakpoints are 1180px, 1080px, and 980px; a max-height pass at 700px compresses the AI empty state. The supported window runs from 1000×640 to 1280×820 and wider; at the minimum size, author and date columns may drop before the work area is allowed to feel cramped.

The rhythm is 4/6/8/12/16/24/36/56. Panel insets are 22–24px, row gaps 10–15px, and section separation 24–36px. Density is welcome; crowding is not.

### Named Rules
**The One Column Rule.** Index and settings surfaces use a single 1180px column. Project workspaces split into work area plus at most two fixed rails (a domain rail and one inspector). No app-navigation sidebar and no widget grid; the domain rail carries repository content only — refs in 图谱, working files in 变更 — never pages or app modes.

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
- **No sidebars for navigation.** The project context always shows where you are and offers one circular return control. The one exception is the commit view's domain rail: it is a map legend for the repository, not a navigation surface — it lists refs in 图谱 and working files in 变更, never pages or app modes. The 图谱 / 变更 switch that repurposes it sits in the toolbar, exactly once.

### Delta Plate (signature)
A generated river delta drawn from the repository path in six channel colors (teal, coral, ochre, moss, plum, slate) over dotted survey contours. It animates once when a project opens: contours fade in, channels draw in 0.95s with staggered delays, joints and endpoints settle, and a slow current pulses along one channel. The same branching language becomes the commit graph, so the identity is functional, not ornamental.

### Project Row (signature)
A full-width row on paper, separated by hairlines: 84×52 delta plate, 19px display name with a teal “正在查看” tag, mono path at 11px, branch metadata at 11px, and a 34px circle that fills teal and lifts one pixel on hover. Hover warms the row rather than raising it.

### Commit Row (signature)
A 42px row with a river-line graph gutter, 12.5px summary, ref chips, and tabular date at the right. Hover is a 4.5% teal wash; selection is 8.5% teal with a filled node. The selected commit opens in the inspector, not in a modal. Rows and ref chips are drag sources and drop targets: dragging a ref or row shows a floating pill hint at the sheet's bottom edge, and a valid target warms with an ochre dashed outline while hovered.

### Branch Rail (signature)
A 236px paper column of the repository's own refs: filter pill, three hairline sections (本地 / 远程 / 标签) with mono counts, 7px hollow lane dots that fill teal for the checked-out branch, mono 11.5px ref names, a teal “当前” badge, and ↑/↓ tabular sync marks. The overflow trigger (24px, more icon) appears only on row hover. The footer is a 11px hint line above a hairline. Drag a branch onto a branch or commit to start merge / rebase / reset / cherry-pick; right-click opens the canvas menu.

### Working Changes & Commit Composer (signature)
The 变更 half of the same three-column atlas, in the same paper-and-hairline language. The rail groups changes as 冲突 / 已暂存 / 未暂存 / 未跟踪 / 贮藏 with hairline headers and tabular counts; each row is a status glyph in its signal color (teal for staged, ochre for unstaged, moss for untracked, coral for conflicted), a mono path split into a muted directory and an ink filename, and +n −n counts that match `git diff --numstat` exactly. Row actions are one-icon controls at the right edge: ⊕/⊖ stages or unstage the whole file, the trash icon opens a discard confirmation. Group headers carry the bulk action (暂存全部 / 取消暂存全部); 贮藏 collapses to a count line and each entry shows a plum `stash@{n}` badge.

The center sheet shows one file's diff split into hunks. Each hunk has a hairline head carrying its `@@` range, the function context, its own +n −n, and a stage dot — ⊕ to stage that hunk, ⊖ to unstage it. Files that cannot be split (untracked, renamed, deleted, conflicted) render the dot disabled with the reason in its tooltip, and the pane head offers the whole-file action instead. Conflict files get a card that explains the state and offers 标记为已解决 rather than a diff.

The inspector's third dossier is the composer: 摘要 (72-character cap) and 详细描述 on the shared input system, an amend checkbox, a hairline line reading 已暂存 N 个文件 with tabular +n −n, and the split commit control — one teal voice for 提交 with a chevron cell joined at the same radius offering 暂存全部并提交 and 提交并推送. Both halves disable together while the summary is empty. The 提交者 line shows `git var GIT_AUTHOR_IDENT` without its epoch, and an ochre warning replaces it when the repository has no identity. 写提交信息 sits in the dossier head and streams through the same AI path as the assistant; credentials never leave the keychain. `Ctrl+Enter` commits from anywhere in the view.

### Canvas Menu & Action Dialog
Destructive or parameterized git operations run through two overlay primitives: a floating menu (Floating Menu shadow, 12px corners, 14px icons, coral for danger rows) and a centered action dialog on an ink veil at 34% (14–16px corners, parameter fields reuse the input system, radio choice cards highlight with a teal border, danger confirmations use Signal Coral). The operation result returns as a banner between toolbar and graph: moss for success, coral for errors, and an ochre conflict state that lists conflicting paths as mono chips and offers 继续 / 中止 inline.

### Operation Notice (signature)
A 12px-radius banner replacing alerts for git mutations. Success is a 9% moss wash, conflict is a 9% ochre wash with a file-chip row and a collapsed “查看 Git 输出” detail; continue/abort actions sit at the right edge. It never blocks the graph.

### Merge Request Report (signature)
A field-report layout: a 320–380px list with state dots, mono IIDs, 13px titles, and mono branch routes; a flexible detail pane with title, route, actions, description, and file/comment tabs. Destructive merge actions are confirmed in a dock; creation uses a right sheet. The list and report scroll independently, never the page.

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
