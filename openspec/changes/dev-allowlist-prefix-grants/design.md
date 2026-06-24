# Design — dev-allowlist-prefix-grants

## Context

The v0.7.0 dogfood showed agents stalling on the *same* safe dev-command
permission prompts every cycle. Two mechanisms compounded the friction, and a
third made the seeded preset actively wrong for non-git-paw repos:

1. **Exact-string whitelisting defeated by exit-probe wrappers.** Claude's
   "don't ask again for: `<cmd>`" affordance whitelists by the literal command
   string for some shapes. When an agent wraps a dev command as
   `cargo test … && echo "EXIT $?"` (or `RC=$?; echo $RC`), the trailing text
   varies per run, so the approval never matches the next invocation and the
   agent re-prompts forever. A bare, prefix-matchable command
   (`cargo check *`, `python3 -c`) generalises fine and only prompts once.

2. **Insufficient prefix coverage in the seeded preset.** The seeder
   (`src/supervisor/dev_allowlist.rs::DEV_ALLOWLIST_PRESET`) seeds
   `allowed_bash_prefixes`, which already prefix-matches, but the set is too
   narrow — common dev-loop verbs re-prompt because they aren't seeded.

3. **The preset hardcodes git-paw's own stack into every consumer.** The preset
   bakes `cargo *`, `just`, `mdbook build`, and `openspec …` into the allowlist
   of *any* repo that runs supervisor mode. A Node/Python/Go project gets those
   useless grants while its own `pytest` / `npm test` / `go test` are not
   covered — so it re-prompts on its real dev loop and carries dead grants for
   tools it does not have.

This change is the **lite** slice of the approval problem: better seeding plus
a skill nudge. It is deliberately distinct from the full broker-mediated
approval architecture (drift F, deferred to v0.9.0): no classifier, no
broker-as-trigger, no "chat-with-options" approvals.

### Affected code (informational; no code in this change)

- `src/supervisor/dev_allowlist.rs` — `DEV_ALLOWLIST_PRESET`, `effective_patterns`.
- `src/config.rs` — `CommonDevAllowlistConfig` (already has `enabled` + `extra`).
- `assets/agent-skills/*.md` — bundled supervisor/coordination skill bodies.

## Goals

- Seed **prefix-matchable** grants so a dev-loop command prompts at most once
  regardless of per-invocation argument variation.
- Make the built-in preset **stack-neutral**: only universally-safe commands are
  hardcoded; everything stack-specific is opt-in.
- Provide a low-friction path for a repo to declare its own stack grants
  (existing `[supervisor.common_dev_allowlist] extra` plus optional named stack
  presets `rust` / `node` / `python` / `go`).
- Nudge agents (via bundled skill prose) away from `&& echo "… $?"` /
  `EXIT=$?` exit-probe wrappers that defeat command-string whitelisting.

## Non-Goals

- A command classifier or risk model (v0.9.0, drift F).
- Broker-mediated interactive approvals / "chat-with-options" (v0.9.0).
- Writing allowlists to non-Claude CLIs (Codex/Gemini/etc. — v1.0.0
  hook-providers).
- Auto-detecting a repo's stack and seeding the matching preset implicitly. A
  stack preset is **opt-in** via config; auto-detection is out of scope here to
  avoid surprising grants.
- Removing or weakening any existing exclusion (destructive git/cargo verbs stay
  excluded from the hardcoded universal set).

## Decisions

### D1 — Prefix grants, not exact-string grants

The seeder writes to `allowed_bash_prefixes`, which already matches by prefix.
The fix is to ensure the *seeded entries are themselves prefix forms* (a verb or
verb+subcommand that subsumes all argument variants) rather than full
command lines. Concretely: seed `git diff` (matches `git diff --stat HEAD~1`),
not `git diff --stat HEAD~1`. Entries remain plain strings — Claude treats
`allowed_bash_prefixes` as a prefix set — so no wire-format change is needed;
the requirement is on the *shape* of the seeded values.

Rationale: a prefix grant collapses the infinite set of per-run argument
variations into one approval, which is the entire point of seeding.

### D2 — Universal-vs-stack split

Partition the preset into two tiers:

- **Universal (hardcoded).** Commands safe and useful in essentially any repo,
  independent of language/toolchain: read-only filesystem/search (`find`,
  `grep`, `sed -n`), and the non-destructive git verbs already in the preset
  (`git status/log/diff/show/fetch/commit/push/pull/merge/stash/add/restore/rm`).
  These stay in `DEV_ALLOWLIST_PRESET` as the single hardcoded source of truth.
