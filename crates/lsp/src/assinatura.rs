//! Ajuda de assinatura (`textDocument/signatureHelp`) no formato do servidor
//! do Dart (`DartUnitSignatureComputer` + `toSignatureHelp`, ver
//! `docs/LSP.md`, "Paridade com o servidor do Dart"):
//!
//! * a lista de argumentos mais interna que contém o cursor, de uma chamada
//!   de método ou função (`f(…)`, `a.m(…)`) ou de uma criação de instância
//!   (`A(…)`, `A.nome(…)`, `new p.A<T>.nome(…)`); uma expressão de função
//!   entre o cursor e a lista interrompe a busca (o cursor no corpo de um
//!   *closure* passado como argumento não mostra a chamada de fora);
//! * o rótulo é `nome(int a, [int b = 0], {required String c})`, cada
//!   parâmetro `tipo nome = padrão` com o tipo já substituído pelo receptor
//!   (`List<int>.add` mostra `int value`) e o padrão como escrito;
//! * o parâmetro ativo é o do argumento sob o cursor (nomeado pelo nome,
//!   posicional pela posição) ou, entre argumentos, o próximo posicional;
//! * a documentação é a do hover (inclusive a herdada de um membro
//!   sobrescrito);
//! * disparo automático por `(` só responde quando o `(` digitado é o que abre
//!   a lista.
//!
//! Tudo vem da inferência comum (`get_resolved`, `get_type`,
//! `MemberResolver::lookup_member`); nada é resolvido por nome aqui.

use crate::projeto::{Concreto, Projeto};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{FunctionElementId, FunctionKind, FunctionRef, UnitId};
use dartforge_frontend::ast::{self, ExprKind, MemberKind, ParameterKind};
use dartforge_intern::SymbolId;
use dartforge_types::{Type, TypeId};
use std::collections::HashMap;

/// Uma assinatura pronta para o protocolo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assinatura {
    /// `nome(int a, {int b = 0})`.
    pub rotulo: String,
    /// O rótulo de cada parâmetro, na ordem declarada.
    pub parametros: Vec<String>,
    pub documentacao: Option<String>,
    /// Índice em `parametros` do parâmetro ativo, quando há.
    pub ativo: Option<usize>,
}

/// Um parâmetro como a assinatura o mostra.
struct Parametro {
    rotulo: String,
    tipo: ParameterKind,
    /// Nome externo (o da chamada).
    nome: Option<String>,
}

impl Projeto {
    /// A ajuda de assinatura em `offset` de `unidade`. `automatica`: pedida
    /// pelo editor ao digitar `(` (sem a ajuda já aberta).
    pub(crate) fn assinatura(
        &mut self,
        unidade: UnitId,
        offset: usize,
        automatica: bool,
    ) -> Option<Assinatura> {
        let (chamada, argumentos) = self.lista_de_argumentos(unidade, offset)?;
        if automatica && offset != argumentos.start + 1 {
            return None;
        }
        let (nome, parametros, documentacao) = self.chamado(unidade, chamada)?;
        let ativo = self.parametro_ativo(unidade, chamada, offset, &parametros);
        let mut obrigatorios = Vec::new();
        let mut opcionais = Vec::new();
        let mut nomeados = Vec::new();
        for p in &parametros {
            match p.tipo {
                ParameterKind::Required => obrigatorios.push(p.rotulo.clone()),
                ParameterKind::Optional => opcionais.push(p.rotulo.clone()),
                ParameterKind::Named => nomeados.push(p.rotulo.clone()),
            }
        }
        let mut partes = Vec::new();
        if !obrigatorios.is_empty() {
            partes.push(obrigatorios.join(", "));
        }
        if !opcionais.is_empty() {
            partes.push(format!("[{}]", opcionais.join(", ")));
        }
        if !nomeados.is_empty() {
            partes.push(format!("{{{}}}", nomeados.join(", ")));
        }
        Some(Assinatura {
            rotulo: format!("{nome}({})", partes.join(", ")),
            parametros: parametros.into_iter().map(|p| p.rotulo).collect(),
            documentacao,
            ativo,
        })
    }

