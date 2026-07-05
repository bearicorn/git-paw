## 1. Template completeness

- [x] 1.1 Extend `generate_default_config` (`src/config.rs`) to emit commented example stanzas for `[mcp]`, `[layout]`, `[broker.watcher]`, `[supervisor.auto_approve]`, `[supervisor.learnings_config]`, and `[governance]` — all commented, no default change (also added `[dashboard.broker_log]`: documented in the reference, so required by the "complete schema" / parity SHALL)

## 2. Tests

- [x] 2.1 Test the generated template contains a commented example for each of the six added sections
- [x] 2.2 Parity test: every config section documented in `docs/src/configuration` has a commented stanza in the init template (guards against future drift)
- [x] 2.3 Confirm an untouched generated config still loads with unchanged effective defaults (backward compatible)

## 3. Docs

- [x] 3.1 Cross-check that the configuration reference and the init template list the same sections (both list the identical 18 section families; the parity test in 2.2 now enforces this automatically, so no reference edit was needed)
