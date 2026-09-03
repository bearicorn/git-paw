## 1. Sandbox profile fix

- [ ] 1.1 Add `(subpath "/private/tmp")` to the macOS `sandbox-exec` write allow-list in `docs/src/user-guide/sandbox.md`, with an explanatory comment (shell heredocs)
- [ ] 1.2 Add a prose note explaining the grant and the `git commit -m "$(cat <<'EOF')"` idiom it enables
- [ ] 1.3 Confirm the Linux `bwrap` example binds `TMPDIR`/`/tmp` writable (it already `--bind`s `${TMPDIR:-/tmp}`); add a heredoc note if needed

## 2. Docs-parity guard

- [ ] 2.1 Extend `tests/security_posture_docs.rs` to assert the sandbox chapter grants `/private/tmp` and mentions heredocs
- [ ] 2.2 `mdbook build docs/` succeeds

## 3. Gates

- [ ] 3.1 `just check` green (docs-parity test passes)
- [ ] 3.2 Gate-5 note: the grant is write-only to a world-writable, secret-free temp dir; read-confidentiality tiers unchanged
