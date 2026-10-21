//! Session surfaces: the recorded pane layout and the role each surface plays.
//!
//! A git-paw session arranges its work across *surfaces* — today tmux panes.
//! Which index holds which surface is a property of the **session**, not of
//! any one consumer, so this module owns every reserved-index fact in one
//! place. No orchestrator CLI runs inside a session: the orchestrator drives
//! from outside tmux, so a window holds a dashboard (when it has one) and
//! coding agents only.
//!
//! | layout | dashboard | first coding agent |
//! |---|---|---|
//! | supervised mode | 0 | [`SUPERVISOR_PANE_OFFSET`] (1) |
//! | bare mode, broker enabled | 0 | 1 |
//! | bare mode, no broker | — | 0 |
//!
//! The layout is read from the RECORDED `Session::dashboard_pane`: `Some(n)`
//! puts the dashboard at `n` and the first coding agent at `n + 1`, so a
//! session saved by a pre-relocation binary (`Some(1)`: supervisor 0,
//! dashboard 1, agents from 2) keeps resolving while it is still alive.
//! `None` falls back to the earlier rules: a supervised session's agents start
//! at 2 (after the old supervisor and dashboard panes), a bare session's at 1
//! with a broker and 0 without.
//!
//! [`SessionLayout`] is that record, derived from a saved
//! [`Session`](crate::session::Session) or from a launch plan before one
//! exists. [`SurfaceRole`] is the closed enum a single resolver
//! ([`SessionLayout::resolve`]) produces for a surface. Decision code matches
//! on the role; it does not compare a pane index to a literal, and it does not
//! re-derive a role by comparing an agent id against the broker wire identity
//! [`SUPERVISOR_AGENT_ID`].
//!
//! The grid geometry for the supervised window ([`SupervisorLayout`],
//! [`supervisor_layout`], [`layout_for`]) lives here too, so the tmux adapter
//! can compute pane targets without depending on `crate::supervisor`.

use std::path::{Path, PathBuf};

use crate::error::PawError;
use crate::session::{Session, SessionMode};

// ---------------------------------------------------------------------------
// Wire identity
// ---------------------------------------------------------------------------

/// The broker wire identity of the orchestrator (`supervisor`).
///
/// No surface carries it: the orchestrator runs outside the session. It
/// survives as a *sender* and a *roster row* — the drive loop publishes its
/// escalations and branch-refresh feedback as `supervisor`, the bundled
/// helper's verdict verbs do too, and the broker seeds that row's CLI column.
/// This is the single source of the string; a comparison against it
/// identifies a message sender, a roster row or a pane label, never the role
/// of a surface (that is [`SessionLayout::resolve`]).
pub const SUPERVISOR_AGENT_ID: &str = "supervisor";

// ---------------------------------------------------------------------------
// Grid geometry
// ---------------------------------------------------------------------------

/// Maximum agents per supervised session. Above this, the launch is rejected
/// with an actionable "split into multiple sessions" error. Configurable
/// extension deferred to v1.0.0 (issue #17).
pub const SUPERVISOR_MAX_AGENTS: usize = 25;

/// Agents per agent-grid row. Hard-coded; configurable in v1.0.0.
pub const SUPERVISOR_AGENTS_PER_ROW: usize = 5;

/// Index of the first coding agent in a supervised window built by this
/// binary: the dashboard is the full-width top row at pane 0, so the agents
/// start at pane 1.
pub const SUPERVISOR_PANE_OFFSET: usize = 1;

/// Index every layout this binary builds reserves for the dashboard. Private:
/// outside this module the answer comes from [`SessionLayout`].
const DASHBOARD_INDEX: usize = 0;

/// Index a pre-relocation supervised window reserved for the dashboard
/// (supervisor at 0). Only a session that recorded no `dashboard_pane` falls
/// back to it.
const LEGACY_SUPERVISED_DASHBOARD_INDEX: usize = 1;

/// First coding-agent index of a pre-relocation supervised window
/// (supervisor 0, dashboard 1). Only a session that recorded no
/// `dashboard_pane` falls back to it.
const LEGACY_SUPERVISED_FIRST_AGENT: usize = 2;

