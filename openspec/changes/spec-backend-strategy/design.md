## Context

The four backends (`OpenSpecBackend`, `MarkdownBackend`, `SpecKitBackend`, `SuperpowersBackend`)
already exist as `SpecBackend` impls, and every `SpecEntry` already carries its `SpecBackendKind`.
What a format *means* beyond scanning lives at the call sites listed in the proposal. The roadmap
mentions a match in `speckit.rs`; there is none — that file only constructs entries.

Two quirks the refactor must keep:

- The conventional directory is supplied only when the format comes from `--specs-format`
  (`resolve_specs_config`); a `[specs]` section with a `type` but no `dir` scans `specs/` for every
  format, as `spec-providers` *Spec-system selection is explicit* specifies.
- `mcp/query/specs.rs::resolve_dir` probes for `.specify/specs` when no `[specs]` section exists. The
  probe is unreachable in effect — `scan_specs` fails without a `[specs]` section, so the listing is
  empty and the directory is never shown — but it contradicts the explicit-selection rule.

## Goals / Non-Goals

**Goals:**

- One place per format for every per-format decision, reached through the backend.
- One format type in the domain; strings exist only at the config, CLI and MCP boundaries.
- Byte-identical prompts, doctrine, MCP payloads and error messages, except the Markdown task prompt.
- Adding a fifth format touches the enum, its `backend()` arm and one new impl — nothing else.

**Non-Goals:**

- Changing the `[specs]` config schema or its accepted values, or making a typed config field (an
  unknown `type` must keep failing at scan time with today's message, not at config load).
- Changing the no-`dir` default for a configured `type` (see Context).
- New MCP spec-authoring tools. v0.18.0's `mcp-spec-authoring-tools` ships them for OpenSpec only;
  its scaffold and check (pure functions in `src/specs/openspec.rs`) move into the OpenSpec Strategy
  here, and other formats keep answering "not supported for format <type>" until a later change adds
  per-format authoring methods to the trait.

## Decisions

**D1 — Enum for identity, trait for behaviour.** `SpecBackendKind` stays the closed identity tag
(with `ALL`, `as_str`, `FromStr`) because entries and the MCP wire carry it; `SpecBackend` is the
Strategy. `SpecBackendKind::backend() -> &'static dyn SpecBackend` is the only exhaustive match.
Same shape as `cli-provider-record`'s `CliFamily` / `CliProvider`.

**D2 — Trait surface.** `kind`, `default_dir`, `scan`, `path_doctrine` (the bundled asset),
`task_prompt(&SpecEntry)` (the bundled asset with placeholders filled), `title_artifact` (the file
whose first heading titles a spec, if any), `artifacts(spec_dir, &SpecEntry)`, `parse_tasks(&str)`,
`uses_opsx_workflow` (true only for OpenSpec; gates the opsx role-gating regions and the guard), and
`constitution(specs_dir)` (Spec Kit's governance auto-wire; `None` elsewhere). Default methods cover
the common case so each impl overrides only what differs; there is no default for `scan`,
`path_doctrine` or `task_prompt`.

**D3 — Strings only at boundaries.** Config: `SpecsConfig.spec_type` stays `Option<String>` on the
wire and is parsed by `SpecBackendKind::from_str` where it is read, with the existing
"unknown spec type: … (known: …)" error built from `ALL`. CLI: `SpecsFormat` keeps its clap
`ValueEnum` (so `--help` and the rejection message are unchanged) and gains
`From<SpecsFormat> for SpecBackendKind`. Deriving `ValueEnum` on `SpecBackendKind` itself was
rejected: it would make the domain type depend on clap. MCP: `backend_str` becomes `as_str`, and the
valid-type set is `ALL`.

**D4 — One directory resolver.** `resolve_specs_config` returns the resolved kind and directory; the
scan and the MCP query layer both use it, so `resolve_dir` and its `.specify` probe go away. No
observable output changes (see Context).

**D5 — The Markdown task prompt.** It currently says sibling artifacts "live under
`openspec/changes/{{SPEC_ID}}/`". A Markdown spec is one file whose body the sidecar already carries
(`assets/spec-path-doctrine/markdown.md`: "the file itself is the contract"). The prompt points at the
sidecar and states that the spec file is the whole contract. Kept in this change because the task
prompt becomes a backend method here, and it is the same defect class as GP-07; it can be split out
if the user prefers a pure refactor.

## Risks / Trade-offs

- **Behaviour drift during the move** → each step is pinned first: a table-driven test captures, per
  backend, today's task prompt, doctrine, title, artifact list and parsed tasks for a fixture spec,
  and must pass unchanged after every step (the Markdown prompt row is updated in its own commit).
- **A `_` arm hid a default** → `get_tasks` parses OpenSpec, Markdown and Superpowers alike with the
  checkbox parser; the trait default keeps that, and Spec Kit overrides it — the same behaviour, now
  explicit.
- **Concurrent edits** → v0.17.0 `mcp-superpowers-support` edits two of the MCP sites first; this
  change lands after it and removes the list it adds to AGENTS.md §7.
