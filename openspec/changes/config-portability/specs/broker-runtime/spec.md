## MODIFIED Requirements

### Requirement: Broker configuration schema

The system SHALL extend `.git-paw/config.toml` with a new optional `[broker]` section containing these fields:

- `enabled: bool` — defaults to `false` when the field or the entire section is absent
- `port: u16` — when no config file sets it, the per-repository default port applies (see *Per-repository default broker port*); a set value is used verbatim
- `bind: String` — defaults to `"127.0.0.1"` when absent
- `wave_stall_minutes: Option<u64>` — defaults to `30` when absent; `0` disables the `stalled` flag while `idle_seconds` is still reported
- `watcher: WatcherConfig` — the filesystem-watcher sub-table (`[broker.watcher]`), itself all-optional

Loading a `.git-paw/config.toml` that omits the `[broker]` section SHALL succeed and produce a `BrokerConfig` with the documented defaults. Loading one that omits any individual field SHALL succeed and default that field. The system MUST NOT change behavior of existing v0.2.0 sessions when this field is absent.

#### Scenario: Config file with no broker section loads successfully

- **WHEN** a `.git-paw/config.toml` containing no `[broker]` section is parsed
- **THEN** the resulting config exposes a `BrokerConfig` with `enabled = false`, no pinned port, and `bind = "127.0.0.1"`

#### Scenario: Config file with explicit broker enabled section

- **WHEN** a `.git-paw/config.toml` containing `[broker]\nenabled = true\nport = 9200\nbind = "127.0.0.1"` is parsed
- **THEN** the resulting `BrokerConfig` has `enabled = true`, pinned `port = 9200`, `bind = "127.0.0.1"`

#### Scenario: Config file with partial broker section uses defaults for missing fields

- **WHEN** a `.git-paw/config.toml` containing only `[broker]\nenabled = true` is parsed
- **THEN** the resulting `BrokerConfig` has `enabled = true`, no pinned port, `bind = "127.0.0.1"`

#### Scenario: BrokerConfig URL helper produces a well-formed URL

- **WHEN** `BrokerConfig { enabled: true, port: 9200, bind: "127.0.0.1" }` calls `url()`
- **THEN** the result is `"http://127.0.0.1:9200"`

#### Scenario: Every documented field round-trips

- **WHEN** a `.git-paw/config.toml` naming every `[broker]` field, including `wave_stall_minutes` and a `[broker.watcher]` sub-table, is parsed
- **THEN** each field SHALL carry the configured value, and no field SHALL be rejected as unknown

#### Scenario: An explicit 9119 stays 9119

- **GIVEN** a repo config with `[broker] port = 9119`
- **WHEN** the effective broker port is resolved
- **THEN** it SHALL be 9119

## ADDED Requirements

### Requirement: Per-repository default broker port

When no config file sets `[broker] port`, the default port SHALL be `9120 + (FNV-1a 64-bit hash of the canonical path of the repository's main working tree, as UTF-8 bytes) mod 800`. It SHALL be the same from the main working tree and from every agent worktree of that repository, and SHALL NOT change between git-paw releases.

#### Scenario: Two repositories get their own defaults

- **GIVEN** two repositories at different paths, neither setting `[broker] port`
- **WHEN** each resolves its default broker port
- **THEN** each port SHALL lie in 9120–9919 and SHALL be computed from its own main working tree path

#### Scenario: Worktrees share the repository's default

- **GIVEN** a repository without `[broker] port` and one of its agent worktrees
- **WHEN** the default port is resolved from the main working tree and from the worktree
- **THEN** both SHALL be the same port

### Requirement: Start steps past a held derived port

When the port is derived, `git paw start` SHALL probe it: this repository's broker SHALL be reattached; a free port SHALL be bound; a port held by another repository's broker or a foreign service SHALL be skipped for the next port in 9120–9919 (wrapping), at most 8 attempts, then the start SHALL fail naming `[broker] port`. A pinned port SHALL never be skipped.

#### Scenario: A second repository starts while the first is live

- **GIVEN** repository A's live session holds A's derived port, and repository B derives the same port
- **WHEN** `git paw start` runs in B with the broker enabled
- **THEN** B's broker SHALL bind the next free port in the range and the session SHALL start

#### Scenario: A pinned port held by another repository still refuses

- **GIVEN** repository B sets `[broker] port = 9200` and repository A's broker holds 9200
- **WHEN** `git paw start` runs in B
- **THEN** the start SHALL fail naming the port and `[broker] port`, as before

### Requirement: Readers use the session's recorded broker URL

`git paw start` SHALL record the broker URL it bound or reattached in the session receipt. While that session exists, `git paw status`, `git paw add`, the dashboard, `git paw doctor` and the `GIT_PAW_BROKER_URL` injection SHALL use the recorded URL. A receipt with no recorded URL (written by an older version) SHALL mean port 9119 when no config file pins a port.

#### Scenario: Status finds a stepped broker

- **GIVEN** a session whose broker was started on the next port after a held derived port
- **WHEN** `git paw status` runs in that repository
- **THEN** it SHALL query the recorded URL, not the derived port

#### Scenario: A session from an older version keeps working

- **GIVEN** a live session receipt without a recorded broker URL and no `[broker] port` in config
- **WHEN** `git paw status` runs
- **THEN** it SHALL query port 9119

### Requirement: Helpers obtain the effective broker URL

The bundled helpers SHALL discover the broker URL in this order: `GIT_PAW_BROKER_URL` when set; a `[broker] port` (and `bind`) set in the config; the output of `git paw __broker-url`, which prints the effective URL for the current repository; then `http://127.0.0.1:9119`. The helpers SHALL NOT compute the derived port themselves.

#### Scenario: Helper outside tmux asks the binary

- **GIVEN** a repository with no `[broker] port`, no `GIT_PAW_BROKER_URL` in the environment, and `git paw` on PATH
- **WHEN** `broker.sh status "x"` runs
- **THEN** it SHALL publish to the URL printed by `git paw __broker-url`

#### Scenario: Pinned port wins over the binary

- **GIVEN** `[broker] port = 9200` in the config and no `GIT_PAW_BROKER_URL`
- **WHEN** a helper discovers the broker URL
- **THEN** it SHALL use port 9200 without running `git paw __broker-url`
