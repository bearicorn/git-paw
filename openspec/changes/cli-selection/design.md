## Context

The existing `interactive.rs` has a `Prompter` trait with `select_cli()` and `select_cli_for_branch()` methods, plus `CliInfo` and `SelectionResult` types. The `config.rs` has `PawConfig` with `default_cli: Option<String>`. The `spec-scanner` provides `SpecEntry` with `cli: Option<String>`.

The resolution chain needs to combine these three sources (flag, spec, config) into a final branch-to-CLI mapping without prompting when the answer is deterministic.

## Goals / Non-Goals

**Goals:**
- Implement the 5-level priority chain for `--from-specs` CLI resolution
- Add `default_spec_cli` to config as a no-prompt bypass
- Add pre-selection to the interactive CLI picker when `default_cli` is set
- Keep the existing non-spec flow (`git paw start --cli` / interactive) unchanged
- Minimize prompts — only show the picker when resolution is ambiguous

**Non-Goals:**
- Changing the branch picker behavior
- Adding per-branch CLI picker for `--from-specs` (too many prompts for spec-driven flow)
- Validating that the resolved CLI exists on PATH (detection handles that separately)

## Decisions

### Decision 1: `resolve_cli_for_specs()` as standalone function

```rust
pub fn resolve_cli_for_specs(
    specs: &[SpecEntry],
    cli_flag: Option<&str>,
    config: &PawConfig,
    available_clis: &[CliInfo],
    prompter: &dyn Prompter,
) -> Result<Vec<(String, String)>, PawError>
```

Returns `(branch, cli_binary_name)` pairs. The function applies the priority chain:
1. If `cli_flag` is Some → use for all branches, done
2. For each spec: if `spec.cli` is Some → use it for that branch
3. For remaining: if `config.default_spec_cli` is Some → use it
4. For remaining: prompt once with `select_cli()` (pre-selected if `default_cli` set)

**Why:** A single function encapsulates the full chain. Taking `&dyn Prompter` keeps it testable — tests inject a mock prompter.

### Decision 2: Pre-selection via default index in `select_cli()`

Update `Prompter::select_cli()` to accept `default: Option<&str>`. The `DialoguePrompter` implementation finds the matching CLI in the list and passes its index to `dialoguer::Select::default()`.

**Why:** `dialoguer::Select` supports `.default(index)` natively. The user sees the picker with the default highlighted — one Enter to confirm, or arrow keys to change.

**Alternative considered:** Separate `select_cli_with_default()` method. Rejected — adds API surface for what's just an optional parameter.

### Decision 3: Prompt at most once for `--from-specs`

If some specs have `paw_cli` and others don't, and there's no `default_spec_cli`, the picker fires once for the "remaining" group. All remaining branches get the same CLI from that single prompt.

**Why:** Prompting per-branch would be tedious for spec-driven launches with many specs. The user can set `default_spec_cli` to eliminate the prompt entirely.

### Decision 4: `default_spec_cli` is separate from `default_cli`

Two distinct config fields:
- `default_cli` — pre-selects in interactive picker (user confirms), used for `git paw start`
- `default_spec_cli` — bypasses picker entirely, used only for `--from-specs`

**Why:** Different UX expectations. Interactive `start` should let users change their mind. Spec-driven `start` should be non-interactive when possible (e.g., CI, scripts, large batches).

### Decision 5: Resolution validates CLI names exist

After resolution, validate that each resolved CLI name matches an entry in `available_clis`. Return `PawError::CliNotFound` if a `paw_cli` or `default_spec_cli` references an unknown CLI.

**Why:** Fail fast with a clear message rather than failing at tmux launch time with a confusing "command not found".

## Risks / Trade-offs

**[Breaking Prompter trait]** → Adding `default: Option<&str>` to `select_cli()` changes the trait signature. → All implementors (DialoguePrompter, MockPrompter in tests) need updating. This is internal — no public API break.

**[CLI name vs binary name]** → `paw_cli` and `default_spec_cli` use binary names (e.g., `"claude"`, not `"Claude"`). → Document this in config comments. Resolution matches against `CliInfo.binary_name`.
