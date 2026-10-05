//! O décimo primeiro lote de regras de lint que só olham a árvore e o texto
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//! `no_adjacent_strings_in_list`, `missing_whitespace_between_adjacent_strings`,
//! `flutter_style_todos`, `unnecessary_library_name`,
//! `prefer_null_aware_method_calls`, `avoid_print`,
//! `avoid_catches_without_on_clauses` e `avoid_setters_without_getters`.
//!
//! Diferenças conhecidas:
//! - `avoid_print` decide "é o `print` do `dart:core`" pelo nome: a unidade
//!   que declara algo chamado `print` (função, método, variável, parâmetro)
//!   fica de fora, e a chamada com prefixo não é olhada. O `kDebugMode` é
//!   reconhecido pelo nome.
//! - `avoid_catches_without_on_clauses` reconhece as chamadas que não
//!   voltam e as que entregam o erro pelo nome (`throwWithStackTrace`,
//!   `Future.error`, `completeError`, `FlutterError.reportError`); a do
//!   usuário que devolve `Never` não conta. O uso da exceção é pelo nome.
//! - `avoid_setters_without_getters` só decide a classe sem `extends` nem
//!   `with` (sem superclasse, o `lookUpGetter` só vê a própria classe e
//!   `Object`); com elas nada se relata.
//! - `missing_whitespace_between_adjacent_strings` lê os brancos do texto
//!   escrito, com os escapes `\n`, `\t` e `\r` contados como branco.
//! - `no_adjacent_strings_in_list` não olha os padrões de lista do `switch`.
//! - `unnecessary_library_name` supõe a versão de linguagem 2.19 ou mais.
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::cordas;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    Ast, BinaryOp, CollectionElement, DeclKind, DirectiveKind, ExprId, ExprKind, FunctionKind, MemberKind, StmtKind, UnaryOp,
};
use dartforge_intern::{Interner, SymbolId};

