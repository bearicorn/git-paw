use super::*;
use crate::command_runner::test_support::FakeCommandRunner;
use std::path::PathBuf;
use tempfile::TempDir;

fn entry(branch: &str, slot: Option<u16>) -> WorktreeEntry {
    WorktreeEntry {
        branch: branch.to_string(),
        worktree_path: PathBuf::from(format!("/tmp/wt-{branch}")),
        cli: "claude".to_string(),
        branch_created: false,
        pending_boot_prompt: None,
        runtime_slot: slot,
    }
}

fn ports(base: u16, stride: u16, vars: &[&str]) -> WorktreePortsConfig {
    WorktreePortsConfig {
        base,
        stride,
        vars: vars.iter().map(|v| (*v).to_string()).collect(),
    }
}

// --- allocate_slot: lowest-unused with free-list reuse ---

#[test]
fn empty_roster_allocates_slot_zero() {
    assert_eq!(allocate_slot(&[]), 0);
}

#[test]
fn allocation_fills_the_lowest_free_slot() {
    let roster = [entry("a", Some(0)), entry("b", Some(1))];
    assert_eq!(allocate_slot(&roster), 2);
}

#[test]
fn removing_a_middle_worktree_frees_its_slot_for_reuse() {
    // "A removed worktree's port block is freed for reuse": dropping the
    // middle entry must hand slot 1 back rather than climbing to 3.
    let mut roster = vec![
        entry("a", Some(0)),
        entry("b", Some(1)),
        entry("c", Some(2)),
    ];
    assert_eq!(allocate_slot(&roster), 3);

    roster.remove(1);
    assert_eq!(allocate_slot(&roster), 1);

    // ...and the reused slot never collides with a still-live worktree.
    let reused = allocate_slot(&roster);
    let live: Vec<u16> = roster.iter().filter_map(|e| e.runtime_slot).collect();
    assert!(
        !live.contains(&reused),
        "slot {reused} is still held by {live:?}"
    );
}

#[test]
fn entries_without_a_slot_hold_no_block() {
    let roster = [entry("a", None), entry("b", Some(0)), entry("c", None)];
    assert_eq!(allocate_slot(&roster), 1);
}

// --- port_assignments: block arithmetic ---

#[test]
fn slot_zero_receives_the_base_block() {
    let assigned = port_assignments(&ports(3000, 10, &["PORT", "VITE_PORT"]), 0).unwrap();
    assert_eq!(
        assigned,
        vec![("PORT".to_string(), 3000), ("VITE_PORT".to_string(), 3001)]
    );
}

#[test]
fn later_slots_receive_stride_offset_blocks() {
    let cfg = ports(3000, 10, &["PORT", "VITE_PORT"]);
    assert_eq!(
        port_assignments(&cfg, 1).unwrap(),
        vec![("PORT".to_string(), 3010), ("VITE_PORT".to_string(), 3011)]
    );
    assert_eq!(
        port_assignments(&cfg, 4).unwrap(),
        vec![("PORT".to_string(), 3040), ("VITE_PORT".to_string(), 3041)]
    );
}

#[test]
fn no_two_slots_share_a_port() {
    let cfg = ports(3000, 10, &["PORT", "VITE_PORT", "API_PORT"]);
    let mut seen: Vec<u16> = Vec::new();
    for slot in 0..25 {
        for (_, port) in port_assignments(&cfg, slot).unwrap() {
            assert!(!seen.contains(&port), "port {port} assigned twice");
            seen.push(port);
        }
    }
}

#[test]
fn undersized_stride_still_yields_disjoint_blocks() {
    // stride 1 with 3 vars would overlap; effective_stride clamps to 3.
    let cfg = ports(3000, 1, &["A", "B", "C"]);
    let first = port_assignments(&cfg, 0).unwrap();
    let second = port_assignments(&cfg, 1).unwrap();
    let first_ports: Vec<u16> = first.iter().map(|(_, p)| *p).collect();
    let second_ports: Vec<u16> = second.iter().map(|(_, p)| *p).collect();
    assert_eq!(first_ports, vec![3000, 3001, 3002]);
    assert_eq!(second_ports, vec![3003, 3004, 3005]);
}

