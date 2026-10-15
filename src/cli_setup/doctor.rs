//! `git paw doctor`'s two read-only reports about per-CLI setup.
//!
//! - [`check_cli_setup`] reports what setup is applied and whether it is still
//!   intact: a recorded entry that went missing, applied entries that no
//!   longer match what this binary would plan, and — because a granted helper
//!   runs without a prompt — a helper script that differs from the copy the
//!   binary bundles.
//! - [`check_broad_grants`] lists the durable allow rules already in the
//!   project's settings whose shape grants more than one specific command. It
//!   is a report over stored rules, not a decision about any prompt: git-paw
//!   does not edit these entries and does not act on them.
//!
//! Both are strictly read-only and neither ever reports ✗ — nothing here is a
//! reason for `doctor` to fail.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::doctor::{CheckResult, GROUP_CLIS};

use super::facts::{self, CliSetupFacts, HELPER_DIR, SetupItem};
use super::record::RecordEntry;

/// The state of one helper script on disk, against the bundled copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelperState {
    /// Where the helper lives.
    pub path: PathBuf,
    /// Whether its contents match the copy the binary bundles.
    pub matches_bundled: bool,
}

/// Everything the setup check reads.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetupProbe {
    /// Every entry in the setup record.
    pub entries: Vec<RecordEntry>,
    /// Contents of each file the record names, `None` when it is gone.
    pub files: BTreeMap<PathBuf, Option<String>>,
    /// Helper scripts in the main checkout and the current session's
    /// worktrees, checked against the bundled copies.
    pub helpers: Vec<HelperState>,
}

/// Everything the broad-grant advisory reads.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BroadGrantProbe {
    /// The allowlist target file and its contents, when it exists.
    pub target: Option<(PathBuf, String)>,
}

/// Reports the setup state for every identity with a fact row or record
/// entries.
///
/// Never returns ✗: an out-of-date or tampered setup is something to tell the
/// user about, not a reason for `doctor` to fail.
#[must_use]
pub fn check_cli_setup(probe: &SetupProbe) -> Vec<CheckResult> {
    let mut identities: Vec<&str> = facts::FACT_TABLE.iter().map(|r| r.identity).collect();
    for entry in &probe.entries {
        if !identities.contains(&entry.cli.as_str()) {
            identities.push(&entry.cli);
        }
    }

    identities
        .into_iter()
        .map(|identity| check_one(identity, probe))
        .collect()
}

