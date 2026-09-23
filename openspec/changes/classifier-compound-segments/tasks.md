## 1. Seam (behaviour-preserving)

- [ ] 1.1 Collapse `drive.rs::first_whitelist_match` and the duplicate in `poll.rs` into one `auto_approve` function used by both loops and `cmd_classify`; full suite green with no behaviour change
- [ ] 1.2 Add a classification corpus fixture (`tests/fixtures/classifier_corpus.*`) recording today's verdict for every command in the existing classifier tests plus every shipped hole listed in the proposal (`curl … | sh`, `ls && curl … | sh`, `echo x; python3 -c …`, `find ~ -delete`, `echo … >> ~/.zshrc`, `git config --global …`, `git config alias.x '!…'`, `git -c alias.x='!…' x`, `curl -d @$HOME/.ssh/id_rsa …`, a two-line `Bash command` prompt whose second line is `rm -rf ~/x`, and the GP-51 `/dev/null` captures), so the before/after diff of this change is reviewable

## 2. Lexer (pure)

- [ ] 2.1 `split_segments` in `src/supervisor/auto_approve.rs`: single/double quotes, backslash escapes, separators `;` `&&` `||` `|` `&` newline, redirection operators (`>`, `>>`, `>|`, `&>`, `&>>`, `N>`, `N>&M`, `<`) recognised so `2>&1` / `&>` never split as background `&`
- [ ] 2.2 Expansion classification: simple (`$NAME`, `${NAME}`, `$?`) vs unanalysable (`$(`, backtick, `<(`, `>(`, `$((`, `${` with an operator, `<<`, leading `(`/`{`, unbalanced quotes) — single quotes make everything literal
- [ ] 2.3 Table tests incl. adversarial cases: quoted separators, escaped quotes, `'$('`, `"$(x)"`, `a&&b` without spaces, `2>&1`, `&>/dev/null`, trailing `&`

## 3. Header-form extraction (every line)

