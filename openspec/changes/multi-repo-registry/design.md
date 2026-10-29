## Context

- **How the server is scoped today.** `src/mcp/mod.rs` resolves one root at startup:
  `resolve_repo(--repo | cwd)` calls `git::validate_repo`, then `RepoContext::for_root`. That
  derives `broker_url` from `session::find_session_for_repo` and `server_name` from
  `[mcp].name`. Every tool reads `self.ctx` (`GitPawMcpServer { ctx, tool_router }`). Path
  confinement (`query::source::read_file`, `get_doc`) is relative to `ctx.root`.
- **The write surface.** There are four worker publish tools, and each derives its `agent_id` from
  `git::current_branch(ctx.root)`. v0.18 `cmd-library-surface` adds root-only lifecycle and
  authority tools, registered only when the resolved repository is the main working tree
  (`.git` is a directory).
- **The tool set is closed.** `server.rs::ALL_ALLOWED_TOOL_NAMES` pins it.
- **Machine-wide state already lives in the platform data directory.** Session receipts are at
  `crate::dirs::data_dir()/git-paw/sessions/` (`session::sessions_dir`) and are written with
  temp-and-rename. The only lock today is `src/lock.rs`: a `create_new` file under `.git-paw/`
  with a manual "remove the stale lock" recovery.
- **How agents reach the server.** Worker CLIs get the server from the user's own MCP client
  config (`assets/boot-block-mcp-template.md`: "already connected"). So a globally configured
  server entry is inherited by every worker running in a worktree.

## Goals / Non-Goals

**Goals:**
- One MCP client entry serves every registered repository. A client that spawns from a fixed
  directory needs no `--repo`.
- The registry is a versioned, concurrency-safe and tamper-resistant wire format from day one.
- Without `--all-repos`, nothing an existing client relies on changes.
- Least privilege throughout:
  - a caller-supplied `repo` can never name an arbitrary path;
  - cross-repo access is read-only;
  - worktree (agent) sessions never leave their worktree.

**Non-Goals:**
- Cross-repository writes, such as starting or stopping a session in another repository or
  publishing into another repository's broker. This is deferred to the v1.4.0 cross-repo
  coordination work; see Open Questions.
- A `git paw repos` management CLI.
- Registration by any command other than `init`.
- Per-repo CLI resolution. That is a separate change, `registry-repo-profiles` (folder-trust
  state is out of scope there too).
- The HTTP MCP transport (unscheduled; the v1.2.0 A2A cycle dropped it).

## Decisions

### D1 — Location and format: `<data_dir>/git-paw/repos.json`, versioned DTO

The registry sits next to `sessions/` under `crate::dirs::data_dir()`:

- macOS: `~/Library/Application Support/git-paw/repos.json`
- Linux: `$XDG_DATA_HOME/git-paw/repos.json` or `~/.local/share/git-paw/repos.json`

The file is machine-generated state, not user-authored config, so it goes in the data
directory, not `config_dir`. MILESTONE's `~/.git-paw/repos.json` matches neither existing
convention.

Wire shape (`registry::wire::RegistryFileV1`, mapped to the core `Registry`):

```json
{ "version": 1,
  "repos": [ { "id": "git-paw", "path": "/Users/me/dev/git-paw", "registered_at": "2027-01-05T10:00:00Z" } ] }
```

Version and unknown-field rules:

- A reader accepts `version <= 1`. A newer version is refused, with no partial read.
- Unknown top-level and per-entry fields are kept through a rewrite
  (`#[serde(flatten)] extra: Map`). That way a newer additive field written by another git-paw
  binary on the same machine is preserved, for example a crates.io install next to a Homebrew
  install.

This follows v0.21's rule of a DTO plus version only at contract edges.

Alternatives considered:
- TOML, for consistency with config. Rejected: this is machine state, readers are code, and
  JSON matches the session receipts.
