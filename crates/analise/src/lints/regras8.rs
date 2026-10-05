//! O oitavo lote de regras de lint que só olham a árvore e o texto
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//! `prefer_conditional_assignment`, `join_return_with_assignment`,
//! `literal_only_boolean_expressions`,
//! `avoid_unused_constructor_parameters`, `unnecessary_const`,
//! `avoid_init_to_null`, `type_init_formals`,
//! `always_put_control_body_on_new_line` e `directives_ordering`.
//!
//! Diferenças conhecidas:
//! - Onde o original compara elementos (`canonicalElementsAreEqual`, o uso
//!   de um parâmetro), aqui se comparam nomes: um nome local que sombreia
//!   outro no meio do caminho não é visto.
//! - `avoid_init_to_null` decide "anulável" pelo tipo escrito (`T?`,
//!   `dynamic`, `Null`, `void`, `FutureOr` de anulável, ou nenhum tipo); um
//!   alias de tipo anulável não é visto, e o parâmetro `super.x` fica fora.
//!   O `this.x` sem tipo usa o tipo escrito do campo.
//! - `type_init_formals` compara o texto do tipo do parâmetro com o do
//!   campo da mesma declaração (iguais, o tipo é o mesmo; diferentes, pode
//!   ser o mesmo por alias, e não se relata). O `super.x` fica fora.
//! - `unnecessary_const`: o contexto constante é o do `constantContext` do
//!   analyzer, andando pelos ancestrais; o `for` com variável `const` não é
//!   contexto aqui.
//! - `directives_ordering` não olha os imports de documentação.
//! Escrito sem compilar nem executar (2026-10-05).

use super::andar::{andar, No};
use super::codigos_g as c;
use super::cordas::literais;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, Ast, BinaryOp, CollectionElement, CreationKeyword, DeclKind, DirectiveKind, ExprId, ExprKind, ForInit, MemberId, MemberKind,
    PatternKind, StmtId, StmtKind, TypeKind, UnaryOp, VariableList,
};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};

fn sem_parenteses(a: &Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
        e = *x;
    }
    e
}

fn nulo(a: &Ast, e: ExprId) -> bool {
    matches!(a.expr(sem_parenteses(a, e)).kind, ExprKind::Null)
}

/// `canonicalElementsFromIdentifiersAreEqual`, pelos nomes: `x` com `x`,
/// `a.b` com `a.b` (em qualquer profundidade).
fn mesmo_identificador(a: &Ast, x: ExprId, y: ExprId) -> bool {
    let (x, y) = (sem_parenteses(a, x), sem_parenteses(a, y));
    match (&a.expr(x).kind, &a.expr(y).kind) {
        (ExprKind::Identifier(m), ExprKind::Identifier(n)) => m.sym == n.sym,
        (ExprKind::Property { target: t1, name: n1, .. }, ExprKind::Property { target: t2, name: n2, .. }) => {
            n1.sym == n2.sym && mesmo_identificador(a, *t1, *t2)
        }
        _ => false,
    }
}

/// `_onlyLiterals`.
fn so_literais(a: &Ast, e: Option<ExprId>) -> bool {
    let Some(e) = e else { return false };
    match &a.expr(sem_parenteses(a, e)).kind {
        ExprKind::Int(_)
        | ExprKind::Double(_)
        | ExprKind::Bool(_)
        | ExprKind::Null
        | ExprKind::String(_)
        | ExprKind::Symbol(_)
        | ExprKind::List { .. }
        | ExprKind::SetOrMap { .. }
        | ExprKind::Record { .. } => true,
        ExprKind::Unary { op: UnaryOp::Neg | UnaryOp::Not | UnaryOp::BitNot | UnaryOp::PrefixInc | UnaryOp::PrefixDec, operand } => {
            so_literais(a, Some(*operand))
        }
        ExprKind::Binary { op: BinaryOp::IfNull, left, .. } => so_literais(a, Some(*left)),
        ExprKind::Binary { left, right, .. } => so_literais(a, Some(*left)) && so_literais(a, Some(*right)),
        _ => false,
    }
}

/// O tipo escrito é anulável (`TypeSystem.isNullable`, pela forma).
fn anulavel(a: &Ast, t: ast::TypeId, interner: &Interner) -> bool {
    let ty = a.ty(t);
    if ty.nullable {
        return true;
    }
    match &ty.kind {
        TypeKind::Void => true,
        TypeKind::Named { name, args } => match name.last().map(|n| interner.resolve(n.sym)) {
            Some("dynamic" | "Null") => name.len() == 1,
            Some("FutureOr") => args.len() == 1 && anulavel(a, args[0], interner),
            _ => false,
        },
        _ => false,
    }
}

