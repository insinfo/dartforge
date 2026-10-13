//! Mais regras de lint que pedem os tipos estáticos e a resolução da
//! inferência (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos
//! emissores da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`).
//! Como em [`crate::lints_tipados`], devolvem achados neutros (a posição, o
//! nome único do código e os argumentos), que quem chama só emite com a
//! regra ligada.
//!
//! Aqui: `avoid_bool_literals_in_conditional_expressions`,
//! `no_literal_bool_comparisons`, `unnecessary_string_interpolations`,
//! `prefer_contains` (os três códigos), `prefer_is_not_empty`,
//! `use_is_even_rather_than_modulo`, `unnecessary_to_list_in_spreads`,
//! `unnecessary_null_aware_assignments` e `await_only_futures`.
//!
//! Cada regra lê o mesmo dado do emissor:
//! - `getIntValue(e, context)` (`prefer_contains`): o literal inteiro ou o
//!   identificador simples avaliado como constante (o `Motor` de
//!   constantes, o `computeConstantValue`), com `-` na frente;
//! - o contexto constante é o `inConstantContext` (`dartforge_frontend::pais`);
//! - o `implementsAnyInterface` desce pelos limites do parâmetro de tipo;
//! - `prefer_is_not_empty` pergunta ao elemento que declara o `isEmpty`
//!   (classe, mixin, enum, tipo de extensão ou extensão) se ele tem um filho
//!   `isNotEmpty` (`getChildren`);
//! - `unnecessary_string_interpolations` é o literal único (não adjacente)
//!   com exatamente uma interpolação e os trechos de antes e de depois
//!   vazios;
//! - `unnecessary_null_aware_assignments` só cala quando a escrita vai para
//!   um setter (o `[]=` não é setter).
//! Escrito sem compilar nem executar (2026-10-05).

use crate::constantes::avaliador::{Constante, Ctx, Motor};
use crate::lints_tipados::Achado;
use crate::resolve::OutlineTypes;
use crate::resolved::{BodyTypes, MemberRef, Resolved};
use crate::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, AssignOp, BinaryOp, CollectionElement, ExprId, ExprKind, Initializer, MemberKind, StringPart, UnaryOp};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};

fn sem_parenteses(a: &ast::Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
        e = *x;
    }
    e
}

/// `IntegerLiteral.value`: o literal inteiro (decimal ou `0x`, até 64 bits
/// no hexadecimal). Um literal que o parser tenha juntado com o `-` vale
/// negativo.
fn integer_literal(a: &ast::Ast, source: &str, e: ExprId) -> Option<i64> {
    let ExprKind::Int(s) = &a.expr(e).kind else { return None };
    let text: String = source.get(s.start..s.end)?.chars().filter(|c| *c != '_' && !c.is_whitespace()).collect();
    let (negative, text) = match text.strip_prefix('-') {
        Some(r) => (true, r.to_string()),
        None => (false, text),
    };
    let v = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => u64::from_str_radix(hex, 16).ok().map(|v| v as i64),
        None => text.parse::<i64>().ok(),
    }?;
    Some(if negative { v.wrapping_neg() } else { v })
}

/// O literal escrito sem sinal (`right is IntegerLiteral`).
fn unsigned_literal(a: &ast::Ast, source: &str, e: ExprId) -> Option<i64> {
    let ExprKind::Int(s) = &a.expr(e).kind else { return None };
    if source.get(s.start..s.end).is_some_and(|t| t.starts_with('-')) {
        return None;
    }
    integer_literal(a, source, e)
}

/// `getIntValue(e, context)`: o literal ou o identificador constante, com
/// `-` na frente (só o `-`).
fn integer_value(a: &ast::Ast, source: &str, evaluated: &HashMap<ExprId, i64>, e: ExprId) -> Option<i64> {
    let base = |x: ExprId| match &a.expr(x).kind {
        ExprKind::Int(_) => integer_literal(a, source, x),
        ExprKind::Identifier(_) => evaluated.get(&x).copied(),
        _ => None,
    };
    match &a.expr(e).kind {
        ExprKind::Unary { op: UnaryOp::Neg, operand } => base(*operand).map(i64::wrapping_neg),
        _ => base(e),
    }
}

