//! Broker-helper allowlist setup for the supervisor.
//!
//! Implements the `curl-allowlist` capability: write a least-privilege,
//! path-based allowlist into `.claude/settings.json` so the coding agents
//! do not hit a permission prompt when they invoke the bundled
//! agent-broker helper (`agent-broker-helper`) on every broker round-trip.
//!
//! The seeded grant is the single, stable path of `.git-paw/scripts/broker.sh`
//! (both the bare path and the `bash .git-paw/scripts/broker.sh` form the
//! boot block may emit) — NOT per-endpoint `curl <broker-url><endpoint>`
//! prefixes and never a broad `curl *` rule. A single literal path cannot
//! drift with URL normalisation or curl flag order, which was the root cause
//! of the boot-publish dead-stall.
//!
//! The allowlist is merged with any existing `allowed_bash_prefixes`
//! field — entries the user added (including stale per-endpoint curl
//! prefixes from older versions) are preserved and harmless.

use std::path::Path;

use crate::error::PawError;

/// Stable relative path of the bundled agent-broker helper, installed by
/// `git paw init` at `<repo>/.git-paw/scripts/broker.sh`.
pub const BROKER_HELPER_PATH: &str = ".git-paw/scripts/broker.sh";

/// Returns the least-privilege allowlist prefixes authorising the bundled
/// agent-broker helper.
///
/// Both the bare path (`.git-paw/scripts/broker.sh`) and the
/// `bash .git-paw/scripts/broker.sh` form the boot block may emit are
/// returned so the match is exact regardless of how the agent invokes the
/// script. The grant is independent of the broker URL — it authorises
/// exactly one script, not a host or all of `curl`.
#[must_use]
pub fn broker_prefixes() -> Vec<String> {
    vec![
        BROKER_HELPER_PATH.to_string(),
        format!("bash {BROKER_HELPER_PATH}"),
    ]
}

/// Merges the broker-helper allowlist into `.claude/settings.json`.
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
pub fn setup_curl_allowlist(settings_path: &Path) -> Result<(), PawError> {
    let new_entries = broker_prefixes();

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
        setup_curl_allowlist(&path).unwrap();
        let entries = read_array(&path);
        assert!(
            entries.iter().any(|s| s == ".git-paw/scripts/broker.sh"),
            "must grant the bare helper path; got: {entries:?}"
        );
        assert!(
            entries
                .iter()
                .any(|s| s == "bash .git-paw/scripts/broker.sh"),
            "must grant the `bash <helper>` form; got: {entries:?}"
        );
    }

    /// The seeded grant must never authorise broad `curl`, and must never
    /// fall back to per-endpoint `curl <broker-url><endpoint>` prefixes.
    #[test]
    fn grants_helper_path_not_broad_curl() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        setup_curl_allowlist(&path).unwrap();
        let entries = read_array(&path);
        assert!(
            entries.iter().any(|s| s == ".git-paw/scripts/broker.sh"),
            "helper-path grant missing: {entries:?}"
        );
        for e in &entries {
            assert_ne!(e, "curl *", "broad `curl *` grant must never be seeded");
            assert!(
                !e.starts_with("curl "),
                "no `curl` prefix should be seeded; found `{e}`"
            );
        }
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
        setup_curl_allowlist(&path).unwrap();
        let entries = read_array(&path);
        assert!(
            entries.iter().any(|s| s == "just check"),
            "must preserve existing entries"
        );
        assert!(entries.iter().any(|s| s == "cargo build"));
        assert!(entries.iter().any(|s| s == ".git-paw/scripts/broker.sh"));
    }

    /// A pre-existing per-endpoint `curl` prefix from an older version is
    /// preserved (harmless) — re-seeding only appends the helper-path grant.
    #[test]
    fn preserves_stale_curl_prefix_and_adds_helper_path() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{"allowed_bash_prefixes":["curl -s http://127.0.0.1:9119/publish"]}"#,
        )
        .unwrap();
        setup_curl_allowlist(&path).unwrap();
        let entries = read_array(&path);
        assert!(
            entries
                .iter()
                .any(|s| s == "curl -s http://127.0.0.1:9119/publish"),
            "stale prefix must be preserved (harmless)"
        );
        assert!(entries.iter().any(|s| s == ".git-paw/scripts/broker.sh"));
    }

    #[test]
    fn does_not_duplicate_existing_helper_grant() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{"allowed_bash_prefixes":[".git-paw/scripts/broker.sh"]}"#,
        )
        .unwrap();
        setup_curl_allowlist(&path).unwrap();
        let entries = read_array(&path);
        let count = entries
            .iter()
            .filter(|s| *s == ".git-paw/scripts/broker.sh")
            .count();
        assert_eq!(count, 1, "no duplicates allowed");
    }

    /// Re-seeding is idempotent: a second pass produces identical content.
    #[test]
    fn re_seeding_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        setup_curl_allowlist(&path).unwrap();
        let first = std::fs::read_to_string(&path).unwrap();
        setup_curl_allowlist(&path).unwrap();
        let second = std::fs::read_to_string(&path).unwrap();
        assert_eq!(first, second, "re-seeding must be a no-op");
    }

    #[test]
    fn invalid_json_returns_error_not_panic() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        std::fs::write(&path, "not json {{{ broken").unwrap();
        let err = setup_curl_allowlist(&path).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("invalid JSON"), "got: {msg}");
    }

    #[test]
    fn creates_parent_directory_when_missing() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".claude").join("settings.json");
        // Parent directory does not yet exist.
        assert!(!path.parent().unwrap().exists());
        setup_curl_allowlist(&path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn rejects_top_level_array() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        std::fs::write(&path, "[]").unwrap();
        let err = setup_curl_allowlist(&path).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("must be a JSON object"), "got: {msg}");
    }
}
