//! The drive-loop binding: at most one drive loop per session instance.
//!
//! A drive loop is an actuator. Two of them on one session would each nudge,
//! submit buffered input and escalate into the same panes, so the exclusion
//! that keeps them apart has to be structural rather than conventional — the
//! same reasoning as the per-pane approval claim
//! ([`crate::supervisor::claim`]), one level up: the claim serialises
//! *approvers on a pane*, this serialises *loops on a session instance*.
//!
//! The binding is a refreshed lease file at
//! `<repo>/.git-paw/tmp/drive-loop.binding` (gitignored by `git paw init`,
//! beside the approval claims). It records the session name, the session's
//! instance token, the holder's pid, a per-loop id and the window the holder
//! refreshes against. A loop refreshes it every tick; an acquirer that finds
//! a binding of the same instance still being refreshed refuses, and one that
//! finds it stale, foreign or unparsable replaces it.
//!
//! Reading the binding, deciding on it and writing it is **one critical
//! section**, held under an exclusive lock on a sibling `drive-loop.lock`.
//! Publishing atomically is not sufficient by itself: two acquirers can each
//! read a replaceable binding, each decide to replace it, and each succeed.
//! On the Paused and Stopped paths that is not a race the next tick repairs,
//! because both racers revive or recover before either loop's first tick —
//! so every held boot prompt would be typed twice.
//!
//! Two properties matter more than the mechanism:
//!
//! - **It fails closed in both directions.** A second `start --unattended`
//!   refuses *before* touching any session, pane or worktree, and a loop
//!   whose binding was revoked or taken over exits on that tick *before*
//!   acting. Neither outcome can produce a doubled actuator.
//! - **It never wedges an unattended run.** A loop killed without releasing
//!   its binding lapses after [`stale_window`], so the remedy for a crashed
//!   loop is re-running the command, never deleting a file by hand. That is
//!   the opposite trade-off from [`crate::lock`]'s create-only session lock,
//!   and it is the one an unattended run needs.
//!
//! Tampering is fail-safe by the same reasoning: deleting or overwriting the
//! binding can only end a loop or delay a re-attach, and a loop that is not
//! running approves nothing — prompts simply keep waiting for a human. The
//! recorded pid is displayed in the refusal so a human can inspect the
//! holder; git-paw never signals it.

use std::fs::{self, File, FileTimes, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::PawError;

/// Floor for the staleness window, regardless of how short the poll interval
/// is.
///
/// A loop refreshes its binding once per tick, so the window has to clear a
/// tick comfortably; two minutes also keeps a coarse filesystem mtime precise
/// enough to separate "being refreshed" from "its process is gone", and keeps
/// a suspended loop (a slept laptop) from being displaced the moment it
/// misses one tick.
pub const MIN_STALE_WINDOW: Duration = Duration::from_secs(120);

/// File name of the per-repository drive-loop binding.
const BINDING_FILE: &str = "drive-loop.binding";

/// File name of the lock guarding the binding's read-check-write sequences.
const LOCK_FILE: &str = "drive-loop.lock";

/// How long a binding may go unrefreshed before an acquirer treats it as
/// abandoned: the larger of four poll intervals and [`MIN_STALE_WINDOW`].
///
/// Four intervals, so a loop that misses a tick to a slow sweep keeps its
/// binding; the floor covers a deliberately shortened poll interval.
#[must_use]
pub fn stale_window(poll_interval: Duration) -> Duration {
    (poll_interval * 4).max(MIN_STALE_WINDOW)
}

/// Returns the drive-loop binding path for `repo_root`:
/// `<repo_root>/.git-paw/tmp/drive-loop.binding`.
///
/// One binding per repository, not per session: a repository has at most one
/// live session (`session-state` find-by-repository), and naming the file
/// after the session would leave a file per purged session behind.
#[must_use]
pub fn binding_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".git-paw").join("tmp").join(BINDING_FILE)
}

