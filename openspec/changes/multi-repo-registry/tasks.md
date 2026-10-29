## 1. Registry core (pure decisions, no I/O)

- [ ] 1.1 Add `src/registry/mod.rs` (module docs, `pub mod model; pub mod wire; pub mod store;`) and `pub mod registry;` in `src/lib.rs`. Confirm `registry` imports nothing from `mcp`, `rmcp` or `axum`.
- [ ] 1.2 In `registry/model.rs`, add:
  - a `RepoId` newtype that validates the identifier grammar (`[a-z0-9_-]`, at most 64 characters);
  - `RepoId::derive(dir_name, taken)` implementing the slug and the `-2`, `-3` suffix rule. Reuse v0.21's slug value object if it exists, otherwise mirror `broker::messages::slugify_branch`.
- [ ] 1.3 In `registry/model.rs`, add the `Registry` and `RepoEntry` core types and a pure `register(registry, Candidate, &Observed) -> (Registry, RegisterOutcome)`.
  - Inputs are observed facts: the candidate's canonical root, whether its `.git` is a directory, whether `.git-paw/` exists, whether it is excluded, the per-entry liveness of existing entries, and `now`.
  - The function prunes first, then adds, keeps, or removes the candidate.
- [ ] 1.4 Add a pure `revalidate(entry, &Observed) -> Availability` that returns either `Available` or `Unavailable(reason)`. It covers a missing path, a canonical path that no longer equals the recorded one, a lost `.git` directory or `.git-paw/` directory, a configuration that cannot be loaded, and exclusion.
- [ ] 1.5 Write unit tables covering these scenarios:
  - Directory names are slugified
  - Colliding names get a numeric suffix
  - Re-registering keeps the id
  - Linked worktree is not registered
  - Deleted repository is pruned at the next registration
  - Moved repository keeps its id when re-registered
  - Opting out removes the entry
  - Stale entry is unavailable before any write prunes it
  - Swapped symbolic link is refused
  - Unloadable configuration makes the entry unavailable
  - Consumers honour an opt-out before the entry is removed

## 2. Registry wire format and file store

- [ ] 2.1 In `registry/wire.rs`, add `RegistryFileV1` and `RepoEntryV1` with `#[serde(flatten)] extra` maps, plus a mapper to and from the core types.
  - The mapper rejects `version > 1`.
  - It skips entries with an invalid id or a non-absolute path, and reports how many it skipped.
- [ ] 2.2 In `registry/store.rs`, add `registry_path()`, which is `crate::dirs::data_dir()/git-paw/repos.json`, plus `load_in(dir)` and `load()`. A missing file loads as an empty registry. On Unix, a file that is group- or world-writable is refused with an error that names the file and the remedy.
- [ ] 2.3 In `registry/store.rs`, add `update_in(dir, timeout, f)` and `update(f)`:
  - Open `repos.json.lock` with mode `0600`.
  - Poll `std::fs::File::try_lock()` until the bound expires, then return a lock-timeout error that names the lock.
  - Load the registry, refusing a newer or unparseable file without writing.
  - Apply `f`.
  - Write `repos.json.<pid>.tmp` with mode `0600`, then rename it over `repos.json`.
  - Release the lock when the handle drops. Never delete the lock file.
- [ ] 2.4 Add `PawError::RegistryError` (or reuse an existing variant if one fits) with actionable messages.
- [ ] 2.5 Write integration tests in `tests/repo_registry.rs`. Use the HOME/XDG override pattern from `test-session-state-isolation`. Cover:
  - Registry is written to the platform data directory
  - Missing registry reads as empty
  - Unrecognised fields survive a rewrite
  - Symlinked path registers the canonical path once
  - Readers never see a partial document (a writer thread looping while a reader thread parses)
  - Lock held past the bound fails without modification
  - A lock left by a killed process does not block (a child process takes the lock and is killed)
  - Registry is created owner-only
  - Group-writable registry is refused
  - Newer schema version is neither read nor overwritten
  - Unparseable registry is not overwritten
  - Malformed entries are ignored

## 3. Configuration: `[mcp] register`

- [ ] 3.1 Add `register: Option<bool>` to `McpConfig` beside the existing `allow_repo_writes`, with `#[serde(default, skip_serializing_if = "Option::is_none")]`, plus a `PawConfig::registry_excluded()` helper. Leave `allow_repo_writes` and its repository-only merge rule unchanged. Add a commented `# register = true` line to the `[mcp]` stanza of the init template in `config/mod.rs`.
- [ ] 3.2 Write tests in the `config` module for these scenarios (the `allow_repo_writes` scenarios already have tests; keep them passing):
  - Config with [mcp] name parses the field
  - Config with [mcp] register = false parses the field
  - Absent register means the repository is registered
  - Config without [mcp] section loads with defaults
  - MCP config survives round-trip serialization

## 4. `git paw init` registers the repository

