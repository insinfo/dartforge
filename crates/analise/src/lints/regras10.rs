//! O décimo lote de regras de lint que só olham a árvore e o texto
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//! `prefer_inlined_adds`, `prefer_spread_collections`,
//! `avoid_field_initializers_in_const_classes`,
//! `avoid_classes_with_only_static_members` e `prefer_final_in_for_each`.
//!
//! Diferenças conhecidas:
//! - `avoid_classes_with_only_static_members` só olha a classe sem
//!   `extends`, `with` nem `implements`: com supertipo, o original pergunta
//!   à interface herdada se há membro de instância, e aqui nada se relata.
//! - `prefer_final_in_for_each` procura a mutação da variável pelo nome
//!   (atribuição, `++`/`--`, padrão de atribuição, `for (x in …)`) no corpo
//!   do laço; no `for` de coleção, do fim do iterável ao fim do literal.
//! - `avoid_field_initializers_in_const_classes` decide "usa um parâmetro"
//!   pelo nome.
//! - `prefer_spread_collections` não tem a isenção de contexto constante
//!   (uma cascata nunca é constante).
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::regras9::variaveis_do_padrao;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, Ast, CollectionElement, DeclKind, ExprKind, ForInTarget, FunctionKind, Initializer, ListPatternElement, MemberKind, ParameterKind,
    PatternId, PatternKind, StmtKind, UnaryOp,
};
use dartforge_intern::{Interner, SymbolId};

