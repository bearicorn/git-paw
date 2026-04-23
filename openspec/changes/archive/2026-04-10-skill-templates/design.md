## Context

This change is independent of the broker's runtime — it ships static text and a small loader. It has no async, no I/O at startup beyond an optional file read, and no new dependencies. It exists primarily to establish a stable, testable interface that `skill-injection` (Wave 2) calls to fetch rendered coordination instructions, which are then embedded inside the existing git-paw marker section of each worktree's `AGENTS.md`.

The design is deliberately minimal because v0.3.0 has exactly one skill (`coordination`). The shape is intentionally extensible so v0.4+ can add `verification`, `governance`, or `escalation` skills without revisiting this module's API.

### Flow context (how this fits the v0.2.0 AGENTS.md machinery)

git-paw v0.2.0 already manages a marker-delimited section inside each worktree's `AGENTS.md` via the `agents-md-injection` and `worktree-agents-md` capabilities. The relevant existing behavior:

- `setup_worktree_agents_md` reads the **root repo's** `AGENTS.md` (treating missing as empty), generates a git-paw section delimited by `<!-- git-paw:start ... -->` and `<!-- git-paw:end -->`, calls `inject_into_content` to either append the section or replace an existing marker block, writes the result to `<worktree>/AGENTS.md`, and excludes that file from git.
- The user's tracked project context lives outside the markers and is preserved across launches.
- On every `git paw start` (new launch or reattach), only the marker-delimited section is regenerated; everything else is left alone.

In v0.3.0, **`skill-injection` (Wave 2) modifies the existing `worktree-agents-md` capability** so the marker-delimited section also includes the rendered coordination skill. That change calls `skills::resolve("coordination")` and `skills::render(template, branch, broker_url)` from this module to obtain the text to embed. No new file write step, no new marker scheme — the existing `inject_into_content` machinery handles replacement on relaunch.

This change (`skill-templates`) is purely additive: it provides the loader and renderer. It does not touch `src/agents.rs` and does not modify the existing `worktree-agents-md` or `agents-md-injection` specs. Those modifications belong to `skill-injection`.

## Goals / Non-Goals

**Goals:**

- Provide a small, deterministic API for resolving and rendering skill templates by name
- Ship the v0.3.0 coordination skill embedded in the binary so `git paw` works out of the box without any user setup
- Allow users to override any embedded skill by dropping a same-named file in `~/.config/git-paw/agent-skills/`
- Substitute `{{BRANCH_ID}}` at git-paw render time using the slug rule from `message-types`
- Substitute `{{GIT_PAW_BROKER_URL}}` at git-paw render time so the agent's curl commands contain a literal URL
- Leave the public API stable enough that future skills are zero-friction additions

**Non-Goals:**

- Per-CLI customization. Dropped for v0.3.0; can be added in v0.4+ via a richer naming convention if needed
- Skill discovery / listing commands (`git paw list-skills` etc.) — out of scope, can be added later
- Watch-and-reload of user override files. Skill content is read once per session at launch; users must restart the session to pick up changes
- Validating skill content against any schema. Skill files are free-form Markdown; the only contract is the substitution placeholders
- The actual injection of skill text into worktree `AGENTS.md` (owned by `skill-injection` in Wave 2)
- The mechanism by which the broker URL is set in pane environments (owned by `broker-integration` in Wave 2)

## Decisions

### Decision 1: Embed defaults via `include_str!`, not via `include_dir!` or runtime extraction

The single default skill is embedded at compile time:

```rust
const COORDINATION_DEFAULT: &str = include_str!("../assets/agent-skills/coordination.md");
```

When a new skill is added in v0.4+, a new constant is added the same way.

**Why:**
- No new dependency. `include_dir`, `rust-embed`, etc. would all require approval
- One file is trivially manageable as a constant; even a handful of skills (5-10) stays manageable
- The skill content ships *inside* the binary — no install-time file copying, no PATH-relative asset discovery, no platform-specific data directories
- Compile-time inclusion guarantees the embedded skill is always available regardless of how git-paw was installed (cargo install, Homebrew, raw binary)

**Alternatives considered:**
- *`include_dir!` macro*. Requires the `include_dir` crate. More magic, more deps. Rejected.
- *Read defaults from `$XDG_DATA_DIRS/git-paw/agent-skills/` at runtime*. Forces the installer to put files there; breaks `cargo install`. Rejected.
- *Bake defaults into a single Rust file as raw strings*. Works but loses the ability to view/edit the canonical skill file in `assets/` separately from Rust source. Rejected — keeping `coordination.md` as a real `.md` file makes it readable in editors and reviewable in PRs.

### Decision 2: Two-level resolution, no chain beyond user override → embedded