#[test]
fn empty_vars_assigns_no_ports() {
    assert!(
        port_assignments(&ports(3000, 10, &[]), 3)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn block_past_the_port_space_is_rejected() {
    let err = port_assignments(&ports(65_000, 1000, &["PORT"]), 5).unwrap_err();
    assert!(
        err.to_string().contains("65535"),
        "error should name the port ceiling; got {err}"
    );
}

// --- write_env_local: managed block handling ---

#[test]
fn managed_block_is_written_with_one_line_per_var() {
    let tmp = TempDir::new().unwrap();
    write_env_local(
        tmp.path(),
        &[
            ("PORT".to_string(), "3000".to_string()),
            ("VITE_PORT".to_string(), "3001".to_string()),
        ],
    )
    .unwrap();

    let contents = std::fs::read_to_string(tmp.path().join(ENV_LOCAL_FILE)).unwrap();
    assert_eq!(
        contents,
        format!("{MANAGED_BLOCK_START}\nPORT=3000\nVITE_PORT=3001\n{MANAGED_BLOCK_END}\n")
    );
}

#[test]
fn regenerating_the_managed_block_is_idempotent() {
    let tmp = TempDir::new().unwrap();
    let assignments = [("PORT".to_string(), "3000".to_string())];
    write_env_local(tmp.path(), &assignments).unwrap();
    let first = std::fs::read_to_string(tmp.path().join(ENV_LOCAL_FILE)).unwrap();

    write_env_local(tmp.path(), &assignments).unwrap();
    let second = std::fs::read_to_string(tmp.path().join(ENV_LOCAL_FILE)).unwrap();

    assert_eq!(first, second);
    assert_eq!(second.matches(MANAGED_BLOCK_START).count(), 1);
}

#[test]
fn user_content_outside_the_managed_block_is_preserved() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join(ENV_LOCAL_FILE);
    std::fs::write(&path, "USER_SETTING=keep-me\n").unwrap();

    write_env_local(tmp.path(), &[("PORT".to_string(), "3000".to_string())]).unwrap();
    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(contents.contains("USER_SETTING=keep-me"));
    assert!(contents.contains("PORT=3000"));

    // Re-provisioning with a different slot replaces only the managed lines.
    write_env_local(tmp.path(), &[("PORT".to_string(), "3010".to_string())]).unwrap();
    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(contents.contains("USER_SETTING=keep-me"));
    assert!(contents.contains("PORT=3010"));
    assert!(!contents.contains("PORT=3000"));
}

#[test]
fn no_assignments_and_no_existing_file_creates_nothing() {
    let tmp = TempDir::new().unwrap();
    write_env_local(tmp.path(), &[]).unwrap();
    assert!(!tmp.path().join(ENV_LOCAL_FILE).exists());
}

#[test]
fn no_assignments_strips_a_stale_managed_block() {
    let tmp = TempDir::new().unwrap();
    write_env_local(tmp.path(), &[("PORT".to_string(), "3000".to_string())]).unwrap();
    write_env_local(tmp.path(), &[]).unwrap();

    let contents = std::fs::read_to_string(tmp.path().join(ENV_LOCAL_FILE)).unwrap();
    assert!(
        !contents.contains("PORT=3000"),
        "stale ports must not survive; got {contents:?}"
    );
}

// --- copy_env_files ---

#[test]
fn declared_files_are_copied_into_the_worktree() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().join("repo");
    let worktree = tmp.path().join("wt");
    std::fs::create_dir_all(repo.join("config")).unwrap();
    std::fs::create_dir_all(&worktree).unwrap();
    std::fs::write(repo.join(".env"), "SECRET=abc\n").unwrap();
    std::fs::write(repo.join("config/.env.dev"), "DEV=1\n").unwrap();

    let warnings = copy_env_files(
        &repo,
        &worktree,
        &WorktreeEnvConfig {
            copy: vec![".env".to_string(), "config/.env.dev".to_string()],
        },
    )
    .unwrap();

    assert!(warnings.is_empty(), "got {warnings:?}");
    assert_eq!(
        std::fs::read_to_string(worktree.join(".env")).unwrap(),
        "SECRET=abc\n"
    );
    assert_eq!(
        std::fs::read_to_string(worktree.join("config/.env.dev")).unwrap(),
        "DEV=1\n"
    );
}

