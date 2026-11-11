## Context

State after `skill-package-layout` (symbols): `skills::bundled_packages()` and
`skills::packages_for_worktree(inputs)` exist; `run_init` and `init::provision_worktree_helpers`
still write each helper flat into `.git-paw/scripts/`; the sidecar's managed block
(`agents::generate_worktree_section` via `WorktreeAssignment.skill_content`) inlines the rendered
coordination (+ docs-fetch) skill; `build_boot_block` renders `boot-block.md` /
`boot-block-mcp.md`, both with hard-coded `.git-paw/scripts/broker.sh`.

v0.17.0 `classifier-retirement` deletes the classifier, including its managed-helper rule
(`auto_approve::is_managed_script_invocation`), so git-paw itself approves no helper call. The one
place git-paw pre-approves helpers is v0.17.0 `cli-setup-consent`'s `git paw setup-cli claude
--allowlist` (`src/cli_setup/`): a grant table of exact relative paths under `.git-paw/scripts/`
(`broker.sh *`, `docs-fetch.sh find *`, ten read-only `sweep.sh` subcommands including v0.18.0
`orchestrator-read-surface`'s `events`), `ask` rules for `sweep.sh approve` and (v0.19.0
`verified-agent-send`) `sweep.sh tell`, the protection rules `Edit(**/.git-paw/scripts/**)` (deny)
and `Edit(**/.git-paw/config.toml)` (ask), a setup record, an undo whose shape check accepts only
`Bash(.git-paw/scripts/<name>…)` values, and a doctor setup check that compares
the applied entries with what the running binary would plan. The row is `verified` only after a scripted fact gate
(F1–F6) passed against the installed Claude Code for those exact entries and rules.

Other path consumers: `doctor::check_bundled_scripts` (messages name `.git-paw/scripts/`) and the
Supervisor check's `sweep.sh` probe, `drive::SweepLearningSink::record`
(`repo_root/.git-paw/scripts/sweep.sh`), `cli_setup::doctor::{is_broad, helper_paths}` (keyed on
`facts::HELPER_DIR`) and its `MAX_WORD` = 32 (sized for `.git-paw/scripts/docs-fetch.sh`; the
package path `.git-paw/skills/docs-fetch/scripts/docs-fetch.sh` is 48), ~99 literals in seven
bundled prose assets (`orchestrator.md` 67, `coordination.md` 16, `boot-block-template.md` 7,
`docs-fetch.md` 4, `boot-block-mcp-template.md` 2, `orchestrator-session-context.md` 2,
`escalation-framing.md` 1), 88 mentions in 16 docs chapters, 34 test files.

State after v0.19.0 that this change builds on (verified 2026-11-11):
- Root installs: `init::refresh_root_scripts` writes the four flat helpers and
  `init::export_skills` writes `.git-paw/skills/{orchestrator,cli-setup}/SKILL.md` (plus a
  self-ignoring `.git-paw/skills/.gitignore`). `run_init` calls both; a supervised launch calls
  both through `supervisor::write_orchestrator_context` (`supervisor-launch` *A supervised launch
  refreshes the bundled root helper scripts*). The bare and spec-driven `start` paths call neither,
  so a fresh clone has no root helpers until `git paw init` (DF-06).
- Writes: `write_managed_file` skips identical content (`WriteOutcome::Unchanged`) and otherwise
  calls `fs::write` on the target in place; `install_script` then `chmod`s it. A reader that opens
  the file mid-write sees a truncated script.
- Worktree helpers: `init::provision_worktree_helpers` is called next to
  `worktree_provision::provision_worktree` (env, ports, hook) at three sites — `commands/start.rs`
  bare and spec-driven loops and `commands/launch.rs::attach_agent` (supervisor `start`, `add`).
  Only `attach_agent` injects docs-fetch.
- Discovery: `_paw_common.sh` sets `PROJECT_ROOT=$(git rev-parse --show-toplevel)` and
  `PAW_DIR=$PROJECT_ROOT/.git-paw`; `sweep.sh` reads `$PAW_DIR/sessions/`, keeps
  `.sweep-progress` / `.sweep-stuck-dedup` there, and claims panes at
  `$PROJECT_ROOT/.git-paw/tmp/approve-pane-<N>.claim` — the same formula the binary uses at the
  main root. Inside a linked worktree all of these point at the worktree (DF-54).

## Goals / Non-Goals

**Goals:**
- Install packages, not flattened scripts; make the package path the one that prose names.
- Keep every existing invocation of `.git-paw/scripts/<helper>` working through 0.x.
- Move the `setup-cli` grants, protection rules and undo to the package paths without git-paw ever
  rewriting a recorded grant itself.
- One package selection for provisioning *and* injection on every launch path.
- The managed block references the packages.

**Non-Goals:**
- Installing into any vendor's skill directory (`.claude/skills/`, …) — v0.18.0's stance: consumers
  copy or symlink; git-paw stays agnostic.
- Writing CLI allow rules outside the consented `git paw setup-cli`, or migrating recorded grants
  on the user's behalf (decided 2026-10-20: doctor reports, the user re-runs `--undo` then
  `--allowlist`).
- Making the granted entry point the git-paw binary itself (`cli-setup-consent` D6 option b): the
  deny rules keep closing the edit path, and a binary entry point would change every helper
  invocation again.
- Removing `.git-paw/scripts/` (v1.0.0 deprecation policy).
- Per-CLI variants (`per-cli-skill-templates`).
- Skill hot-reload (`openspec/deferred/skill-hot-reload`). It stays deferred: installed packages give
  an agent a file to re-read, and MCP `get_skill` (shipped in v0.7.0) already covers its
  `/skills/content` half, so only drift detection would remain; nothing in v0.20.0 needs it. If
  revived, it should hash the installed `SKILL.md` rather than add broker endpoints.

## Decisions

### D1 — Install layout: `SKILL.md` + `scripts/` + `references/`, never `assets/`

`.git-paw/skills/<name>/SKILL.md` is rendered with the same inputs the sidecar uses for that
location (worktree: the agent's branch and session broker URL; repo root: current branch,
configured broker URL, `[supervisor]` gate commands — the inputs MCP `get_skill` and v0.18.0's
orchestrator export already use). `scripts/` are written `0o755`. `references/` are rendered like
`SKILL.md`. A package's `assets/` are render-time inputs (boot-block templates, per-CLI fragments)
and are not installed. Every install writes each file that is missing or differs and leaves an
identical one untouched (binary-managed, as `write_managed_file` does today), so init's
`Updated …` lines and `supervisor-launch`'s "not rewritten" scenario keep their meaning.

### D2 — Prose names helpers by placeholder, rendered from the registry

`{{BROKER_HELPER}}`, `{{SWEEP_HELPER}}`, `{{DOCS_FETCH_HELPER}}` render to the package invocation
path (`.git-paw/skills/coordination/scripts/broker.sh`, …), derived from the registry so a future
move is one edit. They join `render`'s known-placeholder set (no unknown-placeholder warning) and
are substituted by every renderer of bundled prose: `render` (worktree skills, MCP `get_skill`),
`render_standardized` (the exported orchestrator skill, MCP `get_skill` for it),
`build_boot_block`, the orchestrator session context and the escalation framing (the last says
"helpers under `.git-paw/scripts/`" today and is reworded to name the helpers). A guard test fails
if any of those prose files contains a literal `.git-paw/scripts/` or `.git-paw/skills/…/scripts/`
path. The helper scripts' own usage text is not prose an agent is told to follow and is left
alone.
*Alternative:* hand-edit the literals to the new path — rejected: the same ~99-site churn again on
the next move, and nothing stops drift.

### D3 — The compatibility copy

Wherever a package's helpers are installed, the same bytes are also written to
`.git-paw/scripts/<helper>` (with `_paw_common.sh`). Nothing bundled names that path. It keeps
working: boot prompts already sitting in a running agent's context across a binary upgrade; the
maintainer's and consumers' notes; and — the reason it matters — CLI allow rules keyed on
`.git-paw/scripts/…`, whether hand-written or written by v0.17.0–v0.19.0 `git paw setup-cli`.
Those rules will not match the new prose path; the upgrade note and doctor's setup check (D4) tell
users what to do. Removal at v1.0.0 goes through the freeze's deprecation policy.
*Alternatives:* a shim script that `exec`s the package copy (one more script git-paw must trust,
and a second hop for the integrity check); a hard move (breaks every hand-written allow rule and
every in-flight boot prompt at once; MILESTONE frames v0.20.0 as non-breaking); keep
`.git-paw/scripts/` as the invocation path and only package the source (rejected as the primary
option because it leaves MILESTONE's "each owning its helper" unmet — offered as a decision for
the user, see Open Questions).

### D4 — `setup-cli` grants and protection move to the package paths; the user re-applies

- **Grant table.** Allow entries name `.git-paw/skills/<package>/scripts/<helper>`, derived from the
  registry (`coordination`/`broker.sh`, `docs-fetch`/`docs-fetch.sh find`, `orchestrator`/`sweep.sh`
  with the same ten read-only subcommands). No allow entry names the compatibility path: nothing
  bundled invokes it, and granting both would double the surface for no caller.
- **Ask rules at both paths.** `sweep.sh approve` and `sweep.sh tell` get an `ask` rule at the
  package path *and* at `.git-paw/scripts/sweep.sh`, because the compatibility copy still runs and a
  user may hold a broad 0.16.0-era `Bash(.git-paw/scripts/sweep.sh *)` grant; the ask rule wins over
  it.
- **Protection.** `Edit(**/.git-paw/skills/**)` joins `Edit(**/.git-paw/scripts/**)` in
  `permissions.deny`. The whole package directory is denied rather than only `*/scripts/**`: the
  pattern keeps the shape gate F5 verified (`**/<dir>/**`), and nothing an agent should edit lives
  there (it is binary-managed output). The compatibility rule stays because the copies stay.
- **Verification.** The `claude` row is marked verified only after the scripted fact gate is re-run
  with the package-path entries and both deny rules (F2–F6 steps re-pointed; F5/F6 extended to the
  package directory, its `_paw_common.sh`, and a nested worktree's package copy) and every blocking
  step passes. Otherwise the row ships `verified = false`: `--dry-run` and `--undo` work, writing
  exits 2 (`cli-setup` *An item whose facts are unverified is plan-only*).
- **Undo.** The shape check accepts `Bash(.git-paw/skills/<package>/scripts/<name>…)` as well as the
  old form, and protection is removed only when no allow entry under either `.git-paw/scripts/` or
  `.git-paw/skills/` remains. A v0.17.0–v0.19.0 record therefore undoes cleanly.
- **Doctor's grant checks.** The broad-grant advisory's shape 8 (`is_broad`) recognises a helper
  word at either `.git-paw/skills/<pkg>/scripts/` or `.git-paw/scripts/`, and `MAX_WORD` grows to
  keep the longest package path (48 characters) whole, as *A wide helper grant is listed* requires
  verbatim listing. The grant-residue check already matches a helper by its file name whatever path
  names it, so it needs no spec change; its rendering inherits the larger `MAX_WORD`. The
  modified-helper drift check (`helper_paths`) walks the paths the applied allow entries name.
- **Migration is the user's.** A record written before this change no longer matches the plan, so
  doctor's existing out-of-date rule (`preflight-diagnostics` *Doctor reports CLI setup state*)
  reports ⚠ with the remedy `git paw setup-cli claude --undo` then `--allowlist`. `init`,
  provisioning and every other command leave the settings file and the record alone.
*Alternatives:* `--allowlist` replaces stale entries in place — rejected: it would remove entries
without an undo plan the user saw, and "git-paw never rewrites grants itself" is the consent model;
granting both paths — rejected, see above; leaving the protection at `.git-paw/scripts/**` only —
rejected: the granted package helpers would be editable without a prompt in `acceptEdits`.

### D5 — One selection drives provisioning *and* injection on every launch path

`packages_for_worktree` is consulted by bare `start`, spec-driven `start`, supervisor `start`
and `add`; each writes the selected packages and injects the same set's skill text and index. This
makes the bare and spec-driven `start` paths inject `docs-fetch` when `docs_base_url` is set (today
only `attach_agent` — supervisor `start` and `add` — does), as `mcp-agent-docs`'
*Gated injection into agent sessions* already requires — a visible behavior change, called out in
the changelog.

### D6 — Managed block: index + inline bodies

`WorktreeAssignment` gains the package list; `generate_worktree_section` renders a `## Skills`
index (one entry per package: name, frontmatter description, `SKILL.md` path, helper path)
before the inlined skill bodies, all generated from the registry. Bodies stay inline: the boot
prompt tells agents to read `.git-paw/AGENTS.local.md` first, and a reference-only block would add a
second hop that cheap workers are the most likely to skip.
*Alternative:* reference-only (body lives only in `SKILL.md`) — smaller sidecar, single copy;
rejected for now on reliability, listed as a decision.

### D7 — Every `start` installs the root packages (DF-06)

v0.19.0 already refreshes the root helpers on a supervised launch, so the gap is the bare and
spec-driven `start` paths: after a fresh clone they launch agents whose orchestrator (the user's
own root session) has no `sweep.sh`, and nothing tells the user to run `init`. One function —
`init::refresh_root_scripts`, generalised to install every package plus its compatibility copies
— is called by `run_init`, `write_orchestrator_context` and, before the first pane, both
non-supervised `start` paths. It writes only missing or differing files, so a repeat start is a
no-op on disk. It does not create the config, touch `.gitignore` (the self-ignoring
`.git-paw/skills/.gitignore` keeps the packages out of `git add -A`) or inject the `AGENTS.md`
pointer: those remain `init`'s, because they edit tracked files.
*Alternative:* install only files that are missing, never refresh stale ones on a bare start —
rejected as the default: the supervised path already refreshes, and two rules for the same files
would let a bare start leave an old `sweep.sh` next to a new skill. Listed as a decision.

### D8 — Atomic replacement of installed files

`write_managed_file` (and so `install_script`) writes the new content to a temporary file created
exclusively in the target's directory (`O_CREAT|O_EXCL`, a random suffix, owner-only until the
final mode is set), sets the final mode (`0o755` for scripts), then `rename`s it over the target
and removes the temporary on any error. A same-directory rename is atomic on POSIX filesystems, so
a running `sweep.sh` (bash reads its script incrementally) keeps the old inode and the next run sees
the complete new file. The existing symlink refusal stays (`rename` would replace a link rather than
follow it, but refusing keeps the error explicit). No new dependency: `std::fs` only.

### D9 — Session state resolves from the main checkout (DF-54)

`_paw_common.sh` gains `MAIN_ROOT`: the parent of
`git rev-parse --path-format=absolute --git-common-dir` (the main working tree for both the main
checkout and every linked worktree). `sweep.sh` keeps `SESSIONS_DIR`, `.sweep-progress`,
`.sweep-stuck-dedup` and the approve claims under `$MAIN_ROOT/.git-paw/`; the claim path then
matches the binary's again, so the cross-process approval exclusion spans a worktree-run helper.
`PROJECT_ROOT` keeps meaning "the current checkout", which `broker.sh` needs to derive the agent id
from the worktree branch. Resolution depends on git, not on where the script lives, so it holds for
the package path and the compatibility copy alike. `--path-format` needs git ≥ 2.31; on an older
git the helper resolves the relative common dir against `PROJECT_ROOT` itself.

## Risks / Trade-offs

- [Durable "don't ask again" rules for `.git-paw/scripts/…` that 0.16.0's classifier or the user
  left in the settings file keep granting the compatibility copy] → the plan keeps
  `Edit(**/.git-paw/scripts/**)`, so re-applied setup still protects the copies; doctor's
  broad-grant advisory (v0.17.0) lists the wide ones; the upgrade note says to review them; git-paw
  edits none.
- [Grants applied before the upgrade stop matching new invocations] → every helper call prompts
  (and is escalated under `--unattended`) until the user re-runs `--undo` then `--allowlist`;
  doctor ⚠ names the remedy; the upgrade note says so.
- [The fact gate is not re-run before release] → the row ships plan-only; `--dry-run` still shows
  the plan, and the docs say the allowlist item awaits verification.
- [Two copies of each helper on disk] → same bytes from the same constant in the same call; doctor
  flags divergence.
- [Longer command lines in boot blocks] → cosmetic.

## Migration Plan

1. Atomic `write_managed_file`; package install + compatibility copy in `run_init`, the root
   refresh (init, supervised launch, every `start`) and provisioning; doctor per package.
2. Placeholders in templates; every renderer substitutes; guard test; re-point tests.
3. `setup-cli` grant table, protection, undo and doctor at the package paths; fact gate re-run (or
   the row set plan-only).
4. One selection across launch paths; managed-block index.
5. `_paw_common.sh` / `sweep.sh` main-checkout resolution.
6. Docs and upgrade note.
Rollback: revert; consumer repos keep a stale `.git-paw/skills/` (gitignored, inert) and the
compatibility copies keep the old path working.

## Open Questions

- **Decided 2026-11-11 — move the invocation path** (D3, with the compatibility copy). Alternative: keep `.git-paw/scripts/` as the invocation path and treat
  packages as the source/export unit only — zero allow-rule churn, but packages do not own their
  helpers once installed.
- **Decided 2026-11-11 — skill bodies stay inlined** in the managed block, with the `## Skills`
  index beside them (D6).
- **Decided (2026-10-20):** recorded `setup-cli` grants are migrated by the user — doctor reports
  them out of date and the user re-runs `--undo` then `--allowlist` — never by git-paw (D4).
- **Decided 2026-11-11 — DF-06 scope:** a bare or spec-driven `start` also refreshes *stale*
  root packages, the same rule as the supervised launch (D7).
- **Follow-up, not in this change — config resolution from a worktree (DF-54 follow-on):** the helpers
  still read `.git-paw/config.toml` from the current checkout, i.e. a worktree's committed copy,
  which can lag an uncommitted edit in the main checkout (e.g. `[broker] port`). Agents normally get
  `GIT_PAW_BROKER_URL`, so this change leaves it; resolving the config from `MAIN_ROOT` too would be
  a one-line follow-up.
- When is the compatibility copy removed? Recommendation: at v1.0.0 under the deprecation policy,
  with `doctor` warning one release ahead.
