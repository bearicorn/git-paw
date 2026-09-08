//! Supervisor-side mid-session branch refresh (`supervisor-branch-refresh`).
//!
//! Rebasing rewrites history, and doing it to a branch checked out in a
//! **live worktree that a running agent is editing** is the dangerous case.
//! This module holds the pure precondition conjunction that makes the unsafe
//! cases unreachable: every one of the five gates below must independently
//! pass before a branch is rebased, and failing any single gate is a no-op
//! for that branch — never a partial or degraded attempt (design D5).
//!
//! The gate *inputs* (a fresh pane capture, the watcher's tracked
//! `modified_files`, a held [`super::claim::PaneClaim`], a `git merge-tree`
//! prediction, a `git merge-base --is-ancestor` comparison) are resolved by
//! the caller — [`crate::supervisor::drive`]'s drive loop, which already has
//! tmux and broker access — so this module stays free of tmux, HTTP, and
//! `CommandRunner` dependencies and is testable as plain data in, verdict
//! out.

/// Why a branch was left untouched during a refresh evaluation.
///
/// Ordered the way [`RefreshPreconditions::evaluate`] checks them: the
/// verified-awaiting-merge exclusion is an outright disqualification checked
/// first, then the five preconditions from the
/// `supervisor-branch-refresh` spec's "All refresh preconditions are a
/// conjunction" requirement, in the order they are listed there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// The branch has already passed the five-gate supervisor verification
    /// and is awaiting merge — rewriting it would silently invalidate that
    /// verification.
    VerifiedAwaitingMerge,
    /// The worker's working tree has uncommitted changes.
    Dirty,
    /// The worker's pane is mid-response.
    MidResponse,
    /// The supervisor could not take (or does not hold) the pane's exclusive
    /// approval claim.
    ClaimUnavailable,
    /// The rebase is predicted to conflict.
    PredictedConflict,
    /// The branch is already at or ahead of the default branch.
    AlreadyCurrent,
}

impl SkipReason {
    /// A short, human-readable label for logs and the dashboard broker-log
    /// filter — never surfaced to the worker (skips are recorded, not
    /// escalated; see [`SkipReason`]'s module docs and task 4.7).
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::VerifiedAwaitingMerge => "verified-awaiting-merge",
            Self::Dirty => "dirty",
            Self::MidResponse => "mid-response",
            Self::ClaimUnavailable => "claim-unavailable",
            Self::PredictedConflict => "predicted-conflict",
            Self::AlreadyCurrent => "already-current",
        }
    }
}

/// The refresh preconditions for one branch, already resolved to booleans by
/// the caller.
///
/// Deliberately dependency-free: no tmux, no git, no broker — every field is
/// the caller's already-computed answer to one gate, so the conjunction
/// itself is pure and can be unit-tested with plain booleans.
///
/// Six independent booleans is the correct shape here, not a smell: the spec
/// names exactly six independent yes/no preconditions, each resolved by a
/// different, unrelated signal (a broker status field, a pane capture, a
/// filesystem claim, two separate git queries). Merging them into an enum or
/// a bitset would hide which gate is which without changing the conjunction.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy)]
pub struct RefreshPreconditions {
    /// The branch has already passed verification and awaits merge.
    pub verified_awaiting_merge: bool,
    /// The worker's working tree is clean (no uncommitted changes), per the
    /// broker watcher's tracked `modified_files` (design D1 — never a fresh
    /// `git status` probe).
    pub clean: bool,
    /// The worker's pane is mid-response.
    pub mid_response: bool,
    /// The supervisor holds the pane's exclusive approval claim.
    pub claim_held: bool,
    /// The rebase is predicted to conflict (`git merge-tree`).
    pub predicted_conflict: bool,
    /// The branch is behind the default branch.
    pub behind_default: bool,
}

impl RefreshPreconditions {
    /// Evaluates the conjunction. `Ok(())` means every gate passed and the
    /// branch may be refreshed; `Err(reason)` names the first gate that
    /// failed.
    ///
    /// The verified-awaiting-merge exclusion is checked first because it is
    /// an outright disqualification independent of the worker's live state
    /// (design's resolved refresh-breadth question) — the other four
    /// checks are the spec's "All refresh preconditions are a conjunction"
    /// list in order.
    pub fn evaluate(&self) -> Result<(), SkipReason> {
        if self.verified_awaiting_merge {
            return Err(SkipReason::VerifiedAwaitingMerge);
        }
        if !self.clean {
            return Err(SkipReason::Dirty);
        }
        if self.mid_response {
            return Err(SkipReason::MidResponse);
        }
        if !self.claim_held {
            return Err(SkipReason::ClaimUnavailable);
        }
        if self.predicted_conflict {
            return Err(SkipReason::PredictedConflict);
        }
        if !self.behind_default {
            return Err(SkipReason::AlreadyCurrent);
        }
        Ok(())
    }
}