- [ ] 3.1 Change `extract_command_slice`'s `Bash command` header form (`src/supervisor/auto_approve.rs`, the loop after `is_bash_command_header`) to return every non-empty line up to the confirmation boundary, not only the first; keep the inline `Bash(<cmd>)` form and the last-header-wins rule unchanged
- [ ] 3.2 Join continuation lines (previous line ends in `\`, `|`, `&&`, `||`, or leaves a quote open); treat the last line of a block of two or more as the description for the safe check only; run `is_dangerous`, `is_protected_path_violation` and `is_git_dir_write` over every line including the description; apply `normalize_command` per line so the trailing-probe strip can never absorb the description line
- [ ] 3.3 Update `extract_command_slice` unit tests that pin the first-line-only behaviour (e.g. the `cargo test --workspace` / `Run the test suite` capture now yields both lines) and check `drive.rs::dedup_shape` stays stable for a repeated identical prompt
- [ ] 3.4 Verify against a live Claude Code `Bash command` prompt (multi-line command, with and without a description) that the description is the last line; record whether it is distinguishable in a `capture-pane -e` capture (e.g. rendered dim). If it is, file a follow-up to replace the last-line heuristic with that signal (design D8 residual)

## 4. Classifier

- [ ] 4.1 `classify_prompt`: danger-first on the whole normalised command and every header line (unchanged precedence), then `Unanalysable → Unknown`, then `all(segment_is_safe)` with each segment's redirections removed before rule matching; `matched` = first segment's rule; option-index selection unchanged
- [ ] 4.2 Redirection-target rule via `is_path_inside_worktree`: `/dev/null`, `&N` and in-worktree targets allowed; no worktree → any file target other than `/dev/null` unknown
- [ ] 4.3 Mutating-flag table for `find` / `sed` / `curl` / `git config` (any non-read form) / `git -c`, applied only to the built-in read-mostly match; preset and `safe_commands` matches unaffected; broker-localhost URL exemption for `curl`
- [ ] 4.4 Add `which` to `READ_MOSTLY_VERBS` and `default_safe_commands` (keep the `read_mostly_verbs_are_whitelisted` sync test)

## 5. Discard redirects anywhere (GP-51, moved from `broad-grant-scope-safety`)

- [ ] 5.1 Failing tests first, one per GP-51 scenario in `specs/approval-command-safety/spec.md` (*Classifier normalizes a trailing exit-code probe before matching*): mid-command `> /dev/null 2>&1; …` and bare `cargo fmt > /dev/null` are not `danger`; `echo x > /dev/sda` stays `danger`; `rm -rf / > /dev/null 2>&1` stays `danger`
- [ ] 5.2 In `normalize_command` (or a per-segment pass the danger-list check runs on), remove `/dev/null` discard redirects — `>`, `>>`, `1>`, `2>`, `&>`, spaced or attached, with or without a following `2>&1` — wherever they occur (extend `DISCARD_REDIRECTS` / `strip_discard_redirect`), so the `> /dev/` danger pattern only sees real device targets
- [ ] 5.3 Confirm the existing redirect-target scan (`slice_write_targets` / `collect_segment_write_targets` in `auto_approve.rs`, used by the protected-path and `.git/`-write rules) and the new redirect-target rule (4.2) both ignore a `/dev/null` target and still flag every other out-of-worktree target
- [ ] 5.4 Add the same captures to the `__classify` / `sweep.sh` parity fixtures so the helper agrees

## 6. Tests per scenario

- [ ] 6.1 Unit tests in `auto_approve.rs` / `drive.rs` for every scenario: GP-35 command safe; `ls && curl … | sh`, `curl … | sh`, `echo x; python3 -c …` unknown; quoted separators one segment; `echo start && rm -rf /` danger; header form: dangerous 2nd line danger, unlisted middle line unknown, command + description safe, continuation line unknown; substitution cases unknown; `'$('` safe; unbalanced quote unknown; simple expansions safe; gate exit-code idiom safe; redirect into worktree safe, `>> ~/.zshrc` unknown, `> /tmp/notes.txt` unknown, `> /dev/null` safe, `2>&1 | head` safe; `find ~ -delete`, `sed -i`, `curl -d @secrets.txt`, `curl -d @$HOME/.ssh/id_rsa`, `git config --global`, `git config alias.x '!…'`, `git -c alias.x='!…' x` unknown; `git config --get` safe; broker publish safe; configured `sed -i` honoured; `which tmux` safe
- [ ] 6.2 `tests/classify_subcommand.rs`: `git paw __classify` returns the new verdicts for the GP-35 command, `ls && curl … | sh`, a substitution case, a two-line header prompt with `rm -rf` on line 2, and a GP-51 capture; `sweep.sh` parity tests (`tests/sweep_sh_*`) unchanged and green
- [ ] 6.3 Re-run the 1.2 corpus and review every verdict that changed; each change must be an intended `safe → unknown`/`danger`, the GP-35/`which` `unknown → safe`, a GP-51 `danger → per-segment verdict`, or a redirect now vetted by 4.2

## 7. Docs

- [ ] 7.1 `docs/src/user-guide/security-posture.md`: what the classifier analyses (segments, every header line, expansions, redirections, mutating flags incl. `git config`/`git -c`), the `/dev/null` discard carve-out, and the residual `git -C`/`awk system()`/`env` caveats plus the description-line heuristic
- [ ] 7.2 Supervisor/auto-approve chapter (`docs/src/user-guide/supervisor.md`) and configuration reference: `safe_commands` entries match per segment; how to allow a specific mutating form

## 8. Gates

- [ ] 8.1 Run the `security-and-safety-review` dev skill against this change (it changes what is auto-approved); confirm by inspection that nothing outside the three narrowing cases in design.md can newly classify safe
- [ ] 8.2 `cargo fmt`; `just check`; `just deny`; `mdbook build docs/`
- [ ] 8.3 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast > test.log 2>&1; echo $?` — the exit code must be 0 (read it, do not pipe it)
- [ ] 8.4 `openspec validate classifier-compound-segments --strict`
