//! The twenty-sixth batch of lint rules
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), written directly from the 3.6.2
//! emitters (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`),
//! through the unit's semantics:
//!
//! * `unnecessary_statements`: an expression without a clear effect in an
//!   expression statement, in the initialization and the updaters of an
//!   expression `for`, and a cascade section that is a function-typed access.
//!   A written (non-synthetic) getter does not count; `??`, `||` and `&&` only
//!   look at the right side; a conditional, at both branches.
//! * `unnecessary_parenthesis`: the parentheses around an expression, by the
//!   parent the analyzer would give them (the `MethodInvocation` when they are
//!   the target of a method call, the `NamedExpression` of a named argument
//!   or of a record field, the `ArgumentList` of an annotation, of an enum
//!   constant and of `super(…)`/`this(…)`), with all the emitter's exceptions
//!   in its order.
//!
//! Written without compiling or running (2026-10-05).

use super::LintCode;
use super::codes_g as c;
use super::rules::LintReport;
use super::rules16::{
    NodeParents, declared_variable_type, is_method_invocation, is_nullable, unit_parents,
};
use super::walk::Node;
use crate::Unit;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{Element, FunctionKind, FunctionRef, VariableRef};
use dartforge_frontend::ast::{
    self, Ast, BinaryOp, CollectionElement, DeclKind, ExprId, ExprKind, ForInit, FunctionBody,
    Initializer, MemberKind, PatternKind, StmtKind, UnaryOp,
};
use dartforge_intern::Interner;
use dartforge_types::resolved::{MemberRef, Resolved};
use dartforge_types::table::Type;
use std::collections::HashMap;

/// The resolved element is a written (non-synthetic) `GetterElement`.
fn is_written_getter(s: &super::Semantics<'_>, e: ExprId) -> bool {
    let f = match s.body.get_resolved(e) {
        Some(Resolved::Member {
            member: MemberRef::Function(f),
            ..
        })
        | Some(Resolved::Element(Element::Function(f))) => *f,
        Some(Resolved::ExtensionMember { member, .. }) => *member,
        _ => return false,
    };
    let g = s.program.function(f);
    g.kind == FunctionKind::Getter && !matches!(g.node, FunctionRef::None)
}

/// `_ReportNoClearEffectVisitor`: reports `e` (or its parts) when it has no
/// clear effect.
fn no_clear_effect(s: &super::Semantics<'_>, a: &Ast, e: ExprId, out: &mut Vec<Span>) {
    match &a.expr(e).kind {
        ExprKind::As { .. }
        | ExprKind::Assign { .. }
        | ExprKind::Await(_)
        | ExprKind::Cascade { .. }
        | ExprKind::Call { .. }
        | ExprKind::InstanceCreation { .. }
        | ExprKind::PatternAssign { .. }
        | ExprKind::Rethrow
        | ExprKind::Throw(_) => {}
        ExprKind::Unary {
            op:
                UnaryOp::PostfixInc
                | UnaryOp::PostfixDec
                | UnaryOp::NullAssert
                | UnaryOp::PrefixInc
                | UnaryOp::PrefixDec,
            ..
        } => {}
        ExprKind::Binary {
            op: BinaryOp::IfNull | BinaryOp::Or | BinaryOp::And,
            right,
            ..
        } => no_clear_effect(s, a, *right, out),
        ExprKind::Conditional { then, else_, .. } => {
            no_clear_effect(s, a, *then, out);
            no_clear_effect(s, a, *else_, out);
        }
        ExprKind::Identifier(_) | ExprKind::Property { .. } if is_written_getter(s, e) => {}
        _ => out.push(a.expr(e).span),
    }
}

/// The parent of a parenthesized node as the analyzer sees it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ParenParent {
    Parens,
    Interpolation,
    /// Positional argument of an `ArgumentList` with `n` arguments.
    Argument {
        n: usize,
    },
    /// The `NamedExpression` of a named argument.
    Named,
    /// The `NamedExpression` of a record field.
    NamedField,
    IfCondition,
    IfElementCondition,
    WhileCondition,
    DoCondition,
    SwitchStatement,
    SwitchExpression,
    ConstantPattern,
    Spread,
    /// `VariableDeclaration`, with the element's type.
    Variable(Option<dartforge_types::table::TypeId>),
    /// `PropertyAccess` whose target is the parentheses.
    PropertyAccess(ExprId),
    /// `MethodInvocation` (the call and the name access).
    MethodInvocation {
        call_expr: ExprId,
        name_expr: ExprId,
    },
    Index,
    BangPostfix,
    Postfix,
    Prefix,
    Binary,
    Conditional {
        then_branch: bool,
    },
    MapKey,
    Cascade,
    FunctionInvocation {
        has_type_args: bool,
    },
    As,
    Is,
    Assignment,
    Return,
    Yield,
    FieldInitializer,
    ExpressionBody,
    ExpressionStatement,
    OtherExpression,
    Other,
}

