//! The fake terminal backend for orchestration-logic tests.
//!
//! [`FakeBackend`] records every operation it receives and answers reads from
//! a script — surfaces and their owners, screens in sequence, foreground
//! processes — so the drive loop, the launch flows, `add` / `remove` and the
//! inventory are tested against the port without spawning a terminal. Logic
//! tests are written once against this fake; each real backend gets smoke
//! tests only.
//!
//! Compiled into the library (not `#[cfg(test)]`) because the binary crate's
//! own unit tests drive it too. Production never constructs one.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::error::PawError;

use super::{
    GridGeometry, Key, SessionLiveness, SessionPlan, SurfaceId, SurfaceInfo, SurfacePlacement,
    SurfaceSpec, TerminalBackend,
};

/// One operation the fake received, in the order it arrived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    /// [`TerminalBackend::open_session`].
    OpenSession(SessionPlan),
    /// [`TerminalBackend::close_session`].
    CloseSession(String),
    /// [`TerminalBackend::set_session_env`].
    SetSessionEnv {
        /// Session.
        session: String,
        /// Variable name.
        key: String,
        /// Value.
        value: String,
    },
    /// [`TerminalBackend::list_surfaces`].
    ListSurfaces(String),
    /// [`TerminalBackend::spawn_surface`].
    SpawnSurface {
        /// Session.
        session: String,
        /// What was spawned.
        spec: SurfaceSpec,
        /// Where.
        placement: SurfacePlacement,
    },
    /// [`TerminalBackend::kill`].
    Kill(SurfaceId),
    /// [`TerminalBackend::label_surface`].
    Label {
        /// Surface.
        surface: SurfaceId,
        /// New label.
        label: String,
    },
    /// [`TerminalBackend::send_text`].
    SendText {
        /// Surface.
        surface: SurfaceId,
        /// Text typed.
        text: String,
    },
    /// [`TerminalBackend::prefill_text`].
    PrefillText {
        /// Surface.
        surface: SurfaceId,
        /// Text left in the composer.
        text: String,
    },
    /// [`TerminalBackend::relaunch_surface`].
    RelaunchSurface {
        /// Surface.
        surface: SurfaceId,
        /// Command re-run.
        command: String,
    },
    /// [`TerminalBackend::answer_dialog`].
    AnswerDialog {
        /// Surface.
        surface: SurfaceId,
        /// Option chosen.
        option: u8,
    },
    /// [`TerminalBackend::send_key`].
    SendKey {
        /// Surface.
        surface: SurfaceId,
        /// Key sent.
        key: Key,
    },
    /// [`TerminalBackend::capture_tail`].
    CaptureTail(SurfaceId),
    /// [`TerminalBackend::foreground_process`].
    ForegroundProcess(SurfaceId),
    /// [`TerminalBackend::stream_to`].
    StreamTo {
        /// Surface.
        surface: SurfaceId,
        /// Log file.
        log: PathBuf,
    },
    /// [`TerminalBackend::layout`].
    Layout {
        /// Session.
        session: String,
        /// Grid.
        grid: GridGeometry,
    },
    /// [`TerminalBackend::reflow_rows`].
    ReflowRows {
        /// Session.
        session: String,
        /// Grid.
        grid: GridGeometry,
    },
    /// [`TerminalBackend::attach`].
    Attach(String),
    /// [`TerminalBackend::detach_clients`].
    DetachClients(String),
}

impl Operation {
    /// The surface this operation acted on, when it acted on one.
    #[must_use]
    pub fn surface(&self) -> Option<&SurfaceId> {
        match self {
            Operation::Kill(s)
            | Operation::CaptureTail(s)
            | Operation::ForegroundProcess(s)
            | Operation::Label { surface: s, .. }
            | Operation::SendText { surface: s, .. }
            | Operation::PrefillText { surface: s, .. }
            | Operation::SendKey { surface: s, .. }
            | Operation::RelaunchSurface { surface: s, .. }
            | Operation::AnswerDialog { surface: s, .. }
            | Operation::StreamTo { surface: s, .. } => Some(s),
            _ => None,
        }
    }

