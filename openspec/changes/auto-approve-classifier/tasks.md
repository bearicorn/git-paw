## 1. Command-slice extraction

- [ ] 1.1 Add a helper that extracts the prompted command slice (text between the `Bash command` / `Bash(` header and the confirmation question) from a pane capture, ignoring surrounding narration
- [ ] 1.2 Unit test: command slice is extracted from a realistic capture and prose elsewhere is ignored (maps to "Narration about a dangerous command is not classified as danger")

## 2. Curated danger-list

- [ ] 2.1 Define the shared (OS-independent) danger base constant (`rm -rf`/`rm -fr`, `git push`, `--force`/`force-push`, `reset --hard`, `git rebase`, branch-switching `git checkout `, `branch -D`, `git worktree remove`, `clean -fd`/`clean -fdx`, `sudo `, `mkfs`, `dd if=`, `> /dev/`, `chmod -R`, `chown -R`, `pkill`/`kill`), exported for tests
- [ ] 2.2 Add `os_addendum()` returning the macOS addendum (`diskutil`, `/Volumes/…` deletes, `rm -rf ~/Library/…`) on macOS and the Linux addendum (`mkfs*`, `/dev/sd*`, `/dev/nvme*`) on Linux/WSL, compiled per-OS
- [ ] 2.3 Implement `is_dangerous(command_slice)` evaluating base + os addendum against the command slice
- [ ] 2.4 Wire danger-first precedence into the classifier so a danger match is a terminal escalate that overrides any whitelist / safe-by-pattern match
- [ ] 2.5 Unit tests: force-push, hard reset, branch switch, privileged/device commands, process-kill, macOS diskutil (macOS-gated), Linux device write (Linux-gated), and danger-overrides-whitelist scenarios

## 3. rm -rf scratch-path exception

- [ ] 3.1 Implement scratch-path recognition (`/tmp/paw-*`, `/private/tmp/paw-*`, `$TMPDIR`-rooted `paw-*`, paths under `.git-paw/tmp/`)
- [ ] 3.2 Resolve `rm -rf "$VAR"` against captured env / preceding `VAR=…` assignment; fail-safe (escalate) when unresolved
- [ ] 3.3 Apply the exception: escalate `rm -rf`/`rm -fr` UNLESS every target is scratch (mixed targets escalate)
- [ ] 3.4 Unit tests: `/tmp/paw-*` approve, `/private/tmp/paw-*` approve, `.git-paw/tmp/` approve, `$VAR`→scratch approve, `~/Documents` escalate, mixed targets escalate

## 4. Read-mostly verb allowlist

- [ ] 4.1 Add the read-mostly verb allowlist (`curl cat ls grep rg git echo sed awk find wc head tail jq mkdir touch openspec just export tmux env`) into the built-in safe-command whitelist, subordinate to the danger-list
- [ ] 4.2 Unit tests: read-mostly verb classifies safe; danger match still wins over the whitelisted `git` verb

## 5. Worktree-confined git add / git commit pre-approval

- [ ] 5.1 Extend the worktree-confined classification (reusing the canonicalize-then-`starts_with(worktree_root)` check) to cover `git add` and `git commit` prompts
- [ ] 5.2 Ensure `git push` is NOT pre-approved (danger-list wins)
- [ ] 5.3 Unit tests: in-worktree `git commit` approves, in-worktree `git add` approves, `git push` escalates despite worktree confinement

## 6. Live-prompt gate

- [ ] 6.1 Implement the live-prompt check: footer marker `Esc to cancel` present within the last ~4 non-blank lines of the capture
- [ ] 6.2 Make the live-prompt check a precondition for any keystroke dispatch in the auto-approver
- [ ] 6.3 Unit tests: live prompt fires; footer absent does not fire; footer scrolled out of the last ~4 lines does not fire

## 7. Option-index selection and broad-grant rule

- [ ] 7.1 Detect prompt shape (2-option Yes/No vs 3-option Yes / Yes-don't-ask / No)
- [ ] 7.2 Define the arbitrary-code-runner predicate (`python`, `bash -c`, `sh -c`, `eval`, `node`, bare ` -c `)
- [ ] 7.3 Implement option selection: 2-option → option 1; 3-option → option 2 only when verb is allowlisted AND not arbitrary-code, else option 1
- [ ] 7.4 Unit tests: 2-option selects Yes; 3-option allowlisted selects broad grant; `python3 -c` and `bash -c` select one-time Yes (never broad grant)

## 8. sweep.sh helper parity

- [ ] 8.1 Mirror the danger-list, scratch exception, live-prompt gate, and option-index selection in `assets/scripts/sweep.sh` so the bundled helper classifies identically to the Rust path
- [ ] 8.2 Add fixture-driven tests (via a `classify`/`stuck-eval`-style subcommand) covering one danger pattern, the scratch approve, the worktree commit approve, and the non-live no-op

## 9. Docs and quality gates

- [ ] 9.1 Update `--help` / supervisor docs and mdBook chapter describing the escalate-vs-auto-approve rules, scratch exception, and arbitrary-code policy
- [ ] 9.2 Run `just check` (fmt + clippy + tests) and `just deny`; confirm no `unwrap()`/`expect()` in non-test code and all public items have doc comments
- [ ] 9.3 Confirm every spec scenario maps to at least one behavioral test; `mdbook build docs/` succeeds
