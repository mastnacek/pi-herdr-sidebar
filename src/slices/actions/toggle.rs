use crate::shared::{HerdrClient, PluginContext};

pub fn run_toggle() -> Result<(), String> {
    let client = HerdrClient::new();
    let ctx = PluginContext::load();

    // Check if sidebar pane already exists in this tab
    if let Some(existing) = client.find_sidebar_pane(ctx.tab_id.as_deref()) {
        client.close_pane(&existing.pane_id)?;
        client.notify("Pi Herdr Sidebar", "Sidebar closed.");
        return Ok(());
    }

    #[cfg(windows)]
    let entrypoint = "sidebar-win";
    #[cfg(not(windows))]
    let entrypoint = "sidebar";

    client.open_plugin_pane(entrypoint)?;
    client.notify("Pi Herdr Sidebar", "Sidebar opened.");

    Ok(())
}
