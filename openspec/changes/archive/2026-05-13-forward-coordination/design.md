## Context

The `BrokerMessage` enum currently has five variants — `Status`, `Artifact`, `Blocked`, `Verified`, `Feedback`. Each is documented in `openspec/specs/broker-messages/spec.md` with a fixed JSON tag (`agent.<name>`), a payload struct, validation rules, and a `Display` format. Delivery rules live in `openspec/specs/message-delivery/spec.md` and split into three patterns:

- **Not routed** (just updates sender record): `Status`
- **Broadcast to all peers**: `Artifact`, `Verified`
- **Targeted single inbox**: `Blocked` (to `payload.from`), `Feedback` (to `agent_id`)

The embedded coordination skill (`assets/agent-skills/coordination.md`) treats coordination as *automatic status* + *opt-in blocked/artifact* + *cherry-pick on incoming artifact*. There is no explicit "I'm about to touch these files" signal.

This design adds a sixth variant, `Intent`, fits it into the existing protocol shape, and rewrites the agent's playbook around it. The supervisor side stays minimal in this change; the policy lives in `conflict-detection`.

## Goals / Non-Goals

**Goals:**
- Add `agent.intent` to the broker protocol with the same shape conventions as the existing variants (internally tagged JSON, validating constructor, `Display` impl, helper methods).
- Pick a delivery pattern that lets *both* the supervisor *and* peer agents see every intent, with no new endpoint or routing key.
- Rewrite the embedded coordination skill so the steady-state agent flow is "publish intent → poll once → edit", not "publish intent → wait for go-ahead". Coordination remains exception handling.
- Pre-empt the most common "I forked the skill" failure mode (user override stuck on v0.4 content) with a release-notes call-out.
- Preserve the v0.4 cherry-pick / blocked / artifact / verified-feedback sections of the skill verbatim — the new content extends, it does not replace.
- Fix the stale `${GIT_PAW_BROKER_URL}` assertion in `agent-skills/spec.md` as a side effect (mismatch #3 from the proposal).

**Non-Goals:**
- The supervisor's overlap-detection algorithm, escalation windows, or `[supervisor.conflict]` config — that's `conflict-detection`.
- Watcher-driven in-flight conflict detection, ownership-violation detection — also `conflict-detection`.
- TTL expiry / sweep on the supervisor side. Intents carry `valid_for_seconds` so a future supervisor can act on it; this change just transports the value.
- Per-CLI tuning of the skill content. v1.0.0 owns per-CLI templates.
- Any change to the `agent_id` overload (sender vs. recipient). The new variant follows the `Status`/`Artifact`/`Blocked` pattern: `agent_id` = the publishing agent.

## Decisions

### D1. Variant name `Intent` and tag `agent.intent`

Matches the existing naming convention (`agent.<noun>`). Considered `agent.plan` (rejected: ambiguous with Spec Kit's `plan.md`) and `agent.claim` (rejected: implies enforcement, which this change deliberately doesn't do — claims are advisory until `conflict-detection` lands).

### D2. Payload field names: `files`, `summary`, `valid_for_seconds`

- `files: Vec<String>` — chosen over `intended_files` (verbose) and `paths` (less specific). Existing payloads use `modified_files` for *post-hoc* file lists; `files` here is the *pre-hoc* claim. Different semantics deserve different names.
- `summary: String` — chosen over `description` (longer-form connotation) and `message` (overloaded with `Verified.message`). One-line plan summary.
- `valid_for_seconds: u64` — chosen over `ttl_seconds` (jargon) and `expires_at: DateTime` (introduces clock-sync concerns and a `chrono` dependency we don't carry). Relative seconds are clock-agnostic; the broker can compute absolute expiry on receipt.

### D3. Delivery: broadcast to all peers (sender excluded)

Three options were considered:

| Option | Behaviour | Pros | Cons |
|---|---|---|---|
| Targeted (supervisor only) | Only `supervisor` inbox receives | Cheapest; matches "supervisor watches" framing | Peer agents can't do related-change coordination without the supervisor as a go-between |
| Broadcast to peers (chosen) | Every other inbox receives | Peers see related changes directly; supervisor still receives (it's a peer) | Slightly more inbox traffic |
| Both (broadcast + extra fanout) | New routing key | — | New routing concept for no benefit |

Chose broadcast. The supervisor is registered as a regular agent in the broker (per `supervisor-launch/spec.md` line 79), so a "broadcast to all peers" routing rule covers both audiences with no special-casing. This matches the `Artifact` and `Verified` delivery shape exactly — the spec delta will copy that requirement structure.

Sender-exclusion: the publisher already knows their own intent; routing it back into their own inbox would be noise and would also cause the publisher to "see overlap with themselves" if the conflict-detection algorithm is naive about sender identity. `Artifact` and `Verified` already exclude the sender; `Intent` follows.

### D4. Validation rules

- `agent_id` — same slug rules as every other variant (delegated to existing validator).
- `files` — non-empty array; every entry non-empty after trim. Globs are *allowed* (the skill discourages but does not forbid them) — validation does not parse globs, just rejects empty strings.
- `summary` — non-empty after trim. No length cap in this change; if dogfood shows abuse, add one in `v040-hardening`.
- `valid_for_seconds` — strictly positive (`> 0`). Zero is meaningless (instant expiry); upper bound is left to the supervisor's TTL handling in `conflict-detection`.

### D5. `Display`, `status_label`, `agent_id` helpers

Format chosen for symmetry with existing variants (single line, no ANSI, payload summary):

```
[feat-auth] intent: 3 files for 900s — wire AuthClient
```

- `[<agent_id>]` — bracketed identifier (matches all other variants).
- `intent:` — short label (matches `status:`, `artifact:`, `blocked:`, `feedback from`).
- `<N> files` — count, not list (lists could be long; the `summary` carries the human-readable hint).
- `for <N>s` — TTL hint, useful for dashboard skim-readability.
- `— <summary>` — em-dash separator (matches `Verified` and `Artifact` Display forms).

`status_label()` returns `"intent"`. `agent_id()` returns the `agent_id` field (matches all other variants).

### D6. Skill rewrite shape

The existing `coordination.md` is preserved end-to-end; new content is **inserted** between the existing "Automatic status publishing" section and "Check for messages from peers" section. Rationale: agents read top-to-bottom, and the new "Before you start editing" / "While you're editing" sections are pre-edit guidance — they belong before the polling section.

New structure (only changed sections shown):

```
1. Title + frontmatter (unchanged except compatibility bump → v0.5.0+)
2. ## Coordination Skills (intro, unchanged)
3. ### Automatic status publishing (unchanged)
4. ### Before you start editing  ← NEW
5. ### While you're editing       ← NEW
6. ### Check for messages from peers (unchanged)
7. ### Report blocked (unchanged)
8. ### Report done with specific exports (unchanged)
9. ### Cherry-pick peer commits (unchanged)
10. ### Messages you may receive (unchanged)
```

The two new sections each include one curl example for `agent.intent` publish. The "Before" section also includes the one-time poll-for-warnings pattern (`?since=0` or `?since=<initial_seq>`).

The skill explicitly states what *not* to do: pairwise check-ins on every change, waiting for explicit go-ahead from peers, blocking on broker silence. This anti-pattern list is part of the v0.4 dogfood lesson and is the heart of why the skill rewrite matters — without it, the skill update is just "publish intent at the start" with the rest of the v0.4 over-coordination habits intact.

### D7. Supervisor skill: minimal touch

`assets/agent-skills/supervisor.md` gains one short section between "Poll session status and messages" and "Publish verification outcome":

```
### Watch peer intents

agent.intent messages arrive in the supervisor inbox alongside agent.artifact and
agent.status. Until conflict-detection lands (next change), there is no automatic
warning logic — but you may inspect intents to spot upcoming overlaps and prompt the
involved agents via agent.feedback.
```

Deliberately advisory. The supervisor in v0.5.0 + `conflict-detection` will get programmatic warning logic; this change just makes sure the supervisor agent's *prompt* mentions the new event type so it doesn't dismiss `agent.intent` as an unknown variant.

### D8. Compatibility frontmatter

Bump the embedded skill's frontmatter `compatibility: git-paw v0.3.0+` → `compatibility: git-paw v0.5.0+`. Older binaries don't know `agent.intent` and would reject it on parse (per the existing "Unknown message type is rejected" requirement). Setting the floor to v0.5.0 makes that visible to a user inspecting the skill.

User overrides under `<config_dir>/git-paw/agent-skills/coordination.md` are *not* touched — the resolution order means a forked override continues to win silently. This is by design but is a footgun: users who haven't merged the upstream rewrite will keep publishing v0.4-shaped behaviour. Mitigated via release-notes call-out, not by spec.

### D9. Release notes call-out

The change adds one bullet to the v0.5.0 release notes (already drafted in `MILESTONE.md` under "Behaviour changes worth knowing about"): the coordination skill is rewritten and user-forked overrides are unchanged. The note points at the upstream version diff so users can re-apply their customisations on top.

## Risks / Trade-offs

- **[Risk] Inflated inbox traffic on large sessions.** If 10 agents each publish 2 intents, every agent sees ~18 messages just for intents. → **Mitigation:** the `Display` format is one line and the polling protocol already supports `?since=<seq>`. Intents are small. If dogfood shows pain, the supervisor (in `conflict-detection`) can collapse repeated intents from the same agent into a "current state" view.
- **[Risk] Globs in `files` confuse overlap detection.** Two intents listing `src/**` overlap by definition; the skill teaches "be specific" but allows globs. → **Mitigation:** validation only rejects empty paths, not globs. The detection algorithm in `conflict-detection` is responsible for handling globs (likely: globs match anything, so any glob-listing intent overlaps any other listing in a covered directory). This is a deliberate handoff.
- **[Risk] Stale intents linger as "noise" until TTL expires.** An agent that publishes intent then crashes leaves a phantom claim. → **Mitigation:** `valid_for_seconds` is on the wire; supervisor TTL sweep is `conflict-detection`'s job. In v0.5.0 without `conflict-detection`, intents simply persist in the message log (no harm, since no one acts on them yet).
- **[Trade-off] Broadcast over targeted.** Picked broadcast for symmetry with `Artifact`/`Verified` and to avoid a second routing concept. The cost is that every peer's inbox carries every intent, even when the peer is in a different module. The `summary` field gives skim-readability.
- **[Trade-off] No CLI surface for publishing intent.** Agents publish via curl per the skill, identical to `agent.blocked`. We considered a `git paw intent <files...>` shortcut, decided against: keeping the surface "skill instructions only" matches the v0.4 pattern and keeps the CLI uncluttered.

## Migration Plan

This change is fully additive on the wire. No data migration. Steps to deploy:

1. Land the `BrokerMessage::Intent` variant + delivery rule + skill rewrite in one commit.
2. Existing v0.4 sessions in flight will not see `agent.intent` (no agent publishes one). Behaviour unchanged.
3. New v0.5.0 sessions: agents that pick up the new embedded skill publish intents; user-forked skills continue to behave as v0.4 until the user merges upstream.
4. No rollback step required — the variant can be reverted by deleting it; user-forked skills are unaffected.

## Open Questions

- **Should `valid_for_seconds = 0` mean "no expiry" instead of being rejected?** Decision: rejected for now (forces explicit TTL); revisit if dogfood shows agents wanting permanent claims (e.g. for long-running refactors). The default in the skill curl example is `900` (15 minutes), matching MILESTONE's "Default 15 minutes" decision.
- **Does the `files` list need to be *paths from repo root* or can it include absolute paths?** Decision: relative paths from repo root, matching `modified_files` convention. The skill curl example shows relative paths only. Validation does not enforce this in v0.5.0 — `conflict-detection` may add a normaliser that strips absolute prefixes pointing into the worktree.