#[test]
fn missing_declared_file_warns_and_continues() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().join("repo");
    let worktree = tmp.path().join("wt");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir_all(&worktree).unwrap();
    std::fs::write(repo.join(".env"), "SECRET=abc\n").unwrap();

    let warnings = copy_env_files(
        &repo,
        &worktree,
        &WorktreeEnvConfig {
            copy: vec![".env.absent".to_string(), ".env".to_string()],
        },
    )
    .unwrap();

    assert_eq!(warnings.len(), 1, "got {warnings:?}");
    assert!(warnings[0].contains(".env.absent"));
    assert!(
        worktree.join(".env").exists(),
        "the present file is still copied"
    );
}

#[test]
fn entries_escaping_the_repository_are_rejected() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().join("repo");
    let worktree = tmp.path().join("wt");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir_all(&worktree).unwrap();
    std::fs::write(tmp.path().join("outside.env"), "LEAK=1\n").unwrap();

    let warnings = copy_env_files(
        &repo,
        &worktree,
        &WorktreeEnvConfig {
            copy: vec!["../outside.env".to_string(), "/etc/hosts".to_string()],
        },
    )
    .unwrap();

    assert_eq!(warnings.len(), 2, "got {warnings:?}");
    assert!(!worktree.join("outside.env").exists());
    assert!(!worktree.join("hosts").exists());
}

// --- provision_worktree ---

#[test]
fn absent_configuration_provisions_nothing() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().join("repo");
    let worktree = tmp.path().join("wt");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir_all(&worktree).unwrap();
    std::fs::write(repo.join(".env"), "SECRET=abc\n").unwrap();

    let result =
        provision_worktree(&repo, &worktree, "feat-x", &WorktreeConfig::default(), 0).unwrap();

    assert_eq!(result, Provisioned::default());
    assert!(!worktree.join(".env").exists());
    assert!(!worktree.join(ENV_LOCAL_FILE).exists());
}

#[test]
fn port_allocation_never_mutates_the_copied_env_file() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().join("repo");
    let worktree = tmp.path().join("wt");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir_all(&worktree).unwrap();
    std::fs::write(repo.join(".env"), "PORT=3000\nSECRET=abc\n").unwrap();

    let config = WorktreeConfig {
        env: Some(WorktreeEnvConfig {
            copy: vec![".env".to_string()],
        }),
        ports: Some(ports(3000, 10, &["PORT"])),
        hooks: None,
    };
    let result = provision_worktree(&repo, &worktree, "feat-x", &config, 2).unwrap();

    assert_eq!(result.runtime_slot, Some(2));
    assert_eq!(
        std::fs::read_to_string(worktree.join(".env")).unwrap(),
        std::fs::read_to_string(repo.join(".env")).unwrap(),
        "the copied .env stays byte-for-byte identical to the source"
    );
    let local = std::fs::read_to_string(worktree.join(ENV_LOCAL_FILE)).unwrap();
    assert!(local.contains("PORT=3020"), "got {local:?}");
}

#[test]
fn env_only_configuration_records_no_slot() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().join("repo");
    let worktree = tmp.path().join("wt");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir_all(&worktree).unwrap();
    std::fs::write(repo.join(".env"), "SECRET=abc\n").unwrap();

    let config = WorktreeConfig {
        env: Some(WorktreeEnvConfig {
            copy: vec![".env".to_string()],
        }),
        ports: None,
        hooks: None,
    };
    let result = provision_worktree(&repo, &worktree, "feat-x", &config, 7).unwrap();

    assert_eq!(result.runtime_slot, None);
    assert!(worktree.join(".env").exists());
    assert!(!worktree.join(ENV_LOCAL_FILE).exists());
}

// --- hook placeholder substitution ---

#[test]
fn both_placeholders_are_substituted() {
    let substituted = substitute_placeholders(
        "db-branch.sh {worktree_id} --checkout {worktree_path}",
        "feat-auth-flow",
        Path::new("/repo/.git-paw/worktrees/feat-auth-flow"),
    );

    assert_eq!(
        substituted,
        "db-branch.sh feat-auth-flow --checkout /repo/.git-paw/worktrees/feat-auth-flow"
    );
}