```rust
pub fn resolve(skill_name: &str) -> Result<SkillTemplate, SkillError> {
    if let Some(content) = try_load_user_override(skill_name)? {
        return Ok(SkillTemplate { name: skill_name.into(), content, source: Source::User });
    }
    if let Some(content) = embedded_default(skill_name) {
        return Ok(SkillTemplate { name: skill_name.into(), content: content.into(), source: Source::Embedded });
    }
    Err(SkillError::UnknownSkill { name: skill_name.into() })
}
```

**Why:**
- Two levels are easy to reason about and easy to test
- Zero ambiguity for users: "your file in `~/.config/...` wins, otherwise you get the built-in"
- The third level I had earlier (per-CLI override) was speculative and added a real cost in complexity for no v0.3.0 benefit

**Alternatives considered:**
- *Three-level chain* (user `<cli>.md` → user `<skill>.md` → embedded). Discussed and rejected — see proposal.
- *Merge user file into embedded* (e.g. user file appends to embedded). Would surprise users; the Markdown format makes "merging" ill-defined. Rejected.

### Decision 3: `try_load_user_override` is best-effort, with strict error handling

The user override loader:

1. Looks up `dirs::config_dir()`. If `None`, returns `Ok(None)` — user has no config dir, no override possible
2. Builds the path `<config_dir>/git-paw/agent-skills/<skill-name>.md`
3. Calls `std::fs::read_to_string` on that path
4. If the file does not exist (`io::ErrorKind::NotFound`), returns `Ok(None)`
5. If the file exists but cannot be read (permission denied, I/O error, invalid UTF-8), returns `Err(SkillError::UserOverrideRead { .. })` so the user knows their override is broken instead of being silently ignored

**Why:**
- "File doesn't exist" is a normal condition (no override) → not an error
- "File exists but I can't read it" is almost always a bug the user wants to know about → loud error
- Silent fallback in the second case would mask typos in the override file (e.g. permissions misconfiguration after `sudo cp`)

### Decision 4: `SkillTemplate` is a small value type, `render` is a free function

```rust
pub struct SkillTemplate {
    pub name: String,
    pub content: String,
    pub source: Source,  // Embedded or User
}

pub fn render(template: &SkillTemplate, branch: &str, broker_url: &str) -> String
```

**Why:**
- The `Source` field is useful for diagnostic output ("loaded user override" vs "loaded embedded default")
- Keeping `render` as a free function (rather than `SkillTemplate::render(&self, ...)`) makes it trivially testable in isolation and doesn't require constructing a `SkillTemplate` to test substitution rules
- `SkillTemplate` is `Clone + Debug` for ergonomics in tests

### Decision 5: Substitution is literal string replacement, not a templating engine

```rust
pub fn render(template: &SkillTemplate, branch: &str, broker_url: &str) -> String {
    let branch_id = crate::broker::messages::slugify_branch(branch);
    template.content
        .replace("{{BRANCH_ID}}", &branch_id)
        .replace("{{GIT_PAW_BROKER_URL}}", broker_url)
}
```

**Wait** — re-read this. The proposal says `{{GIT_PAW_BROKER_URL}}` is substituted at render time. Two cases:

- `{{BRANCH_ID}}` — substituted at render time (git-paw replaces it)
- `{{GIT_PAW_BROKER_URL}}` — substituted at render time (git-paw replaces it)

So `render` substitutes both placeholders. The broker URL is embedded directly.

**Why:**
- Branch ID is known at render time, embed it directly
- Broker URL is also known at render time and embedding it means the agent's curl commands contain a literal URL, which keeps allowlist-style permission prompts (e.g. "don't ask again for `curl:*`") working cleanly. Some CLI tools gate shell-variable expansion behind extra permission prompts, which breaks the allowlist flow.
- Plain string `replace` is sufficient for two placeholders; pulling in a templating crate (handlebars, tera, etc.) is overkill

**Alternatives considered:**
- *`handlebars` or `tera`*. Both add a dependency, both support features we don't need (loops, conditionals, helpers). Rejected.
- *Pass `${GIT_PAW_BROKER_URL}` through unchanged*. Would require shell expansion at runtime, which reintroduces permission-prompt friction. Rejected.

**Update to proposal:** the proposal already says `{{GIT_PAW_BROKER_URL}}` is substituted at render time. Spec scenarios will assert this explicitly.

### Decision 6: `assets/agent-skills/coordination.md` has a frozen content shape

The default coordination skill content (committed to the repo) is:

