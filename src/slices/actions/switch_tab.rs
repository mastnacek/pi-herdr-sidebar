use crate::shared::{find_active_snapshot, write_tab_request, PluginContext};

pub fn run_switch_tab() -> Result<(), String> {
    let ctx = PluginContext::load();
    // Require our own pane context: falling back to the newest snapshot in
    // the global dir could route the tab-switch request to another tab's
    // session when multiple agents are running.
    let own = ctx.pane_id.as_deref().ok_or("no pane context")?;
    let snapshot_path =
        find_active_snapshot(Some(own)).ok_or_else(|| "No active Pi snapshot found".to_string())?;

    // Default to triggering column 12 (Skills) or column 2 (Status)
    write_tab_request(&snapshot_path, 12, 1);
    Ok(())
}
