//! Dependency-free parser for the subset of YAML frontmatter used by
//! agentskills.io standardized skills.
//!
//! Replaces `serde_yaml` (see `openspec/changes/drop-serde-yaml/`, archived
//! once merged): that crate depends on the unmaintained `unsafe-libyaml`
//! (RUSTSEC-2024-0449), and the only call site needed a small, bounded
//! subset of YAML — two required strings, two optional strings, and a flat
//! string map. Supporting exactly that subset, and rejecting anything else
//! by name, is a few dozen lines and testable exhaustively.
//!
//! Supported subset: top-level `key: value` pairs with plain, single- or
//! double-quoted scalars; literal (`|`) and folded (`>`) block scalars with
//! their strip (`-`) / keep (`+`) chomping indicators; full-line and
//! trailing `#` comments outside quotes; and a one-level `metadata` mapping
//! of string keys to scalar values. A top-level key this parser does not
//! consume is skipped together with any indented content beneath it. A
//! *consumed* field carrying a construct outside the subset — a flow
//! collection, an anchor/alias, a tag, or a nested mapping inside
//! `metadata` — is rejected by name rather than guessed at, since this
//! parser also reads user-authored override skills it does not control.

use serde_json::{Map, Value};

/// The top-level scalar fields this parser populates directly onto
/// [`ParsedFrontmatter`]. `metadata` is handled separately since it is a
/// mapping, not a scalar.
const CONSUMED_SCALAR_FIELDS: [&str; 4] = ["name", "description", "license", "compatibility"];

/// A field whose value used a YAML construct outside the supported subset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnsupportedConstruct {
    /// The frontmatter field that could not be parsed.
    pub(crate) field: String,
    /// Human-readable detail for the error message.
    pub(crate) detail: String,
    /// The `name` field's value if it was already parsed before this error
    /// was hit, so the caller can identify the skill in its own error.
    pub(crate) partial_name: Option<String>,
}

/// The fields this parser extracts from a frontmatter block.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct ParsedFrontmatter {
    pub(crate) name: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) license: Option<String>,
    pub(crate) compatibility: Option<String>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Chomp {
    Clip,
    Strip,
    Keep,
}

fn unsupported(field: &str, detail: &str, result: &ParsedFrontmatter) -> UnsupportedConstruct {
    UnsupportedConstruct {
        field: field.to_string(),
        detail: detail.to_string(),
        partial_name: result.name.clone(),
    }
}

fn assign_scalar_field(result: &mut ParsedFrontmatter, key: &str, value: String) {
    match key {
        "name" => result.name = Some(value),
        "description" => result.description = Some(value),
        "license" => result.license = Some(value),
        "compatibility" => result.compatibility = Some(value),
        _ => {}
    }
}

/// Parses a frontmatter block (the text between the `---` fences) into its
/// consumed fields. A top-level key outside [`CONSUMED_SCALAR_FIELDS`] and
/// `metadata` is skipped along with any indented content beneath it.
pub(crate) fn parse(frontmatter: &str) -> Result<ParsedFrontmatter, UnsupportedConstruct> {
    let lines: Vec<&str> = frontmatter.lines().collect();
    let mut result = ParsedFrontmatter::default();
    let mut i = 0;

    while i < lines.len() {
        let raw_line = lines[i];
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            i += 1;
            continue;
        }

        let Some(colon_idx) = raw_line.find(':') else {
            return Err(unsupported(
                "frontmatter",
                &format!("expected 'key: value', got {raw_line:?}"),
                &result,
            ));
        };
        let key = raw_line[..colon_idx].trim().to_string();
        let after_colon = &raw_line[colon_idx + 1..];
        let after_trimmed = after_colon.trim_start();
        let is_comment_only = after_trimmed.is_empty() || after_trimmed.starts_with('#');

        let block_style = if !is_comment_only
            && (after_trimmed.starts_with('|') || after_trimmed.starts_with('>'))
        {
            after_trimmed.chars().next()
        } else {
            None
        };

        if let Some(style) = block_style {
            i = handle_block_scalar_key(&lines, i, &key, style, &after_trimmed[1..], &mut result)?;
            continue;
        }

        if is_comment_only {
            let (block_lines, next_i) = collect_indented_block(&lines, i + 1);
            if key == "metadata" {
                result.metadata = if block_lines.is_empty() {
                    None
                } else {
                    Some(parse_metadata_block(&block_lines, &result)?)
                };
            } else if CONSUMED_SCALAR_FIELDS.contains(&key.as_str()) {
                if !block_lines.is_empty() {
                    return Err(unsupported(
                        &key,
                        "expected a scalar value, found a nested block",
                        &result,
                    ));
                }
                assign_scalar_field(&mut result, &key, String::new());
            }
            i = next_i;
            continue;
        }

        if key == "metadata" {
            return Err(unsupported(
                "metadata",
                "expected a mapping, found an inline value",
                &result,
            ));
        }
        match parse_scalar(after_trimmed) {
            Ok(value) => {
                if CONSUMED_SCALAR_FIELDS.contains(&key.as_str()) {
                    assign_scalar_field(&mut result, &key, value);
                }
            }
            Err(()) => {
                if CONSUMED_SCALAR_FIELDS.contains(&key.as_str()) {
                    return Err(unsupported(&key, "unsupported YAML construct", &result));
                }
            }
        }
        i += 1;
    }

    Ok(result)
}

