use crate::shared::{pi_sidebar_snapshots_dir, HerdrClient, PluginContext};
use std::fs::{self, File};
use std::path::PathBuf;

/// Marketplace-proven strategy (herdr-sidebar v0.13): open the sidebar on
/// every ensure event, in any tab, without waiting for an agent to register.
/// A snooze list records tabs the user closed via toggle so hooks don't fight
/// the user. Serialization uses an OS advisory lock (std File::try_lock) —
/// the OS releases it if a process crashes, so no pid files, no stale-lock
/// cleanup, no PID-recycling false positives.

fn entrypoint() -> &'static str {
    #[cfg(windows)]
    {
        "sidebar-win"
    }
    #[cfg(not(windows))]
    {
        "sidebar"
    }
}

fn launcher_lock_path() -> PathBuf {
    pi_sidebar_snapshots_dir().join("launcher.lock")
}

fn snooze_path() -> PathBuf {
    pi_sidebar_snapshots_dir().join("snoozed-tabs.json")
}

/// OS-backed launcher lock. `blocking` for discrete user actions (toggle),
/// non-blocking try for bursty focus hooks (redundant invocations yield —
/// the winner opens the pane and later events observe it).
struct LaunchLock {
    _file: File,
}

impl LaunchLock {
    fn acquire(wait: bool) -> Option<Self> {
        let path = launcher_lock_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .ok()?;
        let acquired = if wait {
            file.lock().is_ok()
        } else {
            file.try_lock().is_ok()
        };
        acquired.then_some(Self { _file: file })
    }
}

/// Tabs where the user explicitly closed the sidebar (via toggle). Hooks skip
/// these until the user opens the sidebar again.
pub fn snoozed_tabs() -> Vec<String> {
    fs::read_to_string(snooze_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn set_snoozed(tab_id: &str, snoozed: bool) {
    let mut tabs: Vec<String> = snoozed_tabs()
        .into_iter()
        .filter(|t| t != tab_id)
        .collect();
    if snoozed {
        tabs.push(tab_id.to_string());
    }
    if let Some(parent) = snooze_path().parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(&tabs) {
        let _ = fs::write(snooze_path(), json);
    }
}

/// Pick a pane in the event's tab to anchor the split against, preferring the
/// pane that fired the event, then a pi agent pane, then any pane in the tab.
/// Returns None when the tab has no panes yet — opening without an anchor
/// lands in the currently active tab, so callers must wait for the next
/// pane.focused event instead.
fn anchor_pane(client: &HerdrClient, ctx: &PluginContext) -> Option<String> {
    let panes = client.list_panes();
    let in_tab = |p: &crate::shared::HerdrPaneInfo| {
        ctx.tab_id
            .as_deref()
            .is_none_or(|tid| p.tab_id.as_deref() == Some(tid))
    };

    // 1. The pane the event fired for.
    if let Some(pid) = ctx.pane_id.as_deref() {
        if let Some(p) = panes.iter().find(|p| p.pane_id == pid && in_tab(p)) {
            return Some(p.pane_id.clone());
        }
    }
    // 2. A pi agent pane in the tab.
    if let Some(p) = panes
        .iter()
        .find(|p| in_tab(p) && p.agent.as_deref() == Some("pi"))
    {
        return Some(p.pane_id.clone());
    }
    // 3. Any pane in the tab.
    panes
        .iter()
        .find(|p| in_tab(p))
        .map(|p| p.pane_id.clone())
}

/// Event handler for tab.created / tab.focused / pane.focused /
/// workspace.focused / workspace.created. Idempotently opens the sidebar in
/// the event's tab when it is missing and not snoozed by the user.
pub fn run_ensure() -> Result<(), String> {
    let client = HerdrClient::new();
    let ctx = PluginContext::load();

    if let Some(tid) = ctx.tab_id.as_deref() {
        if snoozed_tabs().iter().any(|t| t == tid) {
            return Ok(()); // user closed it here; respect that
        }
    }

    // Anchor required: without a pane in this tab, `plugin pane open` would
    // land in whatever tab is active right now. A later pane.focused event
    // gives us another chance.
    let Some(anchor) = anchor_pane(&client, &ctx) else {
        return Ok(());
    };

    // Non-blocking: concurrent focus events bail; the winner opens.
    let Some(_lock) = LaunchLock::acquire(false) else {
        return Ok(());
    };

    // Re-check under the lock.
    if client.find_sidebar_pane(ctx.tab_id.as_deref()).is_some() {
        return Ok(());
    }

    client.open_plugin_pane(entrypoint(), Some(&anchor))?;
    Ok(())
}

/// Toggle action: open-or-close like the ensure path, but records user intent
/// (snooze) so hooks don't reopen the sidebar behind the user's back.
pub fn run_toggle() -> Result<(), String> {
    let client = HerdrClient::new();
    let ctx = PluginContext::load();

    // Blocking lock: a discrete user action should wait out a concurrent hook.
    let _lock = LaunchLock::acquire(true)
        .ok_or_else(|| "could not acquire launcher lock".to_string())?;

    if let Some(existing) = client.find_sidebar_pane(ctx.tab_id.as_deref()) {
        client.close_pane(&existing.pane_id)?;
        if let Some(tid) = ctx.tab_id.as_deref() {
            set_snoozed(tid, true);
        }
        client.notify("Pi Herdr Sidebar", "Sidebar closed.");
        return Ok(());
    }

    let anchor = anchor_pane(&client, &ctx)
        .ok_or_else(|| "no pane to anchor the sidebar to in this tab".to_string())?;
    client.open_plugin_pane(entrypoint(), Some(&anchor))?;
    if let Some(tid) = ctx.tab_id.as_deref() {
        set_snoozed(tid, false);
    }
    client.notify("Pi Herdr Sidebar", "Sidebar opened.");
    Ok(())
}