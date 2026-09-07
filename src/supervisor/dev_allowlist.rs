//! Common dev-command allowlist preset data for the supervisor.
//!
//! Implements the `dev-command-allowlist` capability: a curated preset of
//! prefix patterns the supervisor composes into its own auto-approve
//! whitelist ([`crate::config::SupervisorConfig::effective_whitelist`]) and
//! renders into the bundled supervisor skill's permission guidance
//! ([`crate::skills`]), so the supervisor does not hand-approve every
//! dev-loop command variant in a stalled agent pane.
//!
//! The preset is split into two tiers (see `design.md` D2):
//!
//! - [`DEV_ALLOWLIST_PRESET`] — the **universal** set, hard-coded and
//!   always included. It contains only commands that are safe and useful
//!   in essentially any repository regardless of language or toolchain
//!   (non-destructive git verbs plus read-only `find` / `grep` /
//!   `sed -n`). It is the single source of truth for the universal
//!   tier — no other location may hard-code these patterns.
//! - [`stack_preset`] / the named `*_STACK_PRESET` constants — curated,
//!   opt-in **stack-specific** bundles (`rust` / `node` / `python` /
//!   `go`). A repository opts in via
//!   `[supervisor.common_dev_allowlist] stacks = ["rust", ...]`; callers
//!   resolve the selected stacks to the union of the universal preset,
//!   each selected stack, and any `extra` patterns via
//!   [`effective_patterns`].
//!
//! Users further extend the result through
//! `[supervisor.common_dev_allowlist] extra = [...]`.
//!
//! Every listed value is a command **prefix** (a verb, or verb plus
//! subcommand) that subsumes all per-invocation argument variations —
//! e.g. `git diff` (which prefix-matches `git diff --stat HEAD~1`),
//! never a fully-argumented command line. A prefix grant collapses the
//! infinite set of per-run argument variations into one approval.

/// Universal dev-loop prefix patterns composed into the supervisor's
/// auto-approve whitelist, independent of the repository's language or
/// toolchain.
///
/// Inclusion rubric (see `design.md` D3): bounded side-effects (no
/// arbitrary network or arbitrary code execution); aligns with
/// CLAUDE.md's git-safety protocol. Destructive git operations
/// (`git rebase`, `git reset`, `git checkout`, `git push --force`)
/// and write-mode `sed` are intentionally excluded; stack-specific
/// toolchain commands are NOT hard-coded here — they are opt-in via
/// the named stack presets (see [`stack_preset`]) and/or `extra`.
///
/// The constant is the single source of truth for the universal tier
/// — no other location in the codebase may hard-code these patterns.
pub const DEV_ALLOWLIST_PRESET: &[&str] = &[
    // Git read-only.
    "git status",
    "git log",
    "git diff",
    "git show",
    "git fetch",
    // Git write (non-destructive; rebase/reset/checkout excluded).
    "git commit",
    "git push",
    "git pull",
    "git merge",
    "git stash",
    "git add",
    "git restore",
    "git rm",
    // Search (read-only; `sed -n` is the read-only invocation).
    "find",
    "grep",
    "sed -n",
];

/// Curated `rust` stack preset (opt-in via `stacks = ["rust"]`).
///
/// Build/test/lint verbs only; `cargo install` / `cargo run` /
/// `cargo bench` are excluded per the D3 rubric.
pub const RUST_STACK_PRESET: &[&str] = &[
    "cargo build",
    "cargo test",
    "cargo clippy",
    "cargo fmt",
    "cargo check",
    "cargo tree",
    "cargo deny",
    "cargo update",
];

/// Curated `node` stack preset (opt-in via `stacks = ["node"]`).
///
/// Package-manager build/test/install verbs only; `publish` /
/// `uninstall` and arbitrary-script execution beyond the curated
/// verbs are excluded per the D3 rubric.
pub const NODE_STACK_PRESET: &[&str] = &[
    "npm install",
    "npm ci",
    "npm test",
    "npm run",
    "pnpm install",
    "pnpm test",
    "pnpm run",
    "yarn install",
    "yarn test",
];