- One file per repository, which would avoid locking. Rejected: ids must be unique across the
  whole set, and that needs a single serialized writer anyway.

### D2 — Identity: a stable, human-readable `id`; `repo` is matched as an opaque string

The `id` is the slugified repository directory name:

- ASCII-lowercase;
- any character outside `[a-z0-9_-]` becomes `-`;
- runs of `-` collapse and leading and trailing `-` are trimmed;
- the result is capped at 64 characters, and an empty result becomes `repo`.

This is the same rule shape as `broker::messages::slugify_branch`. Use v0.21's unified slug value
object if it has landed.

On a collision the id gets `-2`, `-3`, and so on. An id never changes while its entry exists.

The MCP `repo` argument is compared by exact string equality with entry ids. It is never
canonicalized, joined or otherwise used as a path, so the argument cannot probe the filesystem.

Alternatives considered:
- A path-hash id. Rejected: stable but meaningless to a model or a human.
- Accepting a path as `repo`. Rejected: it reintroduces arbitrary-path input.

### D3 — Writers: `git paw init` only; eligibility is "main working tree with `.git-paw/`"

`run_init` registers after the `.gitignore` step. Registration validity is decided by a pure
function (`registry::model::register`) over observed facts. The candidate root:

- is canonical;
- has a `.git` *directory*, which is the main working tree and the same test v0.18 uses for root
  detection;
- has a `.git-paw/` directory;
- is not excluded by its merged config (`[mcp] register = false`).

Linked worktrees are skipped, so an agent running `init` inside its worktree never registers it.
Exclusion also removes an existing entry for that path.

The step is non-fatal. If it fails (lock timeout, unwritable directory, corrupt or newer or unsafe
file), init prints a warning to stderr naming the file and the fix, and still exits 0. The
registry is consumed only on opt-in, and init's primary job is repo setup.

Alternative considered: registering on `git paw start` too. Rejected for v1.1: it adds a hot-path
writer to the registry, and `init` is already the documented upgrade step after a version bump.
See Open Questions.

### D4 — Stale entries: prune on write, re-validate on use

There are two parts:

- **Prune on write.** Every registry update first drops entries whose path is no longer a
  directory holding both a `.git` directory and a `.git-paw/` directory. This only checks the
  filesystem; no `git` is spawned.
- **Re-validate on use.** Before an entry is served, the server checks that the entry's path
  canonicalizes to itself (no symlink swap), still meets the eligibility test, and is not
  excluded by its config. A config that fails to load also makes the entry unavailable, because
  exclusion cannot be ruled out. Otherwise the entry is unavailable with a reason.

`list_repos` shows unavailable entries with `available: false` and a `reason`, so a client learns
why a call failed.

A moved repository has its old entry pruned the next time `init` runs in it. Pruning runs before
adding, so the repository keeps its id when the directory name is unchanged.

Alternatives considered:
- Never auto-prune and ship an explicit `prune` command. Rejected: it adds CLI surface, and stale
  entries keep holding their ids.
- Prune on read in the server. Rejected: it would make the read-only server a registry writer.

Trade-off: an entry on an unmounted volume is pruned. Re-running `init` there restores it.

### D5 — Concurrency: sidecar lock plus atomic replace; readers are lock-free

A writer:

1. opens or creates `repos.json.lock` with mode `0600`;
2. takes an exclusive `std::fs::File::try_lock()`, polling up to 5 s;
3. reads the registry and applies the pure update;
4. writes `repos.json.<pid>.tmp` with mode `0600` in the same directory;
5. renames it over `repos.json`;
6. drops the lock.

Readers (the MCP server) never lock: the rename guarantees they see the whole old document or the
whole new one.

Two concurrent `init`s serialize, and both entries survive. The OS lock is released when its
process dies, so unlike `src/lock.rs`'s `create_new` file there is no stale-lock recovery step. The
lock file is never deleted, because deleting a locked file races.