impl ParenParent {
    /// The parent is an `Expression` in the analyzer.
    fn is_expression(self) -> bool {
        matches!(
            self,
            ParenParent::Parens
                | ParenParent::Named
                | ParenParent::NamedField
                | ParenParent::SwitchExpression
                | ParenParent::PropertyAccess(_)
                | ParenParent::MethodInvocation { .. }
                | ParenParent::Index
                | ParenParent::BangPostfix
                | ParenParent::Postfix
                | ParenParent::Prefix
                | ParenParent::Binary
                | ParenParent::Conditional { .. }
                | ParenParent::Cascade
                | ParenParent::FunctionInvocation { .. }
                | ParenParent::As
                | ParenParent::Is
                | ParenParent::Assignment
                | ParenParent::OtherExpression
        )
    }
}

/// The role of `me` in a collection element; `direct`: the element is a
/// child of the literal (the parent is the literal's `Expression`) and not of
/// an `IfElement`/`ForElement`.
fn collection_role(
    s: &super::Semantics<'_>,
    x: &CollectionElement,
    me: ExprId,
    direct: bool,
) -> Option<ParenParent> {
    match x {
        CollectionElement::Expression(e) if *e == me => Some(if direct {
            ParenParent::OtherExpression
        } else {
            ParenParent::Other
        }),
        CollectionElement::NullAwareExpression(e) if *e == me => Some(ParenParent::Other),
        CollectionElement::MapEntry { key, .. } if *key == me => Some(ParenParent::MapKey),
        CollectionElement::MapEntry { value, .. } if *value == me => Some(ParenParent::Other),
        CollectionElement::Spread { value, .. } if *value == me => Some(ParenParent::Spread),
        CollectionElement::If {
            condition,
            guard,
            then,
            else_,
            ..
        } => {
            if *condition == me {
                return Some(ParenParent::IfElementCondition);
            }
            if *guard == Some(me) {
                return Some(ParenParent::Other);
            }
            collection_role(s, then, me, false).or_else(|| {
                else_
                    .as_deref()
                    .and_then(|x| collection_role(s, x, me, false))
            })
        }
        CollectionElement::For {
            init,
            condition,
            updates,
            body,
            ..
        } => {
            match init {
                Some(ForInit::Variables(l)) => {
                    if let Some(v) = l.variables.iter().find(|v| v.initializer == Some(me)) {
                        return Some(ParenParent::Variable(s.body.local_type(v.name.span.start)));
                    }
                }
                Some(ForInit::Expression(e)) if *e == me => return Some(ParenParent::Other),
                Some(ForInit::Pattern { value, .. }) if *value == me => {
                    return Some(ParenParent::Other);
                }
                _ => {}
            }
            if *condition == Some(me) || updates.contains(&me) {
                return Some(ParenParent::Other);
            }
            collection_role(s, body, me, false)
        }
        CollectionElement::ForIn { iterable, body, .. } => {
            if *iterable == me {
                return Some(ParenParent::Other);
            }
            collection_role(s, body, me, false)
        }
        _ => None,
    }
}

/// The role of `me` in an argument list.
fn argument_role(args: &ast::Arguments, me: ExprId) -> ParenParent {
    match args.args.iter().find(|x| x.value == me) {
        Some(x) if x.name.is_some() => ParenParent::Named,
        Some(_) => ParenParent::Argument { n: args.args.len() },
        None => ParenParent::Other,
    }
}

