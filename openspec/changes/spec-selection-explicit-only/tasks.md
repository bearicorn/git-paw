## 1. Remove launch-time auto-detection

- [x] 1.1 Drop the `.specify/` and `docs/superpowers/plans/` probes (and the now-unused `dir_has_md` + `repo_root` param) from `resolve_specs_config`; resolution = `--specs-format` → config `[specs]`
- [x] 1.2 Sharpen the "not configured" error to name both remedies (`[specs]` section and `--specs-format`)
- [x] 1.3 Tests: unconfigured repo with layouts on disk resolves to `None` / errors with an actionable message; config used verbatim; `--specs-format` supplies the format's default dir

## 2. Init records the spec system explicitly

- [x] 2.1 Remove `detect_speckit_section` / `detect_superpowers_section`
- [x] 2.2 Add `prompt_specs_section`: interactive `Select` over the four formats → writes `[specs]`; non-interactive → commented `[specs]` template
- [x] 2.3 Tests: non-interactive init writes the commented template listing the four formats; the format list is complete/ordered; the pure index→`[specs]` mapping (`specs_section_for`) is unit-tested for all four choices; a tmux `send-keys` integration test drives the real interactive `Select` and asserts the chosen `speckit` section lands in the written config (`tests/init_interactive_specs.rs`)

## 3. Docs

- [x] 3.1 Rewrite the auto-detection passages in `spec-driven-launch.md`, `configuration/README.md`, and `cli-reference.md` to describe config/CLI selection + the init prompt
- [x] 3.2 `mdbook build docs/` passes

## 4. Verification

- [x] 4.1 `openspec validate spec-selection-explicit-only --strict` passes
- [x] 4.2 `just check` green (fmt + clippy + full suite); every scenario maps to a test — the interactive-init `Select` is driven end-to-end in a tmux pane (`tests/init_interactive_specs.rs`), since it needs a real TTY that in-process `assert_cmd` pipes cannot provide
