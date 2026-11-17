## Context

**The level set.** `ApprovalLevel` has three variants, `Manual`, `Auto` (the `#[default]`) and
`FullAuto`, serialized in kebab-case (`src/config/supervisor.rs:22-32`). `KEBAB_NAMES` is the
accepted key set of `[clis.<name>].approval_args` (`src/config/supervisor.rs:37`), and
`validate_approval_args` enforces it with a message that lists the three names
(`src/config/mod.rs:827-833`). Profiles parse their `approval` string by hand against the same
three names (`src/config/profiles.rs:203-215`). The `init` template documents the set at
`src/config/mod.rs:1280`.

**Levels to flags.** Each provider family maps a level to flags:

- Claude (`src/agent_cli/mod.rs:410-416`):
  - `FullAuto` → `--dangerously-skip-permissions`
  - `Auto` → `--permission-mode acceptEdits`
  - `Manual` → `""`
- Codex (`:484-489`): `FullAuto` → `--dangerously-bypass-approvals-and-sandbox`, `Auto` →
  `--sandbox workspace-write`.
- `agy` (`:537-541`): only `FullAuto` → `--dangerously-skip-permissions`.
- `qwen` (`:555-559`): only `FullAuto` → `--yolo`.

`ResolvedCli::approval_flags` returns the `[clis.<id>].approval_args` override when there is one,
and the family row otherwise (`src/agent_cli/profile.rs:145-154`). The frozen pre-provider table
in the tests pins `("claude", Auto)` to `--permission-mode acceptEdits`
(`src/agent_cli/tests.rs:61-70`, line 65). So do `src/config/tests.rs:1985-2012`, the launch
golden at `src/commands/launch.rs:1786`, and the headless hand-off tests at
`src/supervisor/escalation.rs:1296,1349,1696`.

**Why `auto` meant `acceptEdits`.** v0.16.0 `unattended-boot-hardening` (commit `19a8798`,
2026-09-04) mapped Claude `Auto` to `acceptEdits` so that an `auto` pane would not inherit
whatever default the CLI's configuration carried. It checked the live enumeration, which was then
`default`, `acceptEdits`, `plan` and `bypassPermissions`. The `supervisor-config` spec repeats that
enumeration (**Permission flag mapping**). Claude Code had no auto mode at the time.

**What the installed CLI offers.** Probed on 2026-11-16:
- `claude --version` reports `2.1.325 (Claude Code)`.
- `claude --help` lists `--permission-mode` choices `"acceptEdits", "auto", "bypassPermissions",
  "manual", "dontAsk", "plan"`.
- `claude auto-mode {config,defaults,critique,reset}` inspects auto mode's rules.
  `claude auto-mode defaults` prints `allow`, `soft_deny` and `hard_deny` rule sets. These include
  "Local Operations: … local file operations within project scope" and "Read-Only Operations",
  which are exactly the #23 and #29 cases.

So `--permission-mode auto` is a real, deterministic choice. Whether it is *available* to a given
account or model, and how it behaves in an unattended worker, is not something `--help` shows.
That is assumption A1 below.

