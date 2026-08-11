//! Exclusive per-pane approval claim.
//!
//! Approval keystrokes reach an agent pane from three independent approvers:
//! the in-process drive loop ([`crate::supervisor::drive`]), the separate
//! `__dashboard` process ([`crate::supervisor::approve`]), and the bundled
//! `sweep.sh approve` helper run by the orchestrator or a human. Two of them
//! landing on the same pane produces a double-approval — the second keystroke
//! falls through onto whatever prompt comes next — and until now only a prose
//! "sole approver" convention kept them apart.
//!
//! This module makes the exclusion structural. Before sending, an approver
//! takes the pane's claim: an atomic create-if-absent of
//! `<repo>/.git-paw/tmp/approve-pane-<N>.claim`, so the kernel — not a
//! convention — decides which acquirer wins. `sweep.sh` computes the same path
//! and acquires it with bash noclobber, so the exclusion spans processes and
//! languages. Acquisition is non-blocking: a loser skips the pane for this
//! attempt and retries on a later tick, which is what keeps the drive loop's
//! non-blocking escalation guarantee intact.
//!
//! Release is by RAII ([`PaneClaim`]'s `Drop`), so a normal return, an early
//! `?`, and a panic unwinding through the send all release the pane. A hard
//! kill (SIGKILL) runs no destructor, so a claim left untouched for longer than
//! [`CLAIM_TTL`] is treated as abandoned and may be stolen — a crashed approver
//! wedges a pane for one TTL, never forever.

