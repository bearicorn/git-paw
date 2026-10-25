//! The terminal port: every orchestration-path interaction with the terminal
//! that hosts git-paw's agents, in git-paw's own vocabulary.
//!
//! git-paw runs each coding agent on a **surface** of a **session** — a place
//! where a CLI process runs, owned by one working tree, whose screen can be
//! read and into which text and keys can be sent. [`TerminalBackend`] is the
//! port the drive loop, the launch paths, `add`, `remove`, prompt submission
//! and the agent inventory go through. tmux is the first adapter
//! ([`tmux::TmuxBackend`]); [`fake::FakeBackend`] records operations and
//! answers scripted screens and owners so orchestration logic is tested
//! without a terminal.
//!
//! The port speaks no multiplexer's model. A surface is addressed by an
//! opaque [`SurfaceId`] that only a backend constructs; the session's layout
//! record ([`crate::surface::SessionLayout`]) names surfaces by **position**,
//! and the backend converts a position to a [`SurfaceId`]
//! ([`TerminalBackend::surface_at`]). How a backend encodes either is its own
//! business: the tmux adapter persists its encoding in the existing
//! `pane_index` / `dashboard_pane` fields, and a future backend adds its own
//! optional field rather than repurposing those integers.
//!
//! Adapters are compile-time and in-tree, selected at construction — there is
//! no runtime plugin mechanism.

pub mod fake;
pub mod tmux;

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::error::PawError;
use crate::session::SessionMode;

/// Opaque handle to one surface of a session.
///
/// Constructed only by a backend, from its own listing or from a layout
/// position ([`TerminalBackend::surface_at`]); callers compare and pass it
/// back, never inspect it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SurfaceId(String);

impl SurfaceId {
    /// Wraps a backend's own encoding of a surface. Private to the port's
    /// adapters so no caller can forge a handle from a multiplexer value.
    pub(in crate::terminal) fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    /// The backend's encoding, for the adapter that made it.
    pub(in crate::terminal) fn token(&self) -> &str {
        &self.0
    }
}

/// Opaque handle to one service of a session: a non-interactive process the
/// session hosts outside its agent grid, addressed by the session and the
/// service's name.
///
/// Constructed only by a backend ([`TerminalBackend::spawn_service`] or
/// [`TerminalBackend::service`]); callers pass it back, never inspect it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServiceHandle {
    session: String,
    name: String,
}

impl ServiceHandle {
    /// Names the service `name` of `session`. Private to the port's adapters.
    pub(in crate::terminal) fn new(session: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            session: session.into(),
            name: name.into(),
        }
    }

    /// The session hosting the service.
    pub(in crate::terminal) fn session(&self) -> &str {
        &self.session
    }

    /// The service's name.
    pub(in crate::terminal) fn name(&self) -> &str {
        &self.name
    }
}

/// One surface of a live session, as a backend lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceInfo {
    /// The surface's handle.
    pub id: SurfaceId,
    /// The surface's position in the session's layout — the numbering the
    /// session's layout record ([`crate::surface::SessionLayout`]) uses.
    pub position: usize,
    /// The working tree that owns the surface (where its process runs), or
    /// `None` when the backend cannot tell.
    pub owner: Option<PathBuf>,
}

/// One surface to create: what it is labelled, which working tree owns it,
/// and the command it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceSpec {
    /// The surface's label (an agent's branch, or `dashboard`).
    pub label: String,
    /// The working tree the surface's process starts in.
    pub owner: PathBuf,
    /// The command the surface runs at launch.
    pub launch_command: String,
}

/// A session to open: its surfaces, environment and presentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionPlan {
    /// The resolved session name.
    pub name: String,
    /// Which layout family the session is built as.
    pub mode: SessionMode,
    /// The dashboard surface, when the session has one. A supervised session
    /// always has one.
    pub dashboard: Option<SurfaceSpec>,
    /// The coding-agent surfaces, in layout order.
    pub surfaces: Vec<SurfaceSpec>,
    /// Session-scoped environment variables, set before any agent starts.
    pub env: Vec<(String, String)>,
    /// Whether a pointing device can select and resize surfaces.
    pub mouse: bool,
    /// Whether surfaces show their labels and active/inactive styling.
    pub show_labels: bool,
}

/// The agent grid a layout operation applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridGeometry {
    /// Position of the first coding-agent surface.
    pub first_agent: usize,
    /// How many coding-agent surfaces the grid holds.
    pub agent_count: usize,
    /// Whether a full-width dashboard row sits above the grid.
    pub has_top_row: bool,
}

/// Where a new surface goes in a live session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfacePlacement {
    /// Appended to the agent grid after its last coding agent; `grid`
    /// describes the grid BEFORE the new surface joins it.
    AppendToGrid {
        /// The grid as it stands.
        grid: GridGeometry,
        /// Whether the new surface shows its label.
        show_labels: bool,
    },
    /// Inserted before an existing surface, which keeps its own handle.
    Before(SurfaceId),
}