    /// Whether this operation typed into a surface (text, pre-fill, key,
    /// relaunch or dialog answer).
    #[must_use]
    pub fn is_keystroke(&self) -> bool {
        matches!(
            self,
            Operation::SendText { .. }
                | Operation::PrefillText { .. }
                | Operation::SendKey { .. }
                | Operation::RelaunchSurface { .. }
                | Operation::AnswerDialog { .. }
        )
    }
}

/// Scripted answers and the operation log.
#[derive(Debug, Default)]
struct State {
    ops: Vec<Operation>,
    surfaces: HashMap<String, Vec<(usize, Option<PathBuf>)>>,
    screens: HashMap<SurfaceId, VecDeque<Option<String>>>,
    processes: HashMap<SurfaceId, Option<String>>,
    alive: HashMap<String, bool>,
    liveness: HashMap<String, SessionLiveness>,
}

/// The default foreground process of a surface with none scripted: an agent
/// CLI, not a command interpreter.
pub const DEFAULT_FOREGROUND_PROCESS: &str = "claude";

/// A recording, scripted [`TerminalBackend`].
#[derive(Debug, Default)]
pub struct FakeBackend {
    state: RefCell<State>,
}

impl FakeBackend {
    /// An empty fake: no sessions, no surfaces.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder form of [`Self::add_surface`].
    #[must_use]
    pub fn with_surface(self, session: &str, position: usize, owner: Option<&Path>) -> Self {
        self.add_surface(session, position, owner);
        self
    }

    /// Adds a surface at `position` of `session`, owned by `owner`, and
    /// returns its handle.
    pub fn add_surface(&self, session: &str, position: usize, owner: Option<&Path>) -> SurfaceId {
        self.state
            .borrow_mut()
            .surfaces
            .entry(session.to_string())
            .or_default()
            .push((position, owner.map(Path::to_path_buf)));
        self.surface_at(session, position)
    }

    /// Scripts the screens successive [`TerminalBackend::capture_tail`] reads
    /// of `surface` return, in order; the last one repeats. `None` is a
    /// failed read.
    pub fn script_screens(&self, surface: &SurfaceId, screens: &[Option<&str>]) {
        self.state.borrow_mut().screens.insert(
            surface.clone(),
            screens.iter().map(|s| s.map(str::to_string)).collect(),
        );
    }

    /// Scripts `surface`'s foreground process (`None` is a failed read).
    /// Unscripted surfaces report [`DEFAULT_FOREGROUND_PROCESS`].
    pub fn script_foreground_process(&self, surface: &SurfaceId, process: Option<&str>) {
        self.state
            .borrow_mut()
            .processes
            .insert(surface.clone(), process.map(str::to_string));
    }

    /// Scripts whether `session` is alive and what liveness it reports.
    pub fn script_session(&self, session: &str, alive: bool, liveness: SessionLiveness) {
        let mut state = self.state.borrow_mut();
        state.alive.insert(session.to_string(), alive);
        state.liveness.insert(session.to_string(), liveness);
    }

    /// Every operation received so far, in order.
    #[must_use]
    pub fn operations(&self) -> Vec<Operation> {
        self.state.borrow().ops.clone()
    }

    /// The operations that acted on `surface`, in order.
    #[must_use]
    pub fn operations_on(&self, surface: &SurfaceId) -> Vec<Operation> {
        self.state
            .borrow()
            .ops
            .iter()
            .filter(|op| op.surface() == Some(surface))
            .cloned()
            .collect()
    }

    fn record(&self, op: Operation) {
        self.state.borrow_mut().ops.push(op);
    }
}

impl TerminalBackend for FakeBackend {
    fn open_session(&self, plan: &SessionPlan) -> Result<(), PawError> {
        self.record(Operation::OpenSession(plan.clone()));
        let mut state = self.state.borrow_mut();
        state.alive.insert(plan.name.clone(), true);
        let listed = state.surfaces.entry(plan.name.clone()).or_default();
        for (position, spec) in plan.dashboard.iter().chain(&plan.surfaces).enumerate() {
            listed.push((position, Some(spec.owner.clone())));
        }
        Ok(())
    }

