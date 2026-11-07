# Claude Code usage-report fixtures

Hand-written from the record shape observed in Claude Code 2.1.322 session
transcripts; no real transcript content. Only the fields the usage parser reads
are meaningful, and every content value is `REDACTED`.

Observed facts:

- **Location:** `<config dir>/projects/<cwd slug>/<session id>.jsonl`, where the
  config dir is `CLAUDE_CONFIG_DIR` when set (else `~/.claude`) and the slug is
  the worktree path with every non-alphanumeric character replaced by `-`.
  Subagent turns are written to `<session id>/subagents/agent-*.jsonl` beside it.
- **Usage record:** each `"type":"assistant"` line carries `message.usage` with
  `input_tokens`, `output_tokens`, `cache_read_input_tokens` and
  `cache_creation_input_tokens`. One API response is split over several lines
  (one per content block) that repeat the same `message.id` and the same usage,
  so totals count each `message.id` once.
- **Cost:** no cost figure is written.

| File | Totals (input / output / cache read / cache write) |
|---|---|
| `session.jsonl` | 5 / 150 / 2500 / 520 |
| `session.jsonl` + `subagent.jsonl` | 10 / 160 / 2500 / 820 |
| `malformed.jsonl` | unknown (a usage field is not a number) |