/// The durable instance token for a session receipt's `created_at`.
///
/// Whole seconds since the epoch, matching the receipt serializer's own
/// resolution, so a token built from a receipt read back from disk compares
/// equal across processes.
#[must_use]
pub fn instance_token(created_at: SystemTime) -> String {
    created_at
        .duration_since(UNIX_EPOCH)
        .map_or_else(|_| "pre-epoch".to_string(), |d| d.as_secs().to_string())
}

/// Whether an existing binding of the *same* instance may be taken over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindMode {
    /// Refuse when a binding of this instance is still being refreshed — the
    /// Active, Paused and fresh-launch paths.
    Fresh,
    /// Take the binding regardless — the recovery path. A loop bound to a
    /// stopped instance is already required to stop acting, so the binding it
    /// left is not evidence of a live driver of the *recovered* session.
    Supersede,
}

/// On-disk binding record. Parsed as data and never executed; it holds
/// nothing secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BindingRecord {
    /// The tmux session name the holder drives.
    session_name: String,
    /// The holder's session instance token (see [`instance_token`]).
    instance_token: String,
    /// The holding process id — displayed in the refusal, never signalled.
    holder_pid: u32,
    /// Identifies one loop within that process, so a second loop started by
    /// the same pid (sequentially, after the first exited) is still a
    /// different holder.
    loop_id: String,
    /// The staleness window the HOLDER is refreshing against, in seconds.
    ///
    /// An acquirer judges staleness by the larger of this and its own window.
    /// Without it, a loop polling more slowly than the acquirer (a longer
    /// poll interval, or simply a slow tick) looks abandoned between its own
    /// refreshes and gets displaced while it is alive and driving.
    ///
    /// Defaulted, so a binding written by a build that predates this field
    /// parses rather than being treated as unreadable; `0` then makes the
    /// acquirer's own window win, which is the previous behaviour exactly.
    #[serde(default)]
    stale_after_secs: u64,
}

impl BindingRecord {
    /// The holder's own staleness window.
    fn stale_window(&self) -> Duration {
        Duration::from_secs(self.stale_after_secs)
    }
}

/// Exclusive lock over the binding's read-check-write sequences.
///
/// The binding file answers *who holds it*; this answers *who may decide
/// that*. Publishing the binding atomically is not enough on its own — two
/// acquirers can each read a replaceable binding, each decide to replace it,
/// and each succeed, so both believe they hold it. On the Paused and Stopped
/// paths that is not a benign race the next tick cleans up: both racers
/// revive or recover before either loop's first tick, so every held boot
/// prompt is typed twice.
///
/// Lives beside the binding as `drive-loop.lock` and is never deleted —
/// unlinking a lock file is itself a race, and an empty file costs nothing.
/// The OS releases the lock when the holder exits, by any means, so a killed
/// acquirer cannot wedge the next one.
#[derive(Debug)]
struct BindingLock {
    /// Held for the guard's lifetime; dropping it releases the lock.
    _file: File,
}

impl BindingLock {
    /// Takes the exclusive lock guarding `binding`'s directory, blocking
    /// until it is available.
    ///
    /// # Errors
    ///
    /// Returns an error when the lock file cannot be created or locked. An
    /// acquirer that cannot prove exclusivity does not proceed: a refusal is
    /// recoverable, two live drive loops are not.
    fn acquire(binding: &Path) -> Result<Self, PawError> {
        let path = binding.with_file_name(LOCK_FILE);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|e| {
                PawError::SessionError(format!(
                    "cannot open the drive-loop lock {}: {e}",
                    path.display()
                ))
            })?;
        file.lock().map_err(|e| {
            PawError::SessionError(format!(
                "cannot take the drive-loop lock {}: {e}",
                path.display()
            ))
        })?;
        Ok(Self { _file: file })
    }
}

