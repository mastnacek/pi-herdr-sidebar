mod shared;
mod slices;

use slices::view::state::Tab;
use slices::view::{Startup, ViewMode};
use std::env;
use std::io;
use std::path::PathBuf;

/// Composition root. Subcommands map 1:1 onto `herdr-plugin.toml` entrypoints:
/// `usage` runs the standalone plugin-usage overview window, `view` / `popup`
/// render a sidebar face (optionally `--tab <name>`), `renderer` replaces the
/// legacy node renderer, and the remaining arms are Herdr actions that talk back
/// to the multiplexer.
fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("view");
    let startup = Startup {
        tab: parse_tab(&args),
    };

    match command {
        "usage" => {
            // Standalone plugin/skill usage overview window (own Herdr chord).
            slices::shortcuts::overview::run_overview()?;
        }
        "view" => {
            slices::view::run_view(ViewMode::Pane, None, startup)?;
        }
        "popup" => {
            slices::view::run_view(ViewMode::Popup, None, startup)?;
        }
        "renderer" => {
            // Parse --snapshot <path>
            let mut snapshot_path: Option<PathBuf> = None;
            let mut i = 2;
            while i < args.len() {
                if args[i] == "--snapshot" && i + 1 < args.len() {
                    snapshot_path = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else {
                    i += 1;
                }
            }
            slices::view::run_view(ViewMode::Renderer, snapshot_path, startup)?;
        }
        "toggle" => {
            if let Err(e) = slices::actions::run_toggle() {
                eprintln!("Error toggling sidebar: {}", e);
            }
        }
        "popup-action" => {
            if let Err(e) = slices::actions::run_popup() {
                eprintln!("Error opening popup: {}", e);
            }
        }
        "notes-popup-action" => {
            if let Err(e) = slices::actions::run_notes_popup() {
                eprintln!("Error opening notes modal: {}", e);
            }
        }
        "usage-popup-action" => {
            if let Err(e) = slices::actions::run_usage_popup() {
                eprintln!("Error opening usage overview: {}", e);
            }
        }
        "switch-tab" => {
            if let Err(e) = slices::actions::run_switch_tab() {
                eprintln!("Error switching tab: {}", e);
            }
        }
        "ensure" => {
            if let Err(e) = slices::actions::run_ensure() {
                eprintln!("Error ensuring sidebar: {}", e);
            }
        }
        _ => {
            // Default to view
            slices::view::run_view(ViewMode::Pane, None, startup)?;
        }
    }

    Ok(())
}

/// Parses `--tab <name>` so Herdr entrypoints can open a specific face
/// (e.g. `pi_sidebar.exe popup --tab notes` for the notes modal).
fn parse_tab(args: &[String]) -> Option<Tab> {
    let i = args.iter().position(|a| a == "--tab")?;
    match args.get(i + 1)?.to_ascii_lowercase().as_str() {
        "notes" | "4" => Some(Tab::Notes),
        "shortcuts" | "keys" | "5" => Some(Tab::Shortcuts),
        "mcp" | "3" => Some(Tab::Mcp),
        "skills" | "2" => Some(Tab::Skills),
        "status" | "1" => Some(Tab::Status),
        "zen" | "0" => Some(Tab::Zen),
        _ => None,
    }
}
