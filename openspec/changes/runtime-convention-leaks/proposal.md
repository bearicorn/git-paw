## Why

Two of git-paw's own project conventions are compiled into runtime code that every
consumer runs, in violation of the export-agnosticism principle:

1. **A Rust toolchain leak in prompt classification.** `classify_command_class`
   (`src/supervisor/permission_prompt.rs:96-103`) hard-codes `cargo fmt`, `cargo clippy`,
   `cargo test` and `cargo build` to a `PermissionType::Cargo` class. This directly
   contradicts the stack-neutrality that `src/supervisor/auto_approve.rs:30-34` enforces —
   safe-command classification is meant to be driven by the *resolved stack preset*, not
   by one language's build tool. The module's own doc comment
   (`permission_prompt.rs:35-41`) notes the poll loop supersedes this coarse
   classification anyway, so the leak buys nothing.

2. **A hard-coded gate vocabulary.** `GATE_TAGS` (`src/supervisor/drive.rs:550-557`) fixes
   the correction loop's recognised gate names to git-paw's own five-gate framework —
   `testing`, `regression`, `spec audit`, `doc audit`, `security audit`, `scope`. A
   consumer whose review process has different gates cannot name them, so their
   `feedback-gate` verdicts are not recognised as gate verdicts and never start a
   correction cycle.

Both are small. Both are exactly the class of leak the export-agnosticism principle
exists to prevent, and both are invisible to the audits that scan `assets/` because they
live in compiled runtime code.

## What Changes

- **Remove the Rust-specific command class.** Prompt classification no longer recognises
  a `cargo`-specific permission type. Toolchain verbs, where they matter at all, come from
  the resolved stack preset — the mechanism that already exists for exactly this.
- **Make the gate vocabulary configurable**, defaulting to git-paw's current five-gate
  list so existing behaviour is unchanged. A consumer can name their own gates.
- Neither change touches the danger list, the safe-command whitelist semantics, the
  correction budget, or any safety gate.

**Not breaking:** the gate vocabulary default reproduces the current list exactly. The
removed `Cargo` classification was coarse and superseded; commands previously classified
`Cargo` are classified by the same path as every other command, and auto-approval
decisions continue to be made by the classifier, which is unchanged.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `approval-command-safety`: prompt class identification no longer defines a
  toolchain-specific permission class. The detection mechanism, the `Curl` and `Unknown`
  classes, and the rule that `Unknown` never auto-approves are unchanged.
- `supervisor-correction-loop`: the set of gate names the correction loop recognises is
  configurable, defaulting to the current list.

## Impact

- **Code:** `src/supervisor/permission_prompt.rs` — remove the `cargo` arms from
  `classify_command_class` and the `PermissionType::Cargo` variant.
  `src/supervisor/drive.rs:550-557` — `GATE_TAGS` becomes config-sourced.
  `src/config/supervisor.rs` — the new `[supervisor.correction]` field.
- **Enum-variant ripple — `PermissionType`, small but real.** Removing the `Cargo`
  variant touches `src/supervisor/poll.rs` (the `inspect` / `forward_question` signatures
  and its test fixtures) and `src/commands/supervisor.rs:224`. This enum is not on the
  AGENTS.md ripple watchlist (`BrokerMessage`, `SpecBackendKind`), but grep
  `PermissionType::Cargo` across `src/` and `tests/` before finishing so the removal is
  scoped rather than discovered mid-build.
- **Backward compatibility:** the gate list field uses `#[serde(default)]` and
  `skip_serializing_if`, defaulting to the current five gates plus `scope`; existing
  configs load unchanged and behave identically.
- **Docs:** the configuration reference gains the gate-vocabulary field; the supervisor
  guide's correction-loop section notes the gate names are configurable; any doc
  enumerating permission types drops `Cargo`.