    /// A chamada (`Call` ou `InstanceCreation`) cuja lista de argumentos é a
    /// mais interna que contém `offset` (depois do `(` e até o `)`), e o
    /// intervalo da lista. `None` se uma expressão de função dentro da lista
    /// contém o cursor.
    fn lista_de_argumentos(&self, unidade: UnitId, offset: usize) -> Option<(ast::ExprId, Span)> {
        let u = self.programa().unit(unidade);
        let ast = &u.ast;
        let fonte = u.source.as_bytes();
        let mut melhor: Option<(ast::ExprId, Span)> = None;
        for (i, e) in ast.exprs.iter().enumerate() {
            let argumentos = match &e.kind {
                ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } => arguments,
                _ => continue,
            };
            let s = argumentos.span;
            let fechada = s.end > s.start && fonte.get(s.end - 1) == Some(&b')');
            let dentro = s.start < offset && if fechada { offset < s.end } else { offset <= s.end };
            if dentro && melhor.is_none_or(|(_, m)| s.end - s.start < m.end - m.start) {
                melhor = Some((ast::ExprId(i as u32), s));
            }
        }
        let (chamada, s) = melhor?;
        // `FunctionExpression` entre o cursor e a lista: o analyzer para.
        let barrada = ast
            .functions
            .iter()
            .any(|f| f.name.is_none() && s.start < f.span.start && f.span.end <= s.end && f.span.start <= offset && offset <= f.span.end);
        (!barrada).then_some((chamada, s))
    }

    /// Nome exibido, parâmetros e documentação do que a chamada invoca.
    fn chamado(&mut self, unidade: UnitId, chamada: ast::ExprId) -> Option<(String, Vec<Parametro>, Option<String>)> {
        /// O que a chamada é, copiado da árvore antes das consultas mutáveis.
        enum Forma {
            Criacao { tipo: Span, construtor: Option<ast::Name> },
            Chamada { alvo: ast::ExprId, alvo_span: Span, nome: Span, receptor: Option<ast::ExprId> },
        }
        let fonte = self.programa().unit(unidade).source.clone();
        let texto = |s: Span| limpar_nome(&fonte[s.start..s.end]);
        let forma = {
            let ast = &self.programa().unit(unidade).ast;
            match &ast.expr(chamada).kind {
                ExprKind::InstanceCreation { ty, constructor, .. } => Forma::Criacao { tipo: ast.ty(*ty).span, construtor: *constructor },
                ExprKind::Call { target, .. } => {
                    // `A<int>(…)` e `f<int>(…)`: o nome está dentro da
                    // instanciação explícita.
                    let alvo = match &ast.expr(*target).kind {
                        ExprKind::TypeArguments { target: t, .. } => ast.expr(*t),
                        _ => ast.expr(*target),
                    };
                    let alvo_span = ast.expr(*target).span;
                    let (nome, receptor) = match &alvo.kind {
                        ExprKind::Identifier(n) => (n.span, None),
                        ExprKind::Property { target: r, name, .. } => (name.span, Some(*r)),
                        _ => return None,
                    };
                    Forma::Chamada { alvo: *target, alvo_span, nome, receptor }
                }
                _ => return None,
            }
        };
        match forma {
            Forma::Criacao { tipo, construtor } => {
                let mut nome = texto(tipo);
                if let Some(c) = construtor {
                    nome = format!("{nome}.{}", &fonte[c.span.start..c.span.end]);
                }
                let f = match self.consulta.corpos.units[unidade.0 as usize].get_resolved(chamada) {
                    Some(dartforge_types::Resolved::Constructor(f)) => *f,
                    _ => return None,
                };
                let parametros = self.parametros_do_construtor(unidade, chamada, f);
                let documentacao = self.documentacao_de(unidade, construtor.map_or(tipo, |c| c.span));
                Some((nome, parametros, documentacao))
            }
            Forma::Chamada { alvo, alvo_span, nome: nome_span, receptor } => {
                let d = self.identificar(unidade, nome_span.start).ok()??;
                let documentacao = self.hover(unidade, &d).and_then(|h| h.documentacao);
                match d.concreto {
                    Some(Concreto::Funcao(f)) => match { self.programa().function(f).kind } {
                        FunctionKind::Constructor | FunctionKind::SyntheticConstructor => {
                            let parametros = self.parametros_do_construtor(unidade, chamada, f);
                            Some((texto(alvo_span), parametros, documentacao))
                        }
                        // Getter ou campo que devolve função: o analyzer
                        // não tem lista de parâmetros para mostrar.
                        FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor => None,
                        _ => {
                            let nome = fonte[nome_span.start..nome_span.end].to_string();
                            let tipo = self.tipo_do_membro(unidade, receptor, alvo, f);
                            Some((nome, self.parametros_da_funcao(f, tipo), documentacao))
                        }
                    },
                    Some(Concreto::Variavel(_)) => None,
                    None => {
                        // Função local: a árvore dá os nomes, o tipo do local
                        // dá os tipos.
                        let crate::projeto::Alvo::Local { unidade: ul, declaracao } = d.alvo else {
                            return None;
                        };
                        let nome = fonte[nome_span.start..nome_span.end].to_string();
                        let tipo = self.consulta.corpos.units[ul.0 as usize].tipo_local(declaracao)?;
                        let ast_local = &self.programa().unit(ul).ast;
                        let parametros = match ast_local.functions.iter().find(|f| f.name.is_some_and(|n| n.span.start == declaracao)) {
                            Some(f) => {
                                let ps = f.parameters.as_deref().unwrap_or(&[]);
                                let tipos = tipos_alinhados(&self.consulta.tabela, ps, tipo);
                                self.rotulos(ul, ps, &tipos)
                            }
                            // Variável local de tipo função: só os tipos.
                            None => self.parametros_do_tipo(tipo),
                        };
                        Some((nome, parametros, documentacao))
                    }
                }
            }
        }
    }

    /// A documentação do hover no nome em `nome`.
    fn documentacao_de(&self, unidade: UnitId, nome: Span) -> Option<String> {
        let d = self.identificar(unidade, nome.start).ok()??;
        self.hover(unidade, &d).and_then(|h| h.documentacao)
    }

    /// O tipo de função do membro chamado, já substituído pelo receptor:
    /// pela busca de membros da inferência comum quando há receptor, senão
    /// o tipo estático do alvo da chamada.
    fn tipo_do_membro(
        &mut self,
        unidade: UnitId,
        receptor: Option<ast::ExprId>,
        alvo: ast::ExprId,
        f: FunctionElementId,
    ) -> Option<TypeId> {
        let fe = self.programa().function(f);
        if fe.static_ || (fe.class.is_none() && fe.extension.is_none()) {
            return None;
        }
        let simbolo: SymbolId = fe.name;
        let lib = self.programa().unit(unidade).library;
        let tipo_r = receptor.and_then(|r| self.consulta.corpos.units[unidade.0 as usize].get_type(r));
        if let Some(tipo_r) = tipo_r
            && let Some((_, t)) = self.consulta.resolvedor().lookup_member(tipo_r, simbolo, false, lib)
            && matches!(self.consulta.tabela.get(t), Type::Function { .. })
        {
            return Some(t);
        }
        self.consulta.corpos.units[unidade.0 as usize]
            .get_type(alvo)
            .filter(|t| matches!(self.consulta.tabela.get(*t), Type::Function { .. }))
    }

    /// Parâmetros de uma função, método ou operador; `tipo` (substituído)
    /// vale mais que os tipos declarados quando tem a mesma forma.
    fn parametros_da_funcao(&self, f: FunctionElementId, tipo: Option<TypeId>) -> Vec<Parametro> {
        let p = self.programa();
        let FunctionRef::Function { unit, function } = p.function(f).node else {
            return Vec::new();
        };
        let ps = p.unit(unit).ast.function(function).parameters.as_deref().unwrap_or(&[]);
        let declarados: Vec<Option<TypeId>> = self.consulta.outline.functions[f.0 as usize].parameters.iter().map(|q| Some(q.ty)).collect();
        let tipos = match tipo {
            Some(t) => {
                let alinhados = tipos_alinhados(&self.consulta.tabela, ps, t);
                if alinhados.iter().all(Option::is_some) { alinhados } else { declarados }
            }
            None => declarados,
        };
        self.rotulos(unit, ps, &tipos)
    }

    /// Parâmetros de um construtor com os parâmetros de tipo da classe
    /// trocados pelos argumentos do tipo criado (`List<int>.filled` mostra
    /// `int fill`).
    fn parametros_do_construtor(&mut self, unidade: UnitId, chamada: ast::ExprId, f: FunctionElementId) -> Vec<Parametro> {
        let p = self.programa();
        let FunctionRef::Constructor { unit, member } = p.function(f).node else {
            return Vec::new();
        };
        let MemberKind::Constructor(k) = &p.unit(unit).ast.member(member).kind else {
            return Vec::new();
        };
        let n = k.parameters.len();
        let declarados: Vec<TypeId> = self.consulta.outline.functions[f.0 as usize].parameters.iter().map(|q| q.ty).collect();
        let mut mapa = HashMap::new();
        if let Some(c) = p.function(f).class
            && let Some(t) = self.consulta.corpos.units[unidade.0 as usize].get_type(chamada)
            && let Type::Interface { class, args, .. } = self.consulta.tabela.get(t).clone()
            && class == c
            && let Some(dados) = self.consulta.outline.classes.get(c.0 as usize)
            && dados.type_params.len() == args.len()
        {
            for (tp, a) in dados.type_params.iter().zip(args.iter()) {
                mapa.insert(*tp, *a);
            }
        }
        let tipos: Vec<Option<TypeId>> = declarados
            .into_iter()
            .map(|t| Some(dartforge_types::ops::substitute(t, &mapa, &mut self.consulta.tabela)))
            .chain(std::iter::repeat(None))
            .take(n)
            .collect();
        let p = self.programa();
        let MemberKind::Constructor(k) = &p.unit(unit).ast.member(member).kind else {
            return Vec::new();
        };
        self.rotulos(unit, &k.parameters, &tipos)
    }

    /// Parâmetros só pelo tipo de função (sem nomes posicionais).
    fn parametros_do_tipo(&self, tipo: TypeId) -> Vec<Parametro> {
        let Type::Function { positional, optional, named, .. } = self.consulta.tabela.get(tipo).clone() else {
            return Vec::new();
        };
        let fmt = |t: TypeId| self.consulta.formatar(t);
        let mut saida: Vec<Parametro> = positional
            .iter()
            .map(|t| Parametro { rotulo: format!("{} ", fmt(*t)), tipo: ParameterKind::Required, nome: None })
            .chain(optional.iter().map(|t| Parametro { rotulo: format!("{} ", fmt(*t)), tipo: ParameterKind::Optional, nome: None }))
            .collect();
        for (n, t, obrigatorio) in named.iter() {
            let nome = self.nome(*n).to_string();
            let prefixo = if *obrigatorio { "required " } else { "" };
            saida.push(Parametro { rotulo: format!("{prefixo}{} {nome}", fmt(*t)), tipo: ParameterKind::Named, nome: Some(nome) });
        }
        saida
    }

    /// `required int c = 0` de cada parâmetro escrito em `u`.
    fn rotulos(&self, u: UnitId, ps: &[ast::Parameter], tipos: &[Option<TypeId>]) -> Vec<Parametro> {
        let unidade = self.programa().unit(u);
        let fonte = unidade.source.as_str();
        ps.iter()
            .enumerate()
            .map(|(i, p)| {
                let tipo = tipos
                    .get(i)
                    .copied()
                    .flatten()
                    .map_or_else(|| "dynamic".to_string(), |t| self.consulta.formatar(t));
                let nome = p.public_name.or(p.name).map(|n| fonte[n.span.start..n.span.end].to_string());
                let padrao = p.default_value.map_or(String::new(), |e| {
                    let s = unidade.ast.expr(e).span;
                    format!(" = {}", &fonte[s.start..s.end])
                });
                let prefixo = if p.kind == ParameterKind::Named && p.required { "required " } else { "" };
                Parametro {
                    rotulo: format!("{prefixo}{tipo} {}{padrao}", nome.as_deref().unwrap_or("")),
                    tipo: p.kind,
                    nome,
                }
            })
            .collect()
    }

    /// O parâmetro ativo: o do argumento sob o cursor, ou o próximo
    /// posicional entre argumentos.
    fn parametro_ativo(&self, unidade: UnitId, chamada: ast::ExprId, offset: usize, parametros: &[Parametro]) -> Option<usize> {
        let ast = &self.programa().unit(unidade).ast;
        let argumentos = match &ast.expr(chamada).kind {
            ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } => arguments,
            _ => return None,
        };
        let faixa = |a: &ast::Argument| {
            let v = ast.expr(a.value).span;
            (a.name.map_or(v.start, |n| n.span.start), v.end)
        };
        let posicional_n = |k: usize| {
            parametros
                .iter()
                .enumerate()
                .filter(|(_, p)| p.tipo != ParameterKind::Named)
                .nth(k)
                .map(|(i, _)| i)
        };
        let mut posicionais_antes = 0;
        for a in argumentos.args.iter() {
            let (de, ate) = faixa(a);
            if de <= offset && offset <= ate {
                return match a.name {
                    Some(n) => {
                        let nome = &self.programa().unit(unidade).source[n.span.start..n.span.end];
                        parametros.iter().position(|p| p.tipo == ParameterKind::Named && p.nome.as_deref() == Some(nome))
                    }
                    None => posicional_n(posicionais_antes),
                };
            }
            if a.name.is_none() {
                posicionais_antes += 1;
            }
        }
        let antes = argumentos.args.iter().filter(|a| a.name.is_none() && faixa(a).1 < offset).count();
        posicional_n(antes)
    }
}