/// Computed layout parameters for a supervised tmux session.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SupervisorLayout {
    /// Number of horizontal rows holding coding agents (excludes the top row).
    pub agent_rows: usize,
    /// Total tmux rows = `agent_rows + 1` (the dashboard row + agent rows).
    pub total_rows: usize,
    /// Height percentage allocated to the top (dashboard) row.
    /// Always 50: the top row is half the session whatever the agent count.
    pub top_row_pct: u8,
    /// Height percentage allocated to each agent row: `50 / agent_rows`, the
    /// even split of the half the top row leaves. `f32` because the split is
    /// fractional for some row counts (3 agent rows land on 16.67%).
    pub agent_row_pct: f32,
}

/// Compute the layout for a supervised session with `agent_count` coding agents.
///
/// The top row is always 50% of the session height; the agent rows split the
/// remaining 50% evenly. Adding agents shrinks the agent rows, never the top row.
///
/// # Errors
/// Returns [`PawError::ConfigError`] when `agent_count > SUPERVISOR_MAX_AGENTS`.
pub fn supervisor_layout(agent_count: usize) -> Result<SupervisorLayout, PawError> {
    if agent_count > SUPERVISOR_MAX_AGENTS {
        return Err(PawError::ConfigError(format!(
            "{agent_count} agents requested; maximum is {SUPERVISOR_MAX_AGENTS} per session.\n\
             \n\
             Split into multiple sessions:\n  \
             git paw start --branches <subset>\n\
             \n\
             (Configurable max_agents is planned for v1.0.0 — see milestone.)"
        )));
    }

    let agent_rows = agent_count.div_ceil(SUPERVISOR_AGENTS_PER_ROW).max(1);
    let total_rows = agent_rows + 1;

    // `agent_rows` is capped at SUPERVISOR_MAX_AGENTS / SUPERVISOR_AGENTS_PER_ROW,
    // so the widening cast is exact.
    #[allow(clippy::cast_precision_loss)]
    let agent_row_pct = 50.0_f32 / agent_rows as f32;

    Ok(SupervisorLayout {
        agent_rows,
        total_rows,
        top_row_pct: 50,
        agent_row_pct,
    })
}

/// Pure grid-geometry function of agent count, named per the add/remove
/// design (D1). The layout builder ([`supervisor_layout`]) is already a pure
/// function of `agent_count`; `layout_for` is the canonical name the
/// `add-branch` / `remove-branch` specs use to make explicit that the same
/// geometry is recomputed for `N → N+1` (add) and `N → N−1` (remove)
/// re-tiling, not just the initial start-time layout.
///
/// # Errors
/// Returns [`PawError::ConfigError`] when `agent_count > SUPERVISOR_MAX_AGENTS`
/// — the same "split into multiple sessions" error `git paw start` surfaces.
pub fn layout_for(agent_count: usize) -> Result<SupervisorLayout, PawError> {
    supervisor_layout(agent_count)
}

// ---------------------------------------------------------------------------
// Surface roles
// ---------------------------------------------------------------------------

/// The role a session surface plays.
///
/// Produced only by [`SessionLayout::resolve`], so every consumer agrees on
/// what a surface is. There is no supervisor role: the orchestrator runs
/// outside the session. Only [`SurfaceRole::Coding`] is ever acted on — no
/// keystroke sender types into the dashboard or an unattributed surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceRole {
    /// The dashboard TUI; a `git-paw __dashboard` process, not an agent CLI,
    /// and never acted on.
    Dashboard,
    /// A coding-agent surface, resolved by its owning worktree path. Carries
    /// the resolved agent id.
    Coding(String),
    /// The surface's owner matched no known worktree and its index is not the
    /// one the layout reserves for the dashboard — including a pre-relocation
    /// session's old supervisor pane. Never acted on.
    Unknown,
}