/// Handles a `key: |` / `key: >` header: parses the chomping indicator,
/// collects the block, renders it, and assigns it if `key` is a consumed
/// scalar field. `rest` is everything after the `|`/`>` style character on
/// the header line. Returns the index to resume scanning from.
fn handle_block_scalar_key(
    lines: &[&str],
    i: usize,
    key: &str,
    style: char,
    rest: &str,
    result: &mut ParsedFrontmatter,
) -> Result<usize, UnsupportedConstruct> {
    let (chomp, rest) = if let Some(r) = rest.strip_prefix('-') {
        (Chomp::Strip, r)
    } else if let Some(r) = rest.strip_prefix('+') {
        (Chomp::Keep, r)
    } else {
        (Chomp::Clip, rest)
    };
    let rest_trimmed = rest.trim();
    let header_ok = rest_trimmed.is_empty() || rest_trimmed.starts_with('#');

    let (block_lines, next_i) = collect_indented_block(lines, i + 1);
    let value = render_block_scalar(style, chomp, &block_lines);

    if key == "metadata" {
        return Err(unsupported(
            "metadata",
            "a block scalar cannot represent the metadata mapping",
            result,
        ));
    }
    if !header_ok {
        if CONSUMED_SCALAR_FIELDS.contains(&key) {
            return Err(unsupported(key, "unsupported block scalar header", result));
        }
    } else if CONSUMED_SCALAR_FIELDS.contains(&key) {
        assign_scalar_field(result, key, value);
    }
    Ok(next_i)
}

/// Collects the lines following a top-level key that belong to it — every
/// line indented more than column 0, plus any blank lines among them —
/// stopping at the next top-level key or end of input. Returns the
/// collected lines and the index to resume scanning from.
fn collect_indented_block<'a>(lines: &[&'a str], start: usize) -> (Vec<&'a str>, usize) {
    let mut out = Vec::new();
    let mut j = start;
    while j < lines.len() {
        let l = lines[j];
        if l.trim().is_empty() {
            out.push(l);
            j += 1;
            continue;
        }
        let indent = l.len() - l.trim_start().len();
        if indent == 0 {
            break;
        }
        out.push(l);
        j += 1;
    }
    (out, j)
}

/// Parses the indented body of a `metadata:` key as a one-level mapping of
/// string keys to scalar values. Every entry must share the first entry's
/// indentation and carry an inline scalar value — a different indentation
/// or an absent value both indicate a nested mapping, which is outside the
/// supported subset.
fn parse_metadata_block(
    block_lines: &[&str],
    result: &ParsedFrontmatter,
) -> Result<Value, UnsupportedConstruct> {
    let Some(baseline_indent) = block_lines
        .iter()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
    else {
        return Ok(Value::Object(Map::new()));
    };

    let mut map = Map::new();
    for line in block_lines {
        if line.trim().is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if indent != baseline_indent {
            return Err(unsupported(
                "metadata",
                "nested mapping is not supported in metadata",
                result,
            ));
        }
        let entry = &line[indent..];
        let Some(colon_idx) = entry.find(':') else {
            return Err(unsupported(
                "metadata",
                &format!("expected 'key: value' in metadata entry, got {entry:?}"),
                result,
            ));
        };
        let subkey = entry[..colon_idx].trim().to_string();
        let after = entry[colon_idx + 1..].trim_start();
        if after.is_empty() || after.starts_with('#') {
            return Err(unsupported(
                "metadata",
                "metadata entries must be scalar values",
                result,
            ));
        }
        match parse_scalar(after) {
            Ok(value) => {
                map.insert(subkey, Value::String(value));
            }
            Err(()) => {
                return Err(unsupported(
                    "metadata",
                    "unsupported YAML construct in metadata entry",
                    result,
                ));
            }
        }
    }
    Ok(Value::Object(map))
}

