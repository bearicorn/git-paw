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
| `sweep.sh` | `snapshot`, `capture`, `status`, `worktrees-status`, `inbox`, `detect-stuck`, `stuck-eval`, `status-publish`, `learn`, `events` | `approve`, `verified`, `feedback-gate`, `conflict-ack`, `classify` |
| `_paw_common.sh` | — (it is sourced, never invoked) | — |

That is **twelve** allow entries in total.

Each entry names the helper by its relative invocation path, like
`Bash(.git-paw/scripts/broker.sh *)`. No entry names an interpreter, an
absolute path, a directory wildcard or `curl`.

`sweep.sh events` is granted — as
`Bash(.git-paw/scripts/sweep.sh events *)` — because it only reads the
broker's log and status to serve the
[event long poll](coordination.md#the-bundled-helper-verb). It sends no
keystroke and publishes no verdict.

What is withheld, and why:

- **`docs-fetch.sh get`** takes an absolute `http(s)://` or `file://` URL. It
  is a read-anywhere and exfiltration primitive; `find` only fetches the one
  configured index.
- **`sweep.sh approve`** presses keys in another pane. It also gets an
  explicit **ask** rule, so it prompts even if some broader rule of your own
  would otherwise allow it.
- **`sweep.sh verified` and `feedback-gate`** publish supervisor verdicts, and
  **`conflict-ack`** writes feedback into other agents' inboxes. The target
  file reaches worker sessions as well as the supervisor's, so granting these
  would let a worker forge its own verification or tell a peer a conflict was
  cleared.
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
| `Edit(**/.git-paw/scripts/**)` | deny | refuses the CLI's file-editing tools on any helper directory, at any depth |
| `Edit(**/.git-paw/config.toml)` | ask | the helpers read this file (the broker address), so a change to it always prompts |

The fact gate confirmed both against the installed CLI, including that the
deny rule also blocks the filesystem commands `acceptEdits` auto-approves
(`sed -i`, `cp`, `mv`, `rm`, `touch`, and `cd`-prefixed forms). `git paw
doctor` additionally warns when a granted helper no longer matches the copy
the binary bundles.

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
entry to the grant table — `sweep.sh events` is the v0.18 example — a file you
applied under an earlier version keeps exactly the entries that version wrote.
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
`.git-paw/scripts/`, including one you added yourself.

Undo removes entries, not containers: an `allow`/`ask`/`deny` list that setup
created and undo emptied is left in place, empty. An empty list grants
nothing, and removing it would mean touching something git-paw cannot prove
it created.

A file git-paw created is deleted only when nothing but its own skeleton is
left; if you added a key of your own, the file stays.

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

The `claude` allowlist item ships **verified**, against Claude Code 2.1.220
(2026-10-15). `git paw doctor` reports the current status.

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
- **A grant is not a judgement about safety.** git-paw decides nothing about
  your prompts. What runs without one is decided entirely by the agent CLI's
  own permission system and the rules in its settings.

See also: [Security Posture](security-posture.md) and
[Doctor](doctor.md).
