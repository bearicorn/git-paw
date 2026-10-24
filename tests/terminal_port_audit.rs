//! Source audits for the terminal port (`terminal-backend-seam`).
//!
//! `code-architecture`, "Direct multiplexer invocation is confined to the
//! backend": no non-test code outside `src/tmux/`, the tmux adapter
//! (`src/terminal/tmux.rs`) and the named environment probes may spawn `tmux`,
//! execute a tmux command builder, or hand-build a `<session>:0.<pane>` target.
//! "Port vocabulary is domain-shaped": the port's public identifiers carry no
//! tmux vocabulary. "The readiness gate no longer imports supervisor code": no
//! `src/tmux/` file references `crate::supervisor`.

use std::path::{Path, PathBuf};

use regex::Regex;

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

fn relative(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Blanks out every `#[cfg(test)]` item (walking the braces of the item that
/// follows the attribute), keeping line numbers. A test may drive a private
/// tmux server directly; the rule governs production code.
fn strip_test_modules(src: &str) -> String {
    let mut out = src.to_string();
    while let Some(attr) = out.find("#[cfg(test)]") {
        let Some(open_rel) = out[attr..].find('{') else {
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

/// Where direct tmux invocation is allowed: the command-building substrate
/// and the adapter (with its argv-fixture tests).
fn is_allowed(rel: &str) -> bool {
    rel.starts_with("src/tmux/")
        || rel == "src/terminal/tmux.rs"
        || rel == "src/terminal/tmux_tests.rs"
}

/// The environment probes that test the tmux binary itself, allowed by
/// `(file, enclosing fn)` — not by file, so a tmux call added anywhere else
/// in `doctor.rs` or `selftest.rs` still fails the audit. `doctor`'s probe
/// spawns its tool by a name it is handed (`probe_tool("tmux", ...)`), which
/// no shape below matches, so only the `selftest` probes need naming.
const ALLOWED_PROBES: &[(&str, &str)] = &[
    ("src/selftest.rs", "tmux_available"),
    ("src/selftest.rs", "session_on_private_socket"),
    ("src/selftest.rs", "cleanup"),
];

/// The shapes that mean "this code drives tmux itself".
fn invocation_shapes() -> Vec<(&'static str, Regex)> {
    [
        ("tmux process", r#"Command::new\(\s*"tmux"\s*\)"#),
        ("CommandRunner tmux call", r#"\brun(_inheriting_stdio)?\(\s*"tmux""#),
        (
            "tmux command builder",
            r"\b(TmuxCommand|TmuxSession|TmuxSessionBuilder|build_supervisor_session|build_add_agent_commands|build_remove_retile_commands|build_boot_inject_args)\b",
        ),
        ("hand-built pane target", r#":0\.\{|:0\.\d|"\{\}:0"#),
    ]
    .into_iter()
    .map(|(name, re)| (name, Regex::new(re).expect("audit regex compiles")))
    .collect()
}

/// Every violation in `body` (a file at `rel`), as `rel:line (shape): text`.
fn violations(rel: &str, body: &str) -> Vec<String> {
    if is_allowed(rel) {
        return Vec::new();
    }
    let code = strip_test_modules(body);
    let shapes = invocation_shapes();
    // The enclosing `fn` of each line: the nearest declaration at or above it.
    let decl = Regex::new(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)").expect("fn-decl regex compiles");
    let mut current_fn: Option<String> = None;
    let mut found = Vec::new();
    for (idx, line) in code.lines().enumerate() {
        if let Some(c) = decl.captures(line) {
            current_fn = Some(c[1].to_string());
        }
        if line.trim_start().starts_with("//") {
            continue;
        }
        let in_probe = current_fn
            .as_deref()
            .is_some_and(|f| ALLOWED_PROBES.contains(&(rel, f)));
        if in_probe {
            continue;
        }
        for (name, re) in &shapes {
            if re.is_match(line) {
                found.push(format!("{rel}:{} ({name}): {}", idx + 1, line.trim()));
            }
        }
    }
    found
}

/// Scenario "Source audit finds no stray tmux invocation".
#[test]
fn source_audit_finds_no_stray_tmux_invocation() {
    let mut found = Vec::new();
    for path in rust_files(&repo_root().join("src")) {
        let rel = relative(&path);
        let body = std::fs::read_to_string(&path).expect("read source");
        found.extend(violations(&rel, &body));
    }
    assert!(
        found.is_empty(),
        "tmux driven outside the terminal backend:\n{}",
        found.join("\n")
    );
}

/// Scenario "A reintroduced direct call fails the audit".
#[test]
fn a_reintroduced_direct_call_fails_the_audit_and_names_the_file() {
    let rel = "src/supervisor/drive.rs";
    let body = std::fs::read_to_string(repo_root().join(rel)).expect("read drive.rs");
    let reintroduced = format!(
        "{body}\nfn sneaky() {{ let _ = std::process::Command::new(\"tmux\").arg(\"ls\").status(); }}\n"
    );
    let found = violations(rel, &reintroduced);
    assert_eq!(found.len(), 1, "exactly the reintroduced call: {found:?}");
    assert!(
        found[0].starts_with("src/supervisor/drive.rs:"),
        "{found:?}"
    );
    // Each shape is caught, and test modules stay out of scope.
    for line in [
        "runner.run(\"tmux\", &[\"ls\"]);",
        "let s = TmuxSessionBuilder::new(\"p\");",
        "let t = format!(\"{session}:0.{idx}\");",
    ] {
        assert_eq!(violations(rel, line).len(), 1, "{line}");
    }
    assert!(
        violations(
            rel,
            "#[cfg(test)]\nmod tests { fn t() { Command::new(\"tmux\"); } }"
        )
        .is_empty()
    );
    assert!(violations("src/tmux/session.rs", "Command::new(\"tmux\")").is_empty());
    // The probe exemption is by function: the named probe passes, a new tmux
    // call elsewhere in the same file does not.
    let probe = "fn tmux_available() -> bool {\n    Command::new(\"tmux\").status().is_ok()\n}\n";
    assert!(violations("src/selftest.rs", probe).is_empty());
    let stray = "fn report() {\n    Command::new(\"tmux\").arg(\"ls\").status();\n}\n";
    assert_eq!(violations("src/selftest.rs", stray).len(), 1);
    assert_eq!(violations("src/doctor.rs", stray).len(), 1);
}

/// Public identifiers declared in `src` (items, fields, variants, methods).
fn declared_identifiers(src: &str) -> Vec<String> {
    let item = Regex::new(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:fn|struct|enum|trait|type|const|mod)\s+([A-Za-z_][A-Za-z0-9_]*)")
        .expect("item regex");
    let field = Regex::new(r"^\s*pub\s+([a-z_][a-z0-9_]*)\s*:").expect("field regex");
    let variant = Regex::new(r"^\s{4}([A-Z][A-Za-z0-9]*)\s*[,({]").expect("variant regex");
    let mut out = Vec::new();
    for line in strip_test_modules(src).lines() {
        if line.trim_start().starts_with("//") {
            continue;
        }
        for re in [&item, &field, &variant] {
            if let Some(c) = re.captures(line) {
                out.push(c[1].to_string());
            }
        }
    }
    out
}

/// Scenario "Port vocabulary is domain-shaped".
#[test]
fn port_vocabulary_is_domain_shaped() {
    let src = std::fs::read_to_string(repo_root().join("src/terminal/mod.rs")).expect("read port");
    let ids = declared_identifiers(&src);
    for expected in [
        "TerminalBackend",
        "SurfaceId",
        "capture_tail",
        "foreground_process",
        "send_text",
        "send_key",
        "stream_to",
        "list_surfaces",
        "owner",
    ] {
        assert!(
            ids.iter().any(|i| i == expected),
            "{expected} not found in {ids:?}"
        );
    }
    let banned = [
        "tmux",
        "pane",
        "send_keys",
        "capture_pane",
        "pipe_pane",
        "split_window",
        "pane_current_path",
    ];
    let leaks: Vec<&String> = ids
        .iter()
        .filter(|id| {
            let lower = id.to_lowercase();
            // `pub mod tmux` names the in-tree adapter module, not the port.
            id.as_str() != "tmux" && banned.iter().any(|b| lower.contains(b))
        })
        .collect();
    assert!(leaks.is_empty(), "tmux vocabulary in the port: {leaks:?}");
}

/// Scenario "The readiness gate no longer imports supervisor code".
#[test]
fn no_tmux_module_imports_supervisor_code() {
    let offenders: Vec<String> = rust_files(&repo_root().join("src/tmux"))
        .into_iter()
        .filter(|p| {
            std::fs::read_to_string(p)
                .expect("read")
                .contains("crate::supervisor")
        })
        .map(|p| relative(&p))
        .collect();
    assert!(
        offenders.is_empty(),
        "src/tmux depends on supervisor: {offenders:?}"
    );
}

/// Scenario "tmux handle conversion is confined to the backend": only the
/// adapters construct a `SurfaceId` from their own encoding.
#[test]
fn surface_handle_construction_is_confined_to_the_backends() {
    let ctor = Regex::new(r"SurfaceId::new\(|SurfaceId\(").expect("ctor regex");
    let mut offenders = Vec::new();
    for path in rust_files(&repo_root().join("src")) {
        let rel = relative(&path);
        if rel.starts_with("src/terminal/") {
            continue;
        }
        let body = std::fs::read_to_string(&path).expect("read");
        if ctor.is_match(&body) {
            offenders.push(rel);
        }
    }
    assert!(
        offenders.is_empty(),
        "SurfaceId built outside src/terminal: {offenders:?}"
    );
}

/// Scenario "No runtime plugin loading": the dependency set is the approved
/// one (guarded elsewhere) and the port loads nothing dynamically.
#[test]
fn the_port_loads_no_adapter_at_runtime() {
    for path in rust_files(&repo_root().join("src/terminal")) {
        let body = std::fs::read_to_string(&path).expect("read");
        for token in ["libloading", "dlopen", "Library::new"] {
            assert!(!body.contains(token), "{}: {token}", relative(&path));
        }
    }
    let manifest = std::fs::read_to_string(repo_root().join("Cargo.toml")).expect("manifest");
    assert!(
        !manifest.contains("libloading"),
        "no dynamic-loading dependency"
    );
}

/// Scenario "tmux backend answers the owner query from `pane_current_path`"
/// (second THEN): the drive loop reads owners through the port, never the
/// multiplexer's path variable.
#[test]
fn the_drive_loop_source_never_names_the_multiplexer_path_variable() {
    // Production code only: the loop's argv-parity test names tmux's own
    // listing format to pin what the tmux backend emits.
    let src = std::fs::read_to_string(repo_root().join("src/supervisor/drive.rs")).expect("read");
    assert!(!strip_test_modules(&src).contains("pane_current_path"));
}

/// Scenario "Text and keys are separate operations": no port operation takes
/// both a `Key` and free text, so typing and submitting are always two calls.
#[test]
fn no_port_operation_types_text_and_submits_it() {
    let src = std::fs::read_to_string(repo_root().join("src/terminal/mod.rs")).expect("read port");
    let sig = Regex::new(r"(?s)\bfn\s+(\w+)\s*\(([^)]*)\)").expect("signature regex");
    for c in sig.captures_iter(&src) {
        let (name, params) = (&c[1], &c[2]);
        let takes_key = params.contains(": Key");
        let takes_text = params.contains("text: &str");
        assert!(
            !(takes_key && takes_text),
            "{name} types text and sends a key"
        );
    }
    assert!(src.contains("fn send_text(") && src.contains("fn send_key("));
}