/// Renders a literal (`|`) or folded (`>`) block scalar from the raw lines
/// following its header, applying the chomping indicator to the trailing
/// line breaks.
fn render_block_scalar(style: char, chomp: Chomp, block_lines: &[&str]) -> String {
    let content_indent = block_lines
        .iter()
        .find(|l| !l.trim().is_empty())
        .map_or(0, |l| l.len() - l.trim_start().len());

    let physical: Vec<String> = block_lines
        .iter()
        .map(|l| {
            if l.trim().is_empty() {
                String::new()
            } else if l.len() >= content_indent {
                l[content_indent..].to_string()
            } else {
                l.trim_start().to_string()
            }
        })
        .collect();

    let Some(last_content_idx) = physical.iter().rposition(|l| !l.is_empty()) else {
        return match chomp {
            Chomp::Keep => "\n".repeat(physical.len()),
            Chomp::Clip | Chomp::Strip => String::new(),
        };
    };

    let trailing_blanks = physical.len() - 1 - last_content_idx;
    let core_lines = &physical[..=last_content_idx];

    let core = match style {
        '|' => core_lines.join("\n"),
        _ => fold_lines(core_lines),
    };

    match chomp {
        Chomp::Strip => core,
        Chomp::Clip => format!("{core}\n"),
        Chomp::Keep => format!("{core}{}", "\n".repeat(trailing_blanks + 1)),
    }
}

/// Folds a run of physical lines per YAML's folded (`>`) scalar: adjacent
/// non-blank lines join with a single space, and each blank line becomes a
/// line break. This covers the common case; it does not special-case
/// more-indented "literal" lines within a folded block, since no frontmatter
/// in the tree uses them.
fn fold_lines(lines: &[String]) -> String {
    let mut out = String::new();
    let mut first = true;
    let mut pending_breaks = 0usize;
    for line in lines {
        if line.is_empty() {
            pending_breaks += 1;
            continue;
        }
        if first {
            out.push_str(line);
            first = false;
        } else if pending_breaks > 0 {
            out.push_str(&"\n".repeat(pending_breaks));
            out.push_str(line);
        } else {
            out.push(' ');
            out.push_str(line);
        }
        pending_breaks = 0;
    }
    out
}

/// Parses a single inline scalar value: plain, single-quoted, or
/// double-quoted. Returns `Err(())` for anything else (flow collections,
/// anchors, aliases, tags) — the caller decides whether that is fatal
/// (a consumed field) or ignorable (an unconsumed one).
fn parse_scalar(raw: &str) -> Result<String, ()> {
    match raw.chars().next() {
        Some('"') => parse_double_quoted(raw),
        Some('\'') => parse_single_quoted(raw),
        Some('{' | '[' | '&' | '*' | '!') => Err(()),
        Some(_) => Ok(parse_plain(raw)),
        None => Ok(String::new()),
    }
}

/// A plain scalar runs to a `#` that is preceded by whitespace (a trailing
/// comment) or to the end of the line; surrounding whitespace is trimmed.
fn parse_plain(s: &str) -> String {
    let mut end = s.len();
    let mut prev_ws = true;
    for (i, ch) in s.char_indices() {
        if ch == '#' && prev_ws {
            end = i;
            break;
        }
        prev_ws = ch == ' ' || ch == '\t';
    }
    s[..end].trim_end().to_string()
}

/// A double-quoted scalar supports `\"`, `\\`, `\n` and `\t` escapes; any
/// other backslash sequence is passed through literally. Trailing content
/// after the closing quote must be empty or a comment.
fn parse_double_quoted(s: &str) -> Result<String, ()> {
    let mut chars = s.chars();
    chars.next(); // opening quote, already matched by the caller
    let mut value = String::new();
    loop {
        match chars.next() {
            Some('\\') => match chars.next() {
                Some('"') => value.push('"'),
                Some('\\') => value.push('\\'),
                Some('n') => value.push('\n'),
                Some('t') => value.push('\t'),
                Some(other) => {
                    value.push('\\');
                    value.push(other);
                }
                None => return Err(()),
            },
            Some('"') => break,
            Some(ch) => value.push(ch),
            None => return Err(()),
        }
    }
    let trailer: String = chars.collect();
    let trailer = trailer.trim();
    if !trailer.is_empty() && !trailer.starts_with('#') {
        return Err(());
    }
    Ok(value)
}

