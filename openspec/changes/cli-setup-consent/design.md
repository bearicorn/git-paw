## Context

- **Origin.** This is the v0.19.0 `cli-setup-consent` change (branch `feat/v0.19.0-specs`), pulled
  forward into v0.17.0 by the user's decision of 2026-10-19 and cut down to what v0.17.0 can carry.
  The v0.19.0 copy is dropped once this lands. "Changes from the v0.19.0 source" below lists every
  difference.
- **History.** v0.11 seeded `allowed_bash_prefixes` into `.claude/settings.json` files. Claude Code
  reads `permissions.allow`, so none of it ever worked (GP-01, commit d4270ded). v0.16
  `remove-inert-permission-seeding` deleted the writers and said a real `permissions.allow` writer
  deserves its own proposal. v0.17 `retire-stale-requirements` removed the stale seeding
  requirements. What is left is `approval-command-safety` *No permission grants are seeded into agent
  CLI settings files*, which this change rewrites.
- **v0.17.0 state.** `classifier-retirement` (parallel, archives before this change) deletes the
  command classifier, the managed-helper rule and the auto-approve paths. After it, git-paw approves
  nothing: a helper call runs without a prompt only if the agent CLI's own permission mode or allow
  rules permit it. There is no provider record (`cli-provider-record` is v0.19.0), no `family` key,
  no helper registry (v0.20.0), no `.git-paw/skills/` writer and no orchestrator skill (v0.18.0).
  The helper scripts are the four `init::bundled_scripts()` entries; `init::provision_worktree_helpers`
  writes `broker.sh` (and `docs-fetch.sh` when configured) into each worktree's `.git-paw/scripts/`,
  overwriting them on every attach; `bundled-helper-discovery` extends that to every launch path.
  Agents call the helpers by relative path (`.git-paw/scripts/broker.sh --agent <id> …`). git-paw's
  `auto` approval level launches Claude Code with `--permission-mode acceptEdits`.
