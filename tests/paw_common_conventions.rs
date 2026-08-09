//! Drift guard for the shared shell preamble `assets/scripts/_paw_common.sh`.
//!
//! Mirrors `sweep_sh_conventions` / `broker_sh_conventions` for the
//! de-duplication contract (`core-init` / "Init installs the shared shell
//! preamble"): every bundled helper SHALL source the preamble and SHALL NOT
//! re-define a function the preamble provides. Without this guard a later edit
//! can silently reintroduce an inline copy — the exact drift this change
//! retired — and nothing would fail until the two copies diverged in behavior.

use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

/// Helpers that source the shared preamble.
const HELPERS: &[&str] = &["broker.sh", "sweep.sh", "docs-fetch.sh"];

/// The preamble itself plus every helper that sources it.
const ALL_SCRIPTS: &[&str] = &["_paw_common.sh", "broker.sh", "sweep.sh", "docs-fetch.sh"];

/// Functions defined once in `_paw_common.sh`. A helper re-declaring any of
/// them has forked the shared definition.
const SHARED_FUNCTIONS: &[&str] = &[
    "repo_root",
    "discover_broker_url",
    "discover_docs_base_url",
    "slugify",
];

fn script_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("scripts")
        .join(name)
}

fn read_script(name: &str) -> String {
    let path = script_path(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Returns the script's non-comment lines, so a comment mentioning a function
/// name or the source line never stands in for the real thing.
fn code_lines(body: &str) -> impl Iterator<Item = &str> {
    body.lines().filter(|l| !l.trim_start().starts_with('#'))
}

/// Scenario `Bundled helpers source the shared preamble and do not re-define
/// it`, part (a): each helper sources `_paw_common.sh`, resolved relative to
/// its own location so the same line works from `assets/scripts/` and from a
/// deployed `.git-paw/scripts/`.
#[test]
fn every_helper_sources_the_shared_preamble() {
    for helper in HELPERS {
        let body = read_script(helper);
        let sources = code_lines(&body)
            .any(|l| l.contains(". \"${_PAW_COMMON}\"") || l.contains("source \"${_PAW_COMMON}\""));
        assert!(
            sources,
            "{helper} must source the shared preamble via `. \"${{_PAW_COMMON}}\"`"
        );

        let resolves_relative = code_lines(&body).any(|l| {
            l.contains("_PAW_COMMON=")
                && l.contains("BASH_SOURCE[0]")
                && l.contains("_paw_common.sh")
        });
        assert!(
            resolves_relative,
            "{helper} must resolve _paw_common.sh relative to its own location \
             (`$(dirname \"${{BASH_SOURCE[0]:-$0}}\")`), so the source works from both \
             assets/scripts/ and a deployed .git-paw/scripts/"
        );
    }
}

/// Scenario `Bundled helpers source the shared preamble and do not re-define
/// it`, part (b): no helper carries its own definition of a shared function.
#[test]
fn no_helper_redefines_a_shared_function() {
    for helper in HELPERS {
        let body = read_script(helper);
        for func in SHARED_FUNCTIONS {
            let definition = format!("{func}() {{");
            let offenders: Vec<String> = code_lines(&body)
                .filter(|l| l.trim_start().starts_with(&definition))
                .map(|l| l.trim().to_string())
                .collect();
            assert!(
                offenders.is_empty(),
                "{helper} re-defines `{func}`, which `_paw_common.sh` provides — \
                 delete the inline copy and rely on the sourced preamble.\n  {}",
                offenders.join("\n  ")
            );
        }
    }
}

/// The preamble is the single definition site: every shared function it is
/// supposed to provide is actually defined there. Pairs with the test above so
/// "nobody defines it" can never pass by the function having vanished.
#[test]
fn the_preamble_defines_every_shared_function() {
    let body = read_script("_paw_common.sh");
    for func in SHARED_FUNCTIONS {
        let definition = format!("{func}() {{");
        assert!(
            code_lines(&body).any(|l| l.trim_start().starts_with(&definition)),
            "_paw_common.sh must define `{func}` — it is the single definition site"
        );
    }
}

/// Design decision D2 — fail loudly if the preamble is missing. A helper
/// deployed without its sibling `_paw_common.sh` SHALL exit non-zero with a
/// one-line diagnostic rather than run on undefined functions (which would let
/// it silently mis-resolve the broker URL).
#[test]
fn a_helper_without_the_preamble_fails_loudly() {
    for helper in HELPERS {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let lone = tmp.path().join(helper);
        std::fs::copy(script_path(helper), &lone).expect("copy helper");

        let out = StdCommand::new("bash")
            .arg(&lone)
            .output()
            .expect("run helper without preamble");
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();

        assert!(
            !out.status.success(),
            "{helper} must exit non-zero without _paw_common.sh; stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("_paw_common.sh"),
            "{helper} must name the missing preamble in its diagnostic; stderr:\n{stderr}"
        );
    }
}

/// Part (c): all four scripts parse. Guards the embedded-Python heredoc
/// quote-tracking landmine that a move between files can easily trip.
#[test]
fn all_bundled_scripts_parse_under_bash_n() {
    for script in ALL_SCRIPTS {
        let path = script_path(script);
        let out = StdCommand::new("bash")
            .arg("-n")
            .arg(&path)
            .output()
            .expect("run bash -n");
        assert!(
            out.status.success(),
            "bash -n must accept {script}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