/// As condições `when` das coleções (o `if (x case p when c)` elemento).
fn guardas_de_elemento(el: &CollectionElement, saida: &mut Vec<ExprId>) {
    match el {
        CollectionElement::If { guard, then, else_, .. } => {
            saida.extend(guard.iter().copied());
            guardas_de_elemento(then, saida);
            if let Some(x) = else_ {
                guardas_de_elemento(x, saida);
            }
        }
        CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => guardas_de_elemento(body, saida),
        _ => {}
    }
}

/// O primeiro token a partir de `inicio` (o `beginToken` de um comando).
fn primeiro_token(fonte: &str, inicio: usize) -> Span {
    let b = fonte.as_bytes();
    let de_palavra = |c: u8| c == b'_' || c == b'$' || c.is_ascii_alphanumeric();
    let mut fim = inicio;
    match b.get(inicio).copied() {
        Some(c) if de_palavra(c) => {
            while b.get(fim).copied().is_some_and(de_palavra) {
                fim += 1;
            }
        }
        Some(b'\'' | b'"') => {
            fim = literais(fonte, Span { start: inicio, end: fonte.len() }).first().map_or(inicio + 1, |l| l.span.end);
        }
        Some(c @ (b'+' | b'-')) if b.get(inicio + 1) == Some(&c) => fim = inicio + 2,
        Some(_) => fim = inicio + fonte[inicio..].chars().next().map_or(1, char::len_utf8),
        None => {}
    }
    Span { start: inicio, end: fim }
}

/// `compareDirectives`: pelo pacote (até a primeira `/`) e depois pelo resto.
fn comparar_diretivas(x: &str, y: &str) -> std::cmp::Ordering {
    if (!x.starts_with("package:") || !y.starts_with("package:")) && !x.starts_with('/') && !y.starts_with('/') {
        return x.cmp(y);
    }
    let (Some(i), Some(j)) = (x.find('/'), y.find('/')) else { return x.cmp(y) };
    x[..i].cmp(&y[..j]).then_with(|| x[i + 1..].cmp(&y[j + 1..]))
}

