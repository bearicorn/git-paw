## MODIFIED Requirements

### Requirement: Documented FS-scoped sandbox workflow

git-paw SHALL document a per-operating-system FS-scoped sandbox workflow that
confines an agent CLI's writes to its worktree by wrapping the CLI through the
existing `[clis.<name>].command` seam — `sandbox-exec` on macOS, `bwrap` on
Linux and WSL. The sandbox is the **user's own local configuration**: git-paw
neither invokes it nor ships a maintained profile; it documents a worked,
copy-and-adapt example and the setup requirements proven necessary in practice.
The documentation MUST make clear that the example profile is a starting point
the reader adapts to their CLI, OS, and stack.

**Test:** `tests/security_posture_docs.rs`

#### Scenario: Both supported operating systems are documented
- **WHEN** a reader follows the sandbox chapter
- **THEN** it provides a worked macOS (`sandbox-exec`) example and a worked Linux/WSL (`bwrap`) example, each confining writes to the worktree while leaving reads and network open, framed as adapt-to-your-setup starting points

#### Scenario: The wrapper is pointed at by `command`
- **WHEN** the docs explain how to point a CLI at the wrapper
- **THEN** they state that git-paw launches the executable `command` resolves to, so `command` may be the wrapper's path and the wrapper need not be on PATH under the entry's name
- **AND** they state that a wrapper kept on PATH under the entry's name must be the same file `command` resolves to, or git-paw refuses to launch that CLI rather than guess which one was meant

#### Scenario: The writable allow-list is documented
- **WHEN** the docs describe the sandbox profile
- **THEN** they enumerate the required writable set — the worktree; the shared `.git` for commits with `.git/hooks` and `.git/config` kept read-only to close the persistence gap; the CLI's own config/auth and per-session temp/cwd paths; `TMPDIR`; device nodes; and toolchain caches — and note that network egress (loopback broker + model API) stays open

#### Scenario: The git persistence gap is explained
- **WHEN** the docs describe binding `.git`
- **THEN** they explain that `.git/hooks` and `.git/config` must be read-only so a confined agent cannot plant a hook or `core.pager`/`editor` that would later run unsandboxed with full privileges

#### Scenario: Credential-refresh behavior is documented per OS
- **WHEN** the docs describe running a long-lived agent under the sandbox
- **THEN** they explain that on macOS a CLI whose OAuth token lives in the login Keychain needs the Keychain writable for token refresh (or the token expires mid-run), that this grants broad Keychain access as a tradeoff, and that Linux credential stores (file- or daemon-based) do not have this constraint

#### Scenario: Confidentiality hardening tiers are documented
- **WHEN** the docs present the example profile
- **THEN** they explain that the default write-integrity profile does not restrict reads, and offer read-hardening options — a blocklist of secret paths (e.g. `~/.ssh`, cloud credentials) and a stronger deny-home-then-allowlist approach — noting which host paths the agent legitimately needs and that a worktree-only worker does not require `~/.ssh`
