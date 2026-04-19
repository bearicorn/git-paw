//! Curl allowlist setup for the supervisor.
//!
//! Implements the `curl-allowlist` capability of the
//! `auto-approve-patterns` change: write a curl-prefix allowlist into
//! `.claude/settings.json` so the supervisor and coding agents do not
//! hit a permission prompt on every broker round-trip.
//!
//! The allowlist is merged with any existing `allowed_bash_prefixes`
//! field — entries the user added are preserved.

use std::path::Path;

use crate::error::PawError;

/// Broker endpoints the supervisor and coding agents call most often.
///
/// The list is intentionally narrow — adding a new endpoint requires a
/// code change so the allowlist cannot drift silently.
pub const BROKER_ENDPOINTS: &[&str] = &["/publish", "/status", "/poll", "/feedback"];

/// Returns the prefixes that auto-approval would whitelist for `broker_url`.
///
/// Each entry is a `curl -s <url><endpoint>` string suitable for matching
/// the agent CLI's "allowed bash prefixes" rule. Both `curl -s ...` and
/// `curl ...` are emitted so agents that omit `-s` still bypass the
/// prompt.
#[must_use]
pub fn broker_prefixes(broker_url: &str) -> Vec<String> {
    let url = broker_url.trim_end_matches('/');
    let mut out = Vec::with_capacity(BROKER_ENDPOINTS.len() * 2);
    for endpoint in BROKER_ENDPOINTS {
        out.push(format!("curl -s {url}{endpoint}"));
        out.push(format!("curl {url}{endpoint}"));
    }
    out
}

/// Merges the broker allowlist into `.claude/settings.json`.
///
/// Behaviour:
///
/// - When `settings_path` does not exist, a new JSON object is created
///   with `allowed_bash_prefixes` set to [`broker_prefixes`].
/// - When the file exists and contains valid JSON, the existing
///   `allowed_bash_prefixes` array is preserved and missing entries are
///   appended.
/// - When the file exists but is not valid JSON, an error is returned.
///   The function never panics.
pub fn setup_curl_allowlist(broker_url: &str, settings_path: &Path) -> Result<(), PawError> {
    let new_entries = broker_prefixes(broker_url);

    // Load existing JSON or start from a fresh object.
    let mut value: serde_json::Value = if settings_path.exists() {
        let raw = std::fs::read_to_string(settings_path).map_err(|e| {
            PawError::ConfigError(format!("failed to read {}: {e}", settings_path.display()))
        })?;
        if raw.trim().is_empty() {
            serde_json::Value::Object(serde_json::Map::new())
        } else {
            serde_json::from_str(&raw).map_err(|e| {
                PawError::ConfigError(format!("{}: invalid JSON: {e}", settings_path.display()))
            })?
        }
    } else {
        serde_json::Value::Object(serde_json::Map::new())
    };

    let obj = value.as_object_mut().ok_or_else(|| {
        PawError::ConfigError(format!(
            "{}: top-level value must be a JSON object",
            settings_path.display()
        ))
    })?;

    let entry = obj
        .entry("allowed_bash_prefixes".to_string())
        .or_insert_with(|| serde_json::Value::Array(Vec::new()));

    let array = entry.as_array_mut().ok_or_else(|| {
        PawError::ConfigError(format!(
            "{}: allowed_bash_prefixes must be an array",
            settings_path.display()
        ))
    })?;

    // Preserve existing entries; append any missing ones.
    for new_entry in new_entries {
        let already_present = array
            .iter()
            .any(|v| v.as_str().is_some_and(|s| s == new_entry));
        if !already_present {
            array.push(serde_json::Value::String(new_entry));
        }
    }

    if let Some(parent) = settings_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| {
            PawError::ConfigError(format!("failed to create {}: {e}", parent.display()))
        })?;
    }

    let serialized = serde_json::to_string_pretty(&value).map_err(|e| {
        PawError::ConfigError(format!(
            "failed to serialize {}: {e}",
            settings_path.display()
        ))
    })?;
    std::fs::write(settings_path, serialized).map_err(|e| {
        PawError::ConfigError(format!("failed to write {}: {e}", settings_path.display()))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn read_array(path: &Path) -> Vec<String> {
        let raw = std::fs::read_to_string(path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        v.get("allowed_bash_prefixes")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn writes_fresh_settings_when_file_absent() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        setup_curl_allowlist("http://127.0.0.1:9119", &path).unwrap();
        let entries = read_array(&path);
        assert!(
            entries
                .iter()
                .any(|s| s == "curl -s http://127.0.0.1:9119/publish")
        );
        assert!(
            entries
                .iter()
                .any(|s| s == "curl -s http://127.0.0.1:9119/status")
        );
        assert!(
            entries
                .iter()
                .any(|s| s == "curl -s http://127.0.0.1:9119/poll")
        );
        assert!(
            entries
                .iter()
                .any(|s| s == "curl -s http://127.0.0.1:9119/feedback")
        );
    }

    #[test]
    fn merges_with_existing_entries() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{"allowed_bash_prefixes":["just check","cargo build"]}"#,
        )
        .unwrap();
        setup_curl_allowlist("http://127.0.0.1:9119", &path).unwrap();
        let entries = read_array(&path);
        assert!(
            entries.iter().any(|s| s == "just check"),
            "must preserve existing entries"
        );
        assert!(entries.iter().any(|s| s == "cargo build"));
        assert!(
            entries
                .iter()
                .any(|s| s == "curl -s http://127.0.0.1:9119/publish")
        );
    }

    #[test]
    fn does_not_duplicate_existing_broker_entries() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{"allowed_bash_prefixes":["curl -s http://127.0.0.1:9119/publish"]}"#,
        )
        .unwrap();
        setup_curl_allowlist("http://127.0.0.1:9119", &path).unwrap();
        let entries = read_array(&path);
        let count = entries
            .iter()
            .filter(|s| *s == "curl -s http://127.0.0.1:9119/publish")
            .count();
        assert_eq!(count, 1, "no duplicates allowed");
    }

    #[test]
    fn invalid_json_returns_error_not_panic() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        std::fs::write(&path, "not json {{{ broken").unwrap();
        let err = setup_curl_allowlist("http://127.0.0.1:9119", &path).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("invalid JSON"), "got: {msg}");
    }

    #[test]
    fn updates_when_broker_url_changes() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        setup_curl_allowlist("http://127.0.0.1:9119", &path).unwrap();
        // Re-invoke with a different URL — both URLs should be present.
        setup_curl_allowlist("http://127.0.0.1:9120", &path).unwrap();
        let entries = read_array(&path);
        assert!(
            entries
                .iter()
                .any(|s| s == "curl -s http://127.0.0.1:9119/publish")
        );
        assert!(
            entries
                .iter()
                .any(|s| s == "curl -s http://127.0.0.1:9120/publish")
        );
    }

    #[test]
    fn includes_feedback_endpoint() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        setup_curl_allowlist("http://127.0.0.1:9119", &path).unwrap();
        let entries = read_array(&path);
        assert!(
            entries.iter().any(|s| s.contains("/feedback")),
            "feedback endpoint missing: {entries:?}"
        );
    }

    #[test]
    fn creates_parent_directory_when_missing() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".claude").join("settings.json");
        // Parent directory does not yet exist.
        assert!(!path.parent().unwrap().exists());
        setup_curl_allowlist("http://127.0.0.1:9119", &path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn rejects_top_level_array() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        std::fs::write(&path, "[]").unwrap();
        let err = setup_curl_allowlist("http://127.0.0.1:9119", &path).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("must be a JSON object"), "got: {msg}");
    }
}
