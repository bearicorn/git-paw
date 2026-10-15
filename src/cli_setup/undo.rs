//! The pure undo core: what `git paw setup-cli --undo` would take back.
//!
//! [`plan_undo`] takes the setup record, the fact row and the same file
//! snapshots setup uses, and returns an [`UndoPlan`] describing exactly which
//! entries to remove, which to leave alone, and which to drop from the record.
//! Like [`super::plan::plan`] it touches no filesystem and runs no command.
//!
//! Three properties this layer is responsible for:
//!
//! - **Only git-paw's own entries are ever removed.** An entry is removed only
//!   when it is still present, byte-equal to its recorded value, and of a shape
//!   the item produces — so a tampered record can take back nothing but
//!   git-paw-shaped entries.
//! - **Shape is checked by form, not against today's grant table.** Entries
//!   written by an older version of git-paw stay undoable after the table
//!   changes.
//! - **A grant never outlives its protection.** The protection rules are
//!   removed only once no allow entry naming a helper-script path is left,
//!   whether git-paw wrote that grant or the user did.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::facts::{CliSetupFacts, HELPER_DIR, SetupItem};
use super::plan::{Refusal, Snapshots, exclusion_line};
use super::record::{EntryKind, RecordEntry, SetupRecord};

/// Why a recorded entry is dropped from the record without being removed from
/// disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropReason {
    /// The file, or the container the entry was written to, is gone — so
    /// there is nothing to remove.
    Absent,
    /// The container is still there but no longer holds the recorded value:
    /// someone edited it after git-paw wrote it.
    Changed,
    /// The recorded value is not of a shape this item produces. A record
    /// edited by hand cannot make undo delete an arbitrary entry.
    InvalidShape,
}

impl DropReason {
    /// Returns a one-line explanation for the plan output.
    #[must_use]
    pub fn describe(self) -> &'static str {
        match self {
            Self::Absent => "no longer in the file",
            Self::Changed => "changed since git-paw wrote it",
            Self::InvalidShape => "not a shape this item writes — the record may have been edited",
        }
    }
}

/// Why a recorded entry is kept, both on disk and in the record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetainReason {
    /// A protection rule whose grants have not all gone: the file still holds
    /// an allow entry naming a helper-script path, so removing the protection
    /// would leave that grant able to run a rewritten helper.
    ProtectionStillNeeded,
}

impl RetainReason {
    /// Returns a one-line explanation for the plan output.
    #[must_use]
    pub fn describe(self) -> &'static str {
        match self {
            Self::ProtectionStillNeeded => {
                "kept: the file still allows a helper-script path, which this rule protects"
            }
        }
    }
}

/// One entry undo would remove from a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoRemoval {
    /// Index of the entry in the record's `entries`.
    pub record_index: usize,
    /// What kind of entry it is.
    pub kind: EntryKind,
    /// Dotted key path, for a JSON entry.
    pub key_path: Option<String>,
    /// The value to remove, byte-for-byte.
    pub value: String,
}

/// One entry undo leaves on disk and drops from the record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportedEntry {
    /// Index of the entry in the record's `entries`.
    pub record_index: usize,
    /// The file it names.
    pub file: PathBuf,
    /// The recorded value.
    pub value: String,
    /// Why it is not removed.
    pub reason: DropReason,
}

/// One entry undo leaves on disk **and** in the record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedEntry {
    /// Index of the entry in the record's `entries`.
    pub record_index: usize,
    /// The file it names.
    pub file: PathBuf,
    /// The recorded value.
    pub value: String,
    /// Why it is kept.
    pub reason: RetainReason,
}

/// One file undo would change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoFile {
    /// Absolute path of the file.
    pub path: PathBuf,
    /// The entries to remove, in record order.
    pub removals: Vec<UndoRemoval>,
    /// Whether the file itself should be deleted: git-paw created it and what
    /// remains after the removals is the empty structure git-paw created.
    pub delete_file: bool,
    /// Directories git-paw created on the way to the file, innermost first.
    /// The shell removes each only if it is empty.
    pub remove_dirs: Vec<PathBuf>,
}

/// A complete plan for one `--undo` run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoPlan {
    /// The CLI identity whose entries are being taken back.
    pub cli: String,
    /// The files to change, in order.
    pub files: Vec<UndoFile>,
    /// Entries left on disk and dropped from the record.
    pub reported: Vec<ReportedEntry>,
    /// Entries left on disk and kept in the record.
    pub retained: Vec<RetainedEntry>,
    /// Absolute path of the setup record.
    pub record_path: PathBuf,
}

