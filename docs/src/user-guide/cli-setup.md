# Per-CLI Setup

`git paw setup-cli` is the one command that writes into an agent CLI's own
settings file. Running it is the consent.

Agents call git-paw's bundled helper scripts constantly — to publish status,
poll for messages, capture panes. Under most agent CLIs every one of those
calls raises a permission prompt. `setup-cli` lets you pre-approve exactly
those calls, in the CLI's own allow rules, once.

## Setup is consent-only

Nothing else in git-paw writes an agent CLI's settings. Not `start`, not
`add`, not `init`, not session recovery, not the drive loop, not the
dashboard, and no tool exposed over MCP. Earlier versions of git-paw tried to
seed permissions automatically; that was removed, and it is not coming back
under a different name.

Everything the command does is therefore gated:

- it prints its **plan** before anything is written;
- it writes nothing without `--yes` or your confirmation on a terminal;
- it **records** every entry, so it can take back exactly what it added;
- it refuses to run from a linked worktree, so a coding agent in a worktree
  cannot run it by accident.

## What it writes, and where

For `claude`, the allowlist item targets exactly one file:

```
<main checkout>/.claude/settings.local.json
```

That one file is enough. Claude Code reads the settings file at the
repository root even when the session starts in a subdirectory or in a linked
worktree — which is what the change's fact gate confirmed against the
installed CLI. So a single write covers your root session, every agent
worktree nested under `.git-paw/worktrees/`, and every session you start
later, including worktrees that do not exist yet.