/// Distinguishing bracket tag for a branch-refresh notification, mirroring
/// the `[conflict-detector]` precedent (`src/broker/conflict.rs`) so
/// dashboards and humans can tell a supervisor-emitted refresh notice apart
/// from human-typed `agent.feedback`.
pub const BRANCH_REFRESH_TAG: &str = "[branch-refresh]";

/// Builds the `agent.feedback` text notifying a worker that its branch HEAD
/// was rewritten by a successful refresh.
///
/// Deliberately NOT `agent.advanced-main`: that event means "the base
/// moved" and the worker-side *When main advances* discipline keys off that
/// meaning; this means "your OWN branch was rewritten", a different fact
/// that would corrupt that discipline if conflated (design D3).
#[must_use]
pub fn refreshed_notification_text(default_branch: &str) -> String {
    format!(
        "{BRANCH_REFRESH_TAG} your branch was rebased onto {default_branch} after a merge — \
         HEAD moved. Re-check file contents and commit SHAs before continuing; do not assume \
         your working tree still matches what you last read."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_pass() -> RefreshPreconditions {
        RefreshPreconditions {
            verified_awaiting_merge: false,
            clean: true,
            mid_response: false,
            claim_held: true,
            predicted_conflict: false,
            behind_default: true,
        }
    }

    /// Spec scenario "Clean, idle, claimed, conflict-free, behind branch is
    /// refreshed".
    #[test]
    fn every_gate_passing_permits_refresh() {
        assert_eq!(all_pass().evaluate(), Ok(()));
    }

    /// Spec scenario "Dirty working tree is never rebased".
    #[test]
    fn dirty_working_tree_is_skipped() {
        let gates = RefreshPreconditions {
            clean: false,
            ..all_pass()
        };
        assert_eq!(gates.evaluate(), Err(SkipReason::Dirty));
    }

    /// Spec scenario "Mid-response pane is not rebased".
    #[test]
    fn mid_response_pane_is_skipped() {
        let gates = RefreshPreconditions {
            mid_response: true,
            ..all_pass()
        };
        assert_eq!(gates.evaluate(), Err(SkipReason::MidResponse));
    }

    /// Spec scenario "Idle but dirty is still skipped": idleness alone must
    /// not satisfy the conjunction when cleanliness fails too — the FIRST
    /// failing gate wins (dirty), never a false pass because some other gate
    /// happened to be fine.
    #[test]
    fn idle_but_dirty_is_still_skipped_as_dirty() {
        let gates = RefreshPreconditions {
            clean: false,
            mid_response: false,
            ..all_pass()
        };
        assert_eq!(gates.evaluate(), Err(SkipReason::Dirty));
    }

    #[test]
    fn unclaimed_pane_is_skipped() {
        let gates = RefreshPreconditions {
            claim_held: false,
            ..all_pass()
        };
        assert_eq!(gates.evaluate(), Err(SkipReason::ClaimUnavailable));
    }

    /// Spec scenario "Predicted conflict skips the branch".
    #[test]
    fn predicted_conflict_is_skipped() {
        let gates = RefreshPreconditions {
            predicted_conflict: true,
            ..all_pass()
        };
        assert_eq!(gates.evaluate(), Err(SkipReason::PredictedConflict));
    }

    /// Spec scenario "Branch already current is not rebased".
    #[test]
    fn already_current_is_skipped() {
        let gates = RefreshPreconditions {
            behind_default: false,
            ..all_pass()
        };
        assert_eq!(gates.evaluate(), Err(SkipReason::AlreadyCurrent));
    }

    /// Spec scenario "Verified branch is excluded from refresh": the
    /// exclusion wins even when every other gate would have passed.
    #[test]
    fn verified_awaiting_merge_is_excluded_even_if_otherwise_eligible() {
        let gates = RefreshPreconditions {
            verified_awaiting_merge: true,
            ..all_pass()
        };
        assert_eq!(gates.evaluate(), Err(SkipReason::VerifiedAwaitingMerge));
    }

    #[test]
    fn skip_reason_labels_are_distinct() {
        let labels: std::collections::HashSet<&str> = [
            SkipReason::VerifiedAwaitingMerge,
            SkipReason::Dirty,
            SkipReason::MidResponse,
            SkipReason::ClaimUnavailable,
            SkipReason::PredictedConflict,
            SkipReason::AlreadyCurrent,
        ]
        .into_iter()
        .map(SkipReason::label)
        .collect();
        assert_eq!(
            labels.len(),
            6,
            "every skip reason must have a distinct label"
        );
    }

    /// Task 6.1/6.3: the notification carries the distinguishing tag and is
    /// text, not a message-type decision — the caller wraps it in
    /// `agent.feedback`, never `agent.advanced-main`.
    #[test]
    fn notification_text_carries_the_tag_and_names_the_default_branch() {
        let text = refreshed_notification_text("main");
        assert!(text.starts_with(BRANCH_REFRESH_TAG));
        assert!(text.contains("main"));
        assert!(text.contains("HEAD moved"));
    }
}
