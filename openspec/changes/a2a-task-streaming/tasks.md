## 0. Gate

- [ ] 0.1 Get the user's explicit approval to add `futures-util` as a direct dependency, and record it in the AGENTS.md approved list. Do not start section 1 without it. If approval is declined, close this change unimplemented.

## 1. Fan-out

- [ ] 1.1 Add a bounded `tokio::sync::broadcast` of `(task_id, StreamEvent)`. The task-registry fold from `a2a-task-lifecycle` emits on it after each apply. Add `StreamEvent` and its mapping to the A2A `StreamResponse` (`task`, `statusUpdate`, `artifactUpdate`) in `broker::wire::a2a`.
- [ ] 1.2 Implement the subscription. Subscribe first, then take the snapshot, then drop events already reflected in the snapshot (design D2). On `Lagged`, close the stream. Add an SSE keep-alive comment with an internal interval that tests can override.

## 2. Methods

- [ ] 2.1 `SubscribeToTask`: authenticate and check the version as for other methods. A terminal task gets `-32004`, an unknown one gets `-32001`. Otherwise answer `200 text/event-stream`, with each `data:` line a JSON-RPC response carrying a `StreamResponse`, and close after the terminal event. Write tests for "Subscriber sees the snapshot then each update", "Stream closes at completion" and "Subscribing to a terminal task is refused".
- [ ] 2.2 `SendStreamingMessage`: reuse the `SendMessage` validation and delivery, then stream. Write tests for "Guidance then live updates" and "Refused message is not a stream".
- [ ] 2.3 Set the card's `streaming` to `true` while streaming is served, and remove the lifecycle's `-32004` answers for these two methods. Write a test for "Card shows streaming", and update the lifecycle test "Streaming refused while not served" so it covers only a broker built without streaming.

## 3. Fan-out properties

- [ ] 3.1 In `tests/a2a_e2e.rs`, test "Two subscribers see the same order" over real HTTP with two SSE clients.
- [ ] 3.2 Test "Lagging subscriber is closed, not gapped": use a small test buffer capacity and a non-reading client.

## 4. Docs and gates

- [ ] 4.1 In the A2A user chapter (`a2a-migration-guide`), document streaming, the keep-alive comments, the close-on-lag behavior and the re-subscribe pattern. Run `mdbook build docs/`.
- [ ] 4.2 Map every scenario to a named test. Run `just check` and `just deny`; `futures-util` must pass the license, advisory and duplicate checks.