/// O contexto constante de um nó (`Expression.constantContext`, sem o
/// próprio): anotação, argumentos de constante de enum, criação ou literal
/// `const`, padrão constante com `const`, lista de variáveis `const`.
fn em_contexto_constante(a: &Ast, fonte: &str, pilha: &[No]) -> bool {
    for no in pilha.iter().rev() {
        match no {
            No::Anotacao(_) | No::ArgumentosDeEnum(_) => return true,
            No::Expr(e) => {
                if matches!(
                    a.expr(*e).kind,
                    ExprKind::InstanceCreation { keyword: Some(CreationKeyword::Const), .. }
                        | ExprKind::List { const_: true, .. }
                        | ExprKind::SetOrMap { const_: true, .. }
                        | ExprKind::Record { const_: true, .. }
                ) {
                    return true;
                }
            }
            No::Padrao(p) => {
                let n = a.pattern(*p);
                return matches!(n.kind, PatternKind::Constant(_)) && fonte.get(n.span.start..).is_some_and(|t| t.starts_with("const"));
            }
            No::Stmt(s) => return matches!(&a.stmt(*s).kind, StmtKind::Variables(l) if l.const_),
            No::Decl(d) => return matches!(&a.decl(*d).kind, DeclKind::Variables(l) if l.const_),
            No::Membro(m) => return matches!(&a.member(*m).kind, MemberKind::Field(l) if l.const_),
            No::Funcao(_) => return false,
        }
    }
    false
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, _sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let a = u.ast;
    let fonte = u.fonte;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
    // O texto de um trecho sem os brancos.
    let compacto = |s: Span| -> String { fonte.get(s.start..s.end).unwrap_or("").chars().filter(|x| !x.is_whitespace()).collect() };
    // O número da linha (a partir de 0) de um lugar.
    let mut comecos: Vec<usize> = vec![0];
    comecos.extend(fonte.bytes().enumerate().filter(|(_, b)| *b == b'\n').map(|(i, _)| i + 1));
    let linha_de = |lugar: usize| comecos.partition_point(|&x| x <= lugar);

    // `prefer_conditional_assignment`: `if (x == null) x = v;`.
    if ligada("prefer_conditional_assignment") {
        fn atribui(a: &Ast, s: StmtId, testada: ExprId) -> bool {
            match &a.stmt(s).kind {
                StmtKind::Expression(e) => matches!(&a.expr(*e).kind, ExprKind::Assign { target, .. } if mesmo_identificador(a, *target, testada)),
                StmtKind::Block(ss) if ss.len() == 1 => atribui(a, ss[0], testada),
                _ => false,
            }
        }
        for s in a.stmts.iter() {
            let StmtKind::If { condition, case_pattern: None, then, else_: None, .. } = &s.kind else { continue };
            let ExprKind::Binary { op: BinaryOp::Eq, left, right } = &a.expr(sem_parenteses(a, *condition)).kind else { continue };
            let testada = if nulo(a, *right) {
                *left
            } else if nulo(a, *left) {
                *right
            } else {
                continue;
            };
            if atribui(a, *then, testada) {
                relatar(&c::PREFER_CONDITIONAL_ASSIGNMENT, s.span, &[]);
            }
        }
    }
    // `join_return_with_assignment`: `x = v; return x;` no fim de um bloco.
    if ligada("join_return_with_assignment") {
        // O alvo da atribuição ou do `++`/`--`/prefixo de um comando.
        let alvo = |s: StmtId| -> Option<ExprId> {
            let StmtKind::Expression(e) = &a.stmt(s).kind else { return None };
            match &a.expr(sem_parenteses(a, *e)).kind {
                ExprKind::Assign { target, .. } => Some(*target),
                ExprKind::Unary { operand, .. } => Some(*operand),
                _ => None,
            }
        };
        for s in a.stmts.iter() {
            let StmtKind::Block(ss) = &s.kind else { continue };
            let n = ss.len();
            if n < 2 {
                continue;
            }
            let StmtKind::Return(Some(devolvida)) = &a.stmt(ss[n - 1]).kind else { continue };
            let Some(penultimo) = alvo(ss[n - 2]) else { continue };
            let antepenultimo = if n >= 3 { alvo(ss[n - 3]) } else { None };
            if !antepenultimo.is_some_and(|x| mesmo_identificador(a, penultimo, x)) && mesmo_identificador(a, *devolvida, penultimo) {
                relatar(&c::JOIN_RETURN_WITH_ASSIGNMENT, a.stmt(ss[n - 2]).span, &[]);
            }
        }
    }
    // `literal_only_boolean_expressions`: `do`, `for`, `if`, `while` (menos
    // `while (true)`) e as cláusulas `when`.
    if ligada("literal_only_boolean_expressions") {
        let mut guardas: Vec<ExprId> = Vec::new();
        for s in a.stmts.iter() {
            let relata = match &s.kind {
                StmtKind::DoWhile { condition, .. } => so_literais(a, Some(*condition)),
                StmtKind::For { condition, .. } => so_literais(a, *condition),
                StmtKind::If { condition, case_pattern, guard, .. } => {
                    guardas.extend(guard.iter().copied());
                    case_pattern.is_none() && so_literais(a, Some(*condition))
                }
                StmtKind::While { condition, .. } => !matches!(a.expr(*condition).kind, ExprKind::Bool(true)) && so_literais(a, Some(*condition)),
                StmtKind::Switch { cases, .. } => {
                    guardas.extend(cases.iter().filter_map(|k| k.guard));
                    false
                }
                _ => false,
            };
            if relata {
                relatar(&c::LITERAL_ONLY_BOOLEAN_EXPRESSIONS, s.span, &[]);
            }
        }
        for e in a.exprs.iter() {
            match &e.kind {
                ExprKind::Switch { cases, .. } => guardas.extend(cases.iter().filter_map(|k| k.guard)),
                ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                    for el in elements.iter() {
                        guardas_de_elemento(el, &mut guardas);
                    }
                }
                _ => {}
            }
        }
        for g in guardas {
            if so_literais(a, Some(g)) {
                let s = a.expr(g).span;
                let inicio = fonte.get(..s.start).and_then(|t| t.rfind("when")).unwrap_or(s.start);
                relatar(&c::LITERAL_ONLY_BOOLEAN_EXPRESSIONS, Span { start: inicio, end: s.end }, &[]);
            }
        }
    }

    // Por declaração, os campos (nome → tipo escrito) e os construtores.
    let por_declaracao: Vec<(HashMap<SymbolId, Option<ast::TypeId>>, Vec<MemberId>)> = a
        .decls
        .iter()
        .filter_map(|d| {
            let membros: &[MemberId] = match &d.kind {
                DeclKind::Class(x) => &x.members,
                DeclKind::Enum(x) => &x.members,
                DeclKind::ExtensionType(x) => &x.members,
                _ => return None,
            };
            let mut campos = HashMap::new();
            let mut construtores = Vec::new();
            for &m in membros {
                match &a.member(m).kind {
                    MemberKind::Field(l) => {
                        for v in l.variables.iter() {
                            campos.insert(v.name.sym, l.ty);
                        }
                    }
                    MemberKind::Constructor(_) => construtores.push(m),
                    MemberKind::Method(_) => {}
                }
            }
            Some((campos, construtores))
        })
        .collect();

    // `type_init_formals`: `this.x` com o tipo do campo repetido.
    if ligada("type_init_formals") {
        for (campos, construtores) in por_declaracao.iter() {
            for &m in construtores {
                let MemberKind::Constructor(k) = &a.member(m).kind else { continue };
                for p in k.parameters.iter().filter(|p| p.this_ && p.function_parameters.is_none()) {
                    if let (Some(t), Some(n)) = (p.ty, p.name)
                        && let Some(Some(do_campo)) = campos.get(&n.sym)
                        && compacto(a.ty(t).span) == compacto(a.ty(*do_campo).span)
                    {
                        relatar(&c::TYPE_INIT_FORMALS, a.ty(t).span, &[]);
                    }
                }
            }
        }
    }
    // `avoid_init_to_null`: variável não `final`/`const` e parâmetro
    // opcional de tipo anulável iniciados com `null`.
    if ligada("avoid_init_to_null") {
        let mut da_lista = |l: &VariableList| {
            if l.final_ || l.const_ || !l.ty.is_none_or(|t| anulavel(a, t, interner)) {
                return;
            }
            for v in l.variables.iter() {
                if let Some(i) = v.initializer
                    && nulo(a, i)
                {
                    relatar(&c::AVOID_INIT_TO_NULL, Span { start: v.name.span.start, end: a.expr(i).span.end }, &[]);
                }
            }
        };
        for d in a.decls.iter() {
            if let DeclKind::Variables(l) = &d.kind {
                da_lista(l);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Field(l) = &m.kind {
                da_lista(l);
            }
        }
        for s in a.stmts.iter() {
            match &s.kind {
                StmtKind::Variables(l) | StmtKind::For { init: Some(ForInit::Variables(l)), .. } => da_lista(l),
                _ => {}
            }
        }
        // Os parâmetros: de função (todas as listas) e de construtor (com
        // os campos da declaração para o `this.x` sem tipo).
        fn do_parametro(
            a: &Ast,
            interner: &Interner,
            p: &ast::Parameter,
            campos: Option<&HashMap<SymbolId, Option<ast::TypeId>>>,
            saida: &mut Vec<Span>,
        ) {
            if let Some(internos) = &p.function_parameters {
                for x in internos.iter() {
                    do_parametro(a, interner, x, None, saida);
                }
            }
            let Some(padrao) = p.default_value else { return };
            if !nulo(a, padrao) || p.super_ {
                return;
            }
            let e_anulavel = if p.function_parameters.is_some() {
                p.function_nullable
            } else if let Some(t) = p.ty {
                anulavel(a, t, interner)
            } else if p.this_ {
                p.name.and_then(|n| campos?.get(&n.sym).copied().flatten()).is_some_and(|t| anulavel(a, t, interner))
            } else {
                true
            };
            if e_anulavel {
                saida.push(p.span);
            }
        }
        let mut achados: Vec<Span> = Vec::new();
        for f in a.functions.iter() {
            for p in f.parameters.iter().flat_map(|ps| ps.iter()) {
                do_parametro(a, interner, p, None, &mut achados);
            }
        }
        let mut com_declaracao: HashSet<MemberId> = HashSet::new();
        for (campos, construtores) in por_declaracao.iter() {
            for &m in construtores {
                com_declaracao.insert(m);
                if let MemberKind::Constructor(k) = &a.member(m).kind {
                    for p in k.parameters.iter() {
                        do_parametro(a, interner, p, Some(campos), &mut achados);
                    }
                }
            }
        }
        for (i, m) in a.members.iter().enumerate() {
            if let MemberKind::Constructor(k) = &m.kind
                && !com_declaracao.contains(&MemberId(i as u32))
            {
                for p in k.parameters.iter() {
                    do_parametro(a, interner, p, None, &mut achados);
                }
            }
        }
        for s in achados {
            relatar(&c::AVOID_INIT_TO_NULL, s, &[]);
        }
    }
    // `avoid_unused_constructor_parameters`.
    if ligada("avoid_unused_constructor_parameters") {
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
        let primarios: HashSet<MemberId> = a
            .decls
            .iter()
            .filter_map(|d| match &d.kind {
                DeclKind::Class(x) => x.primary_constructor,
                DeclKind::Enum(x) => x.primary_constructor,
                _ => None,
            })
            .collect();
        for (i, m) in a.members.iter().enumerate() {
            let MemberKind::Constructor(k) = &m.kind else { continue };
            if m.augment || k.redirect.is_some() || k.external || k.parte_primaria || primarios.contains(&MemberId(i as u32)) {
                continue;
            }
            // O corpo e os inicializadores vêm depois da lista de parâmetros.
            let depois = k.parameters.last().map_or(m.span.start, |p| p.span.end);
            let (de, ate) = (usos.partition_point(|x| x.0 < depois), usos.partition_point(|x| x.0 < m.span.end));
            let usados: HashSet<SymbolId> = usos[de..ate.max(de)].iter().map(|x| x.1).collect();
            for p in k.parameters.iter().filter(|p| !p.this_ && !p.super_) {
                let Some(n) = p.name else { continue };
                let texto = interner.resolve(n.sym);
                let depreciado = p
                    .metadata
                    .iter()
                    .any(|x| x.name.last().is_some_and(|z| matches!(interner.resolve(z.sym), "deprecated" | "Deprecated")));
                if depreciado || texto.bytes().all(|b| b == b'_') || usados.contains(&n.sym) {
                    continue;
                }
                relatar(&c::AVOID_UNUSED_CONSTRUCTOR_PARAMETERS, p.span, &[texto]);
            }
        }
    }
    // `always_put_control_body_on_new_line`.
    if ligada("always_put_control_body_on_new_line") {
        // O corpo não começa na linha em que o controle termina.
        let mut conferir = |corpo: StmtId, fim_do_controle: usize| {
            let n = a.stmt(corpo);
            let primeiro = match &n.kind {
                StmtKind::Block(ss) => match ss.first() {
                    Some(&x) => a.stmt(x).span.start,
                    None => return,
                },
                _ => n.span.start,
            };
            if linha_de(fim_do_controle) == linha_de(primeiro) {
                relatar(&c::ALWAYS_PUT_CONTROL_BODY_ON_NEW_LINE, primeiro_token(fonte, n.span.start), &[]);
            }
        };
        // O fim do `)` que fecha o cabeçalho, antes do corpo.
        let fecha = |inicio: usize, corpo: StmtId| -> Option<usize> {
            let ate = a.stmt(corpo).span.start;
            fonte.get(inicio..ate).and_then(|t| t.rfind(')')).map(|i| inicio + i + 1)
        };
        for s in a.stmts.iter() {
            match &s.kind {
                StmtKind::DoWhile { body, .. } => conferir(*body, s.span.start + 2),
                StmtKind::For { body, .. } | StmtKind::ForIn { body, .. } | StmtKind::While { body, .. } => {
                    if let Some(f) = fecha(s.span.start, *body) {
                        conferir(*body, f);
                    }
                }
                StmtKind::If { then, else_, .. } => {
                    if let Some(f) = fecha(s.span.start, *then) {
                        conferir(*then, f);
                    }
                    if let Some(senao) = else_
                        && !matches!(a.stmt(*senao).kind, StmtKind::If { .. })
                        && let Some(palavra) = fonte.get(a.stmt(*then).span.end..a.stmt(*senao).span.start).and_then(|t| t.rfind("else"))
                    {
                        conferir(*senao, a.stmt(*then).span.end + palavra + 4);
                    }
                }
                _ => {}
            }
        }
    }
    // `directives_ordering`.
    if ligada("directives_ordering") {
        // (índice da diretiva, é export, URI) dos imports e exports.
        let mut diretivas: Vec<(usize, bool, String)> = Vec::new();
        for (i, d) in u.unit.directives.iter().enumerate() {
            let (uri, exporta) = match &d.kind {
                DirectiveKind::Import { uri, .. } => (uri, false),
                DirectiveKind::Export { uri, .. } => (uri, true),
                _ => continue,
            };
            let texto = literais(fonte, uri.span).iter().map(|l| l.texto(fonte)).collect::<String>();
            diretivas.push((i, exporta, texto));
        }
        let mut relatadas: HashSet<usize> = HashSet::new();
        let mut relatar_uma = |i: usize, codigo: &'static CodigoLint, args: &[&str]| {
            if relatadas.insert(i) {
                relatar(codigo, u.unit.directives[i].span, args);
            }
        };
        let de_dart = |x: &&(usize, bool, String)| x.2.starts_with("dart:");
        let de_pacote = |x: &&(usize, bool, String)| x.2.starts_with("package:");
        let absoluta = |x: &&(usize, bool, String)| x.2.contains(':');
        for (exporta, palavra) in [(false, "import"), (true, "export")] {
            // As `dart:` vêm antes das outras.
            for x in diretivas.iter().filter(|x| x.1 == exporta).skip_while(de_dart).filter(de_dart) {
                relatar_uma(x.0, &c::DIRECTIVES_ORDERING_DART, &[palavra]);
            }
        }
        for (exporta, palavra) in [(false, "import"), (true, "export")] {
            // As `package:` vêm antes das relativas.
            for x in diretivas.iter().filter(|x| x.1 == exporta && !de_dart(x)).skip_while(absoluta).filter(de_pacote) {
                relatar_uma(x.0, &c::DIRECTIVES_ORDERING_PACKAGE_BEFORE_RELATIVE, &[palavra]);
            }
        }
        // Os exports ficam numa seção depois dos imports.
        let do_fim: Vec<usize> = (0..u.unit.directives.len())
            .rev()
            .skip_while(|&i| matches!(u.unit.directives[i].kind, DirectiveKind::Part { .. }))
            .skip_while(|&i| matches!(u.unit.directives[i].kind, DirectiveKind::Export { .. }))
            .filter(|&i| matches!(u.unit.directives[i].kind, DirectiveKind::Export { .. }))
            .collect();
        for i in do_fim {
            relatar_uma(i, &c::DIRECTIVES_ORDERING_EXPORTS, &[]);
        }
        // Cada seção em ordem: `dart:`, relativas e `package:`, imports e
        // depois exports.
        let relativa = |x: &&(usize, bool, String)| !x.2.contains(':');
        let secoes: [&dyn Fn(&&(usize, bool, String)) -> bool; 3] = [&de_dart, &relativa, &de_pacote];
        for secao in secoes {
            for exporta in [false, true] {
                let da_secao: Vec<&(usize, bool, String)> = diretivas.iter().filter(|x| x.1 == exporta).filter(|x| secao(x)).collect();
                for par in da_secao.windows(2) {
                    if comparar_diretivas(&par[0].2, &par[1].2) == std::cmp::Ordering::Greater {
                        relatar_uma(par[1].0, &c::DIRECTIVES_ORDERING_ALPHABETICAL, &[]);
                    }
                }
            }
        }
    }
    // `unnecessary_const`: `const` dentro de contexto constante.
    if ligada("unnecessary_const") {
        andar(u, &mut |no, pilha| {
            let No::Expr(e) = no else { return };
            let n = a.expr(e);
            let candidato = match &n.kind {
                ExprKind::InstanceCreation { keyword: Some(CreationKeyword::Const), .. } | ExprKind::Record { const_: true, .. } => true,
                ExprKind::List { const_: true, .. } | ExprKind::SetOrMap { const_: true, .. } => {
                    // O literal que é o próprio padrão constante fica fora.
                    let mut j = pilha.len();
                    while j > 0 && matches!(pilha[j - 1], No::Expr(x) if matches!(a.expr(x).kind, ExprKind::Parenthesized(_))) {
                        j -= 1;
                    }
                    !(j > 0 && matches!(pilha[j - 1], No::Padrao(_)))
                }
                _ => false,
            };
            if candidato && fonte.get(n.span.start..).is_some_and(|t| t.starts_with("const")) && em_contexto_constante(a, fonte, pilha) {
                relatar(&c::UNNECESSARY_CONST, Span { start: n.span.start, end: n.span.start + 5 }, &[]);
            }
        });
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;

    fn so(regra: &str, fonte: &str) -> Vec<String> {
        let mut nomes = Interner::new();
        let p = dartforge_frontend::parser::parse(fonte, &mut nomes);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte };
        let mut relatos = executar(u, &nomes, &|r| r == regra, None);
        relatos.sort_by_key(|r| (r.span.start, r.span.end));
        relatos.into_iter().map(|r| fonte[r.span.start..r.span.end].to_string()).collect()
    }

    #[test]
    fn atribuicoes() {
        assert_eq!(
            so("prefer_conditional_assignment", "void f(int? x, int? y) {\n  if (x == null) x = 1;\n  if (null == y) {\n    y = 2;\n  }\n  if (x == null) y = 3;\n}\n"),
            vec!["if (x == null) x = 1;".to_string(), "if (null == y) {\n    y = 2;\n  }".to_string()]
        );
        assert_eq!(
            so("join_return_with_assignment", "class A {\n  int x = 0;\n  int m() {\n    x = 1;\n    return x;\n  }\n  int n() {\n    x = 1;\n    x = 2;\n    return x;\n  }\n}\n"),
            vec!["x = 1;".to_string()]
        );
    }

    #[test]
    fn literais_em_condicoes() {
        let fonte = "void f(int x) {\n  if (true) {}\n  while (true) {}\n  while (1 > 2) {}\n  for (; 1 == 1;) {}\n  if (x > 0) {}\n  switch (x) {\n    case 1 when true:\n      break;\n  }\n}\n";
        assert_eq!(
            so("literal_only_boolean_expressions", fonte),
            vec!["if (true) {}".to_string(), "while (1 > 2) {}".to_string(), "for (; 1 == 1;) {}".to_string(), "when true".to_string()]
        );
    }

    #[test]
    fn construtores() {
        let fonte = "class A {\n  final int? a;\n  int b;\n  A(int? this.a, this.b, int c, int d, int _) : assert(d > 0);\n  A.n(int e) : a = null, b = 0;\n}\n";
        assert_eq!(so("type_init_formals", fonte), vec!["int?".to_string()]);
        assert_eq!(so("avoid_unused_constructor_parameters", fonte), vec!["int c".to_string(), "int e".to_string()]);
    }

    #[test]
    fn nulos_iniciais() {
        let fonte = "int? a = null;\nvar b = null;\nfinal int? c = null;\nint d = 0;\nclass A {\n  int? e;\n  A([this.e = null, int? f = null, int g = 0]);\n}\n";
        assert_eq!(
            so("avoid_init_to_null", fonte),
            vec!["a = null".to_string(), "b = null".to_string(), "this.e = null".to_string(), "int? f = null".to_string()]
        );
    }

    #[test]
    fn const_desnecessario() {
        let fonte = "class A {\n  const A([Object? x]);\n}\nconst a = const A();\nconst b = [const A(), const [1]];\nfinal c = const A(const A());\nfinal d = const A();\n@A(const A())\nvoid f([x = const A()]) {}\n";
        assert_eq!(so("unnecessary_const", fonte).len(), 5);
    }

    #[test]
    fn corpo_na_mesma_linha() {
        let fonte = "void f(bool x) {\n  if (x) return;\n  if (x) {\n    return;\n  } else return;\n  while (x) { f(x); }\n  do f(x); while (x);\n  if (x)\n    return;\n}\n";
        assert_eq!(
            so("always_put_control_body_on_new_line", fonte),
            vec!["return".to_string(), "return".to_string(), "{".to_string(), "f".to_string()]
        );
    }

    #[test]
    fn ordem_das_diretivas() {
        assert_eq!(comparar_diretivas("package:a/z.dart", "package:b/a.dart"), std::cmp::Ordering::Less);
        assert_eq!(comparar_diretivas("b.dart", "a.dart"), std::cmp::Ordering::Greater);
        let fonte = "import 'package:b/b.dart';\nimport 'dart:io';\nimport 'package:a/a.dart';\nexport 'x.dart';\nimport 'z.dart';\nimport 'y.dart';\n";
        assert_eq!(
            so("directives_ordering", fonte),
            vec![
                "import 'dart:io';".to_string(),
                "import 'package:a/a.dart';".to_string(),
                "export 'x.dart';".to_string(),
                "import 'y.dart';".to_string(),
            ]
        );
    }
}
