## Context

The bundled coordination skill is the one piece of prose every worker reads at boot,
regardless of CLI. It already covers intent publishing, heartbeats, stash hygiene,
peer-dependency etiquette and commit cadence. It says nothing about how to read a file.

Across one three-agent dogfood wave that omission cost roughly a dozen permission
approvals, and wrote a handful of allowlist entries that can never match again. The
agents were not behaving unreasonably — shell pipelines are the obvious reflex, and
nothing told them otherwise.

Of the seventeen findings in that dogfood this is the only one a skill can fix. The others
are actuator defects: a scan window, a path, a dedup key, a missing injection. That ratio
matters when deciding how to fix things here: the instinct to answer a defect with more
skill text is usually wrong, which is exactly why the one case where it is right should be
kept short and precise.

## Goals / Non-Goals

**Goals:**

- Remove a recurring approval tax that nothing in the code can remove.
- Stay export-agnostic: the guidance must hold for any CLI a consumer runs.
- Keep it short. This is a paragraph, not a chapter.

**Non-Goals:**

- Forbidding shell tools. Plenty of legitimate work is a shell command; the guidance is
  about *inspecting source*, where a native tool exists and is strictly cheaper.
- Changing the classifier so these pipelines auto-approve. That would widen the safe set
  to buy convenience — the wrong trade, and the opposite of `broad-grant-scope-safety`.
- Teaching the agent which specific tools it has. It knows; the skill only needs to say
  "prefer them".

## Decisions

**Capability classes, never tool names.** AGENTS.md's export-agnosticism principle is
binding for `assets/agent-skills/*`. Writing "use Read and Grep instead of `sed`" would
ship a Claude Code instruction to every consumer, including those running Codex, Qwen or a
custom CLI, for whom it is meaningless or wrong. The guidance says "your CLI's own
file-read and search facilities". A conformance scenario pins this so a future edit cannot
quietly reintroduce a vendor name.

**Explain the cost, do not just issue the rule.** An agent given a bare prohibition will
work around it when the prohibition seems inconvenient. Telling it *why* — the approval
stall, and the dead allowlist entry — lets it generalise to cases the rule does not
enumerate, which is the point of putting this in a skill rather than a lint.

**Include the scratch-path rule here rather than as its own change.** It is the same
underlying mechanic (a durable grant keyed on an exact string) producing the same symptom
(repeated approvals), and it lands in the same paragraph. Splitting it would mean two
changes editing one file for one reason.

**No code change.** Deliberately. The temptation is to pair this with a classifier tweak
that makes `grep | awk` auto-approve. That buys convenience by widening what executes
without review, while `broad-grant-scope-safety` is narrowing exactly that surface in the
same release. Prose is the correct instrument here.

## Risks / Trade-offs

- **Prose is advisory.** An agent may ignore it. That is inherent to the instrument, and
  it is why this is the only finding being answered this way — the other sixteen get
  enforcement instead.
- **Vendor-name creep.** The most likely regression is a well-meaning future edit that
  adds a concrete example naming a specific tool, because concrete examples read better.
  The conformance scenario is the guard; it should assert the absence of CLI names, not
  merely the presence of the section.
- **Skill length.** Every addition to the coordination skill costs every agent context at
  boot. Keeping this to a short section is a real constraint, not a stylistic preference.
- **Partial overlap with `broad-grant-scope-safety`.** That change makes compound commands
  take one-time approvals, which increases prompting. This change reduces the number of
  shell commands issued at all. They pull in opposite directions on prompt count and
  together should roughly cancel — worth watching in the next dogfood rather than assuming.
