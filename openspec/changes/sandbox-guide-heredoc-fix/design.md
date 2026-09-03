## Context

A one-line gap in the v0.15 documented sandbox profile: `zsh` writes heredoc bodies to
`/private/tmp/zsh*`, which the profile denies, breaking the standard heredoc commit
idiom for every sandboxed worker. Docs-only.

## Goals / Non-Goals

**Goals:** the documented profile lets a sandboxed worker use `git commit -m "$(cat
<<'EOF')"` without hitting `Operation not permitted`.

**Non-Goals:** any change to how git-paw launches the sandbox (it does not — docs-only);
loosening the read-confidentiality tiers.

## Decisions

- **Grant `/private/tmp` write, not a narrower `/private/tmp/zsh*`.** Heredoc temp
  names are shell- and version-specific; `/private/tmp` is already world-writable and
  secret-free, so a subpath grant is simpler and no weaker than the status quo.
  Alternative rejected: enumerating per-shell temp prefixes (brittle across shells).
- **Keep it a *write* grant.** The confidentiality tiers are read-side; this touches
  only `(allow file-write* …)`, so the documented read tiers are unchanged.

## Risks / Trade-offs

- **A shared world-writable temp dir is now writable by the confined worker** → it
  already was for any process; `/private/tmp` holds no secrets and is not the work
  area, so blast radius is unchanged.

## Migration Plan

Docs-only; no migration. Readers who copied the old profile add one line.

## Open Questions

None.
