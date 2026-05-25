## Context

Three independent bugs surfaced during a v0.4.0 dogfood pass:

1. **D5 (dispatcher collision)** — `src/main.rs:55-79` routes `--from-specs` to `cmd_start_from_specs` before consulting the `supervisor` flag. `--from-specs --supervisor` silently degrades to spec-mode-without-supervisor.
2. **D4 (missing injection)** — `cmd_start_from_specs` (`src/main.rs:940-1179`) has zero `tmux send-keys` calls. Bare `cmd_start` already injects a broker boot block at `src/main.rs:389-398`; from-specs lacks the equivalent. Agents launched via `--from-specs` start at the Claude welcome screen with no broker context.
3. **D2 (non-TTY attach error)** — every launch path ends with `tmux::attach(...)`. When stdin is not a terminal (CI, scripted invocation, Claude Code's Bash tool), tmux's `attach-session` fails with "open terminal failed: not a terminal" and git-paw wraps it in "failed to attach to session" — making a successfully-launched session look like a fatal error.

All three are bugfixes against shipped v0.4 behaviour. They cluster in the launch flow, so they ship together.

## Goals / Non-Goals

**Goals:**
- Make `git paw start --from-specs --supervisor` actually engage supervisor mode end-to-end (D5).
- Make `git paw start --from-specs` (without supervisor) inject the broker boot block per pane, matching bare `cmd_start`'s parity (D4).
- Replace the misleading "failed to attach" error on non-TTY invocations with an informational hint (D2).
- Keep the change small, mechanical, and reviewable. No new architectural surface.

**Non-Goals:**
- The format-native apply skill boot prompt (`/opsx:apply <change>`, Spec Kit equivalent) — that's dogfood D1, scope-isolated as a separate change.
- Tmux layout improvements for n>4 panes — dogfood D3, separate item.
- Man-page generation for `git paw --help` — dogfood D6, v1.0.0 polish.
- A `git paw version` subcommand — dogfood D7, v1.0.0 polish.
- Any change to `cmd_supervisor`'s internal logic. The fix routes the user there; the function already does the right thing once invoked.

## Decisions

### D1. Dispatcher reorder, minimal surgery

The current dispatch (`src/main.rs:55-79`):

```rust
if from_specs {
    return cmd_start_from_specs(...);
}
let supervisor_enabled = resolve_supervisor_mode_from_cwd(supervisor, dry_run)?;
if supervisor_enabled {
    return cmd_supervisor(...);
}
cmd_start(...)
```

The fix reorders the supervisor decision to happen *before* the from-specs short-circuit:

```rust
let supervisor_enabled = resolve_supervisor_mode_from_cwd(supervisor, dry_run)?;
if supervisor_enabled {
    let cwd = std::env::current_dir()...;
    let repo_root = git::validate_repo(&cwd)?;
    let config = config::load_config(&repo_root)?;
    return cmd_supervisor(
        &repo_root,
        &config,
        cli_flag.as_deref(),
        // When --from-specs is set, pass `branches_flag = None` so cmd_supervisor's
        // existing scan_specs() fallback runs.
        if from_specs { None } else { branches_flag.as_deref() },
        dry_run,
    );
}
if from_specs {
    return cmd_start_from_specs(cli_flag.as_deref(), dry_run, force);
}
cmd_start(cli_flag, branches_flag, dry_run, preset.as_deref())
```

Why this ordering:
- The `--supervisor` flag is the user's strongest signal of intent. If they passed it, they want supervisor mode regardless of how they're choosing branches.
- `cmd_supervisor` already has spec-scanning fallback at `src/main.rs:586-604`. Passing `branches_flag = None` triggers it cleanly.
- `--from-specs` without supervisor still routes to `cmd_start_from_specs` — no behaviour change for users who explicitly want spec-mode-only.
- Three branches → three behaviours, no overlap, no silent flag-drop.

Edge cases:
- `--from-specs --branches feat/x,feat/y --supervisor`: with the reorder, supervisor wins and `branches_flag = None` is passed to cmd_supervisor (forcing spec-scan). The user's `--branches` is silently ignored. **Acceptable**: combining `--from-specs` with `--branches` is already inconsistent in v0.4 (`cmd_start_from_specs` ignores `--branches` too), and this fix preserves that inconsistency rather than introducing new behaviour. A follow-up could add a "cannot combine --from-specs and --branches" parse error if dogfood shows users hit this.
- `--supervisor` without `--from-specs` and without `--branches`: cmd_supervisor's existing fallback path applies — it scans specs from config. Already works.

### D2. Boot-block injection in `cmd_start_from_specs`, mechanical mirror

After `tmux_session.execute()?;` and before `tmux::attach(...)`, insert the same injection pattern `cmd_start` uses at `src/main.rs:389-398`:

```rust
// Inject broker boot blocks for spec-driven agent panes (mirrors cmd_start
// for parity; from-specs was missing this).
if broker_config.enabled {
    let pane_offset = usize::from(broker_config.enabled);
    for (idx, (branch, _)) in mappings.iter().enumerate() {
        let pane_idx = idx + pane_offset;
        let boot_block = git_paw::skills::build_boot_block(branch, &broker_config.url());
        let args = git_paw::tmux::build_boot_inject_args(
            &tmux_session.name,
            pane_idx,
            &boot_block,
        );
        let _ = std::process::Command::new("tmux").args(&args).status();
    }
}
```

The pane offset accounts for the dashboard pane at index 0 when broker is enabled. Same offset logic `cmd_supervisor` uses.

Notably this injects **only the boot block**, not the spec content / task prompt. Reasons:
- The boot block is the v0.4-baseline injection bare `cmd_start` already does. Mirroring it gets from-specs to parity.
- The spec-content prompt is a separate concern — its shape is settled by D1 (format-native apply skill), which lands as a separate v0.5.0 change.
- Boot block injection is mechanical (exact code lifted from `cmd_start`); spec-content injection involves backend-aware logic that needs its own design.

If users want immediate "agent starts working" behaviour without waiting for D1 to ship, AGENTS.md still carries the spec content (per `worktree-agents-md` capability). The boot block tells the agent it's a git-paw worktree with a broker URL; the AGENTS.md tells them the task. Together that's enough to start. The D1 change later upgrades this to a one-shot `/opsx:apply <change>` invocation.

### D3. Non-TTY attach handling

Two call-sites for `tmux::attach` in the launch paths affected by this change: `cmd_start_from_specs` (line 1178) and `cmd_supervisor`'s implicit attach (whichever happens via the supervisor CLI launch). For each, gate the attach on `IsTerminal::is_terminal(&std::io::stdin())`:

```rust
use std::io::IsTerminal;

if std::io::stdin().is_terminal() {
    tmux::attach(&tmux_session.name)
} else {
    println!(
        "Session '{}' started in detached mode.",
        tmux_session.name
    );
    println!("Attach with:  tmux attach -t {}", tmux_session.name);
    Ok(())
}
```

For `cmd_supervisor`, the foreground-supervisor-CLI launch (`src/main.rs:870`) is itself only meaningful from a TTY (Claude in particular doesn't run interactively without one). When stdin is not a TTY, skip the supervisor CLI launch as well and print a hint:

```
Session '{name}' started in detached mode.
Supervisor agent NOT started — supervisor mode requires an interactive terminal.
Attach with:  tmux attach -t {name}
Run the supervisor manually from a real terminal:  cd <repo>; claude  (with supervisor.md as AGENTS.md)
```

This lets non-TTY invocations still set up the session for someone to attach later, instead of bombing out partway through.

### D4. Pure functions, easy to test

The dispatcher fix is in `main`'s top-level `match Command::Start`. The existing `resolve_supervisor_mode` and `cmd_supervisor` functions stay exactly as they are — no change to their bodies, no change to their tests.

The boot-block injection is mechanical lift from `cmd_start`. The `tmux::build_boot_inject_args` function is already pure and unit-testable (per the existing comment at `src/main.rs:386-388`).

The non-TTY check uses the standard library's `IsTerminal` trait — no new dependency.

Test surface for this change is therefore narrow:
- Dispatcher: parse-and-route tests.
- Boot injection: assert `tmux send-keys` is called with the expected boot-block content (use a process-call recorder, or run end-to-end and verify pane content).
- Non-TTY: integration test with stdin redirected; assert exit code 0 and the attach-hint stdout line.

## Risks / Trade-offs

- **[Risk] The dispatcher reorder might break a user who was relying on `--from-specs --supervisor` silently meaning "from-specs only".** → **Mitigation:** that user is broken either way (their session never had a supervisor agent; they thought it did). Fixing the dispatch matches the user's actual intent. Loud release-notes call-out: "v0.4.x: --from-specs --supervisor now actually engages supervisor mode."
- **[Risk] The boot-block injection in `cmd_start_from_specs` might surface latent bugs in the broker-aware skill content for spec-mode agents.** → **Mitigation:** the boot block is identical to what bare `cmd_start` already injects in broker mode. If it works there, it works here. The only difference is that from-specs agents *also* have spec content in their AGENTS.md, which is independent.
- **[Risk] Non-TTY skipping the attach + supervisor CLI launch could leave users confused if they expected an attached session.** → **Mitigation:** explicit message naming the session and the manual attach command. The hint includes a follow-up for supervisor mode (manually run the supervisor CLI from a real terminal). Better than the current "failed to attach" error which makes the session look broken.
- **[Trade-off] We're not fixing D1 (format-native apply skill) here.** Bare from-specs gets boot block parity but agents still won't know to invoke `/opsx:apply`. → Acceptable: D1 is its own design exercise (per-backend apply-skill mapping). This change closes the v0.4 baseline gap; D1 layers improvements on top.

## Migration Plan

Pure bugfix change. No migration step required.

1. Land this change. Existing scripts using `--from-specs --supervisor` start engaging supervisor mode (their original intent). Existing scripts using bare `--from-specs` start seeing the broker boot block in panes.
2. Existing CI / non-TTY users see "Session '...' started" instead of "failed to attach"; their previous workaround (parsing the error message and ignoring it) keeps working but the new path is cleaner.
3. Rollback: revert. Behaviour returns to the v0.4 broken-but-shipped state.

Release-notes call-outs:
- `--from-specs --supervisor` now actually engages supervisor mode (was silently degrading to spec-mode-without-supervisor in v0.4).
- Bare `--from-specs` now injects the broker boot block per pane (matching bare `cmd_start`'s parity); your AGENTS.md content was already there.
- Non-TTY invocations no longer error on attach; they print a "tmux attach -t <session>" hint and exit cleanly.

## Open Questions

- **Should `--from-specs` and `--branches` be made mutually exclusive at parse time?** Decision: not in this change. v0.4 already silently ignores `--branches` when `--from-specs` is set; this change preserves that. A follow-up could add `conflicts_with` if dogfood shows users hit this combination.
- **Should the supervisor-CLI launch be retried with a different mechanism on non-TTY (e.g. spawn detached, log to file)?** Decision: no. Supervisor mode requires interactive supervision by design (foreground Claude reading agent panes via tmux capture-pane). Auto-headless supervisor is a v1.0.0+ design exercise.
- **Should we add a `--no-attach` flag for users who explicitly want detached launch even with TTY available?** Decision: not in this change. The non-TTY auto-detect covers the operational use case (CI, scripts). An explicit flag is small but separable.
