//! The production-code filter every source audit shares (`code-architecture`
//! "Source audits read every production line").
//!
//! Included by path (`#[path = "helpers/source_scan.rs"] mod source_scan;`)
//! so an audit does not compile the whole `helpers` module; the crate's own
//! audits include it once, as `crate::source_scan`, from `src/lib.rs`.
#![allow(dead_code)]

/// `source` with every comment line and every `#[cfg(test)]` item blanked,
/// keeping line numbers, so only production code is audited.
///
/// Comment lines are blanked first, so a `#[cfg(test)]` mentioned in a
/// comment does not start a blank-out. A test item is blanked up to the `;`
/// that ends it or the brace that closes it; braces inside string, char and
/// raw-string literals and comments do not count, so production code after a
/// test item stays visible.
#[must_use]
pub fn production_code(source: &str) -> String {
    let text: Vec<char> = uncommented(source).chars().collect();
    let mut out = text.clone();
    for (start, end) in test_items(&text) {
        for c in out.iter_mut().take((end + 1).min(text.len())).skip(start) {
            if *c != '\n' {
                *c = ' ';
            }
        }
    }
    out.into_iter().collect()
}

/// The 1-based lines of `source` whose `#[cfg(test)]` item opens a brace that
/// never closes, which would blank everything after it.
#[must_use]
pub fn unclosed_test_items(source: &str) -> Vec<usize> {
    let text: Vec<char> = uncommented(source).chars().collect();
    test_items(&text)
        .into_iter()
        .filter(|&(_, end)| end >= text.len())
        .map(|(start, _)| text[..start].iter().filter(|c| **c == '\n').count() + 1)
        .collect()
}

/// `source` with every whole comment line emptied, keeping line numbers.
fn uncommented(source: &str) -> String {
    source
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("//") {
                ""
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Each `#[cfg(test)]` item in `text` as `(start, end)`: `end` is the index
/// of its `;` or closing brace, or `text.len()` when the brace never closes.
fn test_items(text: &[char]) -> Vec<(usize, usize)> {
    let marker: Vec<char> = "#[cfg(test)]".chars().collect();
    let mut items = Vec::new();
    let mut i = 0;
    while i + marker.len() <= text.len() {
        if text[i..i + marker.len()] != marker[..] {
            i += 1;
            continue;
        }
        let mut end = i + marker.len();
        while end < text.len() && text[end] != '{' && text[end] != ';' {
            end += 1;
        }
        if end < text.len() && text[end] == '{' {
            end = closing_brace(text, end);
        }
        items.push((i, end));
        i = end + 1;
    }
    items
}

/// The index of the brace that closes the one at `open`, or `text.len()`.
/// Braces inside literals and comments are skipped.
fn closing_brace(text: &[char], open: usize) -> usize {
    let mut depth = 0usize;
    let mut i = open;
    while i < text.len() {
        match text[i] {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            '"' => i = string_end(text, i),
            'r' if raw_string_start(text, i) => i = raw_string_end(text, i),
            '\'' => i = char_literal_end(text, i),
            '/' if text.get(i + 1) == Some(&'/') => {
                while i < text.len() && text[i] != '\n' {
                    i += 1;
                }
            }
            '/' if text.get(i + 1) == Some(&'*') => {
                i += 2;
                while i + 1 < text.len() && !(text[i] == '*' && text[i + 1] == '/') {
                    i += 1;
                }
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    text.len()
}

/// The index of the `"` closing the string opened at `open`.
fn string_end(text: &[char], open: usize) -> usize {
    let mut i = open + 1;
    while i < text.len() {
        match text[i] {
            '\\' => i += 2,
            '"' => return i,
            _ => i += 1,
        }
    }
    text.len()
}

/// Whether a raw string (`r"`, `r#"`, `br"`) starts at the `r` at `i`, and
/// the `r` is not the end of an identifier.
fn raw_string_start(text: &[char], i: usize) -> bool {
    let ident_before = i > 0
        && (text[i - 1].is_alphanumeric() || text[i - 1] == '_')
        && !(text[i - 1] == 'b' && (i < 2 || !text[i - 2].is_alphanumeric()));
    if ident_before {
        return false;
    }
    let mut j = i + 1;
    while text.get(j) == Some(&'#') {
        j += 1;
    }
    text.get(j) == Some(&'"')
}

/// The index of the last `#` (or the `"`) closing the raw string at `r`.
fn raw_string_end(text: &[char], r: usize) -> usize {
    let mut hashes = 0;
    let mut i = r + 1;
    while text.get(i) == Some(&'#') {
        hashes += 1;
        i += 1;
    }
    i += 1;
    while i < text.len() {
        if text[i] == '"' && (1..=hashes).all(|k| text.get(i + k) == Some(&'#')) {
            return i + hashes;
        }
        i += 1;
    }
    text.len()
}

/// The index closing a char literal at `open`, or `open` itself for a
/// lifetime such as `'a`.
fn char_literal_end(text: &[char], open: usize) -> usize {
    match (text.get(open + 1), text.get(open + 2)) {
        (Some('\\'), _) => {
            let mut i = open + 3;
            while i < text.len() && text[i] != '\'' {
                i += 1;
            }
            i
        }
        (Some(_), Some('\'')) => open + 2,
        _ => open,
    }
}
