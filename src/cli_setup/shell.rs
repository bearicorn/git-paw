//! The imperative shell around the planning core.
//!
//! Collects the snapshots [`super::plan::plan`] is pure over (git queries go
//! through the [`CommandRunner`] seam) and applies the resulting plan to the
//! filesystem. Every decision lives in the core; this module only reads the
//! world and writes what it was told to.

use std::fs;
use std::path::{Path, PathBuf};

use crate::command_runner::CommandRunner;
use crate::error::PawError;

use super::facts::CliSetupFacts;
use super::plan::{FileSnapshot, PlannedEntry, PlannedFile, SetupPlan, Snapshots};
use super::record::{self, EntryKind, RecordEntry, SetupRecord};
use super::undo::{UndoFile, UndoPlan, UndoRemoval};

/// Mode for files git-paw creates: readable and writable by the owner only.
///
/// A settings file pre-approves commands and the record says what git-paw
/// wrote; neither is anyone else's business.
#[cfg(unix)]
const OWNER_ONLY: u32 = 0o600;

/// Where the repository's git directories are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoLayout {
    /// Root of the main checkout — the directory holding the common git
    /// directory's working tree.
    pub main_checkout: PathBuf,
    /// The common git directory, shared by every worktree.
    pub git_common_dir: PathBuf,
    /// This checkout's own git directory.
    pub git_dir: PathBuf,
}

impl RepoLayout {
    /// Returns whether the current checkout is a linked worktree.
    #[must_use]
    pub fn is_linked_worktree(&self) -> bool {
        self.git_dir != self.git_common_dir
    }
}

/// Resolves the repository layout from `cwd` through the git seam.
///
/// # Errors
///
/// Returns a [`PawError`] when git cannot be run or `cwd` is not inside a
/// repository.
pub fn resolve_layout(runner: &dyn CommandRunner, cwd: &Path) -> Result<RepoLayout, PawError> {
    let git_dir = rev_parse(runner, cwd, "--absolute-git-dir")?;
    let git_common_dir = canonical(&rev_parse_common(runner, cwd, &git_dir)?);
    let git_dir = canonical(&git_dir);
    // The main checkout is the common git directory's parent — true for a
    // normal `.git` directory and for the `--separate-git-dir` layout, where
    // git records the real worktree in the common dir's `gitdir` pointer.
    let main_checkout = git_common_dir
        .parent()
        .ok_or_else(|| {
            PawError::ConfigError(format!(
                "cannot locate the main checkout above {}",
                git_common_dir.display()
            ))
        })?
        .to_path_buf();
    Ok(RepoLayout {
        main_checkout,
        git_common_dir,
        git_dir,
    })
}

/// Refuses to continue when `cwd` is inside a linked worktree.
///
/// Defence in depth: every coding agent runs in a linked worktree, so this
/// makes the common accidental invocation fail loudly.
///
/// # Errors
///
/// Returns a [`PawError`] naming the main checkout when the current checkout
/// is a linked worktree.
pub fn require_main_checkout(layout: &RepoLayout) -> Result<(), PawError> {
    if layout.is_linked_worktree() {
        return Err(PawError::ConfigError(format!(
            "setup-cli must run from the repository's main checkout, not a linked worktree — \
             re-run it from {}",
            layout.main_checkout.display()
        )));
    }
    Ok(())
}

/// Runs `git rev-parse <flag>` in `cwd` and returns the single-line answer.
fn rev_parse(runner: &dyn CommandRunner, cwd: &Path, flag: &str) -> Result<PathBuf, PawError> {
    let cwd = cwd.to_string_lossy().to_string();
    let out = runner
        .run("git", &["-C", &cwd, "rev-parse", flag])
        .map_err(|e| PawError::ConfigError(format!("cannot run git: {e}")))?;
    if !out.success {
        return Err(PawError::NotAGitRepo);
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        return Err(PawError::NotAGitRepo);
    }
    Ok(PathBuf::from(text))
}

/// Runs `git rev-parse --git-common-dir`, resolving its answer against the
/// checkout's git directory when git reports a relative path.
fn rev_parse_common(
    runner: &dyn CommandRunner,
    cwd: &Path,
    git_dir: &Path,
) -> Result<PathBuf, PawError> {
    let common = rev_parse(runner, cwd, "--git-common-dir")?;
    if common.is_absolute() {
        return Ok(common);
    }
    // `--git-common-dir` answers `.git` (or a relative path) in a plain
    // checkout; resolve it the way git does, against the checkout.
    if common == Path::new(".git") {
        return Ok(git_dir.to_path_buf());
    }
    Ok(cwd.join(common))
}

/// Canonicalises a path, falling back to the original when it cannot be
/// resolved (a path that does not exist yet is not an error here).
fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Collects the snapshots the planner needs.
///
/// # Errors
///
/// Returns a [`PawError`] when a git query cannot be run or a file cannot be
/// read.
pub fn collect_snapshots(
    runner: &dyn CommandRunner,
    layout: &RepoLayout,
    row: &CliSetupFacts,
) -> Result<Snapshots, PawError> {
    let target_path = layout.main_checkout.join(row.allowlist_target);
    let exclude_path = layout.git_common_dir.join("info").join("exclude");

    let mut target = snapshot_file(&target_path, &layout.main_checkout)?;
    target.tracked = is_tracked(runner, &layout.main_checkout, &target_path);
    target.ignored = is_ignored(runner, &layout.main_checkout, &target_path);

    let exclude = snapshot_file(&exclude_path, &layout.git_common_dir)?;

    Ok(Snapshots {
        target,
        exclude,
        record_path: record::record_path(&layout.git_common_dir),
    })
}

/// Snapshots one file, anchored at `anchor`.
///
/// Reports the first symlinked component between `anchor` and the file, and
/// whether the resolved path escapes `anchor` — the two ways a planted link
/// could redirect a write outside the repository.
fn snapshot_file(path: &Path, anchor: &Path) -> Result<FileSnapshot, PawError> {
    let mut snapshot = FileSnapshot::absent(path);

    snapshot.symlink_component = first_symlink_between(anchor, path);
    snapshot.missing_dirs = missing_dirs(path);

    // Resolve as far as the filesystem allows; a path that does not exist yet
    // is judged by its deepest existing ancestor.
    let probe = existing_ancestor(path);
    if let (Ok(resolved), Ok(anchor_real)) = (fs::canonicalize(&probe), fs::canonicalize(anchor)) {
        snapshot.escapes_anchor = !resolved.starts_with(&anchor_real);
    }

    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_file() => {
            snapshot.content = Some(fs::read_to_string(path).map_err(|e| {
                PawError::ConfigError(format!("cannot read {}: {e}", path.display()))
            })?);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                snapshot.mode = Some(meta.permissions().mode());
            }
        }
        // A symlinked file itself is reported through `symlink_component`.
        _ => {}
    }
    Ok(snapshot)
}

