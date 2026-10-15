//! The pure planning core: what `git paw setup-cli` would write.
//!
//! [`plan`] takes the fact row, the requested items and a snapshot of every
//! file it might touch, and returns either a [`SetupPlan`] describing exactly
//! what would be written or a [`Refusal`] saying why nothing can be. It
//! touches no filesystem and runs no command: the shell collects the snapshots
//! (see [`super::shell`]) and applies the result.
//!
//! Two properties this layer is responsible for:
//!
//! - **Nothing unplanned is ever written.** The plan is the complete list of
//!   entries, and apply writes exactly it.
//! - **An entry the user already has is never claimed.** An entry already
//!   present is neither planned nor recorded, so undo can never take back
//!   something git-paw did not add.

use std::path::{Path, PathBuf};

use super::facts::{self, CliSetupFacts, SetupItem};

/// What a target file looked like when the plan was made.
///
/// The shell collects this; the planner is pure over it. Re-reading the file
/// before the write and comparing against the same snapshot is what makes a
/// concurrent change detectable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSnapshot {
    /// Absolute path of the file.
    pub path: PathBuf,
    /// The file's content, or `None` when it does not exist.
    pub content: Option<String>,
    /// Whether git tracks the file.
    pub tracked: bool,
    /// Whether git ignores the file.
    pub ignored: bool,
    /// The first path component between the anchor directory and the file
    /// that is a symbolic link, if any.
    pub symlink_component: Option<PathBuf>,
    /// Whether the resolved path lies outside the directory it is anchored
    /// to (the repository root, or the common git directory).
    pub escapes_anchor: bool,
    /// The file's permission bits, when it exists.
    pub mode: Option<u32>,
    /// Directories on the way to the file that do not exist yet, outermost
    /// first. The shell resolves these; the planner only reports them.
    pub missing_dirs: Vec<PathBuf>,
}

impl FileSnapshot {
    /// Returns a snapshot of a file that does not exist.
    #[must_use]
    pub fn absent(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            content: None,
            tracked: false,
            ignored: false,
            symlink_component: None,
            escapes_anchor: false,
            mode: None,
            missing_dirs: Vec::new(),
        }
    }

    /// Returns a snapshot of an existing file holding `content`.
    #[must_use]
    pub fn present(path: impl Into<PathBuf>, content: impl Into<String>) -> Self {
        Self {
            content: Some(content.into()),
            ..Self::absent(path)
        }
    }

    /// Returns whether the file existed when the snapshot was taken.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.content.is_some()
    }
}

/// Everything the planner needs to know about the filesystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshots {
    /// The item's target settings file, in the repository's main checkout.
    pub target: FileSnapshot,
    /// The repository's shared `info/exclude` file.
    pub exclude: FileSnapshot,
    /// Absolute path of the setup record.
    pub record_path: PathBuf,
}

/// One entry the plan would add to a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannedEntry {
    /// An item appended to a JSON array at `key_path`.
    Json {
        /// Key path of the array, e.g. `permissions.allow`.
        key_path: Vec<String>,
        /// The value appended to it.
        value: String,
    },
    /// A line appended to a line-oriented file.
    Line {
        /// The line, without its newline.
        line: String,
    },
}

impl PlannedEntry {
    /// Returns the entry's key path rendered for display, or `None` for a
    /// line entry.
    #[must_use]
    pub fn key_path_display(&self) -> Option<String> {
        match self {
            Self::Json { key_path, .. } => Some(key_path.join(".")),
            Self::Line { .. } => None,
        }
    }

    /// Returns the value (or line) the entry writes.
    #[must_use]
    pub fn value(&self) -> &str {
        match self {
            Self::Json { value, .. } | Self::Line { line: value } => value,
        }
    }
}

/// One file the plan would create or modify.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFile {
    /// Absolute path of the file.
    pub path: PathBuf,
    /// Whether git-paw would create the file (as opposed to modifying one
    /// that already exists).
    pub created: bool,
    /// Directories git-paw would create on the way to it.
    pub created_dirs: Vec<PathBuf>,
    /// The entries to add, in write order.
    pub entries: Vec<PlannedEntry>,
}

/// A complete plan for one `setup-cli` run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupPlan {
    /// The CLI identity.
    pub cli: String,
    /// The item being applied.
    pub item: SetupItem,
    /// Whether the fact row is verified. An unverified row can be planned but
    /// never written.
    pub verified: bool,
    /// Which copy of the CLI the facts were confirmed against, and when.
    pub verified_with: String,
    /// The files to write, in order.
    pub files: Vec<PlannedFile>,
    /// Absolute path of the setup record.
    pub record_path: PathBuf,
}