- [ ] 4.1 In `run_init`, after the `.gitignore` step:
  - observe the facts (canonical root, `.git` is a directory, exclusion from the merged config, liveness of existing entries);
  - call `registry::store::update` with `registry::model::register`;
  - print `  Registered in the repository registry as <id>` when an entry is added, and `  Removed from the repository registry ([mcp] register = false)` when one is removed;
  - on any registry error, `eprintln!` a warning that names the file and the remedy, and continue.

  A registry no-op must not change the `Already initialized. Nothing to do.` decision.
- [ ] 4.2 Update the `init` `long_about` in `src/cli.rs` to describe registration and the opt-out.
- [ ] 4.3 Write init integration tests (`tests/init_registry_e2e.rs`, using the HOME/XDG override) for these scenarios:
  - Fresh init registers the repository and reports its id
  - Re-running init leaves the entry unchanged
  - Init in a linked worktree does not register it
  - Opted-out repository is removed on init
  - Registry failure does not fail init
- [ ] 4.4 Write a process-level concurrency test for "Two concurrent inits both register": two `git paw init` children run in two repositories against one temporary HOME, and the test asserts both entries are present and the file parses.

## 5. MCP access policy (pure)

- [ ] 5.1 Add `src/mcp/access.rs` containing:
  - `ServeMode { SingleRepo, AllRepos }`;
  - `Access { Read, Write }`;
  - a home descriptor (none, a main working tree with an optional `RepoId`, or a linked worktree);
  - a pure `decide_target(mode, &home, repo_arg, access, &registry_ids) -> Result<Target, Refusal>`.

  `Refusal` variants: repo argument in single-repo mode, no default repository, unknown repository, cross-repository write. `repo_arg` is compared as a string only.
- [ ] 5.2 Add a pure `effective_mode(all_repos_flag, &home) -> (ServeMode, Option<Warning>)`. A linked-worktree home downgrades the mode to `SingleRepo` and returns a warning.
- [ ] 5.3 Write a unit table covering every row of design D8, including "repo equals the home id" for both reads and writes.

## 6. MCP wiring

- [ ] 6.1 In `src/cli.rs`, add `--all-repos` to `Mcp` with a `help` string and update `long_about`:
  - describe the flag;
  - add a Claude Desktop snippet that uses `["paw", "mcp", "--all-repos"]`;
  - describe the access policy and the warning about configurations inherited by agent CLIs;
  - add an example line.

  Thread the flag through `main.rs` dispatch into `mcp::cmd_mcp`.
- [ ] 6.2 In `src/mcp/mod.rs`, resolve an optional home and keep the existing `--repo` error paths. Without `--all-repos`, an unresolvable current directory is still the existing error. Classify the home as a main working tree or a linked worktree, reusing v0.18 `RepoContext::is_main_worktree` when present. Compute the effective mode and log the downgrade warning to stderr.
- [ ] 6.3 Add `GitPawMcpServer::target(&self, repo: Option<&str>, access) -> Result<Cow<'_, RepoContext>, ErrorData>`. It:
  - loads the registry and calls `decide_target`;
  - for a registered target, observes the facts and calls `registry::model::revalidate`;
  - builds `RepoContext::for_root(entry.path)` for that call;
  - validates the target's configuration and returns a per-target error;
  - maps refusals to `ErrorData::invalid_params` with the messages the spec requires.
- [ ] 6.4 Add `repo: Option<String>` (`#[serde(default)]`, documented) to every tool's params struct. Tools that take no parameters today get a shared `RepoArg` struct. Route each tool through `self.target(...)`: read tools with `Access::Read`, the four publish tools and any root-session tool with `Access::Write`.
- [ ] 6.5 Add `src/mcp/query/repos.rs` (the listing: registry entries plus `revalidate`, with exclusion omitted and the list sorted by id) and `src/mcp/tools/repos.rs` (`list_repos`, its response DTO, and `repos_router`).
- [ ] 6.6 In `src/mcp/server.rs`, build the router per mode:
  - In `SingleRepo` mode, do not merge `repos_router`, and strip the `repo` property (and any `required` mention) from every route's `attr.input_schema`.
  - In `AllRepos` mode with no home, remove the publish routes and every root-only route.
  - Resolve `serverInfo.name` from the home configuration, or else from the user configuration.
  - Extend the `instructions` text in `AllRepos` mode.
  - Run `validate_startup_config` only when there is a home.
- [ ] 6.7 Keep `no_stdout_macros_under_src_mcp` green. Every new message goes to stderr or into an MCP error.

## 7. MCP tests

- [ ] 7.1 Update `ALL_ALLOWED_TOOL_NAMES` to stay the single-repo closed set. Add a closed `ALL_REPOS_TOOL_NAMES` (single-repo set plus `list_repos`) and a no-home variant without write tools. Add a registry-inspection test asserting that no single-repo schema declares `repo` and that every all-repos schema except `list_repos` does.
- [ ] 7.2 Write e2e tests in `tests/mcp_e2e.rs` (stdio JSON-RPC, HOME/XDG override, repositories registered via `git paw init`) for these scenarios:
  - --all-repos starts without a home repository
  - --all-repos with an invalid --repo still fails clearly
  - Single-repository mode advertises no repository surface
  - Single-repository mode refuses a supplied repo argument
  - All-repos mode advertises the repository surface
  - A worktree home disables all-repos mode
  - No home repository means no write tools
  - Identity without a home repository defaults to git-paw