/// Returns the first symlinked path component strictly below `anchor`, up to
/// and including `path` itself.
fn first_symlink_between(anchor: &Path, path: &Path) -> Option<PathBuf> {
    let rest = path.strip_prefix(anchor).ok()?;
    let mut cursor = anchor.to_path_buf();
    for component in rest.components() {
        cursor.push(component);
        if fs::symlink_metadata(&cursor).is_ok_and(|m| m.file_type().is_symlink()) {
            return Some(cursor);
        }
    }
    None
}

/// Returns the deepest ancestor of `path` that exists.
fn existing_ancestor(path: &Path) -> PathBuf {
    let mut cursor = path.to_path_buf();
    while !cursor.exists() {
        match cursor.parent() {
            Some(parent) if parent != cursor => cursor = parent.to_path_buf(),
            _ => break,
        }
    }
    cursor
}

/// Returns the directories on the way to `path` that do not exist, outermost
/// first.
fn missing_dirs(path: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut cursor = path.parent();
    while let Some(dir) = cursor {
        if dir.as_os_str().is_empty() || dir.exists() {
            break;
        }
        dirs.push(dir.to_path_buf());
        cursor = dir.parent();
    }
    dirs.reverse();
    dirs
}

/// Returns whether git tracks `path`.
fn is_tracked(runner: &dyn CommandRunner, repo: &Path, path: &Path) -> bool {
    let repo = repo.to_string_lossy().to_string();
    let path = path.to_string_lossy().to_string();
    runner
        .run(
            "git",
            &["-C", &repo, "ls-files", "--error-unmatch", "--", &path],
        )
        .is_ok_and(|o| o.success)
}

/// Returns whether git ignores `path`.
fn is_ignored(runner: &dyn CommandRunner, repo: &Path, path: &Path) -> bool {
    let repo = repo.to_string_lossy().to_string();
    let path = path.to_string_lossy().to_string();
    runner
        .run("git", &["-C", &repo, "check-ignore", "-q", "--", &path])
        .is_ok_and(|o| o.success)
}

// ---------------------------------------------------------------------------
// The record store
// ---------------------------------------------------------------------------

/// Reads the setup record, returning an empty one when it does not exist.
///
/// # Errors
///
/// Returns a [`PawError`] when the file exists but cannot be read or parsed.
pub fn load_record(path: &Path) -> Result<SetupRecord, PawError> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| {
            PawError::ConfigError(format!(
                "cannot read the setup record {}: {e}",
                path.display()
            ))
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(SetupRecord::default()),
        Err(e) => Err(PawError::ConfigError(format!(
            "cannot read the setup record {}: {e}",
            path.display()
        ))),
    }
}

/// Writes the setup record atomically, owner-only.
///
/// # Errors
///
/// Returns a [`PawError`] when the record cannot be serialized or written.
pub fn save_record(path: &Path, rec: &SetupRecord) -> Result<(), PawError> {
    let json = serde_json::to_string_pretty(rec)
        .map_err(|e| PawError::ConfigError(format!("cannot serialize the setup record: {e}")))?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)
            .map_err(|e| PawError::ConfigError(format!("cannot create {}: {e}", dir.display())))?;
    }
    write_atomically(path, &format!("{json}\n"), Some(OWNER_ONLY))
}

// ---------------------------------------------------------------------------
// Applying a plan
// ---------------------------------------------------------------------------

/// What applying a plan did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyOutcome {
    /// Files that were written, in order.
    pub written: Vec<PathBuf>,
    /// How many entries were added in total.
    pub entries_added: usize,
}

/// Applies a plan: records each entry, then writes it.
///
/// Each file is re-read and compared against the snapshot the plan was made
/// from. A file that changed in between stops the run naming it, and no later
/// file is touched — a half-applied plan is better than one that silently
/// clobbers a concurrent write.
///
/// # Errors
///
/// Returns a [`PawError`] when a target changed since the plan was made, or
/// when a file cannot be written.
pub fn apply(
    plan: &SetupPlan,
    snapshots: &Snapshots,
    version: &str,
    now: &str,
) -> Result<ApplyOutcome, PawError> {
    let mut record = load_record(&plan.record_path)?;
    let mut written = Vec::new();
    let mut entries_added = 0;

    for file in &plan.files {
        if file.entries.is_empty() {
            continue;
        }
        let snapshot = snapshot_for(snapshots, &file.path);
        let current = read_optional(&file.path)?;
        if current.as_deref() != snapshot.and_then(|s| s.content.as_deref()) {
            return Err(PawError::ConfigError(format!(
                "{} changed since the plan was made — nothing further was written; re-run to \
                 plan against the current file",
                file.path.display()
            )));
        }

        // Record before writing: a crash in between leaves a recorded entry
        // that is not on disk, which undo drops. The reverse would leave an
        // entry undo could never find.
        for entry in &file.entries {
            record
                .entries
                .push(record_entry_for(plan, file, entry, version, now));
        }
        save_record(&plan.record_path, &record)?;

        let next = render_file(&file.path, current.as_deref(), &file.entries)?;
        let mode = snapshot.and_then(|s| s.mode);
        write_atomically(&file.path, &next, mode.or(Some(OWNER_ONLY)))?;

        entries_added += file.entries.len();
        written.push(file.path.clone());
    }

    Ok(ApplyOutcome {
        written,
        entries_added,
    })
}

/// Returns the snapshot the plan was made from for `path`.
fn snapshot_for<'s>(snapshots: &'s Snapshots, path: &Path) -> Option<&'s FileSnapshot> {
    [&snapshots.target, &snapshots.exclude]
        .into_iter()
        .find(|s| s.path == path)
}

/// Reads a file, returning `None` when it does not exist.
fn read_optional(path: &Path) -> Result<Option<String>, PawError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(PawError::ConfigError(format!(
            "cannot read {}: {e}",
            path.display()
        ))),
    }
}

/// Produces the new content of a file with `entries` added.
fn render_file(
    path: &Path,
    current: Option<&str>,
    entries: &[PlannedEntry],
) -> Result<String, PawError> {
    if entries
        .iter()
        .all(|e| matches!(e, PlannedEntry::Line { .. }))
    {
        return Ok(append_lines(current, entries));
    }
    add_json_entries(path, current, entries)
}

