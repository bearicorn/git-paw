# core-security-posture Specification

## Purpose
State git-paw's trust model once, honestly, and document the optional
containment that bounds it. git-paw runs AI agents that execute arbitrary code
with the user's privileges — often in full-auto — so this capability owns the
consolidated security disclaimer (what the FS-scoped sandbox, the auto-approve
classifier, the command allowlists, the protected-paths rule, and remote control
each do and, critically, do NOT guarantee) and the documented FS-scoped sandbox
workflow: a per-OS wrapper (`sandbox-exec` / `bwrap`) applied through the
existing custom-CLI seam that confines an agent's writes to its worktree.
git-paw neither invokes nor ships a sandbox profile — the sandbox is the user's
own local config — but `git paw doctor` reports backend availability and points
at this posture.
## Requirements
### Requirement: Consolidated security disclaimer and trust-model posture

git-paw SHALL present a single consolidated security disclaimer, reachable from
the README, the mdBook user guide, and a `git paw doctor` pointer. The disclaimer
SHALL state plainly that git-paw runs AI agents which can execute arbitrary code
(including in full-auto with skipped permissions), and SHALL describe the blast
radius of that execution. It SHALL say what each control does and does **not**
guarantee: the agent CLI's own permission system (the approval level git-paw maps
to launch flags, and the CLI's own allow rules), the FS-scoped sandbox, and remote
control. It SHALL state that git-paw itself makes no decision about whether an
agent's command is safe and approves no permission prompt on its own, so every
prompt the CLI raises is answered by a human. The disclaimer MUST be stated once as
the authoritative source rather than scattered per feature, and MUST NOT overstate
any control's guarantee.

**Test:** `tests/security_posture_docs.rs`

#### Scenario: Disclaimer is present and reachable
- **WHEN** a reader opens the README or the mdBook user guide
- **THEN** a security disclaimer is present that describes arbitrary-code execution, the blast radius, and the guarantees and non-guarantees of the agent CLI's permission system, the sandbox, and remote control
- **AND** it states that git-paw approves no permission prompt on its own

#### Scenario: Doctor points at the posture
- **WHEN** `git paw doctor` runs
- **THEN** its output references where to read the consolidated security posture

#### Scenario: Controls are not overstated
- **WHEN** the disclaimer describes the agent CLI's permission system or the FS-scoped sandbox
- **THEN** it states their limits: a CLI allow rule is only as narrow as it is written and a full-auto mode skips the prompts entirely; the sandbox protects the machine around the worktree, not the worktree's own contents, and by default protects write-integrity only — not the confidentiality of host secrets — and does not cover network, resource-exhaustion, or exposed-socket vectors unless explicitly added

#### Scenario: No classifier is presented as a control
- **WHEN** the disclaimer's control list is inspected
- **THEN** it SHALL NOT present a git-paw command classifier, a git-paw command allowlist, or a git-paw protected-paths rule as a control

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

#### Scenario: PATH-resolvable wrapper requirement is documented
- **WHEN** the docs explain how to point a CLI at the wrapper
- **THEN** they state that git-paw launches a custom CLI by its PATH name, so the wrapper must be a PATH-resolvable executable, not only a path assigned to `command`

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

### Requirement: The documented sandbox profile permits shell heredoc temp files

The documented FS-scoped sandbox profile SHALL grant write access to the shell's temporary directory used for heredocs — `/private/tmp` on macOS (`sandbox-exec`) and the bound `TMPDIR`/`/tmp` on Linux (`bwrap`) — so the standard `git commit -m "$(cat <<'EOF' … EOF)"` heredoc idiom works under the sandbox. A profile that grants only `$TMPDIR` and the CLI's per-session temp path is insufficient, because `zsh` writes heredoc bodies under `/private/tmp/zsh*`; denying that path breaks every heredoc-based commit. The FS-scoped sandbox chapter SHALL document this grant and state why it is required.

#### Scenario: The sandbox chapter documents the heredoc temp-dir grant

- **WHEN** the FS-scoped sandbox chapter is inspected
- **THEN** its example macOS profile SHALL grant write access to `/private/tmp`
- **AND** the chapter SHALL note that this grant is required for shell heredocs (so the standard `git commit -m "$(cat <<'EOF')"` idiom works)