- [ ] 7.3 Write e2e tests for these `list_repos` scenarios:
  - Registered repositories are listed with the home flag
  - Unavailable entry carries a reason
  - Excluded repository is omitted
  - Missing registry yields an empty list
  - Newly initialized repository appears without a restart
  - Unreadable registry is reported, not hidden
- [ ] 7.4 Write e2e tests for these targeting scenarios:
  - A read tool targets another registered repository
  - A path-shaped repo value is refused without touching the filesystem
  - Unknown repository id is refused
  - Unavailable repository is refused with its reason
  - Omitting repo without a home repository is refused
  - Path confinement applies relative to the target repository
  - A target with no session degrades gracefully
  - The home repository's own id behaves as omission
- [ ] 7.5 Write e2e tests for these write-policy scenarios. Run them against an in-process broker, following the `publish.rs` test fixtures.
  - Cross-repository publish is refused
  - A home write through the home id is allowed
  - Cross-repository session lifecycle is refused (only if v0.18 `cmd-library-surface` has landed; otherwise record it as covered by the `decide_target` table and add the e2e test when the lifecycle tools exist)
- [ ] 7.6 Write e2e tests for these configuration-isolation scenarios:
  - Malformed target configuration errors only that target
  - Startup ignores a malformed non-home configuration
- [ ] 7.7 Write tests for the modified `mcp-server` requirements:
  - Invocation outside any git repository fails clearly (no `--all-repos`)
  - --help text describes only the supported flags (now including `--all-repos`)

  Confirm the existing *Repository resolution* scenarios pass unchanged.

## 8. Documentation

- [ ] 8.1 In `docs/src/user-guide/mcp.md`:
  - Add a "Serving several repositories" section covering registration through `git paw init` (and re-running it after upgrading), the `--all-repos` client snippet, `list_repos`, the `repo` parameter, the access-policy table, the worktree restriction, the `[mcp] register = false` opt-out, and the warning about configurations inherited by worker CLIs.
  - Update Known limitations to say per-repository entries are needed unless the entry uses `--all-repos`, and that Claude Desktop needs `--repo` or `--all-repos`.
  - Add `list_repos` and `repo` to the tool reference.
- [ ] 8.2 Update `docs/src/cli-reference.md` for `git paw mcp --all-repos` and the registration step of `git paw init`.
- [ ] 8.3 Add `[mcp] register` to `docs/src/configuration/`, and document the registry file location and format, linking from the MCP chapter. Add the registry to the v1.0 frozen wire-format reference page if that page exists.
- [ ] 8.4 Update the README: the MCP feature line and the `git paw mcp` entry in the CLI table.
- [ ] 8.5 Update `docs/src/architecture.md` with the `src/registry/` module and its place in the layering, and the `mcp/access.rs` policy.
- [ ] 8.6 Satisfy the Documentation deliverable scenarios (mdBook chapter exists with per-client walkthroughs, Known limitations are documented prominently, All-repos serving and its access policy are documented) with a docs-content test that asserts stable anchors in `mcp.md`.

## 9. Quality gates

- [ ] 9.1 Run `cargo fmt` and `just check` (fmt, clippy pedantic `--all-targets`, all tests via `cargo test --no-fail-fast`). Read the real exit code, not piped output.
- [ ] 9.2 Run `just deny` and confirm no new dependency appears in `Cargo.lock`.
- [ ] 9.3 Run `mdbook build docs/`.
- [ ] 9.4 Confirm coverage stays at or above 80% for `src/registry/` and `src/mcp/access.rs`.
- [ ] 9.5 Walk the `security-and-safety-review` checklist and record the result in the PR:
  - `repo` is never used as a path;
  - every target is re-validated;
  - cross-repository writes are refused;
  - worktree sessions are confined;
  - registry files are `0600`, and unsafe or newer or corrupt files are refused;
  - no agent CLI is spawned.

## 10. Archive follow-through

- [ ] 10.1 After `openspec archive multi-repo-registry`, replace the placeholder `## Purpose` in `openspec/specs/core-repo-registry/spec.md` with:

  > The machine-wide registry of git-paw-managed repositories, a versioned JSON file in the platform data directory written by `git paw init`. It covers which repositories are eligible, how stable identifiers are derived, how stale entries are pruned and re-validated, how concurrent updates are serialized and replaced atomically, file integrity and permissions, and the per-repository opt-out. Consumers such as the all-repos MCP server resolve repositories only through it.

  Then add the capability to `docs/src/specifications/README.md`, and re-run `tests/spec_purpose_backfilled.rs` and `tests/specifications_page_lists_every_capability.rs`.