    fn session_alive(&self, session: &str) -> Result<bool, PawError> {
        Ok(self
            .state
            .borrow()
            .alive
            .get(session)
            .copied()
            .unwrap_or(false))
    }

    fn close_session(&self, session: &str) -> Result<(), PawError> {
        self.record(Operation::CloseSession(session.to_string()));
        let mut state = self.state.borrow_mut();
        state.alive.insert(session.to_string(), false);
        state.surfaces.remove(session);
        Ok(())
    }

    fn set_session_env(&self, session: &str, key: &str, value: &str) -> Result<(), PawError> {
        self.record(Operation::SetSessionEnv {
            session: session.to_string(),
            key: key.to_string(),
            value: value.to_string(),
        });
        Ok(())
    }

    fn liveness(
        &self,
        session: &str,
        _created_at: SystemTime,
        _first_agent: usize,
    ) -> SessionLiveness {
        self.state
            .borrow()
            .liveness
            .get(session)
            .copied()
            .unwrap_or(SessionLiveness::Stale)
    }

    fn surface_at(&self, session: &str, position: usize) -> SurfaceId {
        SurfaceId::new(format!("{session}#{position}"))
    }

    fn list_surfaces(&self, session: &str) -> Result<Vec<SurfaceInfo>, PawError> {
        self.record(Operation::ListSurfaces(session.to_string()));
        let listed = self
            .state
            .borrow()
            .surfaces
            .get(session)
            .cloned()
            .unwrap_or_default();
        Ok(listed
            .into_iter()
            .map(|(position, owner)| SurfaceInfo {
                id: self.surface_at(session, position),
                position,
                owner,
            })
            .collect())
    }

    fn surface_owned_by(&self, session: &str, owner: &Path) -> Result<Option<SurfaceId>, PawError> {
        Ok(self
            .list_surfaces(session)?
            .into_iter()
            .find(|s| s.owner.as_deref() == Some(owner))
            .map(|s| s.id))
    }

    fn spawn_surface(
        &self,
        session: &str,
        spec: &SurfaceSpec,
        placement: &SurfacePlacement,
    ) -> Result<SurfaceId, PawError> {
        self.record(Operation::SpawnSurface {
            session: session.to_string(),
            spec: spec.clone(),
            placement: placement.clone(),
        });
        match placement {
            SurfacePlacement::AppendToGrid { grid, .. } => Ok(self.add_surface(
                session,
                grid.first_agent + grid.agent_count,
                Some(&spec.owner),
            )),
            SurfacePlacement::Before(anchor) => Ok(anchor.clone()),
        }
    }

    fn kill(&self, surface: &SurfaceId) -> Result<(), PawError> {
        self.record(Operation::Kill(surface.clone()));
        Ok(())
    }

    fn label_surface(&self, surface: &SurfaceId, label: &str) -> Result<(), PawError> {
        self.record(Operation::Label {
            surface: surface.clone(),
            label: label.to_string(),
        });
        Ok(())
    }

    fn label_of(&self, surface: &SurfaceId) -> Option<String> {
        self.state
            .borrow()
            .ops
            .iter()
            .rev()
            .find_map(|op| match op {
                Operation::Label { surface: s, label } if s == surface => Some(label.clone()),
                _ => None,
            })
    }

    fn send_text(&self, surface: &SurfaceId, text: &str) -> Result<(), PawError> {
        self.record(Operation::SendText {
            surface: surface.clone(),
            text: text.to_string(),
        });
        Ok(())
    }

    fn prefill_text(&self, surface: &SurfaceId, text: &str) -> Result<(), PawError> {
        self.record(Operation::PrefillText {
            surface: surface.clone(),
            text: text.to_string(),
        });
        Ok(())
    }

    fn relaunch_surface(&self, surface: &SurfaceId, launch_command: &str) -> Result<(), PawError> {
        self.record(Operation::RelaunchSurface {
            surface: surface.clone(),
            command: launch_command.to_string(),
        });
        Ok(())
    }

    fn answer_dialog(&self, surface: &SurfaceId, option: u8) -> Result<(), PawError> {
        self.record(Operation::AnswerDialog {
            surface: surface.clone(),
            option,
        });
        Ok(())
    }