/// A named key, kept distinct from text so a submit is always its own
/// operation — the split-send-then-`Enter` discipline survives any backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Submits the composer's contents.
    Enter,
    /// Clears the composer's current line.
    ClearLine,
}

/// Outcome of a session-liveness probe.
///
/// Distinguishes a genuinely-absent session (`Stale`) from a session that
/// exists but whose agent surfaces have all exited (`AgentsExited`), and both
/// from a probe that could not be run at all (`Indeterminate`, e.g. the
/// terminal's binary is missing). Receipt-staleness detection SHALL NOT report
/// `🔴 stale` on an `Indeterminate` probe — a missing binary is not evidence
/// the session died — and SHALL NOT report it on `AgentsExited` either: the
/// session is still running there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionLiveness {
    /// The session exists.
    Alive,
    /// The probe ran and the session is gone.
    Stale,
    /// The session exists and the launch grace period has passed, but no
    /// agent surface runs anything but a command interpreter — every agent's
    /// CLI has exited. Only [`TerminalBackend::liveness`] can return this.
    AgentsExited,
    /// The probe could not be run. The caller SHALL preserve the receipt's
    /// current state.
    Indeterminate,
}

/// The port every orchestration-path terminal operation goes through.
///
/// Each method answers a question any terminal host could answer — a tmux
/// server, a PTY the process owns, a headless runner — in git-paw's terms.
/// Text and keys are separate operations, and there is deliberately no
/// operation that types text and submits it in one step.
pub trait TerminalBackend {
    /// Opens a session from `plan`: creates every surface, starts its launch
    /// command, and sets the session environment.
    ///
    /// # Errors
    /// Returns [`PawError`] when the session cannot be created.
    fn open_session(&self, plan: &SessionPlan) -> Result<(), PawError>;

    /// Whether `session` exists.
    ///
    /// # Errors
    /// Returns [`PawError`] when the backend cannot be asked.
    fn session_alive(&self, session: &str) -> Result<bool, PawError>;

    /// Closes `session` and every surface in it.
    ///
    /// # Errors
    /// Returns [`PawError`] when the session cannot be closed.
    fn close_session(&self, session: &str) -> Result<(), PawError>;

    /// Sets a session-scoped environment variable for surfaces created later.
    ///
    /// # Errors
    /// Returns [`PawError`] when the variable cannot be set.
    fn set_session_env(&self, session: &str, key: &str, value: &str) -> Result<(), PawError>;

    /// Probes `session`'s liveness, including whether any agent surface at
    /// or after `first_agent` still runs its CLI once the launch grace period
    /// since `created_at` has passed.
    fn liveness(
        &self,
        session: &str,
        created_at: SystemTime,
        first_agent: usize,
    ) -> SessionLiveness;

    /// The surface at `position` of `session`'s layout, as the session's
    /// layout record names it.
    fn surface_at(&self, session: &str, position: usize) -> SurfaceId;

    /// Every surface of `session` with its position and owning working tree.
    /// A session that no longer exists has no surfaces.
    ///
    /// # Errors
    /// Returns [`PawError`] when the backend cannot be asked.
    fn list_surfaces(&self, session: &str) -> Result<Vec<SurfaceInfo>, PawError>;

    /// The surface whose owning working tree is exactly `owner`, if one is
    /// live.
    ///
    /// # Errors
    /// Returns [`PawError`] when the backend cannot be asked.
    fn surface_owned_by(&self, session: &str, owner: &Path) -> Result<Option<SurfaceId>, PawError>;

    /// Creates a surface from `spec` at `placement` and starts its launch
    /// command, returning its handle.
    ///
    /// # Errors
    /// Returns [`PawError`] when the surface cannot be created.
    fn spawn_surface(
        &self,
        session: &str,
        spec: &SurfaceSpec,
        placement: &SurfacePlacement,
    ) -> Result<SurfaceId, PawError>;

    /// Closes one surface. A surface that is already gone is not an error.
    ///
    /// # Errors
    /// Returns [`PawError`] when the surface exists but cannot be closed.
    fn kill(&self, surface: &SurfaceId) -> Result<(), PawError>;

    /// Sets a surface's label.
    ///
    /// # Errors
    /// Returns [`PawError`] when the label cannot be set.
    fn label_surface(&self, surface: &SurfaceId, label: &str) -> Result<(), PawError>;

    /// A surface's current label, or `None` when it cannot be read.
    fn label_of(&self, surface: &SurfaceId) -> Option<String>;