#[test]
fn a_command_with_no_placeholder_is_unchanged() {
    let raw = "docker compose up -d";

    assert_eq!(
        substitute_placeholders(raw, "feat-auth-flow", Path::new("/repo/wt")),
        raw
    );
}

#[test]
fn every_occurrence_of_a_placeholder_is_substituted() {
    // A teardown script that names the resource twice (once to look it up, once
    // to log it) must not be left with a literal `{worktree_id}` in the second
    // position.
    let substituted = substitute_placeholders(
        "drop.sh {worktree_id} && echo dropped {worktree_id}",
        "fix-42",
        Path::new("/repo/wt"),
    );

    assert_eq!(substituted, "drop.sh fix-42 && echo dropped fix-42");
}

// --- on_create hook: argv, stdout parsing, failure policy ---

/// A worktree config carrying only an `on_create` hook.
fn on_create(command: &str) -> WorktreeConfig {
    WorktreeConfig {
        env: None,
        ports: None,
        hooks: Some(crate::config::WorktreeHooksConfig {
            on_create: Some(command.to_string()),
            on_remove: None,
        }),
    }
}

/// A repo + worktree pair under a fresh temp dir.
fn sandbox() -> (TempDir, PathBuf, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().join("repo");
    let worktree = tmp.path().join("wt");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir_all(&worktree).unwrap();
    (tmp, repo, worktree)
}

#[test]
fn the_hook_runs_in_the_worktree_with_placeholders_expanded() {
    let (_tmp, repo, worktree) = sandbox();
    let runner = FakeCommandRunner::succeeding("");

    provision_worktree_with(
        &runner,
        &repo,
        &worktree,
        "feat/auth-flow",
        &on_create("db-branch.sh {worktree_id}"),
        0,
    )
    .unwrap();

    let calls = runner.calls();
    assert_eq!(calls.len(), 1, "exactly one hook invocation; got {calls:?}");
    let (program, argv) = &calls[0];
    assert_eq!(program, "/bin/sh");
    assert_eq!(argv[0], "-c");
    assert!(
        argv[1].contains("db-branch.sh feat-auth-flow"),
        "the id placeholder must be expanded; got {:?}",
        argv[1]
    );
    assert!(
        argv[1].starts_with(&format!("cd {}", shell_quote(&worktree.to_string_lossy()))),
        "the hook must start in the worktree; got {:?}",
        argv[1]
    );
}

#[test]
fn hook_output_is_merged_into_the_managed_block() {
    let (_tmp, repo, worktree) = sandbox();
    let runner = FakeCommandRunner::succeeding("DATABASE_URL=postgres://localhost/paw_wt1\n");

    let result =
        provision_worktree_with(&runner, &repo, &worktree, "wt1", &on_create("provision"), 0)
            .unwrap();

    let contents = std::fs::read_to_string(worktree.join(ENV_LOCAL_FILE)).unwrap();
    assert_eq!(
        contents,
        format!(
            "{MANAGED_BLOCK_START}\nDATABASE_URL=postgres://localhost/paw_wt1\n\
             {MANAGED_BLOCK_END}\n"
        )
    );
    assert_eq!(result.hook_env_keys, vec!["DATABASE_URL".to_string()]);
}

#[test]
fn ports_and_hook_keys_share_one_managed_block() {
    let (_tmp, repo, worktree) = sandbox();
    std::fs::write(
        worktree.join(ENV_LOCAL_FILE),
        "USER_SETTING=keep-me\nSECOND=also-keep\n",
    )
    .unwrap();
    let runner = FakeCommandRunner::succeeding("DATABASE_URL=postgres://localhost/wt\n");
    let mut config = on_create("provision");
    config.ports = Some(ports(3000, 10, &["PORT"]));

    provision_worktree_with(&runner, &repo, &worktree, "wt", &config, 1).unwrap();

    let contents = std::fs::read_to_string(worktree.join(ENV_LOCAL_FILE)).unwrap();
    assert_eq!(
        contents,
        format!(
            "USER_SETTING=keep-me\nSECOND=also-keep\n{MANAGED_BLOCK_START}\n\
             PORT=3010\nDATABASE_URL=postgres://localhost/wt\n{MANAGED_BLOCK_END}\n"
        ),
        "ports and hook keys share one block and user content is untouched"
    );
}

