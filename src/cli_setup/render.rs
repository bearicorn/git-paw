//! Rendering a plan for the user to read before anything is written.
//!
//! The command always prints the plan first — before the confirmation prompt
//! and before any write — so the user approves the exact set of entries that
//! will land. [`render_plan`] produces that text.
//!
//! The renderer is pure, and deliberately takes only the [`SetupPlan`]: it is
//! handed the entries git-paw would *add* and nothing else, so there is no
//! path by which existing settings content — a token, a key, a private
//! command — could reach the output. That is a structural guarantee, not a
//! filtering one.

use std::fmt::Write as _;

use super::facts::SetupItem;
use super::plan::{PlannedEntry, PlannedFile, SetupPlan};
use super::undo::{UndoFile, UndoPlan};

/// Marker for a file git-paw would create.
const CREATE: &str = "create";
/// Marker for a file git-paw would modify in place.
const MODIFY: &str = "modify";
/// Marker for a file git-paw would delete.
const DELETE: &str = "delete";

/// Renders the plan the user is asked to approve.
///
/// The output names the CLI and item, the item's verification status, every
/// file by absolute path marked created or modified, every entry with its key
/// path and value (or, for a line file, the line), and the setup-record file.
/// When the row is unverified it also carries the plan-only notice.
///
/// It never contains any existing content of a target file.
#[must_use]
pub fn render_plan(plan: &SetupPlan) -> String {
    let mut out = String::new();

    let _ = writeln!(
        out,
        "Plan for `git paw setup-cli {} --{}`",
        plan.cli,
        plan.item.as_str()
    );
    out.push('\n');
    out.push_str(&render_verification(plan));

    if plan.is_empty() {
        out.push('\n');
        out.push_str("Nothing to do: every entry this item writes is already present.\n");
        return out;
    }

    for file in &plan.files {
        if file.entries.is_empty() {
            continue;
        }
        out.push('\n');
        out.push_str(&render_file(file));
    }

    let _ = write!(out, "\nrecord  {}\n", plan.record_path.display());
    out
}

/// Renders the verification status, and the plan-only notice when the facts
/// behind the item have not been confirmed against an installed CLI.
fn render_verification(plan: &SetupPlan) -> String {
    let mut out = String::new();
    if plan.verified {
        let _ = writeln!(
            out,
            "These facts were verified against {}.",
            plan.verified_with
        );
    } else {
        out.push_str(
            "These facts are UNVERIFIED: they have not been confirmed against an installed\n\
             copy of this CLI. Writing is disabled — this plan can be shown but not applied.\n",
        );
    }
    out
}

/// Renders one file's header and its entries.
fn render_file(file: &PlannedFile) -> String {
    let mut out = String::new();
    let marker = if file.created { CREATE } else { MODIFY };
    let _ = writeln!(out, "{marker}  {}", file.path.display());
    for dir in &file.created_dirs {
        let _ = writeln!(out, "        (creates directory {})", dir.display());
    }
    for entry in &file.entries {
        let _ = writeln!(out, "    {}", render_entry(entry));
    }
    out
}

/// Renders one entry as `<where> += <value>`.
///
/// `+=` says what the write actually is: the value is appended to a list the
/// user may already have other items in, not substituted for it.
fn render_entry(entry: &PlannedEntry) -> String {
    let location = entry
        .key_path_display()
        .unwrap_or_else(|| "line".to_string());
    format!("{location} += {}", entry.value())
}

/// Renders the report for a request naming items the CLI does not declare.
///
/// Nothing is planned for any requested item when this happens, so the report
/// states that too — a mixed request writes nothing at all.
#[must_use]
pub fn render_unsupported(cli: &str, items: &[SetupItem]) -> String {
    let mut out = String::new();
    for item in items {
        let _ = writeln!(
            out,
            "The `{}` item ({}) is not supported for {cli}.",
            item.as_str(),
            item.description()
        );
    }
    out.push_str("Nothing was written.\n");
    out
}

