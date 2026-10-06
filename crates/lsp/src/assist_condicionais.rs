//! Assistências de expressões condicionais, portadas dos produtores do
//! `analysis_server` 3.6.2 sobre a árvore no formato do analyzer.
//!
//! | Título | Espécie | Produtor |
//! |---|---|---|
//! | `Replace conditional with 'if-else'` | `refactor.convert.conditionalToIfElse` | `ReplaceConditionalWithIfElse` |
//! | `Split && condition` | `refactor.splitIfConjunction` | `SplitAndCondition` |
//! | `Convert to use '?.'` | `refactor.convert.toNullAware` | `ConvertToNullAware` |

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

    /// `isOperatorSelected` (`CorrectionProducer`): a seleção fica entre os
    /// operandos, ou é exatamente a expressão quando nenhum operando é
    /// binário.
    pub(crate) fn operador_selecionado(&self, binaria: usize, inicio: usize, fim: usize) -> bool {
        let f = self.filhos(binaria);
        let (Some(&esquerda), Some(&direita)) = (f.first(), f.get(1)) else { return false };
        let (e, d) = (&self.arvore.nos[esquerda], &self.arvore.nos[direita]);
        if inicio >= e.fim && fim <= d.inicio {
            return true;
        }
        if inicio == e.inicio && fim == d.fim {
            return self.especie(esquerda) != "BinaryExpression" && self.especie(direita) != "BinaryExpression";
        }
        false
    }

    /// O operador de uma expressão binária (o texto entre os operandos).
    pub(crate) fn operador_binario(&self, binaria: usize) -> Option<&str> {
        let f = self.filhos(binaria);
        let (e, d) = (*f.first()?, *f.get(1)?);
        Some(self.fonte[self.arvore.nos[e].fim..self.arvore.nos[d].inicio].trim())
    }

    /// `SplitAndCondition` (split_and_condition.dart): com o `&&` de primeiro
    /// nível da condição de um `if` sem `else` selecionado, a parte direita
    /// vira um `if` aninhado em volta do `then`, que ganha um recuo.
    pub(crate) fn dividir_condicao_e(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let binaria = self.arvore.localizar(inicio, fim)?;
        if self.especie(binaria) != "BinaryExpression" || !self.operador_selecionado(binaria, inicio, fim) || self.operador_binario(binaria)? != "&&" {
            return None;
        }
        let comando = self.com_pais(binaria).find(|&k| self.e_comando(k))?;
        if self.especie(comando) != "IfStatement" {
            return None;
        }
        let comandos_do_if: Vec<usize> = self.filhos(comando).iter().copied().filter(|&k| self.e_comando(k)).collect();
        if comandos_do_if.len() != 1 {
            // Sem suporte a `else`.
            return None;
        }
        let entao = comandos_do_if[0];
        // A condição de primeiro nível: sobe pelos `&&`.
        let mut condicao = binaria;
        while let Some(p) = self.pai(condicao) {
            if self.especie(p) == "BinaryExpression" && self.operador_binario(p) == Some("&&") {
                condicao = p;
            } else {
                break;
            }
        }
        if self.filhos(comando).first() != Some(&condicao) {
            return None;
        }
        let tx = Texto::novo(self.fonte);
        let eol = tx.eol();
        let prefixo = self.prefixo_do_no(comando);
        let f = self.filhos(binaria);
        let (esquerda, direita) = (f[0], f[1]);
        let direita_fonte = &self.fonte[self.arvore.nos[direita].inicio..self.arvore.nos[condicao].fim];
        let mut m = Mudanca::default();
        // Tira "&& direita".
        m.adicionar(uri, Span { start: self.arvore.nos[esquerda].fim, end: self.arvore.nos[condicao].fim }, String::new());
        let comandos: Vec<usize> = if self.especie(entao) == "Block" {
            let bloco = self.arvore.span(entao);
            m.adicionar(uri, Span { start: bloco.start + 1, end: bloco.start + 1 }, format!("{eol}{prefixo}{UM_RECUO}if ({direita_fonte}) {{"));
            m.adicionar(uri, Span { start: bloco.end - 1, end: bloco.end - 1 }, format!("{UM_RECUO}}}{eol}{prefixo}"));
            self.filhos(entao).to_vec()
        } else {
            let fecha = self.token_anterior(self.arvore.nos[entao].inicio)?;
            m.adicionar(uri, Span { start: fecha.start + 1, end: fecha.start + 1 }, format!("{eol}{prefixo}{UM_RECUO}if ({direita_fonte})"));
            vec![entao]
        };
        // O recuo dos comandos do `then` (`getLinesRangeStatements`, que
        // falha sem comandos).
        let (&primeiro, &ultimo) = (comandos.first()?, comandos.last()?);
        let linhas = tx.faixa_de_linhas(self.arvore.nos[primeiro].inicio, self.arvore.nos[ultimo].fim);
        let velho = format!("{prefixo}{UM_RECUO}");
        let novo = format!("{velho}{UM_RECUO}");
        m.adicionar(uri, linhas, tx.trocar_recuo(&self.fonte[linhas.start..linhas.end], &velho, &novo, true, true));
        if m.conflito.is_some() {
            return None;
        }
        Some(AcaoDeCodigo {
            titulo: "Split && condition".into(),
            especie: "refactor.splitIfConjunction".into(),
            edicoes: m.arquivos.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostico: None,
            criar_arquivo: None,
        })
    }

    /// `unParenthesized`.
    pub(crate) fn sem_parenteses(&self, mut n: usize) -> usize {
        while self.especie(n) == "ParenthesizedExpression" {
            match self.filhos(n).first() {
                Some(&f) => n = f,
                None => break,
            }
        }
        n
    }

    /// O `toString` de uma expressão (o `toSource`).
    fn como_texto(&self, n: usize) -> String {
        crate::assist_lacos::como_fonte(self.texto_do_no(n))
    }

    /// O operador de acesso (`.`, `?.`, `..`) entre o alvo `alvo` e o nome
    /// seguinte.
    fn operador_de_acesso(&self, alvo: usize) -> Option<Span> {
        let s = self.token_seguinte(self.arvore.nos[alvo].fim)?;
        matches!(&self.fonte[s.start..s.end], "." | "?." | ".." | "?..").then_some(s)
    }

    /// `ConvertToNullAware` (convert_to_null_aware.dart): `x == null ? null :
    /// x.y` (ou com `!=`) vira `x?.y`.
    pub(crate) fn converter_em_null_aware(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        let mut alvo = no;
        if let Some(p) = self.pai(no)
            && self.especie(p) == "BinaryExpression"
            && let Some(a) = self.pai(p)
            && self.especie(a) == "ConditionalExpression"
        {
            alvo = a;
        }
        if self.especie(alvo) != "ConditionalExpression" {
            return None;
        }
        let f = self.filhos(alvo);
        let (condicao, entao, senao) = (self.sem_parenteses(*f.first()?), *f.get(1)?, *f.get(2)?);
        if self.especie(condicao) != "BinaryExpression" {
            return None;
        }
        let fc = self.filhos(condicao);
        let (esquerda, direita) = (*fc.first()?, *fc.get(1)?);
        let nulo = |k: usize| self.especie(k) == "NullLiteral";
        let texto_da_condicao = if nulo(esquerda) && !nulo(direita) {
            self.como_texto(direita)
        } else if nulo(direita) && !nulo(esquerda) {
            self.como_texto(esquerda)
        } else {
            return None;
        };
        let (expressao_nula, nao_nula) = match self.operador_binario(condicao)? {
            "==" => (entao, senao),
            "!=" => (senao, entao),
            _ => return None,
        };
        if !nulo(self.sem_parenteses(expressao_nula)) {
            return None;
        }
        let mut resultado = self.sem_parenteses(nao_nula);
        let mut operador: Option<Span> = None;
        loop {
            match self.especie(resultado) {
                "PrefixedIdentifier" => {
                    let prefixo = *self.filhos(resultado).first()?;
                    operador = Some(self.operador_de_acesso(prefixo)?);
                    resultado = prefixo;
                }
                "MethodInvocation" | "PropertyAccess" => {
                    // Sem alvo, o `default` do produtor.
                    let primeiro = *self.filhos(resultado).first()?;
                    let op = self.operador_de_acesso(primeiro)?;
                    if op.start < self.arvore.nos[primeiro].fim {
                        return None;
                    }
                    if self.especie(resultado) == "MethodInvocation" && self.nome_do_metodo(resultado) == Some(primeiro) {
                        return None;
                    }
                    operador = Some(op);
                    resultado = primeiro;
                }
                "PostfixExpression" if self.fonte[self.arvore.nos[resultado].inicio..self.arvore.nos[resultado].fim].ends_with('!') => {
                    resultado = *self.filhos(resultado).first()?;
                }
                _ => return None,
            }
            if self.como_texto(resultado) == texto_da_condicao {
                break;
            }
        }
        let (faixa, interrogacao) = match operador {
            Some(op) => {
                let q = if &self.fonte[op.start..op.end] == "." { "?" } else { "" };
                (Span { start: self.arvore.nos[resultado].fim, end: op.start }, q)
            }
            None if self.pai(resultado).is_some_and(|p| self.especie(p) == "PostfixExpression") => {
                (Span { start: self.arvore.nos[resultado].fim, end: self.arvore.nos[self.pai(resultado)?].fim }, "")
            }
            None => return None,
        };
        let mut m = Mudanca::default();
        m.adicionar(uri, Span { start: self.arvore.nos[alvo].inicio, end: self.arvore.nos[nao_nula].inicio }, String::new());
        m.adicionar(uri, faixa, interrogacao.to_string());
        m.adicionar(uri, Span { start: self.arvore.nos[nao_nula].fim, end: self.arvore.nos[alvo].fim }, String::new());
        if m.conflito.is_some() {
            return None;
        }
        Some(AcaoDeCodigo {
            titulo: "Convert to use '?.'".into(),
            especie: "refactor.convert.toNullAware".into(),
            edicoes: m.arquivos.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostico: None,
            criar_arquivo: None,
        })
    }
}
