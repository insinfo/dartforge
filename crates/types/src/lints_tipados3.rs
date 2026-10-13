//! Mais regras de lint que pedem os tipos estáticos e a resolução da
//! inferência (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos
//! emissores da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`).
//! Como em [`crate::lints_tipados`], devolvem achados neutros (a posição, o
//! nome único do código e os argumentos), que quem chama só emite com a
//! regra ligada.
//!
//! Aqui: `use_truncating_division`, `avoid_double_and_int_checks`,
//! `only_throw_errors`, `no_runtimeType_toString`, `avoid_dynamic_calls` e
//! `unrelated_type_equality_checks` (expressão e padrão relacional).
//!
//! Cada regra lê o mesmo dado do emissor:
//! - o tipo escrito de um `is` é o resolvido
//!   (`UnitBodyTypes::tipos_de_anotacoes`, o `TypeAnnotation.type`);
//! - o `realTarget` de uma seção de cascata é o alvo da cascata;
//! - o operando de `++`/`--` (posição de escrita) não tem tipo estático no
//!   analyzer: conta o tipo lido da expressão inteira;
//! - `typesAreUnrelated` usa a subtipagem normativa (`subtyping::is_subtype`),
//!   o `promoteToNonNull`, os limites dos parâmetros de tipo e o
//!   `lookUpConcreteMethod('call')`;
//! - o `typeForInterfaceCheck` desce pelo limite promovido e pelo declarado;
//! - a extensão só é pulada pelo `no_runtimeType_toString` quando o tipo
//!   estendido é uma interface que não é classe concreta.
//!
//! O tipo que a inferência não determinou (`unknown`) não é relatado: é falta
//! de dado da inferência, não um `InvalidType`.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::lints_tipados::Achado;
use crate::resolve::OutlineTypes;
use crate::resolved::{MemberRef, Resolved, UnitBodyTypes};
use crate::subtyping::{SubtypeEnv, is_subtype};
use crate::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionKind, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, AssignOp, BinaryOp, DeclKind, ExprId, ExprKind, Initializer, MemberKind, PatternId, PatternKind, StmtKind, StringPart, UnaryOp};
use dartforge_intern::Interner;
use std::collections::HashMap;

fn sem_parenteses(a: &ast::Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
        e = *x;
    }
    e
}