git-paw never writes a settings file **inside** a worktree (that would put the
file inside the agent's own reach), never writes a user-level settings file,
and never writes the path you configured as `[clis.<name>].settings_path`.

If the file is not already ignored by git, the plan also adds one anchored
line to the repository's shared `info/exclude`, so a file git-paw created does
not show up in your `git status`.

## The grant table

The allow entries are a fixed table. Nothing is derived from a command
argument, from your configuration, or from anything in your repository — two
differently configured repositories plan byte-identical entries.

| Helper | Granted | Deliberately not granted |
|---|---|---|
| `broker.sh` | every subcommand | — |
| `docs-fetch.sh` | `find` only | `get` |
| `sweep.sh` | `snapshot`, `capture`, `status`, `worktrees-status`, `inbox`, `detect-stuck`, `stuck-eval`, `status-publish`, `learn`, `events` | `approve`, `tell`, `verified`, `feedback-gate`, `conflict-ack`, `answer`, `advanced-main`, `classify` |
| `_paw_common.sh` | — (it is sourced, never invoked) | — |

That is **twelve** allow entries in total.

Each entry names the helper by its relative package invocation path,
`.git-paw/skills/<package>/scripts/<helper>`, derived from the bundled package
registry — like `Bash(.git-paw/skills/coordination/scripts/broker.sh *)`. No
entry names the deprecated compatibility path `.git-paw/scripts/<helper>`
(nothing bundled invokes it), an interpreter, an absolute path, a directory
wildcard or `curl`.

`sweep.sh events` is granted — as
`Bash(.git-paw/skills/orchestrator/scripts/sweep.sh events *)` — because it only reads the
broker's log and status to serve the
[event long poll](coordination.md#the-bundled-helper-verb). It sends no
keystroke and publishes no verdict.

What is withheld, and why:

- **`docs-fetch.sh get`** takes an absolute `http(s)://` or `file://` URL. It
  is a read-anywhere and exfiltration primitive; `find` only fetches the one
  configured index.
- **`sweep.sh approve`** presses keys in another pane. It also gets an
  explicit **ask** rule — at the package path *and* at the compatibility path
  `.git-paw/scripts/sweep.sh`, because the compatibility copy still runs — so
  it prompts even if some broader rule of your own would otherwise allow it.
- **`sweep.sh tell`** types a message into another agent's input. A
  pre-approved `tell` would let one agent relay text into a peer's pane
  without anyone seeing it, so it is never granted and gets its own **ask**
  rules, `Bash(.git-paw/skills/orchestrator/scripts/sweep.sh tell *)` and
  `Bash(.git-paw/scripts/sweep.sh tell *)`, like `approve`. A broad allow rule
  you wrote yourself, such as `Bash(.git-paw/scripts/sweep.sh *)` or
  `Bash(.git-paw/skills/orchestrator/scripts/sweep.sh *)`, covers `tell` too
  and lets it run silently wherever those `ask` rules are missing;
  `git paw doctor` reports such a rule as grant residue.
- **`sweep.sh verified` and `feedback-gate`** publish supervisor verdicts, and
  **`conflict-ack`** writes feedback into other agents' inboxes. The target
  file reaches worker sessions as well as the supervisor's, so granting these
  would let a worker forge its own verification or tell a peer a conflict was
  cleared.
- **`sweep.sh answer` and `advanced-main`** publish with supervisor authority:
  an answer is authoritative guidance to another agent, and a main-advance
  event makes the drive loop rebase worker branches. Granting either would let
  a worker forge one without a prompt.
- **`sweep.sh classify`** is on its way out with the command classifier. It is
  listed as withheld so the guard that requires every subcommand to be
  explicitly granted or explicitly withheld keeps passing while it still
  exists.

## The protection rules

A granted helper runs **without a prompt**, and it lives inside the agent's
own worktree. Without protection, an agent could edit the script and have the
grant run whatever it wrote. So the allowlist item never writes the grants
alone — it writes these in the same file and the same replacement:

| Rule | Where | Why |
|---|---|---|
| `Edit(**/.git-paw/skills/**)` | deny | refuses the CLI's file-editing tools on any installed skill package — the granted helpers under its `scripts/` included — at any depth |
| `Edit(**/.git-paw/scripts/**)` | deny | the same for any compatibility helper directory, because the copies still run |
| `Edit(**/.git-paw/config.toml)` | ask | the helpers read this file (the broker address), so a change to it always prompts |

The whole package directory is denied, not just its `scripts/`: nothing an
agent should edit lives there. The fact gate confirmed both deny rules against
the installed CLI (Claude Code 2.1.322), including that they block the Edit
and Write tools and every filesystem command `acceptEdits` auto-approves
(`cp`, `mv`, `rm`, `touch`, `mkdir`, a `printf` redirect, plus `sed -i` and
`cd`-prefixed forms) against a package helper, its `_paw_common.sh`, a package
`SKILL.md`, a worker's package copy and a compatibility copy.
`git paw doctor` additionally warns when a helper at a path an applied allow
entry names no longer matches the copy the binary bundles.

## Using it

```bash
# See exactly what would be written. Writes nothing.
git paw setup-cli claude --allowlist --dry-run

# Apply it, with a confirmation on a terminal.
git paw setup-cli claude --allowlist

# Apply it without asking (scripts, CI).
git paw setup-cli claude --allowlist --yes

# Take back exactly what git-paw added.
git paw setup-cli claude --undo --yes
```

Run it from the repository's **main checkout**. At least one of
`--allowlist`, `--trust`, `--hooks` or `--undo` is required.

`--trust` and `--hooks` exist so the flag surface will not change when later
releases add those facts. Today no CLI declares them, so they report "not
supported" and write nothing.

### Re-running after an upgrade

**git-paw never rewrites recorded grants itself.** When a release adds an
entry to the grant table — `sweep.sh events` is the v0.18 example, and the
`sweep.sh tell` ask rule a later one — a file you applied under an earlier
version keeps exactly the entries that version wrote.
Nothing updates it in the background.

`git paw doctor` notices and reports the applied setup as **out of date**,
naming how many entries this version would also write. It judges that against
your CLI's settings file rather than against git-paw's record, so an entry you
already held yourself counts as applied — see
[the per-CLI setup checks](doctor.md#the-per-cli-setup-checks). The remedy is to take back what
git-paw wrote and apply the current table:

```bash
git paw setup-cli claude --undo
git paw setup-cli claude --allowlist
```

Until you do, the new subcommand simply prompts the way any ungranted command
does — nothing breaks, you just answer a prompt.

### Undo

`--undo` removes an entry only when it is still present, still byte-equal to
what was recorded, and still of a shape the item produces. Anything else — an
entry you edited, an entry someone added to the record by hand — is reported
and dropped from the record, never removed from your file. The protection
rules stay as long as any allow entry still names a path under
`.git-paw/scripts/` or `.git-paw/skills/`, including one you added yourself.
Undo accepts entries at either path shape, so a record written by v0.17.0 –
v0.19.0 (which granted `.git-paw/scripts/…`) undoes cleanly.

Undo removes entries, not containers: an `allow`/`ask`/`deny` list that setup
created and undo emptied is left in place, empty. An empty list grants
nothing, and removing it would mean touching something git-paw cannot prove
it created.

A file git-paw created is deleted only when nothing but its own skeleton is
left; if you added a key of your own, the file stays.

### Upgrading from v0.19.0 or earlier

The helpers moved into their skill packages: the grant table now names
`.git-paw/skills/<package>/scripts/<helper>`, and the old
`.git-paw/scripts/<helper>` is a deprecated compatibility copy (see
[Installed packages](skill-templates.md#installed-packages)). An allowlist
applied by an earlier version still grants the old paths, which agents no
longer call, so:

- `git paw doctor` reports the applied setup **out of date**;
- until you re-apply, every helper call at the package path prompts (and is
  escalated under `--unattended`);
- git-paw never rewrites your recorded grants itself.

Re-apply when you are ready:

```bash
git paw setup-cli claude --undo
git paw setup-cli claude --allowlist
```

`--undo` removes the old-path entries and their protection; the new plan adds
the package-path entries, both deny rules and the ask rules at both paths. A
CLI allow rule you wrote by hand for `.git-paw/scripts/…` should name
`.git-paw/skills/<package>/scripts/…` instead. Review any "don't ask again"
rule a CLI saved for `.git-paw/scripts/…` — it keeps granting the
compatibility copy; `git paw doctor` lists the wide ones.

### Upgrading from 0.17.0 or earlier

If you applied the allowlist on an earlier release and `git paw doctor` told
you the applied allowlist was out of date — straight after applying it, with
an `--undo` and re-apply remedy that never cleared the warning — that is
fixed. Setup skips a planned entry your settings file already holds and
records only what it wrote, and doctor used to compare its plan against the
record alone, so any entry you already had read as permanently missing. Doctor
now judges the plan against the settings file, so an entry you already had
counts as applied and the undo/re-apply round trip converges. Nothing about
what setup writes, what it records, or what undo removes has changed.

Doctor also reports **grant residue** now: durable rules left in the
project's CLI settings by earlier versions or by earlier "don't ask again"
answers — a blanket `curl` grant, a `sweep.sh` rule covering a subcommand the
table above withholds, or the inert `allowed_bash_prefixes` seed written
before v0.16.0. git-paw does not rewrite a grant it did not write, so those
must be removed by hand. See [Doctor](doctor.md).

## Verified and plan-only

Each item carries a verification status: whether its facts — the target file,
the key paths, the entry forms, and the protection the entries rely on — were
confirmed against an installed copy of that CLI.

- **Verified**: the plan can be applied.
- **Plan-only**: `--dry-run` still prints the plan, but writing is disabled
  and the command exits `2`. `--undo` keeps working, so entries written by an
  earlier version can still be removed.

The `claude` allowlist item ships **verified**, against Claude Code 2.1.322
(2026-11-12), for the package-path entries and both deny rules. A
confirmation made only for the pre-package `.git-paw/scripts/` entries (as
v0.17.0–v0.19.0's, against 2.1.220) would not count for them; the fact gate
was re-run for this release. `git paw doctor` reports the current status.

Claude Code variants (a wrapper binary registered under another name) are
covered by `git paw setup-cli claude`: the target settings file belongs to the
repository, not to the binary.

## The record

Every entry git-paw writes is recorded at:

```
<git-common-dir>/git-paw/cli-setup.json
```

It lives in the git directory, so it can never be committed, never shows up in
any worktree's `git status`, and is shared by every worktree of the
repository. It is written owner-only, and an entry is recorded before it is
written — a crash in between leaves a recorded entry that is not on disk,
which undo simply drops.

## Residual risks

Read these before applying the item:

- **Prefer the FS-scoped sandbox.** The allowlist reduces prompts; the
  [sandbox](sandbox.md) bounds what a process can reach at all. They are
  complementary, and the sandbox is the stronger control.
- **Do not pair the allowlist with `full-auto` outside a sandbox.** In a mode
  where nothing prompts, the protection rules buy you nothing, because
  everything else is unprompted too.
- **Undo it when you no longer want it.** The grant reaches every session in
  the repository, including ones you start much later.
- **Undo before downgrading.** Entries already written stay in your file after
  a downgrade, and an older git-paw does not know how to remove them.
- **Run setup when no session is waiting on an approval.** The CLI writes to
  the same file when you answer "don't ask again"; a write that lands between
  git-paw's re-read and its rename would be lost. git-paw detects the change
  and stops rather than clobbering it.
- **Re-run setup soon after upgrading.** Until you run `--undo` then
  `--allowlist`, an allowlist applied by v0.19.0 or earlier carries only the
  old `Edit(**/.git-paw/scripts/**)` deny rule. If you answer "don't ask
  again" to a prompt for a package-path helper in that window, the CLI saves
  a grant for `.git-paw/skills/…` that no deny rule protects, so an agent in
  an edit-accepting mode could rewrite that helper and have the grant run it.
  Re-apply the item (which adds `Edit(**/.git-paw/skills/**)`) before
  approving package-path prompts durably, and review any grant you saved
  meanwhile.
- **Durable grants on the old path keep working.** A "don't ask again" rule
  or a hand-written rule for `.git-paw/scripts/…` still grants the
  compatibility copy, which still runs. The re-applied item keeps
  `Edit(**/.git-paw/scripts/**)` so the copies stay protected, and
  `git paw doctor` lists the wide ones; review and remove what you did not
  intend — git-paw edits none of them.
- **A grant is not a judgement about safety.** git-paw decides nothing about
  your prompts. What runs without one is decided entirely by the agent CLI's
  own permission system and the rules in its settings.

See also: [Security Posture](security-posture.md) and
[Doctor](doctor.md).
