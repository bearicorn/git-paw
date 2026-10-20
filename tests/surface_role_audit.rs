//! Source audit for the typed surface role (`supervisor-role-typing`).
//!
//! Spec scenario "No pane-index literal or role-string comparison in decision
//! code" (`code-architecture`): the supervisor, coordination, session and
//! command modules must decide a surface's role by matching on
//! `crate::surface::SurfaceRole`, never by comparing a pane index to a numeric
//! literal and never by comparing an agent id against the broker wire identity
//! `"supervisor"`. The wire-identity comparisons that identify a *message
//! sender, a roster row or a pane label* are allowed by name below.
//!
//! The audit also pins the dependency direction the change restores: the tmux
//! adapter imports its layout geometry from `crate::surface`, not from
//! `crate::supervisor`.

use std::path::{Path, PathBuf};

use regex::Regex;

/// Repository root (the crate directory).
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every `.rs` file under `dir`, recursively, sorted for determinism.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(rust_files(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// The modules the scenario names: the supervisor, coordination, session and
/// command modules. `src/surface.rs` — the layout module that legitimately
/// owns the reserved indices — is deliberately NOT scanned.
fn scanned_files() -> Vec<(String, String)> {
    let root = repo_root();
    let mut paths: Vec<PathBuf> = Vec::new();
    paths.extend(rust_files(&root.join("src/supervisor")));
    paths.extend(rust_files(&root.join("src/coordination")));
    paths.extend(rust_files(&root.join("src/commands")));
    paths.push(root.join("src/session.rs"));
    paths
        .into_iter()
        .filter(|p| p.exists())
        .map(|p| {
            let rel = p
                .strip_prefix(&root)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            let body =
                std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", rel.clone()));
            (rel, strip_test_modules(&body))
        })
        .collect()
}

/// Blanks out every `#[cfg(test)]` item, leaving exactly the production code
/// the rule governs.
///
/// Each block is removed by walking the braces of the item that follows the
/// attribute, so a module with SEVERAL test modules (e.g.
/// `src/commands/supervisor.rs`) keeps every line between them. Truncating at
/// the first attribute instead would silently drop most of that file —
/// including the whole supervised launch path.
///
/// Removed lines are replaced by blank lines so reported line numbers still
/// match the file on disk.
fn strip_test_modules(src: &str) -> String {
    let mut out = src.to_string();
    while let Some(attr) = out.find("#[cfg(test)]") {
        let Some(open_rel) = out[attr..].find('{') else {
            // No braced item follows — nothing more to strip.
            break;
        };
        let open = attr + open_rel;
        let mut depth: usize = 0;
        let mut end = out.len();
        for (i, ch) in out[open..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = open + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        let blanked: String = out[attr..end]
            .chars()
            .map(|c| if c == '\n' { '\n' } else { ' ' })
            .collect();
        out.replace_range(attr..end, &blanked);
    }
    out
}

/// Whether `line` is a comment — prose that names a constant decides
/// nothing, so the rules below skip it.
fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with("//")
}

/// The name of the `fn` enclosing line `idx`, found by walking back to the
/// nearest `fn <name>` declaration. `None` before the first one.
fn enclosing_fn(lines: &[&str], idx: usize) -> Option<String> {
    let decl = Regex::new(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)").expect("fn-decl regex compiles");
    lines[..=idx]
        .iter()
        .rev()
        .find_map(|l| decl.captures(l).map(|c| c[1].to_string()))
}

/// Comparisons against the broker wire identity that identify a message
/// sender, a roster row or a pane label — not the role of a surface. Allowed
/// by `(file, enclosing fn)`.
const ALLOWED_WIRE_IDENTITY_SITES: &[(&str, &str)] = &[
    // The completion check reads the supervisor's own roster row.
    ("src/supervisor/drive.rs", "detect_completion"),
    // The gate-failure handler filters on the message's `from` sender.
    ("src/supervisor/drive.rs", "gate_failure"),
    // The inventory's candidate list excludes the supervisor roster row.
    ("src/coordination/inventory.rs", "candidate_ids"),
    // `match_pane` keys the supervisor's roster row; the INDEX it answers
    // comes from the layout record, which is what the rule is about.
    ("src/coordination/inventory.rs", "match_pane"),
    // `normalize_id` excludes the reserved wire identity from branch
    // slugification when comparing a `/tell` target against a roster row.
    // `supervisor` is an id, not a branch: slugifying it would yield
    // `agent-supervisor` and make the supervisor unaddressable by name. No
    // surface role is decided here — `match_pane` above still answers the
    // supervisor's index from the layout record.
    ("src/coordination/inventory.rs", "normalize_id"),
    // `git paw remove supervisor` is refused by name — a CLI argument, not a
    // surface role.
    ("src/commands/remove.rs", "cmd_remove"),
];

/// The audit is only as good as what it reads. `src/commands/supervisor.rs`
/// carries SEVERAL `#[cfg(test)]` modules, the first of them a third of the
/// way into the file; a stripper that truncated at the first one would skip
/// the entire supervised launch path — the code this change rewired — while
/// still reporting green. This pins that the production code survives
/// stripping and the test code does not.
#[test]
fn stripping_test_modules_keeps_the_production_code_the_rules_govern() {
    let files = scanned_files();
    let supervisor = files
        .iter()
        .find(|(p, _)| p == "src/commands/supervisor.rs")
        .map(|(_, s)| s.clone())
        .expect("src/commands/supervisor.rs is scanned");

    for marker in [
        "pub(crate) fn cmd_supervisor(", // the supervised launch path
        "fn drive_unattended_loop(",     // the drive-loop wiring
        "recorded_dashboard_pane()",     // a recorded index, however it is written
    ] {
        assert!(
            supervisor.contains(marker),
            "stripping removed production code containing `{marker}`"
        );
    }
    assert!(
        !supervisor.contains("mod interactive_cli_selection_tests"),
        "the first `#[cfg(test)]` module survived stripping"
    );
    assert!(
        !supervisor.contains("#[cfg(test)]"),
        "a `#[cfg(test)]` block survived stripping"
    );

    // Line numbers in violation messages must still match the file on disk.
    let on_disk = std::fs::read_to_string(repo_root().join("src/commands/supervisor.rs"))
        .expect("read src/commands/supervisor.rs");
    assert_eq!(
        supervisor.lines().count(),
        on_disk.lines().count(),
        "stripping must preserve line numbering"
    );
}

/// The spellings a role decision against the broker wire identity can take:
/// the constant or the literal on EITHER side of an equality, and the pattern
/// forms that compare without an operator — a `matches!` arm or a `match` arm.
/// A role decision reads the same however it is written, so the audit must
/// catch all of them.
fn wire_identity_shapes() -> Vec<(&'static str, Regex)> {
    let wire = r#"(?:SUPERVISOR_AGENT_ID|"supervisor")"#;
    vec![
        (
            "compared on the right",
            Regex::new(&format!(r"(?:==|!=)\s*{wire}")).expect("rhs regex compiles"),
        ),
        (
            "compared on the left",
            Regex::new(&format!(r"{wire}\s*(?:==|!=)")).expect("lhs regex compiles"),
        ),
        (
            "matched as a pattern",
            // `matches!(x, SUPERVISOR_AGENT_ID)` or a `… => ` match arm.
            Regex::new(&format!(
                r"matches!\s*\([^)]*{wire}|{wire}\s*(?:\|[^=]*)?=>"
            ))
            .expect("pattern regex compiles"),
        ),
    ]
}

/// The audit's own regexes are the thing most likely to rot into a vacuous
/// pass, so each spelling is exercised against a line that must be caught and
/// the set as a whole against lines that must not be.
#[test]
fn every_wire_identity_spelling_is_caught() {
    let shapes = wire_identity_shapes();
    let caught = |line: &str| shapes.iter().any(|(_, r)| r.is_match(line));

    for line in [
        r"if agent_id == SUPERVISOR_AGENT_ID {",
        r#"if agent_id == "supervisor" {"#,
        r"if SUPERVISOR_AGENT_ID == agent_id {",
        r#"if "supervisor" == agent_id {"#,
        r"if payload.from != SUPERVISOR_AGENT_ID {",
        r"if matches!(id, SUPERVISOR_AGENT_ID) {",
        r"    SUPERVISOR_AGENT_ID => SurfaceRole::Supervisor,",
        r#"    "supervisor" => SurfaceRole::Supervisor,"#,
        r#"    "supervisor" | "dashboard" => None,"#,
    ] {
        assert!(caught(line), "the audit must catch: {line}");
    }

    for line in [
        r#"pub const SUPERVISOR_AGENT_ID: &str = "supervisor";"#,
        r"use crate::surface::SUPERVISOR_AGENT_ID;",
        r"agent_id: SUPERVISOR_AGENT_ID.to_string(),",
        r"if role.is_supervisor() {",
        r#"let tmpl = resolve("supervisor")?;"#,
    ] {
        assert!(!caught(line), "the audit must not flag: {line}");
    }
}

#[test]
fn no_agent_id_string_decides_a_surface_role() {
    let shapes = wire_identity_shapes();
    let mut violations: Vec<String> = Vec::new();

    for (path, src) in scanned_files() {
        let lines: Vec<&str> = src.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if is_comment(line) {
                continue;
            }
            let Some((shape, _)) = shapes.iter().find(|(_, r)| r.is_match(line)) else {
                continue;
            };
            let owner = enclosing_fn(&lines, i).unwrap_or_else(|| "<module level>".to_string());
            if ALLOWED_WIRE_IDENTITY_SITES
                .iter()
                .any(|(f, n)| *f == path && *n == owner)
            {
                continue;
            }
            violations.push(format!(
                "{path}:{} (fn {owner}, {shape}): {}",
                i + 1,
                line.trim()
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "a surface role must be decided by matching on `SurfaceRole`, not by comparing an \
         agent id against the supervisor wire identity. Offending sites:\n  {}\n\n\
         If a site identifies a broker message sender, a roster row or a pane label rather \
         than the role of a surface, add it to ALLOWED_WIRE_IDENTITY_SITES with a reason.",
        violations.join("\n  ")
    );
}

/// A FORWARD guard only: verified to pass on the pre-change tree too, because
/// the old code reached pane 0 through a `match pane_index { CONST => … }` arm
/// and named constants rather than an `==` comparison. Those constants are
/// what [`no_reserved_pane_index_is_written_as_a_literal`] catches; this test
/// exists so the next person cannot reintroduce the fact in comparison form.
#[test]
fn no_pane_index_is_compared_against_a_numeric_literal() {
    // `<something>_index / _idx / _pane / _offset == 0` in either direction.
    let forward = Regex::new(
        r"\b[A-Za-z_][A-Za-z0-9_]*(?:pane|index|idx|offset)\s*(?:\(\))?\s*(?:==|!=)\s*(?:Some\()?[0-9]",
    )
    .expect("forward regex compiles");
    let backward = Regex::new(
        r"(?:^|[^A-Za-z0-9_])[0-9]+\s*(?:==|!=)\s*[A-Za-z_][A-Za-z0-9_]*(?:pane|index|idx|offset)",
    )
    .expect("backward regex compiles");
    let mut violations: Vec<String> = Vec::new();

    for (path, src) in scanned_files() {
        for (i, line) in src.lines().enumerate() {
            if !is_comment(line) && (forward.is_match(line) || backward.is_match(line)) {
                violations.push(format!("{path}:{}: {}", i + 1, line.trim()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "a reserved pane index must come from `crate::surface::SessionLayout`, never from a \
         comparison against a numeric literal. Offending sites:\n  {}",
        violations.join("\n  ")
    );
}

#[test]
fn no_reserved_pane_index_is_written_as_a_literal() {
    // The other half of "no pane-index literal": a reserved index that is
    // DECLARED, RECORDED or RETURNED as a literal re-derives the layout fact
    // just as surely as a comparison does.
    let patterns: Vec<(&str, Regex)> = vec![
        (
            "a reserved-index constant declared outside the layout module",
            Regex::new(r"const\s+[A-Z_]*PANE_(?:INDEX|OFFSET)\b").expect("regex compiles"),
        ),
        (
            "`SUPERVISOR_PANE_OFFSET` used to compute an index",
            Regex::new(r"\bSUPERVISOR_PANE_OFFSET\b").expect("regex compiles"),
        ),
        (
            "a `pane_index` recorded as a literal",
            Regex::new(r"\bpane_index\s*:\s*[0-9]").expect("regex compiles"),
        ),
        (
            "a `dashboard_pane` recorded as a literal",
            Regex::new(r"\bdashboard_pane\s*(?::|=)\s*Some\(\s*[0-9]").expect("regex compiles"),
        ),
        (
            "a reserved index returned as a literal",
            Regex::new(r"\breturn\s+Some\(\s*[0-9]\s*\)").expect("regex compiles"),
        ),
    ];
    let mut violations: Vec<String> = Vec::new();

    for (path, src) in scanned_files() {
        // The transitional re-export shim names every moved geometry symbol
        // and decides nothing. `supervisor-pane-removal` deletes it.
        if path == "src/supervisor/layout.rs" {
            continue;
        }
        for (i, line) in src.lines().enumerate() {
            let trimmed = line.trim();
            // An import names the symbol without deciding anything with it.
            if is_comment(trimmed) || trimmed.starts_with("use ") || trimmed.starts_with("pub use ")
            {
                continue;
            }
            for (why, pattern) in &patterns {
                if pattern.is_match(line) {
                    violations.push(format!("{path}:{} ({why}): {trimmed}", i + 1));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "every reserved index must be answered by `crate::surface::SessionLayout`, not written \
         as a literal. Offending sites:\n  {}",
        violations.join("\n  ")
    );
}

#[test]
fn the_tmux_adapter_imports_no_layout_geometry_from_the_supervisor_module() {
    let root = repo_root();
    let mut violations: Vec<String> = Vec::new();

    for path in rust_files(&root.join("src/tmux")) {
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {rel}: {e}"));
        for (i, line) in src.lines().enumerate() {
            if !line.contains("supervisor::") {
                continue;
            }
            // `tmux/readiness.rs` imports the supervisor's screen-capture
            // helper. That is terminal I/O, not layout geometry, and
            // `terminal-backend-seam` moves it behind the terminal port.
            if line.contains("supervisor::permission_prompt::capture_pane") {
                continue;
            }
            violations.push(format!("{rel}:{}: {}", i + 1, line.trim()));
        }
    }

    assert!(
        violations.is_empty(),
        "the tmux adapter must take its layout geometry from `crate::surface`, not from \
         `crate::supervisor` — dependencies point inward. Offending sites:\n  {}",
        violations.join("\n  ")
    );
}

#[test]
fn the_supervisor_wire_identity_has_exactly_one_definition() {
    let root = repo_root();
    let definition = Regex::new(r#"const\s+SUPERVISOR_AGENT_ID\s*:\s*&str\s*=\s*"supervisor""#)
        .expect("definition regex compiles");
    let mut definitions: Vec<String> = Vec::new();

    for path in rust_files(&root.join("src")) {
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {rel}: {e}"));
        for (i, line) in src.lines().enumerate() {
            if definition.is_match(line) {
                definitions.push(format!("{rel}:{}", i + 1));
            }
        }
    }

    assert_eq!(
        definitions.len(),
        1,
        "the supervisor's broker wire identity must be defined in exactly one place \
         (`crate::surface`); every other module re-exports it. Definitions found:\n  {}",
        definitions.join("\n  ")
    );
    assert!(
        definitions[0].starts_with("src/surface.rs:"),
        "the single definition must live in the layout module, found at {}",
        definitions[0]
    );
}