impl UndoPlan {
    /// Returns whether the plan would change no file.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files
            .iter()
            .all(|f| f.removals.is_empty() && !f.delete_file)
    }

    /// Returns the total number of entries the plan would remove.
    #[must_use]
    pub fn removal_count(&self) -> usize {
        self.files.iter().map(|f| f.removals.len()).sum()
    }

    /// Returns the record entries that survive this plan, in record order.
    ///
    /// Everything removed from disk and everything reported is dropped;
    /// entries retained for another CLI, for an item that was not named, or
    /// because their protection is still needed, stay.
    #[must_use]
    pub fn surviving_entries(&self, record: &SetupRecord) -> Vec<RecordEntry> {
        let mut dropped: Vec<usize> = self
            .files
            .iter()
            .flat_map(|f| f.removals.iter().map(|r| r.record_index))
            .collect();
        dropped.extend(self.reported.iter().map(|r| r.record_index));
        record
            .entries
            .iter()
            .enumerate()
            .filter(|(i, _)| !dropped.contains(i))
            .map(|(_, e)| e.clone())
            .collect()
    }
}

/// The shape a recorded value has, if any this item produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// `Bash(.git-paw/scripts/<name>…)` naming a helper-script file name.
    HelperGrant,
    /// A file-tool rule whose path lies under `.git-paw/` — the protection
    /// rules, and anything an older version wrote in their place.
    FileTool,
    /// The anchored exclusion line for the item's target file.
    ExclusionLine,
}

/// Plans the removal of the entries recorded for one CLI.
///
/// `items` narrows the plan to the named items; an empty slice covers every
/// recorded item. Undo does not consult the row's verification status, so
/// entries written by an earlier version remain removable after a fact row
/// has been marked unverified.
///
/// # Errors
///
/// Returns a [`Refusal`] when the target file cannot be parsed, because undo
/// then cannot tell whether a recorded entry is still present and must not
/// guess.
pub fn plan_undo(
    record: &SetupRecord,
    row: &CliSetupFacts,
    items: &[SetupItem],
    snapshots: &Snapshots,
) -> Result<UndoPlan, Refusal> {
    let target_json = parse_target(&snapshots.target)?;
    let exclude_lines: Vec<&str> = snapshots
        .exclude
        .content
        .as_deref()
        .map(|c| c.lines().map(str::trim).collect())
        .unwrap_or_default();

    let Triage {
        mut target_removals,
        exclude_removals,
        reported,
        protection_candidates,
    } = triage(record, row, items, snapshots, &target_json, &exclude_lines);

    // A grant must never outlive its protection. Look at what the allow list
    // would hold *after* the removals above — including entries the user
    // added — and keep the protection rules while any helper-script path is
    // still allowed there.
    let mut retained = Vec::new();
    if helper_grant_remains(&target_json, row, &target_removals) {
        for (index, entry) in protection_candidates {
            retained.push(RetainedEntry {
                record_index: index,
                file: entry.file.clone(),
                value: entry.value.clone(),
                reason: RetainReason::ProtectionStillNeeded,
            });
        }
    } else {
        for (index, entry) in &protection_candidates {
            target_removals.push(removal(*index, entry));
        }
    }

    let mut files = Vec::new();
    if !target_removals.is_empty() {
        let created = created_file(record, &snapshots.target.path);
        let remaining = apply_removals(&target_json, &target_removals);
        let delete_file = created && is_effectively_empty(&remaining);
        files.push(UndoFile {
            path: snapshots.target.path.clone(),
            remove_dirs: if delete_file {
                dirs_to_remove(record, &snapshots.target.path)
            } else {
                Vec::new()
            },
            removals: target_removals,
            delete_file,
        });
    }
    if !exclude_removals.is_empty() {
        let created = created_file(record, &snapshots.exclude.path);
        let remaining: Vec<&&str> = exclude_lines
            .iter()
            .filter(|l| !exclude_removals.iter().any(|r| r.value == **l))
            .filter(|l| !l.is_empty())
            .collect();
        let delete_file = created && remaining.is_empty();
        files.push(UndoFile {
            path: snapshots.exclude.path.clone(),
            remove_dirs: if delete_file {
                dirs_to_remove(record, &snapshots.exclude.path)
            } else {
                Vec::new()
            },
            removals: exclude_removals,
            delete_file,
        });
    }

    Ok(UndoPlan {
        cli: row.identity.to_string(),
        files,
        reported,
        retained,
        record_path: snapshots.record_path.clone(),
    })
}

/// Each recorded entry sorted into what undo would do with it, before the
/// protection rules are decided.
struct Triage {
    /// Entries to remove from the target settings file.
    target_removals: Vec<UndoRemoval>,
    /// Entries to remove from the shared exclude file.
    exclude_removals: Vec<UndoRemoval>,
    /// Entries left on disk and dropped from the record.
    reported: Vec<ReportedEntry>,
    /// Protection entries that are present, held back until it is known
    /// whether any helper grant survives.
    protection_candidates: Vec<(usize, RecordEntry)>,
}

