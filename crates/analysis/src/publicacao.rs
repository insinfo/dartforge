//! The publication rule (plan §2.3), shared by `dartforge analyze` and the
//! LSP: a syntactic diagnostic is always published; a semantic code is only
//! published if it is in `verificados.txt` — nothing it emits is wrong (zero
//! false positives, zero wrong positions, zero wrong messages) in the parity
//! corpus and in the real projects. False negatives are allowed: what the
//! user sees is right, even if something is missing. The rest exists
//! internally and only goes to the scoreboard.

use dartforge_diagnostics::{Diagnostic, TipoErro};

/// The versioned list of verified semantic codes.
pub const VERIFICADOS: &str = include_str!("../verificados.txt");

/// The codes of [`VERIFICADOS`] (without comments).
pub fn verificados() -> Vec<&'static str> {
    VERIFICADOS
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Does the diagnostic go to the editor/CLI? `syntax`: it came from the lexer/parser.
pub fn publicado(d: &Diagnostic, syntax: bool) -> bool {
    if syntax {
        return true;
    }
    match d.code {
        Some(c) if c.info().tipo == TipoErro::SyntacticError => true,
        Some(c) => verificados().contains(&c.info().nome),
        None => false,
    }
}