/// The arguments of the annotations and of the enum constants by range (the
/// walk's `Node::Annotation`/`Node::EnumArguments` nodes).
fn arguments_by_range(a: &Ast) -> HashMap<(usize, usize), &ast::Arguments> {
    let mut m = HashMap::new();
    fn add_annotations<'a>(
        xs: &'a [ast::Annotation],
        m: &mut HashMap<(usize, usize), &'a ast::Arguments>,
    ) {
        for x in xs {
            if let Some(args) = &x.arguments {
                m.insert((x.span.start, x.span.end), args);
            }
        }
    }
    fn add_params<'a>(
        ps: &'a [ast::Parameter],
        m: &mut HashMap<(usize, usize), &'a ast::Arguments>,
    ) {
        for p in ps {
            add_annotations(&p.metadata, m);
            if let Some(inner) = &p.function_parameters {
                add_params(inner, m);
            }
        }
    }
    for d in a.decls.iter() {
        add_annotations(&d.metadata, &mut m);
        match &d.kind {
            DeclKind::Enum(x) => {
                for k in x.constants.iter() {
                    add_annotations(&k.metadata, &mut m);
                    if let Some(args) = &k.arguments {
                        m.insert((k.span.start, k.span.end), args);
                    }
                }
            }
            DeclKind::ExtensionType(x) => add_annotations(&x.representation_metadata, &mut m),
            _ => {}
        }
    }
    for x in a.members.iter() {
        add_annotations(&x.metadata, &mut m);
        if let MemberKind::Constructor(k) = &x.kind {
            add_params(&k.parameters, &mut m);
        }
    }
    for f in a.functions.iter() {
        add_params(f.parameters.as_deref().unwrap_or(&[]), &mut m);
    }
    for t in a.types.iter() {
        if let ast::TypeKind::Function { parameters, .. } = &t.kind {
            add_params(parameters, &mut m);
        }
    }
    m
}