    /// Types `text` into a surface literally, submitting nothing.
    ///
    /// # Errors
    /// Returns [`PawError`] when the text cannot be delivered.
    fn send_text(&self, surface: &SurfaceId, text: &str) -> Result<(), PawError>;

    /// Leaves `text` in a surface's composer for the user to complete and
    /// submit — git-paw never submits it.
    ///
    /// # Errors
    /// Returns [`PawError`] when the text cannot be delivered.
    fn prefill_text(&self, surface: &SurfaceId, text: &str) -> Result<(), PawError>;

    /// Re-runs `launch_command` in a surface whose agent CLI never started
    /// (the readiness gate's bare-interpreter relaunch): clears the line, then
    /// starts the command. Like [`Self::spawn_surface`], this starts a
    /// process; it is not a prompt submission.
    ///
    /// # Errors
    /// Returns [`PawError`] when the command cannot be delivered.
    fn relaunch_surface(&self, surface: &SurfaceId, launch_command: &str) -> Result<(), PawError>;

    /// Chooses numbered `option` of the first-run acceptance dialog an agent
    /// CLI shows at start-up (the readiness gate's dialog answer).
    ///
    /// # Errors
    /// Returns [`PawError`] when the choice cannot be delivered.
    fn answer_dialog(&self, surface: &SurfaceId, option: u8) -> Result<(), PawError>;

    /// Sends one named key to a surface.
    ///
    /// # Errors
    /// Returns [`PawError`] when the key cannot be delivered.
    fn send_key(&self, surface: &SurfaceId, key: Key) -> Result<(), PawError>;

    /// A surface's recent screen, or `None` when it could not be read.
    ///
    /// A failed read is `None`, never an empty screen: an empty screen
    /// satisfies every idle marker there is, so a caller would otherwise
    /// mistake "could not look" for the strongest evidence of idleness.
    fn capture_tail(&self, surface: &SurfaceId) -> Option<String>;

    /// The name of the process in a surface's foreground, or `None` when it
    /// could not be read.
    ///
    /// A surface whose agent CLI exited looks idle to every screen-based
    /// predicate; this is how a caller tells a waiting CLI from a command
    /// interpreter before it types.
    fn foreground_process(&self, surface: &SurfaceId) -> Option<String>;

    /// Appends everything a surface prints from now on to `log`.
    ///
    /// # Errors
    /// Returns [`PawError`] when the stream cannot be started.
    fn stream_to(&self, surface: &SurfaceId, log: &Path) -> Result<(), PawError>;

    /// Evens the column widths of every row of the agent grid. A no-op for a
    /// backend that does not tile.
    ///
    /// # Errors
    /// Returns [`PawError`] when the backend cannot be asked.
    fn layout(&self, session: &str, grid: &GridGeometry) -> Result<(), PawError>;

    /// Re-flows the agent grid's row heights after a surface left it, so the
    /// grid matches a launch of `grid.agent_count` agents. A no-op for a
    /// backend that does not tile.
    ///
    /// # Errors
    /// Returns [`PawError`] when the grid cannot be re-flowed.
    fn reflow_rows(&self, session: &str, grid: &GridGeometry) -> Result<(), PawError>;

    /// Attaches the current terminal to `session` as a client.
    ///
    /// # Errors
    /// Returns [`PawError`] when the attach fails.
    fn attach(&self, session: &str) -> Result<(), PawError>;

    /// Detaches every client attached to `session`, leaving it running.
    ///
    /// # Errors
    /// Returns [`PawError`] when the clients cannot be detached.
    fn detach_clients(&self, session: &str) -> Result<(), PawError>;

    /// Runs `command` as a service named `name` inside `session`, started in
    /// `owner`: a process the human does not watch, which occupies no surface
    /// of the agent grid, so every surface keeps its position. Closing the
    /// session ends the service as it ends every surface, with a hang-up.
    ///
    /// # Errors
    /// Returns [`PawError`] when the service cannot be started.
    fn spawn_service(
        &self,
        session: &str,
        name: &str,
        command: &str,
        owner: &Path,
    ) -> Result<ServiceHandle, PawError>;

    /// The handle of the service `name` of `session`, whether or not it is
    /// running — for a caller that did not spawn it (pause, status).
    fn service(&self, session: &str, name: &str) -> ServiceHandle;

    /// Stops a service, delivering it a hang-up. A service that is already
    /// gone — or whose session is gone — is not an error.
    ///
    /// # Errors
    /// Returns [`PawError`] when the service is running but cannot be stopped.
    fn stop_service(&self, handle: &ServiceHandle) -> Result<(), PawError>;

    /// Whether the service is running. A session that no longer exists has
    /// no running service.
    ///
    /// # Errors
    /// Returns [`PawError`] when the backend cannot be asked.
    fn service_alive(&self, handle: &ServiceHandle) -> Result<bool, PawError>;
}
