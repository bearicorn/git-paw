# v0.5.0 release notes — v040-hardening

Release-note bullets for inclusion in the v0.5.0 release entry. The
maintainer cherry-picks these into `CHANGELOG.md` (auto-generated via
`git cliff`) or the GitHub Release body at tag time.

## Skill curl-example corrections

- The embedded `supervisor.md` skill's `agent.verified` and `agent.feedback`
  curl examples have been corrected to match the validated wire format
  defined in `broker-messages`. The previous v0.4.0 examples used the
  wrong payload field names (`target` / `result` / `notes` for verified,
  `target` / `message` for feedback) and would fail validation when
  copied verbatim. The new examples use `verified_by` / `message` and
  `from` / `errors` respectively, and clarify that the top-level
  `agent_id` is the **recipient** (the agent being verified / receiving
  feedback) while the payload-level field is the **sender**.
- **User-forked supervisor skills with the v0.4.0 examples are now
  stale.** Re-merge the upstream `supervisor.md` (or update the curl
  examples by hand) so your supervisor agents publish validated
  messages.

## `agent.question` spec coverage

- The `BrokerMessage::Question` variant (variant, payload, validation,
  `Display`, `status_label`, `agent_id` helpers) and its
  routing-to-`"supervisor"` delivery semantics — both shipped in v0.4.0
  — now have OpenSpec coverage in `broker-messages` and
  `message-delivery`. No functional change; this is documentation
  catching up to shipped behaviour.

## Resolved MILESTONE drift items

- **#12** — supervisor skill ↔ wire-format alignment (resolved by the
  curl-example fixes above).
- **#13** — `agent.question` spec catch-up (resolved by the
  `broker-messages` and `message-delivery` ADDED requirements).

## Internal hardening (no user-visible change)

- Panic-surface scan re-run is clean: the two
  `regex::Regex::new(...).unwrap()` sites in `src/agents.rs` are now
  hoisted into `LazyLock<Regex>` statics, and the
  `worktree_path.to_str().unwrap()` call in `src/git.rs` is replaced
  with `Path::as_os_str()` so non-UTF-8 paths flow through without
  panicking. The documented `expect("broker state lock poisoned")`
  lock-acquisition uses in `src/broker/mod.rs` are unchanged.
