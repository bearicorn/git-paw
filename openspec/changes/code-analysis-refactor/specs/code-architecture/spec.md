## ADDED Requirements

### Requirement: Refactors preserve the observable public surface

A behavior-preserving refactor SHALL leave the public CLI surface, the config TOML schema, the
broker wire format, and on-disk session state **byte-identical** before and after. This SHALL be
verified by running the existing behavioral test suite against the merge-base before and after each
refactor wave; a wave SHALL NOT land if any observable output (exit codes, stdout/stderr,
`session.json`, config round-trip bytes, broker envelope bytes, tmux panes) differs.

#### Scenario: Behavioral suite is unchanged after a wave

- **GIVEN** a completed refactor wave
- **WHEN** the existing behavioral suite runs against the merge-base
- **THEN** exit codes, stdout/stderr, `session.json`, and created tmux panes SHALL be unchanged
- **AND** any coverage drop on a real branch SHALL be treated as a cut sole-guard and restored

#### Scenario: Config round-trips byte-identically

- **GIVEN** an existing v0.2.0-onward `.git-paw/config.toml`
- **WHEN** it is loaded and re-serialized after a config-module refactor
- **THEN** the emitted TOML bytes SHALL be identical to the pre-refactor output

#### Scenario: Broker envelope round-trips byte-equivalent

- **GIVEN** a `BrokerMessage` fixture on the frozen wire
- **WHEN** it is deserialized and re-serialized after a refactor
- **THEN** the emitted JSON bytes SHALL be byte-equivalent to the pre-refactor output

### Requirement: Process, tmux, and git invocation is reachable through an injectable seam

Process execution on the orchestration path SHALL depend on an injectable `CommandRunner` seam
rather than calling `std::process::Command` inline in logic. Production SHALL wire a real runner
whose behavior is identical to the pre-refactor inline calls; tests SHALL be able to inject a fake
runner that records the invoked argv and returns scripted output, so tmux/git orchestration is
unit-testable without spawning a real process.

#### Scenario: A tmux-orchestration unit test asserts argv without spawning a process

- **GIVEN** the `CommandRunner` seam and a fake runner injected into a command handler
- **WHEN** a tmux-orchestration unit test runs
- **THEN** it SHALL assert the tmux/git argv the handler would invoke
- **AND** no real tmux or git process SHALL be spawned

#### Scenario: Production behavior is unchanged by the seam

- **GIVEN** the production real `CommandRunner`
- **WHEN** an orchestration command runs end-to-end
- **THEN** the external tmux/git calls SHALL be identical to the pre-refactor inline invocations

### Requirement: Injection-prone strings flow through a single newtype construction point

Injection-prone values — the tmux session name, branch slug, and worktree path — SHALL be
constructed through domain newtypes (`SessionName`, `BranchSlug`, `WorktreePath`) at a single
point rather than assembled by scattered inline `format!` calls. In this behavior-preserving
change the newtype output SHALL be byte-identical to the current inline formatting for every
current input (no sanitization is added here — that hardening ships as a separate versioned
change); the newtype establishes the construction seam.

#### Scenario: Newtype output matches the pre-refactor formatting

- **GIVEN** an input that today produces a session name / branch slug / worktree path via inline `format!`
- **WHEN** the value is constructed through its newtype
- **THEN** the resulting string SHALL be byte-identical to the pre-refactor output

#### Scenario: Construction is centralized

- **GIVEN** the newtype seam
- **WHEN** the codebase is searched for the injection-prone value's construction
- **THEN** it SHALL be produced only through the newtype constructor, not re-assembled inline elsewhere

### Requirement: The codebase is organized into domain modules with preserved re-exports

The codebase SHALL be organized into domain modules — `commands/` (one file per command family with
a thin `main`/`run` dispatch), `config/` (per-section files), `tmux/{command,session,readiness,layout}`,
and the existing `broker/`, `supervisor/`, `specs/`, `mcp/`, `dashboard/` domains. Every module split
SHALL preserve the public re-exports at the pre-split paths (e.g. `crate::config::*`, `crate::tmux::*`)
so the binary crate and the integration-test crates compile unchanged, and SHALL keep the
`BrokerMessage` and `SpecBackendKind` exhaustive `match`es co-located so a missed variant remains a
compile error.

#### Scenario: Re-exports keep external callers compiling

- **GIVEN** a `config.rs → config/` or `tmux.rs → tmux/` split
- **WHEN** the binary crate and the `assert_cmd` integration tests are compiled
- **THEN** all `crate::config::*` / `crate::tmux::*` paths SHALL still resolve and the crates SHALL compile unchanged

#### Scenario: main.rs dispatch behavior is unchanged after the split

- **GIVEN** `main.rs` split into `src/commands/*.rs` with a thin dispatch
- **WHEN** the CLI e2e suite runs
- **THEN** exit codes, stdout, and `session.json` SHALL match the pre-split behavior

#### Scenario: Enum-ripple matches stay compiler-caught

- **GIVEN** a module split that touches a file containing a `BrokerMessage` or `SpecBackendKind` exhaustive `match`
- **WHEN** the crate is compiled
- **THEN** the exhaustive `match`es SHALL remain co-located such that omitting a variant is a compile error, not a silently inert branch

### Requirement: Refactors do not modify the frozen serde, wire, lock, or SIGHUP surfaces

A refactor SHALL NOT modify the frozen do-not-touch surfaces: the silently-breaking serde
representations (`FileIntent` `untagged`, `AdvancedMain` `flatten`, `StatusPayload.message` without
`skip_serializing_if`, `SpecsConfig` `rename="type"`, `Session.created_at` custom serializer, and
`PawConfig::merged_with` default-as-"unset" merge semantics), the broker lock discipline (no lock
held across `.await`; no reordering of lock/`.await`/`spawn`), and the dashboard/SIGHUP `unsafe`
path. The `BrokerMessage` / `SpecBackendKind` variant sets SHALL NOT be changed by a refactor.

#### Scenario: Frozen serde surfaces are byte-identical

- **GIVEN** a refactor diff that touches `broker/messages.rs`, `config.rs`, or `session.rs`
- **WHEN** the diff is reviewed
- **THEN** the untagged/flatten/no-skip/rename/created-at/merge-default surfaces SHALL be byte-identical to before

#### Scenario: Broker lock discipline is preserved

- **GIVEN** a refactor touching `broker/{delivery,mod,watcher}.rs`
- **WHEN** the crate is linted and reviewed
- **THEN** `clippy::await_holding_lock` SHALL stay clean
- **AND** no lock acquisition, `.await`, or `tokio::spawn` SHALL be reordered relative to before

#### Scenario: The SIGHUP unsafe path is untouched

- **GIVEN** a refactor wave
- **WHEN** the `dashboard.rs` / `main.rs` SIGHUP `unsafe` path is diffed
- **THEN** it SHALL be unchanged by this change (its fix lands as a separate branch)

### Requirement: Structural refactors land in test-gated waves

Structural refactors SHALL land in ordered, test-gated waves, and a wave whose test net cannot prove
behavior parity SHALL NOT land until the enabling net exists. Specifically, the `main.rs → commands/`
split SHALL be gated on both the PTY interaction net (`cli-interaction-e2e`) and removal of the
source-grep introspection tests (`test-suite-consolidation`); the tmux runtime-path refactor SHALL be
gated on the PTY net; and the broker structural tidy SHALL be deferred until after the freeze. Each
wave SHALL pass the five-gate verification (testing, regression vs merge-base, spec audit, doc audit,
security), cold-start, before the next wave begins.

#### Scenario: main.rs split is blocked until its net exists

- **GIVEN** the `main.rs → commands/` split is proposed
- **WHEN** either the PTY net or the source-grep-test removal is not yet in place
- **THEN** the split SHALL NOT land until both preconditions are met

#### Scenario: A wave passes five gates before the next begins

- **GIVEN** a completed refactor wave
- **WHEN** the supervisor runs verification
- **THEN** all five gates (testing, regression vs merge-base, spec audit, doc audit, security) SHALL pass cold-start before the next wave starts

#### Scenario: A behavior change is not folded into a refactor wave

- **GIVEN** a latent bug surfaced by the analysis (e.g. an unsanitized session name)
- **WHEN** the refactor waves are executed
- **THEN** the bug fix SHALL ship as its own spec+test-gated change with a reproducing test, not inside a refactor wave
