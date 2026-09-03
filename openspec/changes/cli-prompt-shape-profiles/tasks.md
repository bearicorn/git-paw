## 1. Profile surface

- [ ] 1.1 Define the profile type covering readiness markers, approval markers, live-prompt markers, mid-response markers, command-header form, file-prompt pattern, option-line form, broad-grant marker, input sigil, mode markers, stream-error markers, context-bloat marker, paste-buffer marker.
- [ ] 1.2 Keep the serialized shape flat enough for `_paw_common.sh` to read without a real parser (D3).
- [ ] 1.3 Embed the Claude Code profile as the compiled default, populated verbatim from today's constants.
- [ ] 1.4 Implement per-field fallback to the embedded default (D2); an absent field SHALL NOT resolve empty.
- [ ] 1.5 Resolution for an unrecognised CLI returns the embedded default, never an empty profile.
- [ ] 1.6 Tests: partial profile falls back per field; unknown CLI resolves the default; single-binary resolution works with no profile file present.

## 2. Security boundary (do this before the migrations)

- [ ] 2.1 Confirm the profile type has no field capable of expressing a permission decision.
- [ ] 2.2 Adversarial test: a profile attempting to declare a dangerous command safe does not change its classification.
- [ ] 2.3 Adversarial test: a profile attempting to make an arbitrary-code-runner eligible for a durable grant does not change option selection.
- [ ] 2.4 Confirm the danger list, protected-path rules, worktree-boundary resolution, broad-grant rule and approval send gate remain compiled and unreachable from configuration.

## 3. Migrate the compiled marker sites

- [ ] 3.1 `src/tmux/readiness.rs:23-30` `CLI_READY_MARKERS` → profile lookup.
- [ ] 3.2 `src/supervisor/permission_prompt.rs:53` `APPROVAL_MARKERS` → profile lookup.
- [ ] 3.3 `src/supervisor/drive.rs:1786` `MID_RESPONSE_MARKERS` → profile lookup.
- [ ] 3.4 `src/supervisor/manual_approvals.rs:316` `PROMPT_BOILERPLATE` → profile lookup.
- [ ] 3.5 `src/supervisor/auto_approve.rs` — file-prompt regex (`:120`), `do you want to` (`:387`), `Bash command` / `Bash(` header (`:394-417`), broad-grant marker → profile lookup. Leave the surrounding safety logic untouched.
- [ ] 3.6 `src/coordination/inventory.rs:246-256` mode markers → profile lookup.
- [ ] 3.7 After each migration, run the existing detection tests unmodified. A test that needs editing is a red flag — investigate before changing it.

## 4. Helper-script parity

- [ ] 4.1 `assets/scripts/_paw_common.sh` — read the resolved profile (follow the `discover_supervisor_int` precedent at `sweep.sh:158`).
- [ ] 4.2 `assets/scripts/sweep.sh` — remove the local copies: context-bloat regex (`:269`), `Pasted text #[0-9]` (`:1035`), and the marker definitions around `:243-269`.
- [ ] 4.3 Rework the live-gate parity test from "two copies agree" to "both consumers read one source".
- [ ] 4.4 `bash -n` both scripts after editing (embedded-heredoc quoting is fragile here).
- [ ] 4.5 Sync the tracked `.git-paw/scripts/sweep.sh` copy — the feature branch edits `assets/`, but the tracked copy is what a live dogfood session executes.

## 5. Docs

- [ ] 5.1 Profile reference chapter: every field, what it matches, and where it is used.
- [ ] 5.2 Authoring guide for adding a CLI profile, stating explicitly that profiles carry no permission policy.
- [ ] 5.3 Configuration reference entry.
- [ ] 5.4 Note in supported-CLIs that no speculative profiles ship (D5) and an unprofiled CLI falls back to the embedded default.
- [ ] 5.5 `mdbook build docs/` succeeds.

## 6. Quality gates

- [ ] 6.1 `cargo fmt` before committing.
- [ ] 6.2 `just check` — verify by real exit code, not piped output.
- [ ] 6.3 Grep `src/` for remaining vendor product literals (`Claude Code`, `don't ask again`, `accept edits`, `Pasted text`, `Bypassing Permissions`) and confirm each surviving hit is either profile data or a test fixture.
- [ ] 6.4 Security review against `.agents/skills/security-and-safety-review/SKILL.md`, focused on the D1 boundary.
