## Context

The v0.3.0 skills module already supports named skills via `resolve(skill_name)` with two-level resolution (user override → embedded default). Adding a second embedded skill is a one-line change in `embedded_default()`. The real work is writing the supervisor's instruction content.

## Goals / Non-Goals

**Goals:**

- Define the supervisor's complete instruction set as a Markdown skill template
- Make it loadable via the existing `skills::resolve("supervisor")` API
- Add `{{PROJECT_NAME}}` placeholder so the supervisor knows which project it's orchestrating

**Non-Goals:**

- Supervisor runtime behavior (owned by `supervisor-agent` and `supervisor-mode`)
- Spec audit logic (owned by `spec-audit` change — the supervisor template just says "run spec audit")
- Per-CLI supervisor templates (one `supervisor.md` for all supervisor CLIs)

## Decisions

### Decision 1: Supervisor skill content structure

The template is organized into clear sections:

```markdown
## Your Role
## Context (templated)
## Skills (curl commands)
## Workflow
## Rules
```

**Why:**
- **Role** upfront so the CLI knows its purpose immediately
- **Context** with `{{PROJECT_NAME}}` and `{{GIT_PAW_BROKER_URL}}` for project-specific info
- **Skills** are curl commands — same pattern as coordination.md, works with any CLI
- **Workflow** is the step-by-step process (watch → test → verify/feedback → merge → summarize)
- **Rules** are hard constraints (don't write code, ask human for merges, etc.)

### Decision 2: `{{PROJECT_NAME}}` as a new placeholder

The `render()` function gains a third substitution:

```rust
pub fn render(template: &SkillTemplate, branch: &str, broker_url: &str, project: &str) -> String {
    let branch_id = crate::broker::messages::slugify_branch(branch);
    template.content
        .replace("{{BRANCH_ID}}", &branch_id)
        .replace("{{PROJECT_NAME}}", project)
    // {{GIT_PAW_BROKER_URL}} substituted at render time
}
```

**Why:**
- The supervisor needs to know the project name for tmux session targeting (`paw-{{PROJECT_NAME}}`)
- Coding agents also benefit (their coordination.md could reference it in future)
- Adding a parameter to `render()` is a breaking API change — all call sites in `main.rs` need updating to pass `project`

**Alternatives considered:**
- *Environment variable `$GIT_PAW_PROJECT`.* Another env var to inject. Rejected — `{{PROJECT_NAME}}` is known at render time, no reason to defer to shell.
- *Hardcode project name in the supervisor spec template at launch.* Would bypass the render function. Rejected — breaks the skill override mechanism.

### Decision 3: Supervisor template references tmux commands

The supervisor's skills include `tmux capture-pane` and `tmux send-keys` for pane inspection and agent communication:

```markdown
### Inspect agent pane
tmux capture-pane -t paw-{{PROJECT_NAME}}:0.N -p

### Send message to agent pane
tmux send-keys -t paw-{{PROJECT_NAME}}:0.N "message" Enter
```

**Why:**
- The supervisor needs to inspect agent output when tests fail (what did the agent produce?)
- The supervisor needs to send feedback directly to stuck agents via their terminal
- tmux commands work from any CLI that can run shell commands
- The `paw-{{PROJECT_NAME}}` session name matches git-paw's naming convention

### Decision 4: Test command reference uses `{{TEST_COMMAND}}`

The supervisor template says:

```markdown
### Run tests after agent reports done
{{TEST_COMMAND}}
```

But `{{TEST_COMMAND}}` is not substituted by `render()` — it's substituted by the `supervisor-agent` change at launch time when it reads `config.supervisor.test_command`. This avoids coupling the skills module to config.

**Why:**
- The skills module doesn't know about `SupervisorConfig`
- The supervisor-agent change handles the config → template → AGENTS.md pipeline
- If `test_command` is `None`, the supervisor-agent change replaces `{{TEST_COMMAND}}` with guidance to skip testing

## Risks / Trade-offs

- **`render()` API change is breaking** → Adding a `project` parameter breaks all existing call sites. **Mitigation:** only two call sites in `main.rs` (cmd_start and launch_spec_session). Small, mechanical fix.

- **Supervisor template is long** → The supervisor has more instructions than coding agents. A long AGENTS.md may consume context window in some CLIs. **Mitigation:** the template is ~60 lines of Markdown. Within budget for all supported CLIs.

- **`{{TEST_COMMAND}}` is not handled by render()** → Creates an implicit contract between the skills module and the supervisor-agent change. **Mitigation:** the unknown-placeholder warning from v0.3.0 will fire if `{{TEST_COMMAND}}` is not substituted before injection. The supervisor-agent change must handle it.

## Migration Plan

No migration. Adding a second embedded skill is purely additive. The `render()` API change requires updating existing call sites but is not a user-facing change.