fn dentro(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

/// A expressão sem os parênteses de fora.
fn sem_parenteses(a: &Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = a.expr(e).kind {
        e = x;
    }
    e
}

/// O texto da fonte sem os brancos (o `toSource` compara tokens).
fn sem_brancos(fonte: &str, s: Span) -> String {
    fonte.get(s.start..s.end).unwrap_or("").chars().filter(|c| !c.is_whitespace()).collect()
}

/// Strings adjacentes (`'a' 'b'`): um `String` escrito com mais de um literal.
fn adjacentes(a: &Ast, fonte: &str, e: ExprId) -> bool {
    let e = a.expr(e);
    matches!(e.kind, ExprKind::String(_)) && cordas::literais(fonte, e.span).len() > 1
}

/// `flutter_style_todos`: `//+\s*TODO\b` (sem caixa) que não começa com
/// `// TODO\([a-zA-Z0-9][-a-zA-Z0-9\.]*\): `.
fn todo_fora_do_estilo(texto: &str) -> bool {
    let b = texto.as_bytes();
    let barras = b.iter().take_while(|&&x| x == b'/').count();
    if barras < 2 {
        return false;
    }
    let mut i = barras;
    while i < b.len() && (b[i] as char).is_ascii_whitespace() {
        i += 1;
    }
    if b.len() < i + 4 || !b[i..i + 4].eq_ignore_ascii_case(b"todo") {
        return false;
    }
    if b.get(i + 4).is_some_and(|&x| x == b'_' || x.is_ascii_alphanumeric()) {
        return false;
    }
    // O formato esperado.
    let Some(resto) = texto.strip_prefix("// TODO(") else { return true };
    let r = resto.as_bytes();
    if r.first().is_none_or(|x| !x.is_ascii_alphanumeric()) {
        return true;
    }
    let mut k = 1;
    while k < r.len() && (r[k].is_ascii_alphanumeric() || r[k] == b'-' || r[k] == b'.') {
        k += 1;
    }
    !resto[k..].starts_with("): ")
}

/// Os brancos do `missing_whitespace_between_adjacent_strings` (` `, `\n`,
/// `\r`, `\t`), também escritos como escape fora das strings cruas.
fn termina_com_branco(t: &str, crua: bool) -> bool {
    t.ends_with([' ', '\n', '\r', '\t']) || (!crua && (t.ends_with("\\n") || t.ends_with("\\r") || t.ends_with("\\t")))
}

fn comeca_com_branco(t: &str, crua: bool) -> bool {
    t.starts_with([' ', '\n', '\r', '\t']) || (!crua && (t.starts_with("\\n") || t.starts_with("\\r") || t.starts_with("\\t")))
}

fn tem_branco(t: &str, crua: bool) -> bool {
    t.contains([' ', '\n', '\r', '\t']) || (!crua && (t.contains("\\n") || t.contains("\\r") || t.contains("\\t")))
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, _sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let a = u.ast;
    let fonte = u.fonte;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };

    // `no_adjacent_strings_in_list`.
    if ligada("no_adjacent_strings_in_list") {
        fn elementos(a: &Ast, fonte: &str, el: &CollectionElement, direto: bool, saida: &mut Vec<Span>) {
            match el {
                CollectionElement::Expression(e) if direto && adjacentes(a, fonte, *e) => saida.push(a.expr(*e).span),
                // `visitForElement`: o corpo adjacente.
                CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => {
                    if let CollectionElement::Expression(e) = body.as_ref()
                        && adjacentes(a, fonte, *e)
                    {
                        saida.push(a.expr(*e).span);
                    }
                    elementos(a, fonte, body, false, saida);
                }
                // `visitIfElement`: sem `else`, o `then` adjacente; com
                // `else`, só o `else`.
                CollectionElement::If { then, else_, .. } => {
                    match else_ {
                        None => {
                            if let CollectionElement::Expression(e) = then.as_ref()
                                && adjacentes(a, fonte, *e)
                            {
                                saida.push(a.expr(*e).span);
                            }
                        }
                        Some(x) => {
                            if let CollectionElement::Expression(e) = x.as_ref()
                                && adjacentes(a, fonte, *e)
                            {
                                saida.push(a.expr(*e).span);
                            }
                        }
                    }
                    elementos(a, fonte, then, false, saida);
                    if let Some(x) = else_ {
                        elementos(a, fonte, x, false, saida);
                    }
                }
                _ => {}
            }
        }
        let mut achados = Vec::new();
        for e in a.exprs.iter() {
            match &e.kind {
                ExprKind::List { elements, .. } => {
                    for el in elements.iter() {
                        elementos(a, fonte, el, true, &mut achados);
                    }
                }
                ExprKind::SetOrMap { elements, type_args, .. } => {
                    let mapa = type_args.len() == 2
                        || (type_args.is_empty()
                            && (elements.is_empty() || elements.iter().any(|x| matches!(x, CollectionElement::MapEntry { .. }))));
                    for el in elements.iter() {
                        elementos(a, fonte, el, !mapa, &mut achados);
                    }
                }
                _ => {}
            }
        }
        achados.sort_by_key(|s| (s.start, s.end));
        achados.dedup();
        for s in achados {
            relatar(&c::NO_ADJACENT_STRINGS_IN_LIST, s, &[]);
        }
    }

    // `missing_whitespace_between_adjacent_strings`.
    if ligada("missing_whitespace_between_adjacent_strings") {
        // Os argumentos de `RegExp(…)` e de `matches(…)` sem alvo.
        let mut isentos: Vec<ExprId> = Vec::new();
        for e in a.exprs.iter() {
            let args = match &e.kind {
                ExprKind::Call { target, arguments } => match &a.expr(*target).kind {
                    ExprKind::Identifier(n) if matches!(interner.resolve(n.sym), "RegExp" | "matches") => Some(arguments),
                    _ => None,
                },
                ExprKind::InstanceCreation { ty, arguments, .. } => {
                    let texto = fonte.get(a.ty(*ty).span.start..a.ty(*ty).span.end).unwrap_or("");
                    (texto == "RegExp" || texto.ends_with(".RegExp")).then_some(arguments)
                }
                _ => None,
            };
            if let Some(args) = args {
                isentos.extend(args.args.iter().map(|x| x.value));
            }
        }
        for (k, e) in a.exprs.iter().enumerate() {
            if !matches!(e.kind, ExprKind::String(_)) || isentos.contains(&ExprId(k as u32)) {
                continue;
            }
            let lits = cordas::literais(fonte, e.span);
            for par in lits.windows(2) {
                let (atual, proximo) = (&par[0], &par[1]);
                let fim = atual.trechos.last().and_then(|t| fonte.get(t.start..t.end)).unwrap_or("");
                let ini = proximo.trechos.first().and_then(|t| fonte.get(t.start..t.end)).unwrap_or("");
                let termina = (atual.interpolado() && fim.is_empty()) || termina_com_branco(fim, atual.crua);
                let comeca = (proximo.interpolado() && ini.is_empty()) || comeca_com_branco(ini, proximo.crua);
                if termina || comeca {
                    continue;
                }
                if !tem_branco(&atual.texto(fonte), atual.crua) {
                    continue;
                }
                relatar(&c::MISSING_WHITESPACE_BETWEEN_ADJACENT_STRINGS, atual.span, &[]);
            }
        }
    }

    // `flutter_style_todos`.
    if ligada("flutter_style_todos") {
        for s in cordas::comentarios(fonte) {
            if todo_fora_do_estilo(fonte.get(s.start..s.end).unwrap_or("")) {
                relatar(&c::FLUTTER_STYLE_TODOS, s, &[]);
            }
        }
    }

    // `unnecessary_library_name`.
    if ligada("unnecessary_library_name") {
        for d in &u.unit.directives {
            if let DirectiveKind::Library { name } = &d.kind
                && let (Some(p), Some(f)) = (name.first(), name.last())
            {
                relatar(&c::UNNECESSARY_LIBRARY_NAME, Span { start: p.span.start, end: f.span.end }, &[]);
            }
        }
    }

    // `prefer_null_aware_method_calls`.
    if ligada("prefer_null_aware_method_calls") {
        // `x != null`: o operando da esquerda.
        let comparado_com_nulo = |cond: ExprId| -> Option<ExprId> {
            match &a.expr(cond).kind {
                ExprKind::Binary { op: BinaryOp::NotEq, left, right } if matches!(a.expr(*right).kind, ExprKind::Null) => Some(*left),
                _ => None,
            }
        };
        // `x!()` com o mesmo `x`.
        let invocacao = |e: ExprId, esquerda: ExprId| -> Option<Span> {
            let ExprKind::Call { target, .. } = &a.expr(e).kind else { return None };
            let ExprKind::Unary { op: UnaryOp::NullAssert, operand } = &a.expr(*target).kind else { return None };
            (sem_brancos(fonte, a.expr(*operand).span) == sem_brancos(fonte, a.expr(esquerda).span)).then_some(a.expr(e).span)
        };
        let mut achados = Vec::new();
        for e in a.exprs.iter() {
            if let ExprKind::Conditional { condition, then, else_ } = &e.kind
                && matches!(a.expr(*else_).kind, ExprKind::Null)
                && let Some(esq) = comparado_com_nulo(*condition)
                && let Some(s) = invocacao(*then, esq)
            {
                achados.push(s);
            }
        }
        for s in a.stmts.iter() {
            let StmtKind::If { condition, case_pattern: None, then, else_: None, .. } = &s.kind else { continue };
            let Some(esq) = comparado_com_nulo(*condition) else { continue };
            let mut corpo = *then;
            if let StmtKind::Block(b) = &a.stmt(corpo).kind {
                let [unico] = &b[..] else { continue };
                corpo = *unico;
            }
            if let StmtKind::Expression(x) = &a.stmt(corpo).kind
                && let Some(sp) = invocacao(*x, esq)
            {
                achados.push(sp);
            }
        }
        achados.sort_by_key(|s| (s.start, s.end));
        for s in achados {
            relatar(&c::PREFER_NULL_AWARE_METHOD_CALLS, s, &[]);
        }
    }

    // `avoid_print`.
    if ligada("avoid_print") {
        let print = interner.lookup("print");
        let declara_print = |s: SymbolId| {
            a.functions.iter().any(|f| f.name.is_some_and(|n| n.sym == s))
                || a.decls.iter().any(|d| match &d.kind {
                    DeclKind::Variables(v) => v.variables.iter().any(|x| x.name.sym == s),
                    _ => false,
                })
                || a.members.iter().any(|m| match &m.kind {
                    MemberKind::Field(v) => v.variables.iter().any(|x| x.name.sym == s),
                    _ => false,
                })
                || a.stmts.iter().any(|st| match &st.kind {
                    StmtKind::Variables(v) => v.variables.iter().any(|x| x.name.sym == s),
                    _ => false,
                })
                || a.functions.iter().any(|f| f.parameters.as_ref().is_some_and(|ps| ps.iter().any(|p| p.name.is_some_and(|n| n.sym == s))))
        };
        if let Some(print) = print
            && !declara_print(print)
        {
            let kdebug = interner.lookup("kDebugMode");
            // Os ramos `then` de `if (kDebugMode)`.
            let depuracao: Vec<Span> = a
                .stmts
                .iter()
                .filter_map(|s| match &s.kind {
                    StmtKind::If { condition, then, .. }
                        if matches!(a.expr(*condition).kind, ExprKind::Identifier(n) if Some(n.sym) == kdebug) =>
                    {
                        Some(a.stmt(*then).span)
                    }
                    _ => None,
                })
                .collect();
            let so_depuracao = |s: Span| {
                depuracao.iter().any(|&r| {
                    dentro(s, r) && !a.functions.iter().any(|f| dentro(s, f.span) && dentro(f.span, r) && f.span != r)
                })
            };
            for e in a.exprs.iter() {
                let ExprKind::Call { target, arguments } = &e.kind else { continue };
                if let ExprKind::Identifier(n) = &a.expr(*target).kind
                    && n.sym == print
                    && !so_depuracao(e.span)
                {
                    relatar(&c::AVOID_PRINT, n.span, &[]);
                }
                for arg in arguments.args.iter() {
                    if let ExprKind::Identifier(n) = &a.expr(sem_parenteses(a, arg.value)).kind
                        && n.sym == print
                        && arg.value == sem_parenteses(a, arg.value)
                    {
                        relatar(&c::AVOID_PRINT, n.span, &[]);
                    }
                }
            }
        }
    }

    // `avoid_catches_without_on_clauses`.
    if ligada("avoid_catches_without_on_clauses") {
        let usa = |regiao: Span, s: SymbolId| {
            a.exprs.iter().any(|e| matches!(&e.kind, ExprKind::Identifier(n) if n.sym == s) && dentro(e.span, regiao))
        };
        for st in a.stmts.iter() {
            let StmtKind::Try { catches, .. } = &st.kind else { continue };
            for cc in catches.iter() {
                if cc.on_type.is_some() {
                    continue;
                }
                let Some(ex) = cc.exception else { continue };
                let corpo = a.stmt(cc.body).span;
                // `_canRethrow` cai no primeiro `catch` de dentro.
                let primeiro_catch_de_dentro = a
                    .stmts
                    .iter()
                    .filter_map(|s| match &s.kind {
                        StmtKind::Try { catches, .. } if dentro(s.span, corpo) => catches.first().map(|k| k.span.start),
                        _ => None,
                    })
                    .min()
                    .unwrap_or(usize::MAX);
                let mut valido = false;
                for e in a.exprs.iter().filter(|e| dentro(e.span, corpo)) {
                    match &e.kind {
                        ExprKind::Rethrow => valido |= e.span.start < primeiro_catch_de_dentro,
                        ExprKind::Throw(x) => valido |= usa(a.expr(*x).span, ex.sym),
                        ExprKind::Call { target, arguments } => {
                            let entrega = match &a.expr(*target).kind {
                                ExprKind::Property { target: alvo, name, .. } => match interner.resolve(name.sym) {
                                    "throwWithStackTrace" | "completeError" => true,
                                    "error" => matches!(&a.expr(*alvo).kind, ExprKind::Identifier(n) if interner.resolve(n.sym) == "Future"),
                                    "reportError" => {
                                        matches!(&a.expr(*alvo).kind, ExprKind::Identifier(n) if interner.resolve(n.sym) == "FlutterError")
                                    }
                                    _ => false,
                                },
                                _ => false,
                            };
                            valido |= entrega && usa(arguments.span, ex.sym);
                        }
                        ExprKind::InstanceCreation { ty, constructor: Some(nome), arguments, .. } => {
                            let tipo = fonte.get(a.ty(*ty).span.start..a.ty(*ty).span.end).unwrap_or("");
                            valido |= interner.resolve(nome.sym) == "error"
                                && (tipo == "Future" || tipo.starts_with("Future<"))
                                && usa(arguments.span, ex.sym);
                        }
                        _ => {}
                    }
                }
                if !valido {
                    relatar(&c::AVOID_CATCHES_WITHOUT_ON_CLAUSES, Span { start: cc.span.start, end: cc.span.start + "catch".len() }, &[]);
                }
            }
        }
    }

    // `avoid_setters_without_getters`.
    if ligada("avoid_setters_without_getters") {
        for d in a.decls.iter() {
            let (membros, sem_superclasse, de_enum, representacao): (&[_], bool, bool, Option<SymbolId>) = match &d.kind {
                DeclKind::Class(x) => (x.members.as_slice(), x.extends.is_none() && x.with.is_empty(), false, None),
                DeclKind::Enum(x) => (x.members.as_slice(), x.with.is_empty(), true, None),
                DeclKind::ExtensionType(x) => (x.members.as_slice(), true, false, Some(x.representation_name.sym)),
                _ => continue,
            };
            if !sem_superclasse {
                continue;
            }
            let tem_getter = |s: SymbolId| {
                Some(s) == representacao
                    || matches!(interner.resolve(s), "hashCode" | "runtimeType")
                    || (de_enum && interner.resolve(s) == "index")
                    || membros.iter().any(|&m| match &a.member(m).kind {
                        MemberKind::Field(v) => v.variables.iter().any(|x| x.name.sym == s),
                        MemberKind::Method(f) => {
                            let f = a.function(*f);
                            f.kind == FunctionKind::Getter && f.name.is_some_and(|n| n.sym == s)
                        }
                        MemberKind::Constructor(_) => false,
                    })
            };
            for &m in membros {
                let MemberKind::Method(f) = &a.member(m).kind else { continue };
                let f = a.function(*f);
                if f.kind != FunctionKind::Setter {
                    continue;
                }
                let Some(n) = f.name else { continue };
                if !tem_getter(n.sym) {
                    relatar(&c::AVOID_SETTERS_WITHOUT_GETTERS, n.span, &[]);
                }
            }
        }
    }

    out
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn todo_no_estilo_do_flutter() {
        assert!(todo_fora_do_estilo("// TODO: x"));
        assert!(todo_fora_do_estilo("//todo(a): x"));
        assert!(todo_fora_do_estilo("/// TODO(a): x"));
        assert!(!todo_fora_do_estilo("// TODO(user1): fazer"));
        assert!(!todo_fora_do_estilo("// TODOS os casos"));
        assert!(!todo_fora_do_estilo("// nada"));
        assert!(!todo_fora_do_estilo("/* TODO */"));
    }

    #[test]
    fn brancos_escritos_e_por_escape() {
        assert!(termina_com_branco("abc ", false));
        assert!(termina_com_branco("abc\\n", false));
        assert!(!termina_com_branco("abc\\n", true));
        assert!(comeca_com_branco("\\tabc", false));
        assert!(tem_branco("a b", true));
        assert!(!tem_branco("ab", false));
    }
}
