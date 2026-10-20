//! The compiled per-CLI setup fact table and the fixed grant table.
//!
//! Everything in this module is data. A setup item is available for a CLI only
//! when [`FACT_TABLE`] holds a row for that CLI identity declaring the item,
//! and every entry the item writes is derived from that row plus the
//! repository root — never from a command argument, from the consumer's
//! configuration or from repository content.

/// Directory, relative to a session's working directory, that git-paw's
/// bundled helper scripts are invoked from.
///
/// Agents call the helpers by this relative path, which is what makes a
/// path-scoped allow rule possible in the first place.
pub const HELPER_DIR: &str = ".git-paw/scripts";

/// A setup item: one coherent change git-paw can make to an agent CLI's own
/// configuration.
///
/// The names are git-paw's, never a vendor's — what a CLI calls its allowlist
/// is its own business, and the fact row maps the item onto that CLI's file
/// and key paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SetupItem {
    /// Pre-approve git-paw's own helper-script invocations in the CLI's
    /// native allow rules, together with the protection rules that keep an
    /// agent from rewriting what those grants run.
    Allowlist,
    /// Pre-accept the repository as a trusted folder. Reserved: declared by no
    /// CLI today, pending the v0.23.0 per-CLI audit.
    Trust,
    /// Install lifecycle hooks. Reserved: declared by no CLI today, pending
    /// the v0.23.0 per-CLI audit.
    Hooks,
}

impl SetupItem {
    /// Every item, in the order they are reported.
    pub const ALL: [Self; 3] = [Self::Allowlist, Self::Trust, Self::Hooks];

    /// Returns the item's name as it appears on the command line and in the
    /// setup record.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allowlist => "allowlist",
            Self::Trust => "trust",
            Self::Hooks => "hooks",
        }
    }

    /// Returns the human-readable description used when reporting that an
    /// item is not supported for a CLI.
    #[must_use]
    pub fn description(self) -> &'static str {
        match self {
            Self::Allowlist => "helper-script allowlist",
            Self::Trust => "trust pre-acceptance",
            Self::Hooks => "lifecycle hooks",
        }
    }
}

/// How much of one bundled helper script the allowlist item grants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantScope {
    /// Every subcommand, as a single `Bash(<path> *)` entry.
    WholeScript,
    /// Only these subcommands, each as its own `Bash(<path> <sub> *)` entry.
    Subcommands(&'static [&'static str]),
    /// Nothing is granted for this helper.
    Nothing,
}

/// One row of the grant table: what the allowlist item grants for one bundled
/// helper script, and which of its subcommands are deliberately withheld.
#[derive(Debug, Clone, Copy)]
pub struct HelperGrant {
    /// File name of the bundled helper, as [`crate::init::bundled_scripts`] names it.
    pub script: &'static str,
    /// What is granted.
    pub scope: GrantScope,
    /// Subcommands that are deliberately never granted. Recorded so the guard
    /// test can prove every subcommand the helper dispatches was considered,
    /// rather than merely omitted.
    pub never_granted: &'static [&'static str],
}

/// The grant table (design D5), keyed by the names [`crate::init::bundled_scripts`]
/// returns. Least privilege: fixed helper paths and fixed subcommands, nothing
/// derived from an argument.
pub const GRANT_TABLE: &[HelperGrant] = &[
    HelperGrant {
        // Sourced by the other helpers, never invoked, so there is nothing to
        // grant.
        script: "_paw_common.sh",
        scope: GrantScope::Nothing,
        never_granted: &[],
    },
    HelperGrant {
        // Every subcommand is a bounded publish/poll to the configured broker.
        // `--agent <id>` precedes the subcommand, so per-subcommand rules would
        // need mid-pattern wildcards.
        script: "broker.sh",
        scope: GrantScope::WholeScript,
        never_granted: &[],
    },
    HelperGrant {
        // `find` fetches only `<docs_base_url>/llms.txt`. `get` passes absolute
        // `http(s)://` and `file://` URLs through, which is a read-anywhere and
        // exfiltration primitive.
        script: "docs-fetch.sh",
        scope: GrantScope::Subcommands(&["find"]),
        never_granted: &["get"],
    },
    HelperGrant {
        // Only the subcommands that send no keystrokes and publish no verdict.
        script: "sweep.sh",
        scope: GrantScope::Subcommands(SWEEP_GRANTED),
        never_granted: SWEEP_NEVER_GRANTED,
    },
];

