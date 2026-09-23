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

    // Anchor the split next to the pi agent pane when one exists in this tab.
    let target = client
        .list_panes()
        .into_iter()
        .find(|p| {
            let is_tab = ctx
                .tab_id
                .as_deref()
                .is_none_or(|tid| p.tab_id.as_deref() == Some(tid));
            is_tab
                && (p.agent.as_deref() == Some("pi")
                    || p.terminal_title
                        .as_deref()
                        .is_some_and(|t| t.contains('π') || t.to_lowercase().contains("pi")))
        })
        .map(|p| p.pane_id);
    client.open_plugin_pane(entrypoint, target.as_deref())?;
    client.notify("Pi Herdr Sidebar", "Sidebar opened.");

    Ok(())
}
