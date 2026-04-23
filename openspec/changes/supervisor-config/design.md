## Context

This is the first v0.4.0 change — it adds only config, no runtime behavior. The supervisor-agent, supervisor-mode, and auto-start changes will consume these config values. Keeping config separate lets them develop against a stable, tested API without coordination issues (a lesson from v0.3.0 where agents co-owned files).

The existing `PawConfig` already has optional sections (`[broker]`, `[specs]`, `[logging]`) with `serde(default)` patterns. `[supervisor]` follows the same structure.

## Goals / Non-Goals

**Goals:**

- Define the `SupervisorConfig` struct with the three fields needed by v0.4.0
- Provide a permission flag mapping function so the supervisor can construct CLI launch commands
- Update default config generation to include a commented `[supervisor]` example
- Full backward compatibility with v0.3.0 configs

**Non-Goals:**

- Supervisor runtime behavior (owned by `supervisor-agent` and `supervisor-mode`)
- Validation of CLI binary existence (done at launch time, not config time)
- Permission enforcement at the OS level (the flags are passed to the CLI, which enforces them)
- Supporting every possible CLI's permission flags (start with claude and codex, extensible via the mapping)

## Decisions

### Decision 1: `supervisor` field is `Option<SupervisorConfig>` on `PawConfig`

```rust
#[serde(default)]
pub supervisor: Option<SupervisorConfig>,
```

```rust
pub struct SupervisorConfig {
    #[serde(default)]
    pub enabled: bool,
    pub cli: Option<String>,
    pub test_command: Option<String>,
    #[serde(default)]
    pub agent_approval: ApprovalLevel,
}
```

Using `Option` because the **absence** of the section has meaning: "I haven't decided yet — ask me." This enables interactive prompting during `git paw start`.

**Resolution chain for supervisor mode:**
1. `--supervisor` CLI flag → enables for this session (no prompt)
2. `[supervisor] enabled = true` in config → enables by default (no prompt)
3. `[supervisor] enabled = false` in config → disabled (no prompt, explicit opt-out)
4. No `[supervisor]` section at all (`None`) → **prompt the user**: "Start in supervisor mode? (y/n)"

**Why:**
- Three-way state: unconfigured (`None`), enabled (`Some(enabled=true)`), disabled (`Some(enabled=false)`)
- New users discover supervisor mode via the prompt without reading docs
- Experienced users set it in config once and never see the prompt again
- `git paw init` asks "Enable supervisor mode by default?" and writes the section, eliminating future prompts

**Alternatives considered:**
- *Direct struct with `enabled: bool` (like BrokerConfig).* Cannot distinguish "not configured" from "disabled" — users who haven't decided get no prompt. Rejected.

### Decision 2: `agent_approval` is a string enum, not a free-form string

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalLevel {
    /// No permission flags — agents prompt for every action.
    Manual,
    /// Approve repo-scoped file operations automatically.
    #[default]
    Auto,
    /// Skip all permission prompts — agents run unattended.
    FullAuto,
}
```

**Why:**
- Three well-defined levels with clear security implications
- `Default` derive gives `Auto` as the safe middle ground
- Serde `rename_all = "kebab-case"` matches TOML style: `agent_approval = "full-auto"`
- Adding a new level (e.g. `read-only`) is a one-variant change

**Alternatives considered:**
- *Free-form string.* No validation, typos silently ignored. Rejected.
- *Boolean `full_auto: bool`.* Only two levels, not extensible. Rejected.

### Decision 3: Permission flag mapping is a pure function, not config

```rust
pub fn approval_flags(cli: &str, level: &ApprovalLevel) -> &'static str {
    match (cli, level) {
        ("claude", ApprovalLevel::FullAuto) => "--dangerously-skip-permissions",
        ("codex", ApprovalLevel::FullAuto) => "--approval-mode=full-auto",
        ("codex", ApprovalLevel::Auto) => "--approval-mode=auto-edit",
        (_, ApprovalLevel::Manual) => "",
        _ => "",
    }
}
```

**Why:**
- The mapping is git-paw's knowledge of how each CLI handles permissions
- Hardcoded because the CLI flags are stable (they're part of each CLI's public API)
- Pure function — easy to test, no state, no I/O
- The supervisor calls this at agent launch time to construct the pane command

**Alternatives considered:**
- *Config-driven mapping.* Users could define their own `[approval_flags.claude]` table. Overly complex for a small, stable mapping. Rejected — but the function signature allows future extension to read from config if needed.
- *Per-CLI config field.* Each CLI definition could carry its own approval flags. Rejected — mixes CLI detection concern with supervisor concern.

### Decision 4: `cli` field defaults intelligently

When `supervisor.cli` is not set:
1. Use `default_cli` from config if set
2. Otherwise, require explicit `--supervisor-cli` flag or error at launch time

The config struct stores `cli` as `Option<String>`. Resolution happens at runtime in the supervisor-mode change, not during config parsing.

**Why:**
- Config parsing should not depend on CLI detection (which requires PATH scanning)
- The supervisor-mode change already handles CLI resolution for coding agents; it applies the same chain for the supervisor CLI
- Defaulting to `default_cli` is intuitive — "use my preferred CLI for everything unless I say otherwise"

### Decision 5: `test_command` is an optional free-form string

```rust
pub test_command: Option<String>,
```

**Why:**
- Language-agnostic — `"just check"`, `"cargo test"`, `"npm test"`, `"make test"` all work
- `None` = supervisor skips testing (useful for sessions where you just want coordination without verification)
- The supervisor runs this via `std::process::Command::new("sh").args(["-c", &test_command])` so shell features (pipes, &&) work
- No validation at config time — the command is validated when the supervisor actually runs it

## Risks / Trade-offs

- **Hardcoded CLI flag mapping will go stale** → If Claude or Codex change their permission flags, the mapping needs updating. **Mitigation:** the flags are part of each CLI's documented API and rarely change. When they do, it's a one-line fix in `approval_flags()`.

- **`agent_approval = "full-auto"` is a security risk** → Agents can modify any file, run any command without prompting. **Mitigation:** the default is `"auto"` which preserves CLI-default behavior. `"full-auto"` requires explicit opt-in. Document the risk in the config reference and mdBook.

- **`test_command` runs in a shell** → Potential command injection if the config value is attacker-controlled. **Mitigation:** config files are local (`.git-paw/config.toml`), not fetched from remote sources. If someone can edit your config, they can already run arbitrary code.

## Migration Plan

No migration. New optional section with `serde(default)`. Existing v0.3.0 configs load without error and `supervisor` is `None`.