impl SetupPlan {
    /// Returns whether the plan would write nothing because every entry is
    /// already present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.iter().all(|f| f.entries.is_empty())
    }

    /// Returns the total number of entries the plan would add.
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.files.iter().map(|f| f.entries.len()).sum()
    }
}

/// The outcome of planning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanOutcome {
    /// One or more requested items are not declared for this CLI. Nothing is
    /// planned for any requested item — the run is all-or-nothing.
    Unsupported {
        /// The CLI identity the items were requested for.
        cli: String,
        /// Every requested item that is not declared, in request order.
        items: Vec<SetupItem>,
    },
    /// A plan, possibly empty (nothing to do).
    Plan(Box<SetupPlan>),
}

/// Why the command refuses to plan. A refusal writes nothing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    /// A file the plan would write is tracked by git.
    #[error(
        "{} is tracked by git — setup only writes files that are not under version control; \
         remove it from the index or choose a different setup",
        .0.display()
    )]
    Tracked(PathBuf),

    /// A path component is a symbolic link.
    #[error(
        "{} is a symbolic link — setup refuses to write through a link, because the real \
         target may be outside the repository",
        .0.display()
    )]
    Symlink(PathBuf),

    /// The resolved path lies outside the directory it is anchored to.
    #[error("{} resolves outside the repository — refusing to write there", .0.display())]
    Escapes(PathBuf),

    /// An existing JSON target is not valid JSON.
    #[error("{} is not valid JSON ({detail}) — fix or remove it, then re-run", .path.display())]
    MalformedJson {
        /// The file.
        path: PathBuf,
        /// The parser's complaint.
        detail: String,
    },

    /// An existing JSON target's root is not an object.
    #[error("{} does not hold a JSON object at its root — refusing to rewrite it", .path.display())]
    NonObjectRoot {
        /// The file.
        path: PathBuf,
    },

    /// A container on the item's key path holds a value of the wrong type.
    #[error(
        "{} holds a {found} at `{key}`, but setup needs a list there — fix it, then re-run",
        .path.display()
    )]
    WrongType {
        /// The file.
        path: PathBuf,
        /// The key path, dotted.
        key: String,
        /// What was found instead of a list.
        found: String,
    },
}

/// Plans the requested items for one CLI.
///
/// Returns [`PlanOutcome::Unsupported`] when any requested item is not
/// declared by the row — nothing is planned for any of them, so a mixed
/// request writes nothing.
///
/// # Errors
///
/// Returns a [`Refusal`] when a target is unsafe to write (tracked, reached
/// through a symlink, escaping the repository) or when an existing JSON target
/// cannot be merged into safely.
pub fn plan(
    row: &CliSetupFacts,
    requested: &[SetupItem],
    snapshots: &Snapshots,
) -> Result<PlanOutcome, Refusal> {
    let unsupported: Vec<SetupItem> = requested
        .iter()
        .copied()
        .filter(|item| !row.declares(*item))
        .collect();
    if !unsupported.is_empty() {
        return Ok(PlanOutcome::Unsupported {
            cli: row.identity.to_string(),
            items: unsupported,
        });
    }

    // Only the allowlist item is declared by any row today; a row that
    // declared another item would need its own branch here.
    debug_assert!(requested.iter().all(|i| *i == SetupItem::Allowlist));

    let target = &snapshots.target;
    reject_unsafe(target)?;
    reject_unsafe(&snapshots.exclude)?;

    let existing = parse_target(target)?;
    // Refuse a wrong-typed container before planning anything: a run that
    // cannot write every entry writes none of them.
    for key_path in [row.allow_key_path, row.ask_key_path, row.deny_key_path] {
        existing_array(&existing, &target.path, key_path)?;
    }
    let mut files = Vec::new();

    let mut entries = Vec::new();
    push_missing(
        &mut entries,
        &existing,
        row.allow_key_path,
        &facts::planned_allow_entries(),
    );
    push_missing(
        &mut entries,
        &existing,
        row.ask_key_path,
        &facts::planned_ask_entries(),
    );
    push_missing(
        &mut entries,
        &existing,
        row.deny_key_path,
        &facts::planned_deny_entries(),
    );

    files.push(PlannedFile {
        path: target.path.clone(),
        created: !target.exists(),
        created_dirs: target.missing_dirs.clone(),
        entries,
    });

    // The exclusion line keeps a file git-paw created out of the user's
    // `git status`. A target git already ignores needs none.
    if !target.ignored {
        let line = exclusion_line(row);
        let already = snapshots
            .exclude
            .content
            .as_deref()
            .is_some_and(|c| c.lines().any(|l| l.trim() == line));
        if !already {
            files.push(PlannedFile {
                path: snapshots.exclude.path.clone(),
                created: !snapshots.exclude.exists(),
                created_dirs: snapshots.exclude.missing_dirs.clone(),
                entries: vec![PlannedEntry::Line { line }],
            });
        }
    }

    Ok(PlanOutcome::Plan(Box::new(SetupPlan {
        cli: row.identity.to_string(),
        item: SetupItem::Allowlist,
        verified: row.verified,
        verified_with: row.verified_with.to_string(),
        files,
        record_path: snapshots.record_path.clone(),
    })))
}

