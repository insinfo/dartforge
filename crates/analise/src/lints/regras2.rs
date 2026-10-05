//! O segundo lote de regras de lint que só olham a árvore
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8):
//! `avoid_return_types_on_setters`, `empty_constructor_bodies`,
//! `library_prefixes`, `no_leading_underscores_for_library_prefixes`,
//! `constant_identifier_names`, `prefer_is_not_operator`,
//! `prefer_adjacent_string_concatenation`,
//! `unnecessary_null_in_if_null_operators`, `unnecessary_late`,
//! `empty_statements` e `use_rethrow_when_possible`.
//!
//! Escritas com os emissores do `main` do SDK abertos e depois conferidas
//! contra os da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter`, extraído
//! da tag em 2026-10-05). Na 3.6.2 o corpo vazio de um construtor
//! `factory` também é relatado e o prefixo `_` não é isento (o curinga é da
//! 3.7). Diferenças conhecidas:
//! `no_leading_underscores_for_library_prefixes` só relata a variante sem
//! sombreamento; `constant_identifier_names` não olha os padrões de
//! variável; `use_rethrow_when_possible` reconhece a variável do `catch`
//! pelo nome, sem conferir sombreamento por outra declaração.
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::regras::{e_lower_camel_case, RelatoDeLint};
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{BinaryOp, DeclKind, DirectiveKind, ExprId, ExprKind, FunctionBody, FunctionKind, MemberKind, StmtKind, UnaryOp};
use dartforge_intern::Interner;

/// `isValidLibraryPrefix`: `^\$?_*[a-z][_a-z\d]*$`.
pub fn e_prefixo_valido(nome: &str) -> bool {
    let sem_cifrao = nome.strip_prefix('$').unwrap_or(nome);
    let mut letras = sem_cifrao.trim_start_matches('_').chars();
    letras.next().is_some_and(|p| p.is_ascii_lowercase()) && letras.all(|x| x == '_' || x.is_ascii_lowercase() || x.is_ascii_digit())
}

