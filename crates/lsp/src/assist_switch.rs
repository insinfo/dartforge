//! Assistências de `switch`, portadas dos produtores do `analysis_server`
//! 3.6.2 sobre a árvore no formato do analyzer.
//!
//! | Título | Espécie | Produtor |
//! |---|---|---|
//! | `Convert to switch statement` | `refactor.convert.switchStatement` | `ConvertIfStatementToSwitchStatement`, `ConvertSwitchExpressionToSwitchStatement` |

use crate::acoes::AcaoDeCodigo;
use crate::refatoracoes::Contexto;
use crate::refatoracoes_exec::{Mudanca, Texto, UM_RECUO};
use dartforge_diagnostics::Span;
use dartforge_frontend::ast;
use dartforge_types::{Type, TypeId};

/// Um ramo do `if`: o padrão (`None` no `else`) e o comando.
struct Caso {
    padrao: Option<String>,
    expressao: Option<String>,
    comando: usize,
}

const LITERAIS: &[&str] = &[
    "IntegerLiteral",
    "DoubleLiteral",
    "BooleanLiteral",
    "NullLiteral",
    "SimpleStringLiteral",
    "StringInterpolation",
    "AdjacentStrings",
    "SymbolLiteral",
    "ListLiteral",
    "SetOrMapLiteral",
    "RecordLiteral",
];

