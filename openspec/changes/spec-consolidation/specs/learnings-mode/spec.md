## REMOVED Requirements

### Requirement: No agent.learning broker variant in v0.5.0

**Reason:** Superseded and now self-contradictory. This negative requirement (frozen from
the v0.5.0 delta) is directly contradicted by the `agent-learning-variant` capability, which
defines the `agent.learning` `BrokerMessage` variant shipped in a later release. Keeping it
as a permanent requirement would freeze a statement into the v1.0.0 contract that the code
already violates by design.

**Migration:** The positive contract for the learnings wire surface lives in
`agent-learning-variant` (the `agent.learning` variant, dual markdown+broker output, and MCP
`get_learnings` consumption). No behavior changes — this removes an obsolete constraint only.
The forward-design note it carried (that the aggregator's internal data model be serialisable
to a broker variant without re-deriving from messages) is preserved as a positive statement in
the merged `learnings` capability. When `learnings-mode` merges into `learnings`, this removal
is applied first so the merged doc carries no contradiction.