```markdown
## Coordination Skills

You are running inside a git-paw worktree as agent `{{BRANCH_ID}}`. The git-paw broker
is reachable at `{{GIT_PAW_BROKER_URL}}`. Use the following `curl` commands to coordinate
with peer agents.

### Report progress (after each commit)

curl -s -X POST {{GIT_PAW_BROKER_URL}}/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.status","agent_id":"{{BRANCH_ID}}","payload":{"status":"working","modified_files":[]}}'

### Check for messages from peers (before starting new work)

curl -s {{GIT_PAW_BROKER_URL}}/messages/{{BRANCH_ID}}

The response includes a `last_seq` field. To see only new messages on subsequent polls,
pass `?since=<last_seq>` from the previous response:

curl -s {{GIT_PAW_BROKER_URL}}/messages/{{BRANCH_ID}}?since=<last_seq>

### Report completion (when done)

curl -s -X POST {{GIT_PAW_BROKER_URL}}/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.artifact","agent_id":"{{BRANCH_ID}}","payload":{"status":"done","exports":[]}}'

### Report blocked (when you need something from another agent)

curl -s -X POST ${GIT_PAW_BROKER_URL}/publish \
  -H "Content-Type: application/json" \
  -d '{"type":"agent.blocked","agent_id":"{{BRANCH_ID}}","payload":{"needs":"<what>","from":"<agent-id>"}}'
```

**Why:**
- Matches the wire format defined in `message-types` exactly (the four operations correspond to the three message variants plus a poll)
- All four `curl` examples are self-contained — an agent can copy any one and run it
- Uses `{{GIT_PAW_BROKER_URL}}` everywhere so multi-repo users get correct URLs automatically
- The `## Coordination Skills` heading is a clear section marker that `skill-injection` can append under and that the user can visually identify in their worktree `AGENTS.md`

The content is in `assets/agent-skills/coordination.md` as a tracked file so it can be reviewed in PRs and edited without touching Rust code. The file is `include_str!`'d into the binary at build time.

### Decision 7: Skill content is loaded once per `resolve` call, not cached

`resolve` does a fresh file read every call. There is no in-memory cache.

**Why:**
- v0.3.0 calls `resolve` exactly once per session per skill (called from `skill-injection` at session launch)
- A cache would be premature optimization and would introduce a "what if the user updated the file mid-session" question
- File reads are tens of microseconds; the cost is invisible

If v0.4+ adds many skills or calls `resolve` repeatedly, a `OnceLock<HashMap<...>>` cache can be added without changing the public API.

## Risks / Trade-offs

- **Embedded coordination skill drifts from the broker's actual wire format** → If `message-types` or `http-broker` changes the JSON shape after this skill is committed, the embedded `curl` examples become wrong. **Mitigation:** the wire format is frozen in `message-types` for v0.3.0; any future change is a coordinated breaking change that touches multiple specs at once. Add an integration test in v0.3.0 (during integration testing phase) that posts each `curl` example from `coordination.md` against a live broker and asserts a `202`.

- **User override file with broken syntax** → A user could put garbage in `coordination.md` and confuse their agents. **Mitigation:** out of scope. Skill content is free-form Markdown with two placeholders; we cannot validate the curl commands without running them. Document that overrides are at the user's risk in mdBook.

- **`{{GIT_PAW_BROKER_URL}}` typo by user** → The user might write `{{GIT_PAW_BROKER_URL}}` (double curly) thinking it's a placeholder, but `render` only substitutes `{{BRANCH_ID}}`, so the literal `{{GIT_PAW_BROKER_URL}}` would survive into the agent's instructions and break their `curl` calls. **Mitigation:** `render` could detect and warn on unknown `{{...}}` sequences. Add this as a non-functional requirement in the spec — log a warning if the rendered output contains any `{{...}}` substring not consumed by substitution.

- **`assets/` directory missed by `cargo publish`** → If `Cargo.toml` doesn't include `assets/` in the `include` list, `cargo publish` will ship a binary that fails to build because `include_str!` can't find the file. **Mitigation:** explicitly add `"assets/**/*"` to `include` in `Cargo.toml`, and add a publish-dry-run task to `just check` (or document it in the release checklist).

- **Skill content gets stale across major versions** → v0.4 supervisor introduces new message types (`agent.verified`, `agent.feedback`); the v0.3.0 coordination skill won't mention them. **Mitigation:** v0.4 will either update `coordination.md` in place or add a separate `verification.md` skill. Either approach is supported by this design without a breaking API change.

## Migration Plan

No migration. New module, new asset directory, new public API. Rollback is `git revert`.

For users on v0.2.0, no action is required. The new module is unused until `skill-injection` lands in Wave 2 and starts calling it during session launch.

## Open Questions

- **Should the unknown-placeholder warning be a hard error or a log line?** Likely a `log::warn!` or `eprintln!` (depending on logging infrastructure available at the time of injection). Decide during implementation; not blocking the spec.
- **Do we want a `git paw skills list` command in v0.3.0 to show which skills are available and where they're loaded from?** Probably no — it's a developer convenience and out of scope for the wave. Defer to v1.0.0 polish.
