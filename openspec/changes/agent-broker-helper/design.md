## Context

git-paw launches each coding agent into a tmux pane and injects a boot
block (`assets/boot-block-template.md`) at the top of its prompt. That
block instructs the agent to talk to the in-process HTTP broker by
running raw `curl -s -X POST {{GIT_PAW_BROKER_URL}}/publish …` commands
in ~4 places (register/status, done/artifact, blocked, question). For
those curls not to trip a permission prompt, the launch path seeds an
allowlist into the agent CLI's settings file
(`.claude/settings.json::allowed_bash_prefixes`, plus any configured
`[clis.<name>].settings_path`). Today that seed is a set of per-endpoint
`curl -s <broker-url><endpoint>` prefixes (see
`src/supervisor/curl_allowlist.rs::broker_prefixes`).

Two problems compound here:

1. **Over-broad / fragile grant.** Prefix-matching `curl -s http://…`
   authorises the agent to hit *that host* with any path/verb shape it
   can phrase to start with the prefix, and broadens further whenever a
   variant slips in (a missing `-s`, a different flag order, a header
   before the URL). The pragmatic "just allow curl" workaround that
   teams reach for when seeding misses is strictly worse: `curl *` lets
   the agent reach **any** URL on the network.

2. **Dead-stall failure mode.** When the seed misses the exact shape the
   agent emits (URL normalisation, flag order, CLI-specific matching
   rules), the very first boot action — the register curl — raises a
   permission prompt the agent cannot answer, and the agent
   **dead-stalls before it ever registers with the broker**. The v0.7.0
   dogfood hit exactly this: all three coding agents froze ~37 minutes
   on the boot publish prompt and never appeared in the dashboard.

The supervisor side already solved the analogous problem. `git paw init`
bundles a single helper script — `assets/scripts/sweep.sh`, installed at
`<repo>/.git-paw/scripts/sweep.sh` via `include_str!` + a 0o755 write —
and the supervisor invokes it by its stable relative path. Allowlisting
one fixed path is both least-privilege (it authorises *that script*, not
*all of curl*) and robust (one literal string to match, no URL/flag
shape drift). This change mirrors that pattern on the agent side.

## Goals / Non-Goals

**Goals:**

- A bundled agent-side broker helper, `assets/scripts/broker.sh`, that
  wraps every agent→broker `curl` the agent is allowed to make. It
  discovers the broker URL at runtime (same `.git-paw/config.toml`
  `[broker]` discovery `sweep.sh` already uses) and shapes the JSON, so
  callers pass simple positional arguments.
- `git paw init` installs `broker.sh` at `<repo>/.git-paw/scripts/`
  (mode 0o755), exactly as it installs `sweep.sh`.
- The boot block calls `broker.sh <subcommand> …` instead of inlining
  raw `curl`, removing broker-URL knowledge and JSON shaping from the
  boot prose.
- The agent-CLI allowlist seeds the **single, stable script path**
  (least privilege) instead of per-endpoint `curl` prefixes (or a
  `curl *` fallback), removing both the over-broad grant and the
  dead-stall.

**Non-Goals:**

- A `git paw publish` (or any new `git paw …`) subcommand. See D2.
- Changing the broker wire protocol, endpoints, or message shapes — the
  helper emits the same payloads the raw curls do today.
- Removing the supervisor's `sweep.sh` or merging the two scripts. They
  serve different roles (supervisor sweep vs. agent self-report) and stay
  separate files.
- Removing the agent's read polling of `/messages/{{BRANCH_ID}}` from the
  coordination guidance — that stays, but the helper provides a `poll`
  subcommand so the polling curl is also covered by the path grant.

## Decisions

### D1. Command surface: publish (status/artifact/blocked/intent) + poll (feedback/inbox)

`broker.sh` exposes exactly the agent→broker interactions the boot block
and coordination guidance need, as named subcommands that take simple
arguments and assemble the JSON internally:

- `broker.sh status <message>` — publish `agent.status`
  (`status:"working"`, the given message, `modified_files:[]`). This is
  the boot REGISTER action (`broker.sh status booting`).
- `broker.sh artifact [--exports a,b] [--files a,b]` — publish
  `agent.artifact { status:"done" }` (the code-less DONE fallback). Same
  JSON shape as the prior raw curl so code-less agents keep an unchanged
  fallback path.
- `broker.sh blocked <needs> <from>` — publish `agent.blocked` with the
  dependency description and source.
- `broker.sh question <text>` — publish `agent.question`.
- `broker.sh intent <summary> <files> [valid_for_seconds]` — publish
  `agent.intent` (forward-coordination: announce files the agent is
  about to touch). Wraps the coordination-skill example curl.
