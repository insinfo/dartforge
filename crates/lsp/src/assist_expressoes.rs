//! Assistências sobre expressões, portadas dos produtores do
//! `analysis_server` 3.6.2 sobre a árvore no formato do analyzer.
//!
//! | Título | Espécie | Produtor |
//! |---|---|---|
//! | `Convert to 'isNotEmpty'` | `refactor.convert.isNotEmpty` | `ConvertIntoIsNotEmpty` |
//! | `Convert to an int literal` | `refactor.convert.toIntLiteral` | `ConvertToIntLiteral` |
//! | `Convert to a spread`, `Inline invocation of 'addAll'` | `refactor.convert.toSpread`, `refactor.inline` | `ConvertAddAllToSpread` |

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

    /// Os argumentos de uma `ArgumentList`.
    fn argumentos_da_lista(&self, invocacao: usize) -> Vec<usize> {
        self.filhos(invocacao)
            .iter()
            .copied()
            .find(|&k| self.especie(k) == "ArgumentList")
            .map(|l| self.filhos(l).to_vec())
            .unwrap_or_default()
    }

    /// Os elementos de um `ListLiteral` (sem os argumentos de tipo).
    fn elementos_da_lista(&self, lista: usize) -> Vec<usize> {
        self.filhos(lista).iter().copied().filter(|&k| self.especie(k) != "TypeArgumentList").collect()
    }

    /// `ConvertAddAllToSpread` (convert_add_all_to_spread.dart): no nome do
    /// primeiro `..addAll(x)` em cascata sobre um literal de lista, `x` entra
    /// na lista como `...x` (ou `...?a` de `a ?? []`, `if (c) ...a` de
    /// `c ? a : []`); com um literal de lista, os elementos dele
    /// (`Inline invocation of 'addAll'`).
    pub(crate) fn converter_add_all_em_espalhamento(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let nome = self.arvore.localizar(inicio, fim)?;
        if self.especie(nome) != "SimpleIdentifier" {
            return None;
        }
        let invocacao = self.pai(nome)?;
        if self.especie(invocacao) != "MethodInvocation" || self.nome_do_metodo(invocacao) != Some(nome) || self.texto_do_no(nome) != "addAll" {
            return None;
        }
        // `isCascaded`: o operador do nome é `..` ou `?..`.
        let operador = self.token_anterior(self.arvore.nos[nome].inicio)?;
        if !matches!(&self.fonte[operador.start..operador.end], ".." | "?..") {
            return None;
        }
        let argumentos = self.argumentos_da_lista(invocacao);
        if argumentos.len() != 1 {
            return None;
        }
        let argumento = argumentos[0];
        let em_linha = self.especie(argumento) == "ListLiteral";
        let cascata = self.com_pais(invocacao).find(|&k| self.especie(k) == "CascadeExpression")?;
        let fc = self.filhos(cascata);
        let (alvo, primeira) = (*fc.first()?, *fc.get(1)?);
        if self.especie(alvo) != "ListLiteral" || primeira != invocacao {
            return None;
        }
        let lista_vazia = |k: usize| self.especie(k) == "ListLiteral" && self.elementos_da_lista(k).is_empty();
        let mut texto: Option<String> = None;
        match self.especie(argumento) {
            "BinaryExpression" if self.operador_binario(argumento) == Some("??") => {
                let f = self.filhos(argumento);
                if lista_vazia(f[1]) {
                    texto = Some(format!("...?{}", self.texto_do_no(f[0])));
                }
            }
            "ConditionalExpression" => {
                let f = self.filhos(argumento);
                if lista_vazia(f[2]) {
                    texto = Some(format!("if ({}) ...{}", self.texto_do_no(f[0]), self.texto_do_no(f[1])));
                }
            }
            "ListLiteral" => {
                let elementos = self.elementos_da_lista(argumento);
                let (&primeiro, &ultimo) = (elementos.first()?, elementos.last()?);
                texto = Some(self.fonte[self.arvore.nos[primeiro].inicio..self.arvore.nos[ultimo].fim].to_string());
            }
            _ => {}
        }
        let texto = texto.unwrap_or_else(|| format!("...{}", self.texto_do_no(argumento)));
        let elementos_do_alvo = self.elementos_da_lista(alvo);
        let insercao = match elementos_do_alvo.last() {
            Some(&u) => (Span { start: self.arvore.nos[u].fim, end: self.arvore.nos[u].fim }, format!(", {texto}")),
            None => {
                let abre = self.tokens.iter().find(|t| t.span.start >= self.arvore.nos[alvo].inicio && &self.fonte[t.span.start..t.span.end] == "[")?.span;
                (Span { start: abre.end, end: abre.end }, texto)
            }
        };
        let (titulo, especie) = if em_linha {
            ("Inline invocation of 'addAll'", "refactor.inline")
        } else {
            ("Convert to a spread", "refactor.convert.toSpread")
        };
        Some(self.acao_simples(uri, titulo, especie, vec![insercao, (self.arvore.span(invocacao), String::new())]))
    }
}