/// RAII guard for a session instance's drive-loop binding.
///
/// Acquired with [`DriveLoopBinding::acquire`] before anything else changes,
/// refreshed with [`DriveLoopBinding::refresh`] on every tick, and released
/// on drop — but only while the file still names this loop, so a loop that
/// was superseded never deletes its successor's binding.
#[derive(Debug)]
pub struct DriveLoopBinding {
    /// Where the lease lives.
    path: PathBuf,
    /// This loop's id, the value every read compares against.
    loop_id: String,
}

impl DriveLoopBinding {
    /// Binds this process's drive loop to `session_name`'s instance under
    /// `repo_root`.
    ///
    /// Takes the binding when there is none, when the one there belongs to
    /// another session instance, when it has not been refreshed within
    /// `stale_after`, when it cannot be read or parsed, or when `mode` is
    /// [`BindMode::Supersede`].
    ///
    /// # Errors
    ///
    /// Returns [`PawError::SessionError`] when a binding of this same
    /// instance is still being refreshed by another loop (the refusal names
    /// the session, the holding pid and the binding file), or when the
    /// binding cannot be written at all. Callers acquire *before* reviving,
    /// recovering or driving anything, so a refusal leaves every session,
    /// pane and worktree untouched.
    pub fn acquire(
        repo_root: &Path,
        session_name: &str,
        instance_token: &str,
        mode: BindMode,
        stale_after: Duration,
    ) -> Result<Self, PawError> {
        let path = binding_path(repo_root);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                PawError::SessionError(format!(
                    "cannot create {} for the drive-loop binding: {e}",
                    parent.display()
                ))
            })?;
        }

        // Everything from here to the publish is one critical section. Without
        // it, two acquirers that both judge an existing binding replaceable
        // both go on to replace it, and both come away believing they hold it
        // — a doubled actuator. The decision and the write have to be
        // indivisible, not merely each atomic on their own.
        let _lock = BindingLock::acquire(&path)?;

        let record = BindingRecord {
            session_name: session_name.to_string(),
            instance_token: instance_token.to_string(),
            holder_pid: std::process::id(),
            loop_id: new_loop_id(),
            stale_after_secs: stale_after.as_secs(),
        };
        let loop_id = record.loop_id.clone();
        let body = serde_json::to_string(&record).map_err(|e| {
            PawError::SessionError(format!("cannot serialise the drive-loop binding: {e}"))
        })?;

        // Something may be there. Only a live binding of THIS instance, in
        // `Fresh` mode, blocks.
        if mode == BindMode::Fresh
            && let Some(held) = read_record(&path)
            && held.session_name == session_name
            && held.instance_token == instance_token
            // The holder's own window, not just ours: a holder polling more
            // slowly than this acquirer would otherwise look stale between its
            // ticks and be displaced while it is perfectly alive.
            && !is_stale(&path, stale_after.max(held.stale_window()))
        {
            return Err(refusal(session_name, held.holder_pid, &path));
        }

        // Create or replace, both by the same publish: write the record in
        // full to a sibling, then rename it over. The rename is what makes a
        // half-written binding unobservable; the lock above is what makes the
        // create-or-replace DECISION exclusive.
        let temp = write_temp(&path, &body)?;
        if let Err(e) = fs::rename(&temp, &path) {
            let _ = fs::remove_file(&temp);
            return Err(PawError::SessionError(format!(
                "cannot install the drive-loop binding {}: {e}",
                path.display()
            )));
        }
        Ok(Self { path, loop_id })
    }

    /// Re-asserts the binding for one more tick.
    ///
    /// Returns `true` when the file still names this loop — the mtime is
    /// bumped so the lease does not lapse — and `false` when the binding was
    /// revoked (`git paw stop`, a deleted `.git-paw/tmp/`) or taken over by
    /// another loop. A `false` is the loop's signal to exit before acting on
    /// this tick.
    ///
    /// A binding that exists and names this loop but whose mtime cannot be
    /// updated still reports `true`: losing the lease is for another loop to
    /// decide, not for a transient filesystem error to force.
    #[must_use]
    pub fn refresh(&self) -> bool {
        if !self.holds() {
            return false;
        }
        if let Ok(file) = OpenOptions::new().write(true).open(&self.path) {
            let now = SystemTime::now();
            let _ = file.set_times(FileTimes::new().set_accessed(now).set_modified(now));
        }
        true
    }

    /// Whether the binding on disk still names this loop.
    fn holds(&self) -> bool {
        read_record(&self.path).is_some_and(|held| held.loop_id == self.loop_id)
    }
}