fn dentro(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

/// A classe `nome` declarada na biblioteca `lib`.
fn class_of(program: &Program, interner: &Interner, lib: Option<LibraryId>, the_name: &str) -> Option<ClassId> {
    let sym = interner.lookup(the_name)?;
    match program.library(lib?).declared.get(&sym)?.getter? {
        Element::Class(c) => Some(c),
        _ => None,
    }
}

/// O tipo estático da expressão, sem o que a inferência não determinou.
fn tipo(body: &UnitBodyTypes, core: &CoreTypes, table: &TypeTable, e: ExprId) -> Option<TypeId> {
    body.get_type(e).filter(|t| !core.is_unknown(table, *t))
}

/// O tipo é a interface `c`, não anulável (`isDartCoreInt`, …).
fn is_of_class(table: &TypeTable, t: TypeId, c: Option<ClassId>) -> bool {
    matches!(table.get(t), Type::Interface { class, nullable: false, .. } if Some(*class) == c)
}

/// O `realTarget` de cada seção de cascata: o `CascadeTarget` de cada seção
/// aponta para o alvo da cascata.
fn cascade_targets(a: &ast::Ast) -> HashMap<ExprId, ExprId> {
    let mut m = HashMap::new();
    for e in a.exprs.iter() {
        let ExprKind::Cascade { target, sections, .. } = &e.kind else { continue };
        for &s in sections.iter() {
            // Desce pela cadeia de receptores da seção até o marcador.
            let mut x = s;
            loop {
                let next_ = match &a.expr(x).kind {
                    ExprKind::Property { target, .. }
                    | ExprKind::Index { target, .. }
                    | ExprKind::Call { target, .. }
                    | ExprKind::TypeArguments { target, .. }
                    | ExprKind::Assign { target, .. } => *target,
                    ExprKind::Unary { operand, .. } => *operand,
                    ExprKind::CascadeTarget => {
                        m.insert(x, *target);
                        break;
                    }
                    _ => break,
                };
                x = next_;
            }
        }
    }
    m
}

/// Os achados das regras tipadas deste módulo na unidade `u`.
pub fn achados(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    body: &UnitBodyTypes,
    u: UnitId,
) -> Vec<Achado> {
    let mut out: Vec<Achado> = Vec::new();
    let a = &program.unit(u).ast;
    let cascades = cascade_targets(a);
    let real = |x: ExprId| if matches!(a.expr(x).kind, ExprKind::CascadeTarget) { cascades.get(&x).copied() } else { Some(x) };

    truncating_divide(interner, table, core, body, a, &mut out);
    double_e_int(table, core, body, a, &mut out);
    only_throw_errors(program, interner, table, core, outline, body, a, &mut out);
    runtime_type_to_string(program, interner, table, outline, body, u, a, &real, &mut out);
    dynamic_calls(interner, table, core, body, a, &real, &mut out);
    unrelated_equality(program, interner, table, core, outline, body, u, &mut out);
    out
}

/// `use_truncating_division`: `(a / b).toInt()` com `a` e `b` `int` e o `/`
/// do `dart:core`.
fn truncating_divide(interner: &Interner, table: &TypeTable, core: &CoreTypes, body: &UnitBodyTypes, a: &ast::Ast, out: &mut Vec<Achado>) {
    for e in a.exprs.iter() {
        let ExprKind::Call { target, arguments } = &e.kind else { continue };
        if !arguments.args.is_empty() {
            continue;
        }
        let ExprKind::Property { target: tgt, name, .. } = &a.expr(*target).kind else { continue };
        if interner.resolve(name.sym) != "toInt" || !matches!(a.expr(*tgt).kind, ExprKind::Parenthesized(_)) {
            continue;
        }
        let ExprKind::Binary { op: BinaryOp::Div, left, right } = &a.expr(sem_parenteses(a, *tgt)).kind else { continue };
        let int = |x: ExprId| tipo(body, core, table, x).is_some_and(|t| is_of_class(table, t, core.int_class));
        if int(*left) && int(*right) {
            out.push((e.span, "use_truncating_division", Vec::new()));
        }
    }
}

/// `avoid_double_and_int_checks`: `if (x is double) … else if (x is int)`,
/// com `x` parâmetro ou variável local, os tipos resolvidos iguais a
/// `double` e `int` (o `is!` também conta: o emissor não olha o `!`).
fn double_e_int(table: &mut TypeTable, core: &CoreTypes, body: &UnitBodyTypes, a: &ast::Ast, out: &mut Vec<Achado>) {
    for s in a.stmts.iter() {
        let StmtKind::If { condition, else_: Some(otherwise), .. } = &s.kind else { continue };
        let StmtKind::If { condition: c2, .. } = &a.stmt(*otherwise).kind else { continue };
        let (ExprKind::Is { value: v1, ty: t1, .. }, ExprKind::Is { value: v2, ty: t2, .. }) = (&a.expr(*condition).kind, &a.expr(*c2).kind) else {
            continue;
        };
        let (ExprKind::Identifier(n1), ExprKind::Identifier(n2)) = (&a.expr(*v1).kind, &a.expr(*v2).kind) else { continue };
        if n1.sym != n2.sym {
            continue;
        }
        let local = matches!(body.get_resolved(*v1), Some(Resolved::Local(_) | Resolved::Parameter { .. }));
        let mut resolved = |t: ast::TypeId| body.tipos_de_anotacoes.get(&t).map(|&x| crate::ops::without_display(x, table));
        if local && resolved(*t1) == Some(core.double) && resolved(*t2) == Some(core.int) {
            out.push((a.expr(*c2).span, "avoid_double_and_int_checks", Vec::new()));
        }
    }
}

/// O `typeForInterfaceCheck`: o parâmetro de tipo vira o limite promovido
/// ou o declarado, até não ser mais parâmetro.
pub(crate) fn to_interface(table: &TypeTable, core: &CoreTypes, mut t: TypeId) -> TypeId {
    for _ in 0..64 {
        match table.get(t) {
            Type::Intersection { bound, .. } => t = *bound,
            Type::TypeParameter { param, .. } => {
                let p = table.param(*param);
                // Sem limite escrito, o limite é `Object?`.
                t = if p.explicito { p.bound } else { core.object_nullable };
            }
            _ => return t,
        }
    }
    t
}

/// `only_throw_errors`: o literal lançado, ou o tipo que não é `dynamic`,
/// `Never` nem implementa `Exception`/`Error` do `dart:core`.
#[allow(clippy::too_many_arguments)]
fn only_throw_errors(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    body: &UnitBodyTypes,
    a: &ast::Ast,
    out: &mut Vec<Achado>,
) {
    let exception = class_of(program, interner, core.core_library, "Exception");
    let error = class_of(program, interner, core.core_library, "Error");
    // `implementsAnyInterface`: a própria ou algum supertipo.
    let implements = |c: ClassId| {
        [exception, error]
            .into_iter()
            .flatten()
            .any(|k| c == k || (program.class(c).decl.is_some() && outline.hierarchy.get(c).is_some_and(|d| d.supertypes.contains_key(&k))))
    };
    for e in a.exprs.iter() {
        let ExprKind::Throw(x) = &e.kind else { continue };
        let thrown = a.expr(*x);
        // `Literal`: booleano, número, `null`, string, símbolo, coleção, record.
        let literal = matches!(
            thrown.kind,
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
        let throwable = match tipo(body, core, table, *x) {
            None => true,
            Some(t) => {
                if matches!(table.get(t), Type::Never) {
                    true
                } else {
                    match table.get(to_interface(table, core, t)) {
                        Type::Dynamic => true,
                        Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => implements(*class),
                        _ => false,
                    }
                }
            }
        };
        if literal || !throwable {
            out.push((thrown.span, "only_throw_errors", Vec::new()));
        }
    }
}

/// `no_runtimeType_toString`.
#[allow(clippy::too_many_arguments)]
fn runtime_type_to_string(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    outline: &OutlineTypes,
    body: &UnitBodyTypes,
    u: UnitId,
    a: &ast::Ast,
    real: &dyn Fn(ExprId) -> Option<ExprId>,
    out: &mut Vec<Achado>,
) {
    let the_name = |s: dartforge_intern::SymbolId| interner.resolve(s);
    // `_canSkip`: as regiões em que a regra não age.
    let mut outside: Vec<Span> = Vec::new();
    for s in a.stmts.iter() {
        if matches!(s.kind, StmtKind::Assert { .. }) {
            outside.push(s.span);
        }
        if let StmtKind::Try { catches, .. } = &s.kind {
            outside.extend(catches.iter().map(|c| c.span));
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind {
            outside.extend(k.initializers.iter().filter_map(|i| match i {
                Initializer::Assert { span, .. } => Some(*span),
                _ => None,
            }));
        }
    }
    for e in a.exprs.iter() {
        if matches!(e.kind, ExprKind::Throw(_)) {
            outside.push(e.span);
        }
    }
    for (di, d) in a.decls.iter().enumerate() {
        match &d.kind {
            DeclKind::Mixin(_) => outside.push(d.span),
            DeclKind::Class(x) if x.modifiers.abstract_ => outside.push(d.span),
            // A extensão cujo tipo estendido é uma interface que não é
            // classe concreta.
            DeclKind::Extension(_) => {
                let Some(i) = program.extensions.iter().position(|x| x.decl.unit == u && x.decl.decl == ast::DeclId(di as u32)) else { continue };
                let Some(data) = outline.extensions.get(i) else { continue };
                let skips = match table.get(data.on) {
                    Type::Interface { class, .. } => {
                        let c = program.class(*class);
                        let concrete = matches!(c.kind, ClassKind::Class | ClassKind::MixinApplication) && !c.modifiers.abstract_ && !c.modifiers.sealed;
                        !concrete
                    }
                    // `Null` é classe concreta; `FutureOr` é abstrata; o tipo
                    // de extensão não é classe.
                    Type::Null => false,
                    Type::FutureOr { .. } | Type::ExtensionType { .. } => true,
                    _ => false,
                };
                if skips {
                    outside.push(d.span);
                }
            }
            _ => {}
        }
    }
    let skips = |s: Span| outside.iter().any(|&r| dentro(s, r));
    // `_isRuntimeTypeAccess`: `this.runtimeType`, `super.runtimeType`, ou o
    // identificador `runtimeType` cujo elemento é um getter.
    let e_getter = |r: Option<&Resolved>| match r {
        Some(Resolved::Member { member: MemberRef::Variable(_), .. }) | Some(Resolved::Element(Element::Variable(_))) => true,
        Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::Element(Element::Function(f))) => {
            program.function(*f).kind == FunctionKind::Getter || program.function(*f).variable.is_some()
        }
        Some(Resolved::ExtensionMember { member, .. }) => program.function(*member).kind == FunctionKind::Getter,
        _ => false,
    };
    let e_runtime_type = |x: ExprId| match &a.expr(x).kind {
        ExprKind::Property { target, name, .. } => the_name(name.sym) == "runtimeType" && matches!(a.expr(*target).kind, ExprKind::This | ExprKind::Super),
        ExprKind::Identifier(n) => the_name(n.sym) == "runtimeType" && e_getter(body.get_resolved(x)),
        _ => false,
    };
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::String(lit) => {
                for parte in lit.parts.iter() {
                    if let StringPart::Interpolation(x) = parte
                        && e_runtime_type(*x)
                        && !skips(a.expr(*x).span)
                    {
                        out.push((a.expr(*x).span, "no_runtimeType_toString", Vec::new()));
                    }
                }
            }
            ExprKind::Call { target, .. } => {
                if let ExprKind::Property { target: tgt, name, .. } = &a.expr(*target).kind
                    && the_name(name.sym) == "toString"
                    && real(*tgt).is_some_and(&e_runtime_type)
                    && !skips(e.span)
                {
                    out.push((name.span, "no_runtimeType_toString", Vec::new()));
                }
            }
            _ => {}
        }
    }
}

