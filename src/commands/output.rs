//! The output port every command writes through.
//!
//! A command never prints: it hands each progress line to [`Output::line`]
//! and each warning to [`Output::warn`], and the caller decides where they
//! go. The binary's entry layer passes its terminal adapter, which writes them
//! to stdout and stderr exactly as the commands used to print them; an
//! in-process caller (the MCP server, a test) passes [`CaptureOutput`] and
//! reads them back.

/// Where a command's human-readable output goes.
pub trait Output {
    /// One line of progress or result text (the CLI's stdout). `text` carries
    /// no trailing newline; an empty string is a blank line.
    fn line(&mut self, text: &str);

    /// One line of warning or diagnostic text (the CLI's stderr). `text`
    /// carries no trailing newline.
    fn warn(&mut self, text: &str);
}

/// The capturing adapter: collects every line in order, for a caller that is
/// not a terminal.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CaptureOutput {
    /// Lines written with [`Output::line`], in order.
    pub lines: Vec<String>,
    /// Lines written with [`Output::warn`], in order.
    pub warnings: Vec<String>,
}

impl Output for CaptureOutput {
    fn line(&mut self, text: &str) {
        self.lines.push(text.to_string());
    }

    fn warn(&mut self, text: &str) {
        self.warnings.push(text.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_keeps_lines_and_warnings_apart_and_in_order() {
        let mut out = CaptureOutput::default();
        out.line("one");
        out.warn("careful");
        out.line("");
        out.line("two");
        assert_eq!(out.lines, ["one", "", "two"]);
        assert_eq!(out.warnings, ["careful"]);
    }
}
