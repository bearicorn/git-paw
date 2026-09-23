## ADDED Requirements

### Requirement: Supervised launch honours the CLIs chosen in the interactive picker

When a supervised launch has no `--branches` and no spec selection, and so runs the interactive selection flow (branch picker, assignment-mode prompt and CLI picker), each coding agent SHALL launch with the CLI the selection mapped to its branch — the uniform choice, or that branch's own per-branch choice. The launch SHALL NOT discard the selection's CLI mapping in favour of `default_cli` or the supervisor CLI. The session-wide fallback (`--cli` > `default_cli` > the supervisor CLI) SHALL apply only to launch paths that show no CLI picker, such as `--branches` without `--cli`; spec-driven branches keep their spec CLI resolution chain.

#### Scenario: A uniform CLI pick is honoured

- **GIVEN** supervisor mode with `default_cli = "alpha"` and the supervisor CLI `alpha`, two available CLIs `alpha` and `beta`, and no `--branches`, `--specs` or `--cli`
- **WHEN** the user selects `feat/a`, chooses uniform mode and picks `beta`
- **THEN** the coding agent for `feat/a` SHALL launch with `beta`

#### Scenario: Per-branch CLI picks are honoured

- **GIVEN** supervisor mode with no `--branches`, `--specs` or `--cli`
- **WHEN** the user selects `feat/a` and `feat/b`, chooses per-branch mode, and picks `alpha` for `feat/a` and `beta` for `feat/b`
- **THEN** `feat/a`'s coding agent SHALL launch with `alpha` and `feat/b`'s with `beta`

#### Scenario: The flag path keeps the session-wide fallback

- **GIVEN** supervisor mode with `default_cli = "alpha"`
- **WHEN** the launch is given `--branches feat/a` and no `--cli`
- **THEN** no CLI picker SHALL be shown
- **AND** `feat/a`'s coding agent SHALL launch with `alpha`
