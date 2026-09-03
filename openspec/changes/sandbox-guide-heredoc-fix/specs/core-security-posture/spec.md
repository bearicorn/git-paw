## ADDED Requirements

### Requirement: The documented sandbox profile permits shell heredoc temp files

The documented FS-scoped sandbox profile SHALL grant write access to the shell's temporary directory used for heredocs — `/private/tmp` on macOS (`sandbox-exec`) and the bound `TMPDIR`/`/tmp` on Linux (`bwrap`) — so the standard `git commit -m "$(cat <<'EOF' … EOF)"` heredoc idiom works under the sandbox. A profile that grants only `$TMPDIR` and the CLI's per-session temp path is insufficient, because `zsh` writes heredoc bodies under `/private/tmp/zsh*`; denying that path breaks every heredoc-based commit. The FS-scoped sandbox chapter SHALL document this grant and state why it is required.

#### Scenario: The sandbox chapter documents the heredoc temp-dir grant

- **WHEN** the FS-scoped sandbox chapter is inspected
- **THEN** its example macOS profile SHALL grant write access to `/private/tmp`
- **AND** the chapter SHALL note that this grant is required for shell heredocs (so the standard `git commit -m "$(cat <<'EOF')"` idiom works)

#### Scenario: The Linux profile binds the shell temp dir writable

- **WHEN** the FS-scoped sandbox chapter's Linux `bwrap` example is inspected
- **THEN** it SHALL bind the shell temporary directory (`TMPDIR`/`/tmp`) writable, so heredocs work under `bwrap` as well