`File::lock` and `File::try_lock` have been stable since Rust 1.89 (checked against the local std
source), so no new crate is needed. If v0.21's state-file locking lands a shared helper, use it.

Alternative considered: the `create_new` pattern from `src/lock.rs`. Rejected because a crashed
init would leave a lock that needs a manual `rm`.

### D6 — Integrity and permissions

- The file and the lock are created with mode `0600`.
- On Unix, a registry file that is group- or world-writable is refused by readers and writers,
  which say to `chmod 600` or delete it. Another user who could edit it could otherwise point an
  id at any directory they control.
- A corrupt (unparseable) registry is never overwritten: the writer warns and stops.
- Entries with an invalid id or a non-absolute path are skipped by readers and dropped by writers.

These checks are defence in depth. The real bound is D4's re-validation plus D8: the worst a
tampered entry can do is expose, read-only, a directory that already passes the git-paw
eligibility test.

### D7 — Opt-in serving mode: `git paw mcp --all-repos`

`ServeMode` is decided once at startup.

**SingleRepo** (the default) is today's server:

- the home repository is required, and resolution errors are unchanged;
- no `list_repos`;
- no `repo` property in any input schema;
- a call that supplies `repo` anyway is refused with `invalid_params` naming `--all-repos`. Today
  that argument is silently ignored, which would return the home repository's data to a client
  that meant another repository.

**AllRepos** (`--all-repos`):

- The home repository is optional. An explicit `--repo` must still be valid. A current directory
  outside any repository is not an error. If the home resolves to a linked worktree, the server
  downgrades to SingleRepo and warns on stderr (see D8).
- `list_repos` is registered and every other tool's params gain `repo: Option<String>`.
- The router builder strips the `repo` property from each `Tool.input_schema` in SingleRepo mode.
  rmcp 1.7 exposes `ToolRouter.map` and `ToolRoute.attr`, so this is possible without a second
  set of params structs. The guarantee is behavioral, not byte-identical: the same tools and the
  same properties as before.
- `serverInfo.name` comes from the home repository's config, or from the user config when there is
  no home.
- The instructions text mentions `list_repos` and `repo`.
- With no home, the write tools are not registered, because there is no identity for them to act
  as.

Alternatives considered:
- Always advertising `repo` and `list_repos`. Rejected: it silently widens what every existing root
  session can reach after an upgrade, and it puts a parameter on every schema that a single-repo
  user can never use.
- Auto-falling back to registry mode when no repository resolves. Rejected: it turns a
  frozen-contract error exit into a success.
- Other names: `--registry` (names the mechanism, not the effect) and `--multi-repo` (vaguer).

### D8 — Access policy (pure `mcp::access::decide_target`)

| Server started as | `repo` omitted | `repo` = another registered id (read) | `repo` = another id (write) |
|---|---|---|---|
| SingleRepo, any home | home (today) | refused: `--all-repos` required | refused |
| AllRepos, home is the main working tree | home | that repository, if available | refused: cross-repo write |
| AllRepos, no home | refused: pass `repo` | that repository, if available | write tools not registered |
| AllRepos requested, home is a linked worktree | downgraded to SingleRepo | refused | refused |

`repo` equal to the home repository's own id means "home" for both reads and writes.

`decide_target(mode, home, repo_arg, access, registry) -> Target | Refusal` is pure and
table-tested. The shell then re-validates the chosen entry (D4) and builds its `RepoContext`.

Rationale:
- **Worktree confinement.** A coding agent's server is a worktree session, possibly inherited from
  a global config. Letting it read other repositories would widen its reach beyond its own
  worktree, the boundary `security-and-safety-review` requires preserving.
- **Writes stay home-only.** Prompt-injected content read from repository B, for example a
  README, cannot drive a destructive `stop_session`, `purge_session` or a verdict in repository A
  through this surface.
- **Cross-repo reads are acceptable.** They go to the user's own client, which already has
  filesystem access to those repositories. They are opt-in per client entry (`--all-repos`) and
  opt-out per repository (`[mcp] register = false`).

