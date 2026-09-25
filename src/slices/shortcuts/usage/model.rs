//! Aggregate model for the plugin-usage scanner: what we count, and how a tool
//! name is attributed to the plugin that provides it.
use serde::{Deserialize, Serialize};

/// Tools that ship with the agent itself rather than with a plugin.
pub(crate) const BUILTIN_TOOLS: &[&str] = &[
    "bash",
    "read",
    "edit",
    "write",
    "read_all",
    "ls",
    "glob",
    "grep",
    "task",
    "todo",
    "read_skill",
];

/// Attributes a tool name to the plugin that provides it.
///
/// Order matters: `mcp__<server>` namespace proxies and the hyphenated
/// knowledge-base variants must be matched before the generic rules.
pub fn plugin_of(tool: &str) -> &'static str {
    // MCP namespace proxies (`mcp__openrouter`, `mcp__lotusscript_lsp`, ...)
    if let Some(rest) = tool.strip_prefix("mcp__") {
        return match rest {
            r if r.starts_with("lotusscript") => "mcp-lotusscript-lsp",
            r if r.starts_with("openrouter") => "mcp-openrouter",
            r if r.contains("knowledge") => "mcp-knowledge-base",
            _ => "other",
        };
    }

    if tool.starts_with("ctx_") {
        return "context-mode";
    }
    // Both `knowledge_base_kb_*` and `knowledge-base_kb_*` show up in real logs.
    if tool.starts_with("knowledge_base") || tool.starts_with("knowledge-base") {
        return "mcp-knowledge-base";
    }
    if tool.starts_with("lotusscript_lsp") || tool.starts_with("lotusscript-lsp") {
        return "mcp-lotusscript-lsp";
    }
    if tool.starts_with("lotusscript") {
        return "pi-lotusscript-modular";
    }
    if tool.starts_with("openrouter_") {
        return "mcp-openrouter";
    }
    if tool.starts_with("lens_")
        || tool.starts_with("lsp_")
        || tool.starts_with("pi_lens_")
        || tool.starts_with("pi_lsp_")
        || matches!(
            tool,
            "symbol_search"
                | "module_report"
                | "project_report"
                | "read_symbol"
                | "read_enclosing"
                | "effective_config"
                | "lsp_navigation"
                | "lens_diagnostic_mark"
        )
    {
        return "pi-lens";
    }
    if matches!(
        tool,
        "web_search" | "fetch_content" | "get_search_content" | "source_check" | "web_fetch"
    ) {
        return "pi-web-access";
    }
    // Subagent family (incl. its wait tool) — but `batch_*` is its own plugin.
    if tool.starts_with("subagent") || tool == "bg_wait" {
        return "pi-subagents";
    }
    if tool.starts_with("batch_") {
        return "pi-batch";
    }
    if matches!(
        tool,
        "record_spai_item" | "search_spai_items" | "update_spai_status"
    ) {
        return "pi-spai";
    }
    if matches!(
        tool,
        "list_projects" | "search_projects" | "add_project_root" | "add_project_manually"
    ) {
        return "pi-projects";
    }
    if matches!(tool, "mcp" | "mcpScript") {
        return "pi-mcp-adapter";
    }
    if tool.starts_with("herdr_") {
        return "herdr-plugin-dev";
    }
    if tool.starts_with("goal_") {
        return "pi-goal";
    }
    if matches!(tool, "record_adr" | "search_adrs") {
        return "pi-adr";
    }
    if tool.starts_with("metaculus_") {
        return "metaculus";
    }
    if tool.starts_with("pixellab_") {
        return "pixellab";
    }
    if tool.starts_with("career_") {
        return "career";
    }
    if tool.starts_with("spai_") {
        return "pi-spai";
    }
    if BUILTIN_TOOLS.contains(&tool) {
        return "builtin";
    }
    "other"
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginUsage {
    pub plugin: String,
    pub calls: u64,
    /// Distinct tools this plugin answered, most used first.
    pub tools: Vec<(String, u64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Counted {
    pub name: String,
    pub count: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsageStats {
    pub files: usize,
    pub messages: u64,
    pub tool_calls: u64,
    pub builtin_calls: u64,
    pub plugins: Vec<PluginUsage>,
    pub tools: Vec<Counted>,
    pub skills: Vec<Counted>,
    pub commands: Vec<Counted>,
    pub elapsed_ms: u64,
    /// Unix seconds of the newest session file seen.
    pub newest_session: Option<u64>,
}

impl UsageStats {
    /// Calls served by plugins, i.e. everything except the agent's own tools.
    pub fn plugin_calls(&self) -> u64 {
        self.tool_calls.saturating_sub(self.builtin_calls)
    }

    /// Plugin rows worth showing: builtins are reported separately, and the
    /// trailing long tail is summarised by the caller.
    pub fn ranked_plugins(&self) -> Vec<&PluginUsage> {
        self.plugins
            .iter()
            .filter(|p| p.plugin != "builtin")
            .collect()
    }

    pub fn builtin_row(&self) -> Option<&PluginUsage> {
        self.plugins.iter().find(|p| p.plugin == "builtin")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_tool_names_to_plugins() {
        assert_eq!(plugin_of("ctx_execute"), "context-mode");
        assert_eq!(plugin_of("knowledge_base_kb_search"), "mcp-knowledge-base");
        assert_eq!(plugin_of("knowledge-base_kb_search"), "mcp-knowledge-base");
        assert_eq!(plugin_of("mcp__knowledge_base"), "mcp-knowledge-base");
        assert_eq!(plugin_of("lens_diagnostics"), "pi-lens");
        assert_eq!(plugin_of("lsp_diagnostics"), "pi-lens");
        assert_eq!(plugin_of("web_search"), "pi-web-access");
        assert_eq!(plugin_of("fetch_content"), "pi-web-access");
        assert_eq!(plugin_of("mcp"), "pi-mcp-adapter");
        assert_eq!(plugin_of("mcpScript"), "pi-mcp-adapter");
        assert_eq!(plugin_of("mcp__lotusscript_lsp"), "mcp-lotusscript-lsp");
        assert_eq!(
            plugin_of("lotusscript_lsp_lsp_diagnostics"),
            "mcp-lotusscript-lsp"
        );
        assert_eq!(plugin_of("lotusscript_compile"), "pi-lotusscript-modular");
        assert_eq!(plugin_of("subagent"), "pi-subagents");
        assert_eq!(plugin_of("bg_wait"), "pi-subagents");
        assert_eq!(plugin_of("batch_submit_goal"), "pi-batch");
        assert_eq!(plugin_of("record_spai_item"), "pi-spai");
        assert_eq!(plugin_of("openrouter_list-models"), "mcp-openrouter");
        assert_eq!(plugin_of("mcp__openrouter"), "mcp-openrouter");
        assert_eq!(plugin_of("search_projects"), "pi-projects");
        assert_eq!(plugin_of("bash"), "builtin");
        assert_eq!(plugin_of("read"), "builtin");
        assert_eq!(plugin_of("unknown_thing"), "other");
    }

    #[test]
    fn builtins_are_split_out_of_the_plugin_ranking() {
        let stats = UsageStats {
            tool_calls: 10,
            builtin_calls: 4,
            plugins: vec![
                PluginUsage {
                    plugin: "builtin".into(),
                    calls: 4,
                    tools: vec![],
                },
                PluginUsage {
                    plugin: "context-mode".into(),
                    calls: 6,
                    tools: vec![],
                },
            ],
            ..Default::default()
        };
        assert_eq!(stats.plugin_calls(), 6);
        assert_eq!(stats.ranked_plugins().len(), 1);
        assert_eq!(stats.ranked_plugins()[0].plugin, "context-mode");
        assert_eq!(stats.builtin_row().map(|p| p.calls), Some(4));
    }
}