/// Renders the undo plan the user is asked to approve.
///
/// Undo goes through the same plan-then-confirm gate as setup, so the output
/// names the CLI, every file with the entries that would be removed, any file
/// that would be deleted and any directory removed, every entry left in place
/// (with why, and whether it also leaves the record), and the record file.
///
/// Like [`render_plan`] it is given only the plan, so no other content of the
/// user's settings file can reach the output.
#[must_use]
pub fn render_undo_plan(plan: &UndoPlan) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Plan for `git paw setup-cli {} --undo`", plan.cli);

    if plan.is_empty() && plan.reported.is_empty() && plan.retained.is_empty() {
        out.push('\n');
        out.push_str("Nothing to do: nothing is recorded for this CLI.\n");
        return out;
    }

    for file in &plan.files {
        out.push('\n');
        out.push_str(&render_undo_file(file));
    }

    if !plan.reported.is_empty() {
        out.push_str("\nleft in place, and dropped from the record:\n");
        for entry in &plan.reported {
            let _ = writeln!(out, "    {}", entry.value);
            let _ = writeln!(out, "        {}", entry.reason.describe());
            let _ = writeln!(out, "        in {}", entry.file.display());
        }
    }

    if !plan.retained.is_empty() {
        out.push_str("\nleft in place, and still recorded:\n");
        for entry in &plan.retained {
            let _ = writeln!(out, "    {}", entry.value);
            let _ = writeln!(out, "        {}", entry.reason.describe());
            let _ = writeln!(out, "        in {}", entry.file.display());
        }
    }

    let _ = write!(out, "\nrecord  {}\n", plan.record_path.display());
    out
}

