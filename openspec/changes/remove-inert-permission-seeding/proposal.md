## Why

git-paw seeds command allowlists into an agent CLI's settings file at session startup so
routine actions run without permission prompts. The seeded key is
`allowed_bash_prefixes` (`src/supervisor/curl_allowlist.rs:188`, read back at
`src/supervisor/worktree_allowlist.rs:233`). Claude Code does not read that key — it
reads `permissions.allow`. **The seeding is therefore inert on the one CLI it was
designed for**, and meaningless on every other CLI, none of which has an equivalent file.
`allowed_bash_prefixes` is git-paw's own invented key; nothing reads it back except
git-paw's tests.

The cost is not merely a no-op. git-paw writes a `.claude/` directory into every worktree
regardless of which CLI is running there, adds a `.claude/` line to `.git/info/exclude`,
and the supervisor skill tells the supervisor this is how the first broker call avoids a
prompt — guidance that is simply false.

**GP-01's replacement is already specced, and it is a skill.**
`worker-guidance-skill-export` reframes GP-01 exactly this way: rather than seed an
inert allowlist into every CLI's settings file, rely on the CLI's own permission mode
plus git-paw's classifier, and export the *guidance* as agnostic skills — worktree
orientation in the coordination skill, the tiered permission model in the supervisor
skill, deferring to `git paw __classify` as the single source of policy. That change
touches no code and explicitly defers removing the inert machinery as "a separate
deliberate decision."

This change is that decision, and nothing more: delete the dead code the skill replaced.

## What Changes

- **Remove the inert allowlist seeding.** The system no longer writes
  `allowed_bash_prefixes` into any CLI settings file, no longer creates `.claude/`
  directories in worktrees running other CLIs, and no longer adds the corresponding
  `.git/info/exclude` entry on behalf of a CLI that does not use it.
- **Correct the supervisor guidance** that attributes prompt-free broker calls to the
  seeded allowlist. Prompt-free operation comes from the CLI's resolved permission mode
  and git-paw's classifier — which is what `worker-guidance-skill-export` documents.
- **Nothing replaces the code.** The replacement is the exported skill guidance, already
  specced elsewhere. This change adds no configuration field, no strategy declaration,
  and no seeding mechanism of any kind.

**Deliberately NOT in scope — no successor seeding mechanism.** An earlier framing of
this work proposed a per-CLI "seeding strategy" declaration defaulting to none. That was
dropped: no CLI has a surface git-paw should write to, so the declaration would have had
zero implementors — a configuration knob every consumer sets to "off" and nothing ever
sets otherwise. If real seeding is ever wanted (for example a genuine
`permissions.allow` writer), it needs its own proposal, because writing live permission
grants into a user's settings file is a materially larger security decision than
deleting dead code.

**Depends on:** `unattended-boot-hardening` (resolves each CLI's permission mode to an
explicit flag) and `worker-guidance-skill-export` (exports the replacement guidance).
Removing the seeding before those land would remove a no-op but leave the guidance
pointing at it.

**Breaking?** No functional behaviour is lost, because the removed machinery never
functioned. Users with a hand-maintained `allowed_bash_prefixes` array are unaffected —
git-paw stops writing to it, and never read it back except in its own tests.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `approval-command-safety`: the curl-allowlist setup requirement and the
  allowlist-file-format requirement are removed. Prompt-free operation is attributed to
  the resolved permission mode and the classifier instead. The classifier, danger list,
  prompt detection and every safety gate are unchanged.
- `broker-agent-helper`: the scenario asserting that a rich `status-publish` needs no
  broad curl grant no longer premises that on a seeded by-path grant; the guarantee now
  rests on the helper being a single stable path evaluated by the classifier.

## Impact

- **Code (deletions only):** `src/supervisor/curl_allowlist.rs` (the
  `allowed_bash_prefixes` writer), `src/supervisor/dev_allowlist.rs`,
  `src/supervisor/worktree_allowlist.rs` (including `ensure_claude_dir_excluded`),
  `src/commands/supervisor.rs:469-475`, `src/commands/recover.rs:52-53`.
- **Config:** no field is added. `[clis.<name>].settings_path` is **retained** — note
  `core-memory-isolation` keys its isolation set off that field's **parent directory**,
  independent of seeding. That use MUST survive this change; verify it explicitly.
- **Assets:** the supervisor skill's claim that the seeded allowlist is why the first
  curl does not prompt. Coordinate with `worker-guidance-skill-export`, which edits the
  same skill's permission section.
- **Capability purpose text:** `approval-command-safety`'s purpose paragraph describes
  the seeding at length; it must be updated when this change is archived.
- **Enum-variant ripple:** none.
- **Backward compatibility:** configs carrying `settings_path` continue to load. Existing
  `.claude/settings.json` files are left in place, not deleted — git-paw simply stops
  writing to them.
- **Docs:** remove the allowlist-seeding entries from the configuration reference, update
  the supervisor guide's permission-model section, and any quick-start describing
  allowlist seeding.
