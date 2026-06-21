//! Active-intent reads, reconstructed from the broker's `/log`.
//!
//! Intents are not stored in a dedicated queryable endpoint; they flow as
//! `agent.intent` broadcast messages. We fetch the full message log over
//! HTTP, keep the latest intent per agent, and report those whose TTL has
//! not yet expired. When no broker is reachable we return an empty list.

use rmcp::schemars;
use serde::Serialize;

use crate::broker::messages::{BrokerMessage, FileIntent, Region};
use crate::broker::publish::fetch_log_entries_over_http;
use crate::mcp::RepoContext;

use super::now_unix;

/// One active intent, in the shape the `get_intents` tool returns.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema, PartialEq, Eq)]
pub struct Intent {
    /// Publishing agent's branch id.
    pub branch_id: String,
    /// Declared files (paths only; region detail is omitted at this layer).
    pub files: Vec<String>,
    /// Per-file declared regions, parallel to `files` (empty when none).
    pub regions: Vec<Vec<String>>,
    /// One-line human summary.
    pub summary: String,
    /// Unix seconds when the intent was published.
    pub published_at: u64,
    /// Declared TTL in seconds.
    pub valid_for_seconds: u64,
}

/// Renders a region to a short human label for inclusion in tool output.
fn region_label(region: &Region) -> String {
    match region {
        Region::Function { name } => format!("fn:{name}"),
        Region::Class { name } => format!("class:{name}"),
        Region::Block { anchor } => format!("block:{anchor}"),
        Region::Range {
            start_line,
            end_line,
        } => format!("lines:{start_line}-{end_line}"),
    }
}

/// Returns every active (non-expired) intent known to the broker for this
/// repository's session, or an empty vector when no broker is reachable.
#[must_use]
pub fn active_intents(ctx: &RepoContext) -> Vec<Intent> {
    let Some(url) = ctx.broker_url.as_deref() else {
        return Vec::new();
    };
    let Ok(entries) = fetch_log_entries_over_http(url) else {
        return Vec::new();
    };

    let now = now_unix();
    // Keep the latest intent per agent (entries are chronological).
    let mut latest: std::collections::HashMap<String, Intent> = std::collections::HashMap::new();
    for entry in entries {
        if let BrokerMessage::Intent { agent_id, payload } = entry.message {
            let mut files = Vec::with_capacity(payload.files.len());
            let mut regions = Vec::with_capacity(payload.files.len());
            for fi in &payload.files {
                files.push(fi.path().to_string());
                let labels = match fi {
                    FileIntent::Detailed { regions, .. } => {
                        regions.iter().map(region_label).collect()
                    }
                    FileIntent::Path(_) => Vec::new(),
                };
                regions.push(labels);
            }
            latest.insert(
                agent_id.clone(),
                Intent {
                    branch_id: agent_id,
                    files,
                    regions,
                    summary: payload.summary,
                    published_at: entry.timestamp_unix_secs,
                    valid_for_seconds: payload.valid_for_seconds,
                },
            );
        }
    }

    let mut out: Vec<Intent> = latest
        .into_values()
        .filter(|i| i.published_at.saturating_add(i.valid_for_seconds) > now)
        .collect();
    out.sort_by(|a, b| a.branch_id.cmp(&b.branch_id));
    out
}

/// Returns the active intent for a single branch id, or `None`.
#[must_use]
pub fn intent_for(ctx: &RepoContext, branch_id: &str) -> Option<Intent> {
    active_intents(ctx)
        .into_iter()
        .find(|i| i.branch_id == branch_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_broker_yields_empty() {
        let ctx = RepoContext {
            root: std::path::PathBuf::from("/tmp"),
            git_paw_dir: None,
            broker_url: None,
            server_name: "git-paw".to_string(),
        };
        assert!(active_intents(&ctx).is_empty());
        assert!(intent_for(&ctx, "feat-x").is_none());
    }

    #[test]
    fn region_labels_render() {
        assert_eq!(region_label(&Region::Function { name: "f".into() }), "fn:f");
        assert_eq!(
            region_label(&Region::Range {
                start_line: 1,
                end_line: 9
            }),
            "lines:1-9"
        );
    }
}