/// The (analyzer) parent of the parentheses `me`.
fn parent_of(
    s: &super::Semantics<'_>,
    a: &Ast,
    parents: &NodeParents,
    by_range: &HashMap<(usize, usize), &ast::Arguments>,
    me: ExprId,
) -> ParenParent {
    let Some(node) = parents.expr.get(&me).copied() else {
        return ParenParent::Other;
    };
    match node {
        Node::Expr(p) => match &a.expr(p).kind {
            ExprKind::Parenthesized(_) => ParenParent::Parens,
            ExprKind::String(_) => ParenParent::Interpolation,
            ExprKind::Call { target, arguments } => {
                if *target == me {
                    ParenParent::FunctionInvocation {
                        has_type_args: !arguments.type_args.is_empty(),
                    }
                } else {
                    argument_role(arguments, me)
                }
            }
            ExprKind::InstanceCreation { arguments, .. } => argument_role(arguments, me),
            ExprKind::Property { .. } => {
                // The target of `(x).m(…)` is the `MethodInvocation`'s.
                if let Some(Node::Expr(ch)) = parents.expr.get(&p).copied()
                    && matches!(&a.expr(ch).kind, ExprKind::Call { target, .. } if *target == p)
                    && is_method_invocation(s, a, p)
                {
                    ParenParent::MethodInvocation {
                        call_expr: ch,
                        name_expr: p,
                    }
                } else {
                    ParenParent::PropertyAccess(p)
                }
            }
            ExprKind::Index { .. } => ParenParent::Index,
            ExprKind::Unary {
                op: UnaryOp::NullAssert,
                ..
            } => ParenParent::BangPostfix,
            ExprKind::Unary {
                op: UnaryOp::PostfixInc | UnaryOp::PostfixDec,
                ..
            } => ParenParent::Postfix,
            ExprKind::Unary { .. } => ParenParent::Prefix,
            ExprKind::Binary { .. } => ParenParent::Binary,
            ExprKind::Conditional { then, .. } => ParenParent::Conditional {
                then_branch: *then == me,
            },
            ExprKind::Is { .. } => ParenParent::Is,
            ExprKind::As { .. } => ParenParent::As,
            ExprKind::Assign { .. } => ParenParent::Assignment,
            ExprKind::Cascade { .. } => ParenParent::Cascade,
            ExprKind::Switch { value, .. } => {
                if *value == me {
                    ParenParent::SwitchExpression
                } else {
                    // The body and the guard of a case.
                    ParenParent::Other
                }
            }
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => elements
                .iter()
                .find_map(|x| collection_role(s, x, me, true))
                .unwrap_or(ParenParent::Other),
            ExprKind::Record { named, .. } => {
                if named.iter().any(|(_, x)| *x == me) {
                    ParenParent::NamedField
                } else {
                    ParenParent::OtherExpression
                }
            }
            ExprKind::TypeArguments { .. } => match parents.expr.get(&p).copied() {
                Some(Node::Expr(ch)) if matches!(&a.expr(ch).kind, ExprKind::Call { target, .. } if *target == p) => {
                    ParenParent::FunctionInvocation {
                        has_type_args: true,
                    }
                }
                _ => ParenParent::OtherExpression,
            },
            _ => ParenParent::OtherExpression,
        },
        Node::Stmt(st) => match &a.stmt(st).kind {
            StmtKind::Expression(_) => ParenParent::ExpressionStatement,
            StmtKind::Return(_) => ParenParent::Return,
            StmtKind::Yield { .. } => ParenParent::Yield,
            StmtKind::If { condition, .. } if *condition == me => ParenParent::IfCondition,
            StmtKind::While { condition, .. } if *condition == me => ParenParent::WhileCondition,
            StmtKind::DoWhile { condition, .. } if *condition == me => ParenParent::DoCondition,
            StmtKind::Switch { value, .. } if *value == me => ParenParent::SwitchStatement,
            StmtKind::Variables(l)
            | StmtKind::For {
                init: Some(ForInit::Variables(l)),
                ..
            } => match l.variables.iter().find(|v| v.initializer == Some(me)) {
                Some(v) => ParenParent::Variable(s.body.local_type(v.name.span.start)),
                None => ParenParent::Other,
            },
            _ => ParenParent::Other,
        },
        Node::Function(f) => match a.function(f).body {
            FunctionBody::Expression(x) if x == me => ParenParent::ExpressionBody,
            _ => ParenParent::Other,
        },
        Node::Member(m) => match &a.member(m).kind {
            MemberKind::Constructor(k) => {
                for i in k.initializers.iter() {
                    match i {
                        Initializer::Field { value, .. } if *value == me => {
                            return ParenParent::FieldInitializer;
                        }
                        Initializer::Super { arguments, .. }
                        | Initializer::Redirect { arguments, .. }
                            if arguments.args.iter().any(|x| x.value == me) =>
                        {
                            return argument_role(arguments, me);
                        }
                        _ => {}
                    }
                }
                ParenParent::Other
            }
            MemberKind::Field(l) => {
                match l.variables.iter().position(|v| v.initializer == Some(me)) {
                    Some(index) => ParenParent::Variable(declared_variable_type(
                        s,
                        VariableRef::Field {
                            unit: s.unit,
                            member: m,
                            index,
                        },
                    )),
                    None => ParenParent::Other,
                }
            }
            _ => ParenParent::Other,
        },
        Node::Decl(d) => match &a.decl(d).kind {
            DeclKind::Variables(l) => {
                match l.variables.iter().position(|v| v.initializer == Some(me)) {
                    Some(index) => ParenParent::Variable(declared_variable_type(
                        s,
                        VariableRef::TopLevel {
                            unit: s.unit,
                            decl: d,
                            index,
                        },
                    )),
                    None => ParenParent::Other,
                }
            }
            _ => ParenParent::Other,
        },
        Node::Pattern(p) => match &a.pattern(p).kind {
            PatternKind::Constant(_) => ParenParent::ConstantPattern,
            _ => ParenParent::Other,
        },
        Node::Annotation(sp) | Node::EnumArguments(sp) => match by_range.get(&(sp.start, sp.end)) {
            Some(args) => argument_role(args, me),
            None => ParenParent::Other,
        },
    }
}

/// `containsNullAwareInvocationInChain`.
fn null_aware_chain(s: &super::Semantics<'_>, a: &Ast, e: ExprId) -> bool {
    match &a.expr(e).kind {
        // `PropertyAccess` (the simple `a.b` is a `PrefixedIdentifier`).
        ExprKind::Property {
            target, null_aware, ..
        } => {
            if *null_aware {
                return true;
            }
            !matches!(a.expr(*target).kind, ExprKind::Identifier(_))
                && null_aware_chain(s, a, *target)
        }
        ExprKind::Call { target, .. } if is_method_invocation(s, a, *target) => {
            match &a.expr(*target).kind {
                ExprKind::Property {
                    target: t,
                    null_aware,
                    ..
                } => *null_aware || null_aware_chain(s, a, *t),
                _ => false,
            }
        }
        ExprKind::Index {
            target, null_aware, ..
        } => *null_aware || null_aware_chain(s, a, *target),
        _ => false,
    }
}