### D9 — Per-target context and error isolation

A registered target's `RepoContext` is built per call through `RepoContext::for_root(entry.path)`.
Its `broker_url` therefore reflects that repository's current session.

The home context stays built once at startup, exactly as today.

Tools already confine reads to `ctx.root`, so they confine to the target's root unchanged.

`validate_startup_config` still runs for the home repository only. A target whose config is
malformed (for example an invalid `[specs].type`) yields an MCP-level error for calls to that
target; the server keeps running. The registry is re-read on every `list_repos` call and every
`repo`-targeted call, so a newly initialized repository is visible without restarting the client.

### D10 — Module layout (target architecture)

`src/registry/` is in the library crate and has no MCP or rmcp imports:

- `model.rs` (core, pure): `RepoId`, `Registry`, `register` and `prune` decisions over observed
  facts.
- `wire.rs`: the v1 DTO and mapper.
- `store.rs` (adapter): the path, the lock and the atomic write, plus `load_in` / `update_in(dir)`
  variants for tests, following the `session::*_in` precedent.

The MCP side depends on the registry, never the reverse:

- `mcp/access.rs`: the pure policy;
- `mcp/query/repos.rs`: assembles the listing;
- `mcp/tools/repos.rs`: the `list_repos` DTO.

Dependencies point inward (`entry → app → core`). No new port trait is introduced: one file
adapter needs no abstraction, and tests use the `_in(dir)` seam.

## Risks / Trade-offs

- [A worker inherits a global MCP entry with `--repo <main root> --all-repos`] → It resolves as a
  root session and gets cross-repo reads. This is the same property as v0.18 root detection, which
  keys on the resolved repository, not the caller's working directory. Mitigation: docs say never
  to put `--repo <root>` or `--all-repos` in a config that worker CLIs inherit, and the
  linked-worktree downgrade covers the common cwd-derived case.
- [A rogue unsandboxed agent runs `git paw init` in a directory it planted] → The directory gets
  registered and the orchestrator may read it, which is a prompt-injection vector. The marginal
  risk is small: the agent can already plant content in its own worktree. `list_repos` shows the
  path, and the v0.15 sandbox denies writes to the data directory.
- [An unmounted volume's entry is pruned] → Re-running `init` restores it. Another repository may
  take its id in the meantime. That is acceptable because ids are handles, not authority.
- [Repositories initialized before v1.1 are not registered] → Re-run `git paw init`, the
  documented upgrade path. The MCP chapter says so.
- [Building a per-call context costs a session-directory scan and a config load] → This is
  milliseconds for typical N, and a cache would serve stale broker state.
- [Schema stripping depends on the rmcp router internals] → A registry-inspection test asserts
  that no SingleRepo schema declares `repo`, so an rmcp upgrade that breaks this fails loudly.

## Migration Plan

This change is additive. No existing file changes format; the registry is new and starts empty.
Upgrading users re-run `git paw init` per repository, then opt a client entry in with
`--all-repos`. Rollback: remove the flag from the client config; `repos.json` is inert without it
and can be deleted.

## Open Questions

- **Cross-repo writes (decision, gated).** Should root-only lifecycle and authority tools ever act
  on a non-home repository, for example when the target opts in with a config key and the call
  carries `confirm`? Recommendation: no for now. Revisit with v1.4.0 orchestrator-to-orchestrator
  coordination, where cross-repo authority gets a protocol rather than a parameter.
- **Registration from `git paw start`.** Recommendation: no. Revisit if users forget to re-run
  `init`.
- **A `git paw repos [list|prune|remove] [--json]` CLI.** Recommendation: defer until there is
  demand. If it ships, it joins v0.21's uniform `--json` surface.
- **Name of the v1.0 frozen wire-format reference page** that the registry must be added to: to
  verify when v1.0.0 lands.