impl Drop for DriveLoopBinding {
    fn drop(&mut self) {
        // Under the same lock as `acquire`: checking and removing as two
        // separate steps lets a successor's binding land between them and be
        // deleted by the loop it just superseded. That is not hypothetical —
        // a transient liveness misread supersedes a live loop, and a recovery
        // that then fails drops this guard on the way out.
        let _lock = BindingLock::acquire(&self.path);
        // Release only what is still ours.
        if self.holds() {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Revokes `repo_root`'s drive-loop binding, whoever holds it.
///
/// Called by `git paw stop` once the session is recorded `Stopped`: the loop
/// of a stopped instance must not go on driving a session recovered within
/// one poll interval, and recovery keeps the receipt's `created_at`, so the
/// record alone cannot tell the loop its instance ended.
///
/// Best-effort and idempotent — a missing binding is success.
pub fn revoke(repo_root: &Path) {
    let _ = fs::remove_file(binding_path(repo_root));
}

/// The error a caller sees when another loop is already driving the session.
fn refusal(session_name: &str, holder_pid: u32, path: &Path) -> PawError {
    PawError::SessionError(format!(
        "a drive loop (process {holder_pid}) is already driving session '{session_name}'; only \
         one drive loop may drive a session. Wait for that loop to exit (or stop process \
         {holder_pid}), then re-run this command. Binding: {}",
        path.display()
    ))
}

/// A per-loop identity: this process, plus when this loop took its binding.
///
/// The timestamp distinguishes two loops run sequentially by one pid, and the
/// pid distinguishes two loops started in the same instant by different
/// processes.
fn new_loop_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("{}-{nanos}", std::process::id())
}

/// Reads and parses the binding, or `None` when it is absent, unreadable or
/// not valid JSON. An unparsable binding was never written by a loop — every
/// write below lands by an atomic rename — so treating it as absent is safe.
fn read_record(path: &Path) -> Option<BindingRecord> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

/// Whether the binding has gone longer than `stale_after` without a refresh.
///
/// A binding whose mtime cannot be read is reported as NOT stale: an
/// unreadable lease is never displaced on a guess.
///
/// An mtime in the *future* needs both directions. A little ahead is benign
/// skew or filesystem granularity, and displacing a live holder over it would
/// be wrong. A lot ahead — further ahead than a whole window — cannot be a
/// lease anyone is still refreshing, and reading it as fresh forever would
/// wedge every later re-attach behind a dead process: exactly the hand-cleared
/// lock file this design exists to avoid. So that is treated as abandoned.
fn is_stale(path: &Path, stale_after: Duration) -> bool {
    let Ok(modified) = fs::metadata(path).and_then(|meta| meta.modified()) else {
        return false;
    };
    match SystemTime::now().duration_since(modified) {
        Ok(age) => age > stale_after,
        Err(ahead) => ahead.duration() > stale_after,
    }
}

/// Writes `body` to a uniquely-named sibling of `path` and returns it.
///
/// The sibling is what the acquire path renames over the binding, so the
/// binding is only ever observed complete, never half-written. `rename`
/// requires both paths on one filesystem, which a sibling guarantees.
fn write_temp(path: &Path, body: &str) -> Result<PathBuf, PawError> {
    let temp = path.with_extension(format!("tmp-{}", new_loop_id()));
    let write = || -> std::io::Result<()> {
        let mut file = File::create(&temp)?;
        file.write_all(body.as_bytes())?;
        file.sync_all()
    };
    write().map_err(|e| {
        let _ = fs::remove_file(&temp);
        PawError::SessionError(format!(
            "cannot write the drive-loop binding {}: {e}",
            temp.display()
        ))
    })?;
    Ok(temp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const TOKEN: &str = "1700000000";
    const OTHER_TOKEN: &str = "1600000000";
    const WINDOW: Duration = Duration::from_secs(120);

    fn acquire(repo: &Path, token: &str, mode: BindMode) -> Result<DriveLoopBinding, PawError> {
        DriveLoopBinding::acquire(repo, "paw-x", token, mode, WINDOW)
    }

    /// Backdates the binding's mtime by `age`, standing in for a loop that was
    /// hard-killed that long ago and never refreshed again.
    fn backdate(repo: &Path, age: Duration) {
        stamp_mtime(repo, SystemTime::now() - age);
    }

    /// Stamps the binding's mtime `ahead` in the future, standing in for a
    /// clock stepped backwards after the binding was written.
    fn postdate(repo: &Path, ahead: Duration) {
        stamp_mtime(repo, SystemTime::now() + ahead);
    }

    fn stamp_mtime(repo: &Path, when: SystemTime) {
        let file = OpenOptions::new()
            .write(true)
            .open(binding_path(repo))
            .expect("open binding");
        file.set_times(FileTimes::new().set_accessed(when).set_modified(when))
            .expect("stamp binding mtime");
    }

    #[test]
    fn binding_path_is_a_fixed_name_under_the_gitignored_tmp_dir() {
        let repo = TempDir::new().unwrap();
        assert_eq!(
            binding_path(repo.path()),
            repo.path()
                .join(".git-paw")
                .join("tmp")
                .join("drive-loop.binding"),
            "the binding path is the repository root plus a fixed name"
        );
    }

    #[test]
    fn the_stale_window_is_four_poll_intervals_but_never_under_two_minutes() {
        assert_eq!(
            stale_window(Duration::from_secs(15)),
            MIN_STALE_WINDOW,
            "four 15s intervals is under the floor, so the floor wins"
        );
        assert_eq!(
            stale_window(Duration::from_secs(60)),
            Duration::from_secs(240),
            "four intervals wins once it exceeds the floor"
        );
    }

    #[test]
    fn a_first_loop_binds_and_releasing_lets_the_next_bind_at_once() {
        let repo = TempDir::new().unwrap();
        let first = acquire(repo.path(), TOKEN, BindMode::Fresh).expect("first loop binds");
        assert!(binding_path(repo.path()).exists());
        drop(first);
        assert!(
            !binding_path(repo.path()).exists(),
            "a loop releases its binding when it exits"
        );
        assert!(
            acquire(repo.path(), TOKEN, BindMode::Fresh).is_ok(),
            "a released binding must not make the next loop wait out the window"
        );
    }

    /// Spec scenario "A second loop on a live session is refused": the refusal
    /// names the session, the holding pid and the binding file, and the first
    /// loop's binding is left exactly as it was.
    #[test]
    fn a_fresh_binding_of_the_same_instance_refuses_the_second_loop() {
        let repo = TempDir::new().unwrap();
        let _held = acquire(repo.path(), TOKEN, BindMode::Fresh).expect("first loop binds");
        let before = fs::read_to_string(binding_path(repo.path())).unwrap();

        let refused = acquire(repo.path(), TOKEN, BindMode::Fresh).expect_err("second refuses");
        let message = refused.to_string();
        assert!(message.contains("paw-x"), "names the session: {message}");
        assert!(
            message.contains(&std::process::id().to_string()),
            "names the holding process: {message}"
        );
        assert!(
            message.contains("drive-loop.binding"),
            "names the binding file: {message}"
        );
        assert_eq!(
            fs::read_to_string(binding_path(repo.path())).unwrap(),
            before,
            "a refused acquirer must change nothing"
        );
    }

    /// Spec scenario "A killed loop's binding lapses": a binding not refreshed
    /// within the window does not block a new loop, so a crashed loop never
    /// wedges an unattended run.
    #[test]
    fn a_binding_not_refreshed_within_the_window_lapses() {
        let repo = TempDir::new().unwrap();
        let orphan = acquire(repo.path(), TOKEN, BindMode::Fresh).expect("first loop binds");
        std::mem::forget(orphan); // hard-killed: no Drop ever runs

        assert!(
            acquire(repo.path(), TOKEN, BindMode::Fresh).is_err(),
            "a binding inside the window is live and must block"
        );

        backdate(repo.path(), WINDOW + Duration::from_secs(1));
        assert!(
            acquire(repo.path(), TOKEN, BindMode::Fresh).is_ok(),
            "a binding older than the window must not block a new loop"
        );
    }

    /// A clock stepped backwards (or a share with a fast clock) can leave a
    /// dead loop's binding stamped in the future. A little ahead is benign
    /// skew and must not displace a live holder; further ahead than a whole
    /// window cannot be a lease anyone is refreshing, and reading it as fresh
    /// forever would wedge every later re-attach behind a dead process.
    #[test]
    fn a_binding_stamped_far_in_the_future_does_not_wedge_re_attach() {
        let repo = TempDir::new().unwrap();
        let orphan = acquire(repo.path(), TOKEN, BindMode::Fresh).expect("first loop binds");
        std::mem::forget(orphan); // hard-killed: no Drop ever runs

        postdate(repo.path(), Duration::from_secs(5));
        assert!(
            acquire(repo.path(), TOKEN, BindMode::Fresh).is_err(),
            "a few seconds of skew is benign and must not displace a live holder"
        );

        postdate(repo.path(), WINDOW + Duration::from_secs(60));
        assert!(
            acquire(repo.path(), TOKEN, BindMode::Fresh).is_ok(),
            "a binding stamped more than a window ahead must not wedge re-attach forever"
        );
    }

    /// Spec scenario "A binding from another session instance does not block":
    /// a fresh binding left by a purged or replaced instance of the same name
    /// is replaced, not obeyed.
    #[test]
    fn a_binding_for_another_instance_does_not_block() {
        let repo = TempDir::new().unwrap();
        let orphan = acquire(repo.path(), OTHER_TOKEN, BindMode::Fresh).expect("earlier instance");
        std::mem::forget(orphan);

        let bound = acquire(repo.path(), TOKEN, BindMode::Fresh)
            .expect("a binding of another instance must not block the live one");
        assert!(
            bound.refresh(),
            "the new loop holds the binding it just replaced"
        );
    }

    /// Spec scenario "Recovering a stopped session supersedes a leftover
    /// binding": the recovering loop binds even though the leftover binding is
    /// fresh and names the same instance, and the earlier loop's next refresh
    /// reports the binding lost.
    #[test]
    fn recovery_supersedes_a_leftover_binding() {
        let repo = TempDir::new().unwrap();
        let earlier = acquire(repo.path(), TOKEN, BindMode::Fresh).expect("earlier loop binds");
        assert!(earlier.refresh(), "the earlier loop starts out holding it");

        let recovered =
            acquire(repo.path(), TOKEN, BindMode::Supersede).expect("recovery supersedes");

        assert!(
            !earlier.refresh(),
            "the superseded loop must see its binding taken over"
        );
        assert!(recovered.refresh(), "the new loop holds the binding");

        // The superseded loop exiting must not strip its successor's lease.
        drop(earlier);
        assert!(
            recovered.refresh(),
            "a superseded loop must not release the binding it no longer owns"
        );
    }

    /// An unparsable binding was never written by a loop (every write lands by
    /// an atomic rename), so it is replaced rather than treated as a holder.
    #[test]
    fn an_unreadable_binding_does_not_block() {
        let repo = TempDir::new().unwrap();
        let path = binding_path(repo.path());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "}{ not json").unwrap();

        let bound =
            acquire(repo.path(), TOKEN, BindMode::Fresh).expect("unparsable must not block");
        assert!(bound.refresh());
    }

    /// Spec scenario "Stop revokes the binding before a quick recovery", unit
    /// half: revocation is what the loop observes, and it is idempotent.
    #[test]
    fn revoke_ends_the_holders_lease_and_is_idempotent() {
        let repo = TempDir::new().unwrap();
        let held = acquire(repo.path(), TOKEN, BindMode::Fresh).expect("loop binds");
        assert!(held.refresh());

        revoke(repo.path());
        assert!(
            !held.refresh(),
            "a revoked loop must learn its binding is gone"
        );
        revoke(repo.path()); // no binding left: still fine
        assert!(!binding_path(repo.path()).exists());
    }

    /// Concurrent acquirers against an ALREADY-STALE binding — the case an
    /// atomic publish alone does not cover.
    ///
    /// Every racer reads a binding it is entitled to replace, so without a
    /// lock around read-check-write every racer replaces it and every racer
    /// returns `Ok`. The last rename wins the file, but each loop believes it
    /// holds the binding, and on the Paused and Stopped paths they all revive
    /// or recover before any of them reaches a first tick — so each held boot
    /// prompt gets typed once per racer.
    ///
    /// Exactly one `Ok` is the whole contract: the winner replaces the stale
    /// binding, and every later racer then finds a FRESH binding of the same
    /// instance and refuses.
    #[test]
    fn concurrent_acquirers_of_a_stale_binding_yield_exactly_one_holder() {
        use std::sync::Barrier;
        use std::sync::atomic::{AtomicUsize, Ordering};

        const RACERS: usize = 8;
        let repo = TempDir::new().unwrap();

        // A loop that was hard-killed longer than a window ago: present,
        // this instance's, and replaceable by every racer below.
        let orphan = acquire(repo.path(), TOKEN, BindMode::Fresh).expect("stage a binding");
        std::mem::forget(orphan);
        backdate(repo.path(), WINDOW + Duration::from_secs(1));

        let start = Barrier::new(RACERS);
        let attempted = Barrier::new(RACERS);
        let winners = AtomicUsize::new(0);

        std::thread::scope(|scope| {
            for _ in 0..RACERS {
                scope.spawn(|| {
                    start.wait();
                    let bound = acquire(repo.path(), TOKEN, BindMode::Fresh).ok();
                    if bound.is_some() {
                        winners.fetch_add(1, Ordering::SeqCst);
                    }
                    // Hold until every racer has tried, so a released lease
                    // cannot let a later racer legitimately win too.
                    attempted.wait();
                    drop(bound);
                });
            }
        });

        assert_eq!(
            winners.load(Ordering::SeqCst),
            1,
            "exactly one of {RACERS} acquirers may replace a stale binding — any more and \
             each would revive the session and re-type every held boot prompt"
        );
    }

    /// Concurrent first binders race on the atomic create; whoever loses reads
    /// a fresh binding of the same instance and refuses, so exactly one loop
    /// comes away bound.
    #[test]
    fn concurrent_first_binders_yield_exactly_one_holder() {
        use std::sync::Barrier;
        use std::sync::atomic::{AtomicUsize, Ordering};

        const RACERS: usize = 8;
        let repo = TempDir::new().unwrap();
        let start = Barrier::new(RACERS);
        let attempted = Barrier::new(RACERS);
        let winners = AtomicUsize::new(0);

        std::thread::scope(|scope| {
            for _ in 0..RACERS {
                scope.spawn(|| {
                    start.wait();
                    let bound = acquire(repo.path(), TOKEN, BindMode::Fresh).ok();
                    if bound.is_some() {
                        winners.fetch_add(1, Ordering::SeqCst);
                    }
                    // Hold until every racer has tried, so a second winner
                    // would mean a real race rather than a released lease.
                    attempted.wait();
                    drop(bound);
                });
            }
        });

        assert_eq!(
            winners.load(Ordering::SeqCst),
            1,
            "exactly one of {RACERS} concurrent loops may drive the session"
        );
    }
}
