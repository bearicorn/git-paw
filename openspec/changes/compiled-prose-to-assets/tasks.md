## 1. Sequencing and safety net

- [ ] 1.1 Confirm `peer-cherry-pick-removal` has merged — it rewrites the inter-agent rules content this change relocates (D-ordering).
- [ ] 1.2 Check `speckit-spec-path-pointer` and `worker-guidance-skill-export` status; coordinate to avoid conflicting edits to adjacent text.
- [ ] 1.3 **Before any relocation**, add byte-identical render assertions for each block to be moved (D1). These are the acceptance test for the whole change.

## 2. Relocate prose to assets

- [ ] 2.1 `src/agents.rs:147-181` inter-agent rules → asset with a peer-list placeholder. Keep the peer-list join in Rust as a substitution value.
- [ ] 2.2 `src/skills.rs:412-431` `DRIVE_LOOP_DIRECTIVE` → a marker-delimited region inside `supervisor.md`, reusing `render_opsx_regions` (`src/skills.rs:692`) with a second marker pair (D3). Remove the duplicate restatement at `supervisor.md:300-315` so one copy remains.
- [ ] 2.3 `src/skills.rs:798-815` spec-path doctrine sentences → per-backend assets. Keep the dedupe and multi-backend join in Rust. Do NOT change where they point.
- [ ] 2.4 `src/skills.rs:877-879` governance section header → asset. Keep the bullet loop in Rust.
- [ ] 2.5 `src/main.rs:348-376` boot-prompt framing → per-backend asset with placeholders.
- [ ] 2.6 `src/commands/supervisor.rs:742-748` supervisor framing prompt → fold into the supervisor skill's boot block.
- [ ] 2.7 `src/specs/speckit.rs:391-442` and `src/specs/superpowers.rs:121-136` task-prompt section scaffolding → per-backend templates. Keep `parse_tasks_md`, phase grouping and `[P]` detection in Rust. Keep heading text identical — prompt tests assert those strings.
- [ ] 2.8 After each relocation, run its byte-identical assertion from 1.3.

## 3. Hook bodies

- [ ] 3.1 `src/agents.rs:493-584` hook bodies → `assets/hooks/{post-commit,pre-push,pre-commit}.sh`, embedded at compile time.
- [ ] 3.2 Keep marker chaining, mode bits and common-vs-linked gitdir resolution compiled (D4).
- [ ] 3.3 Verify installed-hook behaviour is unchanged: commit publishes artifact, broker failure does not block the commit, existing user hook content is preserved, no-op without the marker file.
- [ ] 3.4 `bash -n` each hook asset.

## 4. Keep compiled — verify not moved

- [ ] 4.1 Confirm `render_dev_allowlist_preset` (`src/skills.rs:738-769`) still generates from the allowlist constant (D2).
- [ ] 4.2 Add the scenario test asserting the dev-allowlist enumeration is not asset-sourced.
- [ ] 4.3 Confirm the `{{CHANGE_ID}}` placeholder whitelist (`src/skills.rs:641`) is untouched.
- [ ] 4.4 Confirm `src/agents.rs:518` branch-guard feedback text behaviour is preserved through the hook relocation.

## 5. Extend the leak audit

- [ ] 5.1 Include every bundled asset carrying agent-facing prose in the audited set, not only the supervisor skill.
- [ ] 5.2 Add vendor product names to the forbidden list — at minimum `Claude Code` and `.claude/` — with an allowed-span mechanism for a deliberate CLI enumeration (D5).
- [ ] 5.3 Run the widened audit and triage every hit as either a real leak to fix or a span to mark allowed. Do NOT weaken the audit to make it pass.
- [ ] 5.4 Test: a product-name leak added outside an allowed span fails the audit and names the offending token and location.

## 6. Docs

- [ ] 6.1 Contributing guide: where agent-facing text lives and why (assets, not compiled strings).
- [ ] 6.2 Update the export-agnosticism dev skill's export-surface inventory to list the new asset locations.
- [ ] 6.3 `mdbook build docs/` succeeds.

## 7. Quality gates

- [ ] 7.1 `cargo fmt` before committing.
- [ ] 7.2 `just check` — verify by real exit code, not piped output.
- [ ] 7.3 Confirm every byte-identical assertion from 1.3 passes; any failure means content changed in transit — investigate rather than update the expectation.
- [ ] 7.4 Confirm `include_str!` embedding is preserved for every new asset — the binary must render with no filesystem assets present.