impl Contexto<'_> {
    /// O `type` de uma anotação de tipo (`NamedType.type`).
    pub(crate) fn tipo_da_anotacao_no(&self, k: usize) -> Option<TypeId> {
        let s = self.arvore.span(k);
        self.ast.types.iter().enumerate().filter(|(_, t)| t.span == s).find_map(|(i, _)| {
            let id = ast::TypeId(i as u32);
            self.corpos.tipos_de_anotacoes.get(&id).or_else(|| self.p.consulta.outline.tipos_escritos.get(&(self.unidade, id))).copied()
        })
    }

    /// `_patternOfBoolCondition`.
    fn padrao_da_condicao(&self, c: usize) -> Option<(String, String)> {
        match self.especie(c) {
            "BinaryExpression" => {
                let f = self.filhos(c);
                let (e, d) = (*f.first()?, *f.get(1)?);
                let op = self.operador_binario(c)?;
                if op == "!=" && self.especie(d) == "NullLiteral" {
                    return Some((self.texto_do_no(e).to_string(), "_?".into()));
                }
                if matches!(op, "<" | ">" | "<=" | ">=") && LITERAIS.contains(&self.especie(d)) {
                    let inicio_op = self.token_seguinte(self.arvore.nos[e].fim)?.start;
                    return Some((self.texto_do_no(e).to_string(), self.fonte[inicio_op..self.arvore.nos[d].fim].to_string()));
                }
                None
            }
            "IsExpression" => {
                let f = self.filhos(c);
                let (e, t) = (*f.first()?, *f.get(1)?);
                let tipo = self.texto_do_no(t);
                let interface = self.tipo_da_anotacao_no(t).is_some_and(|x| {
                    matches!(self.p.consulta.tabela.get(x), Type::Interface { .. } | Type::FutureOr { .. } | Type::ExtensionType { .. } | Type::Null)
                });
                Some((self.texto_do_no(e).to_string(), if interface { format!("{tipo}()") } else { format!("{tipo} _") }))
            }
            _ => None,
        }
    }

    /// `_buildCases`.
    fn casos_do_if(&self, se: usize) -> Option<Vec<Caso>> {
        let filhos = self.filhos(se);
        let condicao = *filhos.first()?;
        let ramos: Vec<usize> = filhos.iter().copied().filter(|&k| self.e_comando(k)).collect();
        let entao = *ramos.first()?;
        // `_buildThenCase`.
        let primeiro = match filhos.iter().copied().find(|&k| self.especie(k) == "CaseClause") {
            Some(clausula) => {
                if self.especie(condicao) != "SimpleIdentifier" {
                    return None;
                }
                let guardado = *self.filhos(clausula).first()?;
                Caso { padrao: Some(self.texto_do_no(guardado).to_string()), expressao: Some(self.texto_do_no(condicao).to_string()), comando: entao }
            }
            None => {
                let (e, p) = self.padrao_da_condicao(condicao)?;
                Caso { padrao: Some(p), expressao: Some(e), comando: entao }
            }
        };
        let expressao = primeiro.expressao.clone();
        let mut casos = vec![primeiro];
        match ramos.get(1) {
            Some(&senao) if self.especie(senao) == "IfStatement" => {
                for c in self.casos_do_if(senao)? {
                    if c.padrao.is_some() && c.expressao != expressao {
                        return None;
                    }
                    casos.push(c);
                }
            }
            Some(&senao) => casos.push(Caso { padrao: None, expressao: None, comando: senao }),
            None => {}
        }
        Some(casos)
    }

    /// `selfOrBlockStatements`.
    fn comandos_de(&self, c: usize) -> Vec<usize> {
        if self.especie(c) == "Block" { self.filhos(c).to_vec() } else { vec![c] }
    }

    /// `ConvertIfStatementToSwitchStatement`.
    pub(crate) fn if_em_switch(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        if !self.com_padroes() {
            return None;
        }
        let se = self.arvore.localizar(inicio, fim)?;
        if self.especie(se) != "IfStatement" {
            return None;
        }
        let casos = self.casos_do_if(se)?;
        let expressao = casos.first()?.expressao.clone()?;
        let tx = Texto::novo(self.fonte);
        let eol = tx.eol();
        let recuo = tx.prefixo_da_linha(self.arvore.nos[se].inicio).to_string();
        let recuo_do_caso = format!("{recuo}{UM_RECUO}");
        let mut s = format!("switch ({expressao}) {{{eol}");
        for (i, c) in casos.iter().enumerate() {
            match &c.padrao {
                Some(p) => s.push_str(&format!("{recuo_do_caso}case {p}:{eol}")),
                None => s.push_str(&format!("{recuo_do_caso}default:{eol}")),
            }
            // `_writeStatement`: `getLinesRangeStatements` falha sem comandos.
            let comandos = self.comandos_de(c.comando);
            let (&primeiro, &ultimo) = (comandos.first()?, comandos.last()?);
            let _ = i;
            let faixa = tx.faixa_de_linhas(self.arvore.nos[primeiro].inicio, self.arvore.nos[ultimo].fim);
            s.push_str(&tx.trocar_recuo(&self.fonte[faixa.start..faixa.end], &recuo, &recuo_do_caso, true, true));
        }
        s.push_str(&format!("{recuo}}}"));
        Some(AcaoDeCodigo {
            titulo: "Convert to switch statement".into(),
            especie: "refactor.convert.switchStatement".into(),
            edicoes: vec![crate::Edicao { uri: uri.to_string(), span: self.arvore.span(se), texto: s }],
            diagnostico: None,
            criar_arquivo: None,
        })
    }

    /// `_rewriteCases`.
    fn reescrever_casos(&self, uri: &str, m: &mut Mudanca, switch: usize, recuo: &str, antes: &str, ponto_e_virgula: Span) {
        let eol = Texto::novo(self.fonte).eol();
        for &caso in self.filhos(switch).iter().filter(|&&k| self.especie(k) == "SwitchExpressionCase") {
            let f = self.filhos(caso);
            let (Some(&guardado), Some(&expressao)) = (f.first(), f.get(1)) else { continue };
            let g = self.filhos(guardado);
            let curinga = g.len() == 1 && self.especie(g[0]) == "WildcardPattern" && self.filhos(g[0]).is_empty();
            if curinga {
                m.adicionar(uri, self.arvore.span(guardado), "default");
            } else {
                let i = self.arvore.nos[guardado].inicio;
                m.adicionar(uri, Span { start: i, end: i }, "case ");
            }
            m.adicionar(uri, Span { start: self.arvore.nos[guardado].fim, end: self.arvore.nos[expressao].inicio }, format!(":{eol}{recuo}    {antes}"));
            match self.token_seguinte(self.arvore.nos[expressao].fim) {
                Some(v) if &self.fonte[v.start..v.end] == "," => m.adicionar(uri, v, ";"),
                _ => {
                    let e = self.arvore.nos[expressao].fim;
                    m.adicionar(uri, Span { start: e, end: e }, ";");
                }
            }
        }
        m.adicionar(uri, ponto_e_virgula, String::new());
    }

    /// `ConvertSwitchExpressionToSwitchStatement`: a expressão `switch` de uma
    /// declaração, de uma atribuição ou de um `return` vira um comando.
    pub(crate) fn expressao_switch_em_comando(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let switch = self.arvore.localizar(inicio, fim)?;
        if self.especie(switch) != "SwitchExpression" {
            return None;
        }
        let pai = self.pai(switch)?;
        let tx = Texto::novo(self.fonte);
        let eol = tx.eol();
        let mut m = Mudanca::default();
        let mut importar = std::collections::BTreeSet::new();
        match self.especie(pai) {
            "VariableDeclaration" => {
                let lista = self.pai(pai)?;
                let comando = self.pai(lista)?;
                if self.especie(lista) != "VariableDeclarationList"
                    || self.filhos(lista).iter().filter(|&&k| self.especie(k) == "VariableDeclaration").count() != 1
                    || self.especie(comando) != "VariableDeclarationStatement"
                {
                    return None;
                }
                let recuo = tx.prefixo_da_linha(self.arvore.nos[comando].inicio).to_string();
                let nome = self.token_seguinte(self.arvore.nos[pai].inicio)?;
                let tem_tipo = self.filhos(lista).iter().any(|&k| matches!(self.especie(k), "NamedType" | "GenericFunctionType" | "RecordTypeAnnotation"));
                if !tem_tipo {
                    let mut k = self.token_seguinte(self.arvore.nos[lista].inicio)?;
                    if &self.fonte[k.start..k.end] == "late" {
                        k = self.token_seguinte(k.end)?;
                    }
                    let palavra = &self.fonte[k.start..k.end];
                    if matches!(palavra, "final" | "var" | "const") {
                        let mut escritor = crate::escrever_tipo::Escritor::novo(self, k.start);
                        let tipo = self.expr_do_no(switch).and_then(|x| self.corpos.get_type(x));
                        let escrito = escritor.escrever_tipo(tipo, false).unwrap_or_default();
                        importar = escritor.importar;
                        if palavra == "final" {
                            m.adicionar(uri, Span { start: nome.start, end: nome.start }, format!("{escrito} "));
                        } else {
                            m.adicionar(uri, k, escrito);
                        }
                    }
                }
                m.adicionar(uri, Span { start: nome.end, end: self.arvore.nos[switch].inicio }, format!(";{eol}{recuo}"));
                let ponto = self.token_anterior(self.arvore.nos[comando].fim)?;
                self.reescrever_casos(uri, &mut m, switch, &recuo, &format!("{} = ", &self.fonte[nome.start..nome.end]), ponto);
            }
            "AssignmentExpression" => {
                let f = self.filhos(pai);
                if f.get(1) != Some(&switch) {
                    return None;
                }
                let variavel = *f.first()?;
                let comando = self.pai(pai)?;
                if self.especie(variavel) != "SimpleIdentifier" || self.especie(comando) != "ExpressionStatement" {
                    return None;
                }
                let ponto = self.token_anterior(self.arvore.nos[comando].fim).filter(|s| &self.fonte[s.start..s.end] == ";")?;
                let recuo = tx.prefixo_da_linha(self.arvore.nos[comando].inicio).to_string();
                m.adicionar(uri, Span { start: self.arvore.nos[variavel].inicio, end: self.arvore.nos[switch].inicio }, String::new());
                self.reescrever_casos(uri, &mut m, switch, &recuo, &format!("{} = ", self.texto_do_no(variavel)), ponto);
            }
            "ReturnStatement" => {
                let recuo = tx.prefixo_da_linha(self.arvore.nos[pai].inicio).to_string();
                m.adicionar(uri, Span { start: self.arvore.nos[pai].inicio, end: self.arvore.nos[switch].inicio }, String::new());
                let ponto = self.token_anterior(self.arvore.nos[pai].fim)?;
                self.reescrever_casos(uri, &mut m, switch, &recuo, "return ", ponto);
            }
            _ => return None,
        }
        crate::refatoracoes_mover::imports_do_builder(self, &mut m, &importar);
        if m.conflito.is_some() {
            return None;
        }
        Some(AcaoDeCodigo {
            titulo: "Convert to switch statement".into(),
            especie: "refactor.convert.switchStatement".into(),
            edicoes: m.arquivos.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostico: None,
            criar_arquivo: None,
        })
    }
}