/// Returns the anchored exclusion line for a row's target file.
#[must_use]
pub fn exclusion_line(row: &CliSetupFacts) -> String {
    format!("/{}", row.allowlist_target)
}

/// Refuses a snapshot that is unsafe to write.
fn reject_unsafe(snapshot: &FileSnapshot) -> Result<(), Refusal> {
    if let Some(link) = &snapshot.symlink_component {
        return Err(Refusal::Symlink(link.clone()));
    }
    if snapshot.escapes_anchor {
        return Err(Refusal::Escapes(snapshot.path.clone()));
    }
    if snapshot.tracked {
        return Err(Refusal::Tracked(snapshot.path.clone()));
    }
    Ok(())
}

/// Parses an existing JSON target, refusing anything git-paw cannot merge
/// into. An absent file parses as an empty object.
fn parse_target(snapshot: &FileSnapshot) -> Result<serde_json::Value, Refusal> {
    let Some(content) = &snapshot.content else {
        return Ok(serde_json::Value::Object(serde_json::Map::new()));
    };
    if content.trim().is_empty() {
        return Ok(serde_json::Value::Object(serde_json::Map::new()));
    }
    let value: serde_json::Value =
        serde_json::from_str(content).map_err(|e| Refusal::MalformedJson {
            path: snapshot.path.clone(),
            detail: e.to_string(),
        })?;
    if !value.is_object() {
        return Err(Refusal::NonObjectRoot {
            path: snapshot.path.clone(),
        });
    }
    Ok(value)
}

/// Returns the existing array at `key_path`, refusing a wrong-typed container.
///
/// A missing container is not an error — it is created on write.
fn existing_array<'v>(
    value: &'v serde_json::Value,
    path: &Path,
    key_path: &[&str],
) -> Result<Option<&'v Vec<serde_json::Value>>, Refusal> {
    let mut cursor = value;
    for (depth, key) in key_path.iter().enumerate() {
        let Some(obj) = cursor.as_object() else {
            return Err(Refusal::WrongType {
                path: path.to_path_buf(),
                key: key_path[..depth].join("."),
                found: type_name(cursor).to_string(),
            });
        };
        let Some(next) = obj.get(*key) else {
            return Ok(None);
        };
        cursor = next;
    }
    match cursor.as_array() {
        Some(items) => Ok(Some(items)),
        None => Err(Refusal::WrongType {
            path: path.to_path_buf(),
            key: key_path.join("."),
            found: type_name(cursor).to_string(),
        }),
    }
}

/// Returns a JSON value's type name, for a type-mismatch message.
fn type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "list",
        serde_json::Value::Object(_) => "object",
    }
}

