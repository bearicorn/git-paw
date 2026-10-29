## 1. Card builder (pure)

- [ ] 1.1 Add `src/broker/a2a/card.rs` with `build_agent_card(view: &CardView) -> AgentCard`. `CardView` carries the session name, broker URL, git-paw version and whether streaming is served. The function emits the three skills from design D2, the Bearer scheme and its requirement, the coordination extension (`required: false`) and the default modes.
- [ ] 1.2 Write unit tests on the builder:
  - "Interface and capabilities match the served face";
  - "Bearer scheme is declared and required";
  - "Exactly three capability skills";
  - every proto-required field present, with non-empty required arrays;
  - the JSON uses `securityRequirements`, not `security`.

## 2. Route

- [ ] 2.1 Mount `GET /.well-known/agent-card.json` only while the A2A face is served. It needs no authentication; the listener guard from v0.17.0's `broker-listener-origin-guard` still applies. Add the `Cache-Control: max-age=60` header, a strong `ETag` over the serialized card, and a `304` on a matching `If-None-Match`.
- [ ] 2.2 Write router tests for "Card served without credentials", "Card absent while A2A is not served", "Legacy card path is not served", "Conditional request returns 304" and "Cache headers present".
- [ ] 2.3 Write integration tests in `tests/a2a_e2e.rs`:
  - "Card carries no secret, path or agent id": use a live session fixture with a worker and an injected credential, and search the body for each.
  - "Card is unchanged when a worker is added": register a watch target with `POST /watch`, then refetch and compare the body and the ETag.

## 3. Docs and gates

- [ ] 3.1 The A2A user chapter (owned by `a2a-migration-guide`) documents discovery at `<broker_url>/.well-known/agent-card.json` and the three skills. In `docs/src/architecture.md`, add the card route to the broker's route table.
- [ ] 3.2 Record `GET /.well-known/agent-card.json` in the major-1 contract manifest. Run `mdbook build docs/`.
- [ ] 3.3 Map every scenario to a named test. Run `just check` and `just deny`; there is no new dependency.
