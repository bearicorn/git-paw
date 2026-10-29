## ADDED Requirements

### Requirement: Branch-derived names come from one value object

Every name git-paw derives from a branch SHALL be produced by one domain value object, `BranchName`, through a method named for its purpose: the broker agent id (`agent_id()`), the child-layout worktree directory slug (`worktree_slug()`), and the session log file stem (`log_file_stem()`, with the inverse `from_log_file_stem()`). Each derivation SHALL be byte-identical to the rule it replaces — the five-step agent-id rule of `slugify_branch`, the `BranchSlug` rule, and the reversible `/` → `--` log-stem rule respectively — and the rules SHALL NOT be merged into one output. The existing free functions (`slugify_branch`, `sanitize_branch_for_filename`, `unsanitize_branch_from_filename`, `git::branch_slug`) SHALL delegate to the value object, and no other code SHALL re-implement any of the three rules.

#### Scenario: Each derivation matches the rule it replaces

- **GIVEN** the branch `feat/Auth#2`
- **WHEN** its derivations are computed
- **THEN** `agent_id()` SHALL be `feat-auth-2`, `worktree_slug()` SHALL be `feat-Auth2`, and `log_file_stem()` SHALL be `feat--Auth#2`
- **AND** `from_log_file_stem("feat--Auth#2")` SHALL return `feat/Auth#2`

#### Scenario: The existing rule tables pass against the value object

- **GIVEN** the existing unit tables for `slugify_branch`, `branch_slug`, and the log-name sanitizer
- **WHEN** they are run against both the value object's methods and the delegating free functions
- **THEN** every row SHALL produce the same output through both

### Requirement: Domain identifiers are typed at port and use-case boundaries

Functions at the terminal backend port, the session store, the command library entry points, and the broker client publish identity SHALL take session names, agent ids, and worktree paths as the domain types `SessionName`, `AgentId`, and `WorktreePath` rather than as raw strings or paths. Raw strings SHALL remain only inside the tmux adapter's command building and at deserialization edges, where a session name read back from a session file SHALL be wrapped with a constructor that accepts the recorded value verbatim. A standing source guard SHALL fail if a public function outside the tmux adapter declares a `session_name: &str` parameter. Observable behavior, paths, and bytes SHALL be unchanged.

#### Scenario: No public signature outside the adapter takes a raw session name

- **WHEN** the standing guard scans public function signatures outside `src/tmux/` and the tmux terminal adapter
- **THEN** none SHALL declare a `session_name: &str` parameter

#### Scenario: A recorded pre-hardening session name still recovers

- **GIVEN** a session file whose recorded `session_name` was produced before project-name sanitisation
- **WHEN** the session is loaded and recovered
- **THEN** the recorded name SHALL be used verbatim as the session name
