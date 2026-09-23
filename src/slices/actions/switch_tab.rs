use crate::shared::{find_active_snapshot, write_tab_request, PluginContext};

pub fn run_switch_tab() -> Result<(), String> {
    let ctx = PluginContext::load();
    let snapshot_path = find_active_snapshot(ctx.pane_id.as_deref())
        .ok_or_else(|| "No active Pi snapshot found".to_string())?;

    // Default to triggering column 12 (Skills) or column 2 (Status)
    write_tab_request(&snapshot_path, 12, 1);
    Ok(())
}
