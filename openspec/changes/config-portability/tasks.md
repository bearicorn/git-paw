## 0. Preconditions

- [ ] 0.1 Confirm `config-schema-versioning` (wire/model split, D4) has landed; `BrokerConfig.port` becomes optional in the internal model only
- [ ] 0.2 Confirm `uniform-json-output` has landed (the new doctor rows ride in the existing `--json` document)

## 1. Configured CLIs (DF-03)

- [ ] 1.1 `src/detect.rs`: return unresolved `[clis.*]` entries as data next to the resolved list; move the stderr line to the picker / `start` callers; keep `custom_cli_excluded_when_binary_missing` green
- [ ] 1.2 `src/doctor.rs`: probe every configured CLI id (`default_cli`, `default_spec_cli`, `[supervisor].cli`, presets, worker profiles, `[clis.*]`) with the key and file that set it; pure `check_configured_clis` (✓ path / ⚠ remedy); unit tests per row state
- [ ] 1.3 Launch rule: picker drops an absent default with one notice; `default_spec_cli` / preset / profile tiers refuse before any worktree is created; `--cli` pass-through unchanged; resolver unit tests
- [ ] 1.4 `tests/doctor_e2e.rs`: committed `default_cli` naming an absent CLI → ⚠ row with remedy and nothing printed to stderr above the report

## 2. Relative command (PK-20)

- [ ] 2.1 Resolve a `/`-containing relative `command` against the main working tree in detection and in `CliProfile`'s launch path (`src/agent_cli/profile.rs`); bare names unchanged
- [ ] 2.2 Tests: wrapper at `scripts/agent-wrap` resolves from a subdirectory and from an agent worktree to the main working tree's copy; bare name still uses `PATH`

## 3. Per-repository broker port (DV-01)

- [ ] 3.1 Check whether the session receipt already records the broker URL; add `broker_url` (`#[serde(default)]`) if not
- [ ] 3.2 `src/config/broker.rs`: internal `port: Option<u16>`, `effective_port(repo_root)`; FNV-1a 64 of the canonical main working tree path → 9120–9919; golden test pinning the hash for a fixed path
- [ ] 3.3 Start: probe derived port, step to the next on another repo's broker / foreign service (max 8, wrapping), reattach on own broker; pinned port keeps today's refusal; record `broker_url`
- [ ] 3.4 Switch readers (`status`, `add`, dashboard, doctor `probe_broker`, `GIT_PAW_BROKER_URL` injection, root skill render at `src/init.rs:261`) to the recorded URL, else `effective_port`; receipt without `broker_url` → 9119
- [ ] 3.5 Hidden `git paw __broker-url`; `_paw_common.sh` `discover_broker_url` order: env → pinned config → `__broker-url` → 9119; helper parity test
- [ ] 3.6 Doctor Broker row names the port and its source (pinned / derived / session); template comment at `src/config/mod.rs:1242`
- [ ] 3.7 e2e: two repositories with no `[broker] port` run live sessions at once; a pinned port held by another repo still refuses

## 4. Spec Kit directory (PK-02)

- [ ] 4.1 `SpecKitBackend::default_dir` → `specs`; legacy fallback to `.specify/specs` when only it exists; `COMMENTED_SPECS_TEMPLATE` comment; update `init::tests::specs_section_for_maps_each_index_to_its_backend_section`
- [ ] 4.2 Tests: speckit with no `dir` scans `specs/001-x`; with only `.specify/specs/001-x` scans the legacy dir; explicit `dir` verbatim

## 5. Docs

- [ ] 5.1 Configuration reference (`[broker] port` default, relative `command`, Spec Kit `dir`); `docs/src/user-guide/doctor.md`; broker docs; CHANGELOG (default port no longer 9119 unless pinned)

## 6. Gates

- [ ] 6.1 `cargo fmt`; `just check`; `just deny`; `mdbook build docs/`; `openspec validate config-portability --strict`
