# v0.5.0 release-notes bullets (conflict-detection)

These bullets are intended to be copied into the v0.5.0 release-prep
commit's CHANGELOG.md / archive plan. They cover the new conflict
detector subsystem and its configuration surface.

## Highlights

- Broker auto-detects forward / in-flight / ownership conflicts when
  supervisor mode is enabled. Auto-emitted feedback is tagged
  `[conflict-detector]` and uses `from: "supervisor"`.
- Configurable via the `[supervisor.conflict]` table — `window_seconds`
  (default `120`), `warn_on_intent_overlap` (default `true`),
  `escalate_on_violation` (default `true`).

## Follow-up

- MILESTONE.md item #15 (v0.4 `supervisor.md` "### Conflict detection"
  manual-comparison section) is resolved by the supervisor-skill rewrite
  shipped in this change.