/// Reports one identity's setup state.
fn check_one(identity: &str, probe: &SetupProbe) -> CheckResult {
    let name = format!("{identity} setup");
    let row = facts::row_for(identity);
    let recorded: Vec<&RecordEntry> = probe.entries.iter().filter(|e| e.cli == identity).collect();

    if recorded.is_empty() {
        return CheckResult::ok(GROUP_CLIS, name, available_items(row));
    }

    // A recorded entry that is gone from its file, or no longer equal to what
    // was recorded, means somebody edited it by hand.
    let stale: Vec<&&RecordEntry> = recorded
        .iter()
        .filter(|e| !entry_present(probe, e))
        .collect();
    if !stale.is_empty() {
        return CheckResult::warn(
            GROUP_CLIS,
            name,
            format!(
                "{} of {} recorded entr{} no longer present as recorded ({})",
                stale.len(),
                recorded.len(),
                if recorded.len() == 1 {
                    "y is"
                } else {
                    "ies are"
                },
                stale
                    .iter()
                    .map(|e| abbreviate(&e.value))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            format!(
                "run `git paw setup-cli {identity} --undo` to drop what git-paw still claims, \
                 then re-apply the item if you want it back"
            ),
        );
    }

    if let Some(warning) = check_against_current_plan(identity, &name, row, &recorded, probe) {
        return warning;
    }

    CheckResult::ok(
        GROUP_CLIS,
        name,
        format!(
            "{} recorded entr{} present and unchanged",
            recorded.len(),
            if recorded.len() == 1 { "y" } else { "ies" }
        ),
    )
}

/// Reports an applied setup that no longer matches what this binary would
/// write, or a granted helper that no longer matches the bundled copy.
fn check_against_current_plan(
    identity: &str,
    name: &str,
    row: Option<&CliSetupFacts>,
    recorded: &[&RecordEntry],
    probe: &SetupProbe,
) -> Option<CheckResult> {
    row?;

    let applied: Vec<&str> = recorded
        .iter()
        .filter(|e| e.is_for_item(SetupItem::Allowlist))
        .map(|e| e.value.as_str())
        .collect();
    if !applied.is_empty() {
        let mut wanted = facts::planned_allow_entries();
        wanted.extend(facts::planned_ask_entries());
        wanted.extend(facts::planned_deny_entries());
        let missing = wanted
            .iter()
            .filter(|w| !applied.contains(&w.as_str()))
            .count();
        if missing > 0 {
            return Some(CheckResult::warn(
                GROUP_CLIS,
                name.to_string(),
                format!(
                    "the applied allowlist is out of date — this version of git-paw would also \
                     write {missing} entr{}",
                    if missing == 1 { "y" } else { "ies" }
                ),
                format!(
                    "run `git paw setup-cli {identity} --undo`, then \
                     `git paw setup-cli {identity} --allowlist` to re-apply the current set"
                ),
            ));
        }
    }

    // A granted helper that differs from the bundled copy would run without a
    // prompt, which is exactly what the grant assumed it would not do.
    let modified: Vec<&HelperState> = probe
        .helpers
        .iter()
        .filter(|h| !h.matches_bundled)
        .collect();
    if modified.is_empty() {
        return None;
    }
    Some(CheckResult::warn(
        GROUP_CLIS,
        name.to_string(),
        format!(
            "a granted helper script differs from the copy this binary bundles: {}",
            modified
                .iter()
                .map(|h| h.path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        format!(
            "the allowlist lets that script run without a prompt — restore it with \
             `git paw init` (main checkout) or by restarting the session (a worktree copy), \
             or run `git paw setup-cli {identity} --undo`"
        ),
    ))
}

/// Describes the items an identity offers and whether they are verified.
fn available_items(row: Option<&CliSetupFacts>) -> String {
    let Some(row) = row else {
        return "no setup items are available".to_string();
    };
    let status = if row.verified {
        format!("verified against {}", row.verified_with)
    } else {
        "plan-only: the facts are unverified, so writing is disabled".to_string()
    };
    format!(
        "not applied — available: {} ({status})",
        row.items
            .iter()
            .map(|i| i.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// Returns whether a recorded entry is still in its file, exactly as
/// recorded.
fn entry_present(probe: &SetupProbe, entry: &RecordEntry) -> bool {
    let Some(Some(content)) = probe.files.get(&entry.file) else {
        return false;
    };
    match &entry.key_path {
        None => content.lines().any(|l| l.trim() == entry.value),
        Some(key_path) => {
            let Ok(root) = serde_json::from_str::<serde_json::Value>(content) else {
                return false;
            };
            let mut cursor = &root;
            for key in key_path.split('.') {
                match cursor.get(key) {
                    Some(next) => cursor = next,
                    None => return false,
                }
            }
            cursor
                .as_array()
                .is_some_and(|items| items.iter().any(|v| v.as_str() == Some(&entry.value)))
        }
    }
}

// ---------------------------------------------------------------------------
// The broad-grant advisory
// ---------------------------------------------------------------------------

/// Shells, interpreters and command wrappers: a prefix grant on one of these
/// grants whatever is passed to it.
const WRAPPERS: &[&str] = &[
    "sh", "bash", "zsh", "fish", "dash", "ksh", "python", "python3", "node", "deno", "bun", "ruby",
    "perl", "php", "lua", "env", "xargs", "timeout", "nohup", "nice", "sudo", "su", "doas", "eval",
    "exec", "ssh",
];

/// Git subcommands that publish, rewrite or discard history or working-tree
/// state.
const RISKY_GIT: &[&str] = &[
    "push",
    "reset",
    "clean",
    "rebase",
    "branch",
    "checkout",
    "restore",
    "stash",
    "worktree",
    "update-ref",
    "reflog",
    "gc",
    "filter-branch",
    "config",
];

/// git-paw subcommands that change state.
const STATEFUL_GIT_PAW: &[&str] = &[
    "start",
    "add",
    "remove",
    "stop",
    "pause",
    "purge",
    "init",
    "add-cli",
    "remove-cli",
    "setup-cli",
];

/// Commands whose prefix grant reaches well beyond one specific invocation.
const REACHY: &[&str] = &["rm", "chmod", "chown", "dd", "curl", "wget"];

/// Lists the durable allow rules whose shape grants more than one specific
/// command.
///
/// Advisory only: git-paw does not edit these entries, and this report has no
/// bearing on how any permission prompt is handled.
#[must_use]
pub fn check_broad_grants(probe: &BroadGrantProbe) -> Vec<CheckResult> {
    let Some((path, content)) = &probe.target else {
        return Vec::new();
    };
    let name = "project CLI grants".to_string();

    let Ok(root) = serde_json::from_str::<serde_json::Value>(content) else {
        return vec![CheckResult::warn(
            GROUP_CLIS,
            name,
            format!("{} was not inspected: it is not valid JSON", path.display()),
            "fix the file if you want git-paw to report the broad permission grants in it"
                .to_string(),
        )];
    };

    let mut ours = facts::planned_allow_entries();
    ours.extend(facts::planned_ask_entries());
    ours.extend(facts::planned_deny_entries());

    let broad: Vec<String> = root
        .get("permissions")
        .and_then(|p| p.get("allow"))
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .filter(|e| !ours.iter().any(|o| o == *e))
                .filter(|e| is_broad(e))
                .map(abbreviate)
                .collect()
        })
        .unwrap_or_default();

    if broad.is_empty() {
        return vec![CheckResult::ok(
            GROUP_CLIS,
            name,
            format!("no broad allow rules in {}", path.display()),
        )];
    }
    vec![CheckResult::warn(
        GROUP_CLIS,
        name,
        format!(
            "{} grants more than one specific command: {}",
            path.display(),
            broad.join(", ")
        ),
        "git-paw does not edit these entries — review them and remove the ones you did not \
         intend to grant"
            .to_string(),
    )]
}

/// Splits a rule into its tool name and its specifier.
fn split_rule(rule: &str) -> (&str, Option<&str>) {
    match rule.split_once('(') {
        Some((tool, rest)) => (tool, Some(rest.strip_suffix(')').unwrap_or(rest))),
        None => (rule, None),
    }
}

/// Strips a trailing wildcard, returning the prefix it grants and whether a
/// wildcard was there at all.
fn strip_wildcard(spec: &str) -> Option<&str> {
    for suffix in [":*", " *", "*"] {
        if let Some(prefix) = spec.strip_suffix(suffix) {
            return Some(prefix.trim_end());
        }
    }
    None
}

/// Returns whether a rule's shape grants more than one specific command.
fn is_broad(rule: &str) -> bool {
    let (tool, spec) = split_rule(rule);

    // 1. no specifier, or a bare wildcard.
    let Some(spec) = spec else { return true };
    if matches!(spec.trim(), "" | "*" | ":*") {
        return true;
    }

    if matches!(tool, "Read" | "Edit" | "Write") {
        // 7. a file-tool rule anchored at the filesystem root or the home
        //    directory and ending in a wildcard.
        // `//path` is absolute, `~/path` is the home directory, `/path` is
        // relative to the settings source — all three are anchors a wildcard
        // can sweep a whole tree from.
        let anchored = spec.starts_with('/') || spec.starts_with('~');
        return anchored && spec.ends_with('*');
    }

    let Some(prefix) = strip_wildcard(spec) else {
        return false;
    };
    let words: Vec<&str> = prefix.split_whitespace().collect();
    let Some(first) = words.first().copied() else {
        return true;
    };

    // 8. a grant naming a helper script that is broader than the setup
    //    item's own entry for it: a whole-script wildcard where the grant
    //    table grants only named subcommands. A whole-script grant for a
    //    helper the table itself grants whole is not broader than git-paw's
    //    own entry, so it is not reported.
    if first.starts_with(HELPER_DIR) {
        if words.len() > 1 {
            return false;
        }
        let name = first.rsplit('/').next().unwrap_or(first);
        return !matches!(
            facts::GRANT_TABLE
                .iter()
                .find(|g| g.script == name)
                .map(|g| g.scope),
            Some(facts::GrantScope::WholeScript)
        );
    }

    // 2. a single-word prefix grant.
    if words.len() == 1 {
        return true;
    }

    // 3. a shell, interpreter or command wrapper.
    if WRAPPERS.contains(&first) {
        return true;
    }

    // 6. a far-reaching command, except a loopback curl.
    if REACHY.contains(&first) {
        let loopback = first == "curl"
            && (spec.contains("127.0.0.1") || spec.contains("localhost") || spec.contains("[::1]"));
        return !loopback;
    }

    let second = words.get(1).copied().unwrap_or_default();

    // 4. a git subcommand that publishes, rewrites or discards state.
    if first == "git" {
        if second == "paw" {
            // 5. git-paw through its git subcommand form.
            let third = words.get(2).copied().unwrap_or_default();
            return third.is_empty() || STATEFUL_GIT_PAW.contains(&third);
        }
        return RISKY_GIT.contains(&second);
    }

    // 5. git-paw invoked directly.
    if first == "git-paw" {
        return second.is_empty() || STATEFUL_GIT_PAW.contains(&second);
    }

    false
}

/// The longest word this report will print whole.
///
/// Chosen so the longest helper path the grant table can name
/// (`.git-paw/scripts/docs-fetch.sh`, 30 characters) survives intact — the
/// spec requires listing a wide helper grant verbatim — while an over-long
/// word, which is where a token embedded in a URL lives, is cut.
const MAX_WORD: usize = 32;

/// Shortens a rule to its tool name and the first two words of its
/// specifier, each capped at [`MAX_WORD`], so a credential in the rule is
/// not echoed.
///
/// Two cuts, because either alone leaks. Dropping everything after the
/// second word misses a token inside word two —
/// `Bash(curl https://host/v1?token=…*)` is a two-word rule — so each kept
/// word is capped as well. Doctor output gets pasted into issues; it has to
/// be safe to paste.
///
/// This is a mitigation, not a guarantee: a short rule that is entirely a
/// secret cannot be distinguished from a short rule that is not. It bounds
/// the common shape — a long URL carrying a token — and leaves anything
/// short enough to read in full.
fn abbreviate(rule: &str) -> String {
    let (tool, Some(spec)) = split_rule(rule) else {
        return rule.to_string();
    };
    let words: Vec<&str> = spec.split_whitespace().collect();
    let kept: Vec<String> = words.iter().take(2).map(|w| truncate_word(w)).collect();
    let cut = words.len() > 2 || kept.iter().zip(words.iter()).any(|(k, w)| k.as_str() != *w);
    if !cut {
        return rule.to_string();
    }
    format!("{tool}({} …)", kept.join(" "))
}

/// Caps one word at [`MAX_WORD`] characters, on a character boundary.
fn truncate_word(word: &str) -> String {
    if word.chars().count() <= MAX_WORD {
        return word.to_string();
    }
    let head: String = word.chars().take(MAX_WORD).collect();
    format!("{head}…")
}

/// Returns the helper paths a session would run, for the drift check.
#[must_use]
pub fn helper_paths(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for root in roots {
        for (name, _) in crate::init::bundled_scripts() {
            paths.push(root.join(HELPER_DIR).join(name));
        }
    }
    paths
}

/// Reads one helper's state against the copy the binary bundles.
///
/// A helper that is absent is not drift — it simply is not deployed there.
#[must_use]
pub fn helper_state(path: &Path) -> Option<HelperState> {
    let name = path.file_name()?.to_str()?;
    let bundled = crate::init::bundled_scripts()
        .into_iter()
        .find(|(n, _)| *n == name)
        .map(|(_, content)| content)?;
    let on_disk = std::fs::read_to_string(path).ok()?;
    Some(HelperState {
        path: path.to_path_buf(),
        matches_bundled: on_disk == bundled,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli_setup::record::EntryKind;
    use crate::doctor::CheckStatus;

    fn target() -> PathBuf {
        PathBuf::from("/repo/.claude/settings.local.json")
    }

    fn entry(value: &str, kind: EntryKind, key: Option<&str>) -> RecordEntry {
        RecordEntry {
            cli: "claude".into(),
            item: "allowlist".into(),
            file: target(),
            kind,
            key_path: key.map(ToString::to_string),
            value: value.into(),
            created_file: true,
            created_dirs: Vec::new(),
            recorded_at: "2026-10-15T12:00:00Z".into(),
            git_paw_version: "0.17.0".into(),
        }
    }

    /// A probe whose record holds every entry the current binary would plan,
    /// all present in the file — the "applied and intact" state.
    fn applied_probe() -> SetupProbe {
        let mut entries = Vec::new();
        let mut allow = Vec::new();
        let mut ask = Vec::new();
        let mut deny = Vec::new();
        for v in facts::planned_allow_entries() {
            entries.push(entry(&v, EntryKind::Allow, Some("permissions.allow")));
            allow.push(v);
        }
        for v in facts::planned_ask_entries() {
            entries.push(entry(&v, EntryKind::Ask, Some("permissions.ask")));
            ask.push(v);
        }
        for v in facts::planned_deny_entries() {
            entries.push(entry(&v, EntryKind::Deny, Some("permissions.deny")));
            deny.push(v);
        }
        let body = serde_json::json!({
            "permissions": { "allow": allow, "ask": ask, "deny": deny }
        });
        let mut files = BTreeMap::new();
        files.insert(target(), Some(body.to_string()));
        SetupProbe {
            entries,
            files,
            helpers: Vec::new(),
        }
    }

    fn only(checks: &[CheckResult], name: &str) -> CheckResult {
        checks
            .iter()
            .find(|c| c.name == name)
            .cloned()
            .unwrap_or_else(|| panic!("no check named {name} in {checks:?}"))
    }

    // -----------------------------------------------------------------------
    // The setup check (preflight-diagnostics: "Doctor reports CLI setup state")
    // -----------------------------------------------------------------------

    #[test]
    fn not_applied_reports_ok_with_the_items_and_their_verification_status() {
        let checks = check_cli_setup(&SetupProbe::default());
        let c = only(&checks, "claude setup");
        assert_eq!(c.status, CheckStatus::Ok);
        assert!(c.detail.contains("allowlist"), "{}", c.detail);
        assert!(
            c.detail.contains("verified against") || c.detail.contains("plan-only"),
            "the verification status is stated: {}",
            c.detail
        );
    }

    #[test]
    fn applied_and_intact_reports_ok() {
        let c = only(&check_cli_setup(&applied_probe()), "claude setup");
        assert_eq!(c.status, CheckStatus::Ok, "{}", c.detail);
    }

    #[test]
    fn a_recorded_entry_removed_by_hand_warns_naming_undo() {
        let mut probe = applied_probe();
        // The user deleted one entry from the file but the record still claims it.
        probe.files.insert(
            target(),
            Some(serde_json::json!({"permissions":{"allow":[]}}).to_string()),
        );
        let c = only(&check_cli_setup(&probe), "claude setup");
        assert_eq!(c.status, CheckStatus::Warn);
        let remedy = c.remedy.clone().unwrap_or_default();
        assert!(remedy.contains("--undo"), "{remedy}");
    }

    #[test]
    fn applied_entries_out_of_date_warn_naming_undo_and_reapplying() {
        let mut probe = applied_probe();
        // An older version recorded one entry fewer than this binary plans.
        probe.entries.remove(0);
        let c = only(&check_cli_setup(&probe), "claude setup");
        assert_eq!(c.status, CheckStatus::Warn, "{}", c.detail);
        let remedy = c.remedy.clone().unwrap_or_default();
        assert!(remedy.contains("--undo"), "{remedy}");
        assert!(remedy.contains("--allowlist"), "{remedy}");
    }

    #[test]
    fn a_modified_granted_helper_warns_naming_its_path() {
        let mut probe = applied_probe();
        probe.helpers.push(HelperState {
            path: PathBuf::from("/repo/.git-paw/worktrees/feat-x/.git-paw/scripts/broker.sh"),
            matches_bundled: false,
        });
        let c = only(&check_cli_setup(&probe), "claude setup");
        assert_eq!(c.status, CheckStatus::Warn);
        assert!(
            c.detail.contains("feat-x/.git-paw/scripts/broker.sh"),
            "{}",
            c.detail
        );
    }

    #[test]
    fn an_intact_helper_does_not_warn() {
        let mut probe = applied_probe();
        probe.helpers.push(HelperState {
            path: PathBuf::from("/repo/.git-paw/scripts/broker.sh"),
            matches_bundled: true,
        });
        assert_eq!(
            only(&check_cli_setup(&probe), "claude setup").status,
            CheckStatus::Ok
        );
    }

    #[test]
    fn an_identity_with_no_row_and_no_record_gets_no_check() {
        // Exercised against a probe that DOES carry another identity's
        // entries, so the assertion can fail: the check set is derived from
        // the fact table plus the record, and a configured `[clis.*]` entry
        // with neither contributes nothing.
        let mut other = entry(
            "Bash(.git-paw/scripts/broker.sh *)",
            EntryKind::Allow,
            Some("permissions.allow"),
        );
        other.cli = "codex".into();
        let probe = SetupProbe {
            entries: vec![other],
            ..SetupProbe::default()
        };
        let checks = check_cli_setup(&probe);
        let names: Vec<&str> = checks.iter().map(|c| c.name.as_str()).collect();
        assert!(
            names.contains(&"claude setup"),
            "the fact table contributes claude: {names:?}"
        );
        assert!(
            names.contains(&"codex setup"),
            "the record contributes codex: {names:?}"
        );
        assert!(
            !names.iter().any(|n| n.contains("my-agent")),
            "an identity with neither a row nor record entries gets no check: {names:?}"
        );
    }

    #[test]
    fn an_identity_with_only_record_entries_still_gets_a_check() {
        let mut e = entry(
            "Bash(.git-paw/scripts/broker.sh *)",
            EntryKind::Allow,
            Some("permissions.allow"),
        );
        e.cli = "my-agent".into();
        let probe = SetupProbe {
            entries: vec![e],
            ..SetupProbe::default()
        };
        assert!(
            check_cli_setup(&probe)
                .iter()
                .any(|c| c.name == "my-agent setup")
        );
    }

    #[test]
    fn the_setup_check_never_reports_fail() {
        let mut broken = applied_probe();
        broken.files.insert(target(), None);
        broken.helpers.push(HelperState {
            path: PathBuf::from("/repo/.git-paw/scripts/broker.sh"),
            matches_bundled: false,
        });
        for probe in [SetupProbe::default(), applied_probe(), broken] {
            for c in check_cli_setup(&probe) {
                assert_ne!(c.status, CheckStatus::Fail, "{c:?}");
            }
        }
    }

    // -----------------------------------------------------------------------
    // The broad-grant advisory (preflight-diagnostics: "Doctor lists broad
    // durable grants in the project's CLI settings")
    // -----------------------------------------------------------------------

    fn grants(entries: &[&str]) -> BroadGrantProbe {
        BroadGrantProbe {
            target: Some((
                target(),
                serde_json::json!({ "permissions": { "allow": entries } }).to_string(),
            )),
        }
    }

    #[test]
    fn broad_grants_left_by_earlier_approvals_are_listed() {
        let probe = grants(&[
            "Bash(git-paw purge:*)",
            "Bash(git push:*)",
            "Bash(cargo:*)",
            "Bash(npm run test:*)",
        ]);
        let c = only(&check_broad_grants(&probe), "project CLI grants");
        assert_eq!(c.status, CheckStatus::Warn);
        assert!(c.detail.contains("Bash(git-paw purge:*)"), "{}", c.detail);
        assert!(c.detail.contains("Bash(git push:*)"), "{}", c.detail);
        assert!(c.detail.contains("Bash(cargo:*)"), "{}", c.detail);
        assert!(
            !c.detail.contains("npm run test"),
            "a two-word prefix grant is specific enough: {}",
            c.detail
        );
    }

    #[test]
    fn git_paws_own_planned_entries_are_not_listed() {
        let own: Vec<&str> = facts::planned_allow_entries()
            .iter()
            .map(String::as_str)
            .map(|s| Box::leak(s.to_string().into_boxed_str()) as &str)
            .collect();
        let c = only(&check_broad_grants(&grants(&own)), "project CLI grants");
        assert_eq!(c.status, CheckStatus::Ok, "{}", c.detail);
    }

    #[test]
    fn a_wide_helper_grant_is_listed() {
        let c = only(
            &check_broad_grants(&grants(&["Bash(.git-paw/scripts/sweep.sh *)"])),
            "project CLI grants",
        );
        assert_eq!(c.status, CheckStatus::Warn);
        assert!(
            c.detail.contains("Bash(.git-paw/scripts/sweep.sh *)"),
            "{}",
            c.detail
        );
    }

    #[test]
    fn a_long_rule_is_abbreviated_and_never_echoes_a_credential() {
        let probe =
            grants(&[r#"Bash(curl -H "Authorization: Bearer abc123" https://api.example.com/*)"#]);
        let c = only(&check_broad_grants(&probe), "project CLI grants");
        assert_eq!(c.status, CheckStatus::Warn);
        assert!(c.detail.contains("Bash(curl -H …)"), "{}", c.detail);
        assert!(!c.detail.contains("abc123"), "{}", c.detail);
        assert!(!c.remedy.clone().unwrap_or_default().contains("abc123"));
    }

    #[test]
    fn a_credential_inside_the_second_word_is_not_echoed() {
        // The regression this guards: `abbreviate` used to return a rule
        // whole whenever its specifier had two words or fewer, and a token
        // embedded in a URL lives inside word two. Doctor output gets pasted
        // into issues.
        let probe = grants(&["Bash(curl https://api.example.com/v1?token=SUPERSECRET123*)"]);
        let c = only(&check_broad_grants(&probe), "project CLI grants");
        assert_eq!(c.status, CheckStatus::Warn);
        assert!(
            !c.detail.contains("SUPERSECRET123"),
            "a credential in the second word must be cut: {}",
            c.detail
        );
        assert!(
            c.detail.contains("curl"),
            "the entry is still identifiable: {}",
            c.detail
        );
    }

    #[test]
    fn a_rule_short_enough_to_read_is_shown_in_full() {
        // One short word: nothing is cut, so nothing is elided.
        let c = only(
            &check_broad_grants(&grants(&["Bash(cargo:*)"])),
            "project CLI grants",
        );
        assert!(
            c.detail.contains("Bash(cargo:*)") && !c.detail.contains('…'),
            "nothing was cut, so nothing is elided: {}",
            c.detail
        );
    }

    #[test]
    fn a_helper_path_is_never_truncated() {
        // The spec requires a wide helper grant to be listed verbatim, and
        // the longest helper path must survive the word cap.
        for rule in [
            "Bash(.git-paw/scripts/sweep.sh *)",
            "Bash(.git-paw/scripts/docs-fetch.sh *)",
        ] {
            let c = only(&check_broad_grants(&grants(&[rule])), "project CLI grants");
            assert!(c.detail.contains(rule), "{rule} was mangled: {}", c.detail);
        }
    }

    #[test]
    fn a_loopback_curl_grant_is_not_listed() {
        let c = only(
            &check_broad_grants(&grants(&["Bash(curl http://127.0.0.1:9119/*)"])),
            "project CLI grants",
        );
        assert_eq!(c.status, CheckStatus::Ok, "{}", c.detail);
    }

    #[test]
    fn every_broad_shape_is_recognised() {
        for rule in [
            // 1. no specifier or a bare wildcard
            "Bash",
            "Bash(*)",
            "Bash(:*)",
            // 2. a single-word prefix grant
            "Bash(cargo:*)",
            "Bash(git *)",
            "Bash(rm*)",
            // 3. a shell, interpreter or wrapper
            "Bash(bash -c *)",
            "Bash(python3 -c *)",
            "Bash(sudo systemctl *)",
            "Bash(ssh host *)",
            // 4. a git subcommand that publishes or rewrites state
            "Bash(git push:*)",
            "Bash(git reset --hard *)",
            "Bash(git config user.name *)",
            // 5. git-paw itself
            "Bash(git-paw purge:*)",
            "Bash(git paw remove *)",
            // 6. a far-reaching command
            "Bash(rm -rf *)",
            "Bash(chmod 777 *)",
            "Bash(curl https://example.com/*)",
            "Bash(wget https://example.com/*)",
            // 7. a file-tool rule anchored at the root or home
            "Read(//**)",
            "Edit(~/**)",
            "Write(/etc/*)",
        ] {
            assert!(is_broad(rule), "{rule} should be reported as broad");
        }
    }

    #[test]
    fn specific_grants_are_not_recognised_as_broad() {
        for rule in [
            "Bash(npm run test:*)",
            "Bash(git status)",
            "Bash(git log --oneline *)",
            "Bash(git paw status *)",
            "Bash(.git-paw/scripts/broker.sh *)",
            "Bash(.git-paw/scripts/sweep.sh snapshot *)",
            "Edit(src/**)",
            "Read(docs/*)",
        ] {
            assert!(!is_broad(rule), "{rule} should not be reported as broad");
        }
    }

    #[test]
    fn an_unparseable_file_is_reported_without_its_content() {
        let probe = BroadGrantProbe {
            target: Some((target(), "{ not json, secret=s3cret".into())),
        };
        let c = only(&check_broad_grants(&probe), "project CLI grants");
        assert_eq!(c.status, CheckStatus::Warn);
        assert!(c.detail.contains("not inspected"), "{}", c.detail);
        assert!(!c.detail.contains("s3cret"), "{}", c.detail);
    }

    #[test]
    fn an_absent_file_produces_no_check() {
        assert!(check_broad_grants(&BroadGrantProbe::default()).is_empty());
    }

    #[test]
    fn the_advisory_never_reports_fail() {
        for probe in [
            BroadGrantProbe::default(),
            grants(&["Bash(rm -rf *)"]),
            grants(&[]),
            BroadGrantProbe {
                target: Some((target(), "nonsense".into())),
            },
        ] {
            for c in check_broad_grants(&probe) {
                assert_ne!(c.status, CheckStatus::Fail, "{c:?}");
            }
        }
    }
}
