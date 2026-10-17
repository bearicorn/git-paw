//! The single audited compiler for regular expressions whose pattern is fixed
//! at build time.
//!
//! Every static pattern in the crate is compiled through [`static_regex`], the
//! only non-test code allowed to call `expect`. Keeping the suppression in one
//! place means a bad pattern is caught by that module's
//! `static_regexes_compile` unit test rather than panicking in a shipped
//! binary, and the `clippy::expect_used` gate stays meaningful everywhere else.
//!
//! A pattern that comes from configuration, user, or agent input is NOT a
//! static pattern: compile it with `Regex::new` and propagate the error. The
//! `&'static str` parameter enforces that by type.

use regex::Regex;

/// Compiles a regex whose pattern is fixed at build time.
///
/// # Panics
///
/// Panics if `pattern` is not a valid regular expression. The parameter is
/// `&'static str`, so the pattern is a compile-time constant of this crate and
/// every caller's pattern is compiled by a unit test — the panic is
/// unreachable at runtime.
#[allow(clippy::expect_used)] // pattern is a build-time constant; every caller's pattern is
// compiled by a unit test, so failure is unreachable at runtime
pub(crate) fn static_regex(pattern: &'static str) -> Regex {
    Regex::new(pattern).expect("static regex pattern compiles")
}

#[cfg(test)]
mod tests {
    use super::static_regex;

    #[test]
    fn compiles_a_static_pattern() {
        let re = static_regex(r"^paw-(\d+)$");
        assert!(re.is_match("paw-42"));
        assert!(!re.is_match("paw-x"));
    }
}
