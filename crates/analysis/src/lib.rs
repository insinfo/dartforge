//! Analyzer verifiers that do not depend on types, over the `frontend` tree
//! (plan A3). Each one reproduces a verifier of `package:analyzer` 6.11.0 —
//! the codes, the English messages and the positions are its own — and is
//! checked by the scoreboard of `crates/parity`.
//!
//! * [`locals`]: unused local variables and functions (`UnusedLocalElementsVerifier`).
//! * [`duplicates`]: `DuplicateDefinitionVerifier` and
//!   `MemberDuplicateDefinitionVerifier` (`src/error/duplicate_definition_verifier.dart`).
//! * [`externals`]: initializers of `external` fields and variables
//!   (`ErrorVerifier`).
//! * [`private_decls`]: private declarations never referenced
//!   (`UnusedLocalElementsVerifier`, the library part).
//! * [`operators`]: arity, optional parameters and the return of `[]=` in
//!   `operator` methods (`ErrorVerifier`).
//! * [`enums`]: an enum without constants after augmentations (`ErrorVerifier`).
//! * [`clauses`]: inheritance clauses (`subtype_of_disallowed_type`, mixin
//!   errors, `class_used_as_mixin`) and the `ErrorVerifier` gate that turns
//!   off the following checks.
//! * [`modifiers`]: `base`/`final`/`interface`/`sealed` used outside the
//!   library (`ErrorVerifier` and `BaseOrFinalTypeVerifier`).
//! * [`members`]: local checks of declarations and members of the
//!   `ErrorVerifier` (conflicting type parameters, enum and extension type
//!   fields, setters, `this.x` outside a constructor, initializer list,
//!   `return` in a generative constructor).
//! * [`natives`]: `native` (class clause and body) outside the SDK
//!   (`ErrorVerifier`).

pub mod a_context;
pub mod a_doc;
pub mod analyzer_tree;
pub mod c2_syntactic;
pub mod clauses;
pub mod constructors;
pub mod duplicates;
pub mod enums;
pub mod exit_detector;
pub mod externals;
pub mod imports;
pub mod initialization;
pub mod language_version;
pub mod lints;
pub mod locals;
pub mod members;
pub mod meta;
pub mod modifiers;
pub mod natives;
pub mod non_dart;
pub mod operators;
pub mod phases;
pub mod private_decls;
pub mod publication;
pub mod records;
pub mod simple_bounds;
pub mod todos;

use dartforge_frontend::ast::{Ast, CompilationUnit};

/// A unit (file) of a library, in the library's order: the one that declares
/// it first, then the parts.
#[derive(Clone, Copy)]
pub struct Unit<'a> {
    pub ast: &'a Ast,
    pub unit: &'a CompilationUnit,
    /// The unit's text (local annotations are not kept in the tree).
    pub source: &'a str,
}

/// The fragments of a unit that the parser's error recovery discarded
/// (`Unit::pulados`), with its text: the only syntax-based suppression left
/// (docs/ANALYZER-ESPECIFICACAO.md §G, T5, structure (c)). A name cited in a
/// skipped fragment may have there a use or a declaration that fasta kept and
/// our tree does not have.
#[derive(Clone, Copy)]
pub struct Skipped<'a> {
    pub source: &'a str,
    pub spans: &'a [dartforge_diagnostics::Span],
}

impl Skipped<'_> {
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// Does range `s` touch any skipped fragment?
    pub fn intersects(&self, s: dartforge_diagnostics::Span) -> bool {
        self.spans
            .iter()
            .any(|t| t.start < s.end && s.start < t.end)
    }

    /// Does identifier `name` appear, as a whole word, in any skipped
    /// fragment?
    pub fn mentions(&self, name: &str) -> bool {
        self.spans
            .iter()
            .any(|t| mentioned_in_fragment(self.source, *t, name))
    }

    /// Like [`Skipped::mentions`], only in the fragments that touch `within`
    /// (a local's executable declaration). Only a mention starting inside
    /// `within` counts (the declared name itself, in a skipped fragment that
    /// contains it, is not a use: `var enum;`).
    pub fn mentions_within(&self, name: &str, within: dartforge_diagnostics::Span) -> bool {
        self.spans
            .iter()
            .filter(|t| t.start < within.end && within.start < t.end)
            .any(|t| {
                let start = t.start.max(within.start);
                let end = t.end.min(within.end);
                // The whole fragment decides the word boundary; the position, the cut.
                mentions(self.source, *t, name).any(|p| p >= start && p < end)
            })
    }
}

/// The positions (in the unit) of the whole-word mentions of `name` in fragment `t`.
fn mentions<'a>(
    source: &'a str,
    t: dartforge_diagnostics::Span,
    name: &'a str,
) -> impl Iterator<Item = usize> + 'a {
    let text = source.get(t.start..t.end.min(source.len())).unwrap_or("");
    let bytes = text.as_bytes();
    let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'$';
    text.match_indices(name)
        .filter(move |(start, _)| {
            let end = start + name.len();
            !name.is_empty()
                && (*start == 0 || !is_word(bytes[start - 1]))
                && (end >= bytes.len() || !is_word(bytes[end]))
        })
        .map(move |(start, _)| t.start + start)
}

fn mentioned_in_fragment(source: &str, t: dartforge_diagnostics::Span, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let Some(text) = source.get(t.start..t.end.min(source.len())) else {
        return false;
    };
    let bytes = text.as_bytes();
    let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'$';
    let mut from = 0;
    while let Some(i) = text[from..].find(name) {
        let start = from + i;
        let end = start + name.len();
        let before_ok = start == 0 || !is_word(bytes[start - 1]);
        let after_ok = end >= bytes.len() || !is_word(bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        from = start + 1;
        while from < text.len() && !text.is_char_boundary(from) {
            from += 1;
        }
    }
    false
}