/// A single-quoted scalar escapes an embedded quote as `''`. Trailing
/// content after the closing quote must be empty or a comment.
fn parse_single_quoted(s: &str) -> Result<String, ()> {
    let mut chars = s.chars().peekable();
    chars.next(); // opening quote, already matched by the caller
    let mut value = String::new();
    loop {
        match chars.next() {
            Some('\'') => {
                if chars.peek() == Some(&'\'') {
                    chars.next();
                    value.push('\'');
                } else {
                    break;
                }
            }
            Some(ch) => value.push(ch),
            None => return Err(()),
        }
    }
    let trailer: String = chars.collect();
    let trailer = trailer.trim();
    if !trailer.is_empty() && !trailer.starts_with('#') {
        return Err(());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_single_and_double_quoted_scalars() {
        for (input, expected) in [
            ("plain value", "plain value"),
            ("plain value  # trailing comment", "plain value"),
            ("'it''s quoted'", "it's quoted"),
            ("\"ratio 3: 4\"", "ratio 3: 4"),
            ("\"escaped \\\" quote\"", "escaped \" quote"),
        ] {
            assert_eq!(
                parse_scalar(input),
                Ok(expected.to_string()),
                "input: {input}"
            );
        }
    }

    #[test]
    fn unsupported_inline_constructs_are_rejected() {
        for input in ["{ a: 1 }", "[a, b]", "&anchor value", "*alias", "!!str x"] {
            assert_eq!(parse_scalar(input), Err(()), "input: {input}");
        }
    }

    #[test]
    fn block_scalar_chomping_indicators() {
        // One blank line after the content: `parse_standardized_metadata`
        // rejoins frontmatter lines with "\n" before this module re-splits
        // them, which drops one trailing blank line in the round trip — so
        // a *file* with two trailing blank lines (as the fixtures under
        // tests/fixtures/skill_frontmatter/ use) arrives here as one. This
        // matches the serde_yaml parity values frozen in
        // tests/fixtures/skill_frontmatter_expected.json.
        let lines = ["  First line.", "  Second line.", ""];
        assert_eq!(
            render_block_scalar('|', Chomp::Clip, &lines),
            "First line.\nSecond line.\n"
        );
        assert_eq!(
            render_block_scalar('|', Chomp::Strip, &lines),
            "First line.\nSecond line."
        );
        assert_eq!(
            render_block_scalar('|', Chomp::Keep, &lines),
            "First line.\nSecond line.\n\n"
        );
        assert_eq!(
            render_block_scalar('>', Chomp::Clip, &lines),
            "First line. Second line.\n"
        );
        assert_eq!(
            render_block_scalar('>', Chomp::Strip, &lines),
            "First line. Second line."
        );
        assert_eq!(
            render_block_scalar('>', Chomp::Keep, &lines),
            "First line. Second line.\n\n"
        );
    }

    #[test]
    fn folded_block_scalar_turns_a_blank_line_into_a_break() {
        let lines = [
            "  Para one line one.",
            "  Para one line two.",
            "",
            "  Para two.",
        ];
        assert_eq!(
            render_block_scalar('>', Chomp::Clip, &lines),
            "Para one line one. Para one line two.\nPara two.\n"
        );
    }

    #[test]
    fn unconsumed_key_with_nested_sequence_is_skipped() {
        let frontmatter = "name: has-extra\ndescription: desc\nallowed-tools:\n  - Read\n  - Write\ncompatibility: git-paw";
        let parsed = parse(frontmatter).unwrap();
        assert_eq!(parsed.name.as_deref(), Some("has-extra"));
        assert_eq!(parsed.description.as_deref(), Some("desc"));
        assert_eq!(parsed.compatibility.as_deref(), Some("git-paw"));
    }

    #[test]
    fn metadata_map_reads_values_as_strings() {
        let frontmatter =
            "name: n\ndescription: d\nmetadata:\n  author: openspec\n  version: \"1.0\"";
        let parsed = parse(frontmatter).unwrap();
        let metadata = parsed.metadata.unwrap();
        assert_eq!(metadata["author"], Value::String("openspec".to_string()));
        assert_eq!(metadata["version"], Value::String("1.0".to_string()));
    }

    #[test]
    fn unsupported_constructs_in_consumed_fields_name_the_field() {
        for (frontmatter, expected_field) in [
            ("name: n\ndescription: &anchor d", "description"),
            ("name: n\ndescription: *alias", "description"),
            ("name: n\ndescription: !!str d", "description"),
            ("name: n\ndescription: d\nmetadata: { a: 1 }", "metadata"),
            (
                "name: n\ndescription: d\nmetadata:\n  foo:\n    bar: baz",
                "metadata",
            ),
            ("name: n\ndescription: [a, b]", "description"),
        ] {
            let err = parse(frontmatter).expect_err(frontmatter);
            assert_eq!(err.field, expected_field, "frontmatter: {frontmatter}");
        }
    }
}
