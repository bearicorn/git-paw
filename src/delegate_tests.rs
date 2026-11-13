//! Tests for the pure delegate-run decisions.

use std::path::Path;

use super::*;

fn run(profile: &str, level: ApprovalLevel, pid: u32) -> ActiveRun {
    ActiveRun {
        profile: profile.to_string(),
        level,
        pid,
        pgid: None,
        deadline: None,
        program: None,
        leader_start: None,
    }
}

/// Scenario "The document carries framing, role skill and task in order".
#[test]
fn the_document_carries_framing_role_skill_and_task_in_order() {
    let framing = framing("spec-audit", Path::new("/repo"), ApprovalLevel::Manual);
    let doc = document(
        &framing,
        "SPEC-AUDITOR-ROLE",
        "audit feat/x at abc123, change add-x",
    );
    let f = doc.find("one-shot delegate").expect("framing");
    let r = doc.find("SPEC-AUDITOR-ROLE").expect("role skill");
    let t = doc
        .find("audit feat/x at abc123, change add-x")
        .expect("task");
    assert!(f < r && r < t, "{doc}");
    assert!(doc.contains("`spec-audit`"), "framing names the profile");
    assert!(doc.contains("`/repo`"), "framing names the root");
    assert!(doc.contains("`manual`"), "framing names the level");
    assert!(
        doc.trim_end()
            .ends_with("audit feat/x at abc123, change add-x")
    );
}

/// Scenario "Oversized output is truncated" (the truncation half).
#[test]
fn oversized_output_is_truncated() {
    let big = "x".repeat(200_000);
    let out = truncate_result(&big, Path::new("/repo/.git-paw/logs/delegates/r.log"));
    let (body, last) = out.trim_end().rsplit_once('\n').expect("two parts");
    assert!(body.len() <= RESULT_CAP_BYTES);
    assert!(last.contains("truncated"), "{last}");
    assert!(
        last.contains("/repo/.git-paw/logs/delegates/r.log"),
        "{last}"
    );
    let small = "MAP: src/config\n";
    assert_eq!(truncate_result(small, Path::new("/l")), small);
}

/// The cut never splits a multi-byte character.
#[test]
fn truncation_keeps_utf8_intact() {
    let big = "é".repeat(RESULT_CAP_BYTES);
    let out = truncate_result(&big, Path::new("/l"));
    assert!(out.is_char_boundary(out.len()));
    assert!(out.contains("truncated"));
}

/// Scenario "A fifth read-only delegate is refused".
#[test]
fn a_fifth_read_only_delegate_is_refused() {
    let active: Vec<ActiveRun> = ["a", "b", "c", "d"]
        .iter()
        .enumerate()
        .map(|(i, p)| run(p, ApprovalLevel::Manual, u32::try_from(i).unwrap()))
        .collect();
    let err = admit(&active, ApprovalLevel::Manual).expect_err("fifth");
    for p in ["a", "b", "c", "d"] {
        assert!(err.contains(p), "{err}");
    }
    assert!(admit(&active[..3], ApprovalLevel::Manual).is_ok());
}

/// Scenario "A writing delegate runs alone".
#[test]
fn a_writing_delegate_runs_alone() {
    let reader = [run("explorer", ApprovalLevel::Manual, 1)];
    assert!(admit(&reader, ApprovalLevel::Auto).is_err());
    assert!(admit(&[], ApprovalLevel::Auto).is_ok());
    let writer = [run("fixer", ApprovalLevel::Auto, 2)];
    assert!(admit(&writer, ApprovalLevel::Manual).is_err());
    assert!(admit(&writer, ApprovalLevel::Auto).is_err());
}

/// Scenario "A dead run does not block admission".
#[test]
fn a_dead_run_does_not_block_admission() {
    let dead = run("a", ApprovalLevel::Auto, 10);
    assert_eq!(classify(&dead, 0, false, false), RecordState::Dead);
    let live = run("b", ApprovalLevel::Manual, 11);
    assert_eq!(classify(&live, 0, true, false), RecordState::Live);
    assert!(admit(&[live], ApprovalLevel::Manual).is_ok());
}

