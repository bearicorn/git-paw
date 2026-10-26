//! `code-architecture` source audits for the command library surface:
//!
//! - "Command bodies perform no ambient I/O": no production code under
//!   `src/commands/` reads the working directory or stdin, prompts, prints,
//!   takes a stdout / stderr handle, attaches a tmux client or runs the drive
//!   loop — except the two files exempted by name.
//! - "main.rs holds no command body": `src/main.rs` defines no `cmd_`
//!   function beyond the named entry-layer ones, and none of the items that
//!   moved into the library.

use std::path::{Path, PathBuf};

use regex::Regex;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The files under `src/commands/` that are entry-layer or host code by
/// design, exempted BY NAME: `completions.rs` writes a generated completion
/// script to stdout, and `broker_host.rs` is the hidden `__broker` host, a
/// long-running process.
const EXEMPT: &[&str] = &["completions.rs", "broker_host.rs"];

/// What production code under `src/commands/` must not contain, with the
/// reason each is forbidden.
fn forbidden() -> Vec<(Regex, &'static str)> {
    [
        (r"\bcurrent_dir\(\s*\)", "reads the working directory"),
        (r"\bstdin\(\s*\)", "reads stdin"),
        (r"\bdialoguer\b", "prompts interactively"),
        (
            r"\bTerminalPrompter\b",
            "constructs the terminal picker itself",
        ),
        (r"\bis_terminal\b", "probes the terminal"),
        (r"\b(e?println|e?print)!\s*\(", "prints"),
        (r"\bstdout\(\s*\)", "takes a stdout handle"),
        (r"\bstderr\(\s*\)", "takes a stderr handle"),
        (r"\btmux::attach\b", "attaches a tmux client"),
        (
            r"\buse [^;]*\btmux::[^;]*\battach\b",
            "imports the tmux attach",
        ),
        (
            r"\.attach\s*\(",
            "attaches a terminal client through the port",
        ),
        (r"\b(run_)?drive_loop(_logged)?\s*\(", "runs the drive loop"),
    ]
    .into_iter()
    .map(|(pattern, why)| (Regex::new(pattern).expect("valid pattern"), why))
    .collect()
}

