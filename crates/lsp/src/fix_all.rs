//! The pieces of `Fix All` (`dart.edit.fixAll`; docs/LSP-ESPECIFICACAO.md
//! §13.12.7): the edit builder with the conflict rule of `addEditForSource`
//! (`AP:src/protocol/protocol_internal.dart:41-84`), the bulk applicability
//! of the fixes DartForge has, the phase B import removal and the merging of
//! the `SourceChangeMerger` passes
//! (`AS:src/utilities/source_change_merger.dart:30-166`).
//!
//! Written without compiling or running (2026-10-04).

use dartforge_diagnostics::Diagnostic;
use dartforge_frontend::ast::{CompilationUnit, DirectiveKind};

/// An edit: offset, length and new text (source bytes).
pub(crate) type Ed = (usize, usize, String);

/// The edits of one file in one pass, in DESCENDING offset order (applicable
/// in sequence).
#[derive(Default, Clone)]
pub(crate) struct Constructor {
    pub(crate) edits: Vec<Ed>,
}

impl Constructor {
    /// The new edit conflicts with one already accepted: it overlaps the next
    /// one, or an earlier one reaches it, or both replace text at the same
    /// offset.
    fn conflicts(&self, n: &Ed) -> bool {
        self.edits.iter().any(|q| {
            if q.0 > n.0 {
                n.0 + n.1 > q.0
            } else {
                (q.0 == n.0 && q.1 > 0 && n.1 > 0) || q.0 + q.1 > n.0
            }
        })
    }

    /// Adds keeping the descending order. An insertion at the offset of another
    /// goes BEFORE it in the list: applied later, the new text ends up after
    /// what already existed.
    fn append(&mut self, n: Ed) {
        let i = self.edits.iter().position(|q| q.0 <= n.0).unwrap_or(self.edits.len());
        self.edits.insert(i, n);
    }

    /// Applies a producer's edits atomically: if one conflicts, none goes in
    /// (`_applyProducer`: the local builder is thrown away).
    pub(crate) fn apply_producer(&mut self, edits: impl IntoIterator<Item = Ed>) -> bool {
        let mut local = self.clone();
        for n in edits {
            if local.conflicts(&n) {
                return false;
            }
            local.append(n);
        }
        *self = local;
        true
    }

    /// The text with the edits applied.
    pub(crate) fn apply(&self, text: &str) -> String {
        let mut output = text.to_string();
        for (offset, length, new) in &self.edits {
            let end = (offset + length).min(output.len());
            if *offset <= end && output.is_char_boundary(*offset) && output.is_char_boundary(end) {
                output.replace_range(*offset..end, new);
            }
        }
        output
    }
}

/// The fix of kind `kind` (`quickfix.<id>`) is part of `Fix All`:
/// `canBeAppliedAcrossFiles` for a manual request,
/// `canBeAppliedAutomatically` for an automatic one (the `applicability`
/// table of §13.12.7). Only the fixes DartForge produces are here; the
/// others are `singleLocation` or `acrossSingleFile`.
pub(crate) fn in_batch(kind: &str, auto: bool) -> bool {
    let id = kind.strip_prefix("quickfix.").unwrap_or(kind);
    // `automatically`.
    let auto_ = [
        "remove.unnecessaryCast",
        "remove.nonNullAssertion",
        "replace.withNotNullAware",
        "add.trailingComma",
        "remove.questionMark",
        "makeFinal",
        "remove.methodDeclaration",
    ];
    // `acrossFiles`.
    let across_files = ["remove.unusedCatchClause", "remove.unusedCatchStack"];
    auto_.contains(&id) || (!auto && across_files.contains(&id))
}

/// The message template of the fix's `FixKind` (`fixKind.message`, without
/// substituting `{0}`): the description that becomes the change annotation
/// of `Fix All in Workspace` (§13.12.8). Empty for a kind outside the table.
pub(crate) fn description(kind: &str) -> &'static str {
    match kind.strip_prefix("quickfix.").unwrap_or(kind) {
        "remove.unnecessaryCast" => "Remove unnecessary cast",
        "remove.nonNullAssertion" => "Remove the '!'",
        "replace.withNotNullAware" => "Replace with '{0}'",
        "add.trailingComma" => "Add trailing comma",
        "remove.questionMark" => "Remove the '?'",
        "makeFinal" => "Make final",
        "remove.methodDeclaration" => "Remove method declaration",
        "remove.unusedCatchClause" => "Remove unused 'catch' clause",
        "remove.unusedCatchStack" => "Remove unused stack trace variable",
        _ => "",
    }
}

/// The file is generated (`isGenerated`, plus `.macro.dart`): `Fix All` does
/// not touch it.
pub(crate) fn generated(uri: &str) -> bool {
    [".g.dart", ".pb.dart", ".pbenum.dart", ".pbserver.dart", ".pbjson.dart", ".template.dart", ".macro.dart"]
        .iter()
        .any(|s| uri.ends_with(s))
}

/// `utils.getLinesRange`: the whole lines containing `start..end`, with the
/// last one's line break.
fn whole_lines(text: &str, start: usize, end: usize) -> (usize, usize) {
    let from = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let until = text[end..].find('\n').map_or(text.len(), |i| end + i + 1);
    (from, until)
}

