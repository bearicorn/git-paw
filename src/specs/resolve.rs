//! Spec name resolution for the `--specs NAME[,NAME...]` narrow flag.
//!
//! Maps user-supplied names against the discovered `SpecEntry` set returned
//! by `scan_specs()` using a layered matching strategy:
//!
//! 1. **Exact match** on `SpecEntry.id`.
//! 2. **Spec Kit feature-name match** — every `SpecEntry.id` whose feature
//!    prefix equals the requested name.
//! 3. **Spec Kit numeric prefix match** — digit-leading values match a unique
//!    feature directory whose name starts with `<digits>` followed by a
//!    non-digit boundary. Ambiguous matches are rejected.
//!
//! Resolution fails (no partial success) when any requested name is unknown
//! or matches more than one feature ambiguously.

use crate::error::PawError;
use crate::specs::SpecEntry;

/// Resolves the `--specs NAME[,NAME...]` values against the discovered set.
///
/// Returns the union of entries that match any of the supplied names. Order
/// follows the discovered-set order; duplicates (same `SpecEntry.id`) are
/// retained only on first appearance.
///
/// # Errors
///
/// Returns `PawError::SpecError` when:
/// - Any requested name does not match an exact id, a feature, or an
///   unambiguous numeric prefix.
/// - A numeric prefix matches more than one feature.
///
/// The error message lists the unresolved or ambiguous names AND the
/// discovered-set identifier list so the user can correct quickly.
pub fn resolve_specs(entries: &[SpecEntry], names: &[String]) -> Result<Vec<SpecEntry>, PawError> {
    let mut unknown: Vec<String> = Vec::new();
    let mut ambiguous: Vec<(String, Vec<String>)> = Vec::new();
    let mut selected_indices: Vec<usize> = Vec::new();

    for name in names {
        match match_name(entries, name) {
            MatchResult::Indices(idxs) => {
                for idx in idxs {
                    if !selected_indices.contains(&idx) {
                        selected_indices.push(idx);
                    }
                }
            }
            MatchResult::Unknown => unknown.push(name.clone()),
            MatchResult::Ambiguous(features) => ambiguous.push((name.clone(), features)),
        }
    }

    if let Some((prefix, candidates)) = ambiguous.first() {
        return Err(PawError::SpecError(format!(
            "spec name '{prefix}' is ambiguous; matches: {}\n  \
             Run `git paw start --specs <full-name>` to disambiguate.",
            candidates.join(", ")
        )));
    }

    if !unknown.is_empty() {
        let discovered: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();
        return Err(PawError::SpecError(format!(
            "spec(s) not found: {}\n  \
             Discovered specs: {}\n  \
             Run `git paw start --specs` for an interactive picker.",
            unknown.join(", "),
            discovered.join(", ")
        )));
    }

    Ok(selected_indices
        .into_iter()
        .map(|i| entries[i].clone())
        .collect())
}

enum MatchResult {
    Indices(Vec<usize>),
    Unknown,
    Ambiguous(Vec<String>),
}

fn match_name(entries: &[SpecEntry], name: &str) -> MatchResult {
    if let Some(idx) = entries.iter().position(|e| e.id == name) {
        return MatchResult::Indices(vec![idx]);
    }

    // Pure-numeric names are reserved for the prefix-match path; they must
    // not collide with the feature-name match below (otherwise "003" would
    // greedily match every "003-*" entry without surfacing ambiguity across
    // multiple "003*-" features).
    if !is_numeric_prefix(name) {
        let feature_matches: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| is_feature_match(&e.id, name))
            .map(|(i, _)| i)
            .collect();
        if !feature_matches.is_empty() {
            return MatchResult::Indices(feature_matches);
        }
        return MatchResult::Unknown;
    }

    let features = collect_feature_ids_with_prefix(entries, name);
    match features.len() {
        0 => MatchResult::Unknown,
        1 => {
            let feature = &features[0];
            let idxs: Vec<usize> = entries
                .iter()
                .enumerate()
                .filter(|(_, e)| is_feature_match(&e.id, feature))
                .map(|(i, _)| i)
                .collect();
            if idxs.is_empty() {
                MatchResult::Unknown
            } else {
                MatchResult::Indices(idxs)
            }
        }
        _ => MatchResult::Ambiguous(features),
    }
}

/// True when `id` belongs to feature `feature`: either `id == feature`, or
/// `id` is `feature` followed by `-<decomposition-suffix>`.
fn is_feature_match(id: &str, feature: &str) -> bool {
    if id == feature {
        return true;
    }
    id.strip_prefix(feature)
        .is_some_and(|rest| rest.starts_with('-'))
}

/// True when `name` is a non-empty digits-only string (e.g. `003`).
fn is_numeric_prefix(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_digit())
}

/// Returns the deduplicated feature ids whose feature-id begins with
/// `prefix` followed by a non-digit boundary (so `003` does not match
/// `0034-…`).
fn collect_feature_ids_with_prefix(entries: &[SpecEntry], prefix: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for entry in entries {
        let feature = feature_id_of(&entry.id);
        let Some(rest) = feature.strip_prefix(prefix) else {
            continue;
        };
        let bounded = rest.chars().next().is_none_or(|c| !c.is_ascii_digit());
        if bounded && !out.contains(&feature) {
            out.push(feature);
        }
    }
    out
}

