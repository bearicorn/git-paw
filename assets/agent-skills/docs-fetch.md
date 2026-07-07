---
name: docs-fetch
description: Consult git-paw's documentation on demand via the bundled docs-fetch helper
license: MIT
compatibility: git-paw v0.10.0+
---

## Consulting git-paw Documentation

You have a bundled helper, `.git-paw/scripts/docs-fetch.sh`, that retrieves
git-paw's own documentation on demand from the configured docs site. Reach for
it when a git-paw convention, command, configuration field, or coordination
detail is unclear — instead of guessing or stalling. Documentation is fetched
live and only when you ask; none of it is embedded in your boot prompt, so you
pull just the page (or section) you need, when you need it.

### When to consult the docs

Consult the docs when the answer is a git-paw fact you are unsure of, for example:

- how a git-paw subcommand, flag, or config field behaves,
- what a coordination or governance convention requires,
- how a workflow (spec-driven launch, supervisor mode, pause/resume) is meant to run.

Do not consult the docs for facts already in your context (this skill, your
assignment, the specs, the code) or for questions about the project you are
working on rather than git-paw itself.

### How to use the helper

Two steps: discover the right page, then retrieve it.

1. **Find the right page.** Search the docs index (`llms.txt`) by keyword; the
   helper prints the best-matching pages as title + absolute URL + summary:

   ```bash
   .git-paw/scripts/docs-fetch.sh find "<keywords>"
   ```

2. **Retrieve the page — or a single section.** Fetch a page by the URL or path
   from step 1. Pass a section anchor as a second argument to narrow the output
   to just that section instead of the whole page:

   ```bash
   .git-paw/scripts/docs-fetch.sh get "<page-or-url>" [anchor]
   ```

Always go through `.git-paw/scripts/docs-fetch.sh`. Do **not** hand-write a
`curl` request to the docs site: the helper resolves the docs base URL, shapes
the request, and returns readable text, and it is the only docs-fetch command
covered by your permission grant — a hand-rolled request re-derives logic the
helper already owns and will not be pre-approved.

### If a lookup fails

Documentation is an aid, never a hard dependency of your task. If the helper
exits non-zero — the docs site is unreachable, the page does not exist, or the
anchor is unknown — it prints a short diagnostic on stderr and stops. When that
happens, **continue your task without the docs**: do not retry in a loop and do
not wait on the site. Fall back to the specs, the code, and the conventions
already in your context, and proceed.
