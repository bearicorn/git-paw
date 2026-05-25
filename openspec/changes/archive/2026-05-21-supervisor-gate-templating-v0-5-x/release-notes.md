# v0.5.0 release-notes bullets (supervisor-gate-templating-v0-5-x)

These bullets are intended to be copied into the v0.5.0 release-prep
commit's CHANGELOG.md / archive plan. They cover the supervisor skill's
new repo-configurable gate commands.

## Highlights

- **Supervisor gate commands are now repo-configurable.** The five-gate
  verification workflow (testing → regression → spec audit → doc audit
  → security audit) reads its tooling steps from new
  `[supervisor].{test,lint,build,doc_build,spec_validate,fmt_check,security_audit}_command`
  keys in `.git-paw/config.toml`. Pre-v0.5.x configs continue to work
  unchanged: missing keys render as `(not configured)` in the supervisor
  skill, and the supervisor agent skips that gate's tooling invocation
  while still applying the gate's manual review (e.g. OWASP-category
  diff scan for the security gate, spec scenario coverage check for the
  spec gate).

- **`git paw init` writes a commented-out `[supervisor]` block** listing
  every gate-command key (`enabled`, `cli`, `test_command`,
  `lint_command`, `build_command`, `fmt_check_command`,
  `doc_build_command`, `spec_validate_command`,
  `security_audit_command`) with example values for Rust, Node, Python,
  and Go stacks. Uncomment the block to opt in; the supervisor skill
  picks up the values at session boot.

- **`{{CHANGE_ID}}` in `spec_validate_command` is per-invocation.** The
  supervisor agent substitutes `{{CHANGE_ID}}` at verification time with
  the change name being audited — `skills::render` does NOT expand it at
  session boot. This mirrors how `{{BRANCH_ID}}` flows on the coding-agent
  side and lets a single rendered skill verify many changes over a
  session lifetime.

## Follow-up

- MILESTONE.md drift 67 ("just/cargo here are specific to this repo") is
  closed by this change. The supervisor skill is now repo-agnostic for
  the five-gate workflow; downstream multi-language repos can adopt
  supervisor mode without inheriting Rust-specific tooling assumptions.