/// Curated `python` stack preset (opt-in via `stacks = ["python"]`).
///
/// Test/lint/format/dependency verbs only; arbitrary `python -c` and
/// publish/upload verbs are excluded per the D3 rubric.
pub const PYTHON_STACK_PRESET: &[&str] = &[
    "pytest",
    "pip install",
    "ruff",
    "black",
    "mypy",
    "flake8",
    "uv pip",
    "uv sync",
];

/// Curated `go` stack preset (opt-in via `stacks = ["go"]`).
///
/// Build/test/vet/format verbs only; `go run` (arbitrary code
/// execution) is excluded per the D3 rubric.
pub const GO_STACK_PRESET: &[&str] = &[
    "go build",
    "go test",
    "go vet",
    "go fmt",
    "gofmt",
    "go mod",
    "golangci-lint",
];

/// Resolves a stack-preset name to its curated prefix list.
///
/// Returns the matching `*_STACK_PRESET` slice for a known name
/// (`rust` / `node` / `python` / `go`), or `None` for an unrecognised
/// name. Matching is case-sensitive against the lowercase names a
/// repository declares in `[supervisor.common_dev_allowlist] stacks`.
/// An unknown stack name contributes nothing (no error) so a typo or a
/// future stack name in an older binary degrades gracefully.
#[must_use]
pub fn stack_preset(name: &str) -> Option<&'static [&'static str]> {
    match name {
        "rust" => Some(RUST_STACK_PRESET),
        "node" => Some(NODE_STACK_PRESET),
        "python" => Some(PYTHON_STACK_PRESET),
        "go" => Some(GO_STACK_PRESET),
        _ => None,
    }
}