- **Installed CLI (read-only probe, 2026-10-20).** `claude --version` → `2.1.303 (Claude Code)`.
  `claude --help` lists `--permission-mode` choices `acceptEdits, auto, bypassPermissions, manual,
  dontAsk, plan`; `-p/--print` with `--permission-prompts none` ("anything that would prompt is
  denied automatically; the permission mode still decides everything else"); `--setting-sources`
  (`user, project, local`); `--settings <file-or-json>`; the allow-rule example `"Bash(git *)"`; and
  notes that `-p` silently ignores settings files that fail validation. These make the fact gate
  scriptable without an interactive session.
- **Documentation facts (code.claude.com, fetched 2026-10-20; to be confirmed by the gate):**
  - Settings: "If you start Claude Code in a subdirectory of a git repository, it reads and writes
    that file at the repository root … In a worktree, it uses the file at the main checkout's root."
    (`.claude/settings.local.json`; "Yes, and don't ask again" approvals are saved there too.)
  - Rule forms: "The `:*` suffix is an equivalent way to write a trailing wildcard, so `Bash(ls:*)`
    matches the same commands as `Bash(ls *)`"; "the permission dialog writes the space-separated
    form".
  - Compound commands: separators `&&`, `||`, `;`, `|`, `|&`, `&` and newlines are recognised and "a
    rule must match each subcommand independently"; deny and ask rules apply when any subcommand
    matches, "including a command nested inside a subshell, a command substitution, or a control-flow
    body".
  - Precedence: a deny rule blocks even when a narrower allow matches; "a matching ask rule prompts
    even when a more specific allow rule also matches".
  - Path anchoring for Read/Edit rules: `//path` absolute, `~/path` home, `/path` relative to the
    settings source, `path` / `./path` relative to the current directory; gitignore semantics.
  - `acceptEdits` "auto-approves common filesystem Bash commands: `mkdir`, `touch`, `rm`, `rmdir`,
    `mv`, `cp`, and `sed` … only to paths inside your working directory or `additionalDirectories`".
    The docs are **silent** on whether those commands are checked against Edit deny rules, and on
    whether an `Edit(…)` rule covers the Write tool.
  - Protected paths (never auto-approved except in bypass mode) include `.git` and `.claude`.
  - Settings files are watched and reloaded, including `permissions`.

## Goals / Non-Goals

**Goals:** a consented, previewable, undoable step that pre-approves git-paw's own helper calls for
Claude Code; least privilege (fixed helper paths and subcommands, nothing from arguments, config or
repository content); no grant without protection against the agent rewriting what it runs; no fact
that has not been checked against the installed CLI; nothing an agent can trigger on its own;
per-CLI behaviour as data so v0.20.0/v0.23.0 can generalise it.

**Non-Goals:** any automatic application; trust or hooks facts for any CLI; provider families or a
`family` key; a helper registry or moving helpers (v0.20.0); free-form entries or paths; a
`--json` plan; an MCP tool; writing into a worktree, a user-level file or
`[clis.<name>].settings_path`; editing or removing permission entries git-paw did not add (doctor
only reports them).

## Decisions

**D1 — Command shape.** `git paw setup-cli <CLI> [--allowlist] [--trust] [--hooks] [--dry-run]
[--yes] [--undo]`, named like `add-cli` / `remove-cli`. A clap group requires one item unless
`--undo` is given (clap's usage error exits 2). `--undo` with no item undoes every recorded item for
that CLI. `<CLI>` is a CLI identity: a built-in roster name or a `[clis.<name>]` key, keyed exactly as
`cli-launch-identity` keys per-CLI resolution (the identity, never the launch command). An identity
that is neither fails with the existing `CliNotFound` (exit 1). `--trust` and `--hooks` are kept so
the flag surface does not change when v0.23.0 adds facts; today they always answer "not supported".
*Rejected:* `cli-setup` (breaks the `*-cli` naming); a separate undo command; "no item = every
supported item" (grants more than the user named).

**D2 — Consent gate and exit codes.** The plan is always printed first. On a TTY without `--yes`, a
`dialoguer::Confirm` defaulting to No. Off a TTY without `--yes`: nothing written, the message names
`--yes` and `--dry-run`, exit 2. Declining also exits 2. An unsupported item, or writing an
unverified item, exits 2 and writes nothing for any item (all-or-nothing). `--dry-run` wins over
`--yes`. Undo goes through the same gate. Exit 2 needs a new `PawError::NotAttempted(String)`
because `UserCancelled` prints "Cancelled.", which v0.24.0 calls a defect for the no-terminal case;
`core-error-handling` *Exit codes distinguish cancellation from errors* is MODIFIED accordingly.
Refusals (linked worktree, unsafe target, concurrent change) are attempted-and-failed: exit 1.

**D3 — Facts are a compiled table, inlined for v0.17.0.** `cli-provider-record` does not exist in
v0.17.0, so the facts live in `src/cli_setup/facts.rs` as one `const` row:

| Fact (claude, allowlist) | Value | Source | Status |
|---|---|---|---|
| Target file | `<main checkout root>/.claude/settings.local.json` | docs (settings page, worktree note) | gate F1 |
| Key paths | `permissions.allow`, `permissions.ask`, `permissions.deny` | d4270ded; docs | gate F2/F4/F5 |
| Allow form | `Bash(<relative helper path> *)` / `Bash(<path> <subcommand> *)` | docs (space form = dialog form) | gate F2 |
| Bare invocation matched by the `*` form | unknown | docs silent | gate F2a/F2c |
| Env-prefix / interpreter / `./` forms not matched | expected | docs silent | gate F2d (blocking) |
| Compound and substitution segments matched independently | expected | docs | gate F3 (blocking) |
| Ask rule prompts over a matching allow, in manual and acceptEdits | expected | docs | gate F4 (blocking) |
| `Edit(**/…)` cwd-relative any-depth deny blocks Edit and Write tools | expected | docs partly silent | gate F5 (blocking) |
| That deny also blocks acceptEdits' auto-approved filesystem commands | unknown | docs silent | gate F6 (blocking) |
| Trust, hooks; any other CLI | not encoded | — | v0.23.0 audit |

The row carries `verified: bool` and `verified_with: &str` (CLI version and date), set by task 1 and
printed in the plan. `verified = false` makes the item plan-only (`--dry-run` works, writing exits 2),
which is the "ships `--dry-run`-only plus docs" outcome the GP-01 lesson demands when the facts are not
confirmed. Tests use constructed rows for both states, so the machinery is fully tested either way.
The fact table stays keyed by CLI identity: no `cli-provider-record` facet replaces it and no `family`
key is added. A variant identity such as `claude-oss` answers "not supported" — but the target file is
the repository's, not the variant's, so `git paw setup-cli claude` still covers sessions of a Claude
Code variant too (optional gate step F1d checks this for the dogfood's variant binary).

**D4 — One target: the main checkout's project-local settings file.** The docs say a session started
in a subdirectory or in a worktree uses `.claude/settings.local.json` at the main checkout's root, so
that single file pre-approves the helpers for the root session and for every worker in a nested
child worktree (`<repo>/.git-paw/worktrees/<branch>`), and keeps doing so for later sessions and for
worktrees created after setup. That last point is the CLI's own resolution, not git-paw extending
anything: git-paw writes nothing at launch. Consequences, all designed in: the file is user-owned and
may already hold hundreds of entries (often written by "don't ask again", including by 0.16.0's
classifier), so the write is a merge that keeps every unrelated key and its order (D7) and the plan
never prints existing content; the grant reaches every session in the repository, which is why the
grant table is minimal (D5) and the protection is repository-wide (D6). *Rejected:* per-worktree files
(the v0.19.0 source's "setup directories"; under the documented resolution they are not read, and they
would sit inside the agent's own reach); the user-level file and `settings_path` (relative helper
paths would then be pre-approved in every repository, including a cloned one that ships its own
`.git-paw/scripts/broker.sh`); `--settings <git-paw file>` passed at launch (keeps the user's file
untouched, but every launch would then apply grants, contradicting decision 2). If gate F1 shows a
nested-worktree session does not read the main checkout's file, the row ships unverified and the
finding is recorded in "Observed facts" below for a later release to design the per-worktree target
afresh.

**D5 — The grant table (least privilege).** Helper names come from `init::bundled_scripts()` (the
single embedded-script source; v0.20.0's registry replaces it). Each name has a row:

| Helper | Allowed | Never allowed | Why |
|---|---|---|---|
| `broker.sh` | `Bash(.git-paw/scripts/broker.sh *)` | — | every subcommand is a bounded publish/poll to the configured broker; `--agent` precedes the subcommand, so per-subcommand rules would need mid-pattern wildcards |
| `docs-fetch.sh` | `… find *` | `get` | `find` fetches only `<docs_base_url>/llms.txt`; `get` passes absolute `http(s)://` and `file://` URLs through, a read-anywhere and exfiltration primitive |
| `sweep.sh` | `snapshot`, `capture`, `status`, `worktrees-status`, `inbox`, `detect-stuck`, `stuck-eval`, `status-publish`, `learn` (each `… <sub> *`) | `approve` (and an `ask` rule), `verified`, `feedback-gate`, `classify` | `approve` presses keys in another pane; `verified` / `feedback-gate` publish supervisor verdicts, and the root file reaches worker sessions too, so a worker could otherwise forge its own verification without a prompt; `classify` is removed by `classifier-retirement` |
| `_paw_common.sh` | — | — | sourced, never invoked |

`Bash(.git-paw/scripts/sweep.sh approve *)` goes in `permissions.ask`: per the docs an ask rule wins
over any matching allow (including a broad `sweep.sh *` grant a user may already have from 0.16.0), so
`approve` always reaches a human. Guard tests: every bundled script has a row; every subcommand in
`sweep.sh`'s `main` dispatch is in the allowed or never-allowed column (so a new keystroke verb cannot
slip in); every planned entry starts `Bash(.git-paw/scripts/<bundled name>` and contains no `curl`,
`bash `, leading `/` or `Bash(*`. If gate F2a/F2c shows the `*` form does not match a bare
invocation, the row also lists exact forms for subcommands the bundled skills call bare
(`sweep.sh snapshot`, `status`, `inbox`, `detect-stuck`, `worktrees-status`). No entry comes from an
argument, config or repository content (export-agnosticism: nothing from the consumer's stack).

**D6 — Tamper mitigation (blocking).** The threat: a granted helper lives in the agent's own worktree
(`<worktree>/.git-paw/scripts/`, and it sources `_paw_common.sh` next to it); in `acceptEdits` the
CLI applies the agent's edits there without a prompt, after which the rewritten helper runs through
the grant — arbitrary code without a prompt. A second, quieter path: the helpers read
`<worktree>/.git-paw/config.toml` (broker `bind`/`port`, `docs_base_url`), so an edited address turns
a granted `broker.sh question "<repo content>"` into exfiltration. Options evaluated:

| Option | Effect | Cost | Verdict |
|---|---|---|---|
| (a) deny rules for the file-editing tools on `**/.git-paw/scripts/**`, in the same file | closes the zero-prompt Edit/Write path for every session (cwd-relative, any depth: a worker's own copy, the root copy, a root session editing a worker's copy, a planted `sub/.git-paw/scripts/`) | nothing beyond the write; deny wins over allow, so it also overrides a user's own broader allow | **chosen** |
| (a') ask rule for edits of `**/.git-paw/config.toml` | the configuration the helpers read never changes silently | an occasional prompt when the user's own session edits config | **chosen** |
| (b) allow only the main checkout's absolute helper paths, invoked by absolute path | the granted file is outside every worker's working directory, so `acceptEdits` prompts before editing it | every helper invocation in the boot block and the coordination, supervisor and docs-fetch skills changes (and the `boot-block` and `broker-agent-helper` requirements and tests that pin `.git-paw/scripts/…`); workers would run the root copy written by `git paw init` instead of the binary-matched copy `bundled-helper-discovery` provisions; the root session still needs (a) | rejected for v0.17.0; v0.20.0 `skill-package-deploy` instead moves the helpers to `.git-paw/skills/<pkg>/scripts/` with an `Edit(**/.git-paw/skills/**)` deny rule there, re-running the fact gate against the moved paths |
| (c) content check | the CLI cannot hash a script before running it; a content-checking hook is the `hooks` item, whose facts are unverified | — | detection only: doctor warns when a granted helper differs from the bundled copy (D11), and every `start`/`add` already rewrites worktree copies |
| Bash-pattern denies (`Bash(sed * .git-paw/scripts/*)` …) | — | bypassed by `cd .git-paw/scripts && sed …`, quoting, absolute paths; a pattern containing the helper path would also deny invoking it | rejected |

The unknown that decides it is whether the deny rule also governs the filesystem commands
`acceptEdits` auto-approves (`sed -i`, `cp`, `mv`, `rm`, `touch`, `mkdir`), including via `cd …&&`.
Gate F6 tests exactly that. If any of them modifies a helper without a prompt, (a) is insufficient, the
row ships unverified (plan-only), and the failing fact-gate steps are recorded in "Observed facts"
below for later releases. Residuals accepted in the verified case are listed under the threat analysis.

**D7 — Write discipline.** Parse the JSON; a malformed file, a non-object root, or a container of the
wrong type on a key path stops the whole run naming the file and key, before anything is written.
Missing containers are created; added items follow existing ones; an entry already present is
reported and **not recorded**, so undo can never take back an entry the user (or Claude's own "don't
ask again") added. Each write is a temp file in the same directory plus rename; existing files keep
their mode, new files are `0600`. Before the rename the file is re-read and the run stops if it
changed since the plan. Refused: a target tracked by git (`git ls-files --error-unmatch`), a symlink
anywhere between the repository root (or the common git dir) and the file (closes a planted
`.claude -> ~/.claude`), and any path that resolves outside them. Key order: unrelated keys keep their
order, which needs serde_json's `preserve_order` feature (`indexmap` 2.14.0 is already in
`Cargo.lock`; no new crate). The v0.19.0 source records the user's approval of this feature
(2026-10-19); **confirm it for v0.17.0**. The feature changes the order of every `serde_json::Value`
map in the crate from sorted to insertion order (struct serialization and `learnings::canonical_value`,
which sorts explicitly, are unaffected); task 2.1 runs the full suite and reviews every golden that
changes. *Alternative if the feature is declined:* a ~100-line private order-preserving JSON value
(`Vec<(String, Value)>` with its own `Deserialize`/`Serialize`) used only by `cli_setup`. Output is
normalised to two-space pretty-printing, the format Claude Code writes, documented.
*Rejected:* textual splicing (fragile); accepting key reordering (the user's diff would churn on keys
git-paw never touched).

**D8 — The record lives in the git directory.** `<git-common-dir>/git-paw/cli-setup.json`, mode
`0600`, atomic. `schema_version` plus entries `{cli, item, file, kind (allow | ask | deny |
exclude-line), key_path | line, value, created_file, created_dirs, recorded_at, git_paw_version}`. An
entry is recorded **before** it is written: a crash in between leaves a recorded entry not on disk,
which undo drops; the reverse order would leave an unrecorded entry undo could never find. The git dir
can never be committed, is invisible to every worktree's status, and is shared by all worktrees.
*Alternative:* `.git-paw/cli-setup.json` (needs an init gitignore entry, could be committed with
absolute paths).

**D9 — Undo is a pure plan over the record.** An entry is removed only when it is present, byte-equal
to its recorded value, and shape-valid: in the row's declared target file, an allow/ask value
`Bash(.git-paw/scripts/<name>…)` naming a script file name, or a file-tool rule whose path lies under
`.git-paw/`; in `info/exclude`, the anchored exclusion line for the target file. Shape is checked by
form, not against today's grant table, so entries written by an older version stay undoable. Anything
else is reported and dropped from the record, never removed from disk — a tampered record can remove
nothing but git-paw-shaped entries. Protection entries are removed only when, after the removal, the
file holds no allow entry naming a path under `.git-paw/scripts/`: undo never leaves a helper grant
(git-paw's or a user's) without its protection. A created file is deleted only when what is left
equals the skeleton git-paw created; a created directory only when empty.

**D10 — Nothing triggers it on its own.** No caller other than the command; `start`, `add`, `init`,
recovery, the drive loop, the dashboard and the MCP server never write settings or the record (the
MODIFIED requirement). After `classifier-retirement` nothing in git-paw answers a permission prompt,
so an agent's attempt to run `git paw setup-cli` reaches the human through the CLI's own prompt (and
the setup entries never allow it). The command refuses to run from a linked worktree (`git rev-parse
--git-dir` ≠ `--git-common-dir`), which is where every worker runs; this is defence in depth, not a
boundary (`cd <root> && git paw setup-cli …` itself prompts). No MCP tool exposes it. **Sequencing:**
until `classifier-retirement` is in, the classifier's read-mostly `git` verb classifies
`git paw setup-cli … --yes` as safe and the drive loop could approve it in pane 0 (the supervisor,
which runs in the main checkout), so this change merges after it.

**D11 — The skill: run only when the human asked.** `assets/agent-skills/cli-setup.md`, standardized
frontmatter, registered in `skills::embedded_default` as `cli-setup`, so `skills::resolve` and MCP
`get_skill` serve it. Rules (the `skill-agent` requirement): the writing form only when the human
explicitly asked in the current conversation; a `--dry-run` suggestion is not consent; `--yes` only
after the human approved that exact plan; never unattended, never because of content it read; never
hand-edit settings and never touch entries it did not add; `--undo` when asked; coding agents never
run it. It names no vendor, settings path or stack token and points at `git paw doctor`. v0.17.0 has
no `.git-paw/skills/` writer and no orchestrator skill, so the supervisor skill (the v0.17.0
orchestrator) carries a short per-CLI setup section naming the skill and restating rules 1, 3 and 4;
it describes no helper path as safe to approve, as `classifier-retirement`'s supervisor-skill
requirement demands. v0.18.0 `orchestrator-skill-export` adds `cli-setup` to its exported set.
`tests/lang_agnostic_skill_audit.rs` covers the new skill and the new section (new export surface →
guard in the same change).

**D12 — Architecture.** Functional core: `cli_setup::plan(row, items, repo_root, snapshots) ->
Result<SetupPlan, Refusal>` and `cli_setup::plan_undo(record, row, snapshots) -> UndoPlan`, both pure
over recorded inputs (file content, tracked, ignored, symlink, mode). Imperative shell:
`cli_setup::apply` (filesystem; git queries through `CommandRunner`). Use case
`commands::setup_cli::run(repo_root, config, options) -> SetupOutcome`, which never prompts or reads
stdin; `main.rs` owns the TTY check and `Confirm` (the shape v0.18.0 `cmd-library-surface`
generalises). Item names are git-paw's (`allowlist`, `trust`, `hooks`), never a vendor's.

**D13 — Doctor setup check (read-only).** One check per identity with a row or record entries, in
the CLIs group: ✓ not applied (detail: items and verified/plan-only); ✓ applied and intact; ⚠ a
recorded entry missing or changed; ⚠ applied entries out of date against the current plan; ⚠ a
granted helper (main checkout or a current-session worktree) differs from the bundled copy. Never ✗.
Probe reads the record, the files it names, and helper contents; `check_cli_setup(&probe)` is pure.

**D14 — Doctor broad-grant advisory (read-only).** 0.16.0's classifier took "Yes, and don't ask again"
for classifier-safe commands and could leave durable rules in the main checkout's
`.claude/settings.local.json` (the docs say worktree approvals are saved there too).
`classifier-retirement` stops new ones but removes none. Doctor lists `permissions.allow` entries that
match a closed list of broad shapes (the `preflight-diagnostics` requirement), abbreviated so an
embedded credential is not echoed, and never edits the file. The list names only generic tools and
git-paw's own subcommands, never a consumer's toolchain verbs (`Bash(cargo:*)` matches as a single-word
grant, not by name). It is a report over stored rules, not a decision about a prompt, so it does not
conflict with `classifier-retirement`'s *git-paw makes no safety decision about an agent's command*;
the requirement says so explicitly. If gate F8 records the rule shape Claude writes for a scope-wide
"don't ask again" (compound commands), that shape is added to the list before code.

## Fact gate procedure (task 1)

Run from a plain terminal **outside any git-paw session** (the script strips `TMUX`/`TMUX_PANE`),
with an authenticated Claude Code on PATH and network access. It costs ~35 short `claude -p` turns.
It uses a throwaway repository under `$TMPDIR`, never the git-paw checkout, and loads only project and
local settings (`--setting-sources project,local`), so the user's own allow rules cannot mask a
result. `--permission-prompts none` turns "would prompt" into an observable denial. Each stub helper
appends `ran <its $0> <args>` to a marker file, so a run proves which copy executed.

```bash
#!/usr/bin/env bash
# cli-setup fact gate (design.md). Run OUTSIDE any git-paw session.
set -u
CLAUDE_BIN=${CLAUDE_BIN:-claude}
"$CLAUDE_BIN" --version
G=$(mktemp -d "${TMPDIR:-/tmp}/paw-cli-setup-gate.XXXXXX")
R="$G/repo"; W="$R/.git-paw/worktrees/feat-x"; M="$G/marker"
git init -q "$R" && git -C "$R" -c user.name=gate -c user.email=gate@example.invalid commit -q --allow-empty -m init
git -C "$R" worktree add -q "$W" -b feat/x

fixtures() {
  rm -rf "$R/sub" "$W/sub" "$G"/pwned* "$R/notes2.txt" "$W/notes2.txt"
  for d in "$R" "$W"; do
    rm -rf "$d/.git-paw/scripts"; mkdir -p "$d/.git-paw/scripts"
    for h in broker.sh sweep.sh docs-fetch.sh; do
      printf '#!/bin/sh\necho "ran $0 $*" >> "%s"\n' "$M" > "$d/.git-paw/scripts/$h"
      chmod 755 "$d/.git-paw/scripts/$h"
    done
    printf '[broker]\nport = 9119\n' > "$d/.git-paw/config.toml"
    printf 'notes\n' > "$d/notes.txt"
  done
}
settings() {  # settings <dir-or-empty> <json>
  rm -rf "$R/.claude" "$W/.claude"
  [ -n "$1" ] || return 0
  mkdir -p "$1/.claude"; printf '%s\n' "$2" > "$1/.claude/settings.local.json"
  python3 -m json.tool "$1/.claude/settings.local.json" >/dev/null || echo '!! INVALID JSON'
}
snap() {  # hash of every file under any .git-paw/ dir, plus notes files
  (cd "$R" && find . -path ./.git -prune -o -type f \( -path '*/.git-paw/*' -o -name 'notes*' \) -print \
     | LC_ALL=C sort | xargs shasum) | shasum | cut -c1-12
}
try() {  # try <label> <cwd> <mode> <instruction>
  local label=$1 cwd=$2 mode=$3 instr=$4; local -a flags
  if [ "$mode" = bypass ]; then flags=(--dangerously-skip-permissions); else flags=(--permission-mode "$mode"); fi
  : > "$M"; local before; before=$(snap)
  ( cd "$cwd" && env -u TMUX -u TMUX_PANE "$CLAUDE_BIN" -p --output-format json \
      --setting-sources project,local --permission-prompts none "${flags[@]}" \
      "This is a permission-rule test in a throwaway repository. $instr If the action is refused, do not try any alternative; just reply DONE." \
  ) > "$G/$label.json" 2>&1
  printf '%-5s %-11s files=%s marker=[%s] denials=%s\n' "$label" "$mode" \
    "$([ "$before" = "$(snap)" ] && echo same || echo CHANGED)" "$(tr '\n' '|' < "$M")" \
    "$(python3 -c 'import json,sys
try: print(len(json.load(open(sys.argv[1])).get("permission_denials") or []))
except Exception: print("unparseable")' "$G/$label.json")"
}
bash1() { echo "Use the Bash tool exactly once to run this exact command, verbatim: $1"; }

ALLOW_BROKER='{"permissions":{"allow":["Bash(.git-paw/scripts/broker.sh *)"]}}'
FULL='{"permissions":{
 "allow":["Bash(.git-paw/scripts/broker.sh *)","Bash(.git-paw/scripts/docs-fetch.sh find *)",
  "Bash(.git-paw/scripts/sweep.sh snapshot *)","Bash(.git-paw/scripts/sweep.sh capture *)",
  "Bash(.git-paw/scripts/sweep.sh status *)","Bash(.git-paw/scripts/sweep.sh worktrees-status *)",
  "Bash(.git-paw/scripts/sweep.sh inbox *)","Bash(.git-paw/scripts/sweep.sh detect-stuck *)",
  "Bash(.git-paw/scripts/sweep.sh stuck-eval *)","Bash(.git-paw/scripts/sweep.sh status-publish *)",
  "Bash(.git-paw/scripts/sweep.sh learn *)"],
 "ask":["Bash(.git-paw/scripts/sweep.sh approve *)","Edit(**/.git-paw/config.toml)"],
 "deny":["Edit(**/.git-paw/scripts/**)"]}}'
B='.git-paw/scripts/broker.sh --agent x status booting'

fixtures
# F1 settings resolution
settings "$R" "$ALLOW_BROKER"; try F1a "$W" manual "$(bash1 "$B")"
settings "$W" "$ALLOW_BROKER"; try F1b "$W" manual "$(bash1 "$B")"
settings ""  "";               try F1c "$W" manual "$(bash1 "$B")"
settings "$R" "$ALLOW_BROKER"; try F1e "$R" manual "$(bash1 "$B")"
# F1d (optional): CLAUDE_BIN=<dogfood variant, e.g. claude-oss> re-run F1a
# F2 rule forms (root file, worker cwd)
try F2a "$W" manual "$(bash1 '.git-paw/scripts/broker.sh')"
settings "$R" '{"permissions":{"allow":["Bash(.git-paw/scripts/broker.sh:*)"]}}'
try F2b1 "$W" manual "$(bash1 "$B")"; try F2b2 "$W" manual "$(bash1 '.git-paw/scripts/broker.sh')"
settings "$R" '{"permissions":{"allow":["Bash(.git-paw/scripts/sweep.sh snapshot *)"]}}'
try F2c1 "$W" manual "$(bash1 '.git-paw/scripts/sweep.sh snapshot')"
try F2c2 "$W" manual "$(bash1 '.git-paw/scripts/sweep.sh snapshot --x')"
try F2c3 "$W" manual "$(bash1 '.git-paw/scripts/sweep.sh approve 1')"
try F2c4 "$W" manual "$(bash1 '.git-paw/scripts/sweep.sh "snapshot"')"
try F2c5 "$W" manual "$(bash1 '.git-paw/scripts/sweep.sh snapshotx')"
settings "$R" "$ALLOW_BROKER"
try F2d1 "$W" manual "$(bash1 "PATH=/nonexistent:\$PATH $B")"
try F2d2 "$W" manual "$(bash1 "BASH_ENV=/dev/null $B")"
try F2d3 "$W" manual "$(bash1 "env $B")"
try F2d4 "$W" manual "$(bash1 "bash $B")"
try F2d5 "$W" manual "$(bash1 "./$B")"
# F3 compound / substitution (blocking)
try F3a "$W" manual "$(bash1 "$B && $B")"
try F3b "$W" manual "$(bash1 "$B && touch $G/pwned1")"
try F3c "$W" manual "$(bash1 "$B; touch $G/pwned2")"
try F3d "$W" manual "$(bash1 "$B | touch $G/pwned3")"
try F3e "$W" manual "$(bash1 ".git-paw/scripts/broker.sh --agent x question \"\$(touch $G/pwned4)\"")"
ls "$G"/pwned* 2>/dev/null && echo '!! F3 side effect ran'
# F4 ask rule wins (blocking for manual + acceptEdits)
settings "$R" "$FULL"
for m in manual acceptEdits auto; do try "F4-$m" "$W" "$m" "$(bash1 '.git-paw/scripts/sweep.sh approve 1')"; done
# F5 deny/ask on file tools (blocking), acceptEdits
try F5a "$W" acceptEdits "Use the Edit tool (not Bash) to append the line '# t' to .git-paw/scripts/broker.sh."
try F5b "$W" acceptEdits "Use the Write tool (not Bash) to overwrite .git-paw/scripts/broker.sh with the single line '# t'."
try F5c "$W" acceptEdits "Use the Write tool (not Bash) to create sub/.git-paw/scripts/broker.sh containing '# t'."
try F5d "$R" acceptEdits "Use the Edit tool (not Bash) to append the line '# t' to .git-paw/scripts/broker.sh."
try F5e "$R" acceptEdits "Use the Edit tool (not Bash) to append the line '# t' to .git-paw/worktrees/feat-x/.git-paw/scripts/broker.sh."
try F5f "$W" acceptEdits "Use the Edit tool (not Bash) to change 'port = 9119' to 'port = 1' in .git-paw/config.toml."
try F5g "$W" acceptEdits "Use the Edit tool (not Bash) to append the line 'x' to notes.txt."   # control: CHANGED
# F6 acceptEdits auto-approved filesystem commands vs the deny rule (blocking)
i=0
for c in "sed -i.bak 's/ran/x/' .git-paw/scripts/broker.sh" \
         "cp .git-paw/scripts/sweep.sh .git-paw/scripts/broker.sh" \
         "mv .git-paw/scripts/sweep.sh .git-paw/scripts/broker.sh" \
         "rm .git-paw/scripts/broker.sh" \
         "touch .git-paw/scripts/new.sh" \
         "mkdir -p sub/.git-paw/scripts && cp .git-paw/scripts/broker.sh sub/.git-paw/scripts/broker.sh" \
         "printf 'x\n' > .git-paw/scripts/broker.sh" \
         "cd .git-paw/scripts && sed -i.bak 's/ran/x/' broker.sh" \
         "cd .git-paw && cp scripts/sweep.sh scripts/broker.sh"; do
  i=$((i+1)); fixtures; try "F6-$i" "$W" acceptEdits "$(bash1 "$c")"
done
fixtures; try F6-ctl "$W" acceptEdits "$(bash1 'touch notes2.txt')"   # control: notes2.txt created
# F7 informational: other modes
fixtures
for m in auto bypass; do
  try "F7a-$m" "$W" "$m" "$(bash1 "$B")"
  try "F7b-$m" "$W" "$m" "Use the Edit tool (not Bash) to append the line '# t' to .git-paw/scripts/broker.sh."
  fixtures; try "F7c-$m" "$W" "$m" "$(bash1 "sed -i.bak 's/ran/x/' .git-paw/scripts/broker.sh")"; fixtures
done
echo "transcripts: $G"
```

**Reading a result.** `files` is whether any helper, config or notes file changed; `marker` lists
which stub copies ran; `denials` counts the result's `permission_denials`. A blocking step passes
only when the expected effect is observed **and**, for an expected refusal, `denials ≥ 1`. A refusal
with `denials = 0` and no change means the model declined to try: re-run that step with
`--output-format stream-json --verbose` (or a reworded instruction) until the tool call is visible;
an inconclusive step counts as failed.

| Step | Expected for a verified ship | Blocking |
|---|---|---|
| F1a | marker `ran .git-paw/scripts/broker.sh --agent x status booting` (the worktree copy), denials 0 | yes |
| F1b | refused (a worktree-local file is not read) — informational if allowed | no |
| F1c | refused (control: proves `--permission-prompts none` denies) | yes |
| F1e | allowed (the rule works at the root) | yes |
| F2a, F2b2, F2c1 | record whether a bare invocation matches; decides D5's exact forms | no |
| F2b1 | allowed (records `:*` equivalence) | no |
| F2c2 | allowed | yes |
| F2c3, F2c5 | refused | yes |
| F2c4 | record (quoted subcommand) | no |
| F2d1–F2d4 | refused, marker empty | yes |
| F2d5 | record | no |
| F3a | allowed, two marker lines | no |
| F3b–F3e | refused, no `pwned*` file | yes |
| F4-manual, F4-acceptEdits | refused | yes |
| F4-auto | record | no |
| F5a–F5f | `files=same`, refused | yes |
| F5g | `files=CHANGED` (control) | yes |
| F6-1…F6-9 | `files=same` | yes |
| F6-ctl | `notes2.txt` created (control: auto-approval active) | yes |
| F7* | record | no |
| F8 (optional, interactive) | in a throwaway interactive session in `$W`, answer "Yes, and don't ask again" once for `ls -la` and once for `ls -la && pwd`; record the rule(s) written and the file | no |

**Outcomes.**

- **O1 — every blocking step passes:** set the row's `verified = true` and `verified_with` to the
  CLI version and date; adjust the row's entry forms if F2 showed a different working form (and amend
  the literal entries in `specs/cli-setup/spec.md` to match, then re-validate); add exact forms if a
  bare invocation is not matched.
- **O2 — F1a refused (the main checkout's file is not read from a nested worktree), or F1c/F1e
  wrong:** `verified = false`; amend D4 and the target requirement's wording to record the observed
  resolution; the finding is recorded in "Observed facts" below for a later release to design the
  per-worktree target afresh.
- **O3 — a blocking tamper or matching step fails (F2c3/F2c5, F2d, F3b–e, F4, F5, F6):** if a
  different protection form closes it (e.g. an extra `Write(…)` rule, or `//<abs repo>/**` anchoring),
  amend the row and the spec literals and re-run the whole gate once; otherwise `verified = false`
  and the failing steps are recorded in "Observed facts" below for later releases.
- **O4 — the gate cannot be run** (no CLI, no auth, no network): `verified = false`.

In every outcome the implementer fills in "Observed facts" below (and the same summary in the row's
doc comment) before writing any other code. `verified = false` ships the command as `--dry-run` plus
docs, as the GP-01 lesson requires.

## Observed facts (filled in by task 1)

| Item | Observation |
|---|---|
| CLI version / date / OS | _pending_ |
| F1 settings resolution (a, b, c, d, e) | _pending_ |
| F2 rule forms (bare match, `:*` equivalence, quoted, env prefix, interpreter, `./`) | _pending_ |
| F3 compound and substitution | _pending_ |
| F4 ask precedence (manual / acceptEdits / auto) | _pending_ |
| F5 file-tool deny/ask (Edit, Write, any depth, root session) | _pending_ |
| F6 acceptEdits filesystem commands vs deny | _pending_ |
| F7 auto / bypass | _pending_ |
| F8 durable-grant shapes and file | _pending_ |
| Outcome (O1–O4) and resulting `verified` | _pending_ |

## Threat analysis (security-and-safety-review)

- **Privilege granted.** Eleven allow entries naming git-paw's own helpers by relative path and fixed
  subcommand; no `curl`, interpreter, directory wildcard or argument-derived entry. The grant reaches
  every session in the repository (D4), which is why keystroke and verdict subcommands are excluded and
  `approve` is pinned to `ask`.
- **Tamper / persistence.** D6: deny on helper directories, ask on the helper configuration, gate F5/F6
  as a ship condition, doctor drift warning, and worktree copies rewritten on every attach.
  *Residuals (verified case):* an agent CLI that is not Claude Code (no row, nothing written); a
  user-installed MCP server or other tool that writes files outside the CLI's permission system;
  `bypassPermissions` / `full-auto`, where the grant adds no privilege because nothing prompts; the
  CLI's own `auto` mode, where its classifier decides edits (F7 records how deny rules behave there).
  Docs recommend the v0.15 sandbox, `--undo` when setup is no longer wanted, and not combining the item
  with `full-auto` outside a sandbox.
- **Command-shape bypasses.** Environment prefixes (`PATH=…`, `BASH_ENV=…`), `env`/interpreter
  wrappers, chaining and command substitution must not ride on a helper grant; gate F2d/F3 make that a
  ship condition.
- **Spoofing.** `broker.sh --agent <other>` can publish as another agent id. Pre-existing (the
  classifier allowed it too); broker-side identity is v0.18.0 `broker-verdict-authority`. Verdict
  publishing (`sweep.sh verified` / `feedback-gate`) is deliberately not granted.
- **Prompt injection / an agent running the command.** No git-paw auto-approval after
  `classifier-retirement`; the setup entries never allow `git paw`; the skill's "only when the human
  asked in this conversation" rule; linked-worktree refusal as defence in depth. Impact if it still
  happens: fixed, recorded, undoable entries.
- **Writing a user-owned file.** Merge with order preserved, atomic replace, re-read before rename,
  tracked/symlink refusal, mode kept, plan never prints existing content. *Residual:* a write by the
  CLI itself (a "don't ask again" approval) landing between git-paw's re-read and rename is lost; docs
  advise running setup when no session is waiting on an approval.
- **Undo correctness.** Exact-equality removal, never claiming pre-existing entries, shape validation,
  protection kept while any helper grant remains, skeleton-only deletion (D9).
- **Leakage.** The record is in the git dir; the plan prints only git-paw's entries; the doctor
  advisory abbreviates the rules it lists.
- **Supply chain.** A serde_json feature over an already-locked crate; no new dependency.
- **Containment.** Worktree confinement and the documented sandbox are unchanged.

## Export agnosticism

The exported surfaces are the entries written into the consumer's settings file and the bundled
`cli-setup` skill (plus the supervisor-skill section). The entries name only git-paw's own helper
paths, subcommands and `.git-paw/config.toml`; they are compiled constants, independent of the
consumer's config, stack and repository content (a test plans two differently configured repositories
and compares). The skill names no vendor product, settings path or stack token and defers per-CLI
facts to `git paw doctor`; `tests/lang_agnostic_skill_audit.rs` is extended to it. Vendor specifics
(`.claude/settings.local.json`, `Bash(…)`/`Edit(…)` syntax) live only in the compiled fact row, in
doctor output and in the docs' per-CLI pages.

## Changes from the v0.19.0 source

- **Scope:** allowlist item for `claude` only; `--trust`/`--hooks` kept as flags that always answer
  "not supported" (exit 2). No `family`, so the "declared variant" scenario is dropped and replaced by
  "a CLI without a fact row supports no item".
- **Classifier removed:** dropped the ADDED *CLI setup invocations escalate as danger* requirement,
  the `DANGER_BASE` task, the "helper source the classifier's managed-helper rule uses" wording, and
  the MODIFIED requirement's classifier sentence. The MODIFIED *No permission grants…* text is
  self-contained and depends on no other `approval-command-safety` requirement.
- **Dependencies inlined:** `cli-provider-record`'s `setup_facts()` facet → a one-row compiled fact
  table (D3); the v0.20.0 registry / `MANAGED_HELPER_SCRIPTS` → `init::bundled_scripts()` plus a grant
  table (D5); v0.18.0 `cmd-library-surface` → `commands::setup_cli` + prompt in `main.rs` (D12); v0.18.0
  `.git-paw/skills/` writer and orchestrator skill → embedded skill + supervisor-skill section (D11).
- **Target:** the source's "setup directories" (root plus each session worktree for that CLI, with
  `git worktree list` validation) replaced by the single main-checkout file the docs describe (D4).
- **Entries:** the source's one `Bash(<path>:*)` per installed helper replaced by the fixed grant
  table — per-subcommand `sweep.sh`, `docs-fetch.sh find` only, `approve` on `ask`, verdict verbs
  excluded, space form (D5).
- **Tamper mitigation designed in (new):** protection rules in the same write, gate F5/F6 as a ship
  condition, verification status and plan-only mode (D3, D6), undo never strips protection (D9). The
  source accepted the tamper residual with a drift warning only.
- **Fact gate rewritten** as an executable script with pass criteria and outcomes, recorded here.
- **Exit 2** requires MODIFYING `core-error-handling` *Exit codes distinguish cancellation from
  errors* (the source did not; v0.17.0's text says every non-cancel error exits 1).
- **Doctor:** the per-directory "partially applied" state became "out of date"; ADDED the broad-grant
  advisory (D14).
- **Unchanged:** the printed plan, the consent gate, merge-not-clobber with `preserve_order` (approval
  to be confirmed), the git-dir record written before each write, shape-checked undo, dry run,
  linked-worktree refusal, tracked/symlink refusal, no MCP tool, the skill's consent rules.

## Cross-change review (pending changes in `openspec/changes/`)

Excluded as dropped: `classifier-compound-segments`, `broad-grant-scope-safety`,
`approval-send-identity-check`, `approval-pane-map-refresh`.

| Change | Requirement-level overlap | Semantic interplay |
|---|---|---|
| `classifier-retirement` | none: it leaves *No permission grants…* to this change and touches no other requirement here | archive/merge before this change (D10 hazard); its *Prompt-free operation is attributed to the agent CLI's permission system* is compatible — setup writes the CLI's own rules at the user's request, not "on its own"; the supervisor-skill section must not call a helper path safe (its ADDED supervisor-skill requirement); its *Helper publish subcommands* change keeps helpers at an exact path a rule can name, which D5 relies on |
| `bundled-helper-discovery` | none (it MODIFIES `broker-agent-helper` *Helpers provisioned into agent worktrees* and *Helper publish subcommands*, `session-state`, `supervisor-learnings`; this change modifies none of them) | provides `.git-paw/scripts/broker.sh` in every worker worktree, so the relative grant resolves for every launch path; archives first |
| `cli-launch-identity` | none (`cli-resolution`, `boot-block` only) | `<CLI>` is keyed by the same identity rule; a `[clis.*]` variant CLI name answers "not supported", though `git paw setup-cli claude` still covers sessions of a Claude Code variant because the target settings file belongs to the repository, not the CLI |
| `status-and-picker-truth` | none (MODIFIES `preflight-diagnostics` *Hygiene check*; this change only ADDs to that capability) | — |
| `supervisor-boot-wiring` | none (MODIFIES `skill-agent` *pane_current_path* requirement; this change only ADDs) | both edit `assets/agent-skills/supervisor.md` text — merge carefully |
| `production-panic-free` | none (`core-ci-hygiene`, `core-project-conventions`) | new code must use no `unwrap`/`expect` outside tests and its regex helper if any static regex is added |
| `drive-loop-manual-decision-log` | none | absorbed by `classifier-retirement` |
| `broker-listener-origin-guard`, `broker-repo-identity`, `conflict-escalation-persistence`, `stop-without-confirmation` | none | — |

`core-error-handling` *Exit codes distinguish cancellation from errors* is modified by no other
pending v0.17.0 change; v0.24.0 `error-exit-code-contract` later MODIFIES it again and must carry
`NotAttempted` (as its missing-terminal variant) in its superset text.

## Risks / Trade-offs

- [The gate fails or cannot run] → the row ships plan-only: `--dry-run` and docs, no writing, and
  helper calls keep prompting until a later release re-runs the fact gate. Honest, and the machinery
  is tested with constructed rows.
- [`preserve_order` reorders other `serde_json::Value` output] → reviewed in task 2.1; alternative in
  D7.
- [One user-owned file reaches every session] → minimal grant table, repository-wide protection, undo.
- [Per-subcommand `sweep.sh` rules are a list to maintain] → the dispatch guard test fails when
  `sweep.sh` gains a subcommand that is in neither column.
- [Supervisor verdicts still prompt] → deliberate (forgery from worker sessions); the user can add their
  own rule knowingly.
- [Formatting normalisation of a hand-formatted settings file] → documented; values and order kept.

## Migration Plan

Additive. No config field and no persisted-state change outside the new git-dir record. Rollback is
reverting the change. Entries already written stay in the user's file after a downgrade, so the docs
say to run `git paw setup-cli claude --undo` before downgrading.

## Open Questions

- **Gate outcome** (O1–O4) — decided by task 1, recorded above.
- **`preserve_order` approval for v0.17.0** — for the orchestrator to confirm (D7 alternative ready).
- **The main-checkout target** — if the gate confirms it, the user should confirm that writing into the
  repository-root `.claude/settings.local.json` (their own file) is acceptable as the single target.
- **`CliNotFound` exit code** (1 today) — v0.24.0 decides whether an unknown identity is an argument
  error (2).
