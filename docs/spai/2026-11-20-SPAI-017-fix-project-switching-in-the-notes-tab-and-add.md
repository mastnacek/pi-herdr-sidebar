---
type: Todo
title: "Fix project switching in the Notes tab and add type-to-search for projects"
timestamp: 2026-11-20 09:45:01
status: done
source: orchestrator
tags: [agent, coder, sidebar]
facets:
  project: pi_sidebar
  project_path: D:/01_programovani/herdr/plugins/pi-herdr-sidebar
spai_symbol: 'x'
lang: en
title_cs: "Opravit přepínání projektů na kartě Poznámky a přidat vyhledávání psaním pro projekty"
---

# SPAI-017: Fix project switching in the Notes tab and add type-to-search for projects

x Fix project switching in the Notes tab and add type-to-search for projects @pi_sidebar :agent:coder:sidebar:

```
WORKDIR         D:/01_programovani/sidebar-nav   (pi-herdr-sidebar, branch fix/project-nav)
SCRATCH         C:/Users/jaroslav/AppData/Local/Temp/claude/ea3dc87168f08f14/ffe01966-1e2f-4b82-b29b-28c254f3953e/scratchpad/spai
ALLOWED_PATHS   src/**
GATE            cargo test; cargo build --release; no new clippy warnings (today 34 lines
                `^warning|^error` from `cargo clippy --all-targets`); grep "slices::" in src/shared
                empty; every .rs file at most 400 lines (src/slices/view/state.rs has 392 - split
                before adding)
HARD_CONSTRAINTS Do not write to real backlogs (D:/01_programovani/*/docs/spai) or the real
                 index.db; tests use the fake Runner and temp folders. Read the herdr-plugin-dev
                 skill and AGENTS.md first.
REMOTE_ALLOW    none
```

## 1. Bug: Right arrow does not move, Left jumps by two (Notes tab)

Cause (orchestrator's analysis, verify it): `SpaiNotesState::refresh`
(src/slices/spai_notes/state.rs:105) compares `fingerprint()`, which includes the mtime of the
SELECTED project's docs/spai and .uma. Switching project changes the fingerprint, so the next
refresh rebuilds `projects` from the fallback `discover_spai_projects` (pi cache, other order)
while keeping `selected_project_idx`; then `spai projects` answers and `apply_project_list`
re-selects by the path the stale index pointed to. Since SPAI-008 the two lists differ, so the
selection jumps.

Fix:
- Two fingerprints: the pi cache fingerprint (and cwd change, force) refreshes the project list;
  the selected project's notes fingerprint only reloads that project's items.
- Every rebuild keeps the selected project by path, not by index.
- Once `spai projects` has answered, never fall back to the discovery list; refresh the list by
  asking spai again.
- Tests: switch project then refresh keeps the selection; Right/Left move exactly one project in
  both lists; notes change of the selected project reloads items without touching the list.

## 2. Type-to-search for a project (Notes tab)

- `/` in the Notes tab opens a project filter line under the project box (`🔍 spa▏`). While it
  is open every printable key goes to the filter (no global shortcuts), Backspace edits.
- Case-insensitive substring match on project name and path, diacritics ignored (reuse
  `normalize_czech` from scratch/filter.rs if it fits); `spa` matches spai, pi-spai, herdr-spai,
  spai-kanban.
- Left/Right and Up/Down move within the matches; the box title shows `Projekt (2/4 · filtr)`.
  Enter selects the project and closes the filter; Esc closes it and returns to the project
  selected before. No match: show `žádný projekt` and keep the previous selection.
- Footer help line mentions `[/] hledat projekt`.
- Tests: matching (case, diacritics, path), navigation among matches, Enter, Esc, empty result,
  keys do not leak to global shortcuts while filtering.

## Done when

GATE green after the last change; report with evidence and a commit proposal:

    fix(spai_notes): project switching keeps its place; type-to-search projects