/// Sorts every recorded entry for this CLI into [`Triage`].
fn triage(
    record: &SetupRecord,
    row: &CliSetupFacts,
    items: &[SetupItem],
    snapshots: &Snapshots,
    target_json: &Value,
    exclude_lines: &[&str],
) -> Triage {
    let mut out = Triage {
        target_removals: Vec::new(),
        exclude_removals: Vec::new(),
        reported: Vec::new(),
        protection_candidates: Vec::new(),
    };

    for (index, entry) in record.entries.iter().enumerate() {
        if !entry.is_for_cli(row.identity) {
            continue;
        }
        if !items.is_empty() && !items.iter().any(|i| entry.is_for_item(*i)) {
            continue;
        }

        let Some(shape) = classify(entry, row, snapshots) else {
            out.reported
                .push(report(index, entry, DropReason::InvalidShape));
            continue;
        };

        match shape {
            Shape::ExclusionLine => {
                if !snapshots.exclude.exists() {
                    out.reported.push(report(index, entry, DropReason::Absent));
                } else if exclude_lines.contains(&entry.value.as_str()) {
                    out.exclude_removals.push(removal(index, entry));
                } else {
                    out.reported.push(report(index, entry, DropReason::Changed));
                }
            }
            Shape::HelperGrant | Shape::FileTool => {
                match presence(target_json, entry, &snapshots.target) {
                    Presence::Present => {
                        if shape == Shape::FileTool {
                            // Decided by the caller, once every grant removal
                            // is known.
                            out.protection_candidates.push((index, entry.clone()));
                        } else {
                            out.target_removals.push(removal(index, entry));
                        }
                    }
                    Presence::ContainerGone => {
                        out.reported.push(report(index, entry, DropReason::Absent));
                    }
                    Presence::ValueGone => {
                        out.reported.push(report(index, entry, DropReason::Changed));
                    }
                }
            }
        }
    }
    out
}

/// Builds a removal from a record entry.
fn removal(index: usize, entry: &RecordEntry) -> UndoRemoval {
    UndoRemoval {
        record_index: index,
        kind: entry.kind,
        key_path: entry.key_path.clone(),
        value: entry.value.clone(),
    }
}

/// Builds a report from a record entry.
fn report(index: usize, entry: &RecordEntry, reason: DropReason) -> ReportedEntry {
    ReportedEntry {
        record_index: index,
        file: entry.file.clone(),
        value: entry.value.clone(),
        reason,
    }
}

/// Whether a recorded value is still in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Presence {
    /// The container holds the recorded value.
    Present,
    /// The file or the container is gone.
    ContainerGone,
    /// The container is there but no longer holds the value.
    ValueGone,
}

/// Returns whether a recorded JSON entry is still present in the target.
fn presence(target: &Value, entry: &RecordEntry, snapshot: &super::plan::FileSnapshot) -> Presence {
    if !snapshot.exists() {
        return Presence::ContainerGone;
    }
    let Some(key_path) = entry.key_path.as_deref() else {
        return Presence::ContainerGone;
    };
    let mut cursor = target;
    for key in key_path.split('.') {
        match cursor.as_object().and_then(|o| o.get(key)) {
            Some(next) => cursor = next,
            None => return Presence::ContainerGone,
        }
    }
    let Some(items) = cursor.as_array() else {
        return Presence::ContainerGone;
    };
    if items
        .iter()
        .filter_map(Value::as_str)
        .any(|v| v == entry.value)
    {
        Presence::Present
    } else {
        Presence::ValueGone
    }
}

/// Classifies a recorded value, returning the shape it matches — or `None`
/// when it matches none the item produces.
///
/// Shape is judged by **form**, so a value written by an older version of
/// git-paw is still recognised after the grant table changes. The file the
/// entry names has to be one the item writes, and its kind has to match that
/// file.
fn classify(entry: &RecordEntry, row: &CliSetupFacts, snapshots: &Snapshots) -> Option<Shape> {
    if entry.file == snapshots.exclude.path {
        return (entry.kind == EntryKind::ExcludeLine && entry.value == exclusion_line(row))
            .then_some(Shape::ExclusionLine);
    }
    if entry.file != snapshots.target.path {
        return None;
    }
    if !matches!(
        entry.kind,
        EntryKind::Allow | EntryKind::Ask | EntryKind::Deny
    ) {
        return None;
    }
    if names_helper_script(&entry.value) {
        return Some(Shape::HelperGrant);
    }
    if is_file_tool_rule_under_git_paw(&entry.value) {
        return Some(Shape::FileTool);
    }
    None
}

/// Returns whether a value is `Bash(<helper dir>/<name>…)` naming a
/// helper-script file name.
fn names_helper_script(value: &str) -> bool {
    let prefix = format!("Bash({HELPER_DIR}/");
    let Some(rest) = value.strip_prefix(&prefix) else {
        return false;
    };
    let name: String = rest
        .chars()
        .take_while(|c| !matches!(c, ' ' | ':' | '*' | ')'))
        .collect();
    !name.is_empty()
        && !name.contains('/')
        && Path::new(&name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("sh"))
}