/// `sweep.sh` subcommands the allowlist item grants: each reads or publishes
/// the agent's own state, sends no keystrokes to another pane, and publishes no
/// supervisor verdict.
pub const SWEEP_GRANTED: &[&str] = &[
    "snapshot",
    "capture",
    "status",
    "worktrees-status",
    "inbox",
    "detect-stuck",
    "stuck-eval",
    "status-publish",
    "learn",
    // `events` only reads the broker's log and status: no keystroke, no
    // publish of any kind. It is the orchestrator's event surface, so an
    // ungranted `events` would prompt on every turn of a loop whose whole
    // purpose is to wait unattended.
    "events",
];

/// `sweep.sh` subcommands that are deliberately never granted.
///
/// The allowlist item's target file is the repository's, so it reaches worker
/// sessions as well as the supervisor's. Any of these would therefore let a
/// worker act with supervisor authority without a prompt:
///
/// - `approve` presses keys in another pane;
/// - `verified` and `feedback-gate` publish supervisor verdicts, so a worker
///   could forge its own verification;
/// - `conflict-ack` publishes an `agent.feedback` into two *other* agents'
///   inboxes, telling peers a conflict was reviewed and ruled acceptable;
/// - `classify` is removed by `classifier-retirement`; it is listed so the
///   dispatch guard keeps passing on either side of that merge.
pub const SWEEP_NEVER_GRANTED: &[&str] = &[
    "approve",
    "verified",
    "feedback-gate",
    "conflict-ack",
    "classify",
];

/// The `sweep.sh` subcommand that is pinned to the CLI's "always ask" rules.
///
/// It sends keystrokes to another pane. An ask rule wins over any matching
/// allow rule, including a broader `sweep.sh *` grant a user may already have,
/// so pinning it here means `approve` always reaches a human (gate F4
/// confirmed this in every permission mode tested).
pub const SWEEP_ASK_SUBCOMMAND: &str = "approve";

/// A rule the allowlist item writes to protect the helpers it grants
/// (design D6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtectionRule {
    /// The rule value, written verbatim.
    pub value: &'static str,
    /// Whether the rule denies outright or merely forces a prompt.
    pub kind: ProtectionKind,
}

/// Whether a protection rule blocks or prompts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectionKind {
    /// Refused outright.
    Deny,
    /// Always prompts.
    Ask,
}

/// The protection rules the allowlist item writes in the same file and the
/// same write as the grants.
///
/// A granted helper runs without a prompt, and it lives inside the agent's own
/// worktree — so without these an agent could rewrite the script and have the
/// grant run arbitrary code. The grants are never planned without them.
pub const PROTECTION_RULES: &[ProtectionRule] = &[
    ProtectionRule {
        // Refuses the CLI's file-editing tools on any helper-script directory
        // at any depth under the session's working directory.
        value: "Edit(**/.git-paw/scripts/**)",
        kind: ProtectionKind::Deny,
    },
    ProtectionRule {
        // The helpers read this file (broker address, docs base URL), so an
        // edit to it changes what a granted invocation does.
        value: "Edit(**/.git-paw/config.toml)",
        kind: ProtectionKind::Ask,
    },
];

/// One row of the per-CLI setup fact table.
#[derive(Debug, Clone, Copy)]
pub struct CliSetupFacts {
    /// The CLI identity this row describes, keyed exactly as per-CLI
    /// resolution keys it (a built-in roster name or a `[clis.<name>]` key).
    pub identity: &'static str,
    /// The setup items this CLI declares. An item absent here is reported as
    /// not supported.
    pub items: &'static [SetupItem],
    /// The allowlist item's target file, relative to the root of the
    /// repository's **main checkout**.
    pub allowlist_target: &'static str,
    /// The CLI's project-shared settings file, relative to the same root.
    ///
    /// git-paw never writes it — it is recorded so the grant-residue report
    /// can read the other place a durable project grant can live, including
    /// the inert seed versions before v0.16 wrote there.
    pub shared_settings: &'static str,
    /// Key path of the allow list inside the target file.
    pub allow_key_path: &'static [&'static str],
    /// Key path of the ask list inside the target file.
    pub ask_key_path: &'static [&'static str],
    /// Key path of the deny list inside the target file.
    pub deny_key_path: &'static [&'static str],
    /// Whether these facts were confirmed against an installed copy of the
    /// CLI. An unverified row can be planned but never written.
    pub verified: bool,
    /// Which copy of the CLI the facts were confirmed against, and when.
    pub verified_with: &'static str,
}

