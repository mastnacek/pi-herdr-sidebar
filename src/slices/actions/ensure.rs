use crate::shared::{pi_sidebar_snapshots_dir, HerdrClient, PluginContext};
use std::fs;
use std::process::Command;
use std::time::{Duration, Instant};

/// How long the detached watcher waits for a pi agent to register in the tab.
const WATCH_TIMEOUT: Duration = Duration::from_secs(90);
/// Poll interval for the watcher.
const WATCH_POLL: Duration = Duration::from_millis(750);

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

/// Is there a pi agent pane in the given tab?
fn has_pi_agent(client: &HerdrClient, tab_id: Option<&str>) -> bool {
    client.list_panes().iter().any(|p| {
        let is_tab = tab_id.is_none_or(|tid| p.tab_id.as_deref() == Some(tid));
        if !is_tab {
            return false;
        }
        if p.agent.as_deref() == Some("pi") {
            return true;
        }
        if let Some(title) = &p.terminal_title {
            if title.contains('π') || title.to_lowercase().contains("pi") {
                return true;
            }
        }
        false
    })
}

/// Open the sidebar pane under a cross-process pid lock so concurrent
/// events/watchers cannot open duplicate panes. Returns Ok(false) when
/// another process won the lock.
fn open_sidebar_exclusive(client: &HerdrClient) -> Result<bool, String> {
    let lock_path = pi_sidebar_snapshots_dir().join("ensure.lock");
    let my_pid = std::process::id();

    if let Ok(content) = fs::read_to_string(&lock_path) {
        if let Ok(prev) = content.trim().parse::<u32>() {
            if prev != my_pid && pid_alive(prev) {
                return Ok(false); // another ensure is opening right now
            }
        }
    }
    let _ = fs::write(&lock_path, my_pid.to_string());

    // Re-check under the lock: winner may have opened it since our last check.
    if client.find_sidebar_pane(None).is_some() {
        let _ = fs::remove_file(&lock_path);
        return Ok(false);
    }

    let res = client.open_plugin_pane(entrypoint());
    let _ = fs::remove_file(&lock_path);
    res.map(|_| true)
}

fn pid_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid)])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
}

/// Spawn a detached watcher process that waits for the pi agent to appear
/// in `tab_id`, then opens the sidebar pane. Used when `ensure` fires before
/// the agent has registered (tab.created race) — the event won't re-fire.
fn spawn_watcher(tab_id: &str) {
    // One watcher per tab: lock file records the WATCHER's pid (written by
    // the watcher itself at startup — the spawning parent exits immediately,
    // so its pid would go stale right away).
    let watch_lock = pi_sidebar_snapshots_dir().join(format!(
        "ensure-watch-{}.lock",
        crate::shared::sanitize_key(tab_id)
    ));

    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(_) => return,
    };
    let mut cmd = Command::new(exe);
    cmd.args(["ensure-watch", "--tab", tab_id, "--watch-lock"]);
    cmd.arg(&watch_lock);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW);
    }

    // Fire and forget; failure is non-fatal (ensure still ran its inline check).
    let _ = cmd.spawn();
}

/// Event handler for tab.created / tab.focused / pane.focused /
/// workspace.focused. Opens the sidebar if this tab hosts a pi agent and the
/// sidebar is missing. If no agent is registered yet, spawns a detached
/// watcher so the sidebar still appears once the agent starts (covers both
/// new sessions and post-`/reload` restarts with a closed sidebar).
pub fn run_ensure() -> Result<(), String> {
    let client = HerdrClient::new();
    let ctx = PluginContext::load();
    let tab_id = ctx.tab_id.as_deref();

    // 1. Sidebar already open in this tab — nothing to do.
    if client.find_sidebar_pane(tab_id).is_some() {
        return Ok(());
    }

    // 2. Agent present right now → open immediately.
    if has_pi_agent(&client, tab_id) {
        open_sidebar_exclusive(&client)?;
        return Ok(());
    }

    // 3. No agent yet → likely a tab.created race (agent starts moments
    //    later) or a fresh session. Spawn detached watcher to open the
    //    sidebar as soon as the pi agent registers.
    if let Some(tid) = ctx.tab_id.clone() {
        spawn_watcher(&tid);
    }

    Ok(())
}

/// Detached watcher loop: poll until a pi agent appears in `tab_id`, then
/// open the sidebar if it is still missing. Exits on success, timeout, or
/// once the sidebar exists.
pub fn run_ensure_watch(tab_id: &str, watch_lock: Option<&std::path::Path>) -> Result<(), String> {
    let client = HerdrClient::new();
    let deadline = Instant::now() + WATCH_TIMEOUT;

    // Claim the lock with our own pid; bail out if a live watcher exists.
    if let Some(lock_path) = watch_lock {
        let my_pid = std::process::id();
        if let Ok(content) = fs::read_to_string(lock_path) {
            if let Ok(prev) = content.trim().parse::<u32>() {
                if prev != my_pid && pid_alive(prev) {
                    return Ok(()); // another watcher already running
                }
            }
        }
        let _ = fs::write(lock_path, my_pid.to_string());
    }

    let _guard = watch_lock.map(|p| WatchLockGuard {
        path: p.to_path_buf(),
    });

    loop {
        // Sidebar opened meanwhile (by another ensure/watcher) → done.
        if client.find_sidebar_pane(Some(tab_id)).is_some() {
            return Ok(());
        }

        if has_pi_agent(&client, Some(tab_id)) {
            if client.find_sidebar_pane(Some(tab_id)).is_none() {
                open_sidebar_exclusive(&client)?;
            }
            return Ok(());
        }

        if Instant::now() >= deadline {
            return Ok(());
        }

        std::thread::sleep(WATCH_POLL);
    }
}

/// Removes the watcher lock file on exit so future events can spawn again.
struct WatchLockGuard {
    path: std::path::PathBuf,
}

impl Drop for WatchLockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
