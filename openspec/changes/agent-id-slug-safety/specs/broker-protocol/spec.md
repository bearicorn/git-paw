## MODIFIED Requirements

### Requirement: Branch slug function

The system SHALL provide a free function with the signature `pub fn slugify_branch(branch: &str) -> String` that converts a git branch name into a stable broker `agent_id`.

The function SHALL be total and infallible. The function SHALL apply the following rules in order:

1. Convert ASCII uppercase letters to lowercase
2. Replace every character not in `[a-z0-9]` with `-` (this includes `_`)
3. Collapse runs of consecutive `-` characters to a single `-`
4. Trim leading and trailing `-` characters
5. If the resulting string is empty, return the literal string `"agent-unnamed"`
6. If the resulting string contains no `-` (a single word), return `"agent-"` followed by it

The output SHALL contain only characters from the set `[a-z0-9-]`, SHALL match the `agent_id` pattern the broker's `/publish` handler enforces (`^(supervisor|[a-z0-9][a-z0-9-]*[/-][a-z0-9][a-z0-9-]*)$`), and SHALL never equal the reserved id `supervisor`. The function SHALL be deterministic — calling it twice with the same input always produces the same output.

Every place git-paw derives an agent id from a branch name SHALL call this function. The bundled helpers' shell `slugify` (in `_paw_common.sh`) SHALL return the same string as this function for every input.

#### Scenario: Simple feature branch is slugified

- **WHEN** `slugify_branch("feat/http-broker")` is called
- **THEN** the result is `"feat-http-broker"`

#### Scenario: Uppercase letters are lowercased

- **WHEN** `slugify_branch("Feat/HTTP_Broker")` is called
- **THEN** the result is `"feat-http-broker"`

#### Scenario: Nested branch path is slugified

- **WHEN** `slugify_branch("users/jane/feat/x")` is called
- **THEN** the result is `"users-jane-feat-x"`

#### Scenario: Underscores become dashes

- **WHEN** `slugify_branch("feat/my_feature")` is called
- **THEN** the result is `"feat-my-feature"`

#### Scenario: A task branch naming an underscored file yields a valid id

- **WHEN** `slugify_branch("task/t001-create-room_setup-md")` is called
- **THEN** the result is `"task-t001-create-room-setup-md"`
- **AND** the broker's `/publish` handler SHALL accept it as an `agent_id`

#### Scenario: Runs of separators are collapsed

- **WHEN** `slugify_branch("feat//x")` is called
- **THEN** the result is `"feat-x"`

#### Scenario: Leading and trailing separators are trimmed

- **WHEN** `slugify_branch("/feat/x/")` is called
- **THEN** the result is `"feat-x"`

#### Scenario: Non-ASCII characters are replaced

- **WHEN** `slugify_branch("feat/日本語")` is called
- **THEN** the result is `"agent-feat"`
- **AND** the result contains only ASCII characters from the slug set

#### Scenario: A single-word branch gets the agent prefix

- **WHEN** `slugify_branch("hotfix")` is called
- **THEN** the result is `"agent-hotfix"`

#### Scenario: No branch derives the reserved supervisor id

- **WHEN** `slugify_branch("supervisor")` is called
- **THEN** the result is `"agent-supervisor"`

#### Scenario: Empty input falls back to default

- **WHEN** `slugify_branch("")` is called
- **THEN** the result is `"agent-unnamed"`

#### Scenario: All-separator input falls back to default

- **WHEN** `slugify_branch("///")` is called
- **THEN** the result is `"agent-unnamed"`

#### Scenario: Slug function is deterministic

- **WHEN** `slugify_branch("feat/http-broker")` is called twice
- **THEN** both calls return the same string

#### Scenario: Every output passes the publish validator

- **GIVEN** a corpus of branch names covering every ASCII printable character in every position, underscores, single words, empty and all-separator input, non-ASCII input, and the branch shapes git-paw itself builds (`<branch_prefix><spec id>`, `task/…`, `phase/…`, `plan/…`)
- **WHEN** `slugify_branch` is applied to each
- **THEN** every result SHALL be accepted by the same predicate the `/publish` handler uses to validate `agent_id`
- **AND** no result SHALL equal `supervisor` or contain `_`

#### Scenario: The shell helper slugifies identically

- **GIVEN** the same corpus
- **WHEN** each name is passed to the `slugify` function sourced from the bundled `_paw_common.sh`
- **THEN** its output SHALL equal `slugify_branch` of the same name, byte for byte

## ADDED Requirements

### Requirement: Agent ids derived for one session are distinct

Every launch path that derives agent ids for a session's branches — bare `start`, spec-driven `start`, supervised `start`, and `git paw add` — SHALL compute each branch's id with `slugify_branch` before creating any worktree or pane, and SHALL refuse the launch when two different branches of the session derive the same id. For `add`, the new branch's id SHALL be compared with the ids of the agents already recorded in the live session. The refusal SHALL name both branches and the shared id, and SHALL create no worktree, pane or session record.

#### Scenario: Two branches that differ only by underscore are refused

- **WHEN** `git paw start --branches feat/a_b,feat/a-b` is run
- **THEN** it SHALL fail naming `feat/a_b`, `feat/a-b` and the id `feat-a-b`
- **AND** no worktree SHALL be created

#### Scenario: Adding a branch that collides with a live agent is refused

- **GIVEN** a live session with an agent on `feat/x`
- **WHEN** `git paw add feat-x` is run
- **THEN** it SHALL fail naming `feat/x`, `feat-x` and the id `feat-x`
- **AND** the session SHALL be unchanged

#### Scenario: Distinct ids launch as before

- **WHEN** `git paw start --branches feat/a,feat/b` is run
- **THEN** no collision SHALL be reported

### Requirement: No publishing agent id is derived in the main working tree

A publish path that derives the publishing `agent_id` from the checked-out branch SHALL NOT derive one in the repository's main working tree, decided as git reports it: the checkout's git directory (`git rev-parse --git-dir`) equals the common git directory (`git rev-parse --git-common-dir`), which also holds under `--separate-git-dir` — the check `git paw setup-cli` already makes (`RepoLayout::is_linked_worktree`). There, the bundled `broker.sh` run without `--agent` SHALL exit non-zero with a diagnostic on stderr that names the main working tree and the `--agent <id>` remedy, and SHALL publish nothing; with an explicit `--agent <id>` it SHALL publish as that id, as before. The MCP worker publish tools follow the same rule (`mcp-agent-publish`, "Publish tools act as the calling agent only"). In a linked worktree the derivation is unchanged. Without this, a publish from the main working tree on a single-word branch (`main` → `agent-main`) would land as a phantom agent instead of being refused.

#### Scenario: broker.sh in the main working tree refuses without --agent

- **GIVEN** a repository's main working tree on branch `main` and a running broker
- **WHEN** `.git-paw/scripts/broker.sh status booting` is run there without `--agent`
- **THEN** it SHALL exit non-zero with a diagnostic on stderr naming `--agent`
- **AND** no message SHALL be published and the broker's `/status` SHALL NOT list `agent-main`

#### Scenario: An explicit agent id still publishes from the main working tree

- **GIVEN** a repository's main working tree and a running broker
- **WHEN** `.git-paw/scripts/broker.sh --agent supervisor question "which order?"` is run there
- **THEN** the broker SHALL receive an `agent.question` from `supervisor`

#### Scenario: A linked worktree derives its id as before

- **GIVEN** a linked worktree on branch `feat/x` and a running broker
- **WHEN** `.git-paw/scripts/broker.sh status booting` is run there without `--agent`
- **THEN** the published message's `agent_id` SHALL be `feat-x`
