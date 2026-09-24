use std::path::PathBuf;

use crate::shared::{snapshot_path_for_pane, HerdrPaneInfo};

/// Find Pi agent in the target tab.
pub fn resolve_target_pane(
    panes: &[HerdrPaneInfo],
    current_tab: Option<&str>,
    own_pane: &str,
) -> Option<HerdrPaneInfo> {
    panes
        .iter()
        .filter(|p| {
            let same_tab = current_tab.is_none_or(|tid| p.tab_id.as_deref() == Some(tid));
            let not_self = p.pane_id.as_str() != own_pane;
            let is_pi = p.agent.as_deref() == Some("pi")
                || p.terminal_title_stripped
                    .as_deref()
                    .is_some_and(|t| t.contains('π'))
                || p.terminal_title.as_deref().is_some_and(|t| t.contains('π'));
            same_tab && not_self && is_pi
        })
        .max_by_key(|p| (p.focused.unwrap_or(false), p.agent.as_deref() == Some("pi")))
        .cloned()
}

pub struct PaneResolution {
    pub target_pane_id: Option<String>,
    pub snapshot_path: Option<PathBuf>,
    pub live_session_id: Option<String>,
    pub refresh_status: String,
}

pub fn resolve_pane_binding(
    panes: &[HerdrPaneInfo],
    current_tab: Option<&str>,
    own_pane: &str,
) -> PaneResolution {
    if let Some(pi_pane) = resolve_target_pane(panes, current_tab, own_pane) {
        let candidate_path = snapshot_path_for_pane(&pi_pane.pane_id);
        let session_id = pi_pane.session_id();
        let status = format!(
            "Tab: {} → Pane: {}",
            current_tab.unwrap_or("?"),
            pi_pane.pane_id
        );
        PaneResolution {
            target_pane_id: Some(pi_pane.pane_id),
            snapshot_path: Some(candidate_path),
            live_session_id: session_id,
            refresh_status: status,
        }
    } else {
        PaneResolution {
            target_pane_id: None,
            snapshot_path: None,
            live_session_id: None,
            refresh_status: format!("Tab: {} (bez pi relace)", current_tab.unwrap_or("?")),
        }
    }
}