/// Extracts the feature id (the leading `<digits>-<slug>` portion) from a
/// Spec Kit entry id. Falls back to the full id when the shape doesn't
/// match the expected decomposition (`<feature>-T<digits>` or
/// `<feature>-phase-<digits>`).
///
/// Examples:
/// - `003-user-list-T009` → `003-user-list`
/// - `003-user-list-phase-2` → `003-user-list`
/// - `add-auth` → `add-auth`
/// - `003a-experiment-T001` → `003a-experiment`
fn feature_id_of(id: &str) -> String {
    if let Some((before, after)) = id.rsplit_once("-phase-")
        && !after.is_empty()
        && after.chars().all(|c| c.is_ascii_digit())
    {
        return before.to_string();
    }
    if let Some((before, after)) = id.rsplit_once("-T")
        && !after.is_empty()
        && after.chars().all(|c| c.is_ascii_digit())
    {
        return before.to_string();
    }
    id.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str) -> SpecEntry {
        SpecEntry {
            id: id.to_string(),
            backend: crate::specs::SpecBackendKind::Markdown,
            branch: format!("spec/{id}"),
            cli: None,
            prompt: String::new(),
            owned_files: None,
        }
    }

    #[test]
    fn exact_match_returns_single_entry() {
        let entries = vec![entry("add-auth"), entry("fix-session")];
        let out = resolve_specs(&entries, &["add-auth".to_string()]).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "add-auth");
    }

    #[test]
    fn exact_match_on_spec_kit_decomposed_id() {
        let entries = vec![
            entry("003-user-list-T009"),
            entry("003-user-list-T010"),
            entry("003-user-list-phase-2"),
        ];
        let out = resolve_specs(&entries, &["003-user-list-T009".to_string()]).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "003-user-list-T009");
    }

    #[test]
    fn feature_name_expands_to_all_decomposed_entries() {
        let entries = vec![
            entry("003-user-list-T009"),
            entry("003-user-list-T010"),
            entry("003-user-list-phase-2"),
            entry("004-error-handling-phase-1"),
        ];
        let out = resolve_specs(&entries, &["003-user-list".to_string()]).unwrap();
        let ids: Vec<&str> = out.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "003-user-list-T009",
                "003-user-list-T010",
                "003-user-list-phase-2",
            ]
        );
    }

    #[test]
    fn numeric_prefix_resolves_unambiguously() {
        let entries = vec![
            entry("003-user-list-T009"),
            entry("003-user-list-T010"),
            entry("003-user-list-phase-2"),
        ];
        let out = resolve_specs(&entries, &["003".to_string()]).unwrap();
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn ambiguous_numeric_prefix_errors_with_candidates() {
        let entries = vec![entry("003-user-list-T009"), entry("003a-experiment-T001")];
        let err = resolve_specs(&entries, &["003".to_string()]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("ambiguous"), "got: {msg}");
        assert!(msg.contains("003-user-list"), "got: {msg}");
        assert!(msg.contains("003a-experiment"), "got: {msg}");
    }

    #[test]
    fn numeric_prefix_with_no_features_errors_as_unknown() {
        let entries = vec![entry("add-auth")];
        let err = resolve_specs(&entries, &["003".to_string()]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not found"), "got: {msg}");
    }

    #[test]
    fn unknown_name_lists_candidates() {
        let entries = vec![entry("add-auth"), entry("fix-session")];
        let err = resolve_specs(&entries, &["no-such-spec".to_string()]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not found"), "got: {msg}");
        assert!(msg.contains("no-such-spec"), "got: {msg}");
        assert!(msg.contains("add-auth"), "got: {msg}");
        assert!(msg.contains("fix-session"), "got: {msg}");
    }

    #[test]
    fn partial_failure_aborts_no_partial_result() {
        let entries = vec![entry("add-auth"), entry("fix-session")];
        let err = resolve_specs(
            &entries,
            &["add-auth".to_string(), "no-such-spec".to_string()],
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("no-such-spec"), "got: {msg}");
    }

    #[test]
    fn multiple_names_resolved_independently() {
        let entries = vec![
            entry("add-auth"),
            entry("fix-session"),
            entry("add-logging"),
        ];
        let out = resolve_specs(
            &entries,
            &["add-auth".to_string(), "add-logging".to_string()],
        )
        .unwrap();
        let ids: Vec<&str> = out.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["add-auth", "add-logging"]);
    }

    #[test]
    fn duplicate_names_are_deduplicated() {
        let entries = vec![entry("add-auth"), entry("fix-session")];
        let out =
            resolve_specs(&entries, &["add-auth".to_string(), "add-auth".to_string()]).unwrap();
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn feature_id_of_handles_t_task_suffix() {
        assert_eq!(feature_id_of("003-user-list-T009"), "003-user-list");
    }

    #[test]
    fn feature_id_of_handles_phase_suffix() {
        assert_eq!(feature_id_of("003-user-list-phase-2"), "003-user-list");
    }

    #[test]
    fn feature_id_of_handles_openspec_flat_id() {
        assert_eq!(feature_id_of("add-auth"), "add-auth");
    }

    #[test]
    fn feature_id_of_handles_alphanumeric_feature_directory() {
        assert_eq!(feature_id_of("003a-experiment-T001"), "003a-experiment");
    }
}
