// @ts-nocheck
// install: cp extensions/prompt-sidecar.ts ~/.pi/agent/extensions/  (then /reload)
/**
 * prompt-sidecar — publish the resolved system prompt for the native sidebar.
 *
 * Standalone by design: no imports beyond node builtins, no dependency on any
 * other pi extension or package. It exists because `before_agent_start` is the
 * only place the engine hands over the prompt it actually rendered — the
 * transcript replay in `pi-herdr-sidebar` cannot see `forceSystemPrompt`, and
 * gets section sources only by inferring them.
 *
 * Writes `<pane>.prompt.json` next to the pane snapshots, keyed by the pi
 * agent's `HERDR_PANE_ID`, which is exactly how the Rust consumer derives the
 * path. A `live: false` marker is written on session start and end, so the
 * consumer falls back to its replay instead of showing a previous session's
 * prompt.
 *
 * Format: see `docs/system-prompt-tree.md` in pi-herdr-sidebar.
 */
import { existsSync, mkdirSync, readFileSync, readdirSync, renameSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";

const VERSION = 1;
const PREVIEW_CHARS = 48;
const BUILTIN = ["preamble", "tools", "rules", "docs", "addendum", "project_context", "skills", "cwd"];

const stateDir = () =>
  process.env.PI_SIDEBAR_STATE_DIR ?? join(homedir(), ".pi", "agent", "pi-sidebar");

const sidecarPath = (paneId) =>
  join(stateDir(), `${paneId.replace(/[^A-Za-z0-9._-]/g, "_")}.prompt.json`);

const normalize = (text) => text.replace(/\r\n/g, "\n").trim();

function previewOf(text) {
  const line =
    text
      .split("\n")
      .map((part) => part.trim())
      // Skip YAML frontmatter fences: `agents/*.md` files often open with `---`.
      .find((part) => part.length > 0 && part !== "---") ?? "";
  return line.length > PREVIEW_CHARS ? `${line.slice(0, PREVIEW_CHARS)}…` : line;
}

/** Files a CLI flag may have pointed at, in the order pi itself would prefer. */
function candidates(cwd, fileName) {
  const out = [];
  if (cwd) out.push(join(cwd, ".pi", fileName));
  out.push(join(homedir(), ".pi", "agent", fileName));
  // `--append-system-prompt <path>` never goes through discovery, and the alias
  // pattern keeps those files here. A file only wins when its text *is* the
  // body, so this stays evidence rather than a guess.
  const agents = join(homedir(), ".pi", "agent", "agents");
  if (existsSync(agents)) {
    for (const name of readdirSync(agents).filter((n) => n.endsWith(".md")).sort()) {
      out.push(join(agents, name));
    }
  }
  return out;
}

/** Attribute prompt text to a file, or report it as inline. */
export function attribute(text, cwd, fileName) {
  const source = { chars: text.length, source: "inline", preview: previewOf(text) };
  const target = normalize(text);
  if (!target) return source;
  for (const candidate of candidates(cwd, fileName)) {
    try {
      const file = normalize(readFileSync(candidate, "utf8"));
      if (file && (target === file || target.endsWith(file))) {
        return { ...source, source: "file", path: candidate };
      }
    } catch {
      // Unreadable or missing candidate: keep looking.
    }
  }
  return source;
}

/**
 * Section set and sizes as the engine sent them.
 *
 * `systemPromptOptions.sections` holds only *extension* sections, so presence is
 * derived with the rules `buildSystemPromptSections` applies and the sizes are
 * measured on the rendered prompt: every section except `preamble` is wrapped as
 * `<name>…</name>`, joined by a blank line.
 *
 * Exported so it can be exercised without a pi session (`harness` in the dev
 * notes); the extension itself has no importers.
 */
export function resolveSections(options, rendered) {
  const custom = (options.customPrompt ?? "").length > 0 || typeof options.forceSystemPrompt === "string";
  const names = ["preamble"];
  if (!custom) names.push("tools", "rules", "docs");
  if ((options.appendSystemPrompt ?? "").length > 0) names.push("addendum");
  if ((options.contextFiles ?? []).length > 0) names.push("project_context");
  const canRead = (options.selectedTools ?? []).some((t) => t === "read" || t === "bash");
  if (canRead && (options.skills ?? []).length > 0) names.push("skills");
  names.push("cwd");
  for (const [name, value] of Object.entries(options.sections ?? {})) {
    if (typeof value === "string" && value.length > 0 && !names.includes(name)) names.push(name);
  }

  const sectionChars = {};
  const starts = [];
  for (const name of names) {
    if (name === "preamble") continue;
    const open = `<${name}>`;
    const at = rendered.indexOf(open);
    if (at < 0) continue;
    const end = rendered.indexOf(`</${name}>`, at + open.length);
    if (end < 0) continue;
    // The engine wraps as `<name>\n{content}\n</name>`, so drop those newlines:
    // the number should describe the content, exactly as the transcript replay
    // measures it after stripping the same wrapper.
    const inner = rendered.slice(at + open.length, end).replace(/^\n+|\n+$/g, "");
    sectionChars[name] = inner.length;
    starts.push(at);
  }
  const first = starts.length > 0 ? Math.min(...starts) : -1;
  sectionChars.preamble = first >= 0 ? Math.max(0, first - 2) : rendered.length;

  return {
    sections: names.filter((name) => name === "preamble" || sectionChars[name] !== undefined),
    sectionChars,
  };
}

function write(file) {
  const paneId = process.env.HERDR_PANE_ID;
  if (!paneId) return false;
  const path = sidecarPath(paneId);
  try {
    mkdirSync(dirname(path), { recursive: true });
    const tmp = `${path}.${process.pid}.tmp`;
    writeFileSync(tmp, JSON.stringify(file), "utf8");
    renameSync(tmp, path);
    return true;
  } catch {
    return false;
  }
}

/** `live: false` payload: no provenance to report. */
const deadFile = () => ({
  version: VERSION,
  live: false,
  generatedAt: new Date().toISOString(),
  source: "before_agent_start",
  capturedAt: "",
  cwd: "",
  reason: "startup",
  forced: false,
  customPrompt: null,
  appendSystemPrompt: null,
  sections: [],
  sectionChars: {},
  contextFiles: [],
  tools: [],
  skills: [],
  systemPromptChars: 0,
  promptChars: 0,
});

export default function (pi) {
  let capture = null;
  let captureReason = "startup";

  pi.on("session_start", () => {
    capture = null;
    captureReason = "startup";
    write(deadFile());
  });

  pi.on("before_agent_start", (event) => {
    const options = event?.systemPromptOptions;
    if (!options) return;

    const rendered = event.systemPrompt ?? "";
    const cwd = options.cwd ?? "";
    const appended = options.appendSystemPrompt ?? "";
    const replaced = options.customPrompt ?? "";
    const { sections, sectionChars } = resolveSections(options, rendered);

    capture = {
      version: VERSION,
      live: true,
      generatedAt: new Date().toISOString(),
      source: "before_agent_start",
      capturedAt: new Date().toISOString(),
      cwd,
      reason: captureReason,
      forced: typeof options.forceSystemPrompt === "string",
      customPrompt: replaced ? attribute(replaced, cwd, "SYSTEM.md") : null,
      appendSystemPrompt: appended ? attribute(appended, cwd, "APPEND_SYSTEM.md") : null,
      sections,
      sectionChars,
      contextFiles: (options.contextFiles ?? [])
        .map((file) => ({ path: String(file?.path ?? ""), chars: String(file?.content ?? "").length }))
        .filter((file) => file.path.length > 0),
      tools: (options.selectedTools ?? []).map(String),
      skills: (options.skills ?? []).map((skill) => ({
        name: String(skill?.name ?? ""),
        descriptionChars: typeof skill?.description === "string" ? skill.description.length : 0,
      })),
      systemPromptChars: rendered.length,
      promptChars: typeof event.prompt === "string" ? event.prompt.length : 0,
    };

    captureReason = "turn";
    write(capture);
  });

  pi.on("session_shutdown", () => {
    capture = null;
    write(deadFile());
  });
}