impl CliSetupFacts {
    /// Returns whether this row declares `item`.
    #[must_use]
    pub fn declares(&self, item: SetupItem) -> bool {
        self.items.contains(&item)
    }
}

/// The `claude` row.
///
/// **Evidence.** Every fact below was confirmed by the change's fact gate
/// (`design.md`, "Fact gate procedure") against Claude Code 2.1.220 on
/// macOS 15.7.7 (arm64) on 2026-10-15, outcome O1 — every blocking step
/// passed. In particular:
///
/// - a session whose working directory is a nested worktree reads the **main
///   checkout's** `.claude/settings.local.json`, and runs that worktree's own
///   helper copy (F1a); the rule also works for a session at the root (F1e);
///   with no settings file the same invocation is denied (F1c, control);
/// - the space form matches both a bare invocation and an argument-bearing one
///   (F2a/F2b2/F2c1), so no extra exact-form entries are needed, and it does
///   not bleed into a longer verb (F2c5, `snapshotx` refused);
/// - environment prefixes, `env`/`bash` wrappers, extra compound segments and
///   command substitutions do not ride on a grant (F2d1–F2d4, F3b–F3e);
/// - the `approve` ask rule wins over the matching allow rules in `manual`,
///   `acceptEdits` and `auto` (F4);
/// - [`PROTECTION_RULES`] hold: the deny rule blocks the Edit **and** Write
///   tools, at any depth, from a worker's working directory and from the root
///   session reaching into a worker's copy (F5a–F5e), and it also blocks every
///   filesystem command `acceptEdits` auto-approves — `sed -i`, `cp`, `mv`,
///   `rm`, `touch`, a `printf` redirect and both `cd`-prefixed forms (F6-1…9),
///   while an ordinary file outside the denied directory is still edited
///   without a prompt (F5g/F6-ctl, controls).
pub const CLAUDE: CliSetupFacts = CliSetupFacts {
    identity: "claude",
    items: &[SetupItem::Allowlist],
    allowlist_target: ".claude/settings.local.json",
    shared_settings: ".claude/settings.json",
    allow_key_path: &["permissions", "allow"],
    ask_key_path: &["permissions", "ask"],
    deny_key_path: &["permissions", "deny"],
    verified: true,
    verified_with: "Claude Code 2.1.220, 2026-10-15",
};

/// The per-CLI setup fact table.
///
/// Exactly one row today. Every other CLI identity — including a
/// `[clis.<name>]` entry whose command launches a variant of a declared CLI —
/// has no row and therefore supports no item. A variant such as `claude-oss`
/// is still covered by `git paw setup-cli claude`, because the target settings
/// file belongs to the repository rather than to the CLI.
pub const FACT_TABLE: &[CliSetupFacts] = &[CLAUDE];

/// Returns the fact row for `identity`, if the table has one.
#[must_use]
pub fn row_for(identity: &str) -> Option<&'static CliSetupFacts> {
    FACT_TABLE.iter().find(|row| row.identity == identity)
}

/// Returns the invocation path a rule names for one bundled helper.
fn helper_path(script: &str) -> String {
    format!("{HELPER_DIR}/{script}")
}

/// Returns the `permissions.allow` entries the allowlist item plans, in grant
/// table order.
///
/// Derived entirely from [`GRANT_TABLE`], so it is identical in every
/// repository whatever the consumer's configuration holds.
#[must_use]
pub fn planned_allow_entries() -> Vec<String> {
    let mut entries = Vec::new();
    for grant in GRANT_TABLE {
        let path = helper_path(grant.script);
        match grant.scope {
            GrantScope::Nothing => {}
            GrantScope::WholeScript => entries.push(format!("Bash({path} *)")),
            GrantScope::Subcommands(subs) => {
                for sub in subs {
                    entries.push(format!("Bash({path} {sub} *)"));
                }
            }
        }
    }
    entries
}

/// Returns the `permissions.ask` entries the allowlist item plans: the pinned
/// keystroke subcommand, then the protection rules that prompt.
#[must_use]
pub fn planned_ask_entries() -> Vec<String> {
    let mut entries = vec![format!(
        "Bash({}/sweep.sh {SWEEP_ASK_SUBCOMMAND} *)",
        HELPER_DIR
    )];
    entries.extend(
        PROTECTION_RULES
            .iter()
            .filter(|rule| rule.kind == ProtectionKind::Ask)
            .map(|rule| rule.value.to_string()),
    );
    entries
}