/// Returns the effective ordered pattern list: the universal preset,
/// followed by each selected stack preset, followed by `extra`,
/// de-duplicated.
///
/// Resolution order (see `design.md` D2): the universal
/// [`DEV_ALLOWLIST_PRESET`] first (declaration order), then each named
/// preset from `stacks` in selection order (unknown names contribute
/// nothing), then `extra` in input order. A pattern already present
/// from an earlier tier is not added again, so the result is the
/// de-duplicated union.
#[must_use]
pub fn effective_patterns(stacks: &[String], extra: &[String]) -> Vec<String> {
    let mut out: Vec<String> = DEV_ALLOWLIST_PRESET
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let push_unique = |out: &mut Vec<String>, pat: &str| {
        if !out.iter().any(|existing| existing == pat) {
            out.push(pat.to_string());
        }
    };
    for stack in stacks {
        if let Some(preset) = stack_preset(stack) {
            for pat in preset {
                push_unique(&mut out, pat);
            }
        }
    }
    for entry in extra {
        push_unique(&mut out, entry);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_constant_contains_only_universal_patterns() {
        // The universal preset SHALL contain exactly the stack-neutral
        // git + search verbs and nothing else.
        let required = [
            "git status",
            "git log",
            "git diff",
            "git show",
            "git fetch",
            "git commit",
            "git push",
            "git pull",
            "git merge",
            "git stash",
            "git add",
            "git restore",
            "git rm",
            "find",
            "grep",
            "sed -n",
        ];
        for r in required {
            assert!(
                DEV_ALLOWLIST_PRESET.contains(&r),
                "universal preset missing required pattern: {r}",
            );
        }
        // The set is *exactly* the universal patterns — no extras crept in.
        assert_eq!(
            DEV_ALLOWLIST_PRESET.len(),
            required.len(),
            "universal preset must contain exactly the required patterns; got {DEV_ALLOWLIST_PRESET:?}",
        );

        // Stack-specific patterns moved to named presets / `extra` SHALL
        // NOT be hard-coded in the universal preset.
        let stack_specific = [
            "cargo build",
            "cargo test",
            "cargo clippy",
            "cargo fmt",
            "cargo check",
            "just",
            "mdbook build",
            "openspec validate",
            "openspec status",
            "npm install",
            "pytest",
            "go build",
        ];
        for s in stack_specific {
            assert!(
                !DEV_ALLOWLIST_PRESET.contains(&s),
                "universal preset must not contain stack-specific pattern: {s}",
            );
        }

        // Destructive patterns stay excluded from the universal set.
        let excluded = [
            "git rebase",
            "git reset",
            "git checkout",
            "git branch -D",
            "git push --force",
            "git push -f",
            "sed",
        ];
        for e in excluded {
            assert!(
                !DEV_ALLOWLIST_PRESET.contains(&e),
                "preset must not contain excluded pattern: {e}",
            );
        }
    }

    #[test]
    fn curated_stack_presets_obey_the_exclusion_rubric() {
        // No curated stack preset may carry a destructive / arbitrary-
        // code-execution verb (design.md D3).
        let forbidden = [
            "cargo install",
            "cargo run",
            "cargo bench",
            "go run",
            "npm publish",
            "npm uninstall",
            "pip uninstall",
        ];
        for stack in ["rust", "node", "python", "go"] {
            let preset = stack_preset(stack).expect("named stack resolves");
            for f in forbidden {
                assert!(
                    !preset.contains(&f),
                    "stack `{stack}` must not contain forbidden verb: {f}",
                );
            }
        }
        // Unknown stack names resolve to nothing (graceful degradation).
        assert!(stack_preset("haskell").is_none());
    }

    #[test]
    fn rust_stack_preset_carries_curated_cargo_verbs() {
        let preset = stack_preset("rust").expect("rust stack resolves");
        for pat in ["cargo build", "cargo test", "cargo clippy"] {
            assert!(preset.contains(&pat), "rust stack missing {pat}");
        }
    }

    #[test]
    fn effective_patterns_orders_preset_before_extra() {
        let extra = vec!["pnpm test".to_string()];
        let out = effective_patterns(&[], &extra);
        let pnpm_idx = out.iter().position(|s| s == "pnpm test").unwrap();
        let git_idx = out.iter().position(|s| s == "git diff").unwrap();
        assert!(
            git_idx < pnpm_idx,
            "preset entries must precede extra: git@{git_idx} vs pnpm@{pnpm_idx}",
        );
    }

    #[test]
    fn effective_patterns_deduplicates_extra_against_preset() {
        let extra = vec!["git diff".to_string()];
        let out = effective_patterns(&[], &extra);
        assert_eq!(out.iter().filter(|s| *s == "git diff").count(), 1);
    }

    #[test]
    fn effective_patterns_universal_only_when_no_stacks_or_extra() {
        let out = effective_patterns(&[], &[]);
        let expected: Vec<String> = DEV_ALLOWLIST_PRESET
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        assert_eq!(
            out, expected,
            "no stacks + no extra must yield exactly the universal preset"
        );
        // Spot-check no stack leakage.
        assert!(!out.iter().any(|s| s == "cargo build"));
    }

    #[test]
    fn effective_patterns_rust_stack_adds_cargo_prefixes() {
        let stacks = vec!["rust".to_string()];
        let out = effective_patterns(&stacks, &[]);
        for pat in RUST_STACK_PRESET {
            assert!(out.iter().any(|s| s == pat), "missing rust prefix {pat}");
        }
        // Universal preset still present; ordering is universal-then-stack.
        let git_idx = out.iter().position(|s| s == "git diff").unwrap();
        let cargo_idx = out.iter().position(|s| s == "cargo build").unwrap();
        assert!(git_idx < cargo_idx, "universal must precede stack prefixes");
    }

    #[test]
    fn effective_patterns_node_stack_has_no_cargo() {
        let stacks = vec!["node".to_string()];
        let out = effective_patterns(&stacks, &[]);
        assert!(out.iter().any(|s| s.starts_with("npm")));
        assert!(
            !out.iter().any(|s| s.starts_with("cargo")),
            "node stack must not seed any cargo prefix: {out:?}",
        );
    }

    #[test]
    fn effective_patterns_multiple_stacks_compose_as_dedup_union() {
        let stacks = vec!["rust".to_string(), "python".to_string()];
        let out = effective_patterns(&stacks, &[]);
        assert!(out.iter().any(|s| s == "cargo build"));
        assert!(out.iter().any(|s| s == "pytest"));
        // No duplicates anywhere in the union.
        let mut seen = std::collections::HashSet::new();
        for s in &out {
            assert!(seen.insert(s.clone()), "duplicate pattern in union: {s}");
        }
    }

    #[test]
    fn effective_patterns_unknown_stack_contributes_nothing() {
        let stacks = vec!["haskell".to_string()];
        let out = effective_patterns(&stacks, &[]);
        let expected: Vec<String> = DEV_ALLOWLIST_PRESET
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        assert_eq!(out, expected, "unknown stack must add nothing");
    }
}
