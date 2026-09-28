# System-prompt tree (Status face)

How the sidebar shows what the model *actually* received at the top of the
request: the base instructions, the `AGENTS.md` chain, and whether an
`APPEND_SYSTEM.md` addendum made it in.

Everything is replayed from the Pi session transcript. No file is
re-discovered to decide what was loaded — discovery is only used to *name* the
source of a section. That keeps the panel honest when a project is untrusted,
a worktree shadows a context file, or `SYSTEM.md` replaced the default prompt.

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
|---|---|---|---|
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
silent. For an exact per-turn loading signal (including forced prompts and CLI
appends that never touch a file), an in-process extension can observe
`before_agent_start.systemPromptOptions` — `contextFiles[{path, content}]`,
`appendSystemPrompt`, `customPrompt`, `selectedTools` — and publish it as a
sidecar the same way `pi-plugin-dev` publishes skill state.

## 5. What the Status face renders

```text
🧠 Systémový prompt (pi replay · 7 sekcí · 18k zn · načteno 10:19:22)
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
- `zn` is the section's character count in the transcript;
- `⟳N` marks a section patched by a later system message; `· odstraněno
  pozdějším patchem` marks a section a later message removed with `null`;
- `[ext]` marks a section Pi did not build (extension-injected);
- children name the **loaded** sources only: every `AGENTS.md` path with its
  character count and the matching `APPEND_SYSTEM.md` / `SYSTEM.md` file. A
  drifted one carries `⚠ změněno na disku — /reload` (or `⚠ soubor zmizel`);
  files that were not loaded get no row at all;
- the replayed tool loadout sits under `tools` (`(+N −M)` only when more than
  one message declared tools);
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