/// Renders one file's header and the entries undo would remove from it.
fn render_undo_file(file: &UndoFile) -> String {
    let mut out = String::new();
    let marker = if file.delete_file { DELETE } else { MODIFY };
    let _ = writeln!(out, "{marker}  {}", file.path.display());
    for dir in &file.remove_dirs {
        let _ = writeln!(
            out,
            "        (removes directory {}, if it is empty)",
            dir.display()
        );
    }
    for removal in &file.removals {
        let location = removal
            .key_path
            .clone()
            .unwrap_or_else(|| "line".to_string());
        let _ = writeln!(out, "    {location} -= {}", removal.value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli_setup::facts::{CLAUDE, CliSetupFacts};
    use crate::cli_setup::plan::{FileSnapshot, PlanOutcome, SetupPlan, Snapshots, plan};
    use std::path::PathBuf;

    fn target_path() -> PathBuf {
        PathBuf::from("/repo/.claude/settings.local.json")
    }

    fn exclude_path() -> PathBuf {
        PathBuf::from("/repo/.git/info/exclude")
    }

    fn record_path() -> PathBuf {
        PathBuf::from("/repo/.git/git-paw/cli-setup.json")
    }

    /// A repository with no settings file and nothing ignored.
    fn snapshots() -> Snapshots {
        Snapshots {
            target: FileSnapshot::absent(target_path()),
            exclude: FileSnapshot::absent(exclude_path()),
            record_path: record_path(),
        }
    }

    fn unverified_row() -> CliSetupFacts {
        CliSetupFacts {
            verified: false,
            verified_with: "",
            ..CLAUDE
        }
    }

    fn plan_ok(row: &CliSetupFacts, snaps: &Snapshots) -> SetupPlan {
        match plan(row, &[SetupItem::Allowlist], snaps) {
            Ok(PlanOutcome::Plan(p)) => *p,
            other => panic!("expected a plan, got {other:?}"),
        }
    }

    fn rendered(row: &CliSetupFacts, snaps: &Snapshots) -> String {
        render_plan(&plan_ok(row, snaps))
    }

    // -----------------------------------------------------------------------
    // The whole output (spec: "The plan lists files and entries")
    //
    // A golden over a real plan, so the pinned text tracks what `plan()`
    // actually produces rather than a hand-built fixture.
    // -----------------------------------------------------------------------

    const GOLDEN: &str = "\
Plan for `git paw setup-cli claude --allowlist`

These facts were verified against Claude Code 2.1.220, 2026-10-15.

create  /repo/.claude/settings.local.json
    permissions.allow += Bash(.git-paw/scripts/broker.sh *)
    permissions.allow += Bash(.git-paw/scripts/docs-fetch.sh find *)
    permissions.allow += Bash(.git-paw/scripts/sweep.sh snapshot *)
    permissions.allow += Bash(.git-paw/scripts/sweep.sh capture *)
    permissions.allow += Bash(.git-paw/scripts/sweep.sh status *)
    permissions.allow += Bash(.git-paw/scripts/sweep.sh worktrees-status *)
    permissions.allow += Bash(.git-paw/scripts/sweep.sh inbox *)
    permissions.allow += Bash(.git-paw/scripts/sweep.sh detect-stuck *)
    permissions.allow += Bash(.git-paw/scripts/sweep.sh stuck-eval *)
    permissions.allow += Bash(.git-paw/scripts/sweep.sh status-publish *)
    permissions.allow += Bash(.git-paw/scripts/sweep.sh learn *)
    permissions.allow += Bash(.git-paw/scripts/sweep.sh events *)
    permissions.ask += Bash(.git-paw/scripts/sweep.sh approve *)
    permissions.ask += Edit(**/.git-paw/config.toml)
    permissions.deny += Edit(**/.git-paw/scripts/**)

create  /repo/.git/info/exclude
    line += /.claude/settings.local.json

record  /repo/.git/git-paw/cli-setup.json
";

    #[test]
    fn a_verified_plan_renders_exactly_the_golden() {
        assert_eq!(rendered(&CLAUDE, &snapshots()), GOLDEN);
    }

    // -----------------------------------------------------------------------
    // Required elements (spec: "The plan lists files and entries")
    // -----------------------------------------------------------------------

    #[test]
    fn it_names_the_cli_and_the_item() {
        let out = rendered(&CLAUDE, &snapshots());
        assert!(out.contains("claude"), "{out}");
        assert!(out.contains("allowlist"), "{out}");
    }

    #[test]
    fn it_names_the_target_file_by_absolute_path_marked_created() {
        let out = rendered(&CLAUDE, &snapshots());
        assert!(
            out.contains("create  /repo/.claude/settings.local.json"),
            "{out}"
        );
    }

    #[test]
    fn an_existing_file_is_marked_modified_not_created() {
        let snaps = Snapshots {
            target: FileSnapshot::present(target_path(), "{}"),
            ..snapshots()
        };
        let out = rendered(&CLAUDE, &snaps);
        assert!(
            out.contains("modify  /repo/.claude/settings.local.json"),
            "{out}"
        );
        assert!(
            !out.contains("create  /repo/.claude/settings.local.json"),
            "{out}"
        );
    }

    #[test]
    fn it_lists_each_entry_with_its_key_path_and_value() {
        let out = rendered(&CLAUDE, &snapshots());
        assert!(
            out.contains("permissions.allow += Bash(.git-paw/scripts/broker.sh *)"),
            "{out}"
        );
        assert!(
            out.contains("permissions.deny += Edit(**/.git-paw/scripts/**)"),
            "{out}"
        );
        assert!(
            out.contains("permissions.ask += Bash(.git-paw/scripts/sweep.sh approve *)"),
            "{out}"
        );
    }

    #[test]
    fn it_renders_a_line_entry_as_a_line() {
        let out = rendered(&CLAUDE, &snapshots());
        assert!(
            out.contains("line += /.claude/settings.local.json"),
            "{out}"
        );
    }

    #[test]
    fn it_names_the_setup_record_file() {
        let out = rendered(&CLAUDE, &snapshots());
        assert!(
            out.contains("/repo/.git/git-paw/cli-setup.json"),
            "the record must be named: {out}"
        );
    }

    #[test]
    fn it_names_a_directory_it_would_create() {
        let snaps = Snapshots {
            target: FileSnapshot {
                missing_dirs: vec![PathBuf::from("/repo/.claude")],
                ..FileSnapshot::absent(target_path())
            },
            ..snapshots()
        };
        let out = rendered(&CLAUDE, &snaps);
        assert!(out.contains("creates directory /repo/.claude"), "{out}");
    }

    // -----------------------------------------------------------------------
    // Verification status (spec: "An unverified item can be planned")
    // -----------------------------------------------------------------------

    #[test]
    fn a_verified_plan_says_what_it_was_verified_against() {
        let out = rendered(&CLAUDE, &snapshots());
        assert!(out.contains("verified against"), "{out}");
        assert!(out.contains("2.1.220"), "{out}");
    }

    #[test]
    fn an_unverified_plan_carries_the_plan_only_notice_and_still_lists_entries() {
        let out = rendered(&unverified_row(), &snapshots());
        assert!(out.contains("UNVERIFIED"), "{out}");
        assert!(
            out.contains("Writing is disabled"),
            "the notice must say writing is disabled: {out}"
        );
        // The plan itself is still shown — that is the point of plan-only.
        assert!(
            out.contains("permissions.allow += Bash(.git-paw/scripts/broker.sh *)"),
            "{out}"
        );
        assert!(out.contains("/repo/.claude/settings.local.json"), "{out}");
    }

    // -----------------------------------------------------------------------
    // Nothing to do (spec: "Nothing to do")
    // -----------------------------------------------------------------------

    #[test]
    fn an_empty_plan_reports_that_there_is_nothing_to_do() {
        let body = serde_json::json!({
            "permissions": {
                "allow": crate::cli_setup::facts::planned_allow_entries(),
                "ask": crate::cli_setup::facts::planned_ask_entries(),
                "deny": crate::cli_setup::facts::planned_deny_entries(),
            }
        });
        let snaps = Snapshots {
            target: FileSnapshot {
                ignored: true,
                ..FileSnapshot::present(target_path(), body.to_string())
            },
            ..snapshots()
        };
        let out = rendered(&CLAUDE, &snaps);
        assert!(out.contains("Nothing to do"), "{out}");
    }

    // -----------------------------------------------------------------------
    // Existing content never leaks (spec: "The plan never echoes existing
    // settings")
    // -----------------------------------------------------------------------

    #[test]
    fn a_secret_in_the_target_file_never_reaches_the_rendered_plan() {
        let snaps = Snapshots {
            target: FileSnapshot::present(
                target_path(),
                r#"{"env":{"API_TOKEN":"s3cret"},"permissions":{"allow":["Bash(make test *)"]}}"#,
            ),
            ..snapshots()
        };
        let out = rendered(&CLAUDE, &snaps);
        assert!(!out.contains("s3cret"), "a secret leaked: {out}");
        assert!(!out.contains("API_TOKEN"), "a key leaked: {out}");
        assert!(
            !out.contains("Bash(make test *)"),
            "an existing entry leaked: {out}"
        );
    }

    // -----------------------------------------------------------------------
    // Determinism (the golden above depends on it)
    // -----------------------------------------------------------------------

    #[test]
    fn rendering_is_deterministic() {
        assert_eq!(
            rendered(&CLAUDE, &snapshots()),
            rendered(&CLAUDE, &snapshots())
        );
    }

    #[test]
    fn the_output_carries_no_colour_escapes() {
        assert!(!rendered(&CLAUDE, &snapshots()).contains('\u{1b}'));
    }

    // -----------------------------------------------------------------------
    // Unsupported items (spec: "Trust is not supported for claude", "A mixed
    // request writes nothing")
    // -----------------------------------------------------------------------

    #[test]
    fn an_unsupported_item_is_reported_by_name_for_that_cli() {
        let out = render_unsupported("claude", &[SetupItem::Trust]);
        assert!(out.contains("trust"), "{out}");
        assert!(out.contains("trust pre-acceptance"), "{out}");
        assert!(out.contains("not supported for claude"), "{out}");
        assert!(out.contains("Nothing was written"), "{out}");
    }

    #[test]
    fn every_unsupported_item_in_a_mixed_request_is_reported() {
        let out = render_unsupported("claude", &[SetupItem::Trust, SetupItem::Hooks]);
        assert!(out.contains("trust pre-acceptance"), "{out}");
        assert!(out.contains("lifecycle hooks"), "{out}");
    }
}
