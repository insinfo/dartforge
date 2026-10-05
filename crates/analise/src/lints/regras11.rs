//! O décimo primeiro lote de regras de lint que só olham a árvore e o texto
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//! `no_adjacent_strings_in_list`, `missing_whitespace_between_adjacent_strings`,
//! `flutter_style_todos`, `unnecessary_library_name`,
//! `prefer_null_aware_method_calls`, `avoid_print`,
//! `avoid_catches_without_on_clauses` e `avoid_setters_without_getters`.
//!
//! Com o mesmo dado do original (as que pedem o elemento só relatam com a
//! semântica da unidade):
//! - `avoid_print`: o `print` do `dart:core` e o `kDebugMode` do
//!   `package:flutter` pelo elemento; o tear-off nos argumentos posicionais.
//! - `avoid_catches_without_on_clauses`: o `_ValidUseVisitor` na ordem do
//!   texto (o `rethrow` antes de um `catch` de dentro, o `throw`, a chamada
//!   `Never`, `Future.error`, `FlutterError.reportError`, `completeError` de
//!   `Completer`), a exceção pelo elemento.
//! - `avoid_setters_without_getters`: `lookUpGetter` e
//!   `lookUpInheritedConcreteSetter` na cadeia de implementação; sem a
//!   semântica, só a declaração sem superclasse nem mixin.
//! - `missing_whitespace_between_adjacent_strings`: os valores
//!   decodificados; a isenção só para o argumento posicional direto de
//!   `RegExp` (pela classe do construtor) e de `matches`.
//! - `no_adjacent_strings_in_list`: o mapa pelo tipo estático, e os padrões
//!   de lista do `switch` (com padrões).
//! - `unnecessary_library_name`: só da 2.19 em diante.
//! - `prefer_null_aware_method_calls`: o `toSource` dos dois lados.
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::cordas;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, Ast, BinaryOp, CollectionElement, DeclKind, DirectiveKind, ExprId, ExprKind, FunctionBody, FunctionKind, MemberKind, StmtKind,
    UnaryOp,
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

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
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
        for (k, e) in a.exprs.iter().enumerate() {
            match &e.kind {
                ExprKind::List { elements, .. } => {
                    for el in elements.iter() {
                        elementos(a, fonte, el, true, &mut achados);
                    }
                }
                ExprKind::SetOrMap { elements, type_args, .. } => {
                    // `isMap`: pelo tipo estático, com a semântica; sem ela,
                    // pelos argumentos de tipo e pelos elementos.
                    let pelo_tipo = sem.and_then(|s| {
                        let t = s.corpo.get_type(ExprId(k as u32))?;
                        match s.table.get(t) {
                            dartforge_types::table::Type::Interface { class, .. } => Some(Some(*class) == s.core.map_class),
                            _ => None,
                        }
                    });
                    let mapa = pelo_tipo.unwrap_or_else(|| {
                        type_args.len() == 2
                            || (type_args.is_empty()
                                && (elements.is_empty() || elements.iter().any(|x| matches!(x, CollectionElement::MapEntry { .. }))))
                    });
                    for el in elements.iter() {
                        elementos(a, fonte, el, !mapa, &mut achados);
                    }
                }
                _ => {}
            }
        }
        // `visitSwitchPatternCase` (com padrões, 3.0 ou mais): os elementos
        // constantes de um padrão de lista.
        if sem.is_some_and(|s| super::versao_ao_menos(s, 3, 0)) {
            for st in a.stmts.iter() {
                let StmtKind::Switch { cases, .. } = &st.kind else { continue };
                for k in cases.iter() {
                    let Some(mut padrao) = k.pattern else { continue };
                    while let ast::PatternKind::Parenthesized(x) = &a.pattern(padrao).kind {
                        padrao = *x;
                    }
                    let ast::PatternKind::List { elements, .. } = &a.pattern(padrao).kind else { continue };
                    for el in elements.iter() {
                        if let ast::ListPatternElement::Pattern(x) = el
                            && let ast::PatternKind::Constant(e) = &a.pattern(*x).kind
                        {
                            let e = sem_parenteses(a, *e);
                            if adjacentes(a, fonte, e) {
                                achados.push(a.expr(e).span);
                            }
                        }
                    }
                }
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
        for (k, e) in a.exprs.iter().enumerate() {
            let args = match &e.kind {
                ExprKind::Call { target, arguments } => match &a.expr(*target).kind {
                    ExprKind::Identifier(n) if matches!(interner.resolve(n.sym), "RegExp" | "matches") => Some(arguments),
                    _ => None,
                },
                // A criação: a classe do construtor chama `RegExp` (pelo
                // elemento, com a semântica; sem ela, pelo nome escrito).
                ExprKind::InstanceCreation { ty, arguments, .. } => {
                    let pelo_elemento = sem.and_then(|s| {
                        match s.corpo.get_resolved(ExprId(k as u32))? {
                            dartforge_types::resolved::Resolved::Constructor(f) => {
                                s.program.function(*f).class.map(|c| interner.resolve(s.program.class(c).name) == "RegExp")
                            }
                            _ => None,
                        }
                    });
                    let texto = fonte.get(a.ty(*ty).span.start..a.ty(*ty).span.end).unwrap_or("");
                    pelo_elemento.unwrap_or(texto == "RegExp" || texto.ends_with(".RegExp")).then_some(arguments)
                }
                _ => None,
            };
            if let Some(args) = args {
                // `parent is ArgumentList`: o argumento nomeado tem o
                // `NamedExpression` no meio.
                isentos.extend(args.args.iter().filter(|x| x.name.is_none()).map(|x| x.value));
            }
        }
        for (k, e) in a.exprs.iter().enumerate() {
            if !matches!(e.kind, ExprKind::String(_)) || isentos.contains(&ExprId(k as u32)) {
                continue;
            }
            let lits = cordas::literais(fonte, e.span);
            for par in lits.windows(2) {
                let (atual, proximo) = (&par[0], &par[1]);
                // Os valores (com os escapes decodificados) do último trecho
                // do atual, do primeiro do próximo e de todo o atual.
                let valor = |l: &cordas::Literal, t: Option<&Span>| -> String {
                    let texto = t.and_then(|x| fonte.get(x.start..x.end)).unwrap_or("");
                    if l.crua { texto.to_string() } else { cordas::decodificar(texto) }
                };
                let fim = valor(atual, atual.trechos.last());
                let ini = valor(proximo, proximo.trechos.first());
                let brancos = [' ', '\n', '\r', '\t'];
                let termina = (atual.interpolado() && fim.is_empty()) || fim.ends_with(brancos);
                let comeca = (proximo.interpolado() && ini.is_empty()) || ini.starts_with(brancos);
                if termina || comeca {
                    continue;
                }
                let todo = atual.trechos.iter().map(|t| valor(atual, Some(t))).collect::<String>();
                if !todo.contains(brancos) {
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
    if ligada("unnecessary_library_name") && sem.is_some_and(|s| super::versao_ao_menos(s, 2, 19)) {
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
            // `toSource()` dos dois.
            let texto = |x: ExprId| dartforge_frontend::fonte::de_expr(a, fonte, interner, x);
            (texto(*operand) == texto(esquerda)).then_some(a.expr(e).span)
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
            let StmtKind::If { condition, then, else_: None, .. } = &s.kind else { continue };
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

    // `avoid_print`: a invocação cujo `methodName` resolve à função `print`
    // do `dart:core`, fora do ramo `then` de um `if (kDebugMode)` (do
    // `package:flutter/src/foundation/constants.dart`) no mesmo corpo de
    // função; e o argumento posicional que é o identificador `print` (o
    // tear-off) de qualquer invocação de método. Pede a semântica da
    // unidade.
    if ligada("avoid_print")
        && let Some(s) = sem
    {
        use dartforge_elements::model::Element;
        use dartforge_types::resolved::Resolved;
        let p = s.program;
        let e_print = |e: ExprId| match s.corpo.get_resolved(e) {
            Some(Resolved::Element(Element::Function(f))) => {
                let g = p.function(*f);
                g.class.is_none() && g.extension.is_none() && interner.resolve(g.name) == "print" && p.library(g.library).uri == "dart:core"
            }
            _ => false,
        };
        let e_kdebug = |e: ExprId| {
            let biblioteca = |l: dartforge_elements::model::LibraryId| p.library(l).uri == "package:flutter/src/foundation/constants.dart";
            match s.corpo.get_resolved(e) {
                Some(Resolved::Element(Element::Variable(v))) => interner.resolve(p.variable(*v).name) == "kDebugMode" && biblioteca(p.variable(*v).library),
                Some(Resolved::Element(Element::Function(f))) => interner.resolve(p.function(*f).name) == "kDebugMode" && biblioteca(p.function(*f).library),
                _ => false,
            }
        };
        // Os ramos `then` de `if (kDebugMode)`.
        let depuracao: Vec<Span> = a
            .stmts
            .iter()
            .filter_map(|st| match &st.kind {
                StmtKind::If { condition, then, .. } if matches!(a.expr(*condition).kind, ExprKind::Identifier(_)) && e_kdebug(*condition) => {
                    Some(a.stmt(*then).span)
                }
                _ => None,
            })
            .collect();
        // Sem corpo de função entre o ramo e a chamada.
        let so_depuracao = |x: Span| {
            depuracao.iter().any(|&r| {
                dentro(x, r)
                    && !a.functions.iter().any(|f| {
                        let corpo = match &f.body {
                            FunctionBody::Block(b) => a.stmt(*b).span,
                            FunctionBody::Expression(e) => a.expr(*e).span,
                            _ => return false,
                        };
                        dentro(x, corpo) && dentro(corpo, r)
                    })
            })
        };
        for (k, e) in a.exprs.iter().enumerate() {
            let ExprKind::Call { target, arguments } = &e.kind else { continue };
            // `MethodInvocation`: o alvo é o nome (com ou sem prefixo), e não
            // uma criação de instância.
            if matches!(s.corpo.get_resolved(ExprId(k as u32)), Some(Resolved::Constructor(_))) {
                continue;
            }
            let nome = match &a.expr(*target).kind {
                ExprKind::Identifier(n) => *n,
                ExprKind::Property { name, .. } => *name,
                _ => continue,
            };
            if e_print(*target) && !so_depuracao(e.span) {
                relatar(&c::AVOID_PRINT, nome.span, &[]);
            }
            for arg in arguments.args.iter().filter(|x| x.name.is_none()) {
                if let ExprKind::Identifier(n) = &a.expr(arg.value).kind
                    && e_print(arg.value)
                {
                    relatar(&c::AVOID_PRINT, n.span, &[]);
                }
            }
        }
    }

    // `avoid_catches_without_on_clauses`: o `catch` sem `on`, com o
    // parâmetro da exceção, cujo corpo não faz uso válido dela (o
    // `_ValidUseVisitor`, na ordem do texto): o `rethrow` vale enquanto
    // nenhum `catch` de dentro foi visitado; o `throw` que cita a exceção; a
    // chamada de tipo `Never`, o `Future.error(…)`, o `FlutterError.reportError(…)`
    // e o `completeError(…)` de um `Completer` que a citam nos argumentos (a
    // invocação de expressão de tipo `Never` não é visitada por dentro). Pede
    // a semântica da unidade.
    if ligada("avoid_catches_without_on_clauses")
        && let Some(s) = sem
    {
        use dartforge_elements::model::{ClassKind, Element};
        use dartforge_types::resolved::Resolved;
        use dartforge_types::table::Type;
        let p = s.program;
        let classe_de = |nome: &str, uri: &str| {
            p.classes.iter().position(|c| interner.resolve(c.name) == nome && p.library(c.library).uri == uri).map(|i| dartforge_elements::model::ClassId(i as u32))
        };
        let (futuro, completer) = (classe_de("Future", "dart:async"), classe_de("Completer", "dart:async"));
        let nunca = |e: ExprId| s.corpo.get_type(e).is_some_and(|t| matches!(s.table.get(t), Type::Never));
        // `extendsClass('Completer', 'dart.async')`: a classe ou uma
        // superclasse.
        let estende_completer = |e: ExprId| {
            let Some(Type::Interface { class, .. }) = s.corpo.get_type(e).map(|t| s.table.get(t)) else { return false };
            let mut atual = Some(*class);
            let mut passos = 0;
            while let Some(k) = atual {
                if Some(k) == completer {
                    return true;
                }
                passos += 1;
                if passos > 64 {
                    break;
                }
                atual = p.class(k).supertype_class;
            }
            false
        };
        for st in a.stmts.iter() {
            let StmtKind::Try { catches, .. } = &st.kind else { continue };
            for cc in catches.iter() {
                if cc.on_type.is_some() {
                    continue;
                }
                let Some(ex) = cc.exception else { continue };
                let corpo = a.stmt(cc.body).span;
                let cita = |regiao: Span| {
                    a.exprs.iter().enumerate().any(|(j, e)| {
                        matches!(e.kind, ExprKind::Identifier(_)) && dentro(e.span, regiao) && s.corpo.declaracao_local(ExprId(j as u32)) == Some(ex.span.start)
                    })
                };
                // Os eventos do visitante, pela posição: (posição, ordem,
                // evento). 0: um `catch` de dentro; 1: `rethrow`; 2: uso
                // válido.
                let mut eventos: Vec<(usize, u8)> = Vec::new();
                let mut fora: Vec<Span> = Vec::new();
                for t in a.stmts.iter() {
                    if let StmtKind::Try { catches: cs, .. } = &t.kind {
                        for k in cs.iter().filter(|k| dentro(k.span, corpo)) {
                            eventos.push((k.span.start, 0));
                        }
                    }
                }
                for (j, e) in a.exprs.iter().enumerate().filter(|(_, e)| dentro(e.span, corpo)) {
                    let id = ExprId(j as u32);
                    match &e.kind {
                        ExprKind::Rethrow => eventos.push((e.span.start, 1)),
                        ExprKind::Throw(x) => {
                            if cita(a.expr(*x).span) {
                                eventos.push((e.span.start, 2));
                            }
                        }
                        ExprKind::InstanceCreation { constructor: Some(nome), arguments, .. } => {
                            let de_futuro = matches!(s.corpo.get_type(id).map(|t| s.table.get(t)), Some(Type::Interface { class, .. }) if Some(*class) == futuro);
                            if interner.resolve(nome.sym) == "error" && de_futuro && cita(arguments.span) {
                                eventos.push((e.span.start, 2));
                            }
                        }
                        ExprKind::Call { target, arguments } => {
                            let alvo = a.expr(*target);
                            // A criação sem `new`: `Future.error(…)`.
                            if let Some(Resolved::Constructor(f)) = s.corpo.get_resolved(id) {
                                let g = p.function(*f);
                                let de_futuro = matches!(s.corpo.get_type(id).map(|t| s.table.get(t)), Some(Type::Interface { class, .. }) if Some(*class) == futuro);
                                if interner.resolve(g.name) == "error" && de_futuro && cita(arguments.span) {
                                    eventos.push((e.span.start, 2));
                                }
                                continue;
                            }
                            match &alvo.kind {
                                // `MethodInvocation`.
                                ExprKind::Identifier(_) | ExprKind::Property { .. } => {
                                    let (nome, receptor) = match &alvo.kind {
                                        ExprKind::Property { target: r, name, .. } => (interner.resolve(name.sym), Some(*r)),
                                        ExprKind::Identifier(n) => (interner.resolve(n.sym), None),
                                        _ => unreachable!(),
                                    };
                                    let entrega = if nunca(id) {
                                        true
                                    } else if nome == "reportError" {
                                        receptor.is_some_and(|r| {
                                            matches!(a.expr(r).kind, ExprKind::Identifier(_))
                                                && matches!(s.corpo.get_resolved(r), Some(Resolved::Element(Element::Class(c)))
                                                    if p.class(*c).kind == ClassKind::Class && interner.resolve(p.class(*c).name) == "FlutterError")
                                        })
                                    } else if nome == "completeError" {
                                        receptor.is_some_and(estende_completer)
                                    } else {
                                        false
                                    };
                                    if entrega && cita(arguments.span) {
                                        eventos.push((e.span.start, 2));
                                    }
                                }
                                // `FunctionExpressionInvocation` de tipo `Never`.
                                _ => {
                                    if nunca(id) {
                                        if cita(arguments.span) {
                                            eventos.push((e.span.start, 2));
                                        }
                                        fora.push(Span { start: e.span.start + 1, end: e.span.end });
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                eventos.retain(|(x, _)| !fora.iter().any(|f| f.start <= *x && *x < f.end));
                eventos.sort();
                let mut pode_relancar = true;
                let mut valido = false;
                for (_, ev) in eventos {
                    match ev {
                        0 => pode_relancar = false,
                        1 => valido = pode_relancar,
                        _ => valido = true,
                    }
                }
                if !valido {
                    relatar(&c::AVOID_CATCHES_WITHOUT_ON_CLAUSES, Span { start: cc.span.start, end: cc.span.start + "catch".len() }, &[]);
                }
            }
        }
    }

    // `avoid_setters_without_getters`: o setter da classe, enum ou tipo de
    // extensão sem setter concreto herdado (`lookUpInheritedConcreteSetter`)
    // e sem getter na cadeia de implementação (`lookUpGetter`: a classe, os
    // mixins do último ao primeiro, a superclasse…). Sem a semântica, só a
    // declaração sem superclasse nem mixin.
    if ligada("avoid_setters_without_getters") {
        for d in a.decls.iter() {
            let (membros, sem_superclasse, de_enum, representacao): (&[_], bool, bool, Option<SymbolId>) = match &d.kind {
                DeclKind::Class(x) => (x.members.as_slice(), x.extends.is_none() && x.with.is_empty(), false, None),
                DeclKind::Enum(x) => (x.members.as_slice(), x.with.is_empty(), true, None),
                DeclKind::ExtensionType(x) => (x.members.as_slice(), true, false, Some(x.representation_name.sym)),
                _ => continue,
            };
            let classe = sem.and_then(|s| membros.first().and_then(|&m| super::classe_do_membro(s, m)).map(|c| (s, c)));
            if classe.is_none() && !sem_superclasse {
                continue;
            }
            for &m in membros {
                let MemberKind::Method(f) = &a.member(m).kind else { continue };
                let f = a.function(*f);
                if f.kind != FunctionKind::Setter {
                    continue;
                }
                let Some(n) = f.name else { continue };
                let tem_getter = match classe {
                    Some((s, c)) => super::busca_na_cadeia(s, interner, c, n.sym, false),
                    None => {
                        Some(n.sym) == representacao
                            || matches!(interner.resolve(n.sym), "hashCode" | "runtimeType")
                            || (de_enum && interner.resolve(n.sym) == "index")
                            || membros.iter().any(|&mm| match &a.member(mm).kind {
                                MemberKind::Field(v) => v.variables.iter().any(|x| x.name.sym == n.sym),
                                MemberKind::Method(g) => {
                                    let g = a.function(*g);
                                    g.kind == FunctionKind::Getter && g.name.is_some_and(|x| x.sym == n.sym)
                                }
                                MemberKind::Constructor(_) => false,
                            })
                    }
                };
                let tem_setter_herdado = classe.is_some_and(|(s, c)| super::busca_na_cadeia(s, interner, c, n.sym, true));
                if !tem_getter && !tem_setter_herdado {
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
}
