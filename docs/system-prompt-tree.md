# System-prompt tree (Status face)

How the sidebar shows what the model *actually* received at the top of the
request: the base instructions, the `AGENTS.md` chain, and whether an
`APPEND_SYSTEM.md` addendum made it in.

The panel has two providers and names the one in use:

* **replay** — parses the section set out of the Pi session transcript. Always
  available, no extension needed. It names context files and `AGENTS.md`
  chains exactly, attributes a CLI append by content (`agents/*.md` plus the
  two `APPEND_SYSTEM.md` locations), previews inline text and reports drift;
  what it cannot see is a *forced* prompt (`forceSystemPrompt`), and it only
  refreshes when the transcript grows;
* **exact** — reads the `.prompt.json` sidecar the TypeScript `pi-sidebar`
  extension captures on `before_agent_start`, i.e. the prompt the engine has
  resolved for the request it is about to send. Real paths, forced prompts and
  CLI appends included.

Either way no file is re-discovered to decide what was loaded — discovery is
only used to *name* the source of a section. That keeps the panel honest when a
project is untrusted, a worktree shadows a context file, or `SYSTEM.md` replaced
the default prompt.

---

## 1. Where Pi persists the prompt

`pi-coding-agent` stores the system prompt in the session JSONL itself
(`docs/session-format.md`, *SessionMessageEntry*):

- the first request persists a `role: "system"` message with the **complete**
  prompt in `sections`, plus the full tool declarations in `toolsAdded`;
- every later change persists another system message that **patches** sections
  by name — `null` removes a section — and lists `toolsAdded` / `toolsRemoved`;
- a `compaction` entry carries an optional `systemMessage` checkpoint that
  becomes the leading system message of the compacted context.

Replaying those messages in order yields the current prompt and tool loadout.
There is no separate prompt-state entry.

```jsonc
{"type":"message","message":{
  "role":"system","content":"",
  "sections":{
    "preamble":"You are an expert coding assistant …",
    "tools":"<tools>\n- read: …\n</tools>",
    "project_context":"Project-specific instructions and guidelines:\n\n<project_instructions path=\"…/AGENTS.md\">…</project_instructions>"
  },
  "toolsAdded":[{"name":"read"}]}}
```

## 2. Section order

`dist/core/system-prompt.js#buildSystemPromptSections` emits the sections in a
fixed order. `preamble` is the only raw section; every other one reaches the
model wrapped as `<name>…</name>`.

| # | Section | Carries | Sourced from |
| --- | --- | --- | --- |
| 1 | `preamble` | base instructions | built-in text, or `SYSTEM.md` / `--system-prompt` (replaces it) |
| 2 | `tools` | one-line snippet per selected tool | tool registry |
| 3 | `rules` | tool + prompt guidelines | tool snippets, extension guidelines |
| 4 | `docs` | pointers into the pi docs tree | built-in |
| 5 | `addendum` | extra instructions | `APPEND_SYSTEM.md`, `--append-system-prompt` |
| 6 | `project_context` | the context-file chain | `AGENTS.md` / `CLAUDE.md` |
| 7 | `skills` | loaded skill descriptions | skill loader |
| 8 | `cwd` | resolved working directory | session cwd |

Extension-supplied sections are appended **after** `cwd`. When a custom prompt
is in force, Pi skips `tools`, `rules` and `docs` entirely — that absence is
how the sidebar detects a `SYSTEM.md` / `--system-prompt` override.

## 3. File precedence

### `SYSTEM.md` / `APPEND_SYSTEM.md`

`discoverSystemPromptFile()` / `discoverAppendSystemPromptFile()`:

1. `<cwd>/.pi/SYSTEM.md` — only when the project is **trusted**;
2. `<agent-dir>/SYSTEM.md` — otherwise (agent dir is `~/.pi/agent`).

The same rule covers `APPEND_SYSTEM.md`, and the two files are **not**
combined: a trusted project file replaces the agent-directory one.

The sidebar attributes the `addendum` / `preamble` text by comparing it with
the candidate files (exact or suffix match, so a CLI flag prepended to the file
content still resolves).

A candidate that exists on disk but did not end up in the prompt is
deliberately **not listed** — that is noise, not signal. The panel only ever
names files that were loaded.

### Context files (`AGENTS.md`)

`loadProjectContextFiles()` walks:

1. the **agent directory** (`~/.pi/agent`) — applies everywhere;
2. then the **ancestor chain** from the filesystem root down to `cwd`, so the
   closest file is rendered last.

Per directory the candidates are tried in order:

```text
AGENTS.override.md → AGENTS.md → AGENTS.MD → CLAUDE.md → CLAUDE.MD
```

`AGENTS.override.md` only replaces a context file **in the same directory**; it
does not suppress the agent-directory file or any other ancestor. A nested git
worktree whose own context file shadows the main repo's copy is skipped, so the
same instructions are not applied twice.

## 4. Loading signal and drift

Pi caches discovered resources at session start and re-reads them only on
`/reload` (`docs/slash-commands.md`). Nothing is written to disk when a file is
loaded, so the panel uses two signals instead:

- **the loading moment** — the timestamp of the newest system message (`···
  načteno HH:MM:SS` in the header). That is when the effective prompt was
  assembled, i.e. when the context files actually entered the request;
- **drift** — every *loaded* source file is stat-ed and compared with that
  timestamp. If the file is newer (`1 s` of slack for the write/replay race),
  the row is flagged `⚠ změněno na disku — /reload`, because the running prompt
  still shows the old text. A loaded file that disappeared is flagged
  `⚠ soubor zmizel`.

Nothing is compared for files that were not loaded, so an untrusted project
file, an `AGENTS.md` in an unrelated ancestor, or a rejected candidate stays
silent.

### Exact loading signal (`.prompt.json` sidecar)

For per-turn precision the Status face prefers a sidecar published from
`before_agent_start` by an in-process extension (only an in-process extension
can see the prompt the engine resolved). **This repo ships one**:
[`extensions/prompt-sidecar.ts`](../extensions/prompt-sidecar.ts) — no imports
beyond node builtins, no dependency on any other plugin or package.

```bash
cp extensions/prompt-sidecar.ts ~/.pi/agent/extensions/   # then /reload in pi
```

It registers three handlers and otherwise stays out of the way:

| Event | What it does |
| --- | --- |
| `before_agent_start` | captures `systemPromptOptions` + the rendered prompt → writes `<pane>.prompt.json` |
| `session_start` | writes a `live: false` marker, so a reload cannot leave a previous session's prompt looking current |
| `session_shutdown` | same dead marker |

A `pi-sidebar`-style sidecar written by any other extension uses the identical
format and is accepted just as well — the reader never cares who published it.

It is written straight from the event handler, keyed by the **pi agent's pane
id** — so it works with no other pane open, which is the normal setup when this
native sidebar is the only pane in the tab. Neither side needs the other's pane
to exist: this plugin derives the file path from the pane id alone
(`snapshot_path_for_pane`), and the extension publishes regardless of pane
state. A dead or newer-version file is ignored in favour of the replay, so exact
data can never outlive the session that produced it.

