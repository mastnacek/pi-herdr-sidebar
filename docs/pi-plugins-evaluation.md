# Evaluation of Pi Plugins for `pi-herdr-sidebar`

This document synthesizes the exploration of all plugins under `D:\01_programovani\pi\plugins\` and evaluates which data, files, and features they fetch, create, or display that are valuable for the native Rust + Ratatui Herdr sidebar (`D:\01_programovani\herdr\plugins\pi-herdr-sidebar`).

---

## 1. High-Priority Telemetry & Data Sources (Zero or Minimal Overhead)

| Plugin | Data Fetched / Created / Cached | File Path / Mechanism | Value & Integration for `pi-herdr-sidebar` |
|---|---|---|---|
| **`pi-batch-openrouter`** | Background OpenRouter Batch API jobs (`id`, `title`, `model`, `status`, `progress`, `cost`, `submitted_at`, `completed_at`). | `~/.pi/agent/pi-batch-openrouter/jobs.json` | **Very High**: Display running/pending batch jobs count, animated spinner, and completion status directly in the Zen or Status face without opening separate TUI modals. |
| **`pi-solodev-adr`** | Architectural Decision Records (MADR format). Ledger of design constraints and architectural choices. | `docs/adr/.index.json` | **High**: Reuses the exact index structure of SPAI (`records`, `version`, `lastUpdated`). Sidebar already parses `docs/spai/.index.json` via `spai_live.rs`. Adding `adr_live.rs` enables displaying active architectural constraints and decision counts alongside tasks. |
| **`pi-model-pricing`** | Live model pricing rates (input, output, cache read/write per 1M tokens) and 30-day OpenRouter token popularity rankings. | `~/.pi/agent/cache/openrouter-pricing-cache.json` (458 models)<br>`~/.pi/agent/cache/openrouter-popularity-cache.json` | **High**: Sidebar currently shows token counts and dollar cost, but does not show active model pricing tiers or token popularity rank (e.g. `#3 on OpenRouter`). Can be read from local disk cache with zero network calls. |
| **`pi-openrouter-accounts`** | Multi-account OpenRouter aliases (`openrouter-work`, `openrouter-personal`, etc.) and data policies (ZDR, data collection deny/allow). | `~/.pi/agent/openrouter-accounts.json` | **High**: Display active account label (e.g. `[Work: ZDR]`) next to model info in Zen and Status faces. |
| **`pi-decision-gate`** | Daily USD/CZK exchange rate (ČNB API + ECB fallback), OpenRouter credit balance, and Jev System One action risk assessment. | ČNB API (`/cnbapi/exrates/daily`)<br>`~/.pi/agent/auth.json` (`/api/v1/credits`) | **Medium-High**: Dual currency cost display (`$0.012 / 0.28 Kč`) using ČNB daily rate; OpenRouter credit balance meter in Quota telemetry. |
| **`pi-architecture-watcher`** | Real-time static Vertical Slice Architecture (VSA) compliance, drift score (0–100), and cross-slice violation detection. | In-memory AST scanner & gate reports | **Medium-High**: Display live VSA compliance badge (`VSA: 100% Clean` or `VSA: 2 Violations`) in Skills face to guard architectural invariants during editing. |
| **`pi-projects`** | Workspace project discovery (170+ projects), technology detection (Rust, TS, Python, Go), and Git status summaries (`dirty`, `clean`, `ahead`, `behind`). | `~/.pi/agent/pi-projects-cache.json` | **Medium**: Cross-reference Herdr pane cwd with the projects cache to display project tags, technology badges, and Git status across all workspace panes in the Herdr tab. |

---

## 2. Complementary & Mode Plugins

- **`pi-klid`**: Quiet zen mode that hides thinking blocks and tool spam behind a static cover or SPAI kanban.
  - *Sidebar Synergy*: `pi-herdr-sidebar` already implements `Tab::Zen` as a dedicated low-dopamine face. The sidebar acts as the external persistent HUD while `pi-klid` keeps the main terminal clean.
- **`pi-mcp-viz`**: Appends transcript cards and self-dismissing modals for MCP tools.
  - *Sidebar Synergy*: `pi-herdr-sidebar` already has native `Tab::Mcp` parsing MCP tool executions directly from the Pi session JSONL (`mcp_live.rs`). Sidebar provides persistent metrics (call counts, tokens, error badges) without cluttering the chat history.
- **`pi-tts` & `pi-tui-sound`**: Text-to-speech audio output and sound effects on turn completion.
  - *Sidebar Synergy*: Sidebar can show an audio badge in the header/footer (e.g. `🔊 TTS: cs-CZ-AntoninNeural [Vader3]`) by checking `~/.pi/agent/pi-tts.json`.
- **`pi-prompt-translate-czk`**: Translates prompts into English and responses back to Czech with CZK tracking.
  - *Sidebar Synergy*: Sidebar can display translation status flag (`🌐 CZ -> EN`) and reuse its ČNB exchange rate cache.
- **`pi-zen-fallback`**: Automatically cascades between free OpenCode Zen models upon HTTP 429 rate limits.
  - *Sidebar Synergy*: Can display model cooldown timers and fallback state (`zen: fallback active`).

---

## 3. Specialized / Domain Plugins (Indirect Value)

- **`pi-herdr-plugin-dev`**: Developer tooling for authoring Herdr plugins (scaffolding, manifest validation, 400-line limit guard). Development guardian for `pi-herdr-sidebar` itself.
- **`pi-apple-rada`**: Multi-persona deliberation harness (Jobs, Woz, Ive, Karpathy). Generates markdown consensus reports.
- **`pi-lotusscript-modular`**: Notes/Domino 9.0.1 virtual modularizer and LSP integration.
- **`pi-anonymizer`**: Secret and credential scrubber before model submission.
- **`pi-at-words` & **`pi-read-all`**: Input editor autocomplete and bulk file injection ergonomics.

---

## 4. Recommended Roadmap for `pi-herdr-sidebar`

1. **Batch Jobs Monitor (`src/slices/telemetry/batch_live.rs`)**:
   - Watch `~/.pi/agent/pi-batch-openrouter/jobs.json`.
   - Add a compact batch indicator into `zen.rs` and `status.rs` (`⚡ Batch: 2 running [1/5 completed]`).
2. **ADR Ledger Integration (`src/slices/telemetry/adr_live.rs`)**:
   - Watch `docs/adr/.index.json`.
   - Render ADR count and latest architectural constraint alongside SPAI counts.
3. **Model Intel & Popularity (`src/slices/telemetry/pricing_live.rs`)**:
   - Parse `~/.pi/agent/cache/openrouter-pricing-cache.json` and `openrouter-popularity-cache.json`.
   - Render model cost rates (`In: $0.15 / Out: $0.60 per 1M`) and popularity rank (`Popularity: #3 (48.8T tokens)`).
4. **Dual Currency USD + CZK**:
   - Integrate ČNB exchange rate for dual cost reporting in session telemetry (`$0.012 (0.28 Kč)`).
