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
(`broker.sh *`, `docs-fetch.sh find *`, ten read-only `sweep.sh` subcommands once v0.18.0
`orchestrator-read-surface` adds `events`), `ask` rules for `sweep.sh approve` and (v0.19.0
`verified-agent-send`) `sweep.sh tell`, the protection rules `Edit(**/.git-paw/scripts/**)` (deny)
and `Edit(**/.git-paw/config.toml)` (ask), a setup record, an undo whose shape check accepts only
`Bash(.git-paw/scripts/<name>…)` values, and a doctor setup check that compares
the applied entries with what the running binary would plan. The row is `verified` only after a scripted fact gate
(F1–F6) passed against the installed Claude Code for those exact entries and rules.

Other path consumers: `doctor::check_bundled_scripts` (messages name `.git-paw/scripts/`),
`drive::SweepLearningSink::record` (`repo_root/.git-paw/scripts/sweep.sh`), ~45 literals in
bundled prose, 11 docs chapters, ~30 tests.

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
and are not installed. Everything is overwritten on each install (binary-managed, as today).

### D2 — Prose names helpers by placeholder, rendered from the registry

`{{BROKER_HELPER}}`, `{{SWEEP_HELPER}}`, `{{DOCS_FETCH_HELPER}}` render to the package invocation
path (`.git-paw/skills/coordination/scripts/broker.sh`, …), derived from the registry so a future
move is one edit. They join `render`'s known-placeholder set (no unknown-placeholder warning) and
are substituted in `build_boot_block` too. A guard test fails if any registered template contains a
literal `.git-paw/scripts/` or `.git-paw/skills/…/scripts/` path.
*Alternative:* hand-edit the literals to the new path — rejected: the same ~45-site churn again on
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
makes the `start` paths inject `docs-fetch` when `docs_base_url` is set, as `mcp-agent-docs`'
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

1. Package install + compatibility copy in `run_init` and provisioning; doctor per package.
2. Placeholders in templates; `build_boot_block`/`render` substitute; guard test; re-point tests.
3. `setup-cli` grant table, protection, undo and doctor at the package paths; fact gate re-run (or
   the row set plan-only).
4. One selection across launch paths; managed-block index.
5. Docs and upgrade note.
Rollback: revert; consumer repos keep a stale `.git-paw/skills/` (gitignored, inert) and the
compatibility copies keep the old path working.

## Open Questions

- **Decision for the user — move the invocation path or not.** Recommended: move it (D3, with the
  compatibility copy). Alternative: keep `.git-paw/scripts/` as the invocation path and treat
  packages as the source/export unit only — zero allow-rule churn, but packages do not own their
  helpers once installed.
- **Decision for the user — inline vs reference-only skill bodies** in the managed block (D6).
- **Decided (2026-10-20):** recorded `setup-cli` grants are migrated by the user — doctor reports
  them out of date and the user re-runs `--undo` then `--allowlist` — never by git-paw (D4).
- When is the compatibility copy removed? Recommendation: at v1.0.0 under the deprecation policy,
  with `doctor` warning one release ahead.