/// Appends a planned entry for each wanted value the file does not already
/// hold at `key_path`.
fn push_missing(
    out: &mut Vec<PlannedEntry>,
    existing: &serde_json::Value,
    key_path: &[&str],
    wanted: &[String],
) {
    // `plan` refuses a wrong-typed container before reaching here, so an
    // unreadable container can only mean "absent" — nothing is present yet.
    let present: Vec<&str> = match existing_array(existing, Path::new(""), key_path) {
        Ok(Some(items)) => items.iter().filter_map(serde_json::Value::as_str).collect(),
        _ => Vec::new(),
    };
    for value in wanted {
        if present.contains(&value.as_str()) {
            continue;
        }
        out.push(PlannedEntry::Json {
            key_path: key_path.iter().map(|k| (*k).to_string()).collect(),
            value: value.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli_setup::facts::CLAUDE;

    const ROOT: &str = "/repo";

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

    fn with_target(content: &str) -> Snapshots {
        Snapshots {
            target: FileSnapshot::present(target_path(), content),
            ..snapshots()
        }
    }

    fn unverified() -> CliSetupFacts {
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

    /// Every value the plan would add, across every file.
    fn planned_values(p: &SetupPlan) -> Vec<String> {
        p.files
            .iter()
            .flat_map(|f| f.entries.iter())
            .map(|e| e.value().to_string())
            .collect()
    }

    /// Every value the plan would add under one key path.
    fn planned_under(p: &SetupPlan, key: &str) -> Vec<String> {
        p.files
            .iter()
            .flat_map(|f| f.entries.iter())
            .filter(|e| e.key_path_display().as_deref() == Some(key))
            .map(|e| e.value().to_string())
            .collect()
    }

    // -----------------------------------------------------------------------
    // Unsupported items (spec: "Trust is not supported for claude", "A mixed
    // request writes nothing", "A CLI without a fact row supports no item")
    // -----------------------------------------------------------------------

    #[test]
    fn a_single_unsupported_item_plans_nothing() {
        let outcome = plan(&CLAUDE, &[SetupItem::Trust], &snapshots()).expect("not a refusal");
        assert_eq!(
            outcome,
            PlanOutcome::Unsupported {
                cli: "claude".into(),
                items: vec![SetupItem::Trust],
            }
        );
    }

    #[test]
    fn a_mixed_request_plans_nothing_at_all() {
        // --allowlist --hooks: the supported item is not planned either.
        let outcome = plan(
            &CLAUDE,
            &[SetupItem::Allowlist, SetupItem::Hooks],
            &snapshots(),
        )
        .expect("not a refusal");
        assert_eq!(
            outcome,
            PlanOutcome::Unsupported {
                cli: "claude".into(),
                items: vec![SetupItem::Hooks],
            },
            "a mixed request is all-or-nothing"
        );
    }

    #[test]
    fn a_row_declaring_nothing_supports_no_item() {
        let row = CliSetupFacts {
            identity: "my-agent",
            items: &[],
            ..CLAUDE
        };
        let outcome = plan(&row, &[SetupItem::Allowlist], &snapshots()).expect("not a refusal");
        assert_eq!(
            outcome,
            PlanOutcome::Unsupported {
                cli: "my-agent".into(),
                items: vec![SetupItem::Allowlist],
            }
        );
    }

    // -----------------------------------------------------------------------
    // Verification status (spec: "An unverified item can be planned")
    // -----------------------------------------------------------------------

    #[test]
    fn an_unverified_row_still_plans_but_is_marked() {
        let p = plan_ok(&unverified(), &snapshots());
        assert!(!p.verified, "the plan carries the plan-only flag");
        assert!(!p.is_empty(), "and still describes what it would write");
    }

    #[test]
    fn a_verified_row_is_marked_with_what_it_was_verified_against() {
        let p = plan_ok(&CLAUDE, &snapshots());
        assert!(p.verified);
        assert!(p.verified_with.contains("2.1.220"), "{}", p.verified_with);
    }

    // -----------------------------------------------------------------------
    // The planned entries (spec: "The allow entries are exactly the grant
    // table", "Protection is planned with the grants")
    // -----------------------------------------------------------------------

    #[test]
    fn allow_entries_are_exactly_the_grant_table() {
        let p = plan_ok(&CLAUDE, &snapshots());
        assert_eq!(
            planned_under(&p, "permissions.allow"),
            facts::planned_allow_entries()
        );
        assert_eq!(
            planned_under(&p, "permissions.allow").len(),
            11,
            "the eleven grant-table entries"
        );
    }

    #[test]
    fn protection_is_planned_with_the_grants_in_one_file() {
        let p = plan_ok(&CLAUDE, &snapshots());
        assert!(
            planned_under(&p, "permissions.deny")
                .contains(&"Edit(**/.git-paw/scripts/**)".to_string())
        );
        assert!(
            planned_under(&p, "permissions.ask")
                .contains(&"Edit(**/.git-paw/config.toml)".to_string())
        );
        assert!(
            planned_under(&p, "permissions.ask")
                .contains(&"Bash(.git-paw/scripts/sweep.sh approve *)".to_string())
        );

        // All of it lands in the one settings file, in a single replacement.
        let settings = p
            .files
            .iter()
            .find(|f| f.path == target_path())
            .expect("the settings file is planned");
        assert_eq!(
            settings.entries.len(),
            p.entry_count() - 1,
            "only the exclusion line lives in another file"
        );
    }

    #[test]
    fn the_only_settings_target_is_the_main_checkouts_file() {
        let p = plan_ok(&CLAUDE, &snapshots());
        assert_eq!(p.files[0].path, target_path());
        assert!(p.files[0].created, "it does not exist yet");
        for file in &p.files {
            let shown = file.path.display().to_string();
            assert!(
                !shown.contains(".git-paw/worktrees"),
                "no worktree path may be a target: {shown}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Already-present entries (spec: "An entry the user already had is not
    // claimed", "A protection rule the user already has is not claimed",
    // "Nothing to do")
    // -----------------------------------------------------------------------

    #[test]
    fn an_entry_already_present_is_not_planned() {
        let snaps =
            with_target(r#"{"permissions":{"allow":["Bash(.git-paw/scripts/broker.sh *)"]}}"#);
        let p = plan_ok(&CLAUDE, &snaps);
        let allow = planned_under(&p, "permissions.allow");
        assert!(
            !allow.contains(&"Bash(.git-paw/scripts/broker.sh *)".to_string()),
            "the user's own entry is not re-planned, so undo can never claim it"
        );
        assert_eq!(allow.len(), 10, "the other ten are still planned");
    }

    #[test]
    fn a_protection_rule_already_present_is_not_planned() {
        let snaps = with_target(r#"{"permissions":{"deny":["Edit(**/.git-paw/scripts/**)"]}}"#);
        let p = plan_ok(&CLAUDE, &snaps);
        assert!(
            planned_under(&p, "permissions.deny").is_empty(),
            "already there, so not claimed"
        );
    }

    #[test]
    fn nothing_to_do_when_every_entry_is_present() {
        let mut all = facts::planned_allow_entries();
        let ask = facts::planned_ask_entries();
        let deny = facts::planned_deny_entries();
        let body = serde_json::json!({
            "permissions": { "allow": all, "ask": ask, "deny": deny }
        });
        all.clear();
        let snaps = Snapshots {
            target: FileSnapshot {
                ignored: true,
                ..FileSnapshot::present(target_path(), body.to_string())
            },
            ..snapshots()
        };
        let p = plan_ok(&CLAUDE, &snaps);
        assert!(
            p.is_empty(),
            "nothing left to write: {:?}",
            planned_values(&p)
        );
    }

    // -----------------------------------------------------------------------
    // The exclusion line (spec: "A new file is kept out of version control")
    // -----------------------------------------------------------------------

    #[test]
    fn the_exclusion_line_is_planned_when_the_target_is_not_ignored() {
        let p = plan_ok(&CLAUDE, &snapshots());
        let exclude = p
            .files
            .iter()
            .find(|f| f.path == exclude_path())
            .expect("the exclude file is planned");
        assert_eq!(
            exclude.entries,
            vec![PlannedEntry::Line {
                line: "/.claude/settings.local.json".into()
            }]
        );
    }

    #[test]
    fn no_exclusion_line_when_git_already_ignores_the_target() {
        let snaps = Snapshots {
            target: FileSnapshot {
                ignored: true,
                ..FileSnapshot::absent(target_path())
            },
            ..snapshots()
        };
        let p = plan_ok(&CLAUDE, &snaps);
        assert!(p.files.iter().all(|f| f.path != exclude_path()));
    }

    #[test]
    fn the_exclusion_line_is_not_planned_twice() {
        let snaps = Snapshots {
            exclude: FileSnapshot::present(
                exclude_path(),
                "# comment\n/.claude/settings.local.json\n",
            ),
            ..snapshots()
        };
        let p = plan_ok(&CLAUDE, &snaps);
        assert!(p.files.iter().all(|f| f.path != exclude_path()));
    }

    // -----------------------------------------------------------------------
    // Refusals (spec: "Unsafe targets are refused")
    // -----------------------------------------------------------------------

    #[test]
    fn a_tracked_target_is_refused_naming_the_file() {
        let snaps = Snapshots {
            target: FileSnapshot {
                tracked: true,
                ..FileSnapshot::present(target_path(), "{}")
            },
            ..snapshots()
        };
        let err = plan(&CLAUDE, &[SetupItem::Allowlist], &snaps).unwrap_err();
        assert_eq!(err, Refusal::Tracked(target_path()));
        assert!(err.to_string().contains("settings.local.json"));
    }

    #[test]
    fn a_symlinked_component_is_refused_naming_the_link() {
        let link = PathBuf::from("/repo/.claude");
        let snaps = Snapshots {
            target: FileSnapshot {
                symlink_component: Some(link.clone()),
                ..FileSnapshot::absent(target_path())
            },
            ..snapshots()
        };
        let err = plan(&CLAUDE, &[SetupItem::Allowlist], &snaps).unwrap_err();
        assert_eq!(err, Refusal::Symlink(link));
        assert!(err.to_string().contains(".claude"));
    }

    #[test]
    fn a_path_escaping_the_repository_is_refused() {
        let snaps = Snapshots {
            target: FileSnapshot {
                escapes_anchor: true,
                ..FileSnapshot::absent(target_path())
            },
            ..snapshots()
        };
        assert_eq!(
            plan(&CLAUDE, &[SetupItem::Allowlist], &snaps).unwrap_err(),
            Refusal::Escapes(target_path())
        );
    }

    #[test]
    fn malformed_json_is_refused_naming_the_file() {
        let snaps = with_target("{ not json");
        let err = plan(&CLAUDE, &[SetupItem::Allowlist], &snaps).unwrap_err();
        assert!(matches!(err, Refusal::MalformedJson { ref path, .. } if *path == target_path()));
        assert!(err.to_string().contains("settings.local.json"));
    }

    #[test]
    fn a_non_object_root_is_refused() {
        let snaps = with_target("[1, 2, 3]");
        assert_eq!(
            plan(&CLAUDE, &[SetupItem::Allowlist], &snaps).unwrap_err(),
            Refusal::NonObjectRoot {
                path: target_path()
            }
        );
    }

    #[test]
    fn a_wrong_typed_key_is_refused_naming_file_and_key() {
        let snaps = with_target(r#"{"permissions":{"allow":"everything"}}"#);
        let err = plan(&CLAUDE, &[SetupItem::Allowlist], &snaps).unwrap_err();
        assert_eq!(
            err,
            Refusal::WrongType {
                path: target_path(),
                key: "permissions.allow".into(),
                found: "string".into(),
            }
        );
        let msg = err.to_string();
        assert!(msg.contains("settings.local.json"), "{msg}");
        assert!(msg.contains("permissions.allow"), "{msg}");
    }

    #[test]
    fn a_wrong_typed_parent_container_is_refused_naming_the_parent() {
        let snaps = with_target(r#"{"permissions":"all"}"#);
        let err = plan(&CLAUDE, &[SetupItem::Allowlist], &snaps).unwrap_err();
        assert_eq!(
            err,
            Refusal::WrongType {
                path: target_path(),
                key: "permissions".into(),
                found: "string".into(),
            }
        );
    }

    #[test]
    fn a_refused_run_plans_nothing_for_any_item() {
        let snaps = with_target("{ not json");
        assert!(plan(&CLAUDE, &[SetupItem::Allowlist], &snaps).is_err());
    }

    // -----------------------------------------------------------------------
    // Independence from configuration (spec: "Entries do not depend on
    // configuration")
    // -----------------------------------------------------------------------

    #[test]
    fn two_differently_configured_repositories_plan_identical_entries() {
        // The planner takes no configuration at all, so the only thing that
        // could differ is the repository path. Entries must not vary with it.
        let other = Snapshots {
            target: FileSnapshot::absent("/elsewhere/.claude/settings.local.json"),
            exclude: FileSnapshot::absent("/elsewhere/.git/info/exclude"),
            record_path: PathBuf::from("/elsewhere/.git/git-paw/cli-setup.json"),
        };
        assert_eq!(
            planned_values(&plan_ok(&CLAUDE, &snapshots())),
            planned_values(&plan_ok(&CLAUDE, &other))
        );
    }

    #[test]
    fn the_plan_names_the_record_file() {
        assert_eq!(plan_ok(&CLAUDE, &snapshots()).record_path, record_path());
    }

    #[test]
    fn an_empty_target_file_is_treated_as_an_empty_object() {
        let p = plan_ok(&CLAUDE, &with_target("   \n"));
        assert_eq!(planned_under(&p, "permissions.allow").len(), 11);
    }

    #[test]
    fn unrelated_keys_do_not_affect_what_is_planned() {
        let snaps = with_target(r#"{"model":"x","env":{"API_TOKEN":"s3cret"}}"#);
        assert_eq!(
            planned_values(&plan_ok(&CLAUDE, &snaps)),
            planned_values(&plan_ok(&CLAUDE, &snapshots())),
        );
        let _ = ROOT;
    }
}
