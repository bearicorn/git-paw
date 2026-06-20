//! Conflict reads.
//!
//! Conflicts are not persisted in a queryable endpoint either. Rather than
//! parse the conflict-detector's human-readable feedback text, we
//! reconstruct the set of currently-active intents from the broker `/log`
//! and re-run the real detection logic ([`crate::broker::conflict`]) over
//! them. This yields the same structured forward-overlap data the live
//! supervisor sees, and degrades to an empty list when no broker is up.

use std::time::{Duration, Instant};

use rmcp::schemars;
use serde::Serialize;

use crate::broker::conflict::{ConflictTracker, NormalizedFileIntent};
use crate::broker::messages::{BrokerMessage, Region};
use crate::broker::publish::fetch_log_entries_over_http;
use crate::mcp::RepoContext;

use super::now_unix;

/// One detected conflict in the shape `get_conflicts` returns.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct Conflict {
    /// Conflict shape (currently always `"forward"` — intent vs intent).
    pub shape: String,
    /// The two branch ids in conflict, sorted.
    pub branches: [String; 2],
    /// The conflicting file paths.
    pub files: Vec<String>,
    /// Unix seconds when the conflict was detected (now — detection is live).
    pub detected_at: u64,
}

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

/// Returns every active forward conflict between the repository session's
/// agents, or an empty list when no broker is reachable.
#[must_use]
pub fn conflicts(ctx: &RepoContext) -> Vec<Conflict> {
    let Some(url) = ctx.broker_url.as_deref() else {
        return Vec::new();
    };
    let Ok(entries) = fetch_log_entries_over_http(url) else {
        return Vec::new();
    };

    // Reconstruct the latest intent per agent into a tracker. We pre-filter to
    // unexpired intents and insert them with a generous TTL so the detector's
    // own expiry never drops them mid-reconstruction.
    let now_secs = now_unix();
    let now_instant = Instant::now();
    let ttl = Duration::from_hours(1);

    let mut latest: std::collections::HashMap<
        String,
        (Vec<NormalizedFileIntent>, String, u64, u64),
    > = std::collections::HashMap::new();
    for entry in entries {
        if let BrokerMessage::Intent { agent_id, payload } = entry.message {
            let files: Vec<NormalizedFileIntent> = payload
                .files
                .iter()
                .cloned()
                .map(NormalizedFileIntent::from)
                .collect();
            latest.insert(
                agent_id,
                (
                    files,
                    payload.summary,
                    entry.timestamp_unix_secs,
                    payload.valid_for_seconds,
                ),
            );
        }
    }

    let mut tracker = ConflictTracker::new();
    let mut agents: Vec<String> = Vec::new();
    for (agent_id, (files, summary, published_at, valid_for)) in latest {
        if published_at.saturating_add(valid_for) <= now_secs {
            continue; // expired — exclude from detection
        }
        agents.push(agent_id.clone());
        tracker.insert_intent(&agent_id, files, summary, ttl, now_instant);
    }
    agents.sort();

    // Collect forward overlaps for every agent, deduping the symmetric pair.
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    let mut out = Vec::new();
    for agent in &agents {
        for fc in tracker.forward_overlaps(agent) {
            let mut pair = [agent.clone(), fc.other_agent.clone()];
            pair.sort();
            let key = (pair[0].clone(), pair[1].clone());
            if !seen.insert(key) {
                continue;
            }
            let mut files: Vec<String> = fc
                .files
                .iter()
                .map(|f| {
                    if f.regions.is_empty() {
                        f.path.clone()
                    } else {
                        let regions: Vec<String> = f.regions.iter().map(region_label).collect();
                        format!("{} [{}]", f.path, regions.join(", "))
                    }
                })
                .collect();
            files.sort();
            out.push(Conflict {
                shape: "forward".to_string(),
                branches: pair,
                files,
                detected_at: now_secs,
            });
        }
    }
    out.sort_by(|a, b| a.branches.cmp(&b.branches));
    out
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
        };
        assert!(conflicts(&ctx).is_empty());
    }
}