/// `avoid_dynamic_calls`.
fn dynamic_calls(
    interner: &Interner,
    table: &TypeTable,
    core: &CoreTypes,
    body: &UnitBodyTypes,
    a: &ast::Ast,
    real: &dyn Fn(ExprId) -> Option<ExprId>,
    out: &mut Vec<Achado>,
) {
    let the_name = |s: dartforge_intern::SymbolId| interner.resolve(s);
    let ty = |e: ExprId| tipo(body, core, table, e);
    let dynamic = |e: ExprId| ty(e).is_some_and(|t| matches!(table.get(t), Type::Dynamic));
    let as_ = |x: ExprId| matches!(a.expr(sem_parenteses(a, x)).kind, ExprKind::As { .. });
    let mut findings: Vec<Span> = Vec::new();
    // `_reportIfDynamic`.
    let if_dynamic = |x: Option<ExprId>, findings: &mut Vec<Span>| -> bool {
        let Some(x) = x else { return false };
        if dynamic(x) && !as_(x) {
            findings.push(a.expr(x).span);
            return true;
        }
        false
    };
    // `_reportIfDynamicOrFunction`.
    let dynamic_or_function = |no: ExprId, static_type: Option<TypeId>, findings: &mut Vec<Span>| {
        let Some(t) = static_type else { return };
        if as_(no) {
            return;
        }
        if matches!(table.get(t), Type::Dynamic) || is_of_class(table, t, core.function_class) {
            findings.push(a.expr(no).span);
        }
    };
    let allowed = |n: &str| matches!(n, "hashCode" | "noSuchMethod" | "runtimeType" | "toString");
    // Os alvos de chamada não são acesso a propriedade.
    let mut called: std::collections::HashSet<ExprId> = std::collections::HashSet::new();
    for e in a.exprs.iter() {
        if let ExprKind::Call { target, .. } = &e.kind {
            called.insert(*target);
        }
    }
    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        match &e.kind {
            // A criação de instância sem `new` não é chamada.
            ExprKind::Call { .. } if matches!(body.get_resolved(id), Some(Resolved::Constructor(_))) => {}
            ExprKind::Call { target, arguments } => match &a.expr(*target).kind {
                // `MethodInvocation` com alvo (ou em cascata).
                ExprKind::Property { target: tgt, name, .. } => {
                    let in_cascade = matches!(a.expr(*tgt).kind, ExprKind::CascadeTarget);
                    let n = the_name(name.sym);
                    if !in_cascade {
                        let single_positional = arguments.args.len() == 1 && arguments.args[0].name.is_none();
                        if (n == "noSuchMethod" && single_positional) || (n == "toString" && arguments.args.is_empty()) {
                            continue;
                        }
                    }
                    let real_target = real(*tgt);
                    if !if_dynamic(real_target, &mut findings) {
                        // O `methodName`: o tipo do membro, salvo `call`
                        // sobre um tipo de função.
                        let target_type = real_target.and_then(ty);
                        let static_ = match target_type {
                            Some(t) if n == "call" && matches!(table.get(t), Type::Function { .. }) => Some(t),
                            _ => ty(*target),
                        };
                        if let Some(t) = static_
                            && (matches!(table.get(t), Type::Dynamic) || is_of_class(table, t, core.function_class))
                        {
                            findings.push(name.span);
                        }
                    }
                }
                // `MethodInvocation` sem alvo: o `methodName`.
                ExprKind::Identifier(_) => dynamic_or_function(*target, ty(*target), &mut findings),
                // `FunctionExpressionInvocation`: `node.function`.
                _ => dynamic_or_function(*target, ty(*target), &mut findings),
            },
            // `PropertyAccess`/`PrefixedIdentifier` (não alvo de chamada).
            ExprKind::Property { target, name, .. } if !called.contains(&id) => {
                if !allowed(the_name(name.sym)) {
                    if_dynamic(real(*target), &mut findings);
                }
            }
            ExprKind::Index { target, .. } => {
                if_dynamic(real(*target), &mut findings);
            }
            ExprKind::Binary { op, left, .. } => {
                let definable = !matches!(op, BinaryOp::Eq | BinaryOp::NotEq | BinaryOp::And | BinaryOp::Or | BinaryOp::IfNull);
                if definable {
                    if_dynamic(Some(*left), &mut findings);
                }
            }
            ExprKind::Unary { op, operand } => match op {
                UnaryOp::NullAssert => {}
                // `++`/`--`: o operando é posição de escrita (sem tipo
                // estático no analyzer); conta o tipo lido.
                UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec => {
                    if dynamic(*operand) {
                        findings.push(e.span);
                    }
                }
                _ => {
                    if_dynamic(Some(*operand), &mut findings);
                }
            },
            // A atribuição com leitura `dynamic` (não `=`, não `??=`).
            ExprKind::Assign { op, target, .. } => {
                if !matches!(op, AssignOp::Assign | AssignOp::Compound(BinaryOp::IfNull)) && dynamic(*target) {
                    findings.push(e.span);
                }
            }
            _ => {}
        }
    }
    findings.sort_by_key(|s| (s.start, s.end));
    findings.dedup();
    out.extend(findings.into_iter().map(|s| (s, "avoid_dynamic_calls", Vec::new())));
}

