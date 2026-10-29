## ADDED Requirements

### Requirement: The posture documents remote workers

The security posture chapter SHALL state, for remote workers: that the task prompt, the orchestrator's relayed answers and failing verification output are sent to the vendor; that the vendor's agent works on the repository through the vendor's own GitHub access, outside any sandbox git-paw configures; that git-paw never pushes, never opens a pull request, never approves a plan and cannot cancel a vendor session; that the vendor's patch is untrusted, is checked (paths, symbolic links, submodules, size) before it touches the worktree, and that verifying it runs the vendor's code locally with the user's privileges, as a local worker's code runs; that the vendor key is read only from the environment and never stored, and that processes git-paw starts in panes or headless turns inherit the user's environment and can therefore read it; that vendor text relayed to the orchestrator is untrusted input; and the vendor's data use as its reviewed privacy notice states it, which for Jules is that sessions on public repositories may be used to train Google's models, that sessions on private repositories are not used for training, and that trained reviewers may read samples.

#### Scenario: The posture names what leaves the machine

- **WHEN** the security posture chapter's remote-workers section is read
- **THEN** it SHALL name the task prompt, relayed answers and failing verification output as sent to the vendor

#### Scenario: The posture names the untrusted patch and the key's residual exposure

- **WHEN** the security posture chapter's remote-workers section is read
- **THEN** it SHALL state that the patch is checked before it touches the worktree and that verifying it runs the vendor's code locally
- **AND** it SHALL state that processes in panes or headless turns inherit the user's environment, including the vendor key when it is exported there

#### Scenario: The posture names the vendor's training use

- **WHEN** the security posture chapter's remote-workers section is read
- **THEN** it SHALL state that Jules sessions on public repositories may be used to train Google's models, that sessions on private repositories are not, and that trained reviewers may read samples