/// Os elementos `...x` de uma coleção, por dentro de `if` e `for`.
fn spread(el: &CollectionElement, output: &mut Vec<ExprId>) {
    match el {
        CollectionElement::Spread { value, .. } => output.push(*value),
        CollectionElement::If { then, else_, .. } => {
            spread(then, output);
            if let Some(x) = else_ {
                spread(x, output);
            }
        }
        CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => spread(body, output),
        _ => {}
    }
}

/// Os identificadores simples que o `getIntValue(…, context)` do
/// `prefer_contains` pode consultar: os operandos das comparações e o
/// segundo argumento de `indexOf`, também atrás de `-`.
fn queried_identifiers(interner: &Interner, a: &ast::Ast) -> Vec<ExprId> {
    let mut v = Vec::new();
    let mut consider = |e: ExprId| {
        let x = match &a.expr(e).kind {
            ExprKind::Unary { op: UnaryOp::Neg, operand } => *operand,
            _ => e,
        };
        if matches!(a.expr(x).kind, ExprKind::Identifier(_)) {
            v.push(x);
        }
    };
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::Binary { op: BinaryOp::Eq | BinaryOp::NotEq | BinaryOp::Gt | BinaryOp::GtEq | BinaryOp::Lt | BinaryOp::LtEq, left, right } => {
                consider(*left);
                consider(*right);
            }
            ExprKind::Call { target, arguments } if arguments.args.len() == 2 => {
                if let ExprKind::Property { name, .. } = &a.expr(*target).kind
                    && interner.resolve(name.sym) == "indexOf"
                {
                    consider(arguments.args[1].value);
                }
            }
            _ => {}
        }
    }
    v
}

/// Os filhos do elemento `getChildren(elemento, nome)` (pelo `displayName`:
/// o setter `nome=` também conta).
fn has_child(program: &Program, interner: &Interner, owner: Dono, the_name: &str) -> bool {
    let symbols: Vec<SymbolId> = [interner.lookup(the_name), interner.lookup(&format!("{the_name}_="))].into_iter().flatten().collect();
    let has = |m: &HashMap<SymbolId, dartforge_elements::model::FunctionElementId>| symbols.iter().any(|s| m.contains_key(s));
    match owner {
        Dono::Classe(c) => {
            let e = program.class(c);
            has(&e.instance_members)
                || has(&e.static_members)
                || e.fields.iter().chain(e.enum_constants.iter()).any(|v| interner.resolve(program.variable(*v).name) == the_name)
        }
        Dono::Extensao(x) => {
            let e = program.extension(x);
            has(&e.instance_members) || has(&e.static_members) || e.fields.iter().any(|v| interner.resolve(program.variable(*v).name) == the_name)
        }
    }
}

#[derive(Clone, Copy)]
enum Dono {
    Classe(ClassId),
    Extensao(dartforge_elements::model::ExtensionId),
}

/// O elemento que declara o membro resolvido (`enclosingElement3`).
fn member_owner(program: &Program, r: &Resolved) -> Option<Dono> {
    let (cls, extension) = match r {
        Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } => {
            let g = program.function(*f);
            (g.class, g.extension)
        }
        Resolved::Member { member: MemberRef::Variable(v), .. } => {
            let x = program.variable(*v);
            (x.class, x.extension)
        }
        _ => return None,
    };
    cls.map(Dono::Classe).or(extension.map(Dono::Extensao))
}

