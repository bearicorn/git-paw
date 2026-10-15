//! The setup record: what git-paw wrote, so it can take back exactly that.
//!
//! The record lives at `<git-common-dir>/git-paw/cli-setup.json`. The git
//! directory can never be committed, is invisible to every worktree's
//! `git status`, and is shared by all worktrees of the repository — so one
//! record covers the whole repository and no working tree ever shows it.
//!
//! An entry is recorded **before** the write it describes. A crash in between
//! leaves a recorded entry that is not on disk, which undo simply drops; the
//! reverse order would leave an entry on disk that undo could never find.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::facts::SetupItem;

/// Current schema version of the record file.
pub const SCHEMA_VERSION: u32 = 1;

/// Path of the record inside a repository's common git directory.
#[must_use]
pub fn record_path(git_common_dir: &Path) -> PathBuf {
    git_common_dir.join("git-paw").join("cli-setup.json")
}

/// What kind of entry a record entry describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EntryKind {
    /// An item in the CLI's allow list.
    Allow,
    /// An item in the CLI's ask list.
    Ask,
    /// An item in the CLI's deny list.
    Deny,
    /// A line in the repository's shared exclude file.
    ExcludeLine,
}

/// One entry git-paw wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordEntry {
    /// The CLI identity the entry was written for.
    pub cli: String,
    /// The item that wrote it.
    pub item: String,
    /// Absolute path of the file it was written to.
    pub file: PathBuf,
    /// What kind of entry it is.
    pub kind: EntryKind,
    /// Dotted key path, for a JSON entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_path: Option<String>,
    /// The value written, byte-for-byte.
    pub value: String,
    /// Whether git-paw created the file itself.
    pub created_file: bool,
    /// Directories git-paw created on the way to it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub created_dirs: Vec<PathBuf>,
    /// When it was recorded, as an ISO-8601 timestamp.
    pub recorded_at: String,
    /// The git-paw version that wrote it.
    pub git_paw_version: String,
}

impl RecordEntry {
    /// Returns whether this entry belongs to `cli`.
    #[must_use]
    pub fn is_for_cli(&self, cli: &str) -> bool {
        self.cli == cli
    }

    /// Returns whether this entry belongs to `item`.
    #[must_use]
    pub fn is_for_item(&self, item: SetupItem) -> bool {
        self.item == item.as_str()
    }
}

/// The record file's contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupRecord {
    /// Schema version, so a later version can migrate this file.
    pub schema_version: u32,
    /// Every entry git-paw has written and not yet taken back.
    #[serde(default)]
    pub entries: Vec<RecordEntry>,
}

impl Default for SetupRecord {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            entries: Vec::new(),
        }
    }
}

impl SetupRecord {
    /// Returns the entries recorded for `cli`, optionally narrowed to
    /// `items` when the caller named some.
    #[must_use]
    pub fn entries_for(&self, cli: &str, items: &[SetupItem]) -> Vec<&RecordEntry> {
        self.entries
            .iter()
            .filter(|e| e.is_for_cli(cli))
            .filter(|e| items.is_empty() || items.iter().any(|i| e.is_for_item(*i)))
            .collect()
    }
}