- **Stack-specific (opt-in).** Everything tied to a particular toolchain —
  `cargo *`, `just`, `mdbook build`, `openspec …`, and the analogous
  `npm`/`pnpm`/`pytest`/`go test` sets — is **not** hardcoded into the universal
  preset. It is contributed two ways, which compose:
  1. `[supervisor.common_dev_allowlist] extra = [...]` — already exists; the
     free-form escape hatch.
  2. **Named stack presets** `rust` / `node` / `python` / `go` — curated,
     reviewed prefix bundles a repo opts into (mechanism: a config list such as
     `[supervisor.common_dev_allowlist] stacks = ["rust"]`, resolved by the
     seeder to the union of universal + each named stack + `extra`). The exact
     config key name follows local serde conventions; the contract is that
     opting into a stack adds that stack's curated prefixes and opting into
     none adds only the universal set.

git-paw's own repo then declares its `rust` stack (plus `extra` for `just` /
`mdbook` / `openspec`) in `.git-paw/config.toml`, restoring today's behaviour
for git-paw while making a bare consumer project stack-neutral.

Rationale: a consumer should never inherit git-paw's toolchain. Hardcoding only
universals means the default is correct everywhere; named presets keep the
common stacks one config line away without a free-form `extra` list.

### D3 — Inclusion/exclusion rubric carries forward

The existing rubric (bounded side-effects, no arbitrary network/code execution,
aligns with the git-safety protocol) still governs what may live in the
*universal* set and in each curated *stack* preset. Destructive verbs stay
excluded everywhere in the hardcoded/curated sets: `cargo install/run/bench`,
`git rebase/reset/checkout`, `git branch -D`, `git push --force`/`-f`, write-mode
`sed` (only `sed -n` is allowed), and arbitrary package-manager mutation beyond
the curated build/test verbs. A repo that genuinely wants a destructive prefix
adds it via `extra` (which is explicitly never validated).

### D4 — Skill nudge against exit-probe wrappers

Add prose to the bundled supervisor/coordination skills instructing agents to
run dev commands **bare** and read the exit status directly, rather than
wrapping them in `&& echo "EXIT $?"` / `RC=$?` probes. The nudge explains the
*why* (the wrapper text varies per run and defeats the CLI's command-string
whitelisting, forcing a re-prompt every invocation) so the guidance is
self-justifying. This lives in `lang-agnostic-skills` because that capability
already owns bundled-skill content, tone/example discipline, and the
no-language-leak audit — the nudge must itself be stack-neutral prose, so the
same audit applies.

### D5 — Home capability for each delta

- `dev-command-allowlist` — owns the preset constant and seeding mechanics, so
  the prefix-grant shape requirement and the universal-vs-stack split land here
  (MODIFIED to the two existing requirements + ADDED stack-preset requirement).
- `lang-agnostic-skills` — owns bundled-skill content + audits, so the
  exit-probe nudge lands here (ADDED requirement). Chosen over
  `skill-standardization`, which only governs the *format/loader* of skills, not
  their prose.

## Risks / Trade-offs

- **Behaviour change for git-paw's own repo.** Removing `cargo`/`just`/`openspec`
  from the universal hardcode means git-paw must opt back in via its committed
  `.git-paw/config.toml`. Mitigation: ship that config change as part of this
  work so git-paw's dogfood loop keeps its grants; the migration is one config
  block, fully backward-compatible for everyone else.
- **Over-broad prefix grants.** A prefix like `git push` matches
  `git push --force` unless excluded. Mitigation: the rubric (D3) keeps the
  universal set to non-destructive verbs; force-push is excluded by the verb not
  being a hardcoded prefix and by the documented exclusion list.
- **Stack-preset drift.** Named presets can rot relative to real toolchains.
  Mitigation: keep them small/curated, single-source-of-truth in code, and
  covered by content tests; `extra` remains the escape hatch for anything not
  curated.
- **Nudge ignored by agents.** Prose guidance is advisory. Mitigation: the
  broadened prefix coverage (D1/D2) already removes most re-prompts even when an
  agent wraps a command; the nudge is the belt-and-suspenders second line.
- **Backward compatibility.** All changes are additive: broader prefixes only
  reduce prompts, the new stack config defaults to empty (universal-only), and
  pre-existing configs/`settings.json` load and merge unchanged.
