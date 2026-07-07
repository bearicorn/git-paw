<!-- summary: A bundled skill and helper that let an agent consult git-paw's documentation on demand. -->

# Docs Fetch Skill

git-paw bundles a **docs-fetch** skill and a matching helper script so an agent
can consult git-paw's own documentation *on demand* — pulling exactly the
convention or example it needs, when it needs it, instead of carrying doc
content in its boot prompt or guessing.

The design mirrors the other bundled helpers (`broker.sh`, `sweep.sh`): the
skill teaches *when and why* to consult the docs, and the helper does the
fetch. The agent invokes one stable script path rather than hand-rolling
`curl`, which keeps the permission grant a single least-privilege path and the
behaviour deterministic.

## What the agent gets

When the docs-fetch skill is rendered into an agent's instructions, the agent
is told to reach for the helper — `.git-paw/scripts/docs-fetch.sh` — whenever a
git-paw fact is unclear (a subcommand's behaviour, a config field, a
coordination or governance convention, a workflow). Documentation is fetched
live and only when asked; none of it is embedded in the binary or injected
wholesale into the boot prompt.

## The two operations

The helper is a thin, dependency-free wrapper (`curl` + Python 3, the same
primitives the broker helper uses). It exposes a discover-then-retrieve flow
that matches the machine-readable surface published by the
[agent-friendly docs site](https://bearicorn.github.io/git-paw/) (`llms.txt`
and per-page anchors).

**`find <query>`** — fetch the site's `llms.txt` index and print the pages that
best match the query, each as a title, absolute URL, and one-line summary:

```bash
.git-paw/scripts/docs-fetch.sh find "worktree placement"
```

**`get <page-or-url> [anchor]`** — fetch a page (by the URL or path from
`find`) and print its documentation content. Pass a section anchor to narrow
the output to just that section instead of the whole page:

```bash
.git-paw/scripts/docs-fetch.sh get user-guide/worktree-placement.html
.git-paw/scripts/docs-fetch.sh get user-guide/worktree-placement.html child-layout
```

## Configurable docs source

The docs base URL defaults to git-paw's published documentation site and is
overridable so a fork or mirror can point the skill at its own docs. Set the
top-level [`docs_base_url`](../configuration/README.md#docs_base_url) field in
`.git-paw/config.toml`:

```toml
docs_base_url = "https://docs.example.com/git-paw"
```

The helper resolves the base URL from config, falling back to the built-in
default when the field is absent. Both `find` and `get` target the configured
URL.

## Best-effort by design

Documentation lookup is an aid, never a hard dependency of the agent's task. If
the docs site is unreachable, a page does not exist, or an anchor is unknown,
the helper exits non-zero with a short diagnostic (and never hangs — every
fetch is time-bounded). The skill instructs the agent to **continue its task
without the docs** in that case rather than block or retry in a loop.

## Least-privilege install

`git paw init` installs `docs-fetch.sh` into `.git-paw/scripts/` alongside
`broker.sh` and `sweep.sh`, and the agent allowlist grants that exact script
path — both the bare path and the `bash <path>` form — never a broad `curl`
wildcard. A single by-path grant covers every subcommand, so an agent consults
the docs without any wildcard network permission.
