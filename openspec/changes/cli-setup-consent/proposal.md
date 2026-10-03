## Why

v0.17.0 retires git-paw's command classifier (`classifier-retirement`). After that nothing in git-paw
approves an agent's command, so every call an agent makes to git-paw's own helper scripts
(`broker.sh` heartbeats, `sweep.sh` snapshots, `docs-fetch.sh find`) stops at a permission prompt
unless the agent CLI's own allow rules permit it. The user decided (2026-10-19) that git-paw's own
workflow may be pre-approved in the CLI's settings, but only through an explicit command the user
runs: running it is the consent. Launches still never write settings.

This change is the v0.19.0 `cli-setup-consent` design pulled forward into v0.17.0 and cut down to what
v0.17.0 can support: one CLI (`claude`), one item (the allowlist), no provider families, no helper
registry. It carries two lessons. GP-01: v0.4–v0.15 wrote an allowlist key the CLI never read, so the
grant ships only with facts checked against the installed CLI, and otherwise stays plan-only. And
helper tampering: a granted helper lives in the agent's own worktree, where a CLI in its
edit-accepting mode lets the agent rewrite it without a prompt and then run it through the grant. The
grant therefore ships together with protection rules, and only once the gate has shown that the
protection holds.

## What Changes

- **`git paw setup-cli <cli> [--allowlist] [--trust] [--hooks] [--dry-run] [--yes] [--undo]`.** It
  prints the exact plan before any write: each file by absolute path (created or modified) and each
  entry with its key path. It writes only those entries and keeps every unrelated key, value and key
  order. `--dry-run` prints the plan and writes nothing. The command refuses to run from a linked
  worktree.
- **Consent gate.** On a terminal the user confirms the plan (default No). Off a terminal it needs
  `--yes`. With neither, or when the user declines, nothing is written and the command exits `2`
  through a new `PawError::NotAttempted` (not `Cancelled.`).
- **A compiled fact table, one row.** `claude` declares the allowlist item. `--trust` and `--hooks`
  exist so the flag surface is stable, but no CLI declares them; any CLI or item without a row answers
  "not supported for `<cli>`", writes nothing and exits `2`. Each item carries a verification status;
  an unverified item can be planned with `--dry-run` but is never written.
- **One target file.** The project-local `.claude/settings.local.json` at the main checkout's root,
  which Claude Code's documentation says a session reads even when started inside a linked worktree.
  No file is written inside any worktree, at user level, or at `[clis.<name>].settings_path`.
- **Least-privilege entries, from a fixed grant table.** `Bash(.git-paw/scripts/broker.sh *)`;
  `docs-fetch.sh find` only (`get` takes arbitrary URLs, including `file://`); nine `sweep.sh`
  subcommands that send no keystrokes and publish no verdict. `sweep.sh approve` is put on an `ask`
  rule so it always prompts; `verified`, `feedback-gate` and `docs-fetch.sh get` are not granted. No
  argument can add an entry.
- **Tamper protection in the same write.** `Edit(**/.git-paw/scripts/**)` in `permissions.deny` and
  `Edit(**/.git-paw/config.toml)` in `permissions.ask`, so no session in the repository can silently
  change what a grant runs or where the helpers send data.
- **Fact gate (task 1, blocking).** Before any writer is built, the implementer runs a scripted check
  against the installed Claude Code: which settings file a nested-worktree session reads, the rule
  forms, compound and substitution matching, environment-prefix matching, ask/deny precedence, and the
  tamper protection against the file-editing tools and the filesystem commands its edit-accepting mode
  auto-approves. The observations go into design.md. If any blocking fact fails, the claude row ships
  unverified: `--dry-run` plus docs only.
- **Every write is recorded** in `<git-common-dir>/git-paw/cli-setup.json` before it is made.
  `--undo` removes exactly the recorded entries that are still present, unchanged and of a git-paw
  shape, never removes the protection while a helper grant remains, and deletes only files and
  directories git-paw created that are now empty of anything else.
- **Never automatic.** `start`, `add`, `init`, recovery, the drive loop, the dashboard and the MCP
  server never write settings or the record. No MCP tool exposes the command.
- **Doctor (read-only).** A per-CLI setup check (applied or not, verified or plan-only, entries
  missing/changed/out of date, a granted helper modified) and an advisory that lists broad durable
  grants already in the project's settings file — the kind 0.16.0's classifier could leave behind
  through "Yes, and don't ask again" (`Bash(git-paw purge:*)`, `Bash(git push:*)`, bare-verb
  wildcards). Report only: doctor never edits the file.
