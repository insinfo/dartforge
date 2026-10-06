//! Assistências de expressões condicionais, portadas dos produtores do
//! `analysis_server` 3.6.2 sobre a árvore no formato do analyzer.
//!
//! | Título | Espécie | Produtor |
//! |---|---|---|
//! | `Replace conditional with 'if-else'` | `refactor.convert.conditionalToIfElse` | `ReplaceConditionalWithIfElse` |

use crate::acoes::AcaoDeCodigo;
use crate::refatoracoes::Contexto;
use crate::refatoracoes_exec::{Mudanca, Texto, UM_RECUO};
use dartforge_diagnostics::Span;

impl Contexto<'_> {
    /// `ReplaceConditionalWithIfElse` (replace_conditional_with_if_else.dart):
    /// no comando ancestral do nó da seleção, `T v = c ? a : b;`,
    /// `v = c ? a : b;` ou `return c ? a : b;` vira um `if`/`else`.
    pub(crate) fn condicional_em_if_else(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        let comando = self.com_pais(no).find(|&k| self.e_comando(k))?;
        let tx = Texto::novo(self.fonte);
        let eol = tx.eol();
        let prefixo = self.prefixo_do_no(comando);
        let mut m = Mudanca::default();
        let partes = |cx: &Self, condicional: usize| -> Option<(String, String, String)> {
            let f = cx.filhos(condicional);
            Some((cx.texto_do_no(*f.first()?).to_string(), cx.texto_do_no(*f.get(1)?).to_string(), cx.texto_do_no(*f.get(2)?).to_string()))
        };
        match self.especie(comando) {
            "VariableDeclarationStatement" => {
                let lista = *self.filhos(comando).first()?;
                let tem_tipo = self.filhos(lista).iter().any(|&k| self.especie(k) != "VariableDeclaration" && self.especie(k) != "Annotation");
                let mut importar = std::collections::BTreeSet::new();
                for &variavel in self.filhos(lista).iter().filter(|&&k| self.especie(k) == "VariableDeclaration") {
                    let Some(&condicional) = self.filhos(variavel).first() else { continue };
                    if self.especie(condicional) != "ConditionalExpression" {
                        continue;
                    }
                    let nome = self.token_seguinte(self.arvore.nos[variavel].inicio)?;
                    if !tem_tipo {
                        let mut escritor = crate::escrever_tipo::Escritor::novo(self, nome.start);
                        let tipo = self.expr_do_no(condicional).and_then(|x| self.corpos.get_type(x));
                        let escrito = escritor.escrever_tipo(tipo, false).unwrap_or_default();
                        importar.extend(escritor.importar.iter().copied());
                        // A palavra-chave da lista (`late` à parte).
                        let mut k = self.token_seguinte(self.arvore.nos[lista].inicio)?;
                        if &self.fonte[k.start..k.end] == "late" {
                            k = self.token_seguinte(k.end)?;
                        }
                        if &self.fonte[k.start..k.end] == "var" {
                            m.adicionar(uri, k, escrito);
                        } else {
                            m.adicionar(uri, Span { start: nome.start, end: nome.start }, format!("{escrito} "));
                        }
                    }
                    // `range.endEnd(variable.name, conditional)`.
                    m.adicionar(uri, Span { start: nome.end, end: self.arvore.nos[condicional].fim }, String::new());
                    let (c, a, b) = partes(self, condicional)?;
                    let n = &self.fonte[nome.start..nome.end];
                    let src = format!(
                        "{eol}{prefixo}if ({c}) {{{eol}{prefixo}{UM_RECUO}{n} = {a};{eol}{prefixo}}} else {{{eol}{prefixo}{UM_RECUO}{n} = {b};{eol}{prefixo}}}"
                    );
                    let fim_do_comando = self.arvore.nos[comando].fim;
                    m.adicionar(uri, Span { start: fim_do_comando, end: fim_do_comando }, src);
                }
                crate::refatoracoes_mover::imports_do_builder(self, &mut m, &importar);
            }
            "ExpressionStatement" => {
                let atribuicao = *self.filhos(comando).first()?;
                if self.especie(atribuicao) != "AssignmentExpression" {
                    return None;
                }
                let f = self.filhos(atribuicao);
                let (&esquerda, &condicional) = (f.first()?, f.get(1)?);
                let operador = self.fonte[self.arvore.nos[esquerda].fim..self.arvore.nos[condicional].inicio].trim();
                if operador != "=" || self.especie(condicional) != "ConditionalExpression" {
                    return None;
                }
                let (c, a, b) = partes(self, condicional)?;
                let n = self.texto_do_no(esquerda);
                let src = format!(
                    "if ({c}) {{{eol}{prefixo}{UM_RECUO}{n} = {a};{eol}{prefixo}}} else {{{eol}{prefixo}{UM_RECUO}{n} = {b};{eol}{prefixo}}}"
                );
                m.adicionar(uri, self.arvore.span(comando), src);
            }
            "ReturnStatement" => {
                let &condicional = self.filhos(comando).first()?;
                if self.especie(condicional) != "ConditionalExpression" {
                    return None;
                }
                let (c, a, b) = partes(self, condicional)?;
                let src = format!(
                    "if ({c}) {{{eol}{prefixo}{UM_RECUO}return {a};{eol}{prefixo}}} else {{{eol}{prefixo}{UM_RECUO}return {b};{eol}{prefixo}}}"
                );
                m.adicionar(uri, self.arvore.span(comando), src);
            }
            _ => return None,
        }
        if m.conflito.is_some() || m.arquivos.is_empty() {
            return None;
        }
        Some(AcaoDeCodigo {
            titulo: "Replace conditional with 'if-else'".into(),
            especie: "refactor.convert.conditionalToIfElse".into(),
            edicoes: m.arquivos.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostico: None,
            criar_arquivo: None,
        })
    }
}
