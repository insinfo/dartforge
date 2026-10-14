//! The publication rule (plan §2.3), shared by `dartforge analyze` and the
//! LSP: a syntactic diagnostic is always published; a semantic code is only
//! published if it is in `verified.txt` — nothing it emits is wrong (zero
//! false positives, zero wrong positions, zero wrong messages) in the parity
//! corpus and in the real projects. False negatives are allowed: what the
//! user sees is right, even if something is missing. The rest exists
//! internally and only goes to the scoreboard.

use dartforge_diagnostics::{Diagnostic, ErrorType};

/// The versioned list of verified semantic codes.
pub const VERIFIED: &str = include_str!("../verified.txt");

/// The codes of [`VERIFIED`] (without comments).
pub fn verified() -> Vec<&'static str> {
    VERIFIED
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Does the diagnostic go to the editor/CLI? `syntax`: it came from the lexer/parser.
pub fn is_published(d: &Diagnostic, syntax: bool) -> bool {
    if syntax {
        return true;
    }
    match d.code {
        Some(c) if c.info().error_type == ErrorType::SyntacticError => true,
        Some(c) => verified().contains(&c.info().name),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dartforge_diagnostics::{Code, Span};

    #[test]
    fn verified_assignment_is_published_without_publishing_unverified_errors() {
        let assignment = Code::by_name("invalid_assignment").unwrap();
        let diagnostic =
            Diagnostic::with_code(assignment, Span { start: 0, end: 7 }, ["String", "int"]);
        assert!(is_published(&diagnostic, false));
        let unverified = Code::by_name("type_argument_not_matching_bounds").unwrap();
        assert!(!verified().contains(&unverified.info().name));
        let diagnostic = Diagnostic::with_code(
            unverified,
            Span { start: 0, end: 1 },
            ["String", "num", "T"],
        );
        assert!(!is_published(&diagnostic, false));
        assert!(is_published(&diagnostic, true));
    }
}
