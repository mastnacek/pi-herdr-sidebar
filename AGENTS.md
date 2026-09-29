# Agent Guidelines for `pi-herdr-sidebar`

## MANDATORY SKILL USAGE

Whenever working on, modifying, designing, or debugging this Herdr plugin (or any Herdr plugin), **YOU MUST ALWAYS USE THE `herdr-plugin-dev` SKILL**:

- **Skill:** `herdr-plugin-dev`
- **Location:** `~/.pi/agent/git/github.com/mastnacek/pi-herdr-plugin-dev/skills/herdr-plugin-dev/SKILL.md`

Read and follow all rules, architectural constraints (VSA), KB queries, and patterns declared in that skill.

## Linux Builds (shared-folder VM)

This tree lives on a VirtualBox shared folder (`vboxsf`). Building in it does not work: cargo's many
small writes are so slow that tiny crates grind for tens of minutes, and full runs ended in disk-full
errors and rustc ICEs (`ArArchiveBuilder` panic while writing rlibs). Never build into the local
`target/`:

```bash
CARGO_TARGET_DIR=/tmp/pi_sidebar_target cargo build --release
CARGO_TARGET_DIR=/tmp/pi_sidebar_target cargo test        # full suite passes on Linux
mkdir -p target/release && cp /tmp/pi_sidebar_target/release/pi_sidebar target/release/pi_sidebar
```

The copy step is mandatory: `herdr-plugin.toml` runs `target/release/pi_sidebar` (and `.exe` on
Windows), so the binary must exist in the tree or Herdr cannot spawn panes/actions. `target/release/`
then holds only the binary — no fingerprints — so every rebuild must redo the copy.

Also on Linux:

- `src/shared/snapshot.rs` `is_process_running()` uses `libc::kill(pid, 0)` under `cfg(unix)`; the
  `libc` dep is declared under `[target.'cfg(unix)'.dependencies]` in `Cargo.toml`. Never compile that
  arm out or the Windows-only build will silently hide a Linux/macOS breakage again.
- Smoke-test without a pane: `timeout 6 script -qec "./target/release/pi_sidebar view" /tmp/frame.log`
  — expect alt-screen escape sequences and exit 0.
- Windows-side tooling writes CRLF into tracked files, which shows as whole-file rewrites in
  `git diff`. Check `git diff --ignore-cr-at-eol --stat` first; if it is empty, the change is pure
  line-ending churn — restore with `git checkout --` instead of committing it.
- The `vboxsf` ownership trips git: `git config --global --add safe.directory
  /home/jara/01_programovani/herdr/plugins/pi-herdr-sidebar` (and the monorepo root) once per machine.