/// Returns whether a value is a file-tool rule whose path lies under
/// `.git-paw/`.
fn is_file_tool_rule_under_git_paw(value: &str) -> bool {
    for tool in ["Edit", "Read", "Write"] {
        let prefix = format!("{tool}(");
        if let Some(inner) = value
            .strip_prefix(&prefix)
            .and_then(|rest| rest.strip_suffix(')'))
        {
            return inner.contains(".git-paw/");
        }
    }
    false
}

/// Returns whether the allow list would still hold an entry naming a path
/// under the helper directory once `removals` are applied.
///
/// Every allow entry counts, not only git-paw's: a grant the user added
/// themselves needs the protection just as much.
fn helper_grant_remains(target: &Value, row: &CliSetupFacts, removals: &[UndoRemoval]) -> bool {
    let allow_key = row.allow_key_path.join(".");
    let mut cursor = target;
    for key in row.allow_key_path {
        match cursor.as_object().and_then(|o| o.get(*key)) {
            Some(next) => cursor = next,
            None => return false,
        }
    }
    let Some(items) = cursor.as_array() else {
        return false;
    };
    items
        .iter()
        .filter_map(Value::as_str)
        .filter(|value| {
            !removals
                .iter()
                .any(|r| r.key_path.as_deref() == Some(allow_key.as_str()) && r.value == *value)
        })
        .any(|value| value.contains(&format!("{HELPER_DIR}/")))
}

/// Returns the target with every removal applied, so emptiness can be judged.
fn apply_removals(target: &Value, removals: &[UndoRemoval]) -> Value {
    let mut result = target.clone();
    for removal in removals {
        let Some(key_path) = removal.key_path.as_deref() else {
            continue;
        };
        let keys: Vec<&str> = key_path.split('.').collect();
        remove_at(&mut result, &keys, &removal.value);
    }
    result
}

/// Removes `value` from the array at `keys`, if both the path and the value
/// are there. A missing path is simply left alone.
fn remove_at(value: &mut Value, keys: &[&str], target: &str) {
    match keys.split_first() {
        None => {
            if let Some(items) = value.as_array_mut() {
                items.retain(|v| v.as_str() != Some(target));
            }
        }
        Some((head, rest)) => {
            if let Some(next) = value.as_object_mut().and_then(|o| o.get_mut(*head)) {
                remove_at(next, rest, target);
            }
        }
    }
}

/// Returns whether a JSON value is the empty structure git-paw creates:
/// empty lists, and non-empty objects holding nothing but empty structures.
///
/// An **empty** object is deliberately not "effectively empty". `{}` is a
/// value a user can have added themselves — a created file whose only
/// remaining content is `"env": {}` is a file the user extended, and
/// deleting it would be discarding their edit. git-paw only ever creates
/// containers that it then fills, so treating `{}` as content costs nothing
/// and closes the data-loss case.
fn is_effectively_empty(value: &Value) -> bool {
    match value {
        Value::Array(items) => items.is_empty(),
        Value::Object(members) => !members.is_empty() && members.values().all(is_effectively_empty),
        _ => false,
    }
}

/// Returns whether the record says git-paw created `path`.
fn created_file(record: &SetupRecord, path: &Path) -> bool {
    record
        .entries
        .iter()
        .any(|e| e.file == path && e.created_file)
}

/// Returns the directories the record says git-paw created on the way to
/// `path`, innermost first.
fn dirs_to_remove(record: &SetupRecord, path: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    for entry in record.entries.iter().filter(|e| e.file == path) {
        for dir in &entry.created_dirs {
            if !dirs.contains(dir) {
                dirs.push(dir.clone());
            }
        }
    }
    dirs.reverse();
    dirs
}