/// Os achados das regras tipadas deste módulo na unidade `u`.
#[allow(clippy::too_many_arguments)]
pub fn achados(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
    the_inferred: &HashSet<LibraryId>,
    u: UnitId,
) -> Vec<Achado> {
    let mut out: Vec<Achado> = Vec::new();
    let unit = program.unit(u);
    let a = &unit.ast;
    let source = unit.source.as_str();
    let Some(the_body) = bodies.units.get(u.0 as usize) else { return out };

    // `computeConstantValue` dos identificadores consultados, com o motor de
    // constantes (só se houver algum).
    let mut evaluated: HashMap<ExprId, i64> = HashMap::new();
    let queried = queried_identifiers(interner, a);
    if !queried.is_empty() {
        let mut engine = Motor::novo(program, interner, table, core, outline, bodies, the_inferred);
        let cx = Ctx::simples(u, unit.library);
        for e in queried {
            if let Constante::Valor(v) = engine.avaliar(&cx, e, false)
                && let crate::constantes::valor::Estado::Int(Some(i)) = v.estado
            {
                evaluated.insert(e, i);
            }
        }
    }
    let table: &TypeTable = table;

    let non_null = |t: TypeId, tgt: Option<ClassId>| matches!(table.get(t), Type::Interface { class, nullable: false, .. } if Some(*class) == tgt);
    // `implementsInterface`: a interface é `alvo` ou o tem entre os
    // supertipos.
    let implements = |t: TypeId, tgt: Option<ClassId>| match table.get(t) {
        Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => tgt.is_some_and(|k| {
            *class == k || (program.class(*class).decl.is_some() && outline.hierarchy.get(*class).is_some_and(|d| d.supertypes.contains_key(&k)))
        }),
        _ => false,
    };
    // `implementsAnyInterface`: pelo `typeForInterfaceCheck`.
    let implements_any = |t: TypeId, targets: &[Option<ClassId>]| {
        let t = crate::lints_tipados3::to_interface(table, core, t);
        targets.iter().any(|k| implements(t, *k))
    };
    let ty = |e: ExprId| the_body.get_type(e).filter(|t| !core.is_unknown(table, *t));
    let pais = crate::lints_tipados::pais_da_unidade(program, u);

    // Os `assert` da lista de inicializadores de construtor `const`.
    let mut asserts_const: Vec<Span> = Vec::new();
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind
            && k.const_
        {
            asserts_const.extend(k.initializers.iter().filter_map(|i| match i {
                Initializer::Assert { span, .. } => Some(*span),
                _ => None,
            }));
        }
    }

    // `_isUnassignedIndexOf`: `x.indexOf(v)` ou `x.indexOf(v, 0)` com `x`
    // um `Iterable` ou uma `String` (por dentro de parênteses e de `as`).
    let of_index = |e: ExprId| {
        let mut e = sem_parenteses(a, e);
        while let ExprKind::As { value, .. } = &a.expr(e).kind {
            e = *value;
        }
        let e = sem_parenteses(a, e);
        let ExprKind::Call { target, arguments } = &a.expr(e).kind else { return false };
        if matches!(the_body.get_resolved(e), Some(Resolved::Constructor(_))) {
            return false;
        }
        let ExprKind::Property { target: receiver, name, .. } = &a.expr(*target).kind else { return false };
        if interner.resolve(name.sym) != "indexOf" {
            return false;
        }
        let Some(t) = ty(*receiver) else { return false };
        if !implements_any(t, &[core.iterable_class, core.string_class]) {
            return false;
        }
        match &arguments.args[..] {
            [_, start] => integer_value(a, source, &evaluated, start.value) == Some(0),
            _ => true,
        }
    };

    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        match &e.kind {
            // `avoid_bool_literals_in_conditional_expressions`.
            ExprKind::Conditional { then, else_, .. } => {
                let (x, y) = (sem_parenteses(a, *then), sem_parenteses(a, *else_));
                let boolean = |z: ExprId| ty(z).is_some_and(|t| non_null(t, core.bool_class));
                if boolean(x) && boolean(y) && (matches!(a.expr(x).kind, ExprKind::Bool(_)) || matches!(a.expr(y).kind, ExprKind::Bool(_))) {
                    out.push((e.span, "avoid_bool_literals_in_conditional_expressions", Vec::new()));
                }
            }
            ExprKind::Binary { op, left, right } => {
                // `no_literal_bool_comparisons`.
                if matches!(op, BinaryOp::Eq | BinaryOp::NotEq) {
                    let literal = |z: ExprId| matches!(a.expr(z).kind, ExprKind::Bool(_));
                    let boolean = |z: ExprId| ty(z).is_some_and(|t| non_null(t, core.bool_class));
                    if literal(*right) && boolean(*left) {
                        out.push((a.expr(*right).span, "no_literal_bool_comparisons", Vec::new()));
                    } else if literal(*left) && boolean(*right) {
                        out.push((a.expr(*left).span, "no_literal_bool_comparisons", Vec::new()));
                    }
                }
                // `prefer_contains`.
                if matches!(op, BinaryOp::Eq | BinaryOp::NotEq | BinaryOp::Gt | BinaryOp::GtEq | BinaryOp::Lt | BinaryOp::LtEq) {
                    let comparison = match integer_value(a, source, &evaluated, *right) {
                        Some(v) => (v <= 0 && of_index(*left)).then_some((v, *op)),
                        None => integer_value(a, source, &evaluated, *left).filter(|v| *v <= 0 && of_index(*right)).map(|v| {
                            // `TokenType.inverted`.
                            let inverted = match op {
                                BinaryOp::Gt => BinaryOp::Lt,
                                BinaryOp::GtEq => BinaryOp::LtEq,
                                BinaryOp::Lt => BinaryOp::Gt,
                                BinaryOp::LtEq => BinaryOp::GtEq,
                                other => *other,
                            };
                            (v, inverted)
                        }),
                    };
                    let code = comparison.and_then(|(v, o)| match v {
                        -1 => match o {
                            BinaryOp::Eq | BinaryOp::NotEq | BinaryOp::LtEq | BinaryOp::Gt => Some("prefer_contains_use_contains"),
                            BinaryOp::Lt => Some("prefer_contains_always_false"),
                            BinaryOp::GtEq => Some("prefer_contains_always_true"),
                            _ => None,
                        },
                        0 => matches!(o, BinaryOp::GtEq | BinaryOp::Lt).then_some("prefer_contains_use_contains"),
                        _ => match o {
                            BinaryOp::Eq | BinaryOp::LtEq | BinaryOp::Lt => Some("prefer_contains_always_false"),
                            BinaryOp::NotEq | BinaryOp::GtEq | BinaryOp::Gt => Some("prefer_contains_always_true"),
                            _ => None,
                        },
                    });
                    if let Some(c) = code {
                        out.push((e.span, c, Vec::new()));
                    }
                }
                // `use_is_even_rather_than_modulo`: `x % 2 == n`, inteiros,
                // com `n` literal sem sinal.
                if *op == BinaryOp::Eq
                    && !pais.em_contexto_constante(a, id)
                    && let Some(the_value) = unsigned_literal(a, source, *right)
                    && let ExprKind::Binary { op: BinaryOp::Rem, right: divisor, .. } = &a.expr(*left).kind
                    && unsigned_literal(a, source, *divisor) == Some(2)
                    && ty(*left).is_some_and(|t| non_null(t, core.int_class))
                    && ty(*right).is_some_and(|t| non_null(t, core.int_class))
                    && ty(*divisor).is_some_and(|t| non_null(t, core.int_class))
                    && !asserts_const.iter().any(|k| k.start <= e.span.start && e.span.end <= k.end)
                {
                    out.push((e.span, "use_is_even_rather_than_modulo", vec![(if the_value == 0 { "isEven" } else { "isOdd" }).to_string()]));
                }
            }
            // `unnecessary_string_interpolations`: o literal único com uma
            // interpolação só e os trechos de antes e de depois vazios.
            ExprKind::String(lit) => {
                let interpolations: Vec<ExprId> = lit
                    .parts
                    .iter()
                    .filter_map(|p| match p {
                        StringPart::Interpolation(x) => Some(*x),
                        StringPart::Text(_) => None,
                    })
                    .collect();
                let [x] = interpolations.as_slice() else { continue };
                let x = *x;
                let Some(text) = source.get(e.span.start..e.span.end) else { continue };
                let Some(quotes) = ["'''", "\"\"\"", "'", "\""].into_iter().find(|q| text.starts_with(q)) else { continue };
                let inside = a.expr(x).span;
                let (Some(before), Some(after)) = (source.get(e.span.start + quotes.len()..inside.start), source.get(inside.end..e.span.end)) else {
                    continue;
                };
                let unique = if before == "$" {
                    after == quotes
                } else if let Some(rest) = before.strip_prefix("${") {
                    // Só brancos e comentários entre `${` e a expressão, e
                    // entre ela e o `}`; depois do `}`, só a aspa.
                    let b = rest.as_bytes();
                    let d = after.as_bytes();
                    let k = dartforge_frontend::fonte::pular_brancos(d, 0);
                    dartforge_frontend::fonte::pular_brancos(b, 0) == b.len() && d.get(k) == Some(&b'}') && &after[k + 1..] == quotes
                } else {
                    false
                };
                if unique && ty(x).is_some_and(|t| non_null(t, core.string_class)) {
                    out.push((e.span, "unnecessary_string_interpolations", Vec::new()));
                }
            }
            // `prefer_is_not_empty`: `!x.isEmpty`, com o elemento que declara
            // o `isEmpty` tendo um filho `isNotEmpty`.
            ExprKind::Unary { op: UnaryOp::Not, operand } => {
                let tgt = sem_parenteses(a, *operand);
                if let ExprKind::Property { target, name, .. } = &a.expr(tgt).kind
                    && !matches!(a.expr(*target).kind, ExprKind::CascadeTarget)
                    && interner.resolve(name.sym) == "isEmpty"
                    && let Some(r) = the_body.get_resolved(tgt)
                    && let Some(owner) = member_owner(program, r)
                    && has_child(program, interner, owner, "isNotEmpty")
                {
                    out.push((e.span, "prefer_is_not_empty", Vec::new()));
                }
            }
            // `unnecessary_to_list_in_spreads`.
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                let mut values = Vec::new();
                for el in elements.iter() {
                    spread(el, &mut values);
                }
                for v in values {
                    if let ExprKind::Call { target, .. } = &a.expr(v).kind
                        && let ExprKind::Property { target: receiver, name, .. } = &a.expr(*target).kind
                        && interner.resolve(name.sym) == "toList"
                        && ty(*receiver).is_some_and(|t| implements(t, core.iterable_class))
                    {
                        out.push((name.span, "unnecessary_to_list_in_spreads", Vec::new()));
                    }
                }
            }
            // `unnecessary_null_aware_assignments`: `x ??= null`, salvo
            // quando a escrita vai para um setter (campo, variável de topo,
            // setter de extensão; o `[]=` não é setter).
            ExprKind::Assign { op: AssignOp::Compound(BinaryOp::IfNull), target, value } => {
                let tgt = a.expr(*target);
                let to_setter = matches!(tgt.kind, ExprKind::Identifier(_) | ExprKind::Property { .. })
                    && matches!(
                        the_body.get_resolved(*target),
                        Some(Resolved::Member { .. } | Resolved::Element(_) | Resolved::ExtensionMember { .. })
                    );
                if matches!(a.expr(sem_parenteses(a, *value)).kind, ExprKind::Null) && !to_setter {
                    out.push((e.span, "unnecessary_null_aware_assignments", Vec::new()));
                }
            }
            // `await_only_futures`.
            ExprKind::Await(the_operand) => {
                if matches!(a.expr(*the_operand).kind, ExprKind::Null) || the_body.invalid_types.contains(the_operand) {
                    continue;
                }
                let Some(t) = ty(*the_operand) else { continue };
                let accepted = match table.get(t) {
                    Type::Dynamic | Type::FutureOr { .. } | Type::ExtensionType { .. } => true,
                    Type::Interface { .. } => implements(t, core.future_class),
                    _ => false,
                };
                if !accepted && source.get(e.span.start..).is_some_and(|x| x.starts_with("await")) {
                    out.push((Span { start: e.span.start, end: e.span.start + 5 }, "await_only_futures", vec![table.format(t, interner, program)]));
                }
            }
            _ => {}
        }
    }
    out
}
