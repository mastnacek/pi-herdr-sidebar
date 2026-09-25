# Pi Herdr Sidebar (Herdr Plugin)

Native Herdr telemetry and skills sidebar for the [Pi coding agent](https://github.com/earendil-works/pi-mono), built in **Rust with Ratatui** following Vertical Slice Architecture (VSA).

Provides high-performance, real-time agent telemetry, skill compliance gates, token economics, context window gauges, and Herdr workspace inspection in a dedicated terminal pane or popup.

**Self-reading telemetry**: the Status face parses the Pi agent's own session JSONL
(`~/.pi/agent/sessions/...`) directly — no snapshot from the TypeScript
`pi-sidebar` extension required. Model, thinking level, context window (resolved
from `models.json` + `models-store.json`, plus the runtime caches extensions
inject, e.g. `zen-free-models.cache.json`), tokens, cache and cost all come from
the session log; git state is read from the session's cwd. The snapshot file is
only used as a fallback for the Skills face (pi-plugin-dev publishes skill state
over an in-process event bus, not to disk) and for legacy renderer mode.

---

## Features

- **Blazing Fast (<8ms Cold Start, <5MB RSS)**:
  Built in native Rust using Ratatui 0.29+ and Crossterm, completely replacing slow node-based renderers.
- **Three Interactive Faces**:
  - `Status`: Full agent telemetry with statusline parity:
    - Context window gauge: `📊 ██████░░░░░░░░ 42.5% (70.1k/1M)`, or `?` when
      pi itself reports the context as unknown (post-compaction gap); no
      extension required, and no dependence on context-mode
    - Token economics: `⬆️ In / ⬇️ Out` compact metrics
    - Cache performance: `📦 Read / Write` totals and `🎯 Hit rate %`
    - Cost tracking: Exact session dollar expenditure
    - Model & thinking level: `(provider) model • 🧠 <level>`
    - Git badge: `🌿 branch ●dirty/○clean ▸N ahead ◂N behind`
  - `Skills`: pi-plugin-dev skill HUD (structured `<pane>.skills.json` sidecar written by the TS pi-sidebar extension):
    - Active skill target: `🎯 <skill-name>`
    - Counters: `<N> refs · elapsed · <N> turns`
    - Guidance: `✓ <doc> — <summary>` checkmarks
    - Focus: recent actions (🧪 bash, ✏️ edit, ✍️ write, 📚 doc_consult)
    - Compliance gates: `🛡 Gates 3/4` scorecard with `PASS` / `WARN` / `FAIL` badges
    - Status footer: `✓ settled · N read · N written` / `● running`
  - `Herdr Panes`:
    - Live workspace pane table: Pane ID, Tab, Workspace, Label, Focus indicator, and working directory.
  - `Notes` (SPAI):
    - Cross-project `docs/spai` browser with a ZEN detail viewer.
    - `e` opens the integrated editor (title + body, `Tab` switches field, `Ctrl+S` saves).
    - `E` opens the selected note in an external editor (`$VISUAL`, then `$EDITOR`, then a platform default), reloading notes on exit.
- **Clickable & Keyboard-Driven**:
  - Click any tab on the top bar or press `Tab`, `1`, `2`, `3` to switch faces.
  - Automatically synchronizes tab changes with the Pi agent session via `<snapshot>.request.json`.
  - Scroll with mouse wheel or `j`/`k`/arrow keys.
- **Flexible Placements**:
  - Dedicated right-side split pane (`placement = "split"`).
  - Floating modal popup (`placement = "popup"`).
  - Standalone replacement for `renderer.mjs` via `--snapshot <path>`.

---

## Keyboard Shortcuts

| Shortcut | Action |
| --- | --- |
| `Tab` / `Right` / `l` | Next face (`Status` → `Skills` → `Herdr`) |
| `Shift+Tab` / `Left` / `h` | Previous face |
| `1` … `5` | Direct jump to face (0: Zen, 1: Status, 2: Skills, 3: MCP, 4: Notes, 5: Shortcuts) |
| `Up` / `k` | Scroll up 1 line |
| `Down` / `j` | Scroll down 1 line |
| `PageUp` / `PageDown` | Scroll up / down 10 lines |
| `r` | Force refresh snapshot & Herdr state |
| `q` / `Esc` / `Ctrl+c` | Exit sidebar |

### SPAI Notes tab (`4`)

| Shortcut | Action |
| --- | --- |
| `←` / `→` (`, .` `[` `]`) | Switch project |
| `p` | Jump to the active (session cwd) project |
| `↑` / `↓` (`k` / `j`) | Select note |
| `PgUp` / `PgDn` (`u` / `d`) | Scroll viewer |
| `e` | Integrated edit dialog for the selected note |
| `E` | Open the selected note in an external editor |
| `n` | New note (SPAI smart input with `@project` autocomplete) |
| `x` | Cycle note status |

Inside the integrated editor (`e`) every printable key inserts its character —
the external editor is on `Ctrl+E`, never a bare `E`.

### Shortcuts tab (`5`)

Lists the keybindings your Herdr actually has: the `[[keys.command]]` entries
from `config.toml` (badged `config`) merged with the documented Herdr defaults,
with the resolved prefix (`ctrl+b` unless overridden).

Actions this plugin offers but you have not bound yet are listed as `návrh`
(suggested) rows with a free chord — that is how the usage overview below is
discoverable without hand-editing `config.toml` first. A suggestion never
proposes a chord Herdr already uses by default.

| Shortcut | Action |
| --- | --- |
| `↑` / `↓` (`k` / `j`) | Select binding |
| `PgUp` / `PgDn` | Scroll the list |
| `r` | Re-read `config.toml` |

The overview is **not** part of this tab: it is a separate window with its own
keybinding (section below).

### Plugin usage overview (standalone window)

Its own `placement = "popup"` pane — the same mechanic the Kanban board uses —
rendered by `pi_sidebar usage`. It shows *only* the overview: no sidebar header,
tab bar or shared banner, so it reads as a dedicated modal window and closes with
`Esc`.

Bind it in `~/.config/herdr/config.toml` (Windows: `%APPDATA%\herdr\config.toml`):

```toml
[[keys.command]]
key = "prefix+u"
type = "plugin_action"
command = "pi.herdr-sidebar.usage-win"
description = "Pi Herdr Plugin Usage"
```

Shows what your plugins, skills and slash-commands are actually used for, counted
from pi's own session logs (`~/.pi/agent/sessions/**/*.jsonl`):

- **every** plugin, with proportional bars and its three most used tools inline,
- **every** skill ranked by `SKILL.md` reads,
- **every** slash-command, and **every** tool with its owning plugin,
- totals in the footer: files, messages, tool calls, plugins, skills, scan time.

The document is scrollable and lists everything — there is no "… and N more"
row. Only names longer than the name column are clipped.

| Shortcut | Action |
| --- | --- |
| `↑` / `↓` (`k` / `j`) | Scroll one row |
| `PgUp` / `PgDn` (`Space`) | Scroll one screen |
| `Home` / `End` (`g` / `G`) | Jump to the top / bottom |
| `r` | Re-scan, ignoring the cache |
| `Esc` / `q` | Close the window |

The scan walks ~300 MB, so it always runs on a background thread with a progress
bar, and the aggregate is cached against a fingerprint of the log set (file
count, total bytes, newest mtime). A cached open is effectively instant.
Tool → plugin attribution is a tested prefix table, including the
`mcp__<server>` namespace proxies and the `knowledge_base_` /
`knowledge-base_` name variants.

---

## Actions & Panes Declared in Herdr

- `pi.sidebar.toggle`: Toggle dedicated sidebar pane split to the right.
- `pi.sidebar.popup`: Open floating popup HUD over active workspace.
- `pi.sidebar.switch-tab`: Switch between Status and Skills faces remotely.
- `pi.sidebar.sidebar`: Split pane entrypoint.
- `pi.sidebar.sidebar-popup`: Popup modal entrypoint.
- `pi.sidebar.edit-notes` / `edit-notes-win`: SPAI note editor as a large modal.
- `pi.sidebar.usage` / `usage-win`: plugin/skill usage overview as a standalone
  window (`pi_sidebar usage`), no sidebar chrome.

Bind them in `~/.config/herdr/config.toml` (Windows:
`%APPDATA%\herdr\config.toml`) — Herdr owns keybindings, the manifest only
declares actions:

```toml
[[keys.command]]
key = "prefix+n"
type = "plugin_action"
command = "pi.herdr-sidebar.edit-notes-win"
description = "Pi Herdr Notes Editor (modal)"

[[keys.command]]
key = "prefix+u"
type = "plugin_action"
command = "pi.herdr-sidebar.usage-win"
description = "Pi Herdr Plugin Usage (window)"
```

---

## Architecture (VSA)

This plugin strictly adheres to Vertical Slice Architecture:

```text
pi-sidebar/
├── herdr-plugin.toml           # Manifest declaring cross-platform actions and panes
├── Cargo.toml                  # Ratatui, Crossterm, Serde, ansi-to-tui
└── src/
    ├── main.rs                 # Composition root & CLI dispatch
    ├── shared/                 # Kernel / Shared domain (Zero slice knowledge)
    │   ├── client.rs           # Herdr CLI subprocess client wrapper
    │   ├── context.rs          # Environment & snapshot path discovery
    │   ├── snapshot.rs         # JSON snapshot transport & PID lock
    │   ├── terminal.rs         # Crossterm terminal setup/cleanup guard
    │   └── theme.rs            # Modal backdrop / focus styling + bars
    └── slices/                 # Isolated feature slices
        ├── telemetry/           # Live telemetry: Pi session JSONL parser
        │   │                    #   (model, usage, cost, context, git) — no TS ext
        ├── view/                # Interactive Ratatui sidebar TUI
        │   ├── state.rs        # SidebarState, active tab, snapshot polling
        │   └── ui.rs           # Ratatui rendering (Tabs, Paragraph, Table)
        └── actions/            # Herdr action invocations
            ├── toggle.rs       # Split pane right / close toggle
            ├── popup.rs        # Popup launcher
            └── switch_tab.rs   # Remote face switcher
```

---

## Local Development & Linking

1. Build release binary:
   ```bash
   cargo build --release
   ```

   > **Close running sidebar panes first.** On Windows the running pane holds a
   > lock on `target/release/pi_sidebar.exe`, so the build dies with
   > `failed to remove ...\pi_sidebar.exe: Přístup byl odepřen. (os error 5)` and
   > the pane silently keeps executing the **old** binary — new shortcuts appear
   > to do nothing. Close the panes, rebuild, then reopen:
   >
   > ```bash
   > herdr pane close <sidebar-pane-id>          # list with: herdr pane list
   > cargo build --release
   > herdr plugin action invoke pi.herdr-sidebar.toggle-win
   > ```
   >
   > Panes also respawn by themselves through the plugin's `ensure` events on
   > pane/tab/workspace focus.

2. Link into Herdr:
   ```bash
   herdr plugin link .
   ```

3. Verify actions and open pane:
   ```bash
   herdr plugin action list --plugin pi.sidebar
   herdr plugin action invoke pi.sidebar.toggle-win   # on Windows
   # or
   herdr plugin action invoke pi.sidebar.toggle       # on Linux / macOS
   ```

---

## License

MIT © mastnacek
