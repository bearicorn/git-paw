## Context

git-paw's export-agnosticism principle is enforced by audits that scan `assets/`. Prose
assembled in Rust is invisible to those audits, which is how the inter-agent rules came to
tell every consumer's agent about "the supervisor's spec audit" — a git-paw workflow
exported to projects that have none. The bug is not that the prose is in Rust; it is that
being in Rust puts it outside the enforcement.

An earlier framing of this work assumed `src/skills.rs` was thousands of lines of
compiled prose. It is not: production code there is ~890 lines and the remaining ~5,100
are a conformance suite over the assets. The genuine compiled-prose surface is about 150
lines spread across seven files. This change is therefore smaller and more surgical than
the file sizes suggest.

## Goals / Non-Goals

**Goals:**

- Put every sentence an agent reads under the audits that already guard the assets.
- Make a wording change a content edit rather than a binary release.
- Preserve single-binary distribution — assets are embedded at compile time, as the
  bundled skills already are.

**Non-Goals:**

- Changing any rendered output. This is a relocation; the agent sees the same text.
- Rewriting the *content* of the inter-agent rules — `peer-cherry-pick-removal` owns that
  and lands first.
- Re-routing the spec-path doctrine — `speckit-spec-path-pointer` owns that.
- Adding new skill sections — `worker-guidance-skill-export` owns that.
- Moving the hook *installation* logic, or any constant-derived prose.

## Decisions

**D1 — Byte-identical rendering is the acceptance test.** Every relocated block is
asserted to render identically before and after. This is what makes a wide, shallow edit
across seven files safe to review: the diff is large but the behaviour delta is provably
zero. A block whose output changes means the relocation was done wrong, or content was
edited in transit — both worth catching loudly.

**D2 — `render_dev_allowlist_preset` stays compiled, and the spec says why.** It is
generated from the allowlist constant so the documented allowlist cannot drift from what
is actually auto-approved. Moving it to an asset would let a hand edit widen the approval
surface silently — a security regression dressed as a refactor. Specified as a scenario
so a future reader does not "finish the job" by moving it.

**D3 — The drive-loop directive becomes a region in `supervisor.md`, not a new file.**
It is a conditional *section* of the supervisor skill and is already restated there
(`supervisor.md:300-315`). Reuse the existing region machinery (`render_opsx_regions`,
`src/skills.rs:692`) with a second marker pair. *Considered and rejected:* a standalone
asset — that would keep two copies of the same guidance in different files, which is the
condition that produced the duplication in the first place.

**D4 — Hook bodies move; hook installation does not.** Marker chaining, mode bits, and
gitdir resolution are mechanism: a bug there silently breaks a user's existing hooks or
leaves a hook non-executable. Wording of the feedback text is content. The split follows
that line. It also lets the hook assets be covered by the same script audits as
`broker.sh` and `sweep.sh`.

**D5 — Extend the leak audit to vendor product names in the same change.** The audit is
what makes the relocation worth doing; relocating prose without widening the audit would
move the text under an enforcement that does not yet check for the leak class that
motivated this (`Claude Code`, `.claude/`). Allowed spans keep a deliberate CLI
enumeration legal.

## Risks / Trade-offs

- **A wide diff across seven files invites a rubber-stamp review.** → D1's byte-identical
  assertions are the real review: if they pass, no agent-visible behaviour changed. Land
  the assertions before the relocations, not after.
- **Content could be edited in transit "while we're in there".** → Explicitly out of
  scope; the in-flight changes own content edits. Any content delta should fail D1.
- **Ordering against three in-flight changes.** `peer-cherry-pick-removal` must land
  first (it rewrites inter-agent rules content). `speckit-spec-path-pointer` and
  `worker-guidance-skill-export` touch adjacent text. → Sequence deliberately; do not
  start the inter-agent-rules relocation before the cherry-pick change has merged.
- **The widened audit may fail on existing assets.** Plausible — the assets have never
  been checked for product names. → Expected and desirable; triage each hit as either a
  real leak to fix or a span to mark allowed. Do not weaken the audit to make it pass.
