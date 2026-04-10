## Context

This change is the foundation of v0.3.0's coordination layer. Three Wave 1 changes (`http-broker`, `dashboard-tui`, `skill-templates`) and two Wave 2 changes (`peer-messaging`, `broker-integration`) all depend on the message types defined here. The wire format must be stable before any of those can begin.

The broker exchanges JSON over HTTP between agents (via `curl` from skill templates) and an in-process axum server. Messages are also persisted to a session log for `git paw replay`-style audit. Message shapes therefore need to round-trip cleanly through serde and remain readable when printed.

This change introduces no async, no I/O, no external dependencies — it is pure data definitions and pure functions, deliberately small so Wave 1 can branch from it immediately after merge.

## Goals / Non-Goals

**Goals:**
- Define a stable JSON wire format for the three v0.3.0 message types
- Provide compile-time-checked Rust types that the broker, dashboard, and tests share
- Define a deterministic, total function from git branch name to broker `agent_id`
- Keep the surface small enough that Wave 1 dependents can use it on day one

**Non-Goals:**
- HTTP transport, routing, or server endpoints (owned by `http-broker`)
- Message queuing, delivery, or polling logic (owned by `peer-messaging`)
- Skill template content (owned by `skill-templates`)
- v0.4 message types like `agent.verified` and `agent.feedback` (deferred to v0.4)
- A2A protocol compatibility (deferred to v2.0; current schema is the proto-A2A layer)

## Decisions

### Decision 1: Tagged enum with `serde(tag = "type")` for the message envelope

A single Rust enum `BrokerMessage` with three variants (`Status`, `Artifact`, `Blocked`), serialized using serde's **internally tagged** representation:

```json
{ "type": "agent.status", "agent_id": "feat-x", "payload": { ... } }
```

**Why:**
- Matches the wire format already documented in MILESTONE.md
- One type to pass around the broker, one match expression to handle delivery routing
- serde handles the discriminator automatically; no hand-rolled `Deserialize`
- Adding v0.4 variants (`agent.verified`, `agent.feedback`) is a one-line enum extension

**Alternatives considered:**
- *Three independent structs.* Forces every consumer to write `enum BrokerMessage` themselves or use `serde_json::Value`. Rejected.
- *Adjacently tagged (`tag` + `content`).* Wire format would be `{"type": "...", "content": {...}}` instead of flattening payload. Rejected — diverges from MILESTONE.md and is uglier in `curl` examples.
- *Externally tagged (default).* Wire format would be `{"agent.status": {...}}`. Rejected — agent_id would need to live inside each variant, and the dot in the tag is awkward as a JSON object key.

### Decision 2: `payload` is a struct per variant, not `serde_json::Value`

Each variant carries a strongly-typed payload struct:

```rust
pub enum BrokerMessage {
    Status { agent_id: String, payload: StatusPayload },
    Artifact { agent_id: String, payload: ArtifactPayload },
    Blocked { agent_id: String, payload: BlockedPayload },
}
```

**Why:**
- Compile-time guarantees that consumers (dashboard, delivery logic) handle every field
- Validation can be expressed once in `TryFrom<RawMessage>` rather than scattered field checks
- Better error messages from serde when wire format drifts

**Alternatives considered:**
- *`payload: serde_json::Value`*. Pushes validation to every consumer. Rejected.
- *Flatten payload fields into the enum variant directly.* serde supports this with `#[serde(flatten)]` on a payload field, but the wire format MILESTONE.md documents has an explicit `payload` object. Match the doc.

### Decision 3: Validation lives in `TryFrom`, not a separate `validate()` method

Construction goes through `BrokerMessage::try_from(raw_json: &str)` (or equivalent), which deserializes into a private `RawMessage` and then validates before producing a public `BrokerMessage`. Once you hold a `BrokerMessage`, it is valid by construction.

**Why:**
- Eliminates the "did I remember to call validate?" footgun
- The dashboard never has to handle invalid messages — they were rejected at the broker boundary
- Tests for validation are concentrated in one place

**Alternatives considered:**
- *Public `BrokerMessage::validate(&self) -> Result<()>`.* Simpler but invites bugs where validation is skipped. Rejected.
- *Validation via serde's `deserialize_with`.* Possible but spreads validation across field attributes; harder to test. Rejected.

### Decision 4: `Display` impl produces a one-line dashboard-friendly summary

