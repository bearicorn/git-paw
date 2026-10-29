## 1. Chapters

- [ ] 1.1 Write `docs/src/user-guide/a2a.md`. Cover: enable; loopback-only; credential through `git paw __authority`; the `A2A-Version` header; discovery; the task model and state table; supported and unsupported methods with their errors; blocking and `returnImmediately`; verdicts through the extension; streaming, if it is served. Include worked `ListTasks` and `SendMessage`-verdict JSON-RPC examples. Add the chapter to `SUMMARY.md`.
- [ ] 1.2 Write `docs/src/user-guide/a2a-migration.md`. Include:
  - the mapping table for the five legacy endpoints and all eleven message types;
  - the orchestrator migration (verdict verbs to `SendMessage`, polling to `ListTasks`/`GetTask`/`SubscribeToTask`);
  - an "unchanged for workers" statement;
  - the legacy-support policy;
  - the corrected facts (card path, one card per broker, eight states);
  - a short "why both faces" explanation that cites A2A §3.7 and the absence of an agent-originated publish operation.

  Add it to `SUMMARY.md`.
- [ ] 1.3 Write `docs/src/a2a/coordination/v1/index.md`, the extension specification: the URI, task metadata, `event` values and fields, verdicts, the activation rule, `required: false` and the versioning rule. Add it to `SUMMARY.md`, and check at publish that `…/a2a/coordination/v1/` resolves.
- [ ] 1.4 In `docs/src/user-guide/coordination.md`, add the agent-side-transport statement and a link to `a2a.md`. In `docs/src/user-guide/mcp.md`, add the stdio-only statement and an A2A pointer. Add a one-line README feature entry with a link. Add `broker-a2a` to the specifications index.

## 2. Tests

- [ ] 2.1 Write `tests/a2a_docs_content.rs`, which asserts stable anchor terms only:
  - "Chapter covers the operational facts";
  - "Every legacy message type is mapped", deriving the type list from the `BrokerMessage` serde names so the guard tracks the enum;
  - "Legacy policy and corrected facts are stated";
  - "Extension page defines the schema";
  - "Both chapters link to A2A".
- [ ] 2.2 Run `mdbook build docs/` for "Chapter is reachable and the book builds".

## 3. Gates

- [ ] 3.1 Map every scenario to a named test. Run `just check`. Confirm that the doc layers are consistent: `--help` is unchanged, and README, the mdBook chapters and the configuration reference agree.