/// Blanks every `#[cfg(test)]` item (by walking the braces of the item that
/// follows the attribute) and every comment line, keeping line numbers, so
/// only production code is audited.
fn production_code(source: &str) -> String {
    let mut out: Vec<char> = source.chars().collect();
    let text: Vec<char> = out.clone();
    let marker: Vec<char> = "#[cfg(test)]".chars().collect();
    let mut i = 0;
    while i + marker.len() <= text.len() {
        if text[i..i + marker.len()] == marker[..] {
            let mut j = i + marker.len();
            while j < text.len() && text[j] != '{' && text[j] != ';' {
                j += 1;
            }
            let mut end = j;
            if j < text.len() && text[j] == '{' {
                let mut depth = 0usize;
                while end < text.len() {
                    match text[end] {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    end += 1;
                }
            }
            for c in out.iter_mut().take((end + 1).min(text.len())).skip(i) {
                if *c != '\n' {
                    *c = ' ';
                }
            }
            i = end + 1;
        } else {
            i += 1;
        }
    }
    out.into_iter()
        .collect::<String>()
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("//") {
                ""
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every violation in `source`, as `line: why: text`.
fn violations(source: &str) -> Vec<String> {
    let rules = forbidden();
    production_code(source)
        .lines()
        .enumerate()
        .flat_map(|(n, line)| {
            rules
                .iter()
                .filter(|(re, _)| re.is_match(line))
                .map(move |(_, why)| format!("{}: {why}: {}", n + 1, line.trim()))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).expect("read dir") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            files.extend(rust_files(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            files.push(path);
        }
    }
    files.sort();
    files
}

/// Scenario "Command bodies perform no ambient I/O".
#[test]
fn command_bodies_perform_no_ambient_io() {
    let dir = repo_root().join("src/commands");
    let mut found = Vec::new();
    let mut audited = 0;
    for path in rust_files(&dir) {
        let name = path.file_name().expect("file name").to_string_lossy();
        if EXEMPT.contains(&name.as_ref()) {
            continue;
        }
        audited += 1;
        let source = std::fs::read_to_string(&path).expect("read source");
        for v in violations(&source) {
            found.push(format!("{}:{v}", path.display()));
        }
    }
    assert!(audited >= 15, "audited only {audited} command modules");
    assert!(
        found.is_empty(),
        "command bodies must not perform ambient I/O — write through the injected `Output`, \
         take inputs from the caller, and leave prompting, attaching and the drive loop to the \
         entry layer:\n{}",
        found.join("\n")
    );
}

/// The exemptions are by name, and each names a file that exists and really
/// is terminal / host code — so neither can go stale and quietly exempt
/// something else.
#[test]
fn only_the_two_named_files_are_exempt() {
    assert_eq!(EXEMPT, ["completions.rs", "broker_host.rs"]);
    let completions =
        std::fs::read_to_string(repo_root().join("src/commands/completions.rs")).expect("read");
    assert!(
        completions.contains("stdout()"),
        "completions.rs is exempt because it writes the script to stdout"
    );
    let host =
        std::fs::read_to_string(repo_root().join("src/commands/broker_host.rs")).expect("read");
    assert!(
        host.contains("fn cmd_broker_host("),
        "broker_host.rs is exempt because it is the hidden `__broker` host"
    );
}

/// The audit catches each forbidden shape — including a stdout / stderr handle
/// passed straight to a helper — and ignores tests and comments.
#[test]
fn the_audit_catches_each_forbidden_shape() {
    for planted in [
        "fn f() { let cwd = std::env::current_dir(); }",
        "fn f() { let mut s = String::new(); std::io::stdin().read_line(&mut s); }",
        "fn f() { dialoguer::Confirm::new(); }",
        "fn f() { let p = interactive::TerminalPrompter; }",
        "fn f() { std::io::stdin().is_terminal(); }",
        "fn f() { println!(\"x\"); }",
        "fn f() { eprintln!(\"x\"); }",
        "fn f() { print!(\"x\"); }",
        "fn f() { helper(&mut std::io::stderr()); }",
        "fn f() { write(&mut std::io::stdout()); }",
        "fn f() { tmux::attach(&name); }",
        "use crate::tmux::{self, attach};\nfn f() { attach(&name); }",
        "fn f() { backend.attach(&name); }",
        "fn f() { drive::run_drive_loop(a, b, c, d); }",
        "fn f() { drive::drive_loop(a, b); }",
        "fn f() { drive::drive_loop_logged(a, b); }",
    ] {
        assert!(!violations(planted).is_empty(), "missed: {planted}");
    }
    for clean in [
        "fn f() { out.line(\"x\"); out.warn(\"y\"); }",
        "fn f() { Command::new(\"git\").current_dir(dir); }",
        "fn f() { let b = bind_drive_loop(a, b); }",
        "// println!(\"only a comment\")\nfn f() {}",
        "fn f() {}\n#[cfg(test)]\nmod tests {\n    fn t() { println!(\"x\"); }\n}\n",
    ] {
        assert!(violations(clean).is_empty(), "false positive: {clean}");
    }
}

/// Scenario "main.rs holds no command body".
#[test]
fn main_rs_holds_no_command_body() {
    let source = std::fs::read_to_string(repo_root().join("src/main.rs")).expect("read main.rs");
    let code = production_code(&source);

    // The entry-layer functions allowed by name: the hidden `__prompt-profile`
    // helper, the `setup-cli` plan print and confirmation, the thin
    // `__dashboard` client, and the hidden `__authority` print — its lookup
    // is `git_paw::commands::authority`; only the print of the credential
    // stays here, deliberately outside the command `Output` sink.
    let allowed = [
        "cmd_prompt_profile",
        "cmd_setup_cli",
        "cmd_dashboard",
        "cmd_authority",
    ];
    let cmd_fn = Regex::new(r"\bfn (cmd_\w+)").expect("valid pattern");
    let extra: Vec<&str> = cmd_fn
        .captures_iter(&code)
        .map(|c| c.get(1).expect("name").as_str())
        .filter(|name| !allowed.contains(name))
        .collect();
    assert!(
        extra.is_empty(),
        "main.rs defines command bodies {extra:?}; they belong in git_paw::commands"
    );

    for moved in [
        "fn attach_agent",
        "struct AttachContext",
        "struct AttachedAgent",
        "fn write_repo_discovery_file",
        "fn write_repo_discovery_file_with_agents",
        "fn submit_prompt_to_pane",
        "fn gate_pane_or_fail_on_dialog",
        "fn detach_worktree",
        "fn build_task_prompt",
        "fn combine_boot_and_task_prompt",
        "enum SpecMode",
        "fn apply_spec_mode",
        "fn invalidate_if_stale",
        "fn purge_with_prompt",
        "fn collect_unmerged_branches",
        "fn created_branch_base",
        "enum MergeState",
        "fn branch_merge_state",
        "fn discard_flag",
        "fn retire_created_branch",
        "enum PurgeOutcome",
        "struct RetainedBranch",
    ] {
        let re = Regex::new(&format!(r"\b{moved}\b")).expect("valid pattern");
        assert!(
            !re.is_match(&code),
            "main.rs still defines `{moved}`; it moved into git_paw::commands"
        );
    }
}