/// Parses the target file, refusing content undo cannot read.
fn parse_target(snapshot: &super::plan::FileSnapshot) -> Result<Value, Refusal> {
    let Some(content) = &snapshot.content else {
        return Ok(Value::Object(serde_json::Map::new()));
    };
    if content.trim().is_empty() {
        return Ok(Value::Object(serde_json::Map::new()));
    }
    serde_json::from_str(content).map_err(|e| Refusal::MalformedJson {
        path: snapshot.path.clone(),
        detail: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli_setup::facts::CLAUDE;
    use crate::cli_setup::plan::FileSnapshot;

    fn target_path() -> PathBuf {
        PathBuf::from("/repo/.claude/settings.local.json")
    }

    fn exclude_path() -> PathBuf {
        PathBuf::from("/repo/.git/info/exclude")
    }

    fn record_file_path() -> PathBuf {
        PathBuf::from("/repo/.git/git-paw/cli-setup.json")
    }

    fn unverified() -> CliSetupFacts {
        CliSetupFacts {
            verified: false,
            verified_with: "",
            ..CLAUDE
        }
    }

    /// A record entry for the target file.
    fn json_entry(kind: EntryKind, key: &str, value: &str) -> RecordEntry {
        RecordEntry {
            cli: "claude".into(),
            item: "allowlist".into(),
            file: target_path(),
            kind,
            key_path: Some(key.into()),
            value: value.into(),
            created_file: false,
            created_dirs: Vec::new(),
            recorded_at: "2026-10-15T00:00:00Z".into(),
            git_paw_version: "0.17.0".into(),
        }
    }

    fn allow_entry(value: &str) -> RecordEntry {
        json_entry(EntryKind::Allow, "permissions.allow", value)
    }

    fn deny_entry(value: &str) -> RecordEntry {
        json_entry(EntryKind::Deny, "permissions.deny", value)
    }

    fn exclude_entry(value: &str) -> RecordEntry {
        RecordEntry {
            file: exclude_path(),
            kind: EntryKind::ExcludeLine,
            key_path: None,
            value: value.into(),
            ..allow_entry(value)
        }
    }

    fn record(entries: Vec<RecordEntry>) -> SetupRecord {
        SetupRecord {
            schema_version: 1,
            entries,
        }
    }

    fn snaps(target: &str) -> Snapshots {
        Snapshots {
            target: FileSnapshot::present(target_path(), target),
            exclude: FileSnapshot::absent(exclude_path()),
            record_path: record_file_path(),
        }
    }

    fn snaps_with_exclude(target: &str, exclude: &str) -> Snapshots {
        Snapshots {
            exclude: FileSnapshot::present(exclude_path(), exclude),
            ..snaps(target)
        }
    }

    /// Every value the plan would remove, across every file.
    fn removed_values(p: &UndoPlan) -> Vec<String> {
        p.files
            .iter()
            .flat_map(|f| f.removals.iter())
            .map(|r| r.value.clone())
            .collect()
    }

    fn plan_ok(rec: &SetupRecord, row: &CliSetupFacts, snapshots: &Snapshots) -> UndoPlan {
        match plan_undo(rec, row, &[], snapshots) {
            Ok(p) => p,
            Err(e) => panic!("expected a plan, got {e:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Removing only what git-paw added (spec: "Undo removes only recorded
    // entries")
    // -----------------------------------------------------------------------

    #[test]
    fn removes_recorded_present_equal_shape_valid_entries_only() {
        let rec = record(vec![
            allow_entry("Bash(.git-paw/scripts/broker.sh *)"),
            allow_entry("Bash(.git-paw/scripts/sweep.sh learn *)"),
        ]);
        let snapshots = snaps(
            r#"{"model":"x","permissions":{"allow":[
                "Bash(make test *)",
                "Bash(.git-paw/scripts/broker.sh *)",
                "Bash(.git-paw/scripts/sweep.sh learn *)"
            ]}}"#,
        );
        let p = plan_ok(&rec, &CLAUDE, &snapshots);

        assert_eq!(
            removed_values(&p),
            vec![
                "Bash(.git-paw/scripts/broker.sh *)",
                "Bash(.git-paw/scripts/sweep.sh learn *)",
            ]
        );
        assert!(
            !removed_values(&p).contains(&"Bash(make test *)".to_string()),
            "a user entry is never removed"
        );
        assert!(p.reported.is_empty());
        assert!(!p.files[0].delete_file, "the user owns this file");
    }

    #[test]
    fn a_user_entry_in_the_record_file_leaves_other_keys_alone() {
        let rec = record(vec![allow_entry("Bash(.git-paw/scripts/broker.sh *)")]);
        let snapshots = snaps(
            r#"{"model":"x","permissions":{"allow":["Bash(.git-paw/scripts/broker.sh *)"]}}"#,
        );
        let p = plan_ok(&rec, &CLAUDE, &snapshots);
        // Only the one entry is touched; nothing else is named in the plan.
        assert_eq!(p.removal_count(), 1);
        assert_eq!(p.files.len(), 1);
    }

    // -----------------------------------------------------------------------
    // A tampered record (spec: "A forged record entry is not removed")
    // -----------------------------------------------------------------------

    #[test]
    fn a_forged_entry_is_reported_dropped_and_left_on_disk() {
        let rec = record(vec![allow_entry("Bash(make test *)")]);
        let snapshots = snaps(r#"{"permissions":{"allow":["Bash(make test *)"]}}"#);
        let p = plan_ok(&rec, &CLAUDE, &snapshots);

        assert!(
            removed_values(&p).is_empty(),
            "a forged entry is never removed from disk"
        );
        assert_eq!(p.reported.len(), 1);
        assert_eq!(p.reported[0].value, "Bash(make test *)");
        assert_eq!(p.reported[0].reason, DropReason::InvalidShape);
        assert!(
            p.surviving_entries(&rec).is_empty(),
            "and it is dropped from the record"
        );
    }

    #[test]
    fn a_recorded_entry_naming_an_unknown_file_is_invalid() {
        let rec = record(vec![RecordEntry {
            file: PathBuf::from("/etc/passwd"),
            ..allow_entry("Bash(.git-paw/scripts/broker.sh *)")
        }]);
        let p = plan_ok(&rec, &CLAUDE, &snaps("{}"));
        assert!(removed_values(&p).is_empty());
        assert_eq!(p.reported[0].reason, DropReason::InvalidShape);
    }

    // -----------------------------------------------------------------------
    // Changed and absent entries (spec: "A changed entry is left in place")
    // -----------------------------------------------------------------------

    #[test]
    fn an_edited_exclusion_line_is_left_in_place_and_reported() {
        let rec = record(vec![exclude_entry("/.claude/settings.local.json")]);
        let snapshots = snaps_with_exclude("{}", "# notes\n/.claude/settings.local.json.bak\n");
        let p = plan_ok(&rec, &CLAUDE, &snapshots);

        assert!(removed_values(&p).is_empty(), "nothing is removed");
        assert_eq!(p.reported.len(), 1);
        assert_eq!(p.reported[0].reason, DropReason::Changed);
        assert!(
            p.surviving_entries(&rec).is_empty(),
            "it is dropped from the record"
        );
    }

    #[test]
    fn an_entry_whose_container_is_gone_is_reported_absent() {
        let rec = record(vec![allow_entry("Bash(.git-paw/scripts/broker.sh *)")]);
        let p = plan_ok(&rec, &CLAUDE, &snaps(r#"{"model":"x"}"#));
        assert!(removed_values(&p).is_empty());
        assert_eq!(p.reported[0].reason, DropReason::Absent);
    }

    #[test]
    fn an_entry_missing_from_a_present_container_is_reported_changed() {
        let rec = record(vec![allow_entry("Bash(.git-paw/scripts/broker.sh *)")]);
        let p = plan_ok(
            &rec,
            &CLAUDE,
            &snaps(r#"{"permissions":{"allow":["Bash(make test *)"]}}"#),
        );
        assert!(removed_values(&p).is_empty());
        assert_eq!(p.reported[0].reason, DropReason::Changed);
    }

    // -----------------------------------------------------------------------
    // Protection (spec: "Protection outlives a remaining helper grant")
    // -----------------------------------------------------------------------

    #[test]
    fn protection_is_kept_while_an_edited_helper_grant_remains() {
        let rec = record(vec![
            allow_entry("Bash(.git-paw/scripts/broker.sh *)"),
            deny_entry("Edit(**/.git-paw/scripts/**)"),
        ]);
        // The user edited the recorded grant, dropping the space.
        let snapshots = snaps(
            r#"{"permissions":{
                "allow":["Bash(.git-paw/scripts/broker.sh*)"],
                "deny":["Edit(**/.git-paw/scripts/**)"]
            }}"#,
        );
        let p = plan_ok(&rec, &CLAUDE, &snapshots);

        assert!(
            p.reported
                .iter()
                .any(|r| r.value == "Bash(.git-paw/scripts/broker.sh *)"
                    && r.reason == DropReason::Changed),
            "the edited grant is reported, not removed"
        );
        assert!(
            !removed_values(&p).contains(&"Edit(**/.git-paw/scripts/**)".to_string()),
            "the protection rule must stay while a helper path is still allowed"
        );
        assert_eq!(p.retained.len(), 1);
        assert_eq!(p.retained[0].reason, RetainReason::ProtectionStillNeeded);

        let surviving = p.surviving_entries(&rec);
        assert_eq!(surviving.len(), 1, "the retained protection stays recorded");
        assert_eq!(surviving[0].value, "Edit(**/.git-paw/scripts/**)");
    }

    #[test]
    fn protection_is_kept_while_a_user_added_helper_grant_remains() {
        let rec = record(vec![
            allow_entry("Bash(.git-paw/scripts/broker.sh *)"),
            deny_entry("Edit(**/.git-paw/scripts/**)"),
        ]);
        let snapshots = snaps(
            r#"{"permissions":{
                "allow":[
                    "Bash(.git-paw/scripts/broker.sh *)",
                    "Bash(.git-paw/scripts/sweep.sh *)"
                ],
                "deny":["Edit(**/.git-paw/scripts/**)"]
            }}"#,
        );
        let p = plan_ok(&rec, &CLAUDE, &snapshots);

        assert!(removed_values(&p).contains(&"Bash(.git-paw/scripts/broker.sh *)".to_string()));
        assert!(
            !removed_values(&p).contains(&"Edit(**/.git-paw/scripts/**)".to_string()),
            "a grant the user added still needs the protection"
        );
        assert_eq!(p.retained.len(), 1);
    }

    #[test]
    fn protection_is_removed_once_the_last_helper_grant_goes() {
        let rec = record(vec![
            allow_entry("Bash(.git-paw/scripts/broker.sh *)"),
            deny_entry("Edit(**/.git-paw/scripts/**)"),
        ]);
        let snapshots = snaps(
            r#"{"permissions":{
                "allow":["Bash(.git-paw/scripts/broker.sh *)"],
                "deny":["Edit(**/.git-paw/scripts/**)"]
            }}"#,
        );
        let p = plan_ok(&rec, &CLAUDE, &snapshots);

        assert!(removed_values(&p).contains(&"Edit(**/.git-paw/scripts/**)".to_string()));
        assert!(p.retained.is_empty());
        assert!(p.surviving_entries(&rec).is_empty());
    }

    // -----------------------------------------------------------------------
    // Created files and directories (spec: "A created file is deleted when
    // only git-paw's skeleton remains", "A created file the user extended is
    // kept")
    // -----------------------------------------------------------------------

    #[test]
    fn a_skeleton_only_created_file_is_deleted_with_its_directory() {
        let created = |value: &str| RecordEntry {
            created_file: true,
            created_dirs: vec![PathBuf::from("/repo/.claude")],
            ..allow_entry(value)
        };
        let rec = record(vec![
            created("Bash(.git-paw/scripts/broker.sh *)"),
            RecordEntry {
                created_file: true,
                created_dirs: vec![PathBuf::from("/repo/.claude")],
                ..deny_entry("Edit(**/.git-paw/scripts/**)")
            },
        ]);
        let snapshots = snaps(
            r#"{"permissions":{
                "allow":["Bash(.git-paw/scripts/broker.sh *)"],
                "deny":["Edit(**/.git-paw/scripts/**)"]
            }}"#,
        );
        let p = plan_ok(&rec, &CLAUDE, &snapshots);

        assert!(p.files[0].delete_file, "only the skeleton would remain");
        assert_eq!(p.files[0].remove_dirs, vec![PathBuf::from("/repo/.claude")]);
    }

    #[test]
    fn a_created_file_the_user_extended_is_kept() {
        let rec = record(vec![RecordEntry {
            created_file: true,
            created_dirs: vec![PathBuf::from("/repo/.claude")],
            ..allow_entry("Bash(.git-paw/scripts/broker.sh *)")
        }]);
        let snapshots = snaps(
            r#"{"model":"x","permissions":{"allow":["Bash(.git-paw/scripts/broker.sh *)"]}}"#,
        );
        let p = plan_ok(&rec, &CLAUDE, &snapshots);

        assert!(!p.files[0].delete_file, "the user's own key must survive");
        assert!(p.files[0].remove_dirs.is_empty());
    }

    #[test]
    fn a_file_git_paw_did_not_create_is_never_deleted() {
        let rec = record(vec![allow_entry("Bash(.git-paw/scripts/broker.sh *)")]);
        let snapshots =
            snaps(r#"{"permissions":{"allow":["Bash(.git-paw/scripts/broker.sh *)"]}}"#);
        let p = plan_ok(&rec, &CLAUDE, &snapshots);
        assert!(!p.files[0].delete_file);
    }

    // -----------------------------------------------------------------------
    // Scope (spec: "Another CLI's entries are untouched")
    // -----------------------------------------------------------------------

    #[test]
    fn another_clis_entries_are_untouched_on_disk_and_in_the_record() {
        let other = RecordEntry {
            cli: "other-cli".into(),
            ..allow_entry("Bash(.git-paw/scripts/sweep.sh learn *)")
        };
        let rec = record(vec![
            allow_entry("Bash(.git-paw/scripts/broker.sh *)"),
            other.clone(),
        ]);
        let snapshots = snaps(
            r#"{"permissions":{"allow":[
                "Bash(.git-paw/scripts/broker.sh *)",
                "Bash(.git-paw/scripts/sweep.sh learn *)"
            ]}}"#,
        );
        let p = plan_ok(&rec, &CLAUDE, &snapshots);

        assert_eq!(
            removed_values(&p),
            vec!["Bash(.git-paw/scripts/broker.sh *)"],
            "only this CLI's entry is removed"
        );
        assert_eq!(
            p.surviving_entries(&rec),
            vec![other],
            "the other CLI's entry stays recorded"
        );
    }

    #[test]
    fn item_flags_narrow_the_plan() {
        let hooks_entry = RecordEntry {
            item: "hooks".into(),
            ..allow_entry("Bash(.git-paw/scripts/sweep.sh learn *)")
        };
        let rec = record(vec![
            allow_entry("Bash(.git-paw/scripts/broker.sh *)"),
            hooks_entry,
        ]);
        let snapshots = snaps(
            r#"{"permissions":{"allow":[
                "Bash(.git-paw/scripts/broker.sh *)",
                "Bash(.git-paw/scripts/sweep.sh learn *)"
            ]}}"#,
        );

        let narrowed =
            plan_undo(&rec, &CLAUDE, &[SetupItem::Allowlist], &snapshots).expect("a plan");
        assert_eq!(
            removed_values(&narrowed),
            vec!["Bash(.git-paw/scripts/broker.sh *)"],
            "only the named item"
        );

        let everything = plan_ok(&rec, &CLAUDE, &snapshots);
        assert_eq!(
            removed_values(&everything).len(),
            2,
            "a bare --undo covers every recorded item"
        );
    }

    // -----------------------------------------------------------------------
    // Verification status (spec: "Undo works for an unverified item")
    // -----------------------------------------------------------------------

    #[test]
    fn undo_works_for_an_unverified_row() {
        let rec = record(vec![allow_entry("Bash(.git-paw/scripts/broker.sh *)")]);
        let snapshots =
            snaps(r#"{"permissions":{"allow":["Bash(.git-paw/scripts/broker.sh *)"]}}"#);
        let p = plan_ok(&rec, &unverified(), &snapshots);
        assert_eq!(
            removed_values(&p),
            vec!["Bash(.git-paw/scripts/broker.sh *)"],
            "an unverified row can still be undone"
        );
    }

    // -----------------------------------------------------------------------
    // Edges
    // -----------------------------------------------------------------------

    #[test]
    fn an_empty_record_plans_nothing() {
        let rec = record(Vec::new());
        let p = plan_ok(&rec, &CLAUDE, &snaps("{}"));
        assert!(p.is_empty());
        assert!(p.reported.is_empty());
    }

    #[test]
    fn a_malformed_target_refuses_rather_than_guessing() {
        let rec = record(vec![allow_entry("Bash(.git-paw/scripts/broker.sh *)")]);
        let err = plan_undo(&rec, &CLAUDE, &[], &snaps("{ not json")).unwrap_err();
        assert!(matches!(err, Refusal::MalformedJson { ref path, .. } if *path == target_path()));
    }

    #[test]
    fn a_recorded_exclusion_line_is_removed_when_still_present() {
        let rec = record(vec![exclude_entry("/.claude/settings.local.json")]);
        let snapshots = snaps_with_exclude("{}", "# notes\n/.claude/settings.local.json\n");
        let p = plan_ok(&rec, &CLAUDE, &snapshots);
        assert_eq!(removed_values(&p), vec!["/.claude/settings.local.json"]);
    }

    #[test]
    fn an_exclusion_line_for_another_target_is_invalid() {
        let rec = record(vec![exclude_entry("/some/other/path")]);
        let snapshots = snaps_with_exclude("{}", "/some/other/path\n");
        let p = plan_ok(&rec, &CLAUDE, &snapshots);
        assert!(removed_values(&p).is_empty());
        assert_eq!(p.reported[0].reason, DropReason::InvalidShape);
    }

    #[test]
    fn shape_is_judged_by_form_not_against_todays_grant_table() {
        // A subcommand no longer in the grant table is still undoable.
        let rec = record(vec![allow_entry(
            "Bash(.git-paw/scripts/sweep.sh classify *)",
        )]);
        let snapshots =
            snaps(r#"{"permissions":{"allow":["Bash(.git-paw/scripts/sweep.sh classify *)"]}}"#);
        let p = plan_ok(&rec, &CLAUDE, &snapshots);
        assert_eq!(
            removed_values(&p),
            vec!["Bash(.git-paw/scripts/sweep.sh classify *)"]
        );
    }

    #[test]
    fn a_bash_rule_outside_the_helper_directory_is_invalid() {
        for value in [
            "Bash(.git-paw/config.toml *)",
            "Bash(/abs/.git-paw/scripts/broker.sh *)",
            "Bash(.git-paw/scripts/sub/broker.sh *)",
            "Bash(.git-paw/scripts/notascript *)",
        ] {
            assert!(
                !names_helper_script(value),
                "{value} must not be a grant shape"
            );
        }
    }

    #[test]
    fn file_tool_shapes_are_recognised_only_under_git_paw() {
        assert!(is_file_tool_rule_under_git_paw(
            "Edit(**/.git-paw/scripts/**)"
        ));
        assert!(is_file_tool_rule_under_git_paw(
            "Edit(**/.git-paw/config.toml)"
        ));
        assert!(is_file_tool_rule_under_git_paw(
            "Write(**/.git-paw/scripts/**)"
        ));
        assert!(!is_file_tool_rule_under_git_paw("Edit(**/src/**)"));
        assert!(!is_file_tool_rule_under_git_paw(
            "Bash(.git-paw/scripts/broker.sh *)"
        ));
    }
}