/// A posição da palavra `palavra` em `fonte[de..ate]` (a última, inteira).
fn palavra_em(fonte: &str, de: usize, ate: usize, palavra: &str) -> Option<Span> {
    let trecho = fonte.get(de..ate)?;
    let b = trecho.as_bytes();
    let de_palavra = |x: u8| x.is_ascii_alphanumeric() || x == b'_' || x == b'$';
    let mut achado = None;
    let mut inicio = 0;
    while let Some(k) = trecho[inicio..].find(palavra) {
        let i = inicio + k;
        let antes = i == 0 || !de_palavra(b[i - 1]);
        let depois = b.get(i + palavra.len()).is_none_or(|x| !de_palavra(*x));
        if antes && depois {
            achado = Some(Span { start: de + i, end: de + i + palavra.len() });
        }
        inicio = i + palavra.len();
    }
    achado
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let a = u.ast;
    let fonte = u.fonte;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
    // Sem comentário: só brancos entre as chaves de um bloco vazio.
    let bloco_vazio = |s: dartforge_frontend::ast::StmtId| {
        let corpo = a.stmt(s);
        let sem_comandos = matches!(&corpo.kind, StmtKind::Block(l) if l.is_empty());
        let miolo = fonte.get(corpo.span.start + 1..corpo.span.end.saturating_sub(1)).unwrap_or("x");
        (sem_comandos && miolo.trim().is_empty()).then_some(corpo.span)
    };

    // `avoid_return_types_on_setters`.
    if ligada("avoid_return_types_on_setters") {
        for f in a.functions.iter() {
            if let (FunctionKind::Setter, Some(t)) = (f.kind, f.return_type) {
                relatar(&c::AVOID_RETURN_TYPES_ON_SETTERS, a.types[t.0 as usize].span, &[]);
            }
        }
    }
    // `empty_constructor_bodies`: construtor de corpo `{}` (na 3.6.2,
    // também o `factory`).
    if ligada("empty_constructor_bodies") {
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind
                && let FunctionBody::Block(s) = &k.body
                && let Some(span) = bloco_vazio(*s)
            {
                relatar(&c::EMPTY_CONSTRUCTOR_BODIES, span, &[]);
            }
        }
    }
    // `library_prefixes` e `no_leading_underscores_for_library_prefixes`.
    let (prefixos, sem_sublinhado) = (ligada("library_prefixes"), ligada("no_leading_underscores_for_library_prefixes"));
    if prefixos || sem_sublinhado {
        for d in u.unit.directives.iter() {
            let DirectiveKind::Import { prefix: Some(p), .. } = &d.kind else { continue };
            let texto = interner.resolve(p.sym);
            if prefixos && !e_prefixo_valido(texto) {
                relatar(&c::LIBRARY_PREFIXES, p.span, &[texto]);
            }
            if sem_sublinhado && texto.starts_with('_') {
                relatar(&c::NO_LEADING_UNDERSCORES_FOR_LIBRARY_PREFIXES, p.span, &[texto]);
            }
        }
    }
    // `constant_identifier_names`: constantes de enum e variáveis `const`.
    if ligada("constant_identifier_names") {
        let mut checar = |n: dartforge_frontend::ast::Name| {
            let texto = interner.resolve(n.sym);
            if !e_lower_camel_case(texto) {
                relatar(&c::CONSTANT_IDENTIFIER_NAMES, n.span, &[texto]);
            }
        };
        for d in a.decls.iter().filter(|d| !d.augment) {
            match &d.kind {
                DeclKind::Enum(x) => x.constants.iter().for_each(|k| checar(k.name)),
                DeclKind::Variables(l) if l.const_ => l.variables.iter().for_each(|v| checar(v.name)),
                _ => {}
            }
        }
        for m in a.members.iter().filter(|m| !m.augment) {
            if let MemberKind::Field(l) = &m.kind
                && l.const_
            {
                l.variables.iter().for_each(|v| checar(v.name));
            }
        }
        for s in a.stmts.iter() {
            if let StmtKind::Variables(l) = &s.kind
                && l.const_
            {
                l.variables.iter().for_each(|v| checar(v.name));
            }
        }
    }
    // As regras de expressão.
    let (e_nao, adjacentes, nulo_em_se_nulo) =
        (ligada("prefer_is_not_operator"), ligada("prefer_adjacent_string_concatenation"), ligada("unnecessary_null_in_if_null_operators"));
    if e_nao || adjacentes || nulo_em_se_nulo {
        for e in a.exprs.iter() {
            match &e.kind {
                // `!(x is T)`.
                ExprKind::Unary { op: UnaryOp::Not, operand } if e_nao => {
                    if let ExprKind::Parenthesized(dentro) = &a.expr(*operand).kind
                        && matches!(a.expr(*dentro).kind, ExprKind::Is { negated: false, .. })
                    {
                        relatar(&c::PREFER_IS_NOT_OPERATOR, e.span, &[]);
                    }
                }
                // `'a' + 'b'`: no `+`.
                ExprKind::Binary { op: BinaryOp::Add, left, right } if adjacentes => {
                    if matches!(a.expr(*left).kind, ExprKind::String(_)) && matches!(a.expr(*right).kind, ExprKind::String(_)) {
                        let (de, ate) = (a.expr(*left).span.end, a.expr(*right).span.start);
                        if let Some(k) = fonte.get(de..ate).and_then(|t| t.find('+')) {
                            relatar(&c::PREFER_ADJACENT_STRING_CONCATENATION, Span { start: de + k, end: de + k + 1 }, &[]);
                        }
                    }
                }
                // `x ?? null` e `null ?? x`.
                ExprKind::Binary { op: BinaryOp::IfNull, left, right } if nulo_em_se_nulo => {
                    if matches!(a.expr(*right).kind, ExprKind::Null) {
                        relatar(&c::UNNECESSARY_NULL_IN_IF_NULL_OPERATORS, a.expr(*right).span, &[]);
                    } else if matches!(a.expr(*left).kind, ExprKind::Null) {
                        relatar(&c::UNNECESSARY_NULL_IN_IF_NULL_OPERATORS, a.expr(*left).span, &[]);
                    }
                }
                _ => {}
            }
        }
    }
    // `unnecessary_late`: variável de topo ou campo estático `late` com
    // todas as variáveis inicializadas; na palavra `late`.
    if ligada("unnecessary_late") {
        let mut checar = |lista: &dartforge_frontend::ast::VariableList, inicio: usize| {
            if !lista.late || lista.variables.iter().any(|v| v.initializer.is_none()) {
                return;
            }
            let Some(primeira) = lista.variables.first() else { return };
            if let Some(span) = palavra_em(fonte, inicio, primeira.name.span.start, "late") {
                relatar(&c::UNNECESSARY_LATE, span, &[]);
            }
        };
        for d in a.decls.iter() {
            if let DeclKind::Variables(l) = &d.kind {
                checar(l, d.span.start);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Field(l) = &m.kind
                && l.static_
            {
                checar(l, m.span.start);
            }
        }
    }
    // `empty_statements`: todo `;` solto, menos o que dá sentido a um caso
    // de `switch` feito só de comandos vazios (o último deles).
    if ligada("empty_statements") {
        let mut com_sentido: Vec<dartforge_frontend::ast::StmtId> = Vec::new();
        for s in a.stmts.iter() {
            if let StmtKind::Switch { cases, .. } = &s.kind {
                for caso in cases.iter() {
                    if let Some(ultimo) = caso.body.last()
                        && caso.body.iter().all(|x| matches!(a.stmt(*x).kind, StmtKind::Empty))
                    {
                        com_sentido.push(*ultimo);
                    }
                }
            }
        }
        for (i, s) in a.stmts.iter().enumerate() {
            if matches!(s.kind, StmtKind::Empty)
                && s.span.end > s.span.start
                && !com_sentido.contains(&dartforge_frontend::ast::StmtId(i as u32))
            {
                relatar(&c::EMPTY_STATEMENTS, s.span, &[]);
            }
        }
    }
    // `use_rethrow_when_possible`: o comando `throw e;` dentro do `catch`
    // que declarou `e`.
    if ligada("use_rethrow_when_possible") {
        // Os `throw` que são um comando de expressão inteiro.
        let comandos: Vec<ExprId> = a
            .stmts
            .iter()
            .filter_map(|s| match &s.kind {
                StmtKind::Expression(e) if matches!(a.expr(*e).kind, ExprKind::Throw(_)) => Some(*e),
                _ => None,
            })
            .collect();
        let mut relatados: Vec<Span> = Vec::new();
        for s in a.stmts.iter() {
            let StmtKind::Try { catches, .. } = &s.kind else { continue };
            for k in catches.iter() {
                let Some(excecao) = k.exception else { continue };
                let corpo = a.stmt(k.body).span;
                for &e in &comandos {
                    let expr = a.expr(e);
                    if !(corpo.start <= expr.span.start && expr.span.end <= corpo.end) || relatados.contains(&expr.span) {
                        continue;
                    }
                    if let ExprKind::Throw(x) = &expr.kind
                        && matches!(&a.expr(*x).kind, ExprKind::Identifier(n) if n.sym == excecao.sym)
                    {
                        relatados.push(expr.span);
                    }
                }
            }
        }
        for span in relatados {
            relatar(&c::USE_RETHROW_WHEN_POSSIBLE, span, &[]);
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
        let mut relatos = executar(u, &nomes, &|_| true);
        relatos.sort_by_key(|r| (r.span.start, r.span.end));
        relatos.into_iter().map(|r| (r.codigo.nome, fonte[r.span.start..r.span.end].to_string())).collect()
    }

    #[test]
    fn prefixos() {
        assert!(e_prefixo_valido("math") && e_prefixo_valido("_a_b1") && e_prefixo_valido("$x"));
        assert!(!e_prefixo_valido("Math") && !e_prefixo_valido("1a"));
        assert_eq!(achados("import 'a.dart' as Abc;\n"), vec![("library_prefixes", "Abc".to_string())]);
        assert_eq!(achados("import 'a.dart' as _a;\n"), vec![("no_leading_underscores_for_library_prefixes", "_a".to_string())]);
    }

    #[test]
    fn declaracoes() {
        assert_eq!(achados("void set x(int v) {}\n"), vec![("avoid_return_types_on_setters", "void".to_string())]);
        assert_eq!(achados("class A {\n  A() {}\n  factory A.f() => A();\n}\n"), vec![("empty_constructor_bodies", "{}".to_string())]);
        // Na 3.6.2 o prefixo `_` não é curinga: as duas regras o relatam.
        assert_eq!(achados("import 'a.dart' as _;\n").len(), 2);
        assert_eq!(achados("const MAX_VALOR = 1;\nenum E { Um, dois }\n"), vec![
            ("constant_identifier_names", "MAX_VALOR".to_string()),
            ("constant_identifier_names", "Um".to_string()),
        ]);
        assert_eq!(achados("late int a = 1;\nlate int b;\n"), vec![("unnecessary_late", "late".to_string())]);
    }

    #[test]
    fn expressoes_e_comandos() {
        assert_eq!(achados("bool f(Object o) => !(o is int);\n"), vec![("prefer_is_not_operator", "!(o is int)".to_string())]);
        assert_eq!(achados("var s = 'a' + 'b';\n"), vec![("prefer_adjacent_string_concatenation", "+".to_string())]);
        assert_eq!(achados("var s = f() ?? null;\nint? f() => 1;\n"), vec![("unnecessary_null_in_if_null_operators", "null".to_string())]);
        assert_eq!(achados("void f() {\n  ;\n}\n"), vec![("empty_statements", ";".to_string())]);
        assert_eq!(
            achados("void f() {\n  try {\n    f();\n  } catch (e) {\n    throw e;\n  }\n}\n"),
            vec![("use_rethrow_when_possible", "throw e".to_string())]
        );
    }
}
