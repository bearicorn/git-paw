## Why

The agent boot block (`assets/boot-block-template.md`) makes each agent run raw `curl -s -X POST {{GIT_PAW_BROKER_URL}}/publish …` in ~4 places. To avoid a permission prompt, the launch path seeds a broad `curl *` allowlist grant — over-broad (authorizes the agent to hit *any* URL) and fragile: when seeding misses, the agent **dead-stalls on the boot publish prompt** and never registers with the broker. The v0.7.0 dogfood hit exactly this — all 3 coding agents froze ~37 min on the boot curl prompt. The supervisor side already has the right pattern: a bundled `sweep.sh` helper installed by `git paw init` and allowlisted by its stable path. The agent side should mirror it.

## What Changes

- **New bundled `assets/scripts/broker.sh`** — an agent-side broker-interaction helper (the analogue of `sweep.sh`) wrapping ALL agent→broker `curl` calls the agent is allowed to make (publish status/artifact/blocked/intent, poll feedback/inbox). It resolves the broker URL and shapes the JSON, so callers pass simple arguments.
- **`git paw init` installs it** at `.git-paw/scripts/broker.sh` (0o755), exactly like `sweep.sh`.
- **Boot block rewritten** to call the helper instead of raw `curl`, removing the broker URL/JSON shaping from the boot prose.
- **Allowlist seeded with the precise script path** (least privilege) instead of `curl *`. Removes the over-broad grant and the dead-stall failure mode.
- Decision (from the finding): keep it a **script**, not a `git paw publish` subcommand — a human might invoke a subcommand and hit errors; a script under `.git-paw/scripts/` is unambiguously agent-internal.

## Capabilities

### New Capabilities
- `agent-broker-helper`: the bundled `broker.sh`, its install via `git paw init`, the agent→broker command surface it wraps, and the least-privilege path-based allowlist seeding.

### Modified Capabilities
- `boot-block-format`: boot block calls `broker.sh` instead of raw `curl`.
- `project-initialization`: `git paw init` installs `broker.sh` (joins `sweep.sh`).
- `curl-allowlist` / `custom-cli-curl-seeding`: seed the precise `broker.sh` path instead of `curl *`.

## Impact

- Affected code: new `assets/scripts/broker.sh` (+ `include_str!` in `src/init.rs`), `assets/boot-block-template.md`, `src/init.rs` (install), the allowlist-seeding path (`src/config.rs` / launch).
- Tests: `sweep_sh_*`-style convention tests for `broker.sh`; boot-block content test asserts it calls the helper not raw curl.
- Docs: boot-block + init docs, helper reference.
- Backward compatible: a session whose CLI self-registers still works; the helper is the supported path and the allowlist no longer needs `curl *`.