/// `unrelated_type_equality_checks`: a expressão `a == b`/`a != b` de tipo
/// `bool` e o padrão relacional `== x`/`!= x`.
#[allow(clippy::too_many_arguments)]
fn unrelated_equality(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    body: &UnitBodyTypes,
    u: UnitId,
    out: &mut Vec<Achado>,
) {
    let unit = program.unit(u);
    let a = &unit.ast;
    let future_or = class_of(program, interner, core.async_library, "FutureOr");
    // `isFixnumIntX` contra `isCoreInt`.
    let fixnum = |table: &TypeTable, l: TypeId, r: TypeId| {
        let l_fixnum = match table.get(l) {
            Type::Interface { class, .. } => {
                let c = program.class(*class);
                matches!(interner.resolve(c.name), "Int32" | "Int64") && program.library(c.library).uri.starts_with("package:fixnum/")
            }
            _ => false,
        };
        l_fixnum && is_of_class(table, r, core.int_class)
    };
    let mut cases: Vec<(Span, &'static str, TypeId, TypeId)> = Vec::new();
    for (k, e) in a.exprs.iter().enumerate() {
        let ExprKind::Binary { op: BinaryOp::Eq | BinaryOp::NotEq, left, right } = &e.kind else { continue };
        if matches!(a.expr(*left).kind, ExprKind::Null) || matches!(a.expr(*right).kind, ExprKind::Null) {
            continue;
        }
        // `node.staticType?.isDartCoreBool`.
        let Some(tb) = tipo(body, core, table, ExprId(k as u32)) else { continue };
        if !is_of_class(table, tb, core.bool_class) {
            continue;
        }
        let (Some(te), Some(td)) = (tipo(body, core, table, *left), tipo(body, core, table, *right)) else { continue };
        // O token do operador: o primeiro depois do operando esquerdo.
        let i = dartforge_frontend::fonte::pular_brancos(unit.source.as_bytes(), a.expr(*left).span.end);
        cases.push((Span { start: i, end: i + 2 }, "unrelated_type_equality_checks_in_expression", te, td));
    }
    for (k, p) in a.patterns.iter().enumerate() {
        let PatternKind::Relational { op: BinaryOp::Eq | BinaryOp::NotEq, value } = &p.kind else { continue };
        let Some(&the_value) = body.tipos_casados.get(&PatternId(k as u32)) else { continue };
        let Some(the_operand) = tipo(body, core, table, *value) else { continue };
        cases.push((p.span, "unrelated_type_equality_checks_in_pattern", the_value, the_operand));
    }
    for (span, code, l, r) in cases {
        let mut rel = Relation { program, table: &mut *table, core, outline, future_or, call: interner.lookup("call") };
        if !rel.unrelated(l, r) || fixnum(table, l, r) {
            continue;
        }
        // `[rightType, leftType]` (na forma do padrão: `[operando, valor]`).
        let args = vec![crate::despejo::formatar(table, r, interner, program), crate::despejo::formatar(table, l, interner, program)];
        out.push((span, code, args));
    }
}

/// `typesAreUnrelated` do linter (`util/dart_type_utilities.dart:150`).
pub(crate) struct Relation<'a> {
    pub(crate) program: &'a Program,
    pub(crate) table: &'a mut TypeTable,
    pub(crate) core: &'a CoreTypes,
    pub(crate) outline: &'a OutlineTypes,
    pub(crate) future_or: Option<ClassId>,
    /// O símbolo `call` (sem ele, nenhuma classe tem `call`).
    pub(crate) call: Option<dartforge_intern::SymbolId>,
}

