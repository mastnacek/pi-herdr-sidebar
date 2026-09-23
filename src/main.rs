mod shared;
mod slices;

use slices::view::ViewMode;
use std::env;
use std::io;
use std::path::PathBuf;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("view");

    match command {
        "view" => {
            slices::view::run_view(ViewMode::Pane, None)?;
        }
        "popup" => {
            slices::view::run_view(ViewMode::Popup, None)?;
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
            slices::view::run_view(ViewMode::Renderer, snapshot_path)?;
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
        "ensure-watch" => {
            // Detached watcher: --tab <tab_id> [--watch-lock <path>]
            let get = |flag: &str| {
                args.iter()
                    .position(|a| a == flag)
                    .and_then(|i| args.get(i + 1))
                    .cloned()
            };
            let tab_id = get("--tab");
            let watch_lock = get("--watch-lock").map(std::path::PathBuf::from);
            match tab_id {
                Some(tid) => {
                    if let Err(e) = slices::actions::run_ensure_watch(&tid, watch_lock.as_deref()) {
                        eprintln!("Error in ensure-watch: {}", e);
                    }
                }
                None => eprintln!("ensure-watch requires --tab <tab_id>"),
            }
        }
        _ => {
            // Default to view
            slices::view::run_view(ViewMode::Pane, None)?;
        }
    }

    Ok(())
}
