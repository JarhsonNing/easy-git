---
version: 1
slug: "src-app-vue"
primary_target: "src/App.vue"
related_targets: ["src/views/ProjectsView.vue","src/views/GraphView.vue","src/views/MrView.vue","src/views/AiView.vue","src/views/SettingsView.vue","src/styles.css"]
---

# Easy Git application shell — surface brief

Scope: the entire desktop application UI — project index, commit graph, merge requests, AI assistant, and application settings. Visitor mode: Operate.

Audience and job: developers working in local Git repositories, including self-hosted GitLab users and people running local AI models. They arrive to resume a repository, scan history, review a merge request, or ask for help with the current changes. Success is reaching the right repository and the right piece of work without reading a control panel.

Constraints: preserve all existing behavior and data; local-first; credentials live only in the OS keychain; Chinese interface copy; desktop window from 1000×640 to 1280×820 and wider.

Memorable moment: a project's generated river-delta plate draws itself while the repository opens, then the same branching language becomes the commit graph.

Unresolved: none blocking; dark mode is not part of this pass.

## Direction contract

THESIS: A repository is a watershed and its history is a river network — branches split and merge like tributaries. This replaces the incumbent console sidebar, gray panels, and dashboard chrome with an atlas you can work in, without disguising standard desktop controls.

OWN-WORLD: Warm survey paper #F5EFE3 and ink #20241F; a deep-teal action/shell #1F6E6A; coral #C9563C for attention and destructive acts; ochre #B47A2C for warnings and tags; moss #587A4B for success; plum #765C88 for AI. Bricolage Grotesque for Latin display and numerals, Microsoft YaHei for Chinese UI, Cascadia/Consolas for code and hashes only. Hairline survey rules, tabular numbers, generous measure, and generated delta line art in six channel colors.

STORY: The user sees an atlas index of their repositories, opens one, and immediately knows which branch they are on and what changed. History reads as flow, merge requests as field reports, and AI as a quiet assistant at the same desk. Every destination is one clear step from the app bar.

FIRST VIEWPORT: a 64px ink app bar with the delta mark, wordmark, back/context, centered project nav, and settings; below it, warm paper. The project home is a single 1180px column: “项目” at 46px, a plain-language lede, add-project action, search line, then full-width repository rows with a generated delta plate, name, path, branch state, and a circular open control. Inside a project, a slim contextual bar sits above a river-line commit graph with a 340px field-report inspector. No fixed sidebar.

FORM: candidate 6 of 7 grounded directions, the river atlas; seed:4bd39ec4897b86ca.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