- **Bundled `cli-setup` skill** (export-agnostic) with the consent rules, served by MCP `get_skill`,
  and a pointer section in the supervisor skill.

## Capabilities

### New Capabilities

- `cli-setup`: the consented per-CLI setup command: items and the fact table, verification status
  and plan-only mode, the printed plan and dry run, the consent gate, the single target file, the
  grant table and its protection rules, write discipline, unsafe-target refusal, the setup record, and
  undo.

### Modified Capabilities

- `approval-command-safety`: *No permission grants are seeded into agent CLI settings files* now reads
  "never automatically; only through the explicit consented `git paw setup-cli`" (MODIFIED). The text
  no longer mentions the classifier and depends on no other requirement of this capability.
- `core-error-handling`: *Exit codes distinguish cancellation from errors* (MODIFIED): a
  `NotAttempted` variant also exits `2`, with its own message.
- `preflight-diagnostics`: ADDED *Doctor reports CLI setup state* and *Doctor lists broad durable
  grants in the project's CLI settings*.
- `skill-agent`: ADDED *Embedded cli-setup skill* (including the supervisor-skill pointer section).

## Impact

- **Code:** new `src/cli_setup/` — `facts.rs` (the one-row fact table and the grant table, inlined
  for v0.17.0), `plan.rs` and `undo.rs` (pure planners), `render.rs` (plan text), `record.rs`,
  `apply.rs` (filesystem plus git queries through `CommandRunner`). `src/commands/setup_cli.rs` (use
  case, never prompts), `src/cli.rs` (`SetupCli`), `src/main.rs` (dispatch, TTY check, `Confirm`),
  `src/error.rs` (`NotAttempted`), `src/doctor.rs` (two probes and two pure checks in the CLIs group),
  `src/skills.rs` (`cli-setup` registered), `src/init.rs` (no behaviour change; its
  `bundled_scripts()` is the helper source), `assets/agent-skills/cli-setup.md` (new),
  `assets/agent-skills/supervisor.md` (pointer section).
- **Dependency feature:** `Cargo.toml` turns on serde_json's `preserve_order` feature so unrelated
  keys in the user's settings file keep their order. No new crate: `indexmap` 2.14.0 is already in
  `Cargo.lock`. The v0.19.0 source records user approval (2026-10-19); confirm it for v0.17.0
  (design D7 gives the alternative).
- **Config / persisted state:** no config field. One new git-paw-owned file inside the git
  directory. Session state is unchanged.
- **Enum-variant ripple:** none for `BrokerMessage` or `SpecBackendKind`. `PawError` gains
  `NotAttempted`; `exit_code()` has a wildcard arm, and the `core-error-handling` tests are updated.
- **Sequencing:** lands after `classifier-retirement`. Until that change is in, the classifier's
  read-mostly `git` verb would classify `git paw setup-cli … --yes` as safe, and the drive loop could
  approve it in the supervisor pane (which is not a linked worktree). Lands after
  `bundled-helper-discovery`, which provisions `.git-paw/scripts/broker.sh` on every launch path and
  so makes the relative grant path true for every worker.
- **Cross-change coordination:** the MODIFIED *No permission grants…* requirement is touched by no
  other pending change (`classifier-retirement` leaves it to this change). *Exit codes distinguish
  cancellation from errors* is touched by no other pending v0.17.0 change. The ADDED requirements are
  new names. Design "Cross-change review" lists the semantic interplay with `classifier-retirement`
  and `bundled-helper-discovery`.
- **Later versions:** the v0.19.0 copy of this change is dropped; nothing moves into
  `cli-provider-record` and the fact table stays keyed by CLI identity, with no `family` key. v0.18.0
  `orchestrator-skill-export` adds `cli-setup` to its exported skill set. v0.20.0 `skill-package-deploy`
  moves the helpers (and therefore the grant entries) to `.git-paw/skills/<pkg>/scripts/` and adds a
  deny rule on `Edit(**/.git-paw/skills/**)`; the claude row stays verified only if the fact gate is
  re-run against the moved paths, and doctor reports pre-move grants as out of date until the user
  re-runs `--undo` then `--allowlist`. v0.23.0's per-CLI audit adds trust and hooks facts. v0.24.0
  `error-exit-code-contract` can absorb `NotAttempted` into its "nothing was attempted" code.
