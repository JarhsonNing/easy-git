# Product

<!-- impeccable:product-schema 1 -->

## Platform

adaptive

Desktop native application (Tauri 2 shell, Windows-first), with a Vue 3 webview interface. Its interaction language should follow desktop-app expectations rather than a marketing website or an admin console.

## Users

Developers and technical team members who work in local Git repositories and collaborate through GitLab, including self-hosted GitLab. They open the app while actively coding, reviewing changes, preparing commits, or checking merge requests. Many work alone at a desktop, sometimes with a local AI model running on the same machine.

## Product Purpose

Easy Git is a local-first desktop Git client that makes everyday repository work feel direct and legible: see the commit graph, understand a commit, review and act on GitLab merge requests, and use an AI assistant grounded in the current repository. Success means a user can move from “which project am I working on?” to the change they care about without decoding the tool.

## Positioning

One focused desktop workspace that combines a visual commit graph, self-hosted GitLab merge request workflow, and local-model AI assistance, while keeping repository data on the machine and credentials in the OS keychain. It is deliberately smaller than a full Git platform client and more approachable than a raw Git UI.

## Operating Context

- The app opens to a project list; a repository becomes a working session only after the user enters it.
- Repository data is read locally. GitLab access uses a configured instance (cloud or self-hosted) and API token; AI uses OpenAI-compatible endpoints, including local Ollama/LM Studio/llama.cpp/vLLM services.
- Settings are application-level and remain available whether or not a project is open.
- Credentials are stored only in the OS keychain (service `com.easygit.app`, accounts `gitlab:{id}` / `ai:{id}`). Non-secret configuration lives at `%APPDATA%\easy-git\config.json`.
- Primary surfaces: project list, commit graph with commit detail and a commit workspace (working changes, hunk staging, stash, commit composer), merge request list/detail/diff/comments, AI conversation, settings.

## Capabilities and Constraints

- Visual commit graph with branch topology, refs, paging, per-commit file/stat detail, full or per-file commit diffs, and two-commit comparison.
- Commit workspace laid out graph-first: an always-visible refs rail with stashes and remote management (add, rename, set fetch/push URL, fetch, prune, remove), a pinned WIP row, commit filtering/search, multi-commit selection with compare and batch cherry-pick, and a right-hand dossier that switches between working changes, ref/stash/commit detail, and multi-selection. Working changes support both a flat path list and a recursive folder tree, with per-file and per-folder tri-state selection, per-file and per-hunk staging, whole-file and bulk stage/unstage, discard with confirmation, stash push/apply/pop/drop, amend, commit-and-push, AI-drafted commit messages, and one-click reuse of a historical message.
- GitLab merge request integration: instance selection, filters, search, detail, file diffs, comments, create, merge (with squash/remove-source options), close, reopen, and open in GitLab.
- AI assistant with streaming output, repository/worktree/selected-commit context, quick actions, and stop control.
- Self-hosted GitLab must be supported.
- Local AI models must be supported; profile configuration is required.
- No additional product scope beyond the four requested areas (graph, merge requests, AI assistance, log/detail viewing).
- Native folder picker for adding repositories.

## Brand Commitments

- Product name: Easy Git.
- The user explicitly wants a consumer-facing, interaction-friendly application, not an admin-backoffice/dashboard aesthetic.
- Tone: calm, direct, human, and competent; Chinese interface copy.
- Local-first and privacy-respecting; credentials belong to the system keychain.

## Evidence on Hand

- Working Tauri application in `D:\study\easy-git` with real backend commands for Git, GitLab, AI streaming, configuration, and keychain secrets.
- Existing views and interaction flows in `src/views/`; existing design tokens and components in `src/styles.css`.
- No marketing claims, customer logos, testimonials, pricing, or benchmark evidence exist, and none should be invented.

## Product Principles

1. The project is the home base: orient the user around the repository they are working in and make entering or leaving it obvious.
2. Show real work, not chrome: commits, diffs, merge state, and AI answers are the visual material; interface furniture stays quiet.
3. Local-first trust is visible in behavior and language, not a badge.
4. Powerful actions stay one step away but never ambush the user; confirm only what is destructive or irreversible.
5. Dense information is welcome when it reads effortlessly; density is not the same as an admin panel.
