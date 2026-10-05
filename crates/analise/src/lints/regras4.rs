//! O quarto lote de regras de lint que só olham a árvore e o texto
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8): `prefer_if_null_operators`,
//! `avoid_single_cascade_in_expression_statements`,
//! `use_function_type_syntax_for_parameters`,
//! `no_leading_underscores_for_local_identifiers` e
//! `prefer_function_declarations_over_variables`.
//!
//! Escritas com os emissores do `main` do SDK abertos e depois conferidas
//! contra os da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter`, extraído
//! da tag em 2026-10-05). Na 3.6.2,
//! `use_function_type_syntax_for_parameters` só olha o parâmetro comum
//! (`FunctionTypedFormalParameter`), não o `this.f(…)` nem o `super.f(…)`.
//! `no_leading_underscores_for_local_identifiers` olha também o `for`/`for-in`
//! de coleção, as variáveis de padrão de declaração e as listas de
//! parâmetros dos tipos `Function(…)` e dos typedefs antigos;
//! `prefer_function_declarations_over_variables` relata fora de corpo de
//! função só a lista `final` (o `isFinal`, sem `const`) e, dentro, o local
//! que nenhuma escrita na unidade muda (`isPotentiallyMutatedInScope`, pelo
//! elemento: pede a semântica da unidade); `prefer_if_null_operators`
//! compara o `toString` dos nós (`dartforge_frontend::fonte`).
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, BinaryOp, DeclKind, ExprId, ExprKind, ForInTarget, ForInit, MemberKind, ParameterKind, StmtKind, VariableList,
};
use dartforge_intern::Interner;

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let a = u.ast;
    let fonte = u.fonte;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
    // O `toString` de uma expressão.
    let fonte_de = |e: ExprId| -> String { dartforge_frontend::fonte::de_expr(a, fonte, interner, e) };
    // Todas as listas de parâmetros da unidade, com as aninhadas.
    fn listas<'x>(lista: &'x [ast::Parameter], saida: &mut Vec<&'x [ast::Parameter]>) {
        saida.push(lista);
        for p in lista {
            if let Some(internos) = &p.function_parameters {
                listas(internos, saida);
            }
        }
    }
    let mut de_parametros: Vec<&[ast::Parameter]> = Vec::new();
    for f in a.functions.iter() {
        if let Some(ps) = &f.parameters {
            listas(ps, &mut de_parametros);
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind {
            listas(&k.parameters, &mut de_parametros);
        }
    }
    for d in a.decls.iter() {
        if let DeclKind::Typedef(x) = &d.kind
            && let ast::TypedefKind::Legacy { parameters, .. } = &x.kind
        {
            listas(parameters, &mut de_parametros);
        }
    }
    for t in a.types.iter() {
        if let ast::TypeKind::Function { parameters, .. } = &t.kind {
            listas(parameters, &mut de_parametros);
        }
    }

    // `prefer_if_null_operators`: `a == null ? b : a` e `a != null ? a : b`.
    if ligada("prefer_if_null_operators") {
        for e in a.exprs.iter() {
            let ExprKind::Conditional { condition, then, else_ } = &e.kind else { continue };
            let ExprKind::Binary { op, left, right } = &a.expr(*condition).kind else { continue };
            if !matches!(op, BinaryOp::Eq | BinaryOp::NotEq) {
                continue;
            }
            let testada = if matches!(a.expr(*left).kind, ExprKind::Null) {
                *right
            } else if matches!(a.expr(*right).kind, ExprKind::Null) {
                *left
            } else {
                continue;
            };
            let ramo = if *op == BinaryOp::Eq { *else_ } else { *then };
            if fonte_de(ramo) == fonte_de(testada) {
                relatar(&c::PREFER_IF_NULL_OPERATORS, e.span, &[]);
            }
        }
    }
    // `avoid_single_cascade_in_expression_statements`: `a..b();` sozinho.
    if ligada("avoid_single_cascade_in_expression_statements") {
        for s in a.stmts.iter() {
            if let StmtKind::Expression(e) = &s.kind
                && let ExprKind::Cascade { sections, null_aware, .. } = &a.expr(*e).kind
                && sections.len() == 1
            {
                relatar(&c::AVOID_SINGLE_CASCADE_IN_EXPRESSION_STATEMENTS, a.expr(*e).span, &[if *null_aware { "?." } else { "." }]);
            }
        }
    }
    // `use_function_type_syntax_for_parameters`: `void f(int g(int x))`.
    if ligada("use_function_type_syntax_for_parameters") {
        for lista in &de_parametros {
            for p in lista.iter().filter(|p| p.function_parameters.is_some() && !p.this_ && !p.super_) {
                relatar(&c::USE_FUNCTION_TYPE_SYNTAX_FOR_PARAMETERS, p.span, &[p.name.map_or("", |n| interner.resolve(n.sym))]);
            }
        }
    }
    // `no_leading_underscores_for_local_identifiers`.
    if ligada("no_leading_underscores_for_local_identifiers") {
        let mut checar = |n: ast::Name| {
            let texto = interner.resolve(n.sym);
            if texto.starts_with('_') && !texto.bytes().all(|b| b == b'_') {
                relatar(&c::NO_LEADING_UNDERSCORES_FOR_LOCAL_IDENTIFIERS, n.span, &[texto]);
            }
        };
        let da_lista = |l: &VariableList, checar: &mut dyn FnMut(ast::Name)| {
            for v in l.variables.iter() {
                checar(v.name);
            }
        };
        for s in a.stmts.iter() {
            match &s.kind {
                StmtKind::Variables(l) | StmtKind::For { init: Some(ForInit::Variables(l)), .. } => da_lista(l, &mut checar),
                StmtKind::ForIn { target: ForInTarget::Declared { name, .. }, .. } => checar(*name),
                StmtKind::Function(f) => {
                    if let Some(n) = a.function(*f).name {
                        checar(n);
                    }
                }
                StmtKind::Try { catches, .. } => {
                    for k in catches.iter() {
                        for n in k.exception.iter().chain(k.stack_trace.iter()) {
                            checar(*n);
                        }
                    }
                }
                _ => {}
            }
        }
        // `visitDeclaredVariablePattern`.
        for n in super::regras::variaveis_declaradas_em_padroes(a) {
            checar(n);
        }
        // `visitForPartsWithDeclarations` e `visitDeclaredIdentifier` do
        // `for` e do `for-in` de coleção.
        fn de_colecao(el: &ast::CollectionElement, saida: &mut Vec<ast::Name>) {
            match el {
                ast::CollectionElement::For { init, body, .. } => {
                    if let Some(ForInit::Variables(l)) = init {
                        saida.extend(l.variables.iter().map(|v| v.name));
                    }
                    de_colecao(body, saida);
                }
                ast::CollectionElement::ForIn { target, body, .. } => {
                    if let ForInTarget::Declared { name, .. } = target {
                        saida.push(*name);
                    }
                    de_colecao(body, saida);
                }
                ast::CollectionElement::If { then, else_, .. } => {
                    de_colecao(then, saida);
                    if let Some(x) = else_ {
                        de_colecao(x, saida);
                    }
                }
                _ => {}
            }
        }
        let mut de_colecoes = Vec::new();
        for e in a.exprs.iter() {
            if let ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } = &e.kind {
                for el in elements.iter() {
                    de_colecao(el, &mut de_colecoes);
                }
            }
        }
        for n in de_colecoes {
            checar(n);
        }
        // Os parâmetros não nomeados; a lista é abandonada no primeiro
        // `this.x` ou `super.x` (o `return` do original).
        for lista in &de_parametros {
            for p in lista.iter() {
                if p.this_ || p.super_ {
                    break;
                }
                if let (false, Some(n)) = (p.kind == ParameterKind::Named, p.name) {
                    checar(n);
                }
            }
        }
    }
    // `prefer_function_declarations_over_variables`.
    if ligada("prefer_function_declarations_over_variables") {
        let mut checar = |l: &VariableList, local: bool| {
            for v in l.variables.iter() {
                let Some(inicial) = v.initializer else { continue };
                if !matches!(a.expr(inicial).kind, ExprKind::FunctionExpression(_)) {
                    continue;
                }
                // Dentro de corpo de função, o elemento não mudado; fora, a
                // lista `final`.
                let relata = if local { sem.is_some_and(|s| !super::mutado(s, a, v.name.span.start)) } else { l.final_ };
                if relata {
                    relatar(&c::PREFER_FUNCTION_DECLARATIONS_OVER_VARIABLES, Span { start: v.name.span.start, end: a.expr(inicial).span.end }, &[]);
                }
            }
        };
        for d in a.decls.iter() {
            if let DeclKind::Variables(l) = &d.kind {
                checar(l, false);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Field(l) = &m.kind {
                checar(l, false);
            }
        }
        for s in a.stmts.iter() {
            match &s.kind {
                StmtKind::Variables(l) | StmtKind::For { init: Some(ForInit::Variables(l)), .. } => checar(l, true),
                _ => {}
            }
        }
        // As listas do `for` de coleção (dentro de corpo de função ou de
        // inicializador: a de inicializador de campo ou de topo fica fora
        // de corpo).
        let corpos: Vec<Span> = a
            .functions
            .iter()
            .filter_map(|f| match &f.body {
                ast::FunctionBody::Block(s) => Some(a.stmt(*s).span),
                ast::FunctionBody::Expression(e) => Some(a.expr(*e).span),
                _ => None,
            })
            .chain(a.members.iter().filter_map(|m| match &m.kind {
                MemberKind::Constructor(k) => match &k.body {
                    ast::FunctionBody::Block(s) => Some(a.stmt(*s).span),
                    ast::FunctionBody::Expression(e) => Some(a.expr(*e).span),
                    _ => None,
                },
                _ => None,
            }))
            .collect();
        fn listas_de_colecao<'x>(el: &'x ast::CollectionElement, saida: &mut Vec<&'x VariableList>) {
            match el {
                ast::CollectionElement::For { init, body, .. } => {
                    if let Some(ForInit::Variables(l)) = init {
                        saida.push(l);
                    }
                    listas_de_colecao(body, saida);
                }
                ast::CollectionElement::ForIn { body, .. } => listas_de_colecao(body, saida),
                ast::CollectionElement::If { then, else_, .. } => {
                    listas_de_colecao(then, saida);
                    if let Some(x) = else_ {
                        listas_de_colecao(x, saida);
                    }
                }
                _ => {}
            }
        }
        for e in a.exprs.iter() {
            if let ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } = &e.kind {
                let mut ls = Vec::new();
                for el in elements.iter() {
                    listas_de_colecao(el, &mut ls);
                }
                let local = corpos.iter().any(|c| c.start <= e.span.start && e.span.end <= c.end);
                for l in ls {
                    checar(l, local);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;

    fn achados(fonte: &str) -> Vec<(&'static str, String)> {
        let mut nomes = Interner::new();
        let p = dartforge_frontend::parser::parse(fonte, &mut nomes);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte };
        let mut relatos = executar(u, &nomes, &|_| true, None);
        relatos.sort_by_key(|r| (r.span.start, r.span.end));
        relatos.into_iter().map(|r| (r.codigo.nome, fonte[r.span.start..r.span.end].to_string())).collect()
    }

    #[test]
    fn expressoes() {
        assert_eq!(achados("int f(int? a) => a == null ? 0 : a;\n"), vec![("prefer_if_null_operators", "a == null ? 0 : a".to_string())]);
        assert!(achados("int f(int? a, int b) => a == null ? 0 : b;\n").is_empty());
        assert_eq!(
            achados("void f(List<int> l) {\n  l..add(1);\n}\n"),
            vec![("avoid_single_cascade_in_expression_statements", "l..add(1)".to_string())]
        );
    }

    #[test]
    fn nomes_e_parametros() {
        assert_eq!(achados("void f(int g(int x)) {}\n"), vec![("use_function_type_syntax_for_parameters", "int g(int x)".to_string())]);
        assert_eq!(
            achados("void f(int _a, {int? _b}) {\n  var _c = 1;\n  var __ = 2;\n}\n"),
            vec![
                ("no_leading_underscores_for_local_identifiers", "_a".to_string()),
                ("no_leading_underscores_for_local_identifiers", "_c".to_string()),
            ]
        );
    }

    #[test]
    fn funcoes_em_variaveis() {
        assert_eq!(achados("final f = () {};\nvar g = () {};\n"), vec![("prefer_function_declarations_over_variables", "f = () {}".to_string())]);
        // O local pede a semântica da unidade (a mutação é pelo elemento).
        assert!(achados("void m() {\n  var a = () {};\n  var b = () {};\n  b = () {};\n}\n").is_empty());
    }
}
