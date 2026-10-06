//! Assistências sobre expressões, portadas dos produtores do
//! `analysis_server` 3.6.2 sobre a árvore no formato do analyzer.
//!
//! | Título | Espécie | Produtor |
//! |---|---|---|
//! | `Convert to 'isNotEmpty'` | `refactor.convert.isNotEmpty` | `ConvertIntoIsNotEmpty` |
//! | `Convert to an int literal` | `refactor.convert.toIntLiteral` | `ConvertToIntLiteral` |

use crate::acoes::AcaoDeCodigo;
use crate::refatoracoes::Contexto;
use crate::Edicao;
use dartforge_diagnostics::Span;
use dartforge_elements::model::FunctionElementId;
use dartforge_types::{MemberRef, Resolved};

impl Contexto<'_> {
    fn acao_simples(&self, uri: &str, titulo: &str, especie: &str, edicoes: Vec<(Span, String)>) -> AcaoDeCodigo {
        AcaoDeCodigo {
            titulo: titulo.into(),
            especie: especie.into(),
            edicoes: edicoes.into_iter().map(|(span, texto)| Edicao { uri: uri.to_string(), span, texto }).collect(),
            diagnostico: None,
            criar_arquivo: None,
        }
    }

    /// `ConvertIntoIsNotEmpty` (convert_into_is_not_empty.dart): `!x.isEmpty`
    /// vira `x.isNotEmpty` quando quem declara o `isEmpty` resolvido declara
    /// também um `isNotEmpty` (`getChildren`).
    pub(crate) fn converter_em_is_not_empty(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        if self.especie(no) != "SimpleIdentifier" {
            return None;
        }
        let acesso = self.pai(no)?;
        if !matches!(self.especie(acesso), "PropertyAccess" | "PrefixedIdentifier") {
            return None;
        }
        let identificador = *self.filhos(acesso).last()?;
        // O elemento do `isEmpty` (o getter ou o campo).
        let x = self.expr_do_no(acesso)?;
        let prog = self.p.programa();
        let nome_e = |f: FunctionElementId| self.p.nome(prog.function(f).name).trim_end_matches('=').to_string();
        let (nome, dono_classe, dono_extensao) = match self.corpos.get_resolved(x)? {
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } => {
                let fe = prog.function(*f);
                (nome_e(*f), fe.class, fe.extension)
            }
            Resolved::Member { member: MemberRef::Variable(v), .. } => {
                let ve = prog.variable(*v);
                (self.p.nome(ve.name).to_string(), ve.class, ve.extension)
            }
            _ => return None,
        };
        if nome != "isEmpty" {
            return None;
        }
        let tem_is_not_empty = |instancia: &std::collections::HashMap<dartforge_intern::SymbolId, FunctionElementId>,
                                estaticos: &std::collections::HashMap<dartforge_intern::SymbolId, FunctionElementId>,
                                campos: &[dartforge_elements::model::VariableId]| {
            instancia.keys().chain(estaticos.keys()).any(|s| self.p.nome(*s).trim_end_matches('=') == "isNotEmpty")
                || campos.iter().any(|v| self.p.nome(prog.variable(*v).name) == "isNotEmpty")
        };
        let tem = match (dono_classe, dono_extensao) {
            (Some(c), _) => {
                let ce = prog.class(c);
                tem_is_not_empty(&ce.instance_members, &ce.static_members, &ce.fields)
            }
            (None, Some(e)) => {
                let ee = prog.extension(e);
                tem_is_not_empty(&ee.instance_members, &ee.static_members, &ee.fields)
            }
            _ => false,
        };
        if !tem {
            return None;
        }
        let prefixo = self.pai(acesso)?;
        if self.especie(prefixo) != "PrefixExpression" {
            return None;
        }
        let operador = self.token_seguinte(self.arvore.nos[prefixo].inicio)?;
        if &self.fonte[operador.start..operador.end] != "!" {
            return None;
        }
        Some(self.acao_simples(
            uri,
            "Convert to 'isNotEmpty'",
            "refactor.convert.isNotEmpty",
            vec![
                // `range.startStart(prefixExpression, prefixExpression.operand)`.
                (Span { start: self.arvore.nos[prefixo].inicio, end: self.arvore.nos[acesso].inicio }, String::new()),
                (self.arvore.span(identificador), "isNotEmpty".into()),
            ],
        ))
    }

    /// `ConvertToIntLiteral` (convert_to_int_literal.dart): um literal
    /// `double` de valor inteiro vira `int` (cortado no `.`, que preserva os
    /// separadores de dígitos, quando não há expoente).
    pub(crate) fn converter_em_literal_int(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        if self.especie(no) != "DoubleLiteral" {
            return None;
        }
        let span = self.arvore.span(no);
        let lexema = &self.fonte[span.start..span.end];
        let valor: f64 = lexema.replace('_', "").parse().ok()?;
        // `value.truncate()` lança em infinito e NaN; fora da faixa do `int`
        // o resultado não é igual ao valor.
        if !valor.is_finite() || valor.trunc() != valor || valor.abs() >= 9.223_372_036_854_775_808e18 {
            return None;
        }
        let inteiro = valor as i64;
        let edicao = match lexema.find('.') {
            Some(ponto) if ponto > 0 && !lexema.to_lowercase().contains('e') => (Span { start: span.start + ponto, end: span.end }, String::new()),
            _ => (span, inteiro.to_string()),
        };
        Some(self.acao_simples(uri, "Convert to an int literal", "refactor.convert.toIntLiteral", vec![edicao]))
    }
}