    fn send_key(&self, surface: &SurfaceId, key: Key) -> Result<(), PawError> {
        self.record(Operation::SendKey {
            surface: surface.clone(),
            key,
        });
        Ok(())
    }

    fn capture_tail(&self, surface: &SurfaceId) -> Option<String> {
        self.record(Operation::CaptureTail(surface.clone()));
        let mut state = self.state.borrow_mut();
        let queue = state.screens.get_mut(surface)?;
        if queue.len() > 1 {
            queue.pop_front().flatten()
        } else {
            queue.front().cloned().flatten()
        }
    }

    fn foreground_process(&self, surface: &SurfaceId) -> Option<String> {
        self.record(Operation::ForegroundProcess(surface.clone()));
        self.state
            .borrow()
            .processes
            .get(surface)
            .cloned()
            .unwrap_or_else(|| Some(DEFAULT_FOREGROUND_PROCESS.to_string()))
    }

    fn stream_to(&self, surface: &SurfaceId, log: &Path) -> Result<(), PawError> {
        self.record(Operation::StreamTo {
            surface: surface.clone(),
            log: log.to_path_buf(),
        });
        Ok(())
    }

    fn layout(&self, session: &str, grid: &GridGeometry) -> Result<(), PawError> {
        self.record(Operation::Layout {
            session: session.to_string(),
            grid: *grid,
        });
        Ok(())
    }

    fn reflow_rows(&self, session: &str, grid: &GridGeometry) -> Result<(), PawError> {
        self.record(Operation::ReflowRows {
            session: session.to_string(),
            grid: *grid,
        });
        Ok(())
    }

    fn attach(&self, session: &str) -> Result<(), PawError> {
        self.record(Operation::Attach(session.to_string()));
        Ok(())
    }

    fn detach_clients(&self, session: &str) -> Result<(), PawError> {
        self.record(Operation::DetachClients(session.to_string()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screens_play_in_order_and_the_last_repeats() {
        let fake = FakeBackend::new();
        let s = fake.add_surface("paw-x", 1, Some(Path::new("/wt/a")));
        fake.script_screens(&s, &[Some("busy"), None, Some("idle")]);
        assert_eq!(fake.capture_tail(&s).as_deref(), Some("busy"));
        assert_eq!(fake.capture_tail(&s), None, "a scripted failed read");
        assert_eq!(fake.capture_tail(&s).as_deref(), Some("idle"));
        assert_eq!(fake.capture_tail(&s).as_deref(), Some("idle"));
    }

    #[test]
    fn an_unscripted_surface_has_no_screen() {
        let fake = FakeBackend::new();
        let s = fake.add_surface("paw-x", 0, None);
        assert_eq!(fake.capture_tail(&s), None);
    }

    #[test]
    fn listing_reports_scripted_owners_by_position() {
        let fake = FakeBackend::new()
            .with_surface("paw-x", 2, Some(Path::new("/wt/b")))
            .with_surface("paw-x", 1, Some(Path::new("/wt/a")));
        let listed = fake.list_surfaces("paw-x").unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].position, 2);
        assert_eq!(listed[0].owner.as_deref(), Some(Path::new("/wt/b")));
        assert_eq!(listed[0].id, fake.surface_at("paw-x", 2));
        assert_eq!(
            fake.surface_owned_by("paw-x", Path::new("/wt/a")).unwrap(),
            Some(fake.surface_at("paw-x", 1))
        );
    }

    #[test]
    fn operations_on_filters_to_one_surface() {
        let fake = FakeBackend::new();
        let a = fake.surface_at("paw-x", 1);
        let b = fake.surface_at("paw-x", 2);
        fake.send_text(&a, "hi").unwrap();
        fake.send_key(&b, Key::Enter).unwrap();
        fake.send_key(&a, Key::Enter).unwrap();
        assert_eq!(
            fake.operations_on(&a),
            vec![
                Operation::SendText {
                    surface: a.clone(),
                    text: "hi".into()
                },
                Operation::SendKey {
                    surface: a.clone(),
                    key: Key::Enter
                },
            ]
        );
    }
}
