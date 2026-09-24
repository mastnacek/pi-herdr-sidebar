use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpCallItem {
    pub server: String,
    pub badge: String,
    pub tool: String,
    pub is_error: bool,
    pub timestamp_ms: u64,
    pub summary: String,
}

#[derive(Debug, Clone, Default)]
pub struct McpTelemetry {
    pub total_calls: u64,
    pub total_errors: u64,
    pub active_calls_count: u64,
    pub servers_used: Vec<String>,
    pub recent_calls: Vec<McpCallItem>,
    pub last_call_ts_ms: u64,
    pub in_flight: bool,
}

/// Known badges for recognizable MCP servers.
fn badge_for(server: &str) -> &'static str {
    match server {
        "knowledge_base" | "knowledge-base" => "KB",
        "openrouter" => "OR",
        "metaculus" => "MC",
        "lotusscript_lsp" | "lotusscript-lsp" => "LSP",
        "hf_mcp_server" | "hf-mcp-server" => "HF",
        "context7" => "C7",
        _ => "MCP",
    }
}

/// Normalize MCP server names from ~/.pi/agent/mcp.json.
pub fn load_configured_servers() -> Vec<String> {
    let Some(home) = crate::shared::dirs_home() else {
        return Vec::new();
    };
    let mcp_path = home.join(".pi").join("agent").join("mcp.json");
    let Ok(content) = fs::read_to_string(mcp_path) else {
        return Vec::new();
    };
    #[derive(Deserialize)]
    struct McpConfig {
        #[serde(default, rename = "mcpServers")]
        mcp_servers: std::collections::BTreeMap<String, serde_json::Value>,
    }
    serde_json::from_str::<McpConfig>(&content)
        .map(|c| c.mcp_servers.into_keys().collect())
        .unwrap_or_default()
}

/// Classify tool name into (Server, Tool).
pub fn classify_mcp_tool(tool_name: &str, known_servers: &[String]) -> Option<(String, String)> {
    if tool_name.is_empty() {
        return None;
    }
    if tool_name == "mcp" {
        return Some(("mcp".to_string(), "gateway".to_string()));
    }
    if let Some(rest) = tool_name.strip_prefix("mcp__") {
        return Some((rest.to_string(), rest.to_string()));
    }
    for s in known_servers {
        let norm_s = s.replace('-', "_");
        for sep in ['_', '-'] {
            let prefix1 = format!("{}{}", s, sep);
            if tool_name.starts_with(&prefix1) && tool_name.len() > prefix1.len() {
                return Some((s.clone(), tool_name[prefix1.len()..].to_string()));
            }
            let prefix2 = format!("{}{}", norm_s, sep);
            if tool_name.starts_with(&prefix2) && tool_name.len() > prefix2.len() {
                return Some((norm_s.clone(), tool_name[prefix2.len()..].to_string()));
            }
        }
    }
    // General fallback for known MCP tool naming conventions
    if tool_name.contains("kb_") || tool_name.starts_with("knowledge_base") {
        return Some(("knowledge_base".to_string(), tool_name.to_string()));
    }
    None
}

/// Parse MCP telemetry directly from session JSONL content.
pub fn parse_mcp_session(path: &Path) -> Option<McpTelemetry> {
    let content = fs::read_to_string(path).ok()?;
    let configured_servers = load_configured_servers();

    let mut t = McpTelemetry::default();
    let mut pending_calls: std::collections::HashMap<String, McpCallItem> =
        std::collections::HashMap::new();

    for line in content.lines() {
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if entry.get("type").and_then(|v| v.as_str()) != Some("message") {
            continue;
        }
        let Some(msg) = entry.get("message") else {
            continue;
        };
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
        let ts_str = entry
            .get("timestamp")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let ts_ms = crate::slices::telemetry::skills_live::iso_to_epoch_ms(ts_str).unwrap_or(0);

        if role == "assistant" {
            if let Some(serde_json::Value::Array(blocks)) = msg.get("content") {
                for b in blocks {
                    if b.get("type").and_then(|v| v.as_str()) == Some("toolCall") {
                        let call_id = b.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let tool_name = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        if let Some((server, tool)) =
                            classify_mcp_tool(tool_name, &configured_servers)
                        {
                            let badge = badge_for(&server).to_string();
                            let summary = b
                                .get("arguments")
                                .map(|args| {
                                    if let Some(q) = args.get("query").and_then(|v| v.as_str()) {
                                        format!("\"{}\"", q)
                                    } else if let Some(p) =
                                        args.get("path").and_then(|v| v.as_str())
                                    {
                                        p.to_string()
                                    } else {
                                        String::new()
                                    }
                                })
                                .unwrap_or_default();

                            let item = McpCallItem {
                                server: server.clone(),
                                badge,
                                tool,
                                is_error: false,
                                timestamp_ms: ts_ms,
                                summary,
                            };
                            if !t.servers_used.contains(&server) {
                                t.servers_used.push(server);
                            }
                            t.total_calls += 1;
                            t.last_call_ts_ms = ts_ms;
                            if !call_id.is_empty() {
                                pending_calls.insert(call_id.to_string(), item.clone());
                            }
                            t.recent_calls.push(item);
                        }
                    }
                }
            }
        } else if role == "toolResult" {
            let tool_id = msg.get("toolCallId").and_then(|v| v.as_str()).unwrap_or("");
            let is_err = msg
                .get("isError")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if let Some(mut item) = pending_calls.remove(tool_id) {
                if is_err {
                    t.total_errors += 1;
                    item.is_error = true;
                    if let Some(last) = t
                        .recent_calls
                        .iter_mut()
                        .rev()
                        .find(|c| c.tool == item.tool && c.server == item.server)
                    {
                        last.is_error = true;
                    }
                }
            }
        }
    }

    t.active_calls_count = pending_calls.len() as u64;
    t.in_flight = !pending_calls.is_empty();
    if t.recent_calls.len() > 20 {
        t.recent_calls = t.recent_calls.split_off(t.recent_calls.len() - 20);
    }

    Some(t)
}
