//! Pure readers of a captured agent screen that `__send` consults before it
//! presses a key: whether a selection menu is showing, and whether the input
//! box's text is drawn dim (a CLI's prompt suggestion, not typed text).
//!
//! Every CLI-specific shape comes from the agent's prompt profile
//! (`menu_cursor_glyphs`, `menu_footer_markers`, `option_line_pattern`); the
//! functions here only read a screen and decide nothing about permissions.

use crate::config::CompiledPromptProfile;
use crate::supervisor::permission_prompt::is_option_line;

/// How many trailing non-blank lines are searched for a selection menu.
const MENU_TAIL_LINES: usize = 15;

/// Whether `capture` shows a selection menu: one of the profile's
/// `menu_cursor_glyphs` on an option line, or one of its
/// `menu_footer_markers` (case-insensitive). An `Enter` typed into one picks
/// an option instead of submitting text.
#[must_use]
pub fn menu_showing(capture: &str, profile: &CompiledPromptProfile) -> bool {
    let lines: Vec<&str> = capture.lines().filter(|l| !l.trim().is_empty()).collect();
    let tail = &lines[lines.len().saturating_sub(MENU_TAIL_LINES)..];
    let footers: Vec<String> = profile
        .menu_footer_markers
        .iter()
        .map(|m| m.to_lowercase())
        .collect();
    tail.iter().any(|line| {
        let trimmed = strip_decoration_keep_cursor(line);
        let cursor = profile
            .menu_cursor_glyphs
            .iter()
            .find_map(|glyph| trimmed.strip_prefix(glyph.as_str()));
        if cursor.is_some_and(|rest| is_option_line(rest.trim_start(), profile)) {
            return true;
        }
        let lower = line.to_lowercase();
        footers.iter().any(|footer| lower.contains(footer.as_str()))
    })
}

/// Trims box-drawing borders and whitespace but keeps a cursor glyph, which
/// is what marks a numbered line as a live menu.
fn strip_decoration_keep_cursor(line: &str) -> &str {
    line.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, '│' | '┃' | '|'))
}

/// Whether the first input-box row `first_row` is drawn dim or grey in the
/// styled capture `styled` — a CLI's prompt suggestion, not text anyone
/// typed.
#[must_use]
pub fn composer_text_is_dim(styled: &str, first_row: &str) -> bool {
    let Some(line) = styled
        .lines()
        .rev()
        .find(|l| crate::replay::strip_ansi(l).contains(first_row))
    else {
        return false;
    };
    let cells = styled_cells(line);
    let plain: String = cells.iter().map(|(c, _)| *c).collect();
    let Some(byte_at) = plain.find(first_row) else {
        return false;
    };
    let start = plain[..byte_at].chars().count();
    cells
        .iter()
        .skip(start)
        .take(first_row.chars().count())
        .filter(|(c, _)| !c.is_whitespace())
        .all(|(_, dim)| *dim)
}

/// Each visible character of `line` with whether it is drawn dim or grey.
fn styled_cells(line: &str) -> Vec<(char, bool)> {
    let mut cells = Vec::new();
    let mut dim = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\x1b' {
            cells.push((c, dim));
            continue;
        }
        if chars.peek() != Some(&'[') {
            chars.next();
            continue;
        }
        chars.next();
        let mut params = String::new();
        let mut final_byte = None;
        for p in chars.by_ref() {
            if ('@'..='~').contains(&p) {
                final_byte = Some(p);
                break;
            }
            params.push(p);
        }
        if final_byte == Some('m') {
            dim = apply_sgr(dim, &params);
        }
    }
    cells
}

/// Applies one SGR parameter list to the dim/grey state.
fn apply_sgr(mut dim: bool, params: &str) -> bool {
    let codes: Vec<u16> = params.split(';').map(|p| p.parse().unwrap_or(0)).collect();
    let mut i = 0;
    while i < codes.len() {
        match codes[i] {
            2 | 90 => dim = true,
            0 | 22 | 39 | 30..=37 | 91..=97 => dim = false,
            38 => match codes.get(i + 1) {
                Some(5) => {
                    let n = codes.get(i + 2).copied().unwrap_or(0);
                    dim = n == 8 || (232..=250).contains(&n);
                    i += 2;
                }
                Some(2) => {
                    let (r, g, b) = (
                        codes.get(i + 2).copied().unwrap_or(0),
                        codes.get(i + 3).copied().unwrap_or(0),
                        codes.get(i + 4).copied().unwrap_or(0),
                    );
                    dim = r == g && g == b && r < 200;
                    i += 4;
                }
                _ => {}
            },
            _ => {}
        }
        i += 1;
    }
    dim
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CliPromptProfileOverride, CompiledPromptProfile, merge_prompt_profile};

    fn default_profile() -> CompiledPromptProfile {
        crate::config::default_compiled_prompt_profile().clone()
    }

    fn overridden(override_: &CliPromptProfileOverride) -> CompiledPromptProfile {
        let shape = merge_prompt_profile(crate::config::claude_prompt_profile(), Some(override_));
        CompiledPromptProfile::compile(shape).expect("compiles")
    }

    #[test]
    fn grey_sgr_forms_read_as_dim() {
        for sgr in ["2", "90", "38;5;244", "38;2;128;128;128", "1;2"] {
            let line = format!("> \x1b[{sgr}mhint\x1b[0m");
            assert!(composer_text_is_dim(&line, "hint"), "{sgr}");
        }
        for sgr in ["0", "97", "38;5;15", "38;2;200;100;100"] {
            let line = format!("> \x1b[{sgr}mtext\x1b[0m");
            assert!(!composer_text_is_dim(&line, "text"), "{sgr}");
        }
    }

    #[test]
    fn the_default_glyphs_and_footer_mark_a_menu() {
        let p = default_profile();
        for screen in [
            "Send anyway?\n❯ 1. Send\n  2. Wait",
            "Send anyway?\n› 1. Send\n  2. Wait",
            "Send anyway?\n> 1. Send\n  2. Wait",
            "Send anyway?\n  1. Send\n  2. Wait\nEnter to select",
        ] {
            assert!(menu_showing(screen, &p), "{screen}");
        }
        assert!(!menu_showing("1. a numbered list\n2. in prose", &p));
    }

    /// Spec scenario "An overridden cursor glyph is honoured", at the reader.
    #[test]
    fn an_overridden_cursor_glyph_replaces_the_defaults() {
        let p = overridden(&CliPromptProfileOverride {
            menu_cursor_glyphs: Some(vec!["»".to_string()]),
            ..CliPromptProfileOverride::default()
        });
        assert!(menu_showing("» 1. Send\n  2. Wait", &p));
        assert!(!menu_showing("❯ 1. Send\n  2. Wait", &p));
    }

    /// A non-ASCII footer matches whatever its case on screen.
    #[test]
    fn a_non_ascii_footer_matches_case_insensitively() {
        let p = overridden(&CliPromptProfileOverride {
            menu_footer_markers: Some(vec!["élire".to_string()]),
            ..CliPromptProfileOverride::default()
        });
        assert!(menu_showing("choix\nÉLIRE", &p));
    }

    /// Spec scenario "An overridden footer replaces the default", at the
    /// reader.
    #[test]
    fn an_overridden_footer_replaces_the_default() {
        let p = overridden(&CliPromptProfileOverride {
            menu_footer_markers: Some(vec!["press space to choose".to_string()]),
            ..CliPromptProfileOverride::default()
        });
        assert!(menu_showing("pick one\nPress space to choose", &p));
        assert!(!menu_showing("pick one\nEnter to select", &p));
    }
}
