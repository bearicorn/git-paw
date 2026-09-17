## Context

`serde_yaml` is used in one place: `parse_standardized_metadata` in `src/skills.rs`, which
splits a skill file at its `---` fences and deserialises the frontmatter into
`StandardizedSkillMetadata`. The fence handling, the required-field checks and the error
type are already hand-written; only the middle step uses the library.

The input comes from two places. Embedded skills (`assets/agent-skills/*`) are ours. User
override skills (`Source::User`) are not — a user can write any frontmatter they like, and
`serde_yaml` would have accepted any valid YAML. That second source sets the bar for this
change: the replacement must never quietly produce a different value from the same input.

Observed frontmatter in the tree exercises more than the five consumed fields: extra keys
(`user-invocable: true`), extra keys with nested block sequences (`allowed-tools:` + a
list), quoted metadata values (`version: "1.0"`), and trailing whitespace on plain scalars.

## Goals / Non-Goals

**Goals:**

- Remove `serde_yaml` and, with it, `unsafe-libyaml`, so `cargo deny` passes.
- Parse every frontmatter block in the tree to exactly the values it parsed to before.
- Fail loudly, naming the field, on anything outside the supported subset.
- Keep `StandardizedSkillMetadata`'s public shape.

**Non-Goals:**

- A general YAML parser. The subset is bounded by the agentskills.io field set on purpose.
- Changing which fields are required, or the validation messages for missing fields.
- Consuming fields git-paw ignores today (`allowed-tools`, `user-invocable`).
- Touching `deny.toml`. Once the crates are gone the advisory no longer applies; adding or
  pruning ignores is a separate decision.

## Decisions

**Hand-roll, bounded by the agentskills.io schema.** The schema fixes a small field set and
gives `metadata` as a map of string to string. Supporting exactly that — plus the scalar
forms real files use — is a few dozen lines, testable exhaustively, and removes a
deprecated dependency rather than swapping it for another one that needs approval. It
follows the repository's own `dirs` → `src/dirs.rs` precedent.

**Skip unconsumed keys structurally, reject unsupported syntax in consumed ones.** These
are different situations. An unknown key is someone else's data and must not break loading,
so it is skipped along with its indented content. A consumed key with syntax we do not
parse is a value we would otherwise get wrong, so it is an error. That asymmetry is what
makes "no silent misparse" achievable without implementing all of YAML.

**Represent every `metadata` value as a string.** The schema types them as strings, and
`StandardizedSkillMetadata.metadata` is already a `serde_json::Value`. Writing a JSON object
of strings keeps the public type unchanged. A YAML number like `version: 1.0` would
previously have become a JSON number; nothing in the tree relies on that, and the corpus
test pins the cases that do exist.

**Freeze parity fixtures before removing the crate.** The parity guarantee is "same values
as before". Once `serde_yaml` is gone there is no "before" to compare against, so the
expected values must be captured from the old parser first and committed as fixtures.

## Risks / Trade-offs

- **A user override using exotic YAML now fails to load.** That is the intended trade: a
  loud error naming the field, instead of a silently different value. It should be called
  out in the changelog for users with custom skills.
- **Block-scalar folding rules are subtle.** Folded scalars (`>`) join lines with spaces but
  preserve blank lines and more-indented lines. Get the common cases right and covered by
  fixtures; reject what is not handled rather than approximating it.
- **AGENTS.md is edited in this change.** It also carries the maintainer's in-progress,
  uncommitted edit in the main checkout. That is an integration-time concern for the
  orchestrator, not the implementer — the implementer edits the committed file in its own
  worktree as normal.
