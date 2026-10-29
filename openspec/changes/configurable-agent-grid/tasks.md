## 0. Preconditions

- [ ] 0.1 v0.18 `supervisor-pane-removal` has landed (dashboard at pane 0, no 50/50 top split, `SUPERVISOR_PANE_OFFSET = 1`); rebase both MODIFIED requirements over its versions

## 1. Seam (behaviour-preserving)

- [ ] 1.1 Introduce `GridSettings { max_agents, agents_per_row }` with `Default` = 25/5; thread it through `supervisor_layout`/`layout_for`, `tmux/command.rs` builders and resize pass, and `tmux/layout.rs` rebalance — every caller passes `GridSettings::default()`; full suite green, byte-identical tmux argv

## 2. Config, flag, state

- [ ] 2.1 `src/config/layout.rs`: `max_agents`, `agents_per_row` (`Option<usize>`, `serde(default, skip_serializing_if)`); load-time validation (`0` → `ConfigError` naming the key); round-trip and old-config tests; commented keys in the init default config
- [ ] 2.2 `src/cli.rs`: `start --max-agents <N>` (`value_parser` range ≥ 1, help text); remove "25" from `add` help
- [ ] 2.3 `src/session.rs`: additive `grid: Option<GridSettings>` (`serde(default, skip_serializing_if)`); old session files load; written at launch
- [ ] 2.4 Resolve settings once in `cmd_supervisor`; `add`/`remove`/`recover` read the session's recorded settings; recovery skips the cap check; cap error text names the effective cap and both ways to raise it; tmux "no space" split failure maps to an error naming both values

## 3. Tests per scenario

- [ ] 3.1 Unit (`supervisor/layout.rs`): default table unchanged; `agents_per_row = 3` with 7 agents → rows 3/3/1 and `50/3`%; cap error text for 26 with defaults
- [ ] 3.2 Builder tests (`tmux/command.rs`): row splits and resize pass for non-default columns; no `swap-pane`
- [ ] 3.3 E2E (real tmux): `max_agents = 30` with 28 stub agents launches; `--max-agents 12` overrides config 10; `agents_per_row = 0` load error; `add` refused at the recorded cap 12 despite config 25; recovery of 20 agents with config 10; old session file → defaults on `add`; bare session with `max_agents = 2` and 3 branches launches tiled; re-tile on `add` uses recorded `agents_per_row = 3`

## 4. Docs

- [ ] 4.1 `--help` for `start --max-agents`; configuration reference `[layout]` (both keys, defaults, whole-table overlay note, width arithmetic); init default config comments
- [ ] 4.2 `docs/src/architecture.md` layout section and `docs/src/user-guide/supervisor.md` layout notes drop "25 agents / 5 per row (fixed)"; README if it states the ceiling

## 5. Gates

- [ ] 5.1 `cargo fmt`; `just check`; `just deny`; `mdbook build docs/`
