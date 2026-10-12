//! O décimo terceiro lote de regras de lint que só olham a árvore
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//! `avoid_shadowing_type_parameters` e `parameter_assignments`.
//!
//! `parameter_assignments` liga o identificador e a variável do padrão ao
//! parâmetro pelo elemento, e o "potencialmente mutado" é o do analyzer
//! (`super::mutado`); pede a semântica da unidade. A isenção do `_` curinga
//! de `avoid_shadowing_type_parameters` depende do recurso da 3.7, que uma
//! biblioteca da 3.6 não liga.
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    Ast, DeclKind, ExprKind, FunctionBody, FunctionId, ListPatternElement, MemberKind, ParameterKind, PatternId, PatternKind, StmtKind,
    TypeKind, TypeParameter, TypedefKind,
};
use dartforge_intern::Interner;

fn dentro(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

/// A região do corpo (`None` no corpo `;`).
fn regiao_do_corpo(a: &Ast, b: &FunctionBody) -> Option<Span> {
    match b {
        FunctionBody::Block(s) => Some(a.stmt(*s).span),
        FunctionBody::Expression(e) => Some(a.expr(*e).span),
        _ => None,
    }
}

/// Os contêineres de parâmetros de tipo que envolvem `span`, de dentro para
/// fora: as declarações de tipo, as funções declaradas e os métodos.
fn ancestrais_de<'x>(
    a: &'x Ast,
    conteineres: &[(Span, &'x [TypeParameter], &'static str)],
    declaradas: &[FunctionId],
    metodos: &[(FunctionId, bool)],
    span: Span,
) -> Vec<(Span, &'x [TypeParameter], &'static str)> {
    let mut v: Vec<(Span, &'x [TypeParameter], &'static str)> = Vec::new();
    for &(r, tps, tipo) in conteineres {
        if dentro(span, r) && r != span {
            v.push((r, tps, tipo));
        }
    }
    for &f in declaradas {
        let r = a.function(f).span;
        if dentro(span, r) && r != span {
            v.push((r, &a.function(f).type_params[..], "function"));
        }
    }
    for &(f, _) in metodos {
        let r = a.function(f).span;
        if dentro(span, r) && r != span {
            v.push((r, &a.function(f).type_params[..], "method"));
        }
    }
    // De dentro para fora: o menor primeiro.
    v.sort_by_key(|x| x.0.end - x.0.start);
    v
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let a = u.ast;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };

    // `avoid_shadowing_type_parameters`.
    if ligada("avoid_shadowing_type_parameters") {
        let mut achados: Vec<(Span, String, &'static str)> = Vec::new();
        let sombra = |tps: &[TypeParameter], de_fora: &[TypeParameter], tipo: &'static str, achados: &mut Vec<(Span, String, &'static str)>| {
            for p in tps {
                if de_fora.iter().any(|q| q.name.sym == p.name.sym) {
                    achados.push((p.span, interner.resolve(p.name.sym).to_string(), tipo));
                }
            }
        };
        // Os contêineres de tipo da unidade, com a região e a espécie.
        let mut conteineres: Vec<(Span, &[TypeParameter], &'static str)> = Vec::new();
        for d in a.decls.iter() {
            match &d.kind {
                DeclKind::Class(x) => conteineres.push((d.span, &x.type_params[..], "class")),
                DeclKind::Enum(x) => conteineres.push((d.span, &x.type_params[..], "enum")),
                DeclKind::Extension(x) => conteineres.push((d.span, &x.type_params[..], "extension")),
                DeclKind::ExtensionType(x) => conteineres.push((d.span, &x.type_params[..], "extension type")),
                DeclKind::Mixin(x) => conteineres.push((d.span, &x.type_params[..], "mixin")),
                _ => {}
            }
        }
        // As funções declaradas (de topo e locais) e os métodos.
        let mut declaradas: Vec<FunctionId> = Vec::new();
        let mut metodos: Vec<(FunctionId, bool)> = Vec::new();
        for d in a.decls.iter() {
            if let DeclKind::Function(f) = &d.kind {
                declaradas.push(*f);
            }
        }
        for s in a.stmts.iter() {
            if let StmtKind::Function(f) = &s.kind {
                declaradas.push(*f);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Method(f) = &m.kind {
                metodos.push((*f, a.function(*f).static_));
            }
        }
        let ancestrais = |span: Span| ancestrais_de(a, &conteineres, &declaradas, &metodos, span);
        // A função local genérica, contra todos os ancestrais.
        for s in a.stmts.iter() {
            let StmtKind::Function(f) = &s.kind else { continue };
            let f = a.function(*f);
            if f.type_params.is_empty() {
                continue;
            }
            for (_, de_fora, tipo) in ancestrais(f.span) {
                sombra(&f.type_params, de_fora, tipo, &mut achados);
            }
        }
        // O método genérico de instância.
        for &(fid, estatico) in &metodos {
            let f = a.function(fid);
            if f.type_params.is_empty() || estatico {
                continue;
            }
            for (_, de_fora, tipo) in ancestrais(f.span) {
                sombra(&f.type_params, de_fora, tipo, &mut achados);
            }
        }
        // O typedef com tipo de função genérico.
        for d in a.decls.iter() {
            if let DeclKind::Typedef(x) = &d.kind
                && let TypedefKind::Alias(t) = &x.kind
                && let TypeKind::Function { type_params, .. } = &a.ty(*t).kind
            {
                sombra(type_params, &x.type_params, "typedef", &mut achados);
            }
        }
        for (s, nome, tipo) in achados {
            relatar(&c::AVOID_SHADOWING_TYPE_PARAMETERS, s, &[&nome, tipo]);
        }
    }

    // `parameter_assignments`: nas funções declaradas (de topo e locais) e
    // nos métodos, o parâmetro que alguma escrita muda (pelo elemento) e que
    // é o comum obrigatório posicional ou tem valor padrão (nunca nulo de
    // início), ou não tem valor padrão (o opcional e o nomeado, também o
    // `required`: nulo de início, e a primeira atribuição passa). Na ordem
    // do texto: a atribuição ao identificador, o pós-fixo e o prefixo sobre
    // ele (só o "nunca nulo"), e o padrão de atribuição com ele num campo ou
    // elemento direto. Pede a semântica da unidade.
    if ligada("parameter_assignments")
        && let Some(sem) = sem
    {
        let mut alvos: Vec<FunctionId> = Vec::new();
        for d in a.decls.iter() {
            if let DeclKind::Function(f) = &d.kind {
                alvos.push(*f);
            }
        }
        for s in a.stmts.iter() {
            if let StmtKind::Function(f) = &s.kind {
                alvos.push(*f);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Method(f) = &m.kind {
                alvos.push(*f);
            }
        }
        let mut achados: Vec<(Span, String)> = Vec::new();
        for fid in alvos {
            let f = a.function(fid);
            let (Some(ps), Some(corpo)) = (&f.parameters, regiao_do_corpo(a, &f.body)) else { continue };
            for p in ps.iter() {
                let Some(n) = p.name else { continue };
                let decl = n.span.start;
                if !super::mutado(sem, a, decl) {
                    continue;
                }
                // `SimpleFormalParameter` sem `DefaultFormalParameter` (o
                // obrigatório posicional comum), ou com valor padrão: nunca
                // nulo de início; `DefaultFormalParameter` sem valor padrão:
                // nulo de início.
                let comum = p.kind == ParameterKind::Required && p.function_parameters.is_none() && !p.this_ && !p.super_;
                let nao_nulo = comum || (p.kind != ParameterKind::Required && p.default_value.is_some());
                let comeca_nulo = p.kind != ParameterKind::Required && p.default_value.is_none();
                if !nao_nulo && !comeca_nulo {
                    continue;
                }
                let o_parametro = |e: dartforge_frontend::ast::ExprId| {
                    matches!(a.expr(e).kind, ExprKind::Identifier(_)) && sem.corpo.declaracao_local(e) == Some(decl)
                };
                let padrao_e_o_parametro = |q: PatternId| sem.corpo.declaracoes_de_padroes.get(&q) == Some(&decl);
                let nome = interner.resolve(n.sym).to_string();
                let mut ja_atribuido = false;
                // Em ordem de fonte (o `RecursiveAstVisitor`).
                let mut no_corpo: Vec<(usize, &dartforge_frontend::ast::Expr)> =
                    a.exprs.iter().enumerate().filter(|(_, e)| dentro(e.span, corpo)).collect();
                no_corpo.sort_by_key(|(_, e)| (e.span.start, std::cmp::Reverse(e.span.end)));
                for (_, e) in no_corpo {
                    match &e.kind {
                        ExprKind::Assign { target, .. } if o_parametro(*target) => {
                            if nao_nulo {
                                achados.push((e.span, nome.clone()));
                            } else if comeca_nulo {
                                if ja_atribuido {
                                    achados.push((e.span, nome.clone()));
                                }
                                ja_atribuido = true;
                            }
                        }
                        // Todo pós-fixo e prefixo sobre o parâmetro (`p++`,
                        // `p!`, `-p`, `!p`).
                        ExprKind::Unary { operand, .. } if nao_nulo && o_parametro(*operand) => achados.push((e.span, nome.clone())),
                        ExprKind::PatternAssign { pattern, .. } => {
                            let relata = match &a.pattern(*pattern).kind {
                                PatternKind::Record { fields } | PatternKind::Object { fields, .. } => fields.iter().any(|f| padrao_e_o_parametro(f.pattern)),
                                PatternKind::List { elements, .. } => elements.iter().any(|el| match el {
                                    ListPatternElement::Pattern(q) => padrao_e_o_parametro(*q),
                                    // O `RestPatternElement` não é a variável.
                                    ListPatternElement::Rest(_) => false,
                                }),
                                PatternKind::Map { entries, .. } => entries.iter().any(|en| padrao_e_o_parametro(en.value)),
                                _ => false,
                            };
                            if relata {
                                achados.push((a.pattern(*pattern).span, nome.clone()));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        achados.sort_by_key(|x| (x.0.start, x.0.end));
        for (s, nome) in achados {
            relatar(&c::PARAMETER_ASSIGNMENTS, s, &[&nome]);
        }
    }

    out
}
