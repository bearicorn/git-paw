## 1. The one missing test

- [ ] 1.1 `tests/emptied_session_refill_e2e.rs::add_on_a_stopped_session_is_refused_naming_start`: start a session, kill its tmux session, run `git paw add b`; assert non-zero exit and `git paw start` in the message (isolated `HOME`/`XDG_*`, `#[serial]`)

## 2. Comments that cite renamed requirements

- [ ] 2.1 `tests/readme_features.rs`: requirement comment → "README Highlights section links each capability family"; scenario comments → the new titles
- [ ] 2.2 `tests/configuration_reference_docs.rs:167` → "Configuration reference documents every specs type value"
- [ ] 2.3 `tests/helpers/source_scan.rs:2`, `tests/source_audit.rs:497` → "Source audits check every production line"
- [ ] 2.4 Tell the owner of `dx-error-cleanups` that its design's citation becomes "Source audits check every production line" once this change archives

## 3. Verify

- [ ] 3.1 Every `Test:` pointer in this change's deltas names an existing function (`rg -n 'fn <name>\('` per pointer)
- [ ] 3.2 `openspec validate spec-hygiene --type change --strict`
- [ ] 3.3 After archive: `openspec validate --specs --strict`; `mdbook build docs/` (the specifications page includes the specs)
- [ ] 3.4 `just check`