/// Os `for-in` de coleção com variável declarada ou padrão: o alvo e o fim
/// do iterável (de onde começa a região em que a variável vive).
fn lacos_de_colecao<'x>(a: &Ast, el: &'x CollectionElement, saida: &mut Vec<(&'x ForInTarget, usize)>) {
    match el {
        CollectionElement::ForIn { target, iterable, body, .. } => {
            saida.push((target, a.expr(*iterable).span.end));
            lacos_de_colecao(a, body, saida);
        }
        CollectionElement::For { body, .. } => lacos_de_colecao(a, body, saida),
        CollectionElement::If { then, else_, .. } => {
            lacos_de_colecao(a, then, saida);
            if let Some(x) = else_ {
                lacos_de_colecao(a, x, saida);
            }
        }
        _ => {}
    }
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let a = u.ast;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };

    // `prefer_inlined_adds` e `prefer_spread_collections`: a primeira seção
    // de uma cascata sobre um literal de lista.
    let (inlinar, espalhar) = (ligada("prefer_inlined_adds"), ligada("prefer_spread_collections"));
    if inlinar || espalhar {
        for e in a.exprs.iter() {
            let ExprKind::Cascade { target, sections, .. } = &e.kind else { continue };
            if !matches!(a.expr(*target).kind, ExprKind::List { .. }) {
                continue;
            }
            let Some(&primeira) = sections.first() else { continue };
            let ExprKind::Call { target: chamado, arguments } = &a.expr(primeira).kind else { continue };
            let ExprKind::Property { target: receptor, name, .. } = &a.expr(*chamado).kind else { continue };
            if !matches!(a.expr(*receptor).kind, ExprKind::CascadeTarget) {
                continue;
            }
            let [argumento] = &arguments.args[..] else { continue };
            let de_lista = matches!(a.expr(argumento.value).kind, ExprKind::List { .. });
            match interner.resolve(name.sym) {
                "add" if inlinar => relatar(&c::PREFER_INLINED_ADDS_SINGLE, name.span, &[]),
                "addAll" if de_lista && inlinar => relatar(&c::PREFER_INLINED_ADDS_MULTIPLE, name.span, &[]),
                "addAll" if !de_lista && espalhar => relatar(&c::PREFER_SPREAD_COLLECTIONS, name.span, &[]),
                _ => {}
            }
        }
    }

    // Os identificadores da unidade, pela posição.
    let mut usos: Vec<(usize, SymbolId)> = a
        .exprs
        .iter()
        .filter_map(|e| match &e.kind {
            ExprKind::Identifier(n) => Some((e.span.start, n.sym)),
            _ => None,
        })
        .collect();
    usos.sort_by_key(|x| x.0);
    let usado_em = |regiao: Span, s: SymbolId| {
        let (de, ate) = (usos.partition_point(|x| x.0 < regiao.start), usos.partition_point(|x| x.0 < regiao.end));
        usos[de..ate.max(de)].iter().any(|x| x.1 == s)
    };

    // `avoid_field_initializers_in_const_classes`.
    if ligada("avoid_field_initializers_in_const_classes") {
        for d in a.decls.iter() {
            let DeclKind::Class(x) = &d.kind else { continue };
            let construtores: Vec<&ast::Constructor> = x
                .members
                .iter()
                .filter_map(|&m| match &a.member(m).kind {
                    MemberKind::Constructor(k) => Some(k),
                    _ => None,
                })
                .collect();
            // O campo `final` de instância com inicializador, numa classe
            // com algum construtor `const`.
            if construtores.iter().any(|k| k.const_) {
                for &m in x.members.iter() {
                    let membro = a.member(m);
                    if let MemberKind::Field(l) = &membro.kind
                        && !membro.augment
                        && !l.static_
                        && l.final_
                    {
                        for v in l.variables.iter() {
                            if let Some(i) = v.initializer {
                                relatar(&c::AVOID_FIELD_INITIALIZERS_IN_CONST_CLASSES, Span { start: v.name.span.start, end: a.expr(i).span.end }, &[]);
                            }
                        }
                    }
                }
            }
            // O inicializador de campo do único construtor, `const`, que não
            // usa parâmetro nenhum.
            if let [k] = &construtores[..]
                && k.const_
            {
                for i in k.initializers.iter() {
                    if let Initializer::Field { span, value, .. } = i {
                        let regiao = a.expr(*value).span;
                        let usa = k.parameters.iter().filter_map(|p| p.name).any(|n| usado_em(regiao, n.sym));
                        if !usa {
                            relatar(&c::AVOID_FIELD_INITIALIZERS_IN_CONST_CLASSES, *span, &[]);
                        }
                    }
                }
            }
        }
    }
    // `avoid_classes_with_only_static_members`.
    if ligada("avoid_classes_with_only_static_members") {
        for d in a.decls.iter().filter(|d| !d.augment) {
            let DeclKind::Class(x) = &d.kind else { continue };
            if x.modifiers.sealed || x.mixin_application || x.extends.is_some() || !x.with.is_empty() || !x.implements.is_empty() {
                continue;
            }
            let mut de_instancia = false;
            let mut construtor_proprio = false;
            let mut metodos_estaticos = false;
            let mut campo_nao_const = false;
            for &m in x.members.iter() {
                match &a.member(m).kind {
                    MemberKind::Field(l) => {
                        de_instancia |= !l.static_;
                        campo_nao_const |= !l.const_;
                    }
                    MemberKind::Method(f) => {
                        let f = a.function(*f);
                        de_instancia |= !f.static_;
                        match f.kind {
                            // O acessor estático vira campo sintético, não `const`.
                            FunctionKind::Getter | FunctionKind::Setter => campo_nao_const = true,
                            FunctionKind::Function | FunctionKind::Operator => metodos_estaticos = true,
                        }
                    }
                    // `isDefaultConstructor`: sem nome e sem parâmetro obrigatório.
                    MemberKind::Constructor(k) => {
                        construtor_proprio |= k.name.is_some() || k.parameters.iter().any(|p| p.kind == ParameterKind::Required || p.required);
                    }
                }
            }
            if !de_instancia && !construtor_proprio && (metodos_estaticos || campo_nao_const) {
                relatar(&c::AVOID_CLASSES_WITH_ONLY_STATIC_MEMBERS, d.span, &[]);
            }
        }
    }
    // `prefer_final_in_for_each`.
    if ligada("prefer_final_in_for_each") {
        // As mutações da unidade, pela posição: atribuição, `++`/`--`,
        // padrão de atribuição e `for (x in …)`.
        let mut mutacoes: Vec<(usize, SymbolId)> = Vec::new();
        let nome_de = |e: ast::ExprId| match &a.expr(e).kind {
            ExprKind::Identifier(n) => Some(n.sym),
            _ => None,
        };
        for e in a.exprs.iter() {
            match &e.kind {
                ExprKind::Assign { target, .. } => mutacoes.extend(nome_de(*target).map(|s| (e.span.start, s))),
                ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } => {
                    mutacoes.extend(nome_de(*operand).map(|s| (e.span.start, s)));
                }
                ExprKind::PatternAssign { pattern, .. } => {
                    let mut variaveis: Vec<PatternId> = Vec::new();
                    variaveis_do_padrao(a, *pattern, &mut variaveis);
                    for v in variaveis {
                        if let PatternKind::Variable { name, .. } = &a.pattern(v).kind {
                            mutacoes.push((e.span.start, name.sym));
                        }
                    }
                }
                _ => {}
            }
        }
        for s in a.stmts.iter() {
            if let StmtKind::ForIn { target: ForInTarget::Expression(x), .. } = &s.kind {
                mutacoes.extend(nome_de(*x).map(|n| (s.span.start, n)));
            }
        }
        mutacoes.sort_by_key(|x| x.0);
        let mutado = |regiao: Span, s: SymbolId| {
            let (de, ate) = (mutacoes.partition_point(|x| x.0 < regiao.start), mutacoes.partition_point(|x| x.0 < regiao.end));
            mutacoes[de..ate.max(de)].iter().any(|x| x.1 == s)
        };
        // `potentiallyMutates`: só a variável declarada, não mutada, passa.
        let quieto = |p: PatternId, regiao: Span| matches!(&a.pattern(p).kind, PatternKind::Variable { name, .. } if !mutado(regiao, name.sym));
        let mut conferir = |alvo: &ForInTarget, regiao: Span| match alvo {
            ForInTarget::Declared { final_: false, name, .. } => {
                if !mutado(regiao, name.sym) {
                    relatar(&c::PREFER_FINAL_IN_FOR_EACH_VARIABLE, name.span, &[interner.resolve(name.sym)]);
                }
            }
            ForInTarget::Pattern { final_: false, pattern } => {
                let n = a.pattern(*pattern);
                let relata = match &n.kind {
                    PatternKind::Record { fields } | PatternKind::Object { fields, .. } => fields.iter().all(|f| quieto(f.pattern, regiao)),
                    PatternKind::List { elements, .. } => elements.iter().all(|el| matches!(el, ListPatternElement::Pattern(x) if quieto(*x, regiao))),
                    PatternKind::Map { entries, rest, .. } => !*rest && entries.iter().all(|en| quieto(en.value, regiao)),
                    _ => false,
                };
                if relata {
                    relatar(&c::PREFER_FINAL_IN_FOR_EACH_PATTERN, n.span, &[]);
                }
            }
            _ => {}
        };
        for s in a.stmts.iter() {
            if let StmtKind::ForIn { target, body, .. } = &s.kind {
                conferir(target, a.stmt(*body).span);
            }
        }
        for e in a.exprs.iter() {
            if let ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } = &e.kind {
                let mut lacos = Vec::new();
                for el in elements.iter() {
                    lacos_de_colecao(a, el, &mut lacos);
                }
                for (alvo, de) in lacos {
                    conferir(alvo, Span { start: de, end: e.span.end });
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;

    fn com_codigo(regra: &str, fonte: &str) -> Vec<(&'static str, String)> {
        let mut nomes = Interner::new();
        let p = dartforge_frontend::parser::parse(fonte, &mut nomes);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte };
        let mut relatos = executar(u, &nomes, &|r| r == regra);
        relatos.sort_by_key(|r| (r.span.start, r.span.end));
        relatos.into_iter().map(|r| (r.codigo.unico, fonte[r.span.start..r.span.end].to_string())).collect()
    }

    #[test]
    fn cascatas_em_listas() {
        let fonte = "var a = [1]..add(2);\nvar b = [1]..addAll([2, 3]);\nvar c = [1]..addAll(a);\nvar d = [1]..length..add(2);\nvar e = a..add(2);\n";
        assert_eq!(
            com_codigo("prefer_inlined_adds", fonte),
            vec![("prefer_inlined_adds_single", "add".to_string()), ("prefer_inlined_adds_multiple", "addAll".to_string())]
        );
        assert_eq!(com_codigo("prefer_spread_collections", fonte), vec![("prefer_spread_collections", "addAll".to_string())]);
    }

    #[test]
    fn classes() {
        let fonte = "class A {\n  final int a = 1;\n  final int b;\n  static final int c = 2;\n  const A(int x) : b = 0;\n}\nclass B {\n  final int d;\n  const B(int x) : d = x;\n}\n";
        let v: Vec<String> = com_codigo("avoid_field_initializers_in_const_classes", fonte).into_iter().map(|x| x.1).collect();
        assert_eq!(v, vec!["a = 1".to_string(), "b = 0".to_string()]);
        let fonte = "class A {\n  static int f() => 0;\n}\nclass B {\n  static const int k = 1;\n}\nclass C {\n  static int v = 1;\n  int m() => 0;\n}\nclass D {\n  static int v = 1;\n  D(int x);\n}\nclass E {\n  static int v = 1;\n}\n";
        let v = com_codigo("avoid_classes_with_only_static_members", fonte);
        assert_eq!(v.len(), 2, "{v:?}");
        assert!(v[0].1.starts_with("class A") && v[1].1.starts_with("class E"));
    }

    #[test]
    fn for_each_final() {
        let fonte = "void f(List<int> l, List<(int, int)> p) {\n  for (var a in l) {}\n  for (var b in l) {\n    b++;\n  }\n  for (final c in l) {}\n  for (var (x, y) in p) {}\n  for (var (z, w) in p) {\n    z = w;\n  }\n  var q = [for (var d in l) d];\n}\n";
        assert_eq!(
            com_codigo("prefer_final_in_for_each", fonte),
            vec![
                ("prefer_final_in_for_each_variable", "a".to_string()),
                ("prefer_final_in_for_each_pattern", "(x, y)".to_string()),
                ("prefer_final_in_for_each_variable", "d".to_string()),
            ]
        );
    }
}
