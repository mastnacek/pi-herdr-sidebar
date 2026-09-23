# Pi Herdr Sidebar (Herdr Plugin)

Native Herdr telemetry and skills sidebar for the [Pi coding agent](https://github.com/earendil-works/pi-mono), built in **Rust with Ratatui** following Vertical Slice Architecture (VSA).

Provides high-performance, real-time agent telemetry, skill compliance gates, token economics, context window gauges, and Herdr workspace inspection in a dedicated terminal pane or popup.

**Self-reading telemetry**: the Status face parses the Pi agent's own session JSONL
(`~/.pi/agent/sessions/...`) directly — no snapshot from the TypeScript
`pi-sidebar` extension required. Model, thinking level, context window (resolved
from `models.json` + `models-store.json`), tokens, cache and cost all come from
the session log; git state is read from the session's cwd. The snapshot file is
only used as a fallback for the Skills face (pi-plugin-dev publishes skill state
over an in-process event bus, not to disk) and for legacy renderer mode.

---

## Features

- **Blazing Fast (<8ms Cold Start, <5MB RSS)**:
  Built in native Rust using Ratatui 0.29+ and Crossterm, completely replacing slow node-based renderers.
- **Three Interactive Faces**:
  - `Status`: Full agent telemetry with statusline parity:
    - Context window gauge: `📊 ██████░░░░░░░░ 42.5%/200k (auto)` with exact decimal precision and auto-compaction indicator
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
| `1` / `2` / `3` | Direct jump to face (1: Status, 2: Skills, 3: Herdr) |
| `Up` / `k` | Scroll up 1 line |
| `Down` / `j` | Scroll down 1 line |
| `PageUp` / `PageDown` | Scroll up / down 10 lines |
| `r` | Force refresh snapshot & Herdr state |
| `q` / `Esc` / `Ctrl+c` | Exit sidebar |

---

## Actions & Panes Declared in Herdr

- `pi.sidebar.toggle`: Toggle dedicated sidebar pane split to the right.
- `pi.sidebar.popup`: Open floating popup HUD over active workspace.
- `pi.sidebar.switch-tab`: Switch between Status and Skills faces remotely.
- `pi.sidebar.sidebar`: Split pane entrypoint.
- `pi.sidebar.sidebar-popup`: Popup modal entrypoint.

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
    │   └── terminal.rs         # Crossterm terminal setup/cleanup guard
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
