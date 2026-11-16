## 1. Resolution (D1)

- [ ] 1.1 Hidden `git paw __pane (--agent=<id> | --pane=<n>)`: resolves session and roster as `__send` does; prints `<agent_id> <pane_index>`; exits 3 with `refused: <reason>` for an unknown id, an agent with no live surface, or a pane no roster agent owns; hidden from `--help`
- [ ] 1.2 Share the id→pane (`surface_of`) and pane→id (`surface::resolve_agent`) lookups between `__send` and `__pane`
- [ ] 1.3 Tests: id resolves after a remove/add renumbering; dashboard pane resolves to no agent; `__pane` hidden from help

## 2. sweep.sh verbs (D1)

- [ ] 2.1 A `resolve_target` helper: digits → pane index (and agent via `__pane --pane`), otherwise → `__pane --agent`; a missing `__pane` exits 4 like a missing `__send`
- [ ] 2.2 `approve <agent|pane>`: resolve an id first, then run the unchanged numeric check, roster check, claim, re-confirm and option-1 guard; result lines name agent and pane
- [ ] 2.3 `capture <agent|pane>`: resolve an id; refuse a non-digit, non-id target (exit 2) before any tmux call; numeric index unchanged
- [ ] 2.4 `tell <agent|pane>`: resolve a numeric argument to its agent id, then `__send --agent=<id>`; a pane with no agent prints `refused:` and exits 3
- [ ] 2.5 Tests (`tests/sweep_*`): every existing `approve` scenario unchanged; `approve feat-x` keys pane 2; `approve feat-zzz` exits non-zero with no keys; `capture feat-x`; `capture 0` unchanged; `capture .` refused; `tell 2` delivers to `feat-x`; `tell <dashboard index>` refused exit 3
- [ ] 2.6 Update the usage text (`sweep.sh:488-547`) and the orchestrator skill's helper reference

## 3. Delivery evidence (D2)

- [ ] 3.1 In `deliver`, after `SubmitResult::Delivered`, accept a profile mid-response marker as turn evidence
- [ ] 3.2 Re-read the evidence up to `DELIVERY_EVIDENCE_POLLS = 3` times, one settle delay apart, before `Wedged`
- [ ] 3.3 Tests (fake backend and clock): reflowed echo plus mid-response → delivered; echo on second read → delivered; scrolled-out earlier copy plus mid-response → delivered; emptied input with no evidence on every read → wedged; existing `send.rs` tests unchanged

## 4. status (D3)

- [ ] 4.1 `cmd_status` prints a `not-seen` row (age `-`) for every roster `branch_id` absent from `/status`, after the broker rows; phantoms unchanged
- [ ] 4.2 Tests: roster `feat-a`, `feat-b` with broker `feat-a` → `feat-b not-seen`; no discovery file → broker rows only; existing phantom scenarios unchanged

## 5. Menu glyphs from the profile (D4)

- [ ] 5.1 `permission_prompt.rs`: drop `'❯'` from `strip_decoration`; the option-line readers skip one profile `menu_cursor_glyphs` entry only when an option line follows
- [ ] 5.2 `readiness.rs::find_option_digit` takes the profile: drop `'❯'` and `'>'` from the compiled set; skip a profile glyph only before a digit option
- [ ] 5.3 `sweep.sh`: add `MENU_CURSOR_GLYPHS_REGEX` (escaped literals from `__prompt-profile --field menu_cursor_glyphs`, fallback `❯|›|>`) to `PROFILE_VARS` and `resolve_cli_profile`; drop `❯` from the decoration class at :297, :313, :318; option and footer regexes take `(<glyphs>)?`
- [ ] 5.4 Tests: overridden `»` permission prompt found by the detector; readiness answers a `»` dialog with the right digit; `> /dev/null` keeps its `>`; `sweep.sh approve` on a `»` prompt sends `1` then `Enter`; a source test that `sweep.sh` has no cursor glyph outside the fallback; the default-profile suites unchanged

## 6. Docs

- [ ] 6.1 Orchestrator skill (`assets/agent-skills/orchestrator/`): `approve`, `capture` and `tell` take an agent id or a pane index; `status` shows `not-seen` until a worker's first publish
- [ ] 6.2 User guide helper table, same content

## 7. Gates

- [ ] 7.1 `cargo fmt`, then `just lint` (including `shellcheck` on `sweep.sh`)
- [ ] 7.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 7.3 `mdbook build docs/`
- [ ] 7.4 `openspec validate helper-consistency --strict`