impl SurfaceRole {
    /// The coding agent's id when this surface is a coding agent — the only
    /// role a keystroke sender acts on — or `None` for every other surface.
    #[must_use]
    pub fn coding_agent(&self) -> Option<&str> {
        match self {
            SurfaceRole::Coding(id) => Some(id.as_str()),
            SurfaceRole::Dashboard | SurfaceRole::Unknown => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Session layout
// ---------------------------------------------------------------------------

/// The reserved-index record for one session's window.
///
/// Derived, never persisted: [`SessionLayout::of`] reads a saved session's
/// mode, broker and recorded dashboard index, and [`SessionLayout::planned`]
/// builds the same value from a launch plan before the session is saved — so
/// the indices a launch *records* are the ones this record *answers*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionLayout {
    mode: SessionMode,
    has_broker: bool,
    /// The dashboard index this session recorded (`Session::dashboard_pane`),
    /// or the one a launch of this shape will record. `None` on a broker-less
    /// bare launch and on sessions saved by earlier binaries that recorded
    /// none — the case the earlier rules cover.
    dashboard_pane: Option<usize>,
}

impl SessionLayout {
    /// The layout of a saved session.
    #[must_use]
    pub fn of(session: &Session) -> Self {
        Self {
            mode: session.mode,
            has_broker: session.broker_port.is_some(),
            dashboard_pane: session.dashboard_pane.map(|p| p as usize),
        }
    }

    /// The layout a launch of this shape produces, before a session exists.
    ///
    /// A supervised launch always builds the dashboard at pane 0, broker or
    /// not, so it records the dashboard unconditionally; a bare launch builds
    /// a dashboard only when the broker is enabled. Recording the supervised
    /// dashboard every time keeps a recorded `None` unambiguous: it means a
    /// broker-less bare session, or a session saved by an earlier binary.
    #[must_use]
    pub fn planned(mode: SessionMode, has_broker: bool) -> Self {
        let dashboard_pane = match mode {
            SessionMode::Supervisor => Some(DASHBOARD_INDEX),
            SessionMode::Bare => has_broker.then_some(DASHBOARD_INDEX),
        };
        Self {
            mode,
            has_broker,
            dashboard_pane,
        }
    }

    /// Index of the dashboard surface for role resolution: what the session
    /// recorded, else the earlier rules (a pre-relocation supervised window
    /// kept its dashboard at 1; a bare window has one only with a broker).
    /// `None` when the layout has no dashboard at all.
    ///
    /// Private: a caller that needs the index a session actually recorded
    /// takes [`Self::recorded_dashboard_pane`], and one that needs a kill or
    /// re-split target takes [`Self::dashboard_index_or_default`].
    fn dashboard_index(&self) -> Option<usize> {
        self.dashboard_pane.or(match self.mode {
            SessionMode::Supervisor => Some(LEGACY_SUPERVISED_DASHBOARD_INDEX),
            SessionMode::Bare => self.has_broker.then_some(DASHBOARD_INDEX),
        })
    }

    /// The dashboard index a session of this layout records in
    /// `Session::dashboard_pane`.
    #[must_use]
    pub fn recorded_dashboard_pane(&self) -> Option<u32> {
        self.dashboard_pane.and_then(|p| u32::try_from(p).ok())
    }

    /// The dashboard index to target when only a recorded value will do, with
    /// the historical `0` fallback for sessions that recorded none (pause's
    /// dashboard kill and the restart-from-pause re-split).
    #[must_use]
    pub fn dashboard_index_or_default(&self) -> u32 {
        self.recorded_dashboard_pane().unwrap_or(0)
    }

    /// Index of the first coding-agent surface — the offset every agent index
    /// is counted from. A recorded dashboard at `n` puts the first agent at
    /// `n + 1`; with none recorded the earlier rules apply (supervised → 2,
    /// bare → 1 with a broker, 0 without).
    #[must_use]
    pub fn first_agent_index(&self) -> usize {
        match self.dashboard_pane {
            Some(dashboard) => dashboard + 1,
            None => match self.mode {
                SessionMode::Supervisor => LEGACY_SUPERVISED_FIRST_AGENT,
                SessionMode::Bare => usize::from(self.has_broker),
            },
        }
    }

    /// Index of the `nth` coding-agent surface (0-based within the agents).
    #[must_use]
    pub fn agent_index(&self, nth: usize) -> usize {
        self.first_agent_index() + nth
    }

    /// Resolves one surface to its [`SurfaceRole`].
    ///
    /// `owners` is the session's coding agents as `(agent_id, worktree root)`
    /// pairs. Coding agents resolve by owning directory FIRST — never by index
    /// or CLI-argument order, because indices are neither alphabetical nor
    /// argument-ordered and drift on layout changes — so a coding worktree
    /// whose surface happens to sit at a reserved index still resolves to
    /// `Coding`. Only then is the layout's dashboard index consulted; any
    /// other surface (a pre-relocation session's old supervisor pane
    /// included) is `Unknown` and never acted on.
    #[must_use]
    pub fn resolve<'a, I>(&self, index: usize, owner_path: &str, owners: I) -> SurfaceRole
    where
        I: IntoIterator<Item = (&'a str, &'a Path)>,
    {
        let candidate = canonical_or_owned(Path::new(owner_path));
        for (agent_id, worktree_root) in owners {
            if paths_match(&candidate, worktree_root) {
                return SurfaceRole::Coding(agent_id.to_string());
            }
        }
        if self.dashboard_index() == Some(index) {
            return SurfaceRole::Dashboard;
        }
        SurfaceRole::Unknown
    }
}

/// Canonicalises `p`, falling back to the path as-given when it cannot be
/// resolved (e.g. it does not exist in a unit test).
fn canonical_or_owned(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

/// Compares a (possibly canonicalised) surface path against a worktree root,
/// tolerating symlink differences by canonicalising the root too.
fn paths_match(owner_path: &Path, worktree_root: &Path) -> bool {
    if owner_path == worktree_root {
        return true;
    }
    canonical_or_owned(worktree_root) == *owner_path
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn owners(pairs: &[(&'static str, &'static str)]) -> Vec<(String, PathBuf)> {
        pairs
            .iter()
            .map(|(id, p)| ((*id).to_string(), PathBuf::from(*p)))
            .collect()
    }

    fn refs(owned: &[(String, PathBuf)]) -> Vec<(&str, &Path)> {
        owned
            .iter()
            .map(|(id, p)| (id.as_str(), p.as_path()))
            .collect()
    }

    fn supervised() -> SessionLayout {
        SessionLayout::planned(SessionMode::Supervisor, true)
    }

    fn bare_with_broker() -> SessionLayout {
        SessionLayout::planned(SessionMode::Bare, true)
    }

    fn bare_without_broker() -> SessionLayout {
        SessionLayout::planned(SessionMode::Bare, false)
    }

    /// A supervised session saved by a pre-relocation binary: supervisor at
    /// 0, dashboard at 1 (recorded), agents from 2.
    fn legacy_supervised(recorded: Option<usize>) -> SessionLayout {
        SessionLayout {
            mode: SessionMode::Supervisor,
            has_broker: recorded.is_some(),
            dashboard_pane: recorded,
        }
    }

    // --- Scenario: Roles resolve from the layout record ---------------------

    #[test]
    fn roles_resolve_from_the_layout_record() {
        let owned = owners(&[("feat-a", "/repo-feat-a"), ("feat-b", "/repo-feat-b")]);
        // (layout, index, owner path) -> expected role. The supervised rows
        // walk 0/1+, the bare-with-broker rows 0/1+, the bare-without-broker
        // rows 0+, and each layout also exercises an unmatched surface.
        let cases: Vec<(&str, SessionLayout, usize, &str, SurfaceRole)> = vec![
            (
                "supervised",
                supervised(),
                0,
                "/repo",
                SurfaceRole::Dashboard,
            ),
            (
                "supervised",
                supervised(),
                1,
                "/repo-feat-a",
                SurfaceRole::Coding("feat-a".to_string()),
            ),
            (
                "supervised",
                supervised(),
                2,
                "/repo-feat-b",
                SurfaceRole::Coding("feat-b".to_string()),
            ),
            (
                "supervised",
                supervised(),
                7,
                "/elsewhere",
                SurfaceRole::Unknown,
            ),
            (
                "bare+broker",
                bare_with_broker(),
                0,
                "/repo",
                SurfaceRole::Dashboard,
            ),
            (
                "bare+broker",
                bare_with_broker(),
                1,
                "/repo-feat-a",
                SurfaceRole::Coding("feat-a".to_string()),
            ),
            (
                "bare+broker",
                bare_with_broker(),
                4,
                "/elsewhere",
                SurfaceRole::Unknown,
            ),
            (
                "bare-no-broker",
                bare_without_broker(),
                0,
                "/repo-feat-a",
                SurfaceRole::Coding("feat-a".to_string()),
            ),
            (
                "bare-no-broker",
                bare_without_broker(),
                0,
                "/repo",
                SurfaceRole::Unknown,
            ),
            (
                "bare-no-broker",
                bare_without_broker(),
                1,
                "/elsewhere",
                SurfaceRole::Unknown,
            ),
        ];

        for (label, layout, index, path, expected) in cases {
            assert_eq!(
                layout.resolve(index, path, refs(&owned)),
                expected,
                "{label} index {index} ({path})"
            );
        }
    }

    #[test]
    fn supervised_layout_yields_dashboard_then_coding_in_index_order() {
        let owned = owners(&[("feat-a", "/repo-feat-a")]);
        let layout = supervised();
        let roles: Vec<SurfaceRole> = [(0, "/repo"), (1, "/repo-feat-a")]
            .into_iter()
            .map(|(i, p)| layout.resolve(i, p, refs(&owned)))
            .collect();
        assert_eq!(
            roles,
            vec![
                SurfaceRole::Dashboard,
                SurfaceRole::Coding("feat-a".to_string()),
            ]
        );
    }

    /// Spec scenario "A pre-relocation live session keeps its agent offset":
    /// a session saved with `dashboard_pane = Some(1)` resolves its agents
    /// from 2, its pane 1 as the dashboard and its old supervisor pane 0 as
    /// `Unknown` — so neither pane 0 nor pane 1 is ever a coding surface.
    #[test]
    fn a_pre_relocation_session_keeps_its_agent_offset_and_acts_on_neither_reserved_pane() {
        let owned = owners(&[("feat-a", "/repo-feat-a")]);
        let layout = legacy_supervised(Some(1));
        assert_eq!(layout.first_agent_index(), 2);
        assert_eq!(layout.agent_index(0), 2);
        assert_eq!(
            layout.resolve(0, "/repo", refs(&owned)),
            SurfaceRole::Unknown
        );
        assert_eq!(
            layout.resolve(1, "/repo", refs(&owned)),
            SurfaceRole::Dashboard
        );
        for index in [0, 1] {
            assert_eq!(
                layout.resolve(index, "/repo", refs(&owned)).coding_agent(),
                None,
                "reserved pane {index} of a legacy session is never a coding surface"
            );
        }
        assert_eq!(
            layout.resolve(2, "/repo-feat-a", refs(&owned)),
            SurfaceRole::Coding("feat-a".to_string())
        );
    }

    // --- Scenario: Coding agents resolve by owner, not index ----------------

    #[test]
    fn coding_agents_resolve_by_owner_not_index() {
        let owned = owners(&[("feat-a", "/repo-feat-a")]);
        // The agent's surface sits at the index the layout reserves for the
        // dashboard (0) and the next one (1); the owner path wins.
        for layout in [supervised(), bare_with_broker()] {
            for index in [0, 1] {
                assert_eq!(
                    layout.resolve(index, "/repo-feat-a", refs(&owned)),
                    SurfaceRole::Coding("feat-a".to_string()),
                    "reserved index {index} owned by a coding worktree"
                );
            }
        }
    }

    /// Only a coding surface names an agent a keystroke sender may act on.
    #[test]
    fn only_a_coding_role_names_an_agent() {
        assert_eq!(
            SurfaceRole::Coding("feat-a".to_string()).coding_agent(),
            Some("feat-a")
        );
        assert_eq!(SurfaceRole::Dashboard.coding_agent(), None);
        assert_eq!(SurfaceRole::Unknown.coding_agent(), None);
    }

    // --- Reserved indices ---------------------------------------------------

    #[test]
    fn reserved_indices_per_layout() {
        // layout -> (dashboard, first agent, recorded dashboard)
        for (label, layout, dashboard, first, recorded) in [
            ("supervised", supervised(), Some(0), 1, Some(0_u32)),
            (
                "supervised without broker",
                SessionLayout::planned(SessionMode::Supervisor, false),
                Some(0),
                1,
                Some(0_u32),
            ),
            ("bare+broker", bare_with_broker(), Some(0), 1, Some(0_u32)),
            ("bare-no-broker", bare_without_broker(), None, 0, None),
        ] {
            assert_eq!(layout.dashboard_index(), dashboard, "{label} dashboard");
            assert_eq!(layout.first_agent_index(), first, "{label} first agent");
            assert_eq!(layout.agent_index(0), first, "{label} agent 0");
            assert_eq!(layout.agent_index(3), first + 3, "{label} agent 3");
            assert_eq!(
                layout.recorded_dashboard_pane(),
                recorded,
                "{label} recorded dashboard_pane"
            );
        }
    }

    /// Spec scenarios "A pre-relocation live session keeps its agent offset"
    /// and "A session without `dashboard_pane` follows the earlier rules": a
    /// recorded dashboard at `n` puts the first agent at `n + 1`; `None`
    /// falls back to the earlier rules (supervised 2, bare 1 with a broker and
    /// 0 without), and the dashboard kill/re-split target keeps its `0`
    /// fallback.
    #[test]
    fn recorded_dashboard_pane_drives_the_dashboard_index_and_its_fallback() {
        for (mode, has_broker, recorded, dashboard, fallback, first) in [
            (
                SessionMode::Supervisor,
                true,
                Some(0_usize),
                Some(0),
                0_u32,
                1,
            ),
            (SessionMode::Supervisor, true, Some(1), Some(1), 1, 2),
            (SessionMode::Supervisor, false, Some(0), Some(0), 0, 1),
            (SessionMode::Supervisor, true, None, Some(1), 0, 2),
            (SessionMode::Supervisor, false, None, Some(1), 0, 2),
            (SessionMode::Bare, true, Some(0), Some(0), 0, 1),
            (SessionMode::Bare, true, None, Some(0), 0, 1),
            (SessionMode::Bare, false, None, None, 0, 0),
        ] {
            let layout = SessionLayout {
                mode,
                has_broker,
                dashboard_pane: recorded,
            };
            assert_eq!(
                layout.dashboard_index(),
                dashboard,
                "{mode:?}/{has_broker}/{recorded:?} dashboard index"
            );
            assert_eq!(
                layout.dashboard_index_or_default(),
                fallback,
                "{mode:?}/{has_broker}/{recorded:?} dashboard fallback"
            );
            assert_eq!(
                layout.first_agent_index(),
                first,
                "{mode:?}/{has_broker}/{recorded:?} first agent index"
            );
        }
    }

    /// Spec scenarios "Supervised start records the dashboard at pane 0" and
    /// "A broker-less supervised start still records the dashboard": every
    /// supervised launch records `Some(0)`, broker or not; only a broker-less
    /// bare launch records `None`.
    #[test]
    fn a_launch_records_the_dashboard_it_builds() {
        for (mode, has_broker, expected) in [
            (SessionMode::Bare, true, Some(0_u32)),
            (SessionMode::Bare, false, None),
            (SessionMode::Supervisor, true, Some(0_u32)),
            (SessionMode::Supervisor, false, Some(0_u32)),
        ] {
            assert_eq!(
                SessionLayout::planned(mode, has_broker).recorded_dashboard_pane(),
                expected,
                "{mode:?} launch with broker={has_broker}"
            );
        }
    }

    #[test]
    fn of_a_saved_session_reads_its_mode_broker_and_recorded_dashboard() {
        use crate::session::{SessionStatus, WorktreeEntry};
        use std::time::SystemTime;

        let base = Session {
            session_name: "paw-proj".to_string(),
            repo_path: PathBuf::from("/repo"),
            project_name: "proj".to_string(),
            created_at: SystemTime::UNIX_EPOCH,
            status: SessionStatus::Active,
            worktrees: Vec::<WorktreeEntry>::new(),
            broker_port: None,
            broker_bind: None,
            broker_log_path: None,
            mode: SessionMode::Bare,
            dashboard_pane: None,
        };

        let supervised_session = Session {
            mode: SessionMode::Supervisor,
            broker_port: Some(9119),
            dashboard_pane: Some(0),
            ..base.clone()
        };
        assert_eq!(
            SessionLayout::of(&supervised_session),
            SessionLayout::planned(SessionMode::Supervisor, true)
        );

        let legacy_session = Session {
            mode: SessionMode::Supervisor,
            broker_port: Some(9119),
            dashboard_pane: Some(1),
            ..base.clone()
        };
        assert_eq!(
            SessionLayout::of(&legacy_session),
            legacy_supervised(Some(1))
        );

        let bare_broker = Session {
            broker_port: Some(9119),
            dashboard_pane: Some(0),
            ..base.clone()
        };
        assert_eq!(
            SessionLayout::of(&bare_broker),
            SessionLayout::planned(SessionMode::Bare, true)
        );

        assert_eq!(
            SessionLayout::of(&base),
            SessionLayout::planned(SessionMode::Bare, false)
        );
    }

    // --- Grid geometry ------------------------------------------------------

    #[test]
    fn supervisor_layout_covers_each_agent_count_bucket() {
        // One row per agent count currently covered:
        // agent_count -> (agent_rows, agent_row_pct). Each row also asserts
        // total_rows == agent_rows + 1 and a top row of 50%. Rows straddle every
        // bucket boundary (lower + upper edge of each row-count tier).
        for (agent_count, expected_rows, expected_agent) in [
            (1, 1, 50.0_f32),
            (5, 1, 50.0),
            (6, 2, 25.0),
            (10, 2, 25.0),
            (11, 3, 50.0 / 3.0),
            (15, 3, 50.0 / 3.0),
            (16, 4, 12.5),
            (20, 4, 12.5),
            (21, 5, 10.0),
            (25, 5, 10.0),
        ] {
            let layout = supervisor_layout(agent_count).expect("layout should compute");
            assert_eq!(
                layout.agent_rows, expected_rows,
                "agent_rows for {agent_count}"
            );
            assert_eq!(
                layout.total_rows,
                expected_rows + 1,
                "total_rows for {agent_count}"
            );
            assert_eq!(
                layout.top_row_pct, 50,
                "top_row_pct for {agent_count} is half the session"
            );
            assert!(
                (layout.agent_row_pct - expected_agent).abs() < 0.01,
                "agent_row_pct for {agent_count}: expected {expected_agent}, got {}",
                layout.agent_row_pct
            );
        }
    }

    #[test]
    fn top_row_stays_half_the_session_as_agents_grow() {
        // Growing the agent count must shrink the agent rows, never the top row.
        let small = supervisor_layout(2).expect("small layout should compute");
        let large = supervisor_layout(25).expect("large layout should compute");

        assert_eq!(small.top_row_pct, large.top_row_pct, "top row is unchanged");
        assert_eq!(small.top_row_pct, 50, "top row is half the session");
        assert!(
            large.agent_row_pct < small.agent_row_pct,
            "only the per-agent-row share shrinks: {} vs {}",
            large.agent_row_pct,
            small.agent_row_pct
        );
    }

    #[test]
    fn agent_rows_evenly_split_the_lower_half() {
        // Whatever the row count, the agent rows sum back to the 50% the top
        // row leaves.
        for agent_count in 1..=SUPERVISOR_MAX_AGENTS {
            let layout = supervisor_layout(agent_count).expect("layout should compute");
            #[allow(clippy::cast_precision_loss)]
            let total = layout.agent_row_pct * layout.agent_rows as f32;
            assert!(
                (total - 50.0).abs() < 0.01,
                "agent rows for {agent_count} should sum to 50%, got {total}"
            );
        }
    }

    #[test]
    fn layout_rejects_26_agents() {
        let err = supervisor_layout(26).expect_err("26 agents should be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("26 agents requested"),
            "error mentions count: {msg}"
        );
        assert!(msg.contains("maximum is 25"), "error mentions max: {msg}");
        assert!(
            msg.contains("--branches"),
            "error suggests --branches workaround: {msg}"
        );
    }

    #[test]
    fn layout_rejects_far_above_cap() {
        let err = supervisor_layout(100).expect_err("100 agents should be rejected");
        assert!(err.to_string().contains("100 agents requested"));
    }

    #[test]
    fn layout_for_matches_supervisor_layout_across_the_range() {
        // layout_for is the D1-named alias; it must be identical to
        // supervisor_layout for every valid count and reject the same way.
        for n in 1..=SUPERVISOR_MAX_AGENTS {
            assert_eq!(
                layout_for(n).expect("layout_for should compute"),
                supervisor_layout(n).expect("supervisor_layout should compute"),
                "layout_for({n}) should match supervisor_layout({n})"
            );
        }
        assert!(
            layout_for(SUPERVISOR_MAX_AGENTS + 1).is_err(),
            "layout_for should reject above the cap like supervisor_layout"
        );
    }
}