Within the extension, `resolveSections()` and `attribute()` are pure and
exported, so the risky parts are checkable without a pi session (see the
`scripts/` note in this file's history) — the write itself is plain atomic
fs code.

If row 5 reads `— nepřítomno`, the engine really did send no `addendum`: there
is no `APPEND_SYSTEM.md` in either discovery location *and* nothing passed
`--append-system-prompt`.

A launch alias that passes the flag itself (e.g.
`pi --append-system-prompt ~/.pi/agent/agents/cim-budu.md`) is invisible to
discovery, so both providers fall back to **content matching**: the `addendum`
body is compared with every `*.md` under `~/.pi/agent/agents/` (sorted), after
the two canonical `APPEND_SYSTEM.md` locations. A file only wins when its text
*is* the body, so the row reads `← /…/agents/cim-budu.md` — evidence, not a
guess. The engine still never reveals the path through `systemPromptOptions`
(`appendSystemPromptSourcePaths` stays internal), so the *exact* provider
necessarily reports such an append as `inline`; the preview and the replay
match are what name it.

| Field | Meaning |
| --- | --- |
| `capturedAt` | when the engine resolved this prompt |
| `sections` / `sectionChars` | the section set and per-section sizes, measured on the rendered prompt |
| `contextFiles[{path, chars}]` | every loaded `AGENTS.md`, with its real path |
| `appendSystemPrompt` / `customPrompt` | `{chars, source: "file" (with `path`) or `source: "inline"`, preview}` |
| `forced` | `forceSystemPrompt` was in play |
| `tools` / `skills` | the resolved loadouts |
| `systemPromptChars` | length of the rendered prompt |

`systemPromptOptions.sections` only carries *extension* sections, so the
publisher derives presence with the same rules `buildSystemPromptSections`
applies and measures sizes on the rendered prompt (each non-`preamble` section
is wrapped as `<name>…</name>`; the wrapper and its newlines are excluded, so
both providers report the same content size). A dead (`live: false`) or
newer-version sidecar is ignored and the transcript replay takes over, so the
panel works with or without the extension.

`preview` exists because the engine exposes only the *text* of an append
(`systemPromptOptions.appendSystemPrompt`); `appendSystemPromptSourcePaths` stays
internal, so a file passed with `--append-system-prompt <path>` is reported as
`inline` by necessity. The preview names it — `← inline · "CIM BUDU…"` instead of
an anonymous inline flag.

The exact provider is strictly optional: the replay covers section sources,
append attribution and previews on its own. It exists for the two things the
transcript cannot express — `forceSystemPrompt` and a per-turn snapshot that
reflects handler changes before the next message is written.

## 5. What the Status face renders

The provider is named in the header: `exact · before_agent_start` (sidecar) or
`replay ze session logu` (transcript). Both render the same frame.

Exact mode, with an inline `--append-system-prompt` addendum — the case no file
can explain:

```text
🧠 Systémový prompt (exact · before_agent_start · 8 sekcí · 1.4k zn · načteno 10:42:20)
├─ 1 preamble            169 zn
├─ 2 tools               90 zn
│    · 12 nástrojů: read, bash, edit, write, bg_wait +7
├─ 3 rules               81 zn
├─ 4 docs                120 zn
├─ 5 addendum            138 zn
│    ← inline · "ALWAYS RESPOND IN CAVEMAN MODE. Drop articles and fi…" (136 zn)
├─ 6 project_context     584 zn
│    └─ D:/…/pi-herdr-sidebar/AGENTS.md (471 zn)
├─ 7 skills              75 zn
│    · 3 skillů (111 zn): herdr-plugin-dev, spai-tasks, pi-lens-lsp-navigation
└─ 8 cwd                 51 zn
```

Replay mode, same session without the extension:

```text
🧠 Systémový prompt (replay ze session logu · 7 sekcí · 18k zn · načteno 10:19:22)
├─ 1 preamble            169 zn  You are an expert coding assistant operating…
├─ 2 tools               8.3k zn  - read: Read file contents
│    · 85 nástrojů: read, bash, edit, write, bg_wait +80
├─ 3 rules               4.1k zn  - Use bash for file operations like ls, rg, …
├─ 4 docs                1.3k zn  Pi documentation (read only when the user as…
├─ 5 addendum            — nepřítomno
├─ 6 project_context     669 zn
│    └─ D:/…/pi-herdr-sidebar/AGENTS.md (470 zn)
├─ 7 skills              4.1k zn  The following skills provide specialized ins…
└─ 8 cwd                 62 zn  D:/01_programovani/herdr/plugins/pi-herdr-sidebar
```

- the documented frame is **always** complete, so an absent addendum is as
  visible as a present one (`— nepřítomno`), and the numbers state the
  sequence;
- `zn` is the section's character count (transcript for replay, measured on the
  rendered prompt for exact);
- `⚠ vynucený prompt` in the header means `forceSystemPrompt` replaced the whole
  prompt — only the exact provider can see that;
- `⟳N` marks a section patched by a later system message; `· odstraněno
  pozdějším patchem` marks a section a later message removed with `null`;
- `[ext]` marks a section Pi did not build (extension-injected);
- children name the **loaded** sources only: every `AGENTS.md` path with its
  character count and the matching `APPEND_SYSTEM.md` / `SYSTEM.md` file. A
  drifted one carries `⚠ změněno na disku — /reload` (or `⚠ soubor zmizel`);
  files that were not loaded get no row at all;
- the replayed tool loadout sits under `tools` (`(+N −M)` only when more than
  one message declared tools); skill names come from the exact provider only;
- the header shows the section count, total characters, the loading moment and
  the number of patching system messages.

## 6. Reading it by hand

The system messages are ordinary JSONL lines — no Pi process needed:

```bash
jq -c 'select(.type=="message" and .message.role=="system") | .message.sections' \
  ~/.pi/agent/sessions/--<cwd>--/<timestamp>_<id>.jsonl
```

Per-section character counts for the newest session:

```bash
head -1 …/session.jsonl
jq -r 'select(.type=="message" and .message.role=="system") | .message.sections | to_entries[] | "\(.key)\t\(.value|length)"' \
  …/session.jsonl
```

`jq` preserves the on-disk key order, which equals Pi's build order. Inside the
sidebar `serde_json` has no `preserve_order`, so the tree is ordered by Pi's
documented build list instead — identical for every section Pi itself emits.