**How workers read settings.** A Claude-family worker launches with `--setting-sources
project,local` and `CLAUDE_CODE_PLUGIN_DIRS=` (operator config), plus
`CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` and `CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false` (worker
safety) (`src/agent_cli/mod.rs:442-467`). It keeps the **project** and **local** layers. Worktrees
sit under `.git-paw/worktrees/`, and a session in a nested worktree reads the **main checkout's**
`.claude/settings.local.json` (`src/cli_setup/facts.rs:287-289`, fact F1a). That file is
`setup-cli`'s allowlist target (`src/cli_setup/facts.rs:312-322`). This is why hand-written rules
"also apply to your own sessions" (#23): they live in the one file the operator's own Claude
sessions in that repository also read. v0.20.0 dropped the *user* layer for workers. It did
**not** give workers a config folder of their own. No `CLAUDE_CONFIG_DIR` is set per worker, and
`src/agent_cli/usage.rs:395-397` resolves transcripts from the entry's settings path, then
`CLAUDE_CONFIG_DIR`, then `~/.claude`.

**Where the flags show today.** `start --dry-run` prints each worker's full pane command as
`runs: <command>` (`src/commands/launch.rs:373-389`). The flags are in that command, but nothing
names the level or says where the flags came from. Doctor reports sandbox mode per worker CLI and
flags `full-auto` without a sandbox (`src/doctor.rs:693-760`, the `full_auto` probe at
`:1931-1935`). No check says what each level launches.

**Delegates.** `widening_arg` denies the flags of the `Auto` and `FullAuto` rows
(`src/agent_cli/profile.rs:209-214`). Admission already treats every non-`Manual` run as the
single writer (`src/delegate.rs:309-315`), but the spec text names only `auto`.

**v0.21.0 context in the same branch.**
- `config-schema-versioning` adds the in-memory migration chain.
- `supervisor-vocabulary-split` adds:
  - version 2, with its v1→v2 step. `[supervisor] agent_approval` moves to `[workers] approval`
    and `[supervisor] approval` moves to `[orchestrator] approval`, "with unchanged meaning, type
    and default".
  - the `core-deprecation` registry and the one-line stderr helper.
  - `init`'s rule to backfill `version = 2` only into a file without `[supervisor]`.

## Goals / Non-Goals

**Goals:**
- An approval level's name says what it launches. `auto` means the CLI's own automatic mode.
- No existing configuration launches a different command after the upgrade.
- `start --dry-run` and `doctor` show the exact flags per level, so nobody has to read source or
  docs to find out (#28).
- A wave in a new version-2 configuration runs routine dev commands without git-paw approving
  anything or writing rules (#23, #29), because `auto` is the default.
- The level names are final before v1.0.0 freezes them.

**Non-Goals:**
- Any git-paw command classifier, allowlist, or prompt answering. `approval-command-safety` stays
  as it is.
- Writing permission rules outside `git paw setup-cli`.
- Worker-only allow rules (alternative B). These are deferred.
- Claude first-launch dialogs (#25, #30), `setup-cli` family matching (#22), and re-verifying
  every family's rows (PK-11). All belong to the v0.23.0 per-CLI audit.
- Changing what `manual` launches. See resolved question 3.

## Decisions

### D1 — Recommended: four levels; `auto` is the CLI's own automatic mode; `accept-edits` keeps today's behaviour (alternatives A + C)

| Level | Meaning | claude | codex | agy | qwen |
|---|---|---|---|---|---|
| `manual` | the CLI's default permission mode | `""` | `""` | `""` | `""` |
| `accept-edits` | file edits without a prompt; anything else the CLI's rules do not allow prompts | `--permission-mode acceptEdits` | `--sandbox workspace-write` | `""` | `""` |
| `auto` | the CLI's own automatic mode: the CLI decides each action | `--permission-mode auto` | `--sandbox workspace-write` | `""` | `""` |
| `full-auto` | no prompts; needs a sandbox | `--dangerously-skip-permissions` | `--dangerously-bypass-approvals-and-sandbox` | `--dangerously-skip-permissions` | `--yolo` |

- Codex has no mode between "edits only" and its own automatic preset that git-paw has verified.
  Both `accept-edits` and `auto` therefore keep today's `--sandbox workspace-write`. This must
  hold, because the migration (D2) turns a version-1 `auto` into `accept-edits`, and a Codex
  worker must not change.
- `agy` and `qwen` keep `""` at both middle levels.
- The table is explicit, with no "fall back to the `auto` row" rule. A future family whose `auto`
  row is an AI-judged mode must not widen `accept-edits` by accident.
- The doc comment at `src/agent_cli/mod.rs:401-403` records "Claude `auto` row verified against
  the live `--permission-mode` enumeration on Claude Code 2.1.325, 2026-11-16". Before the row
  lands, it must also pass the behaviour gate in D6.
- `full-auto` is unchanged.
- **Order.** `manual < accept-edits < auto < full-auto`. Every "above `manual`" rule (delegate
  writer admission and the widening denylist) covers the three upper levels.

**Why rename instead of adding a level.** The name `auto` is what #28 reports as untrue, and
v1.0.0 freezes level names. Keeping `auto` = `acceptEdits` and adding, say, `cli-auto` would
freeze a misleading name for the life of the 1.x line. "Accept edits" is also the CLI-neutral
term Claude Code itself uses.

*Alternative A2 (additive, rejected):* keep `auto` and add `native-auto`. There is no migration
and no warning, but the contract keeps a lie. Rejected for that reason.

*Alternative A3 (rejected):* map `auto` to native auto and add no new level. Every version-1
`auto` would then silently widen from "edits only" to "the CLI judges commands". That breaks the
backward-compatibility rule in spirit: the file loads, but it launches something else.

*Alternative D (rejected):* map `auto` to Claude's `dontAsk`, which denies instead of prompting
anything not pre-allowed. A wave would no longer stall, but it would fail silently on every
command it is not allowed to run, and #23 would still need hand-written rules.

### D2 — Old files keep their meaning: the version-1 `auto` migrates to `accept-edits`

This change adds four rewrites to the v1→v2 migration step that `supervisor-vocabulary-split`
defines. They run on documents that declare `version = 1` or no version:

- `[supervisor] agent_approval = "auto"` becomes `[workers] approval = "accept-edits"`.
- `[supervisor] approval = "auto"` becomes `[orchestrator] approval = "accept-edits"`.
- `[profiles.*] approval = "auto"` becomes `"accept-edits"`.
- every `[clis.*].approval_args` key `auto` is renamed to `accept-edits`, keeping its value.

The step stays pure and in memory, and never touches the file. It also reads an unset worker
approval in a version-1 document as `accept-edits`, not as the version-2 default `auto` (D3).

This keeps the vocabulary split's "unchanged meaning" promise. A version-1 *value* `auto` meant
`acceptEdits`, and it still launches `acceptEdits`. One reading rule is added so that existing
scenarios stay true without being rewritten: a requirement that names a version-1 path set to
`"auto"` (for example `cli-provider`'s `agent_approval = "auto"` → `--permission-mode acceptEdits`,
or `session-state`'s resume scenario) is read as that path's version-2 name set to
`"accept-edits"`. A scenario whose config declares no version and sets no worker approval is a
version-1 document, so its worker approval stays `accept-edits` and needs no rewrite either.

**The #28 user's own file.** It has `approval_args = { auto = "--permission-mode auto" }` and a
worker level of `auto`. Both migrate to `accept-edits`, and the worker still launches
`--permission-mode auto`. Their workaround keeps working, and the warning tells them how to drop
it.

**The warning.** A version-1 file that writes `auto` at any of those paths prints one
`core-deprecation` line per file. It names each path and says that `auto` there means
`accept-edits`, and that in version 2 `auto` launches the CLI's own auto mode. The registry entry
has:

- item: `"auto"` (version-1 approval value)
- kind: `configuration value`, a new kind. The registry is specced in `supervisor-vocabulary-split`,
  which must add this kind.
- replacement: `"accept-edits"`
- deprecating release: 0.21.0
- removal release: none. It is read through migration for the life of the version-1 format.

`config check` reports it from the same entry.

**Same-release constraint.** The version-2 bump and this rewrite must ship together. Once a
released binary writes `version = 2` with `approval = "auto"` meaning `acceptEdits`, the rewrite
needs its own v2→v3 step. That is recorded in tasks as a merge-order gate.

### D3 — The version-2 default is `auto`; a version-1 file keeps `accept-edits`

Maintainer decision, 2026-11-17. The `#[default]` stays on `Auto`, which now means the CLI's own
auto mode. A fresh `git paw init` and a version-2 file with no worker approval launch it. This is
the point of the change: a new user's wave runs routine commands without stalling (#23, #29).

An existing user's launch must not widen. Version 1's implicit default was `auto`, which launched
`acceptEdits`. So the version-1 reading of an unset worker approval is `accept-edits`, and the
loaded worker approval resolves like this:

1. If any loaded document sets a worker approval, merge precedence decides as before. An absent
   value never overrides a set one. This matters for a version-2 global file with `manual` and a
   version-1 repository file with nothing set: the result stays `manual`.
2. Otherwise, if the highest-precedence loaded document (the repository file, else the global
   file) declares `version = 2`, the worker approval is `auto`.
3. Otherwise it is `accept-edits`. That covers a version-1 top document and the case with no
   configuration file at all. Neither of those users chose version 2.

The rule is not a per-document rewrite to an explicit `accept-edits`. Such a rewrite would let a
version-1 repository file with nothing set override a global `manual` and widen it.

**Orchestrator default.** `[orchestrator] approval` keeps its existing default: unset, it inherits
the resolved worker approval. In a version-2 file with neither set, headless orchestrator
invocations therefore run at `auto` (`--permission-mode auto -p` for Claude, D6 step 6). In a
version-1 file they stay at `accept-edits`. This keeps `supervisor-vocabulary-split`'s "unchanged
meaning, type and default" for the moved key, and it needs no second default rule. A separate
orchestrator default would make the pair disagree in a file that sets neither.

**Delegate default.** A delegate profile's approval still defaults to `manual`. It does not follow
the worker default.

The `init` template comment lists the four levels, names `auto` as the default, and has one line
each on what `accept-edits` and `auto` launch for Claude.

### D4 — `init` and the rewriting commands never widen a file's approvals

`supervisor-vocabulary-split` lets `git paw init` backfill `version = 2` into any file without a
`[supervisor]` table. Under D2 and D3, stamping `version = 2` on an existing unversioned file can
widen it in two ways. An `auto` at `[profiles.*] approval` or an `approval_args` key would turn
from `accept-edits` into `auto`. An unset worker approval would turn from `accept-edits` into the
version-2 default `auto`. Every version-1 file lacks `[workers]`, so the second case covers every
unversioned file. Therefore:

- `init` stamps no existing unversioned file. It treats one the way it treats a file with
  `[supervisor]`: it leaves `version` absent and prints its one pointer line to
  `git paw config check`. A fresh file from `init` still declares `version = 2`.
  This overrides `supervisor-vocabulary-split`'s backfill rule and its "init stamps a
  layout-neutral file" scenario. That change must be amended to match before both merge.
- `add-cli` and `remove-cli` serialize the in-memory model, so the global file they rewrite says
  `accept-edits` where the version-1 file said `auto`. When the version-1 file set no worker
  approval, the rewritten file sets `[workers] approval = "accept-edits"`. The launch is
  unchanged. Their existing one-line rewrite notice also names the approval values they renamed
  or added.

### D5 — Show the truth: dry-run line and doctor check (alternative C)

**Dry-run.** `dry_run_launch_line` already has the resolved profile and the worker's level
(`src/commands/launch.rs:385-388`). It adds a second line:

```
      runs: env … claude --setting-sources project,local --permission-mode acceptEdits
  approval: accept-edits → --permission-mode acceptEdits
```

- When the flags are empty, the line says `no flags (the CLI's default permission mode)`.
- When the flags come from an override, it ends with `([clis.<id>].approval_args)`.
- It is printed once per worker, at that worker's own level (profile or default).
- It is informational stdout of the plan, not a warning.

**Doctor.** A new `approval levels` check sits in the CLI group. It covers every CLI a launch can
use: the default worker CLI, spec-driven and preset CLIs, every profile's CLI, and the
orchestrator CLI. That is the set the sandbox-mode check and *Doctor evaluates worker profiles at
their own approval level* already use. For each CLI it prints:

```
claude (claude family)
  manual        no flags
  accept-edits  --permission-mode acceptEdits   ← workers
  auto          --permission-mode auto          ← profile deep
  full-auto     --dangerously-skip-permissions  ← orchestrator
```

- Overrides are tagged `[clis.<id>].approval_args`.
- Status is ✓. It is ⚠ when a level in effect other than `manual` launches no flags: the panes
  then run in the CLI's default mode, and prompts reach a human. The remedy names
  `[clis.<id>].approval_args`.
- It is never ✗, and it never runs the CLI. `--json` carries the same rows under the check's name.
- When a Claude-family CLI is in effect at `accept-edits`, the check detail adds one line saying
  that `auto` would launch the CLI's own auto mode. This is how #28's user would have found it.

### D6 — Fact gate for the Claude `auto` row (assumption A1)

Before the row change merges, the same kind of gate `setup-cli` used (`src/cli_setup/facts.rs:276-311`)
runs against Claude Code ≥ 2.1.325. It drives a worker launched exactly as git-paw launches one
(`--setting-sources project,local`, the isolation env, `--permission-mode auto`) in a
`.git-paw/worktrees/` worktree. It checks that:

1. the mode is on (the status line shows auto mode);
2. `cargo test` or `npm test`, `git add` and `git commit` in the worktree run without a prompt;
3. a read-only `grep` over a file in the worktree runs without a prompt (#29);
4. `setup-cli`'s `ask` rules (`approve`, `tell`) still prompt under auto, as F4 already showed on
   2.1.322;
5. what happens when auto mode is unavailable to the account or model: an error at launch, a
   fallback with a notice, or a silent fallback;
6. a headless `-p` invocation at `--permission-mode auto`, which the orchestrator hand-off
   (`src/supervisor/escalation.rs:352-366`) would run.

The evidence goes into the provider doc comment. Whatever step 5 shows, doctor does not probe
(resolved question 2). If the fallback is silent, the docs warn about it.

### D7 — Alternatives B: worker-scoped allow rules (deferred, not rejected)

#23's second complaint is that rules written for workers also apply to the operator. A
worker-only rules file would fix that. Claude Code accepts `--settings <file>`, and
`delegate_denied_flags` already lists it (`src/agent_cli/mod.rs:425`). `setup-cli` could write
`.git-paw/cli/claude/worker-settings.json` with consent, and the launch builder could pass
`--settings` to workers only. It is deferred because:

- With `auto` available, the common case (#23's `npm test` and `git commit`, #29's reads) needs
  no rules.
- It adds a new git-paw-written settings file, which needs its own fact gate (how `--settings`
  merges with `--setting-sources project,local` and with the ask and deny rules), a `setup-cli`
  item, an undo record and doctor reporting. That is v0.23.0 per-CLI-audit work.
- It touches `approval-command-safety`'s "no seeding" rule, which needs a consented design of its
  own.

**What works today with no code change.** A user can declare
`[clis.claude-worker] family = "claude", args = "--settings .git-paw/worker-settings.json"` with
rules they write themselves. The docs task describes this, marked as verified only once the D6
gate confirms `--settings` is honoured alongside `--setting-sources`.

## Risks / Trade-offs

- **Auto mode is AI-judged, and it is the version-2 default.** `auto` lets the CLI run commands
  that its own model-based rules allow. That is a wider posture than `accept-edits`, and it is
  not a sandbox. A new user gets it without choosing it. → Existing files keep `accept-edits`
  (D3). The security-posture page states the difference. `start --dry-run` and doctor show which
  level is in effect. `manual` and `accept-edits` stay one line away. git-paw still escalates
  every prompt and answers none.
- **Default depends on the file's version.** An unset worker approval means `accept-edits` in a
  version-1 file and `auto` in a version-2 file. A user with no configuration file also gets
  `accept-edits`. → The configuration reference states the rule, and the dry-run line and doctor
  show the result per worker.
- **Same string, two meanings across versions.** `auto` in a version-1 file means
  `accept-edits`, and in a version-2 file it means auto mode. A snippet copied from old docs into a
  version-2 file gets the wider mode. → The warning, the `config check` finding, the migration
  table in the configuration reference, and the dry-run line all show it.
- **Account availability (A1).** If auto mode is gated by plan or model, an `auto` worker might
  not get it, and the CLI may fall back without saying so. Because `auto` is the default, more
  users meet this. → D6 step 5 records the behaviour. The docs warn about it. Doctor does not
  probe, because it never invokes a CLI (resolved question 2).
- **Codex `accept-edits` = `auto`.** The two middle levels are identical for Codex until the
  v0.23.0 audit verifies a distinct Codex mode. → Doctor and dry-run show identical flags, so
  nothing is hidden.
- **Ordering with `supervisor-vocabulary-split`.** If the v1→v2 step lands without D2, some
  pre-release version-2 files may contain `auto` meaning `acceptEdits`. → Merge-order gate in
  tasks. These are dev-only files, since nothing is released between the two changes.

## Migration Plan

1. Land after `config-schema-versioning` and `supervisor-vocabulary-split`, in the same release.
2. No user action is needed. Version-1 files launch identical commands, including those that set
   no worker approval, and warn when they use `auto`.
3. A version-2 file gets `auto` by default. To opt in from a version-1 file, migrate it and write
   `[workers] approval = "auto"` or leave it unset, or set `auto` on a profile.
4. **Rollback.** A version-2 file with `accept-edits` fails to load on v0.20.0. That is already
   true of every version-2 file, because `config-schema-versioning` refuses newer versions, so
   this adds no new rollback hazard.

## Resolved Questions (2026-11-17)

1. **Default worker approval.** `auto` in version 2 (D3). A version-1 file keeps `accept-edits`,
   both where it writes `auto` and where it sets no worker approval (D2, D3).
2. **Silent fallback when auto mode is unavailable.** The docs warn. Doctor does not probe: it
   stays read-only and never invokes a CLI.
3. **`manual` → `--permission-mode manual`.** Deferred to the v0.23.0 per-CLI audit, because it
   changes the launch command of every `manual` user.
4. **Registry kind of the version-1 `auto`.** A new `configuration value` kind. The registry is
   specced in `supervisor-vocabulary-split`, which must add the kind.
5. **Delegates at `auto`.** Allowed, unchanged, and admitted as the single writer. No cap at
   `accept-edits`.