use std::fs::{self, File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long a claim file may sit untouched before an acquirer treats it as
/// abandoned and steals it.
///
/// Sized far above a real acquire→capture→send→release window (sub-second) and
/// far below any tolerable wedge, so a coarse filesystem mtime is precise
/// enough to distinguish the two. The bundled `sweep.sh` applies the same bound.
pub const CLAIM_TTL: Duration = Duration::from_secs(10);

/// Returns the approval-claim path for `pane_index`:
/// `<repo_root>/.git-paw/tmp/approve-pane-<N>.claim`.
///
/// This formula is the cross-process contract. `sweep.sh approve` builds the
/// byte-identical path from its own repository root — both roots come from
/// `git rev-parse --show-toplevel` — so a claim taken on either side is
/// observed by the other. The directory is created on demand by
/// [`PaneClaim::try_acquire`] and is already gitignored.
#[must_use]
pub fn claim_path(repo_root: &Path, pane_index: usize) -> PathBuf {
    repo_root
        .join(".git-paw")
        .join("tmp")
        .join(format!("approve-pane-{pane_index}.claim"))
}

/// RAII guard for one pane's exclusive approval claim.
///
/// Acquired with [`PaneClaim::try_acquire`]; the claim file is removed when the
/// guard drops. Hold it across the whole approval send — the re-confirm capture
/// and every keystroke — and let it drop immediately after, so the contended
/// window stays sub-second.
#[derive(Debug)]
pub struct PaneClaim {
    path: PathBuf,
    // Held only so the underlying handle lives as long as the guard; the file
    // is removed on drop via `path` (mirrors `crate::lock::SessionLock`).
    _file: File,
}

impl PaneClaim {
    /// Attempts to take `pane_index`'s approval claim under `repo_root`.
    ///
    /// Returns `Some(guard)` when this caller now owns the pane and `None` when
    /// another approver holds it — **non-blocking**, so a caller that gets
    /// `None` sends nothing and retries on a later sweep rather than waiting.
    ///
    /// An existing claim older than [`CLAIM_TTL`] is abandoned (its approver was
    /// killed before any release could run); it is removed and the create is
    /// retried exactly once. Every other failure — an unwritable `.git-paw/tmp`,
    /// a racer winning the steal — yields `None`, so an approver that cannot
    /// prove exclusivity never sends.
    #[must_use]
    pub fn try_acquire(repo_root: &Path, pane_index: usize) -> Option<Self> {
        let path = claim_path(repo_root, pane_index);
        fs::create_dir_all(path.parent()?).ok()?;

        match create_new(&path) {
            Ok(file) => return Some(Self { path, _file: file }),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
            Err(_) => return None,
        }

        if !is_abandoned(&path) {
            return None;
        }
        // Steal once. A concurrent stealer may win the re-create, in which case
        // `create_new` fails again and this returns `None` — the non-blocking
        // skip, never a spin.
        fs::remove_file(&path).ok()?;
        create_new(&path)
            .ok()
            .map(|file| Self { path, _file: file })
    }
}

impl Drop for PaneClaim {
    fn drop(&mut self) {
        // Best-effort release; a leftover claim is reclaimed by the next
        // acquirer's TTL steal.
        let _ = fs::remove_file(&self.path);
    }
}

/// Atomically creates `path`, failing with [`ErrorKind::AlreadyExists`] when it
/// is already there — the primitive the whole exclusion rests on.
fn create_new(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

/// Whether `path`'s last modification is older than [`CLAIM_TTL`].
///
/// A path whose metadata or mtime cannot be read, or whose mtime sits in the
/// future (clock skew), is reported as NOT abandoned: an unreadable claim is
/// never stolen.
fn is_abandoned(path: &Path) -> bool {
    fs::metadata(path)
        .and_then(|meta| meta.modified())
        .is_ok_and(|modified| modified.elapsed().is_ok_and(|age| age > CLAIM_TTL))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::FileTimes;
    use std::sync::Barrier;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::SystemTime;
    use tempfile::TempDir;

    /// Backdates `path`'s mtime by `age`, standing in for a claim whose
    /// approver was hard-killed that long ago.
    fn backdate(path: &Path, age: Duration) {
        let file = OpenOptions::new()
            .write(true)
            .open(path)
            .expect("open claim");
        let when = SystemTime::now() - age;
        file.set_times(FileTimes::new().set_accessed(when).set_modified(when))
            .expect("backdate claim mtime");
    }

    #[test]
    fn claim_path_is_the_shared_per_pane_formula() {
        let repo = TempDir::new().unwrap();
        assert_eq!(
            claim_path(repo.path(), 3),
            repo.path()
                .join(".git-paw")
                .join("tmp")
                .join("approve-pane-3.claim"),
            "the path formula is the cross-process contract sweep.sh mirrors"
        );
    }

    /// Spec scenario "An approver holds the pane's claim before sending": the
    /// claim file exists for as long as the guard is held.
    #[test]
    fn acquire_creates_the_claim_file_under_a_gitignored_tmp_dir() {
        let repo = TempDir::new().unwrap();
        let _guard = PaneClaim::try_acquire(repo.path(), 2).expect("first acquire wins");
        assert!(
            claim_path(repo.path(), 2).exists(),
            "the claim file must exist while the guard is held"
        );
    }

    /// Spec scenario "Acquisition is non-blocking when the claim is held": a
    /// second acquirer returns immediately with `None` instead of waiting, and
    /// a different pane is unaffected (the claim is per-pane, not global).
    #[test]
    fn a_held_claim_is_refused_without_blocking_and_only_for_that_pane() {
        let repo = TempDir::new().unwrap();
        let _held = PaneClaim::try_acquire(repo.path(), 2).expect("first acquire wins");
        assert!(
            PaneClaim::try_acquire(repo.path(), 2).is_none(),
            "a held pane must be refused, not waited on"
        );
        assert!(
            PaneClaim::try_acquire(repo.path(), 3).is_some(),
            "claims are per-pane; pane 3 is free while pane 2 is held"
        );
    }

    /// Spec scenario "Two approvers race on one pane — exactly one sends":
    /// concurrent acquirers contend for one pane and exactly one wins; every
    /// loser gets the non-blocking refusal. The winner holds its guard until
    /// every racer has attempted, so the count cannot be inflated by a claim
    /// released mid-race.
    ///
    /// Also covers "The claim is released after the send": once the scope ends
    /// and every guard has dropped, the file is gone and a fresh acquire wins.
    #[test]
    fn concurrent_acquirers_on_one_pane_yield_exactly_one_winner() {
        const RACERS: usize = 8;
        let repo = TempDir::new().unwrap();
        let start = Barrier::new(RACERS);
        let attempted = Barrier::new(RACERS);
        let winners = AtomicUsize::new(0);

        std::thread::scope(|scope| {
            for _ in 0..RACERS {
                scope.spawn(|| {
                    start.wait();
                    let claim = PaneClaim::try_acquire(repo.path(), 2);
                    if claim.is_some() {
                        winners.fetch_add(1, Ordering::SeqCst);
                    }
                    // Hold the winner's claim until every racer has had its
                    // attempt, so a second winner would mean a real race.
                    attempted.wait();
                    drop(claim);
                });
            }
        });

        assert_eq!(
            winners.load(Ordering::SeqCst),
            1,
            "exactly one of {RACERS} concurrent approvers may hold pane 2"
        );
        assert!(
            !claim_path(repo.path(), 2).exists(),
            "every guard dropped, so the claim must be released"
        );
        assert!(
            PaneClaim::try_acquire(repo.path(), 2).is_some(),
            "a released pane must be acquirable again"
        );
    }

    /// Spec scenario "an approver that panics/errors mid-send SHALL NOT leave
    /// pane N permanently claimed": a panic unwinding through the guard
    /// releases it just like a normal return.
    #[test]
    fn a_panic_mid_send_still_releases_the_claim() {
        let repo = TempDir::new().unwrap();
        let panicked = std::panic::catch_unwind(|| {
            let _guard = PaneClaim::try_acquire(repo.path(), 2).expect("acquire");
            panic!("approver died mid-send");
        });
        assert!(panicked.is_err(), "the test's own panic must be caught");
        assert!(
            PaneClaim::try_acquire(repo.path(), 2).is_some(),
            "unwinding through the guard must release the pane"
        );
    }

    /// A fresh claim blocks acquisition; one left untouched for longer than
    /// [`CLAIM_TTL`] is treated as abandoned by a hard-killed approver and
    /// stolen, so a crash wedges the pane for one TTL rather than forever.
    #[test]
    fn a_claim_older_than_the_ttl_is_stolen_but_a_fresh_one_is_not() {
        let repo = TempDir::new().unwrap();
        let path = claim_path(repo.path(), 2);

        // A hard-killed approver leaves the file behind with no guard to drop.
        let orphan = PaneClaim::try_acquire(repo.path(), 2).expect("acquire");
        std::mem::forget(orphan);

        assert!(
            PaneClaim::try_acquire(repo.path(), 2).is_none(),
            "a claim within the TTL is live and must not be stolen"
        );

        backdate(&path, CLAIM_TTL + Duration::from_secs(1));
        let stolen = PaneClaim::try_acquire(repo.path(), 2);
        assert!(
            stolen.is_some(),
            "a claim older than the {CLAIM_TTL:?} TTL is abandoned and may be stolen"
        );
    }
}
