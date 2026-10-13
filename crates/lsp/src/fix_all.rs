//! As peças do `Fix All` (`dart.edit.fixAll`; docs/LSP-ESPECIFICACAO.md
//! §13.12.7): o construtor de edições com a regra de conflito do
//! `addEditForSource` (`AP:src/protocol/protocol_internal.dart:41-84`), a
//! aplicabilidade em lote das correções que o DartForge tem, a remoção de
//! imports da fase B e a fusão das passadas do `SourceChangeMerger`
//! (`AS:src/utilities/source_change_merger.dart:30-166`).
//!
//! Escrito sem compilar nem executar (2026-10-04).

use dartforge_diagnostics::Diagnostic;
use dartforge_frontend::ast::{CompilationUnit, DirectiveKind};

/// Uma edição: offset, comprimento e texto novo (bytes da fonte).
pub(crate) type Ed = (usize, usize, String);

/// As edições de um arquivo numa passada, em ordem DECRESCENTE de offset
/// (aplicáveis em sequência).
#[derive(Default, Clone)]
pub(crate) struct Constructor {
    pub(crate) edits: Vec<Ed>,
}

impl Constructor {
    /// A edição nova conflita com alguma já aceita: sobrepõe a seguinte, ou
    /// uma anterior a alcança, ou as duas trocam texto no mesmo offset.
    fn conflicts(&self, n: &Ed) -> bool {
        self.edits.iter().any(|q| {
            if q.0 > n.0 {
                n.0 + n.1 > q.0
            } else {
                (q.0 == n.0 && q.1 > 0 && n.1 > 0) || q.0 + q.1 > n.0
            }
        })
    }

    /// Acrescenta mantendo a ordem decrescente. Uma inserção no offset de
    /// outra fica ANTES dela na lista: aplicada depois, o texto novo sai
    /// depois do que já existia.
    fn append(&mut self, n: Ed) {
        let i = self.edits.iter().position(|q| q.0 <= n.0).unwrap_or(self.edits.len());
        self.edits.insert(i, n);
    }

    /// Aplica as edições de um produtor de forma atômica: se uma conflita,
    /// nenhuma entra (`_applyProducer`: o construtor local é jogado fora).
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

    /// O texto com as edições aplicadas.
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

/// A correção de espécie `kind` (`quickfix.<id>`) entra no `Fix All`:
/// `canBeAppliedAcrossFiles` no pedido manual, `canBeAppliedAutomatically`
/// no automático (tabela `applicability` da §13.12.7). Só as correções que
/// o DartForge produz estão aqui; as demais são `singleLocation` ou
/// `acrossSingleFile`.
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

/// O modelo da mensagem do `FixKind` da correção (`fixKind.message`, sem
/// substituir `{0}`): a descrição que vira a anotação de mudança do
/// `Fix All in Workspace` (§13.12.8). Vazio para espécie fora da tabela.
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

/// O arquivo é gerado (`isGenerated`, mais `.macro.dart`): o `Fix All` não
/// o toca.
pub(crate) fn generated(uri: &str) -> bool {
    [".g.dart", ".pb.dart", ".pbenum.dart", ".pbserver.dart", ".pbjson.dart", ".template.dart", ".macro.dart"]
        .iter()
        .any(|s| uri.ends_with(s))
}

/// `utils.getLinesRange`: as linhas inteiras que contêm `start..end`, com
/// a quebra da última.
fn whole_lines(text: &str, start: usize, end: usize) -> (usize, usize) {
    let de = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let until = text[end..].find('\n').map_or(text.len(), |i| end + i + 1);
    (de, until)
}

/// A fase B sem lint de ordenação: uma remoção (`RemoveUnusedImport`) por
/// erro `duplicate_import`, `unnecessary_import` ou `unused_import`, em
/// ordem de offset. Só na primeira unidade da biblioteca (sem `part of`).
pub(crate) fn remove_imports(text: &str, unit: &CompilationUnit, diagnostics: &[Diagnostic]) -> Vec<Ed> {
    if unit.directives.iter().any(|d| matches!(d.kind, DirectiveKind::PartOf { .. })) {
        return Vec::new();
    }
    let mut targets: Vec<usize> = diagnostics
        .iter()
        .filter(|d| {
            d.code.is_some_and(|c| {
                matches!(c.info().unico.rsplit('.').next(), Some("DUPLICATE_IMPORT" | "UNNECESSARY_IMPORT" | "UNUSED_IMPORT"))
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
            let (de, until) = whole_lines(text, d.span.start, d.span.end);
            output.push((de, until - de, String::new()));
        }
    }
    output
}

fn delta(e: &Ed) -> i64 {
    e.2.len() as i64 - e.1 as i64
}

/// `SourceChangeMerger.merge` para um arquivo: as edições das passadas,
/// concatenadas na ordem em que foram aplicadas (cada passada pressupõe as
/// anteriores), viram uma lista em ordem decrescente de offset, sem
/// sobreposição, relativa ao texto original.
pub(crate) fn merge(mut edits: Vec<Ed>) -> Vec<Ed> {
    // `_reorder`: cada edição vai para antes das que ficam antes dela no
    // texto, voltando às coordenadas de antes delas.
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
    // `_merge`: vizinhas que se tocam ou se sobrepõem viram uma.
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
        // Sobrepõe a existente.
        assert!(!c.apply_producer([ed(8, 3, "y")]));
        assert!(!c.apply_producer([ed(12, 1, "y")]));
        // Só toca: aceita.
        assert!(c.apply_producer([ed(15, 2, "z")]));
        assert!(c.apply_producer([ed(5, 5, "w")]));
        // Inserção onde começa uma troca: conflita.
        assert!(!c.apply_producer([ed(10, 0, "i")]));
        // Atômico: a primeira edição do produtor recusado não entra.
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
        // Passada 1: `abc` vira `aXXc` (troca `b` por `XX`).
        // Passada 2, sobre `aXXc`: insere `!` no fim e troca o `a` por `A`.
        let original = "abc";
        let merged = merge(vec![ed(1, 1, "XX"), ed(4, 0, "!"), ed(0, 1, "A")]);
        let mut c = Constructor::default();
        c.edits = merged;
        assert_eq!(c.apply(original), "AXXc!");
        // Ordem decrescente de offset.
        assert!(c.edits.windows(2).all(|p| p[0].0 >= p[1].0));
    }

    #[test]
    fn merge_of_overlapping_edits() {
        // Passada 1: `abc` vira `aXYc`. Passada 2 troca o `Y` por `Z`.
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
