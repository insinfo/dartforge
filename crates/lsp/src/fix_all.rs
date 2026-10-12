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
pub(crate) struct Construtor {
    pub(crate) edicoes: Vec<Ed>,
}

impl Construtor {
    /// A edição nova conflita com alguma já aceita: sobrepõe a seguinte, ou
    /// uma anterior a alcança, ou as duas trocam texto no mesmo offset.
    fn conflita(&self, n: &Ed) -> bool {
        self.edicoes.iter().any(|q| {
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
    fn acrescentar(&mut self, n: Ed) {
        let i = self.edicoes.iter().position(|q| q.0 <= n.0).unwrap_or(self.edicoes.len());
        self.edicoes.insert(i, n);
    }

    /// Aplica as edições de um produtor de forma atômica: se uma conflita,
    /// nenhuma entra (`_applyProducer`: o construtor local é jogado fora).
    pub(crate) fn aplicar_produtor(&mut self, edicoes: impl IntoIterator<Item = Ed>) -> bool {
        let mut local = self.clone();
        for n in edicoes {
            if local.conflita(&n) {
                return false;
            }
            local.acrescentar(n);
        }
        *self = local;
        true
    }

    /// O texto com as edições aplicadas.
    pub(crate) fn aplicar(&self, texto: &str) -> String {
        let mut saida = texto.to_string();
        for (offset, comprimento, novo) in &self.edicoes {
            let fim = (offset + comprimento).min(saida.len());
            if *offset <= fim && saida.is_char_boundary(*offset) && saida.is_char_boundary(fim) {
                saida.replace_range(*offset..fim, novo);
            }
        }
        saida
    }
}

/// A correção de espécie `especie` (`quickfix.<id>`) entra no `Fix All`:
/// `canBeAppliedAcrossFiles` no pedido manual, `canBeAppliedAutomatically`
/// no automático (tabela `applicability` da §13.12.7). Só as correções que
/// o DartForge produz estão aqui; as demais são `singleLocation` ou
/// `acrossSingleFile`.
pub(crate) fn em_lote(especie: &str, automatico: bool) -> bool {
    let id = especie.strip_prefix("quickfix.").unwrap_or(especie);
    // `automatically`.
    let automaticas = [
        "remove.unnecessaryCast",
        "remove.nonNullAssertion",
        "replace.withNotNullAware",
        "add.trailingComma",
        "remove.questionMark",
        "makeFinal",
        "remove.methodDeclaration",
    ];
    // `acrossFiles`.
    let entre_arquivos = ["remove.unusedCatchClause", "remove.unusedCatchStack"];
    automaticas.contains(&id) || (!automatico && entre_arquivos.contains(&id))
}

/// O modelo da mensagem do `FixKind` da correção (`fixKind.message`, sem
/// substituir `{0}`): a descrição que vira a anotação de mudança do
/// `Fix All in Workspace` (§13.12.8). Vazio para espécie fora da tabela.
pub(crate) fn descricao(especie: &str) -> &'static str {
    match especie.strip_prefix("quickfix.").unwrap_or(especie) {
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
pub(crate) fn gerado(uri: &str) -> bool {
    [".g.dart", ".pb.dart", ".pbenum.dart", ".pbserver.dart", ".pbjson.dart", ".template.dart", ".macro.dart"]
        .iter()
        .any(|s| uri.ends_with(s))
}

/// `utils.getLinesRange`: as linhas inteiras que contêm `inicio..fim`, com
/// a quebra da última.
fn linhas_inteiras(texto: &str, inicio: usize, fim: usize) -> (usize, usize) {
    let de = texto[..inicio].rfind('\n').map_or(0, |i| i + 1);
    let ate = texto[fim..].find('\n').map_or(texto.len(), |i| fim + i + 1);
    (de, ate)
}

/// A fase B sem lint de ordenação: uma remoção (`RemoveUnusedImport`) por
/// erro `duplicate_import`, `unnecessary_import` ou `unused_import`, em
/// ordem de offset. Só na primeira unidade da biblioteca (sem `part of`).
pub(crate) fn remover_imports(texto: &str, unidade: &CompilationUnit, diagnosticos: &[Diagnostic]) -> Vec<Ed> {
    if unidade.directives.iter().any(|d| matches!(d.kind, DirectiveKind::PartOf { .. })) {
        return Vec::new();
    }
    let mut alvos: Vec<usize> = diagnosticos
        .iter()
        .filter(|d| {
            d.code.is_some_and(|c| {
                matches!(c.info().unico.rsplit('.').next(), Some("DUPLICATE_IMPORT" | "UNNECESSARY_IMPORT" | "UNUSED_IMPORT"))
            })
        })
        .map(|d| d.span.start)
        .collect();
    alvos.sort_unstable();
    let mut saida = Vec::new();
    for alvo in alvos {
        let diretiva = unidade.directives.iter().find(|d| match &d.kind {
            DirectiveKind::Import { uri, .. } => uri.span.start == alvo,
            _ => false,
        });
        if let Some(d) = diretiva
            && d.span.end <= texto.len()
        {
            let (de, ate) = linhas_inteiras(texto, d.span.start, d.span.end);
            saida.push((de, ate - de, String::new()));
        }
    }
    saida
}

fn delta(e: &Ed) -> i64 {
    e.2.len() as i64 - e.1 as i64
}

/// `SourceChangeMerger.merge` para um arquivo: as edições das passadas,
/// concatenadas na ordem em que foram aplicadas (cada passada pressupõe as
/// anteriores), viram uma lista em ordem decrescente de offset, sem
/// sobreposição, relativa ao texto original.
pub(crate) fn fundir(mut edicoes: Vec<Ed>) -> Vec<Ed> {
    // `_reorder`: cada edição vai para antes das que ficam antes dela no
    // texto, voltando às coordenadas de antes delas.
    for i in 1..edicoes.len() {
        let mut atual = edicoes[i].clone();
        let mut j = i;
        while j > 0 {
            let anterior = &edicoes[j - 1];
            let fim_do_resultado = anterior.0 as i64 + anterior.2.len() as i64;
            if (atual.0 as i64) < fim_do_resultado {
                break;
            }
            atual.0 = (atual.0 as i64 - delta(anterior)) as usize;
            edicoes[j] = edicoes[j - 1].clone();
            j -= 1;
        }
        edicoes[j] = atual;
    }
    // `_merge`: vizinhas que se tocam ou se sobrepõem viram uma.
    let mut i = 0;
    while i + 1 < edicoes.len() {
        let (a, b) = (edicoes[i].clone(), edicoes[i + 1].clone());
        let (fim_a, fim_b) = (a.0 + a.1, b.0 + b.1);
        if fim_b < a.0 {
            i += 1;
            continue;
        }
        let inicio = a.0.min(b.0);
        let fim = (fim_a as i64).max(fim_b as i64 - delta(&a)).max(inicio as i64) as usize;
        let prefixo = if b.0 > a.0 { a.2.get(..b.0 - a.0).unwrap_or("") } else { "" };
        let sufixo = if fim_b < a.0 + a.2.len() { a.2.get(fim_b - a.0..).unwrap_or("") } else { "" };
        edicoes[i] = (inicio, fim - inicio, format!("{prefixo}{}{sufixo}", b.2));
        edicoes.remove(i + 1);
    }
    edicoes
}

#[cfg(test)]
mod testes {
    use super::*;

    fn ed(offset: usize, comprimento: usize, texto: &str) -> Ed {
        (offset, comprimento, texto.to_string())
    }

    #[test]
    fn conflitos_do_construtor() {
        let mut c = Construtor::default();
        assert!(c.aplicar_produtor([ed(10, 5, "x")]));
        // Sobrepõe a existente.
        assert!(!c.aplicar_produtor([ed(8, 3, "y")]));
        assert!(!c.aplicar_produtor([ed(12, 1, "y")]));
        // Só toca: aceita.
        assert!(c.aplicar_produtor([ed(15, 2, "z")]));
        assert!(c.aplicar_produtor([ed(5, 5, "w")]));
        // Inserção onde começa uma troca: conflita.
        assert!(!c.aplicar_produtor([ed(10, 0, "i")]));
        // Atômico: a primeira edição do produtor recusado não entra.
        let antes = c.edicoes.len();
        assert!(!c.aplicar_produtor([ed(0, 1, "a"), ed(11, 1, "b")]));
        assert_eq!(c.edicoes.len(), antes);
    }

    #[test]
    fn insercoes_no_mesmo_offset_saem_na_ordem_de_chegada() {
        let mut c = Construtor::default();
        assert!(c.aplicar_produtor([ed(1, 0, "A")]));
        assert!(c.aplicar_produtor([ed(1, 0, "B")]));
        assert_eq!(c.aplicar("xy"), "xABy");
    }

    #[test]
    fn fusao_de_duas_passadas() {
        // Passada 1: `abc` vira `aXXc` (troca `b` por `XX`).
        // Passada 2, sobre `aXXc`: insere `!` no fim e troca o `a` por `A`.
        let original = "abc";
        let fundidas = fundir(vec![ed(1, 1, "XX"), ed(4, 0, "!"), ed(0, 1, "A")]);
        let mut c = Construtor::default();
        c.edicoes = fundidas;
        assert_eq!(c.aplicar(original), "AXXc!");
        // Ordem decrescente de offset.
        assert!(c.edicoes.windows(2).all(|p| p[0].0 >= p[1].0));
    }

    #[test]
    fn fusao_de_edicoes_sobrepostas() {
        // Passada 1: `abc` vira `aXYc`. Passada 2 troca o `Y` por `Z`.
        let fundidas = fundir(vec![ed(1, 1, "XY"), ed(2, 1, "Z")]);
        assert_eq!(fundidas, vec![ed(1, 1, "XZ")]);
    }

    #[test]
    fn aplicabilidade() {
        assert!(em_lote("quickfix.remove.unnecessaryCast", true));
        assert!(em_lote("quickfix.remove.unusedCatchClause", false));
        assert!(!em_lote("quickfix.remove.unusedCatchClause", true));
        assert!(!em_lote("quickfix.insertSemicolon", false));
        assert!(gerado("file:///a/b.g.dart"));
        assert!(!gerado("file:///a/b.dart"));
    }
}