#### Scenario: The Linux profile binds the shell temp dir writable

- **WHEN** the FS-scoped sandbox chapter's Linux `bwrap` example is inspected
- **THEN** it SHALL bind the shell temporary directory (`TMPDIR`/`/tmp`) writable, so heredocs work under `bwrap` as well

### Requirement: The sandbox chapter documents what the FS sandbox breaks

The FS-scoped sandbox chapter SHALL carry a "what breaks under the sandbox" note, adjacent to the heredoc temp-dir explanation, stating that setuid/setgid binaries cannot be executed under `sandbox-exec` — naming macOS `/bin/ps` (and `/usr/bin/top`) — with the observed symptom (`execvp() … Operation not permitted`, surfacing to an agent as a bare `operation not permitted: ps`), that no profile allow-rule fixes it because the refusal is not a file-write denial, that it is expected behaviour rather than a mis-tuned profile, and which non-setuid alternatives work (`pgrep`). The note SHALL add that on Linux `bwrap` runs setuid binaries without their elevated privileges while `ps` itself is not setuid there and works, and SHALL point out that the bundled worker-orientation skill already tells agents the same thing.

#### Scenario: The sandbox chapter documents the setuid exec refusal

- **WHEN** the FS-scoped sandbox chapter is inspected
- **THEN** it SHALL contain a note that setuid binaries such as macOS `/bin/ps` fail under `sandbox-exec` with `Operation not permitted`
- **AND** it SHALL state that no profile rule fixes this and that it is expected
- **AND** it SHALL name `pgrep` as a working non-setuid alternative

#### Scenario: The note sits beside the heredoc explanation and covers Linux

- **WHEN** the FS-scoped sandbox chapter is inspected
- **THEN** the setuid note SHALL appear in the same "what breaks" context as the `/private/tmp` heredoc grant explanation
- **AND** it SHALL state how `bwrap` treats setuid binaries on Linux and that `ps` works there

#### Scenario: The doc test guards the note

- **WHEN** `tests/security_posture_docs.rs` runs against a sandbox chapter lacking the setuid note
- **THEN** the sandbox-chapter test SHALL fail, naming the missing needle

### Requirement: Security posture documents the broker's browser-exposure guard

The security-posture chapter SHALL state four things about the broker:

- the broker refuses requests from browser origins and from non-loopback `Host` names when it is bound to loopback, and why: DNS rebinding and prompt injection into agents that run with auto-approved execution;
- a non-loopback `bind` disables the Host check and exposes an unauthenticated broker to the network;
- the guard is not authentication, because any local process can still reach the broker;
- the migration for reverse proxies.

#### Scenario: Posture chapter covers the guard and its limits

- **WHEN** `docs/src/user-guide/security-posture.md` is read
- **THEN** it SHALL describe the Host and Origin checks, the DNS-rebinding threat they address, the non-loopback-bind exception, and the statement that local processes are not authenticated by this guard

### Requirement: Security posture documents hook bypass and tmux driving

The security-posture chapter SHALL describe two ways an agent session can step around git-paw's own safeguards with ordinary commands, and what git-paw does and does not do about them:

- bypassing git-paw's git hooks with a per-invocation hook override (`git -c core.hooksPath=…`) or `--no-verify`, which silently drops the branch guard and the commit report;
- driving the session's tmux panes (`tmux send-keys`, `tmux run-shell`, `tmux pipe-pane`), which can answer a permission prompt from outside the agent's CLI and so bypass the human and the orchestrator.

The chapter SHALL state that the bundled coordination skill forbids both; that git-paw writes no permission rule for these commands, because it pre-approves only its own helper workflow and leaves every other permission decision to the agent CLI and the human; and that a user who wants these forms to prompt or be refused can add their own ask or deny rules to the agent CLI's settings, which git-paw never writes, edits or removes.

#### Scenario: Posture chapter names both risks and the user's own remedy

- **WHEN** `docs/src/user-guide/security-posture.md` is read
- **THEN** it SHALL name `core.hooksPath`, `--no-verify`, `tmux send-keys` and `tmux run-shell`
- **AND** it SHALL state that git-paw writes no rule for them and that the user may add their own ask or deny rules

Test: `tests/security_posture_docs.rs::posture_chapter_documents_hook_bypass_and_tmux_driving`

