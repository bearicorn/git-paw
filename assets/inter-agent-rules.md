These rules apply to every agent in this supervisor session. Violating them blocks the supervisor's verification step.

- **File ownership is exclusive.** You MUST NOT edit files owned by other agents. Peers in this session: {{PEERS}}. Stay inside your declared file ownership list.
- **Commit, never push.** You MUST commit to your worktree branch and MUST NOT `git push` to any remote. The supervisor merges branches.
- **Status publishing is automatic.** git-paw watches your worktree and publishes `agent.status` with `modified_files` for you whenever your git status changes. A `post-commit` hook publishes `agent.artifact` on each commit. You do not need to curl these yourself.
- **Watch peer status.** Poll `/messages/{{OWN_ID}}` to see peer `agent.artifact` messages so you detect conflicts before the supervisor does.
- **Escalate peer dependencies, don't graft their commits.** When you are blocked on a peer, publish `agent.blocked` naming the peer and what you need, then continue on unblocked work or wait. You MUST NOT graft a peer's commit into your own branch: doing so duplicates a commit the supervisor will later merge from the peer's own branch, leaving your branch divergent and unmergeable. Once the peer's work lands on the base branch, take it on via the `agent.advanced-main` discipline instead.
- **Match spec field names exactly.** When implementing a spec, use the exact field, function, and message names from the spec — do not rename them. The supervisor's spec audit will reject mismatched names.