#[test]
fn non_assignment_output_is_ignored() {
    let (_tmp, repo, worktree) = sandbox();
    let runner = FakeCommandRunner::succeeding(
        "Creating branch...\n\nDATABASE_URL=postgres://localhost/wt\n\
         done in 1.2s\n9INVALID=x\nno-equals-here\n",
    );

    let result =
        provision_worktree_with(&runner, &repo, &worktree, "wt", &on_create("provision"), 0)
            .unwrap();

    assert_eq!(result.hook_env_keys, vec!["DATABASE_URL".to_string()]);
    let contents = std::fs::read_to_string(worktree.join(ENV_LOCAL_FILE)).unwrap();
    for noise in ["Creating branch", "done in", "9INVALID", "no-equals-here"] {
        assert!(
            !contents.contains(noise),
            "{noise:?} leaked into {contents:?}"
        );
    }
}

#[test]
fn a_value_containing_equals_signs_is_kept_verbatim() {
    let (_tmp, repo, worktree) = sandbox();
    let runner = FakeCommandRunner::succeeding("DSN=postgres://h/db?sslmode=require&x=1\n");

    provision_worktree_with(&runner, &repo, &worktree, "wt", &on_create("provision"), 0).unwrap();

    let contents = std::fs::read_to_string(worktree.join(ENV_LOCAL_FILE)).unwrap();
    assert!(
        contents.contains("DSN=postgres://h/db?sslmode=require&x=1"),
        "got {contents:?}"
    );
}

#[test]
fn re_provisioning_with_a_hook_is_idempotent() {
    let (_tmp, repo, worktree) = sandbox();
    let runner = FakeCommandRunner::succeeding("DATABASE_URL=postgres://localhost/wt\n");
    let mut config = on_create("provision");
    config.ports = Some(ports(3000, 10, &["PORT"]));

    provision_worktree_with(&runner, &repo, &worktree, "wt", &config, 0).unwrap();
    let first = std::fs::read_to_string(worktree.join(ENV_LOCAL_FILE)).unwrap();
    provision_worktree_with(&runner, &repo, &worktree, "wt", &config, 0).unwrap();
    let second = std::fs::read_to_string(worktree.join(ENV_LOCAL_FILE)).unwrap();

    assert_eq!(first, second);
    assert_eq!(second.matches(MANAGED_BLOCK_START).count(), 1);
}

#[test]
fn a_secret_value_never_reaches_the_reported_outcome() {
    // The hook's whole purpose is to surface a credential, so `Provisioned`
    // must carry the key and nothing else — anything a caller can print or
    // format is a potential log line.
    let (_tmp, repo, worktree) = sandbox();
    let secret = "postgres://admin:hunter2@localhost/paw_wt1";
    let runner = FakeCommandRunner::succeeding(&format!("DATABASE_URL={secret}\n"));

    let result =
        provision_worktree_with(&runner, &repo, &worktree, "wt", &on_create("provision"), 0)
            .unwrap();

    let rendered = format!("{result:?} {}", result.hook_summary().unwrap());
    assert!(
        !rendered.contains("hunter2") && !rendered.contains(secret),
        "the value leaked into {rendered:?}"
    );
    assert!(rendered.contains("DATABASE_URL"), "the key should be named");
    // ...and the value did land where it belongs.
    let contents = std::fs::read_to_string(worktree.join(ENV_LOCAL_FILE)).unwrap();
    assert!(contents.contains(secret), "got {contents:?}");
}

#[test]
fn a_failing_on_create_reports_its_status_and_stderr() {
    let (_tmp, repo, worktree) = sandbox();
    let runner = FakeCommandRunner::failing("could not reach the database host");

    let err = provision_worktree_with(&runner, &repo, &worktree, "wt", &on_create("provision"), 0)
        .unwrap_err();

    let message = err.to_string();
    assert!(message.contains("on_create"), "got {message}");
    assert!(message.contains("exit 1"), "got {message}");
    assert!(
        message.contains("could not reach the database host"),
        "got {message}"
    );
}