/// Phase B without the sorting lint: one removal (`RemoveUnusedImport`) per
/// `duplicate_import`, `unnecessary_import` or `unused_import` error, in
/// offset order. Only in the library's first unit (without `part of`).
pub(crate) fn remove_imports(text: &str, unit: &CompilationUnit, diagnostics: &[Diagnostic]) -> Vec<Ed> {
    if unit.directives.iter().any(|d| matches!(d.kind, DirectiveKind::PartOf { .. })) {
        return Vec::new();
    }
    let mut targets: Vec<usize> = diagnostics
        .iter()
        .filter(|d| {
            d.code.is_some_and(|c| {
                matches!(c.info().unique.rsplit('.').next(), Some("DUPLICATE_IMPORT" | "UNNECESSARY_IMPORT" | "UNUSED_IMPORT"))
            })
        })
        .map(|d| d.span.start)
        .collect();
    targets.sort_unstable();
    let mut output = Vec::new();
    for target in targets {
        let directive = unit.directives.iter().find(|d| match &d.kind {
            DirectiveKind::Import { uri, .. } => uri.span.start == target,
            _ => false,
        });
        if let Some(d) = directive
            && d.span.end <= text.len()
        {
            let (from, until) = whole_lines(text, d.span.start, d.span.end);
            output.push((from, until - from, String::new()));
        }
    }
    output
}

fn delta(e: &Ed) -> i64 {
    e.2.len() as i64 - e.1 as i64
}

/// `SourceChangeMerger.merge` for one file: the edits of the passes,
/// concatenated in the order they were applied (each pass assumes the
/// previous ones), become a list in descending offset order, without
/// overlap, relative to the original text.
pub(crate) fn merge(mut edits: Vec<Ed>) -> Vec<Ed> {
    // `_reorder`: each edit moves before those that come before it in the
    // text, going back to the coordinates before them.
    for i in 1..edits.len() {
        let mut current = edits[i].clone();
        let mut j = i;
        while j > 0 {
            let previous = &edits[j - 1];
            let result_end = previous.0 as i64 + previous.2.len() as i64;
            if (current.0 as i64) < result_end {
                break;
            }
            current.0 = (current.0 as i64 - delta(previous)) as usize;
            edits[j] = edits[j - 1].clone();
            j -= 1;
        }
        edits[j] = current;
    }
    // `_merge`: neighbors that touch or overlap become one.
    let mut i = 0;
    while i + 1 < edits.len() {
        let (a, b) = (edits[i].clone(), edits[i + 1].clone());
        let (end_a, fim_b) = (a.0 + a.1, b.0 + b.1);
        if fim_b < a.0 {
            i += 1;
            continue;
        }
        let start = a.0.min(b.0);
        let end = (end_a as i64).max(fim_b as i64 - delta(&a)).max(start as i64) as usize;
        let prefix = if b.0 > a.0 { a.2.get(..b.0 - a.0).unwrap_or("") } else { "" };
        let suffix = if fim_b < a.0 + a.2.len() { a.2.get(fim_b - a.0..).unwrap_or("") } else { "" };
        edits[i] = (start, end - start, format!("{prefix}{}{suffix}", b.2));
        edits.remove(i + 1);
    }
    edits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ed(offset: usize, length: usize, text: &str) -> Ed {
        (offset, length, text.to_string())
    }

    #[test]
    fn constructor_conflicts() {
        let mut c = Constructor::default();
        assert!(c.apply_producer([ed(10, 5, "x")]));
        // Overlaps the existing one.
        assert!(!c.apply_producer([ed(8, 3, "y")]));
        assert!(!c.apply_producer([ed(12, 1, "y")]));
        // Only touches: accepted.
        assert!(c.apply_producer([ed(15, 2, "z")]));
        assert!(c.apply_producer([ed(5, 5, "w")]));
        // Insertion where a replacement starts: conflict.
        assert!(!c.apply_producer([ed(10, 0, "i")]));
        // Atomic: the first edit of the rejected producer does not go in.
        let before = c.edits.len();
        assert!(!c.apply_producer([ed(0, 1, "a"), ed(11, 1, "b")]));
        assert_eq!(c.edits.len(), before);
    }

    #[test]
    fn insertions_at_same_offset_keep_arrival_order() {
        let mut c = Constructor::default();
        assert!(c.apply_producer([ed(1, 0, "A")]));
        assert!(c.apply_producer([ed(1, 0, "B")]));
        assert_eq!(c.apply("xy"), "xABy");
    }

    #[test]
    fn merge_of_two_passes() {
        // Pass 1: `abc` becomes `aXXc` (replaces `b` with `XX`).
        // Pass 2, over `aXXc`: inserts `!` at the end and replaces the `a` with `A`.
        let original = "abc";
        let merged = merge(vec![ed(1, 1, "XX"), ed(4, 0, "!"), ed(0, 1, "A")]);
        let mut c = Constructor::default();
        c.edits = merged;
        assert_eq!(c.apply(original), "AXXc!");
        // Descending offset order.
        assert!(c.edits.windows(2).all(|p| p[0].0 >= p[1].0));
    }

    #[test]
    fn merge_of_overlapping_edits() {
        // Pass 1: `abc` becomes `aXYc`. Pass 2 replaces the `Y` with `Z`.
        let merged = merge(vec![ed(1, 1, "XY"), ed(2, 1, "Z")]);
        assert_eq!(merged, vec![ed(1, 1, "XZ")]);
    }

    #[test]
    fn applicability() {
        assert!(in_batch("quickfix.remove.unnecessaryCast", true));
        assert!(in_batch("quickfix.remove.unusedCatchClause", false));
        assert!(!in_batch("quickfix.remove.unusedCatchClause", true));
        assert!(!in_batch("quickfix.insertSemicolon", false));
        assert!(generated("file:///a/b.g.dart"));
        assert!(!generated("file:///a/b.dart"));
    }
}
