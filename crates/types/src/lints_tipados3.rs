//! Mais regras de lint que pedem os tipos estáticos e a resolução da
//! inferência (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos
//! emissores da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`).
//! Como em [`crate::lints_tipados`], devolvem achados neutros (a posição, o
//! nome único do código e os argumentos), que quem chama só emite com a
//! regra ligada.
//!
//! Aqui: `use_truncating_division`, `avoid_double_and_int_checks`,
//! `only_throw_errors`, `no_runtimeType_toString`, `avoid_dynamic_calls` e
//! `unrelated_type_equality_checks` (a forma de expressão).
//!
//! Diferenças conhecidas:
//! - `only_throw_errors`: o tipo parâmetro de tipo não é decidido (o
//!   original olha o limite); nada se relata para ele.
//! - `no_runtimeType_toString`: toda extensão fica de fora (o original só
//!   deixa de fora a que não estende uma classe concreta), e o
//!   `runtimeType` sem alvo é o que não resolve para local nem parâmetro.
//! - `unrelated_type_equality_checks`: o subtipo entre interfaces é pela
//!   classe (os argumentos de tipo só contam na mesma classe); dois
//!   parâmetros de tipo, records e `FutureOr` não se decidem; o padrão
//!   relacional (`== x` num `case`) não é olhado.
//! - `avoid_dynamic_calls`: a cascata não é olhada; a chamada de um membro
//!   cujo tipo é `dynamic` ou `Function` usa o tipo que a inferência gravou
//!   na expressão do alvo da chamada.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::lints_tipados::Achado;
use crate::resolve::OutlineTypes;
use crate::resolved::{Resolved, UnitBodyTypes};
use crate::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, Program, UnitId};
use dartforge_frontend::ast::{self, AssignOp, BinaryOp, DeclKind, ExprId, ExprKind, Initializer, MemberKind, StmtKind, StringPart, UnaryOp};
use dartforge_intern::Interner;

fn sem_parenteses(a: &ast::Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
        e = *x;
    }
    e
}