#[test]
fn a_failing_on_create_never_echoes_its_stdout() {
    // Failure reporting surfaces stderr; stdout is the secret-bearing channel
    // and must stay out of the error a caller will print.
    let (_tmp, repo, worktree) = sandbox();
    let runner = FakeCommandRunner::scripted(|_, _| {
        Ok(crate::command_runner::CommandOutput {
            success: false,
            code: Some(3),
            stdout: b"DATABASE_URL=postgres://admin:hunter2@localhost/db\n".to_vec(),
            stderr: b"provisioning aborted".to_vec(),
        })
    });

    let err = provision_worktree_with(&runner, &repo, &worktree, "wt", &on_create("provision"), 0)
        .unwrap_err();

    let message = err.to_string();
    assert!(
        !message.contains("hunter2"),
        "the value leaked into {message}"
    );
    assert!(message.contains("exit 3"), "got {message}");
}

#[test]
fn an_absent_hook_runs_no_command() {
    let (_tmp, repo, worktree) = sandbox();
    let runner = FakeCommandRunner::succeeding("DATABASE_URL=should-not-happen\n");

    let result = provision_worktree_with(
        &runner,
        &repo,
        &worktree,
        "wt",
        &WorktreeConfig::default(),
        0,
    )
    .unwrap();

    assert!(runner.calls().is_empty(), "no hook was configured");
    assert_eq!(result, Provisioned::default());
    assert!(!worktree.join(ENV_LOCAL_FILE).exists());
}

#[test]
fn the_worktree_id_is_the_branch_derived_slug() {
    // The id a hook receives is the same slug that names the worktree's
    // directory under `.git-paw/worktrees/`, so a script can correlate the two.
    assert_eq!(worktree_id("feat/auth-flow"), "feat-auth-flow");
}

// --- on_remove hook: argv and non-blocking failure policy ---

#[test]
fn the_remove_hook_runs_in_the_worktree_with_placeholders_expanded() {
    let (_tmp, _repo, worktree) = sandbox();
    let runner = FakeCommandRunner::succeeding("");

    let warning = run_remove_hook_with(
        &runner,
        "drop-db.sh {worktree_id} --at {worktree_path}",
        "feat/auth-flow",
        &worktree,
    );

    assert!(warning.is_none(), "a successful hook warns about nothing");
    let calls = runner.calls();
    assert_eq!(calls.len(), 1, "exactly one hook invocation; got {calls:?}");
    let (program, argv) = &calls[0];
    assert_eq!(program, "/bin/sh");
    assert_eq!(argv[0], "-c");
    assert_eq!(
        argv[1],
        format!(
            "cd {q} && drop-db.sh feat-auth-flow --at {path}",
            q = shell_quote(&worktree.to_string_lossy()),
            path = worktree.display()
        )
    );
}

#[test]
fn a_failing_on_remove_returns_a_warning_naming_its_status_and_stderr() {
    // The warning is *returned*, not raised: the caller keeps removing the
    // worktree, so a resource git-paw could not drop never strands a checkout.
    let (_tmp, _repo, worktree) = sandbox();
    let runner = FakeCommandRunner::failing("database paw_wt1 does not exist");

    let warning = run_remove_hook_with(&runner, "drop-db.sh", "wt1", &worktree)
        .expect("a non-zero hook must produce a warning");

    assert!(warning.contains("on_remove"), "got {warning}");
    assert!(warning.contains("exit 1"), "got {warning}");
    assert!(
        warning.contains("database paw_wt1 does not exist"),
        "got {warning}"
    );
}

#[test]
fn an_unspawnable_on_remove_returns_a_warning_rather_than_failing() {
    let (_tmp, _repo, worktree) = sandbox();
    let runner = FakeCommandRunner::scripted(|_, _| {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no such shell",
        ))
    });

    let warning = run_remove_hook_with(&runner, "drop-db.sh", "wt1", &worktree)
        .expect("a hook that cannot spawn must produce a warning");

    assert!(warning.contains("no such shell"), "got {warning}");
}