impl Relation<'_> {
    fn sub(&mut self, a: TypeId, b: TypeId) -> bool {
        let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
        is_subtype(a, b, &mut env)
    }

    /// `isBottom`: `Never`, ou o parâmetro de tipo não anulável cujo limite
    /// (promovido, ou o escrito) é fundo.
    fn bottom(&self, t: TypeId) -> bool {
        match self.table.get(t) {
            Type::Never => true,
            Type::Intersection { bound, .. } => self.bottom(*bound),
            Type::TypeParameter { param, nullable: false } => {
                let p = self.table.param(*param);
                p.explicito && self.bottom(p.bound)
            }
            _ => false,
        }
    }

    /// O elemento e os argumentos de um tipo que o analyzer tem por
    /// `InterfaceType` (classe, enum, mixin, tipo de extensão, `FutureOr`).
    fn interface(&self, t: TypeId) -> Option<(ClassId, Vec<TypeId>)> {
        match self.table.get(t) {
            Type::Interface { class, args, .. } => Some((*class, args.to_vec())),
            Type::ExtensionType { decl, args, .. } => Some((*decl, args.to_vec())),
            Type::FutureOr { arg, .. } => self.future_or.map(|c| (c, vec![*arg])),
            _ => None,
        }
    }

    /// O parâmetro de tipo (também promovido) e o limite escrito dele.
    fn parametro(&self, t: TypeId) -> Option<Option<TypeId>> {
        let param = match self.table.get(t) {
            Type::TypeParameter { param, .. } | Type::Intersection { param, .. } => *param,
            _ => return None,
        };
        let p = self.table.param(param);
        Some(if p.explicito { Some(p.bound) } else { None })
    }

    /// O `supertype` do elemento (nulo para `Object`, mixin e tipo de
    /// extensão).
    fn the_supertype(&mut self, c: ClassId) -> Option<TypeId> {
        if matches!(self.program.class(c).kind, ClassKind::Mixin | ClassKind::ExtensionType) {
            return None;
        }
        let s = self.outline.classes.get(c.0 as usize).and_then(|x| x.supertype)?;
        Some(crate::ops::without_display(s, self.table))
    }

    pub(crate) fn unrelated(&mut self, l: TypeId, r: TypeId) -> bool {
        let dynamic = |t: &Type| matches!(t, Type::Dynamic);
        if self.bottom(l) || dynamic(self.table.get(l)) || self.bottom(r) || dynamic(self.table.get(r)) {
            return false;
        }
        let pl = crate::ops::non_nullable(l, self.table);
        let pr = crate::ops::non_nullable(r, self.table);
        let (cl, cr) = (crate::ops::without_display(pl, self.table), crate::ops::without_display(pr, self.table));
        if cl == cr || self.sub(pl, pr) || self.sub(pr, pl) {
            return false;
        }
        if let (Some((el, al)), Some((er, ar))) = (self.interface(pl), self.interface(pr)) {
            return self.unrelated_interfaces(el, &al, er, &ar);
        }
        if let (Some(bl), Some(br)) = (self.parametro(pl), self.parametro(pr)) {
            return match (bl, br) {
                (Some(x), Some(y)) => self.unrelated(x, y),
                _ => false,
            };
        }
        if matches!(self.table.get(pl), Type::Function { .. }) {
            return self.unrelated_function(pr);
        }
        if matches!(self.table.get(pr), Type::Function { .. }) {
            return self.unrelated_function(pl);
        }
        if matches!(self.table.get(pl), Type::Record { .. }) || matches!(self.table.get(pr), Type::Record { .. }) {
            // `isAssignableTo` entre tipos não `dynamic`: a subtipagem.
            return !self.sub(pl, pr) && !self.sub(pr, pl);
        }
        false
    }

    /// `interfaceTypesAreUnrelated`.
    fn unrelated_interfaces(&mut self, el: ClassId, al: &[TypeId], er: ClassId, ar: &[TypeId]) -> bool {
        if el == er {
            if al.len() != ar.len() {
                return false;
            }
            return al.iter().zip(ar.iter()).any(|(x, y)| self.unrelated(*x, *y));
        }
        let (sl, sr) = (self.the_supertype(el), self.the_supertype(er));
        let same = sl == sr;
        if same && self.program.class(el).kind == ClassKind::Enum {
            return true;
        }
        let of_object = sl.is_some_and(|s| matches!(self.table.get(s), Type::Interface { class, nullable: false, .. } if Some(*class) == self.core.object_class));
        of_object || !same
    }

    /// `_isFunctionTypeUnrelatedToType`: só a classe com `call` concreto
    /// (`lookUpConcreteMethod`) e o tipo de função se relacionam.
    fn unrelated_function(&mut self, other: TypeId) -> bool {
        match self.table.get(other) {
            Type::Function { .. } => false,
            Type::Interface { class, .. } => {
                let c = self.program.class(*class);
                if matches!(c.kind, ClassKind::Class | ClassKind::MixinApplication)
                    && let Some(call) = self.call
                    && crate::fase_super::concreto(self.program, *class, true, call, c.library, false, true)
                {
                    return false;
                }
                true
            }
            _ => true,
        }
    }
}