fn dentro(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

/// A classe `nome` do `dart:core`.
fn classe_do_core(program: &Program, interner: &Interner, nome: &str) -> Option<ClassId> {
    let lib = program.core?;
    let sym = interner.lookup(nome)?;
    match program.library(lib).declared.get(&sym)?.getter? {
        Element::Class(c) => Some(c),
        _ => None,
    }
}

/// Os achados das regras tipadas deste módulo na unidade `u`.
pub fn achados(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    corpo: &UnitBodyTypes,
    u: UnitId,
) -> Vec<Achado> {
    let mut out: Vec<Achado> = Vec::new();
    let unidade = program.unit(u);
    let a = &unidade.ast;
    let tipo = |e: ExprId| corpo.get_type(e).filter(|t| !core.is_unknown(table, *t));
    let da_classe = |t: TypeId, alvo: Option<ClassId>| matches!(table.get(t), Type::Interface { class, .. } if Some(*class) == alvo);
    let dinamico = |e: ExprId| tipo(e).is_some_and(|t| matches!(table.get(t), Type::Dynamic));
    let nome = |s: dartforge_intern::SymbolId| interner.resolve(s);

    // `use_truncating_division`: `(a / b).toInt()` com `a` e `b` `int`.
    for e in a.exprs.iter() {
        let ExprKind::Call { target, arguments } = &e.kind else { continue };
        if !arguments.args.is_empty() {
            continue;
        }
        let ExprKind::Property { target: alvo, name, .. } = &a.expr(*target).kind else { continue };
        if nome(name.sym) != "toInt" || !matches!(a.expr(*alvo).kind, ExprKind::Parenthesized(_)) {
            continue;
        }
        let ExprKind::Binary { op: BinaryOp::Div, left, right } = &a.expr(sem_parenteses(a, *alvo)).kind else { continue };
        let int = |x: ExprId| tipo(x).is_some_and(|t| da_classe(t, core.int_class));
        if int(*left) && int(*right) {
            out.push((e.span, "use_truncating_division", Vec::new()));
        }
    }

    // `avoid_double_and_int_checks`: `if (x is double) … else if (x is int)`.
    for s in a.stmts.iter() {
        let StmtKind::If { condition, else_: Some(senao), .. } = &s.kind else { continue };
        let StmtKind::If { condition: c2, .. } = &a.stmt(*senao).kind else { continue };
        let (ExprKind::Is { value: v1, ty: t1, negated: false }, ExprKind::Is { value: v2, ty: t2, negated: false }) =
            (&a.expr(*condition).kind, &a.expr(*c2).kind)
        else {
            continue;
        };
        let (ExprKind::Identifier(n1), ExprKind::Identifier(n2)) = (&a.expr(*v1).kind, &a.expr(*v2).kind) else { continue };
        if n1.sym != n2.sym {
            continue;
        }
        let local = matches!(corpo.get_resolved(*v1), Some(Resolved::Local(_) | Resolved::Parameter { .. }));
        let escrito = |t: ast::TypeId, n: &str| {
            matches!(&a.ty(t).kind, ast::TypeKind::Named { name, args } if args.is_empty() && name.len() == 1 && nome(name[0].sym) == n)
                && !a.ty(t).nullable
        };
        if local && escrito(*t1, "double") && escrito(*t2, "int") {
            out.push((a.expr(*c2).span, "avoid_double_and_int_checks", Vec::new()));
        }
    }

    // `only_throw_errors`.
    let (excecao, erro) = (classe_do_core(program, interner, "Exception"), classe_do_core(program, interner, "Error"));
    let implementa = |c: ClassId, alvo: Option<ClassId>| {
        alvo.is_some_and(|k| c == k || outline.hierarchy.get(c).is_some_and(|d| d.supertypes.contains_key(&k)))
    };
    for e in a.exprs.iter() {
        let ExprKind::Throw(x) = &e.kind else { continue };
        let lancado = a.expr(*x);
        let literal = matches!(
            lancado.kind,
            ExprKind::Int(_)
                | ExprKind::Double(_)
                | ExprKind::Bool(_)
                | ExprKind::Null
                | ExprKind::String(_)
                | ExprKind::Symbol(_)
                | ExprKind::List { .. }
                | ExprKind::SetOrMap { .. }
                | ExprKind::Record { .. }
        );
        let lancavel = match tipo(*x).map(|t| table.get(t)) {
            None | Some(Type::Dynamic) | Some(Type::Never) | Some(Type::TypeParameter { .. }) => true,
            Some(Type::Interface { class, .. }) => implementa(*class, excecao) || implementa(*class, erro),
            Some(_) => false,
        };
        if literal || !lancavel {
            out.push((lancado.span, "only_throw_errors", Vec::new()));
        }
    }

    // `no_runtimeType_toString`.
    {
        // As regiões em que a regra não age.
        let mut fora: Vec<Span> = Vec::new();
        for s in a.stmts.iter() {
            if matches!(s.kind, StmtKind::Assert { .. }) {
                fora.push(s.span);
            }
            if let StmtKind::Try { catches, .. } = &s.kind {
                fora.extend(catches.iter().map(|c| c.span));
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                fora.extend(k.initializers.iter().filter_map(|i| match i {
                    Initializer::Assert { span, .. } => Some(*span),
                    _ => None,
                }));
            }
        }
        for e in a.exprs.iter() {
            if matches!(e.kind, ExprKind::Throw(_)) {
                fora.push(e.span);
            }
        }
        for d in a.decls.iter() {
            match &d.kind {
                DeclKind::Mixin(_) | DeclKind::Extension(_) => fora.push(d.span),
                DeclKind::Class(x) if x.modifiers.abstract_ => fora.push(d.span),
                _ => {}
            }
        }
        let pula = |s: Span| fora.iter().any(|&r| dentro(s, r));
        // `this.runtimeType`, `super.runtimeType` ou o getter sem alvo.
        let e_runtime_type = |x: ExprId| match &a.expr(x).kind {
            ExprKind::Property { target, name, .. } => {
                nome(name.sym) == "runtimeType" && matches!(a.expr(*target).kind, ExprKind::This | ExprKind::Super)
            }
            ExprKind::Identifier(n) => {
                nome(n.sym) == "runtimeType" && !matches!(corpo.get_resolved(x), Some(Resolved::Local(_) | Resolved::Parameter { .. }))
            }
            _ => false,
        };
        for e in a.exprs.iter() {
            match &e.kind {
                ExprKind::String(lit) => {
                    for parte in lit.parts.iter() {
                        if let StringPart::Interpolation(x) = parte
                            && e_runtime_type(*x)
                            && !pula(a.expr(*x).span)
                        {
                            out.push((a.expr(*x).span, "no_runtimeType_toString", Vec::new()));
                        }
                    }
                }
                ExprKind::Call { target, .. } => {
                    if let ExprKind::Property { target: alvo, name, .. } = &a.expr(*target).kind
                        && nome(name.sym) == "toString"
                        && e_runtime_type(*alvo)
                        && !pula(e.span)
                    {
                        out.push((name.span, "no_runtimeType_toString", Vec::new()));
                    }
                }
                _ => {}
            }
        }
    }

    // `avoid_dynamic_calls`.
    {
        let funcao = |t: TypeId| matches!(table.get(t), Type::Dynamic) || da_classe(t, core.function_class);
        let como = |x: ExprId| matches!(a.expr(sem_parenteses(a, x)).kind, ExprKind::As { .. });
        let mut achados: Vec<Span> = Vec::new();
        // `_reportIfDynamic`.
        let se_dinamico = |x: ExprId, achados: &mut Vec<Span>| -> bool {
            if dinamico(x) && !como(x) {
                achados.push(a.expr(x).span);
                return true;
            }
            false
        };
        let permitido = |n: &str| matches!(n, "hashCode" | "noSuchMethod" | "runtimeType" | "toString");
        // Os alvos de chamada (não são acesso a propriedade).
        let mut chamados: std::collections::HashSet<ExprId> = std::collections::HashSet::new();
        for e in a.exprs.iter() {
            if let ExprKind::Call { target, .. } = &e.kind {
                chamados.insert(*target);
            }
        }
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            match &e.kind {
                ExprKind::Call { target, arguments } => match &a.expr(*target).kind {
                    // A chamada de método com alvo.
                    ExprKind::Property { target: alvo, name, .. } => {
                        if matches!(a.expr(*alvo).kind, ExprKind::CascadeTarget) {
                            continue;
                        }
                        let n = nome(name.sym);
                        let posicionais = arguments.args.iter().filter(|x| x.name.is_none()).count();
                        if (n == "noSuchMethod" && arguments.args.len() == 1 && posicionais == 1) || (n == "toString" && arguments.args.is_empty()) {
                            continue;
                        }
                        if !se_dinamico(*alvo, &mut achados) && tipo(*target).is_some_and(funcao) && !como(*target) {
                            achados.push(name.span);
                        }
                    }
                    // `f(…)`: o tipo do identificador.
                    ExprKind::Identifier(n) => {
                        if tipo(*target).is_some_and(funcao) {
                            achados.push(n.span);
                        }
                    }
                    // `(e)(…)`.
                    _ => {
                        if tipo(*target).is_some_and(funcao) && !como(*target) {
                            achados.push(a.expr(*target).span);
                        }
                    }
                },
                ExprKind::Property { target, name, .. } if !chamados.contains(&id) => {
                    if !permitido(nome(name.sym)) && !matches!(a.expr(*target).kind, ExprKind::CascadeTarget) {
                        se_dinamico(*target, &mut achados);
                    }
                }
                ExprKind::Index { target, .. } => {
                    if !matches!(a.expr(*target).kind, ExprKind::CascadeTarget) {
                        se_dinamico(*target, &mut achados);
                    }
                }
                ExprKind::Binary { op, left, .. } => {
                    let definivel = !matches!(op, BinaryOp::Eq | BinaryOp::NotEq | BinaryOp::And | BinaryOp::Or | BinaryOp::IfNull);
                    if definivel {
                        se_dinamico(*left, &mut achados);
                    }
                }
                ExprKind::Unary { op, operand } => {
                    if *op == UnaryOp::NullAssert {
                        continue;
                    }
                    if !se_dinamico(*operand, &mut achados)
                        && matches!(op, UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec)
                        && dinamico(*operand)
                    {
                        achados.push(e.span);
                    }
                }
                // A atribuição composta com leitura `dynamic` (não `??=`).
                ExprKind::Assign { op, target, .. } => {
                    if !matches!(op, AssignOp::Assign | AssignOp::Compound(BinaryOp::IfNull)) && dinamico(*target) {
                        achados.push(e.span);
                    }
                }
                _ => {}
            }
        }
        achados.sort_by_key(|s| (s.start, s.end));
        achados.dedup();
        out.extend(achados.into_iter().map(|s| (s, "avoid_dynamic_calls", Vec::new())));
    }

    // `unrelated_type_equality_checks`, na expressão `a == b` / `a != b`.
    for e in a.exprs.iter() {
        let ExprKind::Binary { op: BinaryOp::Eq | BinaryOp::NotEq, left, right } = &e.kind else { continue };
        if matches!(a.expr(*left).kind, ExprKind::Null) || matches!(a.expr(*right).kind, ExprKind::Null) {
            continue;
        }
        let (Some(te), Some(td)) = (tipo(*left), tipo(*right)) else { continue };
        if !tipos_sem_relacao(program, interner, table, core, outline, te, td) {
            continue;
        }
        // `Int32`/`Int64` do `package:fixnum` contra `int`.
        let fixnum = match table.get(te) {
            Type::Interface { class, .. } => {
                let c = program.class(*class);
                matches!(interner.resolve(c.name), "Int32" | "Int64") && program.library(c.library).uri.starts_with("package:fixnum/")
            }
            _ => false,
        };
        if fixnum && da_classe(td, core.int_class) {
            continue;
        }
        // O token do operador: o primeiro não branco depois do operando.
        let fonte = unidade.source.as_bytes();
        let mut i = a.expr(*left).span.end;
        while i < fonte.len() && fonte[i].is_ascii_whitespace() {
            i += 1;
        }
        let operador = Span { start: i, end: i + 2 };
        let args = vec![
            crate::despejo::formatar(table, td, interner, program),
            crate::despejo::formatar(table, te, interner, program),
        ];
        out.push((operador, "unrelated_type_equality_checks_in_expression", args));
    }

    out
}

