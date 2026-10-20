//! `git paw doctor`'s read-only reports about per-CLI setup.
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
//! - [`check_grant_residue`] names a closed list of known-dangerous leftovers
//!   in those settings — a blanket network client, a grant that would let an
//!   agent session approve its own prompts, and the inert key earlier versions
//!   seeded. Removing them is the user's to do: git-paw never rewrites a grant
//!   it did not write.
//!
//! All three are strictly read-only and none ever reports ✗ — nothing here is
//! a reason for `doctor` to fail.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::doctor::{CheckResult, GROUP_CLIS};

use super::facts::{self, CliSetupFacts, HELPER_DIR, SetupItem};
use super::record::{EntryKind, RecordEntry};

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
    /// Contents of each file the record names and of each item's target file,
    /// `None` when it is gone.
    pub files: BTreeMap<PathBuf, Option<String>>,
    /// Each identity's allowlist target file, read whether or not the record
    /// names it — an entry the user already held is never recorded.
    pub targets: BTreeMap<String, PathBuf>,
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
    let row = row?;

    // Whether the item was applied at all, which is what decides if its plan
    // is worth checking — NOT whether any permission entry was recorded. When
    // every planned entry already existed there is nothing to record but the
    // exclude line, and keying off the recorded values would skip the check
    // for exactly the repositories this change exists to fix: a later
    // hand-deletion would stay green forever.
    let item_applied = recorded.iter().any(|e| e.is_for_item(SetupItem::Allowlist));

    // The permission-rule entries only: the item also records an exclude-file
    // line, which is not a value the grant table plans.
    let applied: Vec<&str> = recorded
        .iter()
        .filter(|e| e.is_for_item(SetupItem::Allowlist))
        .filter(|e| matches!(e.kind, EntryKind::Allow | EntryKind::Ask | EntryKind::Deny))
        .map(|e| e.value.as_str())
        .collect();
    if item_applied {
        let target = probe
            .targets
            .get(identity)
            .and_then(|path| probe.files.get(path))
            .and_then(Option::as_deref)
            .and_then(|content| serde_json::from_str::<serde_json::Value>(content).ok());

        let plan = [
            (row.allow_key_path, facts::planned_allow_entries()),
            (row.ask_key_path, facts::planned_ask_entries()),
            (row.deny_key_path, facts::planned_deny_entries()),
        ];

        // Judged against the target file, not the record: apply skips a value
        // the file already holds and records only what it wrote, so an entry
        // the user already had would otherwise read as missing forever — and
        // the remedy below could never clear it, because undo leaves that
        // entry in place and a re-apply skips it again.
        let missing = plan
            .iter()
            .flat_map(|(key_path, values)| values.iter().map(move |v| (*key_path, v)))
            .filter(|(key_path, value)| {
                !target
                    .as_ref()
                    .is_some_and(|root| value_at(root, key_path, value))
            })
            .count();

        // The other direction: the record holds a grant this version has
        // withdrawn, so what is applied is equally out of date.
        let wanted: Vec<String> = plan.into_iter().flat_map(|(_, values)| values).collect();
        let withdrawn = applied
            .iter()
            .filter(|a| !wanted.iter().any(|w| w == *a))
            .count();

        if missing > 0 || withdrawn > 0 {
            let mut reasons = Vec::new();
            if missing > 0 {
                reasons.push(format!(
                    "this version of git-paw would also write {missing} entr{}",
                    if missing == 1 { "y" } else { "ies" }
                ));
            }
            if withdrawn > 0 {
                reasons.push(format!(
                    "the record holds {withdrawn} entr{} this version no longer plans",
                    if withdrawn == 1 { "y" } else { "ies" }
                ));
            }
            return Some(CheckResult::warn(
                GROUP_CLIS,
                name.to_string(),
                format!(
                    "the applied allowlist is out of date — {}",
                    reasons.join("; ")
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
            let keys: Vec<&str> = key_path.split('.').collect();
            value_at(&root, &keys, &entry.value)
        }
    }
}

/// Returns whether the array at `key_path` holds `value`.
fn value_at(root: &serde_json::Value, key_path: &[&str], value: &str) -> bool {
    let mut cursor = root;
    for key in key_path {
        match cursor.get(key) {
            Some(next) => cursor = next,
            None => return false,
        }
    }
    cursor
        .as_array()
        .is_some_and(|items| items.iter().any(|v| v.as_str() == Some(value)))
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

// ---------------------------------------------------------------------------
// The grant-residue check
// ---------------------------------------------------------------------------

/// Everything the grant-residue check reads: the project's CLI settings files
/// that exist, in report order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResidueProbe {
    /// Each existing settings file and its contents.
    pub files: Vec<(PathBuf, String)>,
}

/// Why a blanket network-client grant is reported.
const REASON_BLANKET_CURL: &str = "an unrestricted network client";

/// Why the leftover key is reported.
const REASON_INERT_SEED: &str = "an inert key that grants nothing in the current CLI, left over from an earlier version \
     of git-paw";

/// Why a grant on a keystroke or verdict subcommand is reported.
const REASON_SELF_APPROVAL: &str = "lets an agent session approve its own permission prompts or publish a supervisor \
     verdict without a prompt";

/// Why a grant on the pass-through fetch subcommand is reported.
const REASON_READ_ANYWHERE: &str = "passes an arbitrary URL through, so a session can read any file it names or send \
     repository content to any host";

/// Why any other withheld subcommand is reported.
///
/// Accurate for a subcommand with no row in [`WITHHELD_REASONS`] — including
/// one added to the grant table after this code was written, which is
/// reported with this reason rather than silently acquiring another
/// subcommand's.
const REASON_WITHHELD: &str =
    "a helper subcommand the grant table deliberately withholds from the allowlist item";

/// What each withheld helper subcommand actually hands over.
///
/// Keyed by `(script, subcommand)` so the reason printed is true of the
/// subcommand matched: `docs-fetch.sh get` is a read-anywhere primitive, not
/// a way to approve a prompt, and the retired `classify` is neither. A
/// subcommand absent here still gets reported — see [`REASON_WITHHELD`].
const WITHHELD_REASONS: &[(&str, &str, &str)] = &[
    ("sweep.sh", "approve", REASON_SELF_APPROVAL),
    ("sweep.sh", "verified", REASON_SELF_APPROVAL),
    ("sweep.sh", "feedback-gate", REASON_SELF_APPROVAL),
    ("sweep.sh", "conflict-ack", REASON_SELF_APPROVAL),
    ("docs-fetch.sh", "get", REASON_READ_ANYWHERE),
];

/// Returns the reason to print for the withheld subcommands one rule reaches.
///
/// A rule can reach several at once (a wildcard in the subcommand position
/// reaches all of them), so the gravest reason wins: a rule covering both
/// `approve` and `classify` is reported as a self-approval grant, which it
/// is.
fn withheld_reason(script: &str, reached: &[&str]) -> &'static str {
    for reason in [REASON_SELF_APPROVAL, REASON_READ_ANYWHERE] {
        if reached.iter().any(|sub| {
            WITHHELD_REASONS
                .iter()
                .any(|(s, name, r)| *s == script && name == sub && *r == reason)
        }) {
            return reason;
        }
    }
    REASON_WITHHELD
}

/// Reports known-dangerous leftovers in the project's CLI settings files.
///
/// A closed list, not a judgement about breadth: the broad-grant advisory
/// already answers "is this rule broad?". Strictly read-only and never ✗.
/// git-paw does not rewrite a grant it did not write, so the remedy is by
/// hand.
#[must_use]
pub fn check_grant_residue(probe: &ResidueProbe) -> Vec<CheckResult> {
    if probe.files.is_empty() {
        return Vec::new();
    }
    let name = "project CLI grant residue".to_string();

    let mut findings: Vec<String> = Vec::new();
    for (path, content) in &probe.files {
        let Ok(root) = serde_json::from_str::<serde_json::Value>(content) else {
            findings.push(format!(
                "{} was not inspected: it is not valid JSON",
                path.display()
            ));
            continue;
        };
        let matched = residue_in(&root);
        if matched.is_empty() {
            continue;
        }
        findings.push(format!(
            "{}: {}",
            path.display(),
            matched
                .iter()
                .map(|(entry, reason)| format!("{entry} ({reason})"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    if findings.is_empty() {
        return vec![CheckResult::ok(
            GROUP_CLIS,
            name,
            format!(
                "no dangerous grant residue in {}",
                probe
                    .files
                    .iter()
                    .map(|(path, _)| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )];
    }
    vec![CheckResult::warn(
        GROUP_CLIS,
        name,
        findings.join("; "),
        "git-paw never rewrites a grant it did not write — remove these entries by hand"
            .to_string(),
    )]
}

/// Returns each residue entry in one parsed settings file, abbreviated.
fn residue_in(root: &serde_json::Value) -> Vec<(String, &'static str)> {
    let mut matched = Vec::new();
    if let Some(items) = root
        .get("permissions")
        .and_then(|p| p.get("allow"))
        .and_then(serde_json::Value::as_array)
    {
        for rule in items.iter().filter_map(serde_json::Value::as_str) {
            if is_blanket_curl(rule) {
                matched.push((abbreviate(rule), REASON_BLANKET_CURL));
            } else if let Some(grant) = withheld_grant(rule) {
                // Rendered from the helper word, not from the start of the
                // specifier: anything before the helper may be an
                // environment assignment carrying a secret.
                let spec = split_rule(rule).1.unwrap_or(rule);
                matched.push((
                    render_from_helper(spec, grant.helper_index),
                    withheld_reason(grant.script, &grant.reached),
                ));
            }
        }
    }
    if root.get(INERT_SEED_KEY).is_some() {
        matched.push((INERT_SEED_KEY.to_string(), REASON_INERT_SEED));
    }
    matched
}

/// The top-level key earlier versions of git-paw seeded into the CLI's
/// settings, removed in v0.16 because the CLI never read it.
const INERT_SEED_KEY: &str = "allowed_bash_prefixes";

/// The tool whose rules grant a command. Only these can grant execution, so
/// only these can hold the residue this check is about — a `Read` or `Edit`
/// rule naming a helper grants reading or writing the file, not running it.
const COMMAND_TOOL: &str = "Bash";

/// Returns whether a rule grants `curl` with any arguments.
///
/// A rule that names a specific endpoint — the loopback broker, say — is not
/// residue: it grants exactly what it says.
fn is_blanket_curl(rule: &str) -> bool {
    let (tool, Some(spec)) = split_rule(rule) else {
        return false;
    };
    if tool != COMMAND_TOOL {
        return false;
    }
    let Some(prefix) = strip_wildcard(spec) else {
        return false;
    };
    prefix.split_whitespace().eq(["curl"])
}

/// Returns whether a rule grants a helper subcommand the grant table never
/// grants, or puts a wildcard where that subcommand would be.
///
/// The withheld set is read from [`facts::GRANT_TABLE`], so this follows the
/// grant table rather than a second list that could drift from it. The helper
/// is matched by file name anywhere in the specifier, so a relative, absolute
/// or interpreter-prefixed invocation is caught alike — which also means a
/// command that merely *names* the script, such as `cat <path>`, matches. That
/// is the deliberate direction to err in: the alternative is missing a real
/// grant hidden behind a wrapper.
fn withheld_grant(rule: &str) -> Option<WithheldGrant> {
    let (tool, Some(spec)) = split_rule(rule) else {
        return None;
    };
    if tool != COMMAND_TOOL {
        return None;
    }
    let (prefix, wildcarded) = match strip_wildcard(spec) {
        Some(prefix) => (prefix, true),
        None => (spec, false),
    };
    let words: Vec<&str> = prefix.split_whitespace().collect();

    let (index, grant) = words.iter().enumerate().find_map(|(i, word)| {
        let name = word.rsplit('/').next().unwrap_or(word);
        facts::GRANT_TABLE
            .iter()
            .find(|g| g.script == name)
            .map(|g| (i, g))
    })?;
    if grant.never_granted.is_empty() {
        return None;
    }

    let reached: Vec<&'static str> = match words.get(index + 1) {
        Some(subcommand) => {
            // A wildcard directly after the subcommand word extends it, so a
            // rule ending `ap*` covers `approve` without naming it.
            let partial = wildcarded && index + 1 == words.len() - 1;
            grant
                .never_granted
                .iter()
                .copied()
                .filter(|withheld| {
                    withheld == subcommand || (partial && withheld.starts_with(subcommand))
                })
                .collect()
        }
        // Nothing follows the script, so a trailing wildcard stands in the
        // subcommand position and sweeps the withheld ones in with the rest.
        None if wildcarded => grant.never_granted.to_vec(),
        None => Vec::new(),
    };
    if reached.is_empty() {
        return None;
    }
    Some(WithheldGrant {
        helper_index: index,
        script: grant.script,
        reached,
    })
}

/// One rule that grants withheld helper subcommands.
#[derive(Debug, Clone, PartialEq, Eq)]
struct WithheldGrant {
    /// Which word of the specifier names the helper.
    helper_index: usize,
    /// The helper's file name.
    script: &'static str,
    /// The withheld subcommands the rule reaches.
    reached: Vec<&'static str>,
}

/// Renders a matched helper grant starting at the helper word.
///
/// [`abbreviate`] alone is not safe here. It keeps the first two words of the
/// specifier, and a rule may carry an environment assignment before the
/// helper — `Bash(TOKEN=… .git-paw/scripts/sweep.sh approve *)` — so the two
/// words it would keep are the secret and the path. The helper word is where
/// the grant becomes meaningful, so the rendering starts there and says that
/// something preceded it without repeating what.
fn render_from_helper(spec: &str, helper_index: usize) -> String {
    let words: Vec<&str> = spec.split_whitespace().collect();
    let kept: Vec<String> = words
        .iter()
        .skip(helper_index)
        .take(2)
        .map(|word| truncate_word(word))
        .collect();
    let cut_after = words.len() > helper_index + 2
        || kept
            .iter()
            .zip(words.iter().skip(helper_index))
            .any(|(k, w)| k.as_str() != *w);

    let mut inner = kept.join(" ");
    if helper_index > 0 {
        inner = format!("… {inner}");
    }
    if cut_after {
        inner = format!("{inner} …");
    }
    format!("{COMMAND_TOOL}({inner})")
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
            targets: BTreeMap::from([("claude".to_string(), target())]),
            helpers: Vec::new(),
        }
    }

    /// Drops `value` from the target file's `permissions.allow`.
    fn drop_from_file(probe: &mut SetupProbe, value: &str) {
        let content = probe.files.get(&target()).cloned().flatten().unwrap();
        let mut root: serde_json::Value = serde_json::from_str(&content).unwrap();
        root["permissions"]["allow"]
            .as_array_mut()
            .unwrap()
            .retain(|v| v.as_str() != Some(value));
        probe.files.insert(target(), Some(root.to_string()));
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
        // An older version recorded one entry fewer than this binary plans,
        // and the target file does not hold it either.
        let dropped = probe.entries.remove(0);
        drop_from_file(&mut probe, &dropped.value);
        let c = only(&check_cli_setup(&probe), "claude setup");
        assert_eq!(c.status, CheckStatus::Warn, "{}", c.detail);
        let remedy = c.remedy.clone().unwrap_or_default();
        assert!(remedy.contains("--undo"), "{remedy}");
        assert!(remedy.contains("--allowlist"), "{remedy}");
    }

    /// `orchestrator-read-surface`'s documented upgrade path: a user who
    /// applied the allowlist under v0.17 has no `sweep.sh events` entry, so
    /// doctor reports the applied setup out of date and names the `--undo`
    /// then `--allowlist` re-run that fixes it. Re-applying (the full
    /// current plan, as `applied_probe` builds it) clears the warning.
    #[test]
    fn a_v017_record_without_the_events_grant_is_out_of_date_until_reapplied() {
        const EVENTS: &str = "Bash(.git-paw/scripts/sweep.sh events *)";

        // A v0.17 setup: the record lists the eleven pre-`events` entries and
        // the settings file holds exactly those same eleven. Both halves
        // matter — the check judges applied state against the FILE, so a
        // fixture that dropped the entry only from the record would still
        // read as fully applied.
        let mut probe = applied_probe();
        probe.entries.retain(|e| e.value != EVENTS);
        drop_from_file(&mut probe, EVENTS);

        let allow_in_file = |p: &SetupProbe| -> Vec<String> {
            let content = p.files.get(&target()).cloned().flatten().unwrap();
            let root: serde_json::Value = serde_json::from_str(&content).unwrap();
            root["permissions"]["allow"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap_or_default().to_string())
                .collect()
        };
        let before = allow_in_file(&probe);
        assert_eq!(before.len(), 11, "the v0.17 grant table");
        assert!(!before.iter().any(|v| v == EVENTS));

        let c = only(&check_cli_setup(&probe), "claude setup");
        assert_eq!(c.status, CheckStatus::Warn, "{}", c.detail);
        assert!(c.detail.contains("out of date"), "{}", c.detail);
        assert!(
            c.detail.contains("write 1 entry"),
            "the warning must name the single missing entry; got {}",
            c.detail
        );
        let remedy = c.remedy.clone().unwrap_or_default();
        assert!(remedy.contains("--undo"), "{remedy}");
        assert!(remedy.contains("--allowlist"), "{remedy}");

        // After `--undo` + `--allowlist` the file carries the twelfth entry
        // and the check is quiet.
        let reapplied = applied_probe();
        assert!(allow_in_file(&reapplied).iter().any(|v| v == EVENTS));
        assert_eq!(
            only(&check_cli_setup(&reapplied), "claude setup").status,
            CheckStatus::Ok
        );
    }

    #[test]
    fn an_entry_the_user_already_had_counts_as_applied() {
        let mut probe = applied_probe();
        // Apply skips a value the file already held, so the record never
        // names it — but the file does, which is what "applied" means.
        probe.entries.remove(0);
        let c = only(&check_cli_setup(&probe), "claude setup");
        assert_eq!(c.status, CheckStatus::Ok, "{}", c.detail);
    }

    #[test]
    fn a_hand_deletion_is_caught_when_every_planned_entry_pre_existed() {
        // The file already held everything the item plans, so apply wrote no
        // permission entry at all and the record holds only the exclude line.
        // The item IS applied, so its plan still has to be checked — keying
        // the check off the recorded values would leave this repository
        // green no matter what was later removed by hand.
        let mut probe = applied_probe();
        let mut exclude_only = entry(".claude/settings.local.json", EntryKind::ExcludeLine, None);
        exclude_only.file = PathBuf::from("/repo/.git/info/exclude");
        probe.files.insert(
            exclude_only.file.clone(),
            Some(".claude/settings.local.json\n".to_string()),
        );
        probe.entries = vec![exclude_only];

        assert_eq!(
            only(&check_cli_setup(&probe), "claude setup").status,
            CheckStatus::Ok,
            "nothing is missing yet"
        );

        drop_from_file(&mut probe, &facts::planned_allow_entries()[0]);
        let c = only(&check_cli_setup(&probe), "claude setup");
        assert_eq!(c.status, CheckStatus::Warn, "{}", c.detail);
    }

    #[test]
    fn a_recorded_entry_the_binary_no_longer_plans_is_reported() {
        let mut probe = applied_probe();
        // A grant an older version wrote and this one has withdrawn: still in
        // the file, still recorded, no longer planned.
        let withdrawn = "Bash(.git-paw/scripts/sweep.sh retired *)";
        probe.entries.push(entry(
            withdrawn,
            EntryKind::Allow,
            Some("permissions.allow"),
        ));
        let content = probe.files.get(&target()).cloned().flatten().unwrap();
        let mut root: serde_json::Value = serde_json::from_str(&content).unwrap();
        root["permissions"]["allow"]
            .as_array_mut()
            .unwrap()
            .push(withdrawn.into());
        probe.files.insert(target(), Some(root.to_string()));

        let c = only(&check_cli_setup(&probe), "claude setup");
        assert_eq!(c.status, CheckStatus::Warn, "{}", c.detail);
        assert!(c.detail.contains("no longer plans"), "{}", c.detail);
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

    // -----------------------------------------------------------------------
    // The grant-residue check (preflight-diagnostics: "Doctor warns about
    // dangerous grant residue in the project's CLI settings")
    // -----------------------------------------------------------------------

    fn shared() -> PathBuf {
        PathBuf::from("/repo/.claude/settings.json")
    }

    /// A residue probe over the project-local file alone.
    fn residue(entries: &[&str]) -> ResidueProbe {
        ResidueProbe {
            files: vec![(
                target(),
                serde_json::json!({ "permissions": { "allow": entries } }).to_string(),
            )],
        }
    }

    #[test]
    fn a_durable_self_approval_grant_is_flagged() {
        let probe = residue(&["Bash(.git-paw/scripts/sweep.sh approve *)"]);
        let c = only(&check_grant_residue(&probe), "project CLI grant residue");
        assert_eq!(c.status, CheckStatus::Warn);
        assert!(
            c.detail.contains(".claude/settings.local.json"),
            "{}",
            c.detail
        );
        // Abbreviated exactly as the broad-grant advisory abbreviates, so the
        // entry is named without echoing anything past its second word.
        assert!(
            c.detail
                .contains("Bash(.git-paw/scripts/sweep.sh approve …)"),
            "{}",
            c.detail
        );
        assert!(
            c.detail.contains("approve its own permission prompts"),
            "the reason is stated: {}",
            c.detail
        );
        let remedy = c.remedy.clone().unwrap_or_default();
        assert!(remedy.contains("by hand"), "{remedy}");
        assert!(
            remedy.contains("never rewrites a grant it did not write"),
            "{remedy}"
        );
    }

    #[test]
    fn every_never_granted_helper_shape_is_flagged() {
        // Whatever path form names the script, and a wildcard standing in the
        // subcommand position.
        for rule in [
            "Bash(.git-paw/scripts/sweep.sh verified *)",
            "Bash(.git-paw/scripts/sweep.sh feedback-gate *)",
            "Bash(.git-paw/scripts/sweep.sh conflict-ack *)",
            "Bash(.git-paw/scripts/sweep.sh *)",
            "Bash(.git-paw/scripts/sweep.sh:*)",
            "Bash(/abs/repo/.git-paw/scripts/sweep.sh approve *)",
            "Bash(bash .git-paw/scripts/sweep.sh approve *)",
            "Bash(.git-paw/scripts/docs-fetch.sh get *)",
            "Bash(.git-paw/scripts/docs-fetch.sh *)",
            // A wildcard that completes a withheld subcommand rather than
            // standing in for it: this covers `approve` without naming it.
            "Bash(.git-paw/scripts/sweep.sh ap*)",
            "Bash(.git-paw/scripts/sweep.sh c*)",
        ] {
            assert!(
                withheld_grant(rule).is_some(),
                "{rule} should be reported as residue"
            );
        }
    }

    #[test]
    fn a_partial_wildcard_that_completes_only_granted_subcommands_is_not_residue() {
        // `snap*` reaches `snapshot`, which the table grants; no withheld
        // subcommand starts with it.
        assert!(withheld_grant("Bash(.git-paw/scripts/sweep.sh snap*)").is_none());
    }

    #[test]
    fn the_reason_names_what_the_matched_subcommand_hands_over() {
        let cases = [
            (
                "Bash(.git-paw/scripts/sweep.sh approve *)",
                REASON_SELF_APPROVAL,
            ),
            (
                "Bash(.git-paw/scripts/docs-fetch.sh get *)",
                REASON_READ_ANYWHERE,
            ),
            // The retired classifier subcommand approves nothing and fetches
            // nothing; saying otherwise would be false.
            (
                "Bash(.git-paw/scripts/sweep.sh classify *)",
                REASON_WITHHELD,
            ),
            // A whole-script wildcard reaches `approve` among others, so the
            // gravest reason is the true one.
            ("Bash(.git-paw/scripts/sweep.sh *)", REASON_SELF_APPROVAL),
        ];
        for (rule, expected) in cases {
            let grant = withheld_grant(rule).unwrap_or_else(|| panic!("{rule} is residue"));
            assert_eq!(
                withheld_reason(grant.script, &grant.reached),
                expected,
                "{rule}"
            );
        }
    }

    #[test]
    fn a_granted_helper_invocation_is_not_residue() {
        for rule in [
            // The grant table grants broker.sh whole, so a whole-script rule
            // for it withholds nothing.
            "Bash(.git-paw/scripts/broker.sh *)",
            "Bash(.git-paw/scripts/sweep.sh snapshot *)",
            "Bash(.git-paw/scripts/sweep.sh learn *)",
            "Bash(.git-paw/scripts/docs-fetch.sh find *)",
            "Bash(cargo test *)",
        ] {
            assert!(
                withheld_grant(rule).is_none(),
                "{rule} should not be reported as residue"
            );
        }
    }

    #[test]
    fn a_blanket_curl_grant_is_flagged() {
        for rule in ["Bash(curl *)", "Bash(curl:*)", "Bash(curl*)"] {
            let c = only(
                &check_grant_residue(&residue(&[rule])),
                "project CLI grant residue",
            );
            assert_eq!(c.status, CheckStatus::Warn, "{rule}: {}", c.detail);
            assert!(c.detail.contains(rule), "{}", c.detail);
            assert!(
                c.detail.contains("unrestricted network client"),
                "{}",
                c.detail
            );
        }
    }

    #[test]
    fn a_loopback_scoped_curl_grant_is_not_residue() {
        let probe = residue(&["Bash(curl -s http://127.0.0.1:9119/status)"]);
        let c = only(&check_grant_residue(&probe), "project CLI grant residue");
        assert_eq!(c.status, CheckStatus::Ok, "{}", c.detail);
    }

    #[test]
    fn the_inert_seed_in_the_shared_settings_file_is_flagged() {
        let probe = ResidueProbe {
            files: vec![(
                shared(),
                serde_json::json!({ "allowed_bash_prefixes": ["git "] }).to_string(),
            )],
        };
        let c = only(&check_grant_residue(&probe), "project CLI grant residue");
        assert_eq!(c.status, CheckStatus::Warn);
        assert!(c.detail.contains(".claude/settings.json"), "{}", c.detail);
        assert!(c.detail.contains("allowed_bash_prefixes"), "{}", c.detail);
        assert!(c.detail.contains("inert key"), "{}", c.detail);
    }

    #[test]
    fn git_paws_own_entries_are_not_residue() {
        // The whole applied state, not the allow list alone: the item also
        // writes `permissions.ask`, and the entry it pins there is
        // `sweep.sh approve` — a value this check reports as residue when it
        // appears in `permissions.allow`. What makes git-paw's own entry safe
        // is the list it lives in, so the probe has to carry every list or
        // nothing holds that scoping in place.
        let probe = ResidueProbe {
            files: vec![(
                target(),
                serde_json::json!({
                    "permissions": {
                        "allow": facts::planned_allow_entries(),
                        "ask": facts::planned_ask_entries(),
                        "deny": facts::planned_deny_entries(),
                    }
                })
                .to_string(),
            )],
        };
        let c = only(&check_grant_residue(&probe), "project CLI grant residue");
        assert_eq!(c.status, CheckStatus::Ok, "{}", c.detail);
    }

    #[test]
    fn only_a_command_rule_can_be_residue() {
        // A rule on a file-reading or file-editing tool grants reading or
        // writing the helper, not running it, so the self-approval reason
        // would simply be false of it.
        for rule in [
            "Read(.git-paw/scripts/sweep.sh *)",
            "Edit(.git-paw/scripts/sweep.sh *)",
            "Write(.git-paw/scripts/sweep.sh approve *)",
        ] {
            assert!(withheld_grant(rule).is_none(), "{rule} grants no execution");
        }
        for rule in ["Read(curl *)", "WebFetch(curl *)"] {
            assert!(!is_blanket_curl(rule), "{rule} grants no execution");
        }
        // The command tool itself is still matched.
        assert!(withheld_grant("Bash(.git-paw/scripts/sweep.sh approve *)").is_some());
        assert!(is_blanket_curl("Bash(curl *)"));
    }

    #[test]
    fn an_unparseable_settings_file_is_reported_as_not_inspected() {
        let probe = ResidueProbe {
            files: vec![(target(), "{ not json, secret=s3cret".into())],
        };
        let c = only(&check_grant_residue(&probe), "project CLI grant residue");
        assert_eq!(c.status, CheckStatus::Warn);
        assert!(c.detail.contains("not inspected"), "{}", c.detail);
        assert!(!c.detail.contains("s3cret"), "{}", c.detail);
    }

    #[test]
    fn residue_is_reported_per_file_across_both_settings_files() {
        let probe = ResidueProbe {
            files: vec![
                (
                    shared(),
                    serde_json::json!({ "allowed_bash_prefixes": [] }).to_string(),
                ),
                (
                    target(),
                    serde_json::json!({
                        "permissions": { "allow": ["Bash(curl *)"] }
                    })
                    .to_string(),
                ),
            ],
        };
        let c = only(&check_grant_residue(&probe), "project CLI grant residue");
        assert_eq!(c.status, CheckStatus::Warn);
        assert!(c.detail.contains(".claude/settings.json"), "{}", c.detail);
        assert!(
            c.detail.contains(".claude/settings.local.json"),
            "{}",
            c.detail
        );
        assert!(c.detail.contains("Bash(curl *)"), "{}", c.detail);
    }

    #[test]
    fn no_settings_file_produces_no_residue_check() {
        assert!(check_grant_residue(&ResidueProbe::default()).is_empty());
    }

    #[test]
    fn a_residue_entry_carrying_a_credential_is_rendered_from_the_helper() {
        // A rule that really IS residue and really does carry a secret: an
        // environment assignment in front of the helper. Rendering from the
        // start of the specifier would keep its first two words — the
        // assignment and the path — and print the token.
        let probe = residue(&["Bash(GH_TOKEN=SUPERSECRET123 .git-paw/scripts/sweep.sh approve *)"]);
        let c = only(&check_grant_residue(&probe), "project CLI grant residue");

        assert_eq!(c.status, CheckStatus::Warn, "the rule is residue");
        assert!(!c.detail.contains("SUPERSECRET123"), "{}", c.detail);
        assert!(!c.detail.contains("GH_TOKEN"), "{}", c.detail);
        assert!(
            !c.remedy
                .clone()
                .unwrap_or_default()
                .contains("SUPERSECRET123"),
            "{:?}",
            c.remedy
        );
        // Still identifiable: the helper and the subcommand are what the user
        // has to find, and the leading elision says something was dropped.
        assert!(
            c.detail
                .contains("Bash(… .git-paw/scripts/sweep.sh approve …)"),
            "{}",
            c.detail
        );
    }

    #[test]
    fn the_residue_check_never_reports_fail() {
        for probe in [
            ResidueProbe::default(),
            residue(&[]),
            residue(&["Bash(curl *)", "Bash(.git-paw/scripts/sweep.sh approve *)"]),
            ResidueProbe {
                files: vec![(target(), "nonsense".into())],
            },
        ] {
            for c in check_grant_residue(&probe) {
                assert_ne!(c.status, CheckStatus::Fail, "{c:?}");
            }
        }
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