/// Appends each planned line, keeping everything already in the file.
fn append_lines(current: Option<&str>, entries: &[PlannedEntry]) -> String {
    let mut out = current.unwrap_or_default().to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    for entry in entries {
        if let PlannedEntry::Line { line } = entry {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Adds each planned JSON entry, creating missing containers and keeping every
/// existing key, its value and its order.
///
/// Order is preserved by `serde_json`'s `preserve_order` feature: a key
/// git-paw never touched keeps its place in the user's file.
fn add_json_entries(
    path: &Path,
    current: Option<&str>,
    entries: &[PlannedEntry],
) -> Result<String, PawError> {
    let mut root: serde_json::Value = match current {
        Some(text) if !text.trim().is_empty() => serde_json::from_str(text).map_err(|e| {
            PawError::ConfigError(format!("{} is not valid JSON: {e}", path.display()))
        })?,
        _ => serde_json::Value::Object(serde_json::Map::new()),
    };

    for entry in entries {
        let PlannedEntry::Json { key_path, value } = entry else {
            continue;
        };
        let mut cursor = &mut root;
        for key in &key_path[..key_path.len() - 1] {
            let obj = cursor.as_object_mut().ok_or_else(|| {
                PawError::ConfigError(format!(
                    "{} holds an unexpected value at `{key}`",
                    path.display()
                ))
            })?;
            cursor = obj
                .entry(key.clone())
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
        }
        let leaf = &key_path[key_path.len() - 1];
        let obj = cursor.as_object_mut().ok_or_else(|| {
            PawError::ConfigError(format!(
                "{} holds an unexpected value at `{leaf}`",
                path.display()
            ))
        })?;
        let list = obj
            .entry(leaf.clone())
            .or_insert_with(|| serde_json::Value::Array(Vec::new()));
        let items = list.as_array_mut().ok_or_else(|| {
            PawError::ConfigError(format!(
                "{} holds a non-list at `{}`",
                path.display(),
                key_path.join(".")
            ))
        })?;
        // Added items follow the existing ones.
        items.push(serde_json::Value::String(value.clone()));
    }

    // Two-space pretty printing, the format the CLI itself writes.
    let text = serde_json::to_string_pretty(&root)
        .map_err(|e| PawError::ConfigError(format!("cannot serialize {}: {e}", path.display())))?;
    Ok(format!("{text}\n"))
}

/// Builds the record entry for one planned entry.
fn record_entry_for(
    plan: &SetupPlan,
    file: &PlannedFile,
    entry: &PlannedEntry,
    version: &str,
    now: &str,
) -> RecordEntry {
    let (kind, key_path) = match entry {
        PlannedEntry::Line { .. } => (EntryKind::ExcludeLine, None),
        PlannedEntry::Json { key_path, .. } => {
            let kind = match key_path.last().map(String::as_str) {
                Some("ask") => EntryKind::Ask,
                Some("deny") => EntryKind::Deny,
                _ => EntryKind::Allow,
            };
            (kind, Some(key_path.join(".")))
        }
    };
    RecordEntry {
        cli: plan.cli.clone(),
        item: plan.item.as_str().to_string(),
        file: file.path.clone(),
        kind,
        key_path,
        value: entry.value().to_string(),
        created_file: file.created,
        created_dirs: file.created_dirs.clone(),
        recorded_at: now.to_string(),
        git_paw_version: version.to_string(),
    }
}

/// Writes `content` to `path` atomically via a temp file in the same
/// directory plus a rename.
///
/// An existing file keeps its mode; `mode` is applied only to a file git-paw
/// creates.
fn write_atomically(path: &Path, content: &str, mode: Option<u32>) -> Result<(), PawError> {
    let dir = path.parent().ok_or_else(|| {
        PawError::ConfigError(format!("{} has no parent directory", path.display()))
    })?;
    fs::create_dir_all(dir)
        .map_err(|e| PawError::ConfigError(format!("cannot create {}: {e}", dir.display())))?;

    let tmp = dir.join(format!(
        ".{}.git-paw.tmp",
        path.file_name()
            .map_or_else(|| "cli-setup".into(), |n| n.to_string_lossy())
    ));
    fs::write(&tmp, content)
        .map_err(|e| PawError::ConfigError(format!("cannot write {}: {e}", tmp.display())))?;

    #[cfg(unix)]
    if let Some(mode) = mode {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(mode);
        if let Err(e) = fs::set_permissions(&tmp, perms) {
            let _ = fs::remove_file(&tmp);
            return Err(PawError::ConfigError(format!(
                "cannot set permissions on {}: {e}",
                tmp.display()
            )));
        }
    }
    #[cfg(not(unix))]
    let _ = mode;

    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        PawError::ConfigError(format!("cannot replace {}: {e}", path.display()))
    })
}

// ---------------------------------------------------------------------------
// Applying an undo plan
// ---------------------------------------------------------------------------

/// What applying an undo plan did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoOutcome {
    /// Files that were rewritten with entries removed, in order.
    pub files_changed: Vec<PathBuf>,
    /// Files that were deleted because only git-paw's skeleton was left.
    pub files_deleted: Vec<PathBuf>,
    /// Directories that were removed because they were left empty.
    pub dirs_removed: Vec<PathBuf>,
    /// How many entries were removed in total.
    pub entries_removed: usize,
}

/// Applies an undo plan: removes the entries, then rewrites the record.
///
/// Each file is re-read and compared against the snapshot the plan was made
/// from, exactly as [`apply`] does; a file that changed in between stops the
/// run naming it and leaves every later file untouched.
///
/// The record is rewritten **after** the files, the mirror image of [`apply`]'s
/// record-first order. A crash in between then leaves an entry that is gone
/// from disk but still recorded, which the next undo simply reports as absent
/// and drops — whereas dropping it from the record first would leave an entry
/// on disk that undo could never find again.
///
/// # Errors
///
/// Returns a [`PawError`] when a file changed since the plan was made, or when
/// a file cannot be written, deleted or parsed.
pub fn apply_undo(
    plan: &UndoPlan,
    record: &SetupRecord,
    snapshots: &Snapshots,
) -> Result<UndoOutcome, PawError> {
    let mut outcome = UndoOutcome {
        files_changed: Vec::new(),
        files_deleted: Vec::new(),
        dirs_removed: Vec::new(),
        entries_removed: 0,
    };

    for file in &plan.files {
        if file.removals.is_empty() && !file.delete_file {
            continue;
        }
        let snapshot = snapshot_for(snapshots, &file.path);
        let current = read_optional(&file.path)?;
        if current.as_deref() != snapshot.and_then(|s| s.content.as_deref()) {
            return Err(PawError::ConfigError(format!(
                "{} changed since the plan was made — nothing further was removed; re-run to \
                 plan against the current file",
                file.path.display()
            )));
        }
        undo_one_file(file, current.as_deref(), snapshot, &mut outcome)?;
        outcome.entries_removed += file.removals.len();
    }

    save_record(
        &plan.record_path,
        &SetupRecord {
            schema_version: record.schema_version,
            entries: plan.surviving_entries(record),
        },
    )?;

    Ok(outcome)
}