/// Returns the `permissions.deny` entries the allowlist item plans.
#[must_use]
pub fn planned_deny_entries() -> Vec<String> {
    PROTECTION_RULES
        .iter()
        .filter(|rule| rule.kind == ProtectionKind::Deny)
        .map(|rule| rule.value.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init;

    /// The bundled `sweep.sh`, read at compile time so the dispatch guard
    /// below sees the same copy the binary ships.
    const SWEEP_SOURCE: &str = include_str!("../../assets/scripts/sweep.sh");

    /// Extracts the subcommands `sweep.sh`'s `main` dispatch recognises.
    ///
    /// The dispatch is a `case` whose arms read `  <name>) cmd_<fn> "$@" ;;`.
    /// The help and catch-all arms are not subcommands.
    fn dispatched_sweep_subcommands() -> Vec<String> {
        let body = SWEEP_SOURCE
            .split_once("main() {")
            .map_or(SWEEP_SOURCE, |(_, rest)| rest);
        let mut found = Vec::new();
        for line in body.lines() {
            let line = line.trim();
            if line.starts_with('*') || line.starts_with('-') || line.contains("usage;") {
                continue;
            }
            let Some((name, rest)) = line.split_once(')') else {
                continue;
            };
            if !rest.contains("cmd_") {
                continue;
            }
            let name = name.trim();
            if !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
                found.push(name.to_string());
            }
        }
        found
    }

    // -----------------------------------------------------------------------
    // Fact table (spec: "Fact support is data")
    // -----------------------------------------------------------------------

    #[test]
    fn fact_table_holds_exactly_the_claude_allowlist_row() {
        assert_eq!(FACT_TABLE.len(), 1, "exactly one row");
        let row = FACT_TABLE[0];
        assert_eq!(row.identity, "claude");
        assert_eq!(
            row.items,
            &[SetupItem::Allowlist],
            "exactly the allowlist item"
        );
    }

    #[test]
    fn no_row_declares_trust_or_hooks() {
        for row in FACT_TABLE {
            assert!(!row.declares(SetupItem::Trust), "{}", row.identity);
            assert!(!row.declares(SetupItem::Hooks), "{}", row.identity);
        }
    }

    #[test]
    fn an_identity_without_a_row_is_unknown_to_the_table() {
        assert!(row_for("claude").is_some());
        assert!(row_for("my-agent").is_none());
        assert!(
            row_for("claude-oss").is_none(),
            "a variant identity has no row of its own"
        );
    }

    // -----------------------------------------------------------------------
    // Grant table coverage (spec: "The grant table covers every helper and
    // subcommand")
    // -----------------------------------------------------------------------

    #[test]
    fn every_bundled_helper_has_a_grant_table_row() {
        for (name, _) in init::bundled_scripts() {
            assert!(
                GRANT_TABLE.iter().any(|g| g.script == name),
                "bundled helper {name} has no grant-table row"
            );
        }
        for grant in GRANT_TABLE {
            assert!(
                init::bundled_scripts()
                    .iter()
                    .any(|(n, _)| *n == grant.script),
                "grant-table row {} names a helper the binary does not bundle",
                grant.script
            );
        }
    }

    #[test]
    fn every_dispatched_sweep_subcommand_is_granted_or_never_granted() {
        let dispatched = dispatched_sweep_subcommands();
        assert!(
            dispatched.len() >= 10,
            "dispatch parse looks wrong, found {dispatched:?}"
        );
        for sub in &dispatched {
            let granted = SWEEP_GRANTED.contains(&sub.as_str());
            let withheld = SWEEP_NEVER_GRANTED.contains(&sub.as_str());
            assert!(
                granted ^ withheld,
                "sweep.sh subcommand `{sub}` is in neither column (or both) — \
                 decide whether it may run without a prompt"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Planned entries (spec: "The allow entries are exactly the grant table",
    // "Every planned allow entry names a bundled helper", "Keystroke and
    // verdict subcommands are never allowed")
    // -----------------------------------------------------------------------

    #[test]
    fn allow_entries_are_exactly_the_grant_table() {
        let planned = planned_allow_entries();
        assert_eq!(
            planned,
            vec![
                "Bash(.git-paw/scripts/broker.sh *)",
                "Bash(.git-paw/scripts/docs-fetch.sh find *)",
                "Bash(.git-paw/scripts/sweep.sh snapshot *)",
                "Bash(.git-paw/scripts/sweep.sh capture *)",
                "Bash(.git-paw/scripts/sweep.sh status *)",
                "Bash(.git-paw/scripts/sweep.sh worktrees-status *)",
                "Bash(.git-paw/scripts/sweep.sh inbox *)",
                "Bash(.git-paw/scripts/sweep.sh detect-stuck *)",
                "Bash(.git-paw/scripts/sweep.sh stuck-eval *)",
                "Bash(.git-paw/scripts/sweep.sh status-publish *)",
                "Bash(.git-paw/scripts/sweep.sh learn *)",
                "Bash(.git-paw/scripts/sweep.sh events *)",
            ]
        );
        assert_eq!(planned.len(), 12, "the grant table is twelve allow entries");
    }

    /// `cli-setup` scenario "The read-only events subcommand is granted":
    /// `events` only reads the broker's log and status, so it joins the
    /// granted read-only subcommands rather than prompting on every turn of
    /// an unattended orchestrator loop.
    #[test]
    fn the_read_only_events_subcommand_is_granted() {
        assert!(
            planned_allow_entries()
                .contains(&"Bash(.git-paw/scripts/sweep.sh events *)".to_string()),
            "events must be granted read-only"
        );
        assert!(
            !planned_ask_entries()
                .contains(&"Bash(.git-paw/scripts/sweep.sh events *)".to_string()),
            "events sends no keystroke, so it must not be pinned to ask"
        );
    }

    #[test]
    fn the_keystroke_subcommand_is_asked_not_allowed() {
        assert!(
            planned_ask_entries()
                .contains(&"Bash(.git-paw/scripts/sweep.sh approve *)".to_string()),
            "approve must be pinned to the ask rules"
        );
    }

    #[test]
    fn no_allow_entry_names_a_keystroke_or_verdict_subcommand() {
        for entry in planned_allow_entries() {
            for withheld in SWEEP_NEVER_GRANTED {
                assert!(
                    !entry.contains(&format!("sweep.sh {withheld}")),
                    "{entry} grants a withheld subcommand"
                );
            }
            assert!(
                !entry.contains("docs-fetch.sh get"),
                "{entry} grants the read-anywhere subcommand"
            );
            assert!(
                !entry.starts_with("Bash(.git-paw/scripts/sweep.sh *"),
                "{entry} is a whole-script sweep.sh grant"
            );
        }
    }

    #[test]
    fn every_allow_entry_is_a_path_scoped_bundled_helper_invocation() {
        for entry in planned_allow_entries() {
            let named = init::bundled_scripts()
                .iter()
                .any(|(name, _)| entry.starts_with(&format!("Bash(.git-paw/scripts/{name}")));
            assert!(named, "{entry} does not name a bundled helper");

            assert!(!entry.contains("curl"), "{entry} names a network client");
            assert!(!entry.contains("bash "), "{entry} names an interpreter");
            assert!(
                !entry.starts_with("Bash(/"),
                "{entry} uses an absolute path"
            );
            assert!(!entry.starts_with("Bash(*"), "{entry} is a blanket grant");
        }
    }

    #[test]
    fn protection_rules_accompany_the_grants() {
        assert!(planned_deny_entries().contains(&"Edit(**/.git-paw/scripts/**)".to_string()));
        assert!(planned_ask_entries().contains(&"Edit(**/.git-paw/config.toml)".to_string()));
    }

    #[test]
    fn planned_entries_do_not_depend_on_anything_but_the_tables() {
        // Called twice with no inputs at all: there is nowhere for repository
        // state or configuration to enter.
        assert_eq!(planned_allow_entries(), planned_allow_entries());
        assert_eq!(planned_ask_entries(), planned_ask_entries());
        assert_eq!(planned_deny_entries(), planned_deny_entries());
    }

    #[test]
    fn item_names_are_git_paws_own() {
        assert_eq!(SetupItem::Allowlist.as_str(), "allowlist");
        assert_eq!(SetupItem::Trust.as_str(), "trust");
        assert_eq!(SetupItem::Hooks.as_str(), "hooks");
    }
}
