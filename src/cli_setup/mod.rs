//! Consented per-CLI setup: the one way git-paw writes into an agent CLI's
//! own settings.
//!
//! `git paw setup-cli` is the only command that writes a permission rule, a
//! trust marker or hook configuration into an agent CLI's settings file, and
//! running it is the consent. Nothing else in git-paw invokes it: not `start`,
//! `add`, `init`, session recovery, the drive loop, the dashboard or the MCP
//! server.
//!
//! The module is split along the functional-core / imperative-shell seam:
//!
//! - [`facts`] is data — the compiled per-CLI fact table saying which setup
//!   items a CLI supports and whether those facts were verified against an
//!   installed copy of that CLI, plus the fixed grant table of entries the
//!   allowlist item writes. Nothing here depends on a repository, on
//!   configuration or on a command argument.
//!
//! Every entry git-paw writes is derived from the fact row and the repository
//! root alone. No entry comes from a command argument, from the consumer's
//! configuration or from repository content, so two differently configured
//! repositories plan byte-identical entries.

pub mod doctor;
pub mod facts;
pub mod plan;
pub mod record;
pub mod render;
pub mod shell;
pub mod undo;