- `broker.sh poll [since]` — read this agent's inbox
  (`GET /messages/<agent-id>?since=<n>`), for peer artifacts and any
  feedback/inbox messages routed to the agent.

The agent's own id is resolved the same way the boot block substitutes
`{{BRANCH_ID}}` — the helper takes it from `--agent <id>` (the boot
block passes the pre-expanded branch id) or falls back to slugifying the
current worktree branch, mirroring `sweep.sh`'s
`resolve_agent_for_path`. Discovery (broker URL from
`.git-paw/config.toml [broker]`, JSON shaping via Python 3) reuses the
exact patterns already proven in `sweep.sh`.

### D2. A script, not a `git paw publish` subcommand

The interaction surface could in principle be a `git paw publish …`
subcommand. We deliberately keep it a script under
`.git-paw/scripts/broker.sh`:

- **Audience clarity.** A `git paw publish` subcommand is part of the
  user-facing CLI; a human will discover it in `--help`, run it, and hit
  confusing errors (no broker running, no session) because it is really
  an agent-internal mechanism. A script under `.git-paw/scripts/` is
  unambiguously agent-internal — it never appears in the human CLI
  surface.
- **Precedent + symmetry.** The supervisor already coordinates through a
  bundled script (`sweep.sh`), not a `git paw sweep` subcommand, for the
  same reason. Mirroring it keeps one mental model for "agent/supervisor
  coordination helpers live in `.git-paw/scripts/`".
- **Allowlist granularity.** A path grant for `.git-paw/scripts/broker.sh`
  is a single, stable literal. A `git paw publish` grant would have to
  allow the `git-paw`/`git paw` binary broadly (covering every
  subcommand, including state-mutating ones) or match fragile argument
  prefixes — the same drift problem we are removing.

### D3. Least-privilege, path-based allowlist seeding

The agent-CLI allowlist seed changes from per-endpoint `curl` prefixes
to the single helper path. Concretely, the seeded `allowed_bash_prefixes`
entry is the invocation prefix the boot block uses, e.g.
`.git-paw/scripts/broker.sh` (both the bare path and any
`bash .git-paw/scripts/broker.sh` form the boot block emits are seeded so
the match is exact). This:

- authorises exactly one script, not a host or all of `curl`;
- is one literal string, so it cannot drift with URL normalisation or
  curl flag order — the dead-stall root cause;
- removes the need for any `curl *` fallback.

Seeding keeps every property of the existing seeder
(`custom-cli-curl-seeding`): config-driven targets (repo-local
`.claude/settings.json` always, plus each `[clis.<name>].settings_path`),
never create a CLI's config directory, idempotent, deduped across
supervisor/agent paths, and non-fatal on write failure.

### D4. Bundling mirrors `sweep.sh` exactly

`broker.sh` is embedded with `include_str!("../assets/scripts/broker.sh")`
and written by `git paw init` to `<repo>/.git-paw/scripts/broker.sh` with
mode 0o755, overwriting any existing file (binary-managed content, same
contract as `sweep.sh` — users with local edits back the file up before
re-running init). It reports `Created`/`Updated .git-paw/scripts/broker.sh`
exactly like the sweep install. A convention test (analogous to
`sweep_sh_conventions`) pins that `broker.sh` avoids the stdin-claiming
`interpreter - <<` heredoc shape, and a content test asserts the boot
block calls `broker.sh` rather than raw `curl`.

## Risks / Trade-offs

- **Two coordination scripts to maintain.** `broker.sh` and `sweep.sh`
  share discovery logic (broker URL, Python detection, JSON shaping) but
  stay separate files. Trade-off accepted: a shared library file would
  add an install/sourcing dependency between the two; duplicating ~40
  lines of discovery is cheaper than that coupling, and matches the
  existing "each helper is self-contained" convention. Mitigated by the
  shared convention test enforcing the same heredoc discipline on both.
- **Helper requires Python 3 on PATH.** Same dependency `sweep.sh`
  already imposes; the dogfood/CI environment already guarantees it. The
  helper exits with a clear diagnostic if Python 3 is absent, rather than
  emitting malformed JSON.
- **Boot block becomes less self-documenting.** Inlined curls showed the
  exact wire shape; calling `broker.sh status booting` hides it. Mitigated
  by `broker.sh --help`/usage text enumerating each subcommand and its
  payload, and by docs describing the helper's surface.
- **Backward compatibility.** A session whose CLI self-registers (or one
  carrying a pre-existing per-endpoint `curl` allowlist) still works —
  the broker endpoints are unchanged and stale `curl` prefixes are
  harmless. The helper is the supported path going forward and the
  allowlist no longer needs the broad `curl` grant.
