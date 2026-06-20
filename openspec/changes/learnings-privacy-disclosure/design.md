## Context

Learnings mode is already privacy-preserving by construction: the aggregator writes only `.git-paw/session-learnings.md`, the broker binds to `127.0.0.1`, and the crate has no outbound HTTP client. The gap is purely communicative — neither the docs nor the CLI tell the user this, nor that the file exists so they can optionally share it to improve git-paw. `src/main.rs` already computes `learnings_enabled` at session start, so the notice has a natural injection point.

## Goals / Non-Goals

**Goals:**
- Make the no-telemetry / local / opt-in stance explicit in docs and at the CLI.
- Frame the learnings file as an optional, user-initiated contribution channel (GitHub issue), with a clear review-and-anonymise caveat.

**Non-Goals:**
- No change to what the aggregator collects, the file format, or the broker.
- No automated upload, no telemetry, no "share now" command — sharing stays a manual, user-driven action.
- No special GitHub issue template (generic issues link only).

## Decisions

- **Notice location:** print at session start, gated on the existing `learnings_enabled` flag in `src/main.rs`, so it appears exactly when the user has opted in and never otherwise.
- **Notice content:** local path + "nothing is sent anywhere" + optional-share-via-issue + review/anonymise caveat (LLM can help). Keep it to a few lines so it doesn't bury the session-start output.
- **Anonymisation guidance is advisory, not enforced:** git-paw does not attempt to scrub the file itself — it points the user at reviewing it (optionally with their own LLM) because only the user knows what is sensitive in their repo.

## Risks / Trade-offs

- **Notice noise:** an extra few lines at session start for opted-in users. Mitigated by keeping it concise and only on opt-in.
- **Stale link risk:** the GitHub issues URL must track the canonical repo (`bearicorn/git-paw`); keep it consistent with README/other docs links.