/// `typesAreUnrelated` do linter (`util/dart_type_utilities.dart`), com os
/// tipos já sem a nulidade (`promoteToNonNull`).
fn tipos_sem_relacao(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    e: TypeId,
    d: TypeId,
) -> bool {
    let (te, td) = (table.get(e), table.get(d));
    let fundo_ou_dinamico = |t: &Type| matches!(t, Type::Dynamic | Type::Never);
    if fundo_ou_dinamico(te) || fundo_ou_dinamico(td) {
        return false;
    }
    let supertipos_de = |c: ClassId| outline.hierarchy.get(c).map(|h| h.supertypes.keys().copied().collect::<Vec<_>>()).unwrap_or_default();
    // A classe com `call` concreto (o tipo de função se relaciona com ela).
    let tem_call = |c: ClassId| interner.lookup("call").is_some_and(|s| program.class(c).instance_members.contains_key(&s));
    match (te, td) {
        (Type::Interface { class: ce, args: ae, .. }, Type::Interface { class: cd, args: ad, .. }) => {
            if ce == cd {
                // A mesma classe: os argumentos de tipo, par a par.
                if ae.len() != ad.len() {
                    return false;
                }
                return ae.iter().zip(ad.iter()).any(|(x, y)| tipos_sem_relacao(program, interner, table, core, outline, *x, *y));
            }
            // Subtipo pela classe, nos dois sentidos.
            if supertipos_de(*ce).contains(cd) || supertipos_de(*cd).contains(ce) {
                return false;
            }
            // `interfaceTypesAreUnrelated`, elementos diferentes.
            let superclasse = |c: ClassId| outline.classes.get(c.0 as usize).and_then(|x| x.supertype);
            let mesmas = superclasse(*ce) == superclasse(*cd);
            let e_enum = program.class(*ce).kind == dartforge_elements::model::ClassKind::Enum;
            if mesmas && e_enum {
                return true;
            }
            let de_object = superclasse(*ce).is_some_and(|s| matches!(table.get(s), Type::Interface { class, .. } if Some(*class) == core.object_class));
            de_object || !mesmas
        }
        (Type::Function { .. }, outro) | (outro, Type::Function { .. }) => match outro {
            Type::Function { .. } => false,
            Type::Interface { class, .. } => {
                // `Object` e `Function` são supertipos de todo tipo de função.
                !(Some(*class) == core.object_class || Some(*class) == core.function_class || tem_call(*class))
            }
            _ => true,
        },
        _ => false,
    }
}
