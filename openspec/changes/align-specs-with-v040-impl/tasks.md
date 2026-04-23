## 1. Capture deltas

- [x] 1.1 `proposal.md` written
- [x] 1.2 `specs/broker-server/spec.md` modifies `start_broker` signature and `BrokerState` sharing model
- [x] 1.3 `specs/dashboard/spec.md` modifies `run_dashboard` signature, adds `shutdown` flag scenario, adjusts tick cadence wording
- [x] 1.4 `specs/error-handling/spec.md` extends `BrokerError::PortInUse` with `source` field
- [x] 1.5 `specs/configuration/spec.md` aligns `[specs]` field names with implementation and `spec-scanning`

## 2. Verification

- [ ] 2.1 `openspec validate align-specs-with-v040-impl --strict` passes
- [ ] 2.2 Cross-check that no production code change is needed (this change only updates spec text; impls already match)
- [ ] 2.3 Confirm pending changes (`prompt-inbox`, `supervisor-messages`) cover the residual broker-messages and dashboard-keybind drift via their own deltas; archive ordering documented
- [ ] 2.4 After archive, main specs reflect the as-built v0.4.0 surface and CLAUDE.md "match specs exactly" rule is satisfied for the four modified capabilities

## 3. Archive

- [ ] 3.1 Once `prompt-inbox` and `supervisor-messages` are archived (they update `dashboard` and `broker-messages` cumulatively), archive this change so `openspec/specs/{broker-server,dashboard,error-handling,configuration}/spec.md` reflect v0.4.0
