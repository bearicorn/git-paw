# Tasks — dev-allowlist-prefix-grants

## 1. Prefix-grant seeding

- [x] 1.1 Audit `DEV_ALLOWLIST_PRESET` and all curated entries so every seeded
  value is a bare command **prefix** (verb / verb+subcommand) that subsumes
  argument variants — never a fully-argumented command line. Confirm the
  seeder writes prefix forms into `allowed_bash_prefixes`.
- [x] 1.2 Confirm `effective_patterns` and `setup_dev_allowlist` preserve the
  existing merge/dedupe/non-destructive contracts when fed the new
  union (universal + stacks + extra).

## 2. Genericise the preset (universal-only hardcode + stack presets/config)

- [x] 2.1 Reduce `DEV_ALLOWLIST_PRESET` to the **universal** set only: git
  read-only + git non-destructive write + `find` / `grep` / `sed -n`. Remove
  the stack-specific `cargo *`, `just`, `mdbook build`, `openspec *` entries.
- [x] 2.2 Add named, curated stack-preset constants `rust` / `node` / `python` /
  `go` in the dev-allowlist module (single source of truth, prefix forms,
  obeying the D3 exclusion rubric).
- [x] 2.3 Add a `stacks: Vec<String>` (name follows local serde conventions) to
  `CommonDevAllowlistConfig` with `#[serde(default)]`; resolve selected stacks
  to the union (universal + each selected stack + `extra`), de-duplicated, in
  the seeder.
- [x] 2.4 Update git-paw's own `.git-paw/config.toml` to opt into the `rust`
  stack and add `extra = ["just", "mdbook build", "openspec ..."]` so git-paw's
  dogfood loop keeps its current grants.

## 3. Skill nudge prose

- [x] 3.1 Add the "run dev commands bare; don't wrap in `&& echo \"… $?\"` exit
  probes" guidance (with the command-string-whitelisting rationale) to the
  bundled supervisor and coordination skill assets. Author as stack-neutral
  prose so it passes the no-language-leak audit.

## 4. Tests (behavioral)

- [x] 4.1 Seeding emits prefix forms: assert seeded universal entries are bare
  prefixes (e.g. `git diff`), not fully-argumented lines.
- [x] 4.2 Universal-only default: fresh settings + no stacks + empty `extra`
  yields exactly the universal preset (no `cargo`/`just`/`mdbook`/`openspec`).
- [x] 4.3 Non-Rust project does not get cargo grants by default (no stacks
  selected → no `cargo *`).
- [x] 4.4 Stack selection: `stacks = ["rust"]` seeds universal + curated rust
  prefixes; `stacks = ["node"]` seeds node prefixes and NOT `cargo *`; multiple
  stacks compose as a de-duplicated union.
- [x] 4.5 `extra` still appends after preset, dedupes against preset + selected
  stacks, and is not validated (existing contracts hold).
- [x] 4.6 Skill-content test: bundled supervisor skill contains the no-exit-probe
  guidance + rationale; no-language-leak audit still passes with the new prose.

## 5. Docs

- [x] 5.1 Update the configuration reference for `[supervisor.common_dev_allowlist]`:
  document `stacks` (named presets + composition) and the universal-vs-stack split.
- [x] 5.2 Update skill docs / user guide to mention the bare-command guidance.
- [x] 5.3 `mdbook build docs/` succeeds.

## 6. Gates

- [x] 6.1 `openspec validate dev-allowlist-prefix-grants --strict` passes.
- [x] 6.2 `just check` (fmt + clippy + tests) passes.
- [x] 6.3 `just deny` passes; no new dependencies; no `unwrap()`/`expect()` in
  non-test code; public items documented.
