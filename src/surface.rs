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
use crate::session::{PaneLayout, Session, SessionMode};

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

/// Computed geometry for one session's agent grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SupervisorLayout {
    /// Number of horizontal rows holding coding agents (excludes the top row).
    pub agent_rows: usize,
    /// Total tmux rows: `agent_rows + 1` with a top row, `agent_rows` without.
    pub total_rows: usize,
    /// Whether the window has a full-width top row above the agent grid — that
    /// is, whether the session has a dashboard. Without one the agent rows
    /// start at pane 0 and share the whole window height.
    pub has_top_row: bool,
    /// Height percentage allocated to the top (dashboard) row: 50 with a top
    /// row — half the session whatever the agent count — and 0 without one.
    pub top_row_pct: u8,
    /// Height percentage allocated to each agent row: the even split of
    /// whatever height the top row leaves (`50 / agent_rows` with a top row,
    /// `100 / agent_rows` without). `f32` because the split is fractional for
    /// some row counts (3 agent rows under a top row land on 16.67%).
    pub agent_row_pct: f32,
}

/// Compute the agent-grid geometry for a session with `agent_count` coding
/// agents, with (`has_top_row`) or without a full-width dashboard row above
/// the grid.
///
/// With a top row it takes 50% of the session height and the agent rows split
/// the remaining 50% evenly, so adding agents shrinks the agent rows and never
/// the top row. Without one the agent rows split the full height.
///
/// # Errors
/// Returns [`PawError::ConfigError`] when `agent_count > SUPERVISOR_MAX_AGENTS`.
pub fn supervisor_layout(
    agent_count: usize,
    has_top_row: bool,
) -> Result<SupervisorLayout, PawError> {
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
    let total_rows = agent_rows + usize::from(has_top_row);
    let top_row_pct: u8 = if has_top_row { 50 } else { 0 };

    // `agent_rows` is capped at SUPERVISOR_MAX_AGENTS / SUPERVISOR_AGENTS_PER_ROW,
    // so the widening cast is exact.
    #[allow(clippy::cast_precision_loss)]
    let agent_row_pct = f32::from(100 - top_row_pct) / agent_rows as f32;

    Ok(SupervisorLayout {
        agent_rows,
        total_rows,
        has_top_row,
        top_row_pct,
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
pub fn layout_for(agent_count: usize, has_top_row: bool) -> Result<SupervisorLayout, PawError> {
    supervisor_layout(agent_count, has_top_row)
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
    /// The pane layout this session recorded (`Session::layout`), or the one a
    /// launch of this shape will record. `None` only on a session saved by an
    /// earlier binary — see [`Self::is_legacy_tiled`].
    pane_layout: Option<PaneLayout>,
}

impl SessionLayout {
    /// The layout of a saved session.
    #[must_use]
    pub fn of(session: &Session) -> Self {
        Self {
            mode: session.mode,
            has_broker: session.broker_port.is_some(),
            dashboard_pane: session.dashboard_pane.map(|p| p as usize),
            pane_layout: session.layout,
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
            pane_layout: Some(PaneLayout::Grid),
        }
    }

    /// The pane layout a session of this shape records in `Session::layout`.
    #[must_use]
    pub fn recorded_pane_layout(&self) -> Option<PaneLayout> {
        self.pane_layout
    }

    /// Whether this session's window is the whole-window tiled layout an
    /// earlier binary built, rather than the agent grid.
    ///
    /// Every session this binary builds records [`PaneLayout::Grid`]. A
    /// session that recorded none was saved by an earlier binary: a supervised
    /// one was already the grid, so only a bare one is tiled. The grid is the
    /// layout the incremental re-tile builders can extend, so this is the
    /// question `add` and `remove` ask — not whether the session is bare.
    #[must_use]
    pub fn is_legacy_tiled(&self) -> bool {
        self.pane_layout.is_none() && self.mode == SessionMode::Bare
    }

    /// Whether this session's window has a full-width top row above the agent
    /// grid — that is, whether it has a dashboard at all.
    #[must_use]
    pub fn has_top_row(&self) -> bool {
        self.dashboard_index().is_some()
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
        if let Some(agent_id) = resolve_agent(Path::new(owner_path), owners) {
            return SurfaceRole::Coding(agent_id.to_string());
        }
        if self.dashboard_index() == Some(index) {
            return SurfaceRole::Dashboard;
        }
        SurfaceRole::Unknown
    }
}

/// The coding agent whose worktree root is exactly `owner`, if any.
///
/// The single owner rule for surfaces: a surface's owner is the worktree root
/// its process started in, so an owner names an agent only by **exact
/// canonical-path equality** — never by a path prefix (a surface owned by a
/// directory below a worktree root is not that agent's) and never by basename
/// (`/elsewhere/other-feat-a` is not `feat-a`'s). Both sides are canonicalised
/// where possible, so a symlinked root (macOS `/var` → `/private/var`) still
/// matches. The drive loop (via [`SessionLayout::resolve`]), `remove`, `add`'s
/// reconciliation and restart-from-pause all resolve surfaces through it.
///
/// `roster` is the session's coding agents as `(agent_id, worktree root)`
/// pairs; the first match wins.
#[must_use]
pub fn resolve_agent<'a, I>(owner: &Path, roster: I) -> Option<&'a str>
where
    I: IntoIterator<Item = (&'a str, &'a Path)>,
{
    let candidate = canonical_or_owned(owner);
    roster
        .into_iter()
        .find(|(_, worktree_root)| paths_match(&candidate, worktree_root))
        .map(|(agent_id, _)| agent_id)
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
            pane_layout: None,
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

    // --- Core owner resolution (`resolve_agent`) -----------------------------

    const FEAT_A_ROOT: &str = "/work/repo/.git-paw/worktrees/feat-a";

    /// Spec scenario "A path below a worktree root is not an owner match".
    #[test]
    fn a_path_below_a_worktree_root_is_not_an_owner_match() {
        let owned = owners(&[("feat-a", FEAT_A_ROOT)]);
        let below = format!("{FEAT_A_ROOT}/src");
        assert_eq!(resolve_agent(Path::new(&below), refs(&owned)), None);
        assert_eq!(
            supervised().resolve(3, &below, refs(&owned)),
            SurfaceRole::Unknown
        );
    }

    /// Spec scenario "A basename collision does not misattribute".
    #[test]
    fn a_basename_collision_does_not_misattribute() {
        let owned = owners(&[("feat-a", FEAT_A_ROOT)]);
        assert_eq!(
            resolve_agent(Path::new(FEAT_A_ROOT), refs(&owned)),
            Some("feat-a")
        );
        assert_eq!(
            resolve_agent(Path::new("/elsewhere/other-feat-a"), refs(&owned)),
            None
        );
        assert_eq!(
            resolve_agent(Path::new("/elsewhere/feat-a"), refs(&owned)),
            None,
            "an equal basename under another root is not the owner"
        );
    }

    #[test]
    fn an_owner_equal_to_a_worktree_root_resolves_through_a_symlink() {
        let dir = tempfile::TempDir::new().unwrap();
        let real = dir.path().join("feat-a");
        std::fs::create_dir_all(&real).unwrap();
        let link = dir.path().join("link-a");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let roster = [("feat-a", real.as_path())];
        assert_eq!(resolve_agent(&link, roster), Some("feat-a"));
    }

    /// Spec scenario "The dashboard is found by the layout record, not by its
    /// owner": two surfaces share the repository root as owner; only the one
    /// at the layout record's dashboard position is the dashboard.
    #[test]
    fn the_dashboard_is_found_by_the_layout_record_not_by_its_owner() {
        let owned = owners(&[("feat-a", "/repo-feat-a")]);
        let layout = bare_with_broker();
        assert_eq!(
            layout.resolve(0, "/repo", refs(&owned)),
            SurfaceRole::Dashboard
        );
        assert_eq!(
            layout.resolve(2, "/repo", refs(&owned)),
            SurfaceRole::Unknown,
            "a stray repo-root surface is never the dashboard nor a coding agent"
        );
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
                pane_layout: None,
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
            layout: Some(PaneLayout::Grid),
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
            layout: None,
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

    /// Scenarios "An earlier bare session reads as tiled" and "An earlier
    /// supervised session reads as the grid" (`session-state`): what `add` and
    /// `remove` ask is the recorded layout plus the mode, never the mode alone.
    #[test]
    fn a_recorded_layout_decides_tiled_versus_grid_not_the_mode() {
        use crate::session::{SessionStatus, WorktreeEntry};
        use std::time::SystemTime;

        let base = Session {
            session_name: "paw-proj".to_string(),
            repo_path: PathBuf::from("/repo"),
            project_name: "proj".to_string(),
            created_at: SystemTime::UNIX_EPOCH,
            status: SessionStatus::Active,
            worktrees: Vec::<WorktreeEntry>::new(),
            broker_port: Some(9119),
            broker_bind: None,
            broker_log_path: None,
            mode: SessionMode::Bare,
            dashboard_pane: Some(0),
            layout: None,
        };

        // mode, recorded layout, is_legacy_tiled
        for (mode, layout, tiled, case) in [
            (
                SessionMode::Bare,
                None,
                true,
                "a bare session saved by an earlier binary is the tiled layout",
            ),
            (
                SessionMode::Supervisor,
                None,
                false,
                "a supervised session saved by an earlier binary was already the grid",
            ),
            (
                SessionMode::Bare,
                Some(PaneLayout::Grid),
                false,
                "a bare session this binary built records the grid",
            ),
            (
                SessionMode::Supervisor,
                Some(PaneLayout::Grid),
                false,
                "a supervised session this binary built records the grid",
            ),
        ] {
            let session = Session {
                mode,
                layout,
                ..base.clone()
            };
            assert_eq!(
                SessionLayout::of(&session).is_legacy_tiled(),
                tiled,
                "{case}"
            );
        }
    }

    /// Every launch this binary plans records the grid, so a recorded `None`
    /// stays unambiguous ("an earlier binary").
    #[test]
    fn every_planned_launch_records_the_grid() {
        for mode in [SessionMode::Bare, SessionMode::Supervisor] {
            for has_broker in [true, false] {
                let planned = SessionLayout::planned(mode, has_broker);
                assert_eq!(
                    planned.recorded_pane_layout(),
                    Some(PaneLayout::Grid),
                    "{mode:?} launch with broker={has_broker}"
                );
                assert!(
                    !planned.is_legacy_tiled(),
                    "{mode:?} launch with broker={has_broker} is never the tiled layout"
                );
            }
        }
    }

    /// A session has a top row exactly when it has a dashboard — the input the
    /// grid geometry takes.
    #[test]
    fn has_top_row_tracks_the_dashboard() {
        for (mode, has_broker, expected) in [
            (SessionMode::Bare, true, true),
            (SessionMode::Bare, false, false),
            (SessionMode::Supervisor, true, true),
            (SessionMode::Supervisor, false, true),
        ] {
            assert_eq!(
                SessionLayout::planned(mode, has_broker).has_top_row(),
                expected,
                "{mode:?} with broker={has_broker}"
            );
        }
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
            let layout = supervisor_layout(agent_count, true).expect("layout should compute");
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
        let small = supervisor_layout(2, true).expect("small layout should compute");
        let large = supervisor_layout(25, true).expect("large layout should compute");

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
            let layout = supervisor_layout(agent_count, true).expect("layout should compute");
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
        let err = supervisor_layout(26, true).expect_err("26 agents should be rejected");
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
        let err = supervisor_layout(100, true).expect_err("100 agents should be rejected");
        assert!(err.to_string().contains("100 agents requested"));
    }

    #[test]
    fn layout_for_matches_supervisor_layout_across_the_range() {
        // layout_for is the D1-named alias; it must be identical to
        // supervisor_layout for every valid count and reject the same way.
        for n in 1..=SUPERVISOR_MAX_AGENTS {
            for has_top_row in [true, false] {
                assert_eq!(
                    layout_for(n, has_top_row).expect("layout_for should compute"),
                    supervisor_layout(n, has_top_row).expect("supervisor_layout should compute"),
                    "layout_for({n}, {has_top_row}) should match supervisor_layout"
                );
            }
        }
        assert!(
            layout_for(SUPERVISOR_MAX_AGENTS + 1, true).is_err(),
            "layout_for should reject above the cap like supervisor_layout"
        );
        assert!(
            layout_for(SUPERVISOR_MAX_AGENTS + 1, false).is_err(),
            "the cap applies to a session with no top row too"
        );
    }

    /// The no-dashboard grid (design D2): no top row, so the agent rows share
    /// the whole window height and agents start at pane 0.
    #[test]
    fn a_grid_without_a_top_row_gives_the_agent_rows_the_full_height() {
        // agent_count, expected rows, expected per-row share
        for (agent_count, expected_rows, expected_agent) in [
            (1_usize, 1_usize, 100.0_f32),
            (5, 1, 100.0),
            (6, 2, 50.0),
            (10, 2, 50.0),
            (25, 5, 20.0),
        ] {
            let layout = supervisor_layout(agent_count, false).expect("layout should compute");
            assert!(!layout.has_top_row, "{agent_count} agents: no top row");
            assert_eq!(
                layout.agent_rows, expected_rows,
                "agent_rows for {agent_count}"
            );
            assert_eq!(
                layout.total_rows, expected_rows,
                "total_rows for {agent_count} excludes a top row it does not have"
            );
            assert_eq!(
                layout.top_row_pct, 0,
                "top_row_pct for {agent_count} with no dashboard"
            );
            assert!(
                (layout.agent_row_pct - expected_agent).abs() < 0.01,
                "agent_row_pct for {agent_count}: expected {expected_agent}, got {}",
                layout.agent_row_pct
            );
        }
    }

    /// Whatever the row count, a top-row-less grid's rows sum back to the full
    /// height — the counterpart of `agent_rows_evenly_split_the_lower_half`.
    #[test]
    fn agent_rows_without_a_top_row_sum_to_the_whole_window() {
        for agent_count in 1..=SUPERVISOR_MAX_AGENTS {
            let layout = supervisor_layout(agent_count, false).expect("layout should compute");
            #[allow(clippy::cast_precision_loss)]
            let total = layout.agent_row_pct * layout.agent_rows as f32;
            assert!(
                (total - 100.0).abs() < 0.01,
                "agent rows for {agent_count} should sum to 100%, got {total}"
            );
        }
    }

    /// The two variants differ only in what the top row takes: the row *count*
    /// for a given agent count is the same either way.
    #[test]
    fn row_count_is_the_same_with_and_without_a_top_row() {
        for agent_count in 1..=SUPERVISOR_MAX_AGENTS {
            let with = supervisor_layout(agent_count, true).expect("layout");
            let without = supervisor_layout(agent_count, false).expect("layout");
            assert_eq!(
                with.agent_rows, without.agent_rows,
                "agent rows for {agent_count} do not depend on the top row"
            );
            assert!(
                (without.agent_row_pct - with.agent_row_pct * 2.0).abs() < 0.01,
                "without a top row each agent row takes twice the height ({agent_count} agents): \
                 {} vs {}",
                without.agent_row_pct,
                with.agent_row_pct
            );
        }
    }
}
