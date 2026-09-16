## 1. Sandbox chapter (`docs/src/user-guide/sandbox.md`)

- [x] 1.1 Add a `### What breaks under the sandbox` subsection immediately after the `/private/tmp` heredoc paragraph in the macOS section: setuid/setgid exec is refused by `sandbox-exec` (`/bin/ps`, `/usr/bin/top`), the exact symptom (`execvp() of '/bin/ps' failed: Operation not permitted`, seen by an agent as a bare `operation not permitted: ps`), no allow-rule fixes it and it is not a mis-tuned profile, `pgrep` works (not setuid)
- [x] 1.2 Add the Linux clause: `bwrap` runs setuid binaries without elevated privileges (`no_new_privs`); `ps` is not setuid on Linux and works; add a one-line cross-reference from the Linux section's heredoc paragraph
- [x] 1.3 Add a sentence noting the bundled worker-orientation skill tells agents the same, so they do not probe or widen the profile
- [x] 1.4 `mdbook build docs/` succeeds; no `SUMMARY.md` change

## 2. Doc guard (`tests/security_posture_docs.rs`)

- [x] 2.1 Add `setuid` and `/bin/ps` to `sandbox_chapter_covers_the_required_setup`'s needle list with the usual inline comment
- [x] 2.2 Confirm the test fails against the pre-change chapter (temporarily remove the note or run against `git stash`) and passes after; `cargo fmt`; `just check` green
