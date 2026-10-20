//! The `agent_id` validator hard-coded in `sweep.sh` matches the broker's.
//!
//! `sweep.sh`'s phantom filter carries its own copy of the `/publish`
//! `agent_id` pattern. It is unreachable from Rust, so nothing made it move
//! when the broker's did — and a supervisor sweep that disagrees with the
//! broker about which ids are real would either hide a live agent from the
//! roster or admit phantom debris into it.
//!
//! Two copies of one rule drifting apart is exactly the defect
//! `agent-id-slug-safety` exists to fix (PP-26: the derivation kept `_` while
//! the validator refused it). The Rust side now shares one predicate between
//! the handler and its tests; this test extends that guarantee to the shell
//! copy the supervisor runs, by agreeing on VERDICTS over a corpus rather than
//! on pattern text — the two could legitimately be spelled differently, but
//! they may never disagree about an id.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use git_paw::broker::messages::slugify_branch;
use git_paw::broker::server::is_valid_agent_id;

#[path = "helpers/slug_corpus.rs"]
mod slug_corpus;

/// Pulls the compiled pattern out of `sweep.sh`'s phantom filter.
///
/// Deliberately pinned to the `broker_rule = re.compile(r"…")` form: if that
/// line is reshaped, this test fails rather than silently stopping checking
/// anything — a parity test that cannot find its subject is not passing, it is
/// absent.
fn sweep_sh_pattern() -> String {
    let sweep = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/scripts/sweep.sh");
    let text = std::fs::read_to_string(&sweep).expect("read bundled sweep.sh");
    let line = text
        .lines()
        .find(|l| l.trim_start().starts_with("broker_rule = re.compile("))
        .unwrap_or_else(|| {
            panic!(
                "no `broker_rule = re.compile(...)` line in {} — if the phantom filter's \
                 validator moved or was renamed, update this test to find it (or delete it, \
                 if the shell copy is gone)",
                sweep.display()
            )
        });
    let start = line.find("r\"").expect("pattern opens with r\"") + 2;
    let rest = &line[start..];
    let end = rest.find("\")").expect("pattern closes with \")");
    rest[..end].to_string()
}

/// Ids covering both verdicts: everything the derivation can produce (all
/// valid), plus the shapes the validator exists to refuse.
fn id_corpus() -> Vec<String> {
    let mut ids: Vec<String> = slug_corpus::branch_corpus()
        .iter()
        .map(|b| slugify_branch(b))
        .collect();

    for rejected in [
        "",
        "a",
        "main",
        "feat_x",
        "feat-x_y",
        "FEAT-X",
        "<agent-id>",
        "-feat-x",
        "feat-x-",
        "feat x",
        "feat/x/y",
        "feat--x",
        "supervisor-",
        "Supervisor",
    ] {
        ids.push(rejected.to_string());
    }
    ids.push("supervisor".to_string());
    ids.sort();
    ids.dedup();
    ids
}

#[test]
fn sweep_sh_agent_id_filter_agrees_with_the_broker_validator() {
    let ids = id_corpus();

    // One python run for the whole corpus: ids on stdin, one verdict per line.
    let script = r#"
import os, re, sys
rule = re.compile(os.environ["PATTERN"])
for line in sys.stdin.read().split("\n"):
    sys.stdout.write("1\n" if rule.match(line) else "0\n")
"#;
    let mut child = Command::new("python3")
        .arg("-c")
        .arg(script)
        .env("PATTERN", sweep_sh_pattern())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn python3 to run sweep.sh's pattern");

    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(ids.join("\n").as_bytes())
        .expect("write corpus");

    let out = child.wait_with_output().expect("wait for python3");
    assert!(
        out.status.success(),
        "python3 failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let verdicts: Vec<&str> = String::from_utf8(out.stdout)
        .expect("utf-8")
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .iter()
        .map(|s| if *s == "1" { "1" } else { "0" })
        .collect();
    assert_eq!(
        verdicts.len(),
        ids.len(),
        "expected one verdict per corpus id"
    );

    for (id, shell_accepts) in ids.iter().zip(verdicts) {
        let shell_accepts = shell_accepts == "1";
        assert_eq!(
            shell_accepts,
            is_valid_agent_id(id),
            "sweep.sh's agent_id filter and the broker's validator disagree on {id:?} \
             (sweep.sh accepts: {shell_accepts})"
        );
    }
}