/// Removes one file's entries, or deletes the file when the plan says so.
fn undo_one_file(
    file: &UndoFile,
    current: Option<&str>,
    snapshot: Option<&FileSnapshot>,
    outcome: &mut UndoOutcome,
) -> Result<(), PawError> {
    if file.delete_file {
        if current.is_some() {
            fs::remove_file(&file.path).map_err(|e| {
                PawError::ConfigError(format!("cannot delete {}: {e}", file.path.display()))
            })?;
            outcome.files_deleted.push(file.path.clone());
        }
        outcome
            .dirs_removed
            .extend(remove_empty_dirs(&file.remove_dirs));
        return Ok(());
    }

    let next = remove_from_file(&file.path, current, &file.removals)?;
    let mode = snapshot.and_then(|s| s.mode);
    write_atomically(&file.path, &next, mode.or(Some(OWNER_ONLY)))?;
    outcome.files_changed.push(file.path.clone());
    Ok(())
}

/// Removes each directory that is actually empty, innermost first.
///
/// The planner could not check emptiness — it is pure — so its `remove_dirs`
/// is a candidate list. A directory holding anything the user put there is
/// left alone.
fn remove_empty_dirs(dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut removed = Vec::new();
    for dir in dirs {
        let empty = fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_none());
        if empty && fs::remove_dir(dir).is_ok() {
            removed.push(dir.clone());
        }
    }
    removed
}

/// Produces the new content of a file with `removals` taken out.
fn remove_from_file(
    path: &Path,
    current: Option<&str>,
    removals: &[UndoRemoval],
) -> Result<String, PawError> {
    if removals.iter().all(|r| r.kind == EntryKind::ExcludeLine) {
        return Ok(remove_lines(current, removals));
    }
    remove_json_entries(path, current, removals)
}