/// `directlyContainsWhitespace`.
fn has_space(s: &super::Semantics<'_>, a: &Ast, e: ExprId) -> bool {
    match &a.expr(e).kind {
        ExprKind::As { .. }
        | ExprKind::Assign { .. }
        | ExprKind::Await(_)
        | ExprKind::Binary { .. }
        | ExprKind::Is { .. } => true,
        ExprKind::InstanceCreation { keyword, .. } => keyword.is_some(),
        ExprKind::List { const_, .. } | ExprKind::SetOrMap { const_, .. } => *const_,
        ExprKind::Call { target, .. } if is_method_invocation(s, a, *target) => {
            match &a.expr(*target).kind {
                ExprKind::Property { target: t, .. } => has_space(s, a, *t),
                _ => false,
            }
        }
        ExprKind::Property { target, .. } => has_space(s, a, *target),
        _ => false,
    }
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(
    u: Unit<'_>,
    interner: &Interner,
    enabled: &dyn Fn(&str) -> bool,
    sem: Option<&super::Semantics<'_>>,
) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let Some(s) = sem else { return out };
    let a = u.ast;
    let table = s.table;
    let mut report = |code: &'static LintCode, span: Span, args: &[&str]| {
        out.push(LintReport {
            code,
            span,
            args: args.iter().map(|x| x.to_string()).collect(),
        });
    };

    // `unnecessary_statements`.
    if enabled("unnecessary_statements") {
        let mut findings: Vec<Span> = Vec::new();
        for st in a.stmts.iter() {
            match &st.kind {
                StmtKind::Expression(e) => no_clear_effect(s, a, *e, &mut findings),
                // `ForPartsWithExpression`: the initialization and the updaters.
                StmtKind::For { init, updates, .. }
                    if matches!(init, None | Some(ForInit::Expression(_))) =>
                {
                    if let Some(ForInit::Expression(e)) = init {
                        no_clear_effect(s, a, *e, &mut findings);
                    }
                    for x in updates.iter() {
                        no_clear_effect(s, a, *x, &mut findings);
                    }
                }
                _ => {}
            }
        }
        // The cascade section `..x` that is a function-typed `PropertyAccess`
        // (the node starts at the `..`/`?..`).
        for e in a.exprs.iter() {
            let ExprKind::Cascade {
                sections,
                null_aware,
                ..
            } = &e.kind
            else {
                continue;
            };
            for (i, sec) in sections.iter().enumerate() {
                if !matches!(a.expr(*sec).kind, ExprKind::Property { .. }) {
                    continue;
                }
                if !s
                    .body
                    .get_type(*sec)
                    .is_some_and(|t| matches!(table.get(t), Type::Function { .. }))
                {
                    continue;
                }
                let sp = a.expr(*sec).span;
                let dots = if i == 0 && *null_aware { 3 } else { 2 };
                findings.push(Span {
                    start: sp.start.saturating_sub(dots),
                    end: sp.end,
                });
            }
        }
        for sp in findings {
            report(&c::UNNECESSARY_STATEMENTS, sp, &[]);
        }
    }

    // `unnecessary_parenthesis`.
    if enabled("unnecessary_parenthesis") {
        let is_parens = |e: ExprId| matches!(a.expr(e).kind, ExprKind::Parenthesized(_));
        let parents = unit_parents(u, &is_parens);
        let by_range = arguments_by_range(a);
        let is_record = |t: Option<dartforge_types::table::TypeId>| {
            t.is_some_and(|t| matches!(table.get(t), Type::Record { .. }))
        };
        for (k, e) in a.exprs.iter().enumerate() {
            let node = ExprId(k as u32);
            let ExprKind::Parenthesized(x) = &e.kind else {
                continue;
            };
            let x = *x;
            let parent = parent_of(s, a, &parents, &by_range, node);
            if matches!(parent, ParenParent::ConstantPattern | ParenParent::Spread) {
                continue;
            }
            let is_record_literal = matches!(a.expr(x).kind, ExprKind::Record { .. });
            if let ParenParent::Variable(t) = parent
                && is_record(t)
                && !is_record_literal
            {
                continue;
            }
            // The argument's `staticParameterElement` (positional or named).
            if matches!(parent, ParenParent::Argument { .. } | ParenParent::Named)
                && is_record(s.body.parameter_types.get(&node).copied())
                && !is_record_literal
            {
                continue;
            }
            // Already between delimiters: always reports.
            if matches!(
                parent,
                ParenParent::Parens
                    | ParenParent::Interpolation
                    | ParenParent::Argument { n: 1 }
                    | ParenParent::IfCondition
                    | ParenParent::IfElementCondition
                    | ParenParent::WhileCondition
                    | ParenParent::DoCondition
                    | ParenParent::SwitchStatement
                    | ParenParent::SwitchExpression
            ) {
                report(&c::UNNECESSARY_PARENTHESIS, e.span, &[]);
                continue;
            }
            let inner_kind = &a.expr(x).kind;
            if matches!(inner_kind, ExprKind::Conditional { .. }) {
                continue;
            }
            // The `TypeLiteral` (`List<int>`).
            if let ExprKind::TypeArguments { target, .. } = inner_kind
                && matches!(
                    s.body.get_resolved(*target),
                    Some(Resolved::Element(Element::Class(_) | Element::Typedef(_)))
                )
            {
                continue;
            }
            if matches!(inner_kind, ExprKind::Identifier(_)) || null_aware_chain(s, a, x) {
                let is_nullable_extension = |name_expr: ExprId| match s.body.get_resolved(name_expr)
                {
                    Some(Resolved::ExtensionMember { extension, .. }) => s
                        .outline
                        .extensions
                        .get(extension.0 as usize)
                        .is_some_and(|d| is_nullable(table, d.on)),
                    _ => false,
                };
                match parent {
                    ParenParent::PropertyAccess(p) => {
                        let ExprKind::Property { name, .. } = &a.expr(p).kind else {
                            continue;
                        };
                        let n = interner.resolve(name.sym);
                        if n == "hashCode" || n == "runtimeType" || is_nullable_extension(p) {
                            continue;
                        }
                    }
                    ParenParent::MethodInvocation { name_expr, .. } => {
                        let ExprKind::Property { name, .. } = &a.expr(name_expr).kind else {
                            continue;
                        };
                        let n = interner.resolve(name.sym);
                        if n == "noSuchMethod"
                            || n == "toString"
                            || is_nullable_extension(name_expr)
                        {
                            continue;
                        }
                    }
                    ParenParent::BangPostfix => continue,
                    _ => {
                        if matches!(
                            inner_kind,
                            ExprKind::Index {
                                null_aware: true,
                                ..
                            }
                        ) && matches!(
                            parent,
                            ParenParent::Conditional { then_branch: true } | ParenParent::MapKey
                        ) {
                            continue;
                        }
                    }
                }
                report(&c::UNNECESSARY_PARENTHESIS, e.span, &[]);
                continue;
            }
            if matches!(inner_kind, ExprKind::FunctionExpression(_))
                && matches!(
                    parent,
                    ParenParent::MethodInvocation { .. }
                        | ParenParent::PropertyAccess(_)
                        | ParenParent::Binary
                        | ParenParent::Index
                )
            {
                continue;
            }
            // `ConstructorReference`: a constructor tear-off.
            let constructor_reference =
                !matches!(
                    inner_kind,
                    ExprKind::Call { .. } | ExprKind::InstanceCreation { .. }
                ) && matches!(s.body.get_resolved(x), Some(Resolved::Constructor(_)));
            if constructor_reference
                && !matches!(
                    parent,
                    ParenParent::FunctionInvocation {
                        has_type_args: true
                    }
                )
            {
                report(&c::UNNECESSARY_PARENTHESIS, e.span, &[]);
                continue;
            }
            let ancestors: &[Node] = parents.ancestors.get(&node).map(|v| &v[..]).unwrap_or(&[]);
            // `a..b = (c..d)`: the cascade, or the cascade before the statement.
            if matches!(inner_kind, ExprKind::Cascade { .. }) {
                continue;
            }
            let mut in_cascade = false;
            for n in ancestors.iter().rev() {
                match n {
                    Node::Stmt(_) => break,
                    Node::Expr(p) if matches!(a.expr(*p).kind, ExprKind::Cascade { .. }) => {
                        in_cascade = true;
                        break;
                    }
                    _ => {}
                }
            }
            if in_cascade {
                continue;
            }
            // `isBareInConstructorFieldInitializer` and a closure inside.
            let mut nu = false;
            for n in ancestors.iter().rev() {
                match n {
                    Node::Function(_) => break,
                    Node::Expr(p) => {
                        if let ExprKind::Call { target, .. } = &a.expr(*p).kind
                            && is_method_invocation(s, a, *target)
                        {
                            break;
                        }
                    }
                    Node::Member(m) => {
                        if let MemberKind::Constructor(k) = &a.member(*m).kind {
                            nu = k.initializers.iter().any(|i| matches!(i, Initializer::Field { value, .. } if { let sp = a.expr(*value).span; sp.start <= e.span.start && e.span.end <= sp.end }));
                        }
                        break;
                    }
                    _ => {}
                }
            }
            if nu
                && a.exprs.iter().any(|y| {
                    matches!(y.kind, ExprKind::FunctionExpression(_))
                        && y.span.start >= e.span.start
                        && y.span.end <= e.span.end
                })
            {
                continue;
            }
            if matches!(
                inner_kind,
                ExprKind::Binary {
                    op: BinaryOp::Eq | BinaryOp::NotEq,
                    ..
                }
            ) && matches!(
                parent,
                ParenParent::Assignment
                    | ParenParent::Variable(_)
                    | ParenParent::Return
                    | ParenParent::Yield
                    | ParenParent::FieldInitializer
            ) {
                continue;
            }
            if parent == ParenParent::ExpressionStatement
                && matches!(inner_kind, ExprKind::Switch { .. })
            {
                continue;
            }
            let is_argument = matches!(parent, ParenParent::Argument { .. } | ParenParent::Named);
            if has_space(s, a, x)
                && !matches!(
                    parent,
                    ParenParent::Assignment
                        | ParenParent::FieldInitializer
                        | ParenParent::Variable(_)
                        | ParenParent::ExpressionBody
                        | ParenParent::Return
                        | ParenParent::Yield
                )
                && !is_argument
            {
                continue;
            }
            if parent.is_expression() {
                if matches!(
                    parent,
                    ParenParent::Binary
                        | ParenParent::Conditional { .. }
                        | ParenParent::Cascade
                        | ParenParent::As
                        | ParenParent::Is
                ) {
                    continue;
                }
                // `PrefixedIdentifier`: `a.b` of a simple identifier.
                let prefixed_identifier = matches!(inner_kind, ExprKind::Property { target, null_aware: false, .. } if matches!(a.expr(*target).kind, ExprKind::Identifier(_)));
                if matches!(parent, ParenParent::FunctionInvocation { .. }) && !prefixed_identifier
                {
                    continue;
                }
                let parent_node = match parent {
                    ParenParent::MethodInvocation { call_expr, .. } => Some(call_expr),
                    ParenParent::PropertyAccess(p) => Some(p),
                    _ => None,
                };
                if let Some(np) = parent_node {
                    let grandparent_prefix = matches!(parents.expr.get(&np).copied(), Some(Node::Expr(g)) if matches!(a.expr(g).kind, ExprKind::Unary { op: UnaryOp::Neg | UnaryOp::Not | UnaryOp::BitNot | UnaryOp::PrefixInc | UnaryOp::PrefixDec, .. }));
                    if grandparent_prefix && has_space(s, a, x) {
                        continue;
                    }
                    if matches!(inner_kind, ExprKind::Unary { .. }) {
                        continue;
                    }
                }
                // `({1, 2}).forEach(print);`: without the parentheses it becomes a block.
                if matches!(inner_kind, ExprKind::SetOrMap { .. }) {
                    let stmt = ancestors.iter().rev().find_map(|n| match n {
                        Node::Stmt(st) if matches!(a.stmt(*st).kind, StmtKind::Expression(_)) => {
                            Some(*st)
                        }
                        _ => None,
                    });
                    if stmt.is_some_and(|st| a.stmt(st).span.start == e.span.start) {
                        continue;
                    }
                }
            }
            report(&c::UNNECESSARY_PARENTHESIS, e.span, &[]);
        }
    }

    out
}