/// Os tipos de `tipo` (uma função) alinhados aos parâmetros escritos:
/// posicionais pela ordem, nomeados pelo nome.
fn tipos_alinhados(tabela: &dartforge_types::TypeTable, ps: &[ast::Parameter], tipo: TypeId) -> Vec<Option<TypeId>> {
    let Type::Function { positional, optional, named, .. } = tabela.get(tipo).clone() else {
        return vec![None; ps.len()];
    };
    let mut posicionais = positional.iter().chain(optional.iter());
    ps.iter()
        .map(|p| match p.kind {
            ParameterKind::Named => {
                let simbolo = p.public_name.or(p.name)?.sym;
                named.iter().find(|(n, _, _)| *n == simbolo).map(|(_, t, _)| *t)
            }
            _ => posicionais.next().copied(),
        })
        .collect()
}

/// O nome qualificado como o analyzer mostra: sem argumentos de tipo, `?`
/// nem espaços (`p.A<int>.nome` vira `p.A.nome`).
fn limpar_nome(texto: &str) -> String {
    let mut saida = String::new();
    let mut nivel = 0usize;
    for c in texto.chars() {
        match c {
            '<' => nivel += 1,
            '>' => nivel = nivel.saturating_sub(1),
            _ if nivel > 0 => {}
            c if c.is_whitespace() || c == '?' => {}
            c => saida.push(c),
        }
    }
    saida
}

#[cfg(test)]
mod testes {
    use super::limpar_nome;

    #[test]
    fn nome_qualificado_sem_argumentos_de_tipo() {
        assert_eq!(limpar_nome("p.A<int, List<String>>.nome"), "p.A.nome");
        assert_eq!(limpar_nome("Mapa<K,\n V>"), "Mapa");
    }
}
