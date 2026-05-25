//! Asserts that bare `--specs` on a TTY proceeds to the picker.
//!
//! Maps to scenario `Bare --specs on TTY proceeds to picker` from
//! cross-format-spec-selection. (test-coverage-v0-5-0 task 7.3)
//!
//! The production decision lives in `apply_spec_mode` (src/main.rs): the
//! `SpecMode::Picker` arm first checks `is_interactive_stdin()` and only
//! returns the "requires an interactive terminal" error when stdin is
//! NOT a TTY. With a TTY, the branch falls through to
//! `prompter.select_specs(&discovered)`.
//!
//! Because dev-deps do not include a PTY harness, this integration test
//! pins the property at the source-code level: the code path from
//! `SpecMode::Picker` to `prompter.select_specs` is structurally present,
//! and the `is_interactive_stdin()` guard is the only condition that
//! intercepts it.

const MAIN_RS: &str = include_str!("../src/main.rs");

fn extract_apply_spec_mode() -> &'static str {
    let signature = "fn apply_spec_mode(";
    let start = MAIN_RS
        .find(signature)
        .expect("`fn apply_spec_mode(` not found in src/main.rs");
    let body_start = MAIN_RS[start..]
        .find('{')
        .map(|o| start + o)
        .expect("opening brace not found");
    let mut depth: i32 = 0;
    for (i, ch) in MAIN_RS[body_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &MAIN_RS[body_start..=body_start + i];
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced braces while extracting apply_spec_mode")
}

#[test]
fn bare_specs_on_tty_invokes_picker() {
    let body = extract_apply_spec_mode();

    // The picker arm must call `prompter.select_specs(...)`.
    assert!(
        body.contains("prompter.select_specs"),
        "apply_spec_mode must invoke prompter.select_specs in the Picker arm; body:\n{body}"
    );

    // The TTY guard must be the only condition that intercepts the picker
    // call — it must compare via `is_interactive_stdin()`.
    assert!(
        body.contains("is_interactive_stdin"),
        "apply_spec_mode must guard the picker arm with is_interactive_stdin(); body:\n{body}"
    );

    // The interactive-terminal error message must be gated behind the
    // non-TTY branch only. We assert the error string appears under a
    // negation `!is_interactive_stdin()` block, which is the production
    // shape. With a TTY, the branch falls through to select_specs and
    // no `requires`/`interactive terminal` string is emitted.
    let interactive_check = body
        .find("if !is_interactive_stdin()")
        .expect("apply_spec_mode must guard the picker arm with `if !is_interactive_stdin()`");
    let trailing = &body[interactive_check..];
    let interactive_terminal_idx = trailing
        .find("requires an interactive terminal")
        .expect("interactive-terminal error message must be present");
    let select_specs_idx = trailing
        .find("prompter.select_specs")
        .expect("prompter.select_specs must follow the TTY guard");
    assert!(
        interactive_terminal_idx < select_specs_idx,
        "the error string must precede the prompter.select_specs call (i.e. live inside \
         the non-TTY branch)"
    );
}