/// Drops exactly the recorded lines, keeping every other line in place.
fn remove_lines(current: Option<&str>, removals: &[UndoRemoval]) -> String {
    let mut out = String::new();
    for line in current.unwrap_or_default().lines() {
        if removals.iter().any(|r| r.value == line.trim()) {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Drops exactly the recorded values from their arrays, keeping every other
/// key, value and order intact.
fn remove_json_entries(
    path: &Path,
    current: Option<&str>,
    removals: &[UndoRemoval],
) -> Result<String, PawError> {
    let mut root: serde_json::Value = match current {
        Some(text) if !text.trim().is_empty() => serde_json::from_str(text).map_err(|e| {
            PawError::ConfigError(format!("{} is not valid JSON: {e}", path.display()))
        })?,
        _ => serde_json::Value::Object(serde_json::Map::new()),
    };

    for removal in removals {
        let Some(key_path) = removal.key_path.as_deref() else {
            continue;
        };
        let keys: Vec<&str> = key_path.split('.').collect();
        remove_json_value_at(&mut root, &keys, &removal.value);
    }

    let text = serde_json::to_string_pretty(&root)
        .map_err(|e| PawError::ConfigError(format!("cannot serialize {}: {e}", path.display())))?;
    Ok(format!("{text}\n"))
}

/// Removes `target` from the array at `keys`. A path that is not there is
/// left alone — the planner already reported such an entry as absent.
fn remove_json_value_at(value: &mut serde_json::Value, keys: &[&str], target: &str) {
    match keys.split_first() {
        None => {
            if let Some(items) = value.as_array_mut() {
                items.retain(|v| v.as_str() != Some(target));
            }
        }
        Some((head, rest)) => {
            if let Some(next) = value.as_object_mut().and_then(|o| o.get_mut(*head)) {
                remove_json_value_at(next, rest, target);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli_setup::facts::{self, CLAUDE};
    use crate::cli_setup::plan::{PlanOutcome, plan};
    use crate::command_runner::test_support::FakeCommandRunner;
    use std::process::Command;
    use tempfile::TempDir;

    const NOW: &str = "2026-10-15T12:00:00Z";
    const VERSION: &str = "0.17.0";

    /// A real git repository, so the git seam and the filesystem agree.
    fn repo() -> TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        git(dir.path(), &["init", "-q"]);
        git(dir.path(), &["config", "user.name", "t"]);
        git(dir.path(), &["config", "user.email", "t@example.invalid"]);
        // A developer's global ignore file may already cover `.claude/`, which
        // would make `check-ignore` answer about the machine rather than the
        // repository and silently drop the exclusion line from the plan.
        git(dir.path(), &["config", "core.excludesFile", "/dev/null"]);
        git(dir.path(), &["commit", "-q", "--allow-empty", "-m", "init"]);
        dir
    }

    fn git(cwd: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(cwd)
            .args(args)
            .output()
            .expect("git runs");
        assert!(status.status.success(), "git {args:?} failed");
    }

    fn runner() -> crate::command_runner::RealCommandRunner {
        crate::command_runner::RealCommandRunner
    }

    fn layout_of(root: &Path) -> RepoLayout {
        resolve_layout(&runner(), root).expect("layout resolves")
    }

    fn plan_for(snaps: &Snapshots) -> SetupPlan {
        match plan(&CLAUDE, &[facts::SetupItem::Allowlist], snaps) {
            Ok(PlanOutcome::Plan(p)) => *p,
            other => panic!("expected a plan, got {other:?}"),
        }
    }

    #[cfg(unix)]
    fn mode_of(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path).expect("metadata").permissions().mode() & 0o777
    }

    // -----------------------------------------------------------------------
    // 5.1 Layout and the linked-worktree refusal
    // -----------------------------------------------------------------------

    #[test]
    fn a_plain_checkout_is_not_a_linked_worktree() {
        let dir = repo();
        let layout = layout_of(dir.path());
        assert!(!layout.is_linked_worktree());
        assert!(require_main_checkout(&layout).is_ok());
        assert_eq!(
            fs::canonicalize(&layout.main_checkout).ok(),
            fs::canonicalize(dir.path()).ok()
        );
    }

    #[test]
    fn running_from_a_linked_worktree_is_refused_naming_the_main_checkout() {
        let dir = repo();
        let wt = dir.path().join(".git-paw/worktrees/feat-x");
        fs::create_dir_all(wt.parent().expect("parent")).expect("mkdir");
        git(
            dir.path(),
            &[
                "worktree",
                "add",
                "-q",
                &wt.to_string_lossy(),
                "-b",
                "feat/x",
            ],
        );

        let layout = layout_of(&wt);
        assert!(
            layout.is_linked_worktree(),
            "the worktree has its own git dir"
        );

        let err = require_main_checkout(&layout).expect_err("refused");
        let msg = err.to_string();
        assert!(msg.contains("main checkout"), "{msg}");
        assert!(
            msg.contains(&layout.main_checkout.display().to_string()),
            "the message names where to run it instead: {msg}"
        );
        // Both checkouts share one common git dir, so both resolve the same
        // main checkout and the same record.
        assert_eq!(layout.git_common_dir, layout_of(dir.path()).git_common_dir);
    }

    #[test]
    fn a_directory_outside_a_repository_is_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let runner = FakeCommandRunner::failing("not a git repository");
        assert!(resolve_layout(&runner, dir.path()).is_err());
    }

    // -----------------------------------------------------------------------
    // 5.1 Snapshot fields
    // -----------------------------------------------------------------------

    #[test]
    fn an_absent_target_is_snapshotted_as_absent_with_its_missing_dirs() {
        let dir = repo();
        let snaps =
            collect_snapshots(&runner(), &layout_of(dir.path()), &CLAUDE).expect("snapshots");
        assert!(!snaps.target.exists());
        assert_eq!(snaps.target.content, None);
        assert_eq!(snaps.target.mode, None);
        assert!(
            snaps
                .target
                .missing_dirs
                .iter()
                .any(|d| d.ends_with(".claude")),
            "the .claude directory would have to be created: {:?}",
            snaps.target.missing_dirs
        );
    }

    #[test]
    fn an_existing_target_is_snapshotted_with_its_content_and_mode() {
        let dir = repo();
        let target = dir.path().join(".claude/settings.local.json");
        fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
        fs::write(&target, "{\"model\":\"x\"}\n").expect("write");

        let snaps =
            collect_snapshots(&runner(), &layout_of(dir.path()), &CLAUDE).expect("snapshots");
        assert_eq!(snaps.target.content.as_deref(), Some("{\"model\":\"x\"}\n"));
        #[cfg(unix)]
        assert!(snaps.target.mode.is_some());
        assert!(snaps.target.missing_dirs.is_empty());
    }

    #[test]
    fn a_tracked_target_is_reported_as_tracked() {
        let dir = repo();
        let target = dir.path().join(".claude/settings.local.json");
        fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
        fs::write(&target, "{}\n").expect("write");
        git(dir.path(), &["add", "-f", ".claude/settings.local.json"]);

        let snaps =
            collect_snapshots(&runner(), &layout_of(dir.path()), &CLAUDE).expect("snapshots");
        assert!(snaps.target.tracked);
    }

    #[test]
    fn an_ignored_target_is_reported_as_ignored() {
        let dir = repo();
        fs::write(dir.path().join(".gitignore"), ".claude/\n").expect("write");
        let snaps =
            collect_snapshots(&runner(), &layout_of(dir.path()), &CLAUDE).expect("snapshots");
        assert!(snaps.target.ignored);
        assert!(!snaps.target.tracked);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_settings_directory_is_reported_as_a_symlink() {
        let dir = repo();
        let elsewhere = dir.path().join("elsewhere");
        fs::create_dir_all(&elsewhere).expect("mkdir");
        std::os::unix::fs::symlink(&elsewhere, dir.path().join(".claude")).expect("symlink");

        let snaps =
            collect_snapshots(&runner(), &layout_of(dir.path()), &CLAUDE).expect("snapshots");
        let link = snaps
            .target
            .symlink_component
            .as_ref()
            .expect("the link is reported");
        assert!(link.ends_with(".claude"), "{}", link.display());

        // And the planner turns that into a refusal rather than writing.
        assert!(plan(&CLAUDE, &[facts::SetupItem::Allowlist], &snaps).is_err());
    }

    #[test]
    fn the_record_path_is_inside_the_common_git_directory() {
        let dir = repo();
        let layout = layout_of(dir.path());
        let snaps = collect_snapshots(&runner(), &layout, &CLAUDE).expect("snapshots");
        assert_eq!(
            snaps.record_path,
            layout.git_common_dir.join("git-paw").join("cli-setup.json")
        );
    }

    // -----------------------------------------------------------------------
    // 5.2 The record store
    // -----------------------------------------------------------------------

    #[test]
    fn the_record_round_trips_and_an_absent_one_reads_as_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("git-paw/cli-setup.json");
        assert_eq!(load_record(&path).expect("absent"), SetupRecord::default());

        let mut rec = SetupRecord::default();
        rec.entries.push(RecordEntry {
            cli: "claude".into(),
            item: "allowlist".into(),
            file: PathBuf::from("/repo/.claude/settings.local.json"),
            kind: EntryKind::Allow,
            key_path: Some("permissions.allow".into()),
            value: "Bash(.git-paw/scripts/broker.sh *)".into(),
            created_file: true,
            created_dirs: vec![PathBuf::from("/repo/.claude")],
            recorded_at: NOW.into(),
            git_paw_version: VERSION.into(),
        });
        save_record(&path, &rec).expect("save");
        assert_eq!(load_record(&path).expect("load"), rec);
    }

    #[cfg(unix)]
    #[test]
    fn the_record_is_owner_only() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("git-paw/cli-setup.json");
        save_record(&path, &SetupRecord::default()).expect("save");
        assert_eq!(mode_of(&path), 0o600);
    }

    #[test]
    fn the_record_carries_its_schema_version() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("cli-setup.json");
        save_record(&path, &SetupRecord::default()).expect("save");
        let text = fs::read_to_string(&path).expect("read");
        assert!(text.contains("\"schema_version\""), "{text}");
    }

    #[test]
    fn writing_the_record_leaves_git_status_clean_at_the_root_and_in_a_worktree() {
        let dir = repo();
        let wt = dir.path().join(".git-paw/worktrees/feat-x");
        fs::create_dir_all(wt.parent().expect("parent")).expect("mkdir");
        git(
            dir.path(),
            &[
                "worktree",
                "add",
                "-q",
                &wt.to_string_lossy(),
                "-b",
                "feat/x",
            ],
        );
        let layout = layout_of(dir.path());
        save_record(
            &record::record_path(&layout.git_common_dir),
            &SetupRecord::default(),
        )
        .expect("save");

        for cwd in [dir.path(), wt.as_path()] {
            let out = Command::new("git")
                .arg("-C")
                .arg(cwd)
                .args(["status", "--porcelain"])
                .output()
                .expect("git status");
            let shown = String::from_utf8_lossy(&out.stdout);
            assert!(
                !shown.contains("cli-setup.json"),
                "the record must be invisible to {}: {shown}",
                cwd.display()
            );
        }
    }

    // -----------------------------------------------------------------------
    // 5.3 Applying
    // -----------------------------------------------------------------------

    #[test]
    fn applying_writes_the_planned_entries_and_records_them() {
        let dir = repo();
        let layout = layout_of(dir.path());
        let snaps = collect_snapshots(&runner(), &layout, &CLAUDE).expect("snapshots");
        let p = plan_for(&snaps);

        let outcome = apply(&p, &snaps, VERSION, NOW).expect("apply");
        assert_eq!(outcome.entries_added, p.entry_count());

        let written: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(layout.main_checkout.join(".claude/settings.local.json"))
                .expect("read"),
        )
        .expect("valid json");
        let allow = written["permissions"]["allow"]
            .as_array()
            .expect("allow list");
        assert_eq!(allow.len(), 11);

        let rec = load_record(&p.record_path).expect("record");
        assert_eq!(rec.entries.len(), p.entry_count());
        assert!(rec.entries.iter().all(|e| e.cli == "claude"));
        assert!(rec.entries.iter().all(|e| e.item == "allowlist"));
        assert!(
            rec.entries.iter().any(|e| e.kind == EntryKind::ExcludeLine),
            "the exclusion line is recorded too"
        );
    }

    #[test]
    fn unrelated_settings_keep_their_value_and_their_order() {
        let dir = repo();
        let target = dir.path().join(".claude/settings.local.json");
        fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
        fs::write(
            &target,
            r#"{"model":"x","permissions":{"deny":["y"],"allow":["z"]}}"#,
        )
        .expect("write");

        let layout = layout_of(dir.path());
        let snaps = collect_snapshots(&runner(), &layout, &CLAUDE).expect("snapshots");
        let p = plan_for(&snaps);
        apply(&p, &snaps, VERSION, NOW).expect("apply");

        let text = fs::read_to_string(&target).expect("read");
        let written: serde_json::Value = serde_json::from_str(&text).expect("valid json");

        assert_eq!(written["model"], "x");
        assert_eq!(written["permissions"]["deny"][0], "y");
        let allow: Vec<&str> = written["permissions"]["allow"]
            .as_array()
            .expect("allow")
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect();
        assert_eq!(allow[0], "z", "the user's entry keeps its place");
        assert_eq!(
            allow[1], "Bash(.git-paw/scripts/broker.sh *)",
            "git-paw's entries follow it"
        );

        // Key order is preserved, so `model` still precedes `permissions`.
        let model_at = text.find("\"model\"").expect("model");
        let perms_at = text.find("\"permissions\"").expect("permissions");
        assert!(model_at < perms_at, "existing key order is kept:\n{text}");

        // `model` < `permissions` alphabetically, so the assertion above would
        // also hold under sorted output. This one would not: the fixture nests
        // `deny` before `allow`, the reverse of sorted order.
        assert!(
            text.find("\"deny\"") < text.find("\"allow\""),
            "keys keep insertion order, not alphabetical order - this fails if \
             serde_json's preserve_order feature is ever dropped:\n{text}"
        );
    }

    #[test]
    fn a_concurrent_change_stops_the_run_and_leaves_later_files_untouched() {
        let dir = repo();
        let layout = layout_of(dir.path());
        let snaps = collect_snapshots(&runner(), &layout, &CLAUDE).expect("snapshots");
        let p = plan_for(&snaps);

        // Somebody else writes the settings file between plan and apply.
        let target = layout.main_checkout.join(".claude/settings.local.json");
        fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
        fs::write(&target, "{\"model\":\"changed\"}\n").expect("write");

        let err = apply(&p, &snaps, VERSION, NOW).expect_err("stops");
        let msg = err.to_string();
        assert!(msg.contains("settings.local.json"), "{msg}");
        assert!(msg.contains("changed since"), "{msg}");

        assert_eq!(
            fs::read_to_string(&target).expect("read"),
            "{\"model\":\"changed\"}\n",
            "the changed file is left exactly as it was"
        );
        let exclude = layout.git_common_dir.join("info/exclude");
        let exclude_text = fs::read_to_string(&exclude).unwrap_or_default();
        assert!(
            !exclude_text.contains("/.claude/settings.local.json"),
            "a later file in the plan is not written"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_created_file_is_owner_only_and_an_existing_file_keeps_its_mode() {
        let dir = repo();
        let layout = layout_of(dir.path());
        let snaps = collect_snapshots(&runner(), &layout, &CLAUDE).expect("snapshots");
        apply(&plan_for(&snaps), &snaps, VERSION, NOW).expect("apply");
        let target = layout.main_checkout.join(".claude/settings.local.json");
        assert_eq!(mode_of(&target), 0o600, "a created file is owner-only");

        // Now a second repository whose file already exists with a wider mode.
        let dir2 = repo();
        let target2 = dir2.path().join(".claude/settings.local.json");
        fs::create_dir_all(target2.parent().expect("parent")).expect("mkdir");
        fs::write(&target2, "{}\n").expect("write");
        fs::set_permissions(
            &target2,
            <fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o644),
        )
        .expect("chmod");

        let layout2 = layout_of(dir2.path());
        let snaps2 = collect_snapshots(&runner(), &layout2, &CLAUDE).expect("snapshots");
        apply(&plan_for(&snaps2), &snaps2, VERSION, NOW).expect("apply");
        assert_eq!(mode_of(&target2), 0o644, "an existing file keeps its mode");
    }

    #[test]
    fn the_exclusion_line_is_appended_and_hides_the_created_file() {
        let dir = repo();
        let layout = layout_of(dir.path());
        let snaps = collect_snapshots(&runner(), &layout, &CLAUDE).expect("snapshots");
        apply(&plan_for(&snaps), &snaps, VERSION, NOW).expect("apply");

        let exclude =
            fs::read_to_string(layout.git_common_dir.join("info/exclude")).expect("exclude file");
        assert!(
            exclude
                .lines()
                .any(|l| l.trim() == "/.claude/settings.local.json"),
            "{exclude}"
        );

        let out = Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(["status", "--porcelain"])
            .output()
            .expect("git status");
        let shown = String::from_utf8_lossy(&out.stdout);
        assert!(
            !shown.contains(".claude"),
            "the created file must not show up in git status: {shown}"
        );
    }

    #[test]
    fn an_entry_already_present_is_neither_written_twice_nor_recorded() {
        let dir = repo();
        let target = dir.path().join(".claude/settings.local.json");
        fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
        fs::write(
            &target,
            r#"{"permissions":{"allow":["Bash(.git-paw/scripts/broker.sh *)"]}}"#,
        )
        .expect("write");

        let layout = layout_of(dir.path());
        let snaps = collect_snapshots(&runner(), &layout, &CLAUDE).expect("snapshots");
        let p = plan_for(&snaps);
        apply(&p, &snaps, VERSION, NOW).expect("apply");

        let written: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&target).expect("read")).expect("json");
        let broker_entries = written["permissions"]["allow"]
            .as_array()
            .expect("allow")
            .iter()
            .filter(|v| v.as_str() == Some("Bash(.git-paw/scripts/broker.sh *)"))
            .count();
        assert_eq!(broker_entries, 1, "present exactly once");

        let rec = load_record(&p.record_path).expect("record");
        assert!(
            !rec.entries
                .iter()
                .any(|e| e.value == "Bash(.git-paw/scripts/broker.sh *)"),
            "the user's own entry is never claimed, so undo cannot remove it"
        );
    }

    #[test]
    fn every_entry_is_recorded_before_the_file_is_written() {
        // The record must never lag the disk: an entry on disk that is not in
        // the record could never be undone. Asserted by comparing the record
        // against the file after a successful apply — every written value is
        // recorded.
        let dir = repo();
        let layout = layout_of(dir.path());
        let snaps = collect_snapshots(&runner(), &layout, &CLAUDE).expect("snapshots");
        let p = plan_for(&snaps);
        apply(&p, &snaps, VERSION, NOW).expect("apply");

        let rec = load_record(&p.record_path).expect("record");
        let recorded: Vec<&str> = rec.entries.iter().map(|e| e.value.as_str()).collect();
        for entry in p.files.iter().flat_map(|f| f.entries.iter()) {
            assert!(
                recorded.contains(&entry.value()),
                "{} was written but not recorded",
                entry.value()
            );
        }
        assert!(rec.entries.iter().all(|e| e.recorded_at == NOW));
        assert!(rec.entries.iter().all(|e| e.git_paw_version == VERSION));
    }

    #[test]
    fn a_created_file_is_marked_as_created_in_the_record() {
        let dir = repo();
        let layout = layout_of(dir.path());
        let snaps = collect_snapshots(&runner(), &layout, &CLAUDE).expect("snapshots");
        let p = plan_for(&snaps);
        apply(&p, &snaps, VERSION, NOW).expect("apply");

        let rec = load_record(&p.record_path).expect("record");
        let settings = rec
            .entries
            .iter()
            .find(|e| e.kind == EntryKind::Allow)
            .expect("an allow entry");
        assert!(settings.created_file, "git-paw created the settings file");
        assert!(
            settings.created_dirs.iter().any(|d| d.ends_with(".claude")),
            "and the directory it needed: {:?}",
            settings.created_dirs
        );
    }

    // -----------------------------------------------------------------------
    // Applying an undo (spec: "Undo removes exactly what git-paw added")
    // -----------------------------------------------------------------------

    /// Applies setup in a fresh repository and returns the pieces an undo
    /// needs: the layout, fresh snapshots and the record.
    fn after_setup(dir: &TempDir) -> (RepoLayout, Snapshots, SetupRecord) {
        let layout = layout_of(dir.path());
        let snaps = collect_snapshots(&runner(), &layout, &CLAUDE).expect("snapshots");
        apply(&plan_for(&snaps), &snaps, VERSION, NOW).expect("apply");
        let fresh = collect_snapshots(&runner(), &layout, &CLAUDE).expect("snapshots");
        let record = load_record(&fresh.record_path).expect("record");
        (layout, fresh, record)
    }

    fn undo_plan_for(record: &SetupRecord, snaps: &Snapshots) -> crate::cli_setup::undo::UndoPlan {
        crate::cli_setup::undo::plan_undo(record, &CLAUDE, &[], snaps).expect("undo plans")
    }

    fn allow_list(path: &Path) -> Vec<String> {
        let text = fs::read_to_string(path).unwrap_or_default();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
        v["permissions"]["allow"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn deny_list(path: &Path) -> Vec<String> {
        let text = fs::read_to_string(path).unwrap_or_default();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
        v["permissions"]["deny"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn undo_removes_git_paws_entries_and_leaves_the_users_alone() {
        let dir = repo();
        let target = dir.path().join(".claude/settings.local.json");
        fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
        fs::write(
            &target,
            r#"{"model":"x","permissions":{"allow":["Bash(make test *)"]}}"#,
        )
        .expect("write");

        let (_layout, snaps, record) = after_setup(&dir);
        assert!(allow_list(&target).len() > 1);

        let p = undo_plan_for(&record, &snaps);
        apply_undo(&p, &record, &snaps).expect("undo");

        assert_eq!(
            allow_list(&target),
            vec!["Bash(make test *)".to_string()],
            "only the user's own entry survives"
        );
        let text = fs::read_to_string(&target).expect("read");
        let v: serde_json::Value = serde_json::from_str(&text).expect("json");
        assert_eq!(v["model"], "x", "unrelated keys are untouched");
        assert!(
            load_record(&snaps.record_path)
                .expect("record")
                .entries
                .is_empty(),
            "the record is emptied"
        );
    }

    #[test]
    fn setup_then_undo_restores_every_value_the_user_had() {
        let dir = repo();
        let target = dir.path().join(".claude/settings.local.json");
        fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
        let original = "{\n  \"model\": \"x\",\n  \"permissions\": {\n    \"allow\": [\n      \"Bash(make test *)\"\n    ]\n  }\n}\n";
        fs::write(&target, original).expect("write");

        let (_layout, snaps, record) = after_setup(&dir);
        let p = undo_plan_for(&record, &snaps);
        apply_undo(&p, &record, &snaps).expect("undo");

        let left: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&target).expect("read")).expect("json");
        assert_eq!(left["model"], "x");
        assert_eq!(
            left["permissions"]["allow"]
                .as_array()
                .expect("allow")
                .len(),
            1,
            "every entry git-paw added is gone and the user's one remains"
        );
        assert_eq!(left["permissions"]["allow"][0], "Bash(make test *)");
    }

    #[test]
    fn undo_leaves_behind_the_empty_containers_setup_created() {
        // Deliberate, and the reason it is pinned rather than fixed: undo
        // removes entries, never containers. git-paw cannot tell a list it
        // created and then emptied from one the user keeps deliberately
        // empty, and removing the latter would be modifying content it did
        // not add. An empty allow/ask/deny list grants nothing, so the
        // residue is cosmetic.
        let dir = repo();
        let target = dir.path().join(".claude/settings.local.json");
        fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
        fs::write(
            &target,
            "{\"permissions\":{\"allow\":[\"Bash(make test *)\"]}}",
        )
        .expect("write");

        let (_layout, snaps, record) = after_setup(&dir);
        let p = undo_plan_for(&record, &snaps);
        apply_undo(&p, &record, &snaps).expect("undo");

        let left: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&target).expect("read")).expect("json");
        assert!(
            left["permissions"]["ask"]
                .as_array()
                .is_some_and(Vec::is_empty),
            "the ask container setup created is left, empty"
        );
        assert!(
            left["permissions"]["deny"]
                .as_array()
                .is_some_and(Vec::is_empty)
        );
    }

    #[test]
    fn a_forged_record_entry_is_dropped_but_left_on_disk() {
        let dir = repo();
        let target = dir.path().join(".claude/settings.local.json");
        fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
        fs::write(
            &target,
            r#"{"permissions":{"allow":["Bash(make test *)"]}}"#,
        )
        .expect("write");

        let (_layout, snaps, mut record) = after_setup(&dir);
        // Somebody edits the record to claim the user's own entry.
        let forged = RecordEntry {
            cli: "claude".into(),
            item: "allowlist".into(),
            file: target.clone(),
            kind: EntryKind::Allow,
            key_path: Some("permissions.allow".into()),
            value: "Bash(make test *)".into(),
            created_file: false,
            created_dirs: Vec::new(),
            recorded_at: NOW.into(),
            git_paw_version: VERSION.into(),
        };
        record.entries.push(forged);
        save_record(&snaps.record_path, &record).expect("save");

        let p = undo_plan_for(&record, &snaps);
        apply_undo(&p, &record, &snaps).expect("undo");

        assert!(
            allow_list(&target).contains(&"Bash(make test *)".to_string()),
            "a forged entry can remove nothing"
        );
        assert!(
            !load_record(&snaps.record_path)
                .expect("record")
                .entries
                .iter()
                .any(|e| e.value == "Bash(make test *)"),
            "and it is dropped from the record"
        );
    }

    #[test]
    fn protection_survives_a_remaining_helper_grant() {
        let dir = repo();
        let (_layout, snaps, record) = after_setup(&dir);
        let target = snaps.target.path.clone();

        // The user edits one recorded grant, so it no longer matches.
        let text = fs::read_to_string(&target).expect("read");
        let edited = text.replace(
            "Bash(.git-paw/scripts/broker.sh *)",
            "Bash(.git-paw/scripts/broker.sh*)",
        );
        fs::write(&target, &edited).expect("write");

        let fresh =
            collect_snapshots(&runner(), &layout_of(dir.path()), &CLAUDE).expect("snapshots");
        let p = undo_plan_for(&record, &fresh);
        apply_undo(&p, &record, &fresh).expect("undo");

        assert!(
            allow_list(&target).contains(&"Bash(.git-paw/scripts/broker.sh*)".to_string()),
            "the edited entry is left in place"
        );
        assert!(
            deny_list(&target).contains(&"Edit(**/.git-paw/scripts/**)".to_string()),
            "and its protection stays: {:?}",
            deny_list(&target)
        );
    }

    #[test]
    fn a_skeleton_only_created_file_is_deleted_with_its_directory() {
        let dir = repo();
        let (_layout, snaps, record) = after_setup(&dir);
        let target = snaps.target.path.clone();
        assert!(target.exists());

        let p = undo_plan_for(&record, &snaps);
        let outcome = apply_undo(&p, &record, &snaps).expect("undo");

        assert!(!target.exists(), "the file git-paw created is deleted");
        assert!(outcome.files_deleted.contains(&target));
        assert!(
            !dir.path().join(".claude").exists(),
            "and the directory it created, now empty"
        );
    }

    #[test]
    fn a_created_file_the_user_extended_is_kept() {
        let dir = repo();
        let (_layout, snaps, record) = after_setup(&dir);
        let target = snaps.target.path.clone();

        // The user adds a key of their own after setup ran.
        let text = fs::read_to_string(&target).expect("read");
        let mut v: serde_json::Value = serde_json::from_str(&text).expect("json");
        v["model"] = serde_json::Value::String("x".into());
        fs::write(
            &target,
            format!("{}\n", serde_json::to_string_pretty(&v).expect("ser")),
        )
        .expect("write");

        let fresh =
            collect_snapshots(&runner(), &layout_of(dir.path()), &CLAUDE).expect("snapshots");
        let p = undo_plan_for(&record, &fresh);
        apply_undo(&p, &record, &fresh).expect("undo");

        assert!(target.exists(), "the file is kept");
        let left: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&target).expect("read")).expect("json");
        assert_eq!(left["model"], "x");
    }

    #[test]
    fn another_clis_entries_are_untouched() {
        let dir = repo();
        let (_layout, snaps, mut record) = after_setup(&dir);
        let other = RecordEntry {
            cli: "codex".into(),
            item: "allowlist".into(),
            file: snaps.target.path.clone(),
            kind: EntryKind::Allow,
            key_path: Some("permissions.allow".into()),
            value: "Bash(.git-paw/scripts/broker.sh *)".into(),
            created_file: false,
            created_dirs: Vec::new(),
            recorded_at: NOW.into(),
            git_paw_version: VERSION.into(),
        };
        record.entries.push(other.clone());
        save_record(&snaps.record_path, &record).expect("save");

        let p = undo_plan_for(&record, &snaps);
        apply_undo(&p, &record, &snaps).expect("undo");

        assert!(
            load_record(&snaps.record_path)
                .expect("record")
                .entries
                .contains(&other),
            "the other CLI's entry stays in the record"
        );
    }

    #[test]
    fn a_concurrent_change_stops_the_undo_naming_the_file() {
        let dir = repo();
        let (_layout, snaps, record) = after_setup(&dir);
        let p = undo_plan_for(&record, &snaps);

        fs::write(&snaps.target.path, "{\"model\":\"changed\"}\n").expect("write");

        let err = apply_undo(&p, &record, &snaps).expect_err("stops");
        let msg = err.to_string();
        assert!(msg.contains("settings.local.json"), "{msg}");
        assert_eq!(
            fs::read_to_string(&snaps.target.path).expect("read"),
            "{\"model\":\"changed\"}\n"
        );
    }

    #[test]
    fn a_non_empty_directory_is_not_removed() {
        let dir = repo();
        let (_layout, _snaps, record) = after_setup(&dir);
        // The user leaves something else in the directory git-paw created.
        fs::write(dir.path().join(".claude/notes.md"), "mine\n").expect("write");

        let fresh =
            collect_snapshots(&runner(), &layout_of(dir.path()), &CLAUDE).expect("snapshots");
        let p = undo_plan_for(&record, &fresh);
        let outcome = apply_undo(&p, &record, &fresh).expect("undo");

        assert!(dir.path().join(".claude").exists(), "the directory stays");
        assert!(outcome.dirs_removed.is_empty());
        assert_eq!(
            fs::read_to_string(dir.path().join(".claude/notes.md")).expect("read"),
            "mine\n"
        );
    }

    #[test]
    fn undo_removes_the_exclusion_line_and_keeps_the_rest() {
        let dir = repo();
        let (layout, snaps, record) = after_setup(&dir);
        let exclude = layout.git_common_dir.join("info/exclude");
        let before = fs::read_to_string(&exclude).expect("read");
        assert!(before.contains("/.claude/settings.local.json"));

        let p = undo_plan_for(&record, &snaps);
        apply_undo(&p, &record, &snaps).expect("undo");

        let after = fs::read_to_string(&exclude).expect("read");
        assert!(!after.contains("/.claude/settings.local.json"));
        assert!(
            after.contains("# Lines that start with"),
            "git's own comment block survives: {after}"
        );
    }
}