`impl Display for BrokerMessage` produces strings like:

```
[feat-http-broker] status: working (3 files modified)
[feat-errors] artifact: done — exports: PawError, NotAGitRepo
[feat-config] blocked: needs PawError from feat-errors
```

**Why:**
- The dashboard renders one line per recent message; `Display` is the right Rust idiom for that
- Keeps formatting logic next to the type definition rather than scattered in `dashboard.rs`
- Replay/log output can use the same format for consistency

**Non-goal:** rich/colored output. The dashboard adds colors via ratatui styles, not via the `Display` impl. Plain text only here.

### Decision 5: `slugify_branch` is a free function, total, infallible

```rust
pub fn slugify_branch(branch: &str) -> String
```

Rules (in order):
1. Lowercase via `to_ascii_lowercase()` (only ASCII letters folded; non-ASCII passes through unchanged at this step)
2. Map every character: `[a-z0-9_]` → unchanged, everything else → `-`
3. Collapse runs of `-` to a single `-`
4. Trim leading and trailing `-`
5. If the result is empty, return `"agent"` as a fallback

**Why:**
- Total and infallible — no `Result` type, no error path for callers to handle
- Deterministic — same branch always produces the same `agent_id`
- ASCII-only output — safe in URLs (no encoding needed in `/messages/:id`), safe in shell, safe as a filename
- Handles unicode branch names by replacing non-ASCII characters with `-`, which is lossy but correct (the broker and dashboard never need to round-trip back to the original branch name; the session state file holds that mapping)
- Fallback to `"agent"` for the absurd case of a branch that slugifies to empty (e.g. `///`); avoids panics

**Alternatives considered:**
- *`Result<String, SlugError>`.* Forces callers to handle an error case that has no meaningful recovery. Rejected.
- *Preserve non-ASCII via punycode or unicode normalization.* Pulls in `unicode-normalization` or similar; not worth the dep for a v0.3.0 feature where branch names are overwhelmingly ASCII. Rejected.
- *Hash-based ID (e.g. first 8 chars of sha1).* Stable but unreadable; defeats the dashboard's "I can see which agent did what" UX. Rejected.

### Decision 6: This change creates `src/broker/mod.rs` as a minimal stub

`src/broker/mod.rs` will contain only:

```rust
//! HTTP broker for v0.3.0 agent coordination.

pub mod messages;
```

**Why:**
- Resolves the "who creates the directory" ambiguity between this change and `http-broker`
- Lets dependents `use crate::broker::messages::BrokerMessage` immediately after this change merges
- `http-broker` will extend `mod.rs` later (adding `pub mod server;`, `BrokerState`, runtime spawn, etc.) — additive edits, no merge conflict

**Alternative considered:**
- *Put everything at `src/broker_messages.rs` to sidestep the directory question.* Forces a rename when `http-broker` lands. Rejected.

## Risks / Trade-offs

- **Wire format stability** → Once `message-types` merges and other Wave 1 changes consume it, changing the JSON shape becomes a coordinated breaking change. **Mitigation:** the shape is small, derived directly from MILESTONE.md, and the design intentionally matches what curl-based skill templates need. Treat the schema as frozen for v0.3.0.

- **Lossy slugification of non-ASCII branch names** → A branch named `feat/日本語` slugifies to `feat--` then collapses to `feat-`. Two such branches would collide. **Mitigation:** acceptable for v0.3.0 since git-paw users overwhelmingly use ASCII branch names; document the limitation in the slug function's doc comment and the mdBook configuration chapter. If it becomes a real issue, v0.4 can switch to a unicode-aware slug without changing the function signature.

- **Validation rejects messages the dashboard might still want to display** → If a misconfigured agent posts a malformed `agent.status`, the broker rejects it at the HTTP boundary and the dashboard never sees it. The user gets no visibility into "agent X is publishing junk." **Mitigation:** out of scope for this change; `http-broker` is responsible for logging rejected requests so they show up in `git paw replay`.

- **`Display` is plain text only** → If the dashboard wants to bold the agent_id, it has to parse the string back. **Mitigation:** the dashboard uses the structured `BrokerMessage` directly for styled rendering and only falls back to `Display` for log output where plain text is correct.

## Migration Plan

Not applicable. This change adds new files only; nothing existing changes. Rollback is `git revert`.