/// The ledger line carries metadata only, with the exit code of a failed
/// run.
#[test]
fn the_ledger_line_holds_metadata() {
    let line = LedgerLine::new(
        "spec-audit",
        "agent",
        Some("sid"),
        1_700_000_000,
        1.5,
        Outcome::Failed(Some(3)),
    );
    let json: serde_json::Value = serde_json::from_str(&line.to_json_line()).unwrap();
    assert_eq!(json["profile"], "spec-audit");
    assert_eq!(json["cli"], "agent");
    assert_eq!(json["session_id"], "sid");
    assert_eq!(json["outcome"], "failed");
    assert_eq!(json["exit_code"], 3);
    let ok = LedgerLine::new("e", "agent", None, 1, 0.1, Outcome::Ok).to_json_line();
    assert!(ok.contains("\"session_id\":null"), "{ok}");
    assert!(!ok.contains("exit_code"), "{ok}");
}

/// Scenario "The framing is an audited asset": the framing sentences come
/// from the bundled asset, which names no vendor product, model or
/// toolchain.
#[test]
fn the_framing_is_an_audited_asset() {
    let source = include_str!("delegate.rs");
    assert!(source.contains("include_str!(\"../assets/delegate-framing.md\")"));
    let asset = include_str!("../assets/delegate-framing.md");
    let rendered = framing("x", Path::new("/r"), ApprovalLevel::Auto);
    assert!(rendered.starts_with(asset.lines().next().unwrap()));
    assert!(
        !source.contains("one-shot delegate: an agent"),
        "prose lives in the asset"
    );
}

/// An orphaned delegate (its holder gone, its group alive) still counts as
/// live until its deadline, and is overdue — to be killed — after it.
#[test]
fn an_orphaned_delegate_counts_until_its_deadline_then_is_overdue() {
    let orphan = ActiveRun {
        pgid: Some(42),
        deadline: Some(100),
        ..run("explorer", ApprovalLevel::Manual, 7)
    };
    assert_eq!(classify(&orphan, 50, false, true), RecordState::Live);
    assert_eq!(classify(&orphan, 100, false, true), RecordState::Overdue);
    assert_eq!(classify(&orphan, 100, true, true), RecordState::Overdue);
    assert_eq!(classify(&orphan, 150, false, false), RecordState::Dead);
    assert_eq!(
        classify(&orphan, 150, true, false),
        RecordState::Dead,
        "past the deadline a dead group is dead whatever the holder"
    );
    assert_eq!(
        classify(&run("a", ApprovalLevel::Manual, 1), 0, true, false),
        RecordState::Live
    );
}

/// pgid 0 (the caller's own group), 1 (every signallable process) and
/// git-paw's own group are never probed or signalled.
#[test]
fn only_a_real_foreign_group_is_valid() {
    assert!(!valid_pgid(0, None));
    assert!(!valid_pgid(1, None));
    assert!(!valid_pgid(4242, Some(4242)));
    assert!(valid_pgid(4242, Some(17)));
    assert!(valid_pgid(4242, None));
}

/// A reused group id is never mistaken for the delegate's group: the
/// leader must lead it and its first argv word's basename must match.
#[test]
fn the_leader_must_still_be_the_delegate_program() {
    assert!(leader_matches(
        " 4242 /opt/agent --model small -p",
        4242,
        "/opt/agent"
    ));
    assert!(leader_matches("4242 agent --batch", 4242, "/opt/bin/agent"));
    assert!(!leader_matches(
        "4242 /usr/bin/vim /opt/agent",
        4242,
        "/opt/agent"
    ));
    assert!(!leader_matches("17 /opt/agent", 4242, "/opt/agent"));
    assert!(!leader_matches("", 4242, "/opt/agent"));
    assert!(!leader_matches("4242 /opt/agent", 4242, ""));
}

/// Holder ids 0 and 1, and any id above `i32::MAX`, are never real.
#[test]
fn out_of_range_ids_count_as_dead() {
    assert!(!valid_holder(0));
    assert!(!valid_holder(1));
    assert!(!valid_holder(0x8000_0000));
    assert!(valid_holder(4242));
    assert!(!valid_pgid(0x8000_0000, None));
    assert!(!valid_pgid(u32::MAX, None));
}
