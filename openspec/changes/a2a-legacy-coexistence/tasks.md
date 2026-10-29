## 1. Layer model (seam first)

- [ ] 1.1 In `src/layers.rs`, add `A2a` to `LayerSet`. Route the `__broker` host's A2A serving decision through the resolved layer set instead of reading `[broker.a2a] enabled` directly. This is behavior-preserving relative to `a2a-task-lifecycle`, and the suite must stay green.
- [ ] 1.2 `resolve_layers`: A2A requires Broker and does not imply it. When Broker is absent, print the one-line notice. Add unit rows for "Resolution rows for A2A".
- [ ] 1.3 `SurfacePlan`: the A2A surface is the host routes only. Add a unit test for "A2A adds nothing to worktrees", comparing plans with and without the layer.

## 2. CI matrix

- [ ] 2.1 Add the Manual, Broker and A2A entry to the layer-isolation smoke matrix. It uses an isolated HOME, a private tmux socket, an ephemeral port and a stub CLI. Assert "Matrix entry proves agent-side surfaces are unchanged": the artifacts must equal the Manual and Broker entry, and the card route must answer `200`.

## 3. Contract manifest

- [ ] 3.1 Add the A2A entries to the manifest generator and regenerate the major-1 manifest at 1.2.0. Write manifest guard tests for "A2A entries present in the manifest" and "Dropping the card route fails the guard".
- [ ] 3.2 Write guard tests for "Major-1 manifest keeps the legacy entries" and "Legacy entries are not in the deprecation registry". The registry test asserts that there is no broker endpoint, broker message type or helper subcommand entry.

## 4. Docs

- [ ] 4.1 In `docs/src/stability.md`, add the two-roles paragraph, the not-deprecated statement and the future-removal route. Write the doc test "Stability chapter states the two roles".
- [ ] 4.2 In `docs/src/user-guide/modes.md`, add the A2A layer: its enabling key, the Broker requirement, the host-only surface and how to turn it off. The modes parity test must cover "Modes chapter documents the layer". Run `mdbook build docs/`.

## 5. Gates

- [ ] 5.1 Map every scenario to a named test. Run `just check` and `just deny`, and check the full suite against the merge-base, including the CI matrix job.
