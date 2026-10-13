//! The twenty-seventh batch of lint rules
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), written directly from the 3.6.2
//! emitters (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//!
//! * `cascade_invocations`: in each block with two or more statements, the
//!   statement that could be joined to the previous one by a cascade
//!   (`_CascadableExpression`), by the analyzer's `canonicalElement` (an
//!   accessor becomes the variable; the simple left side of an assignment has
//!   no element) and by the critical dependencies (the element of the
//!   previous declaration or assignment used in the arguments, in the right
//!   side or in the sections of the current one).
//! * `require_trailing_commas`: the argument lists (of calls, creations,
//!   annotations, enum constants, `super(…)`/`this(…)`), parameter lists,
//!   `assert` lists and collection literals that span more than one line
//!   without a trailing comma, with the exceptions for the last node (block
//!   closure, string, closure call on one line, collection), by the lexer's
//!   tokens.
//!
//! Written without compiling or running (2026-10-05).

use super::codigos_g as c;
use super::rules::LintReport;
use super::rules16::is_method_invocation;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, ExtensionId, FunctionElementId, FunctionKind, LibraryId, VariableId};
use dartforge_frontend::ast::{self, Ast, AssignOp, CollectionElement, ExprId, ExprKind, FunctionBody, Initializer, MemberKind, StmtKind};
use dartforge_frontend::token::{Kind, Op, Token};
use dartforge_intern::{Interner, SymbolId};
use dartforge_types::resolved::{MemberRef, Resolved};
use std::collections::HashSet;

/// The `canonicalElement` of an element.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Canonical {
    Local(usize),
    Parameter(usize),
    Variable(VariableId),
    Function(FunctionElementId),
    /// The getter/setter pair written without a variable in the model: the
    /// analyzer's synthetic variable.
    Accessor(Option<ClassId>, Option<ExtensionId>, LibraryId, SymbolId),
    Element(Element),
}

/// The parameter `name` of the innermost function or constructor that
/// contains `e` (the offset of the name in the declaration).
fn declared_parameter(a: &Ast, e: Span, name: SymbolId) -> Option<usize> {
    let mut best: Option<(usize, usize)> = None;
    let mut lists: Vec<(Span, &[ast::Parameter])> = Vec::new();
    for f in a.functions.iter() {
        if let Some(ps) = f.parameters.as_deref() {
            lists.push((f.span, ps));
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind {
            lists.push((m.span, &k.parameters[..]));
        }
    }
    for (sp, ps) in lists {
        if e.start < sp.start || e.end > sp.end {
            continue;
        }
        if let Some(n) = ps.iter().find_map(|p| p.name.filter(|n| n.sym == name)) {
            let len = sp.end - sp.start;
            if best.is_none_or(|(t, _)| len < t) {
                best = Some((len, n.span.start));
            }
        }
    }
    best.map(|(_, d)| d)
}

fn function_canonical(s: &super::Semantics<'_>, f: FunctionElementId) -> Canonical {
    let g = s.program.function(f);
    match g.kind {
        FunctionKind::Getter | FunctionKind::Setter => match g.variable {
            Some(v) => Canonical::Variable(v),
            None => Canonical::Accessor(g.class, g.extension, g.library, g.name),
        },
        _ => Canonical::Function(f),
    }
}

/// `AstNode.canonicalElement`: the identifier (simple or prefixed) and the
/// `PropertyAccess`, without parentheses.
fn canonical(s: &super::Semantics<'_>, a: &Ast, mut e: ExprId) -> Option<Canonical> {
    while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
        e = *x;
    }
    if !matches!(a.expr(e).kind, ExprKind::Identifier(_) | ExprKind::Property { .. }) {
        return None;
    }
    element_of(s, a, e)
}

/// The `staticElement?.canonicalElement` of the node `e` (identifier or
/// property name).
fn element_of(s: &super::Semantics<'_>, a: &Ast, e: ExprId) -> Option<Canonical> {
    Some(match s.body.get_resolved(e)? {
        Resolved::Local(_) => Canonical::Local(s.body.local_declaration(e)?),
        Resolved::Parameter { name, .. } => Canonical::Parameter(declared_parameter(a, a.expr(e).span, *name)?),
        Resolved::Member { member: MemberRef::Variable(v), .. } | Resolved::Element(Element::Variable(v)) => Canonical::Variable(*v),
        Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::Element(Element::Function(f)) => function_canonical(s, *f),
        Resolved::ExtensionMember { member, .. } => function_canonical(s, *member),
        Resolved::Constructor(f) => Canonical::Function(*f),
        Resolved::Element(x) => Canonical::Element(*x),
        _ => return None,
    })
}

/// `_CascadableExpression`.
#[derive(Clone, Default)]
struct Cascadable {
    element: Option<Canonical>,
    /// The ranges of the critical nodes.
    critical_spans: Vec<Span>,
    joins: bool,
    receives: bool,
    cascadable: bool,
    critical: bool,
}

fn from_statement(s: &super::Semantics<'_>, a: &Ast, st: &ast::Stmt) -> Cascadable {
    match &st.kind {
        StmtKind::Variables(l) => {
            let element = match &l.variables[..] {
                [v] if !v.initializer.is_some_and(|i| matches!(a.expr(i).kind, ExprKind::Await(_))) => Some(Canonical::Local(v.name.span.start)),
                _ => None,
            };
            Cascadable { element, receives: true, critical: true, ..Default::default() }
        }
        StmtKind::Expression(e) => {
            let mut e = *e;
            while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
                e = *x;
            }
            from_expression(s, a, e)
        }
        _ => Cascadable::default(),
    }
}

fn from_expression(s: &super::Semantics<'_>, a: &Ast, e: ExprId) -> Cascadable {
    match &a.expr(e).kind {
        ExprKind::Assign { op, target, value } => {
            let mut left = *target;
            while let ExprKind::Parenthesized(x) = &a.expr(left).kind {
                left = *x;
            }
            let critical_spans = vec![a.expr(*value).span];
            if matches!(a.expr(left).kind, ExprKind::Identifier(_)) {
                // The simple left side has no `staticElement`.
                return Cascadable { element: None, critical_spans, receives: *op != AssignOp::Compound(dartforge_frontend::ast::BinaryOp::IfNull), critical: true, ..Default::default() };
            }
            // Setter: the prefix of `a.b` (the `PropertyAccess` only with `.` and a
            // simple target).
            let variable = match &a.expr(left).kind {
                ExprKind::Property { target, null_aware: false, .. } if matches!(a.expr(*target).kind, ExprKind::Identifier(_)) => element_of(s, a, *target),
                _ => None,
            };
            let is_instance_variable = match variable {
                Some(Canonical::Local(_) | Canonical::Parameter(_)) => true,
                Some(Canonical::Variable(v)) => !s.program.variable(v).static_,
                _ => false,
            };
            Cascadable { element: variable, critical_spans, joins: true, receives: *op != AssignOp::Compound(dartforge_frontend::ast::BinaryOp::IfNull) && is_instance_variable, cascadable: true, critical: false }
        }
        ExprKind::Call { target, arguments } if is_method_invocation(s, a, *target) => {
            // `_getExecutableElementFromMethodInvocation`: only with `.`.
            let ExprKind::Property { target: recv, null_aware: false, .. } = &a.expr(*target).kind else { return Cascadable::default() };
            let non_static = match element_of(s, a, *target) {
                Some(Canonical::Function(f)) => {
                    let g = s.program.function(f);
                    !g.static_ && (g.class.is_some() || g.extension.is_some())
                }
                _ => false,
            };
            if !non_static {
                return Cascadable::default();
            }
            let simple = matches!(a.expr(*recv).kind, ExprKind::Identifier(_));
            Cascadable { element: canonical(s, a, *recv), critical_spans: vec![arguments.span], joins: simple, receives: simple, cascadable: true, critical: false }
        }
        ExprKind::Cascade { target, sections, .. } => {
            let simple = matches!(a.expr(*target).kind, ExprKind::Identifier(_));
            Cascadable {
                element: canonical(s, a, *target),
                critical_spans: sections.iter().map(|x| a.expr(*x).span).collect(),
                joins: simple,
                receives: simple,
                cascadable: true,
                critical: false,
            }
        }
        ExprKind::Property { target, null_aware: false, .. } => {
            if matches!(a.expr(*target).kind, ExprKind::Identifier(_)) {
                // `PrefixedIdentifier`.
                Cascadable { element: element_of(s, a, *target), joins: true, receives: true, cascadable: true, ..Default::default() }
            } else {
                // `PropertyAccess` with `.`.
                Cascadable { element: canonical(s, a, *target), cascadable: true, ..Default::default() }
            }
        }
        _ => Cascadable::default(),
    }
}

/// The line number of an offset (`LineInfo`: `\n`, `\r\n` and `\r`).
struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    fn new(source: &str) -> LineIndex {
        let b = source.as_bytes();
        let mut starts = vec![0];
        let mut i = 0;
        while i < b.len() {
            match b[i] {
                b'\n' => starts.push(i + 1),
                b'\r' => {
                    if b.get(i + 1) == Some(&b'\n') {
                        i += 1;
                    }
                    starts.push(i + 1);
                }
                _ => {}
            }
            i += 1;
        }
        LineIndex { starts }
    }

    fn line(&self, offset: usize) -> usize {
        self.starts.partition_point(|&x| x <= offset)
    }
}

/// The unit's tokens, to find the neighbours of an offset.
struct Tokens {
    t: Vec<Token>,
}

impl Tokens {
    /// The index of the first token that starts at `offset` or after.
    fn first_at(&self, offset: usize) -> usize {
        self.t.partition_point(|x| x.span.start < offset)
    }

    fn is_op(&self, i: usize, op: Op) -> bool {
        self.t.get(i).is_some_and(|x| x.kind == Kind::Op(op))
    }
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unidade<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let _ = interner;
    let mut out: Vec<LintReport> = Vec::new();
    let a = u.ast;
    let mut report = |code: &'static CodigoLint, span: Span| {
        out.push(LintReport { code, span, args: Vec::new() });
    };

    // `cascade_invocations`.
    if enabled("cascade_invocations")
        && let Some(s) = sem
    {
        for st in a.stmts.iter() {
            let StmtKind::Block(cmds) = &st.kind else { continue };
            if cmds.len() < 2 {
                continue;
            }
            let mut previous = Cascadable::default();
            for &k in cmds.iter() {
                let x = a.stmt(k);
                let current = from_statement(s, a, x);
                // `compatibleWith` and `_hasCriticalDependencies`.
                let compatible = current.element.is_some()
                    && previous.receives
                    && current.joins
                    && (current.cascadable || previous.cascadable)
                    && current.element == previous.element
                    && !(previous.critical
                        && current.critical_spans.iter().any(|sp| {
                            a.exprs.iter().enumerate().any(|(i, y)| {
                                y.span.start >= sp.start
                                    && y.span.end <= sp.end
                                    && matches!(y.kind, ExprKind::Identifier(_) | ExprKind::Property { .. })
                                    && element_of(s, a, ExprId(i as u32)) == previous.element
                            })
                        }));
                if compatible {
                    report(&c::CASCADE_INVOCATIONS, x.span);
                }
                previous = current;
            }
        }
    }

    // `require_trailing_commas`.
    if enabled("require_trailing_commas")
        && let Ok(t) = dartforge_frontend::lexer::lex(u.fonte)
    {
        let tk = Tokens { t };
        let lines = LineIndex::new(u.fonte);
        let same_line = |start: usize, end: usize| lines.line(start) == lines.line(end);
        let mut findings: HashSet<(usize, usize)> = HashSet::new();
        // `_checkTrailingComma`: `open` is the offset of the opening token,
        // `close` the closing token, `last` the range of the last node,
        // `exception` whether the last node allows an exception and `error`
        // the reported token.
        let mut check = |open: usize, close: Span, last: Span, exception: &dyn Fn() -> bool, error: Span| {
            let i = tk.first_at(last.end);
            if tk.is_op(i, Op::Comma) {
                return;
            }
            if same_line(open, close.end) {
                return;
            }
            if !same_line(last.start, last.end) && exception() {
                return;
            }
            findings.insert((error.start, error.end));
        };
        // The exception for an expression as the last node.
        let exception_of = |e: ExprId| -> bool {
            match &a.expr(e).kind {
                ExprKind::FunctionExpression(f) => matches!(a.function(*f).body, FunctionBody::Block(_)),
                ExprKind::String(_) => true,
                ExprKind::Call { target, arguments } if matches!(a.expr(*target).kind, ExprKind::FunctionExpression(_)) => {
                    same_line(arguments.span.start, arguments.span.end)
                }
                ExprKind::List { .. } | ExprKind::SetOrMap { .. } => true,
                _ => false,
            }
        };
        let close_of = |args: &ast::Arguments| Span { start: args.span.end.saturating_sub(1), end: args.span.end };
        // `visitArgumentList`.
        let mut lists: Vec<&ast::Arguments> = Vec::new();
        for e in a.exprs.iter() {
            match &e.kind {
                ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } => lists.push(arguments),
                _ => {}
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                for i in k.initializers.iter() {
                    if let Initializer::Super { arguments, .. } | Initializer::Redirect { arguments, .. } = i {
                        lists.push(arguments);
                    }
                }
            }
        }
        for d in a.decls.iter() {
            if let ast::DeclKind::Enum(x) = &d.kind {
                lists.extend(x.constants.iter().filter_map(|k| k.arguments.as_ref()));
            }
        }
        for m in dartforge_frontend::parents::all_annotations(a, u.unit) {
            if let Some(args) = &m.arguments {
                lists.push(args);
            }
        }
        for args in lists {
            let Some(last) = args.args.last() else { continue };
            let start = last.name.map_or(a.expr(last.value).span.start, |n| n.span.start);
            let sp = Span { start, end: a.expr(last.value).span.end };
            let named = last.name.is_some();
            let v = last.value;
            check(args.span.start, close_of(args), sp, &|| !named && exception_of(v), close_of(args));
        }
        // `visitAssertStatement` and `visitAssertInitializer`: the parentheses
        // after the `assert`.
        let mut asserts: Vec<(Span, ExprId, Option<ExprId>)> = Vec::new();
        for st in a.stmts.iter() {
            if let StmtKind::Assert { condition, message } = &st.kind {
                asserts.push((st.span, *condition, *message));
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                for i in k.initializers.iter() {
                    if let Initializer::Assert { span, condition, message } = i {
                        asserts.push((*span, *condition, *message));
                    }
                }
            }
        }
        for (sp, cond, msg) in asserts {
            let i = tk.first_at(sp.start) + 1;
            if !tk.is_op(i, Op::LParen) {
                continue;
            }
            let open = tk.t[i].span.start;
            let last = msg.unwrap_or(cond);
            let usp = a.expr(last).span;
            // The `)`: after the last node and the optional comma.
            let mut j = tk.first_at(usp.end);
            if tk.is_op(j, Op::Comma) {
                j += 1;
            }
            if !tk.is_op(j, Op::RParen) {
                continue;
            }
            let close = tk.t[j].span;
            check(open, close, usp, &|| exception_of(last), close);
        }
        // `visitFormalParameterList`: the error goes on the `]`/`}` when there is one.
        let mut params: Vec<&[ast::Parameter]> = Vec::new();
        fn with_nested<'a>(ps: &'a [ast::Parameter], v: &mut Vec<&'a [ast::Parameter]>) {
            v.push(ps);
            for p in ps {
                if let Some(inner) = &p.function_parameters {
                    with_nested(inner, v);
                }
            }
        }
        for f in a.functions.iter() {
            if let Some(ps) = f.parameters.as_deref() {
                with_nested(ps, &mut params);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                with_nested(&k.parameters, &mut params);
            }
        }
        for t in a.types.iter() {
            if let ast::TypeKind::Function { parameters, .. } = &t.kind {
                with_nested(parameters, &mut params);
            }
        }
        for d in a.decls.iter() {
            if let ast::DeclKind::Typedef(x) = &d.kind
                && let ast::TypedefKind::Legacy { parameters, .. } = &x.kind
            {
                with_nested(parameters, &mut params);
            }
        }
        for ps in params {
            let (Some(first), Some(last)) = (ps.first(), ps.last()) else { continue };
            // The `(`: before the first parameter, skipping the opening `[`/`{`.
            let mut i = tk.first_at(first.span.start);
            if i == 0 {
                continue;
            }
            i -= 1;
            if (tk.is_op(i, Op::LBracket) || tk.is_op(i, Op::LBrace)) && i > 0 {
                i -= 1;
            }
            if !tk.is_op(i, Op::LParen) {
                continue;
            }
            let open = tk.t[i].span.start;
            let mut j = tk.first_at(last.span.end);
            if tk.is_op(j, Op::Comma) {
                j += 1;
            }
            let mut delimiter = None;
            if tk.is_op(j, Op::RBracket) || tk.is_op(j, Op::RBrace) {
                delimiter = Some(tk.t[j].span);
                j += 1;
            }
            if !tk.is_op(j, Op::RParen) {
                continue;
            }
            let close = tk.t[j].span;
            // A parameter is none of the nodes with an exception.
            check(open, close, last.span, &|| false, delimiter.unwrap_or(close));
        }
        // `visitListLiteral` and `visitSetOrMapLiteral`.
        for e in a.exprs.iter() {
            let (ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. }) = &e.kind else { continue };
            let Some(last) = elements.last() else { continue };
            let (usp, expr) = match last {
                CollectionElement::Expression(x) => (a.expr(*x).span, Some(*x)),
                CollectionElement::NullAwareExpression(x) => (Span { start: tk.t[tk.first_at(a.expr(*x).span.start).saturating_sub(1)].span.start, end: a.expr(*x).span.end }, None),
                CollectionElement::MapEntry { key, value, .. } => (Span { start: a.expr(*key).span.start, end: a.expr(*value).span.end }, None),
                CollectionElement::Spread { value, .. } => {
                    let i = tk.first_at(a.expr(*value).span.start).saturating_sub(1);
                    (Span { start: tk.t[i].span.start, end: a.expr(*value).span.end }, None)
                }
                CollectionElement::If { .. } | CollectionElement::For { .. } | CollectionElement::ForIn { .. } => match element_end(a, last) {
                    Some(end) => (Span { start: element_start(a, &tk, u.fonte, last), end }, None),
                    None => continue,
                },
            };
            // The literal's `]`/`}`: the node's last token.
            let close = Span { start: e.span.end.saturating_sub(1), end: e.span.end };
            // The literal's `[`/`{`: after the type arguments and the `const`.
            let i = tk.first_at(e.span.start);
            let open = (i..tk.t.len()).find(|&k| tk.is_op(k, Op::LBracket) || tk.is_op(k, Op::LBrace)).map_or(e.span.start, |k| tk.t[k].span.start);
            check(open, close, usp, &|| expr.is_some_and(exception_of), close);
        }
        let mut v: Vec<(usize, usize)> = findings.into_iter().collect();
        v.sort();
        for (i, f) in v {
            report(&c::REQUIRE_TRAILING_COMMAS, Span { start: i, end: f });
        }
    }

    out
}

/// The start of a collection `if`/`for` element: the `if`/`for` token (or the
/// `await` of `await for`) before the condition.
fn element_start(a: &Ast, tk: &Tokens, source: &str, x: &CollectionElement) -> usize {
    let first = match x {
        CollectionElement::If { condition, .. } => a.expr(*condition).span.start,
        CollectionElement::For { .. } | CollectionElement::ForIn { .. } => {
            // The first expression inside the header.
            match first_in_for(a, x) {
                Some(p) => p,
                None => return 0,
            }
        }
        _ => return 0,
    };
    // Goes back to the `if`/`for`: the `(` before the condition and the token before it.
    let mut i = tk.first_at(first);
    while i > 0 {
        i -= 1;
        if let Kind::Keyword(k) = tk.t[i].kind
            && matches!(k, dartforge_frontend::token::Keyword::If | dartforge_frontend::token::Keyword::For)
        {
            if i > 0 && tk.t[i - 1].kind == Kind::Ident && &source[tk.t[i - 1].span.start..tk.t[i - 1].span.end] == "await" {
                return tk.t[i - 1].span.start;
            }
            return tk.t[i].span.start;
        }
    }
    0
}

fn first_in_for(a: &Ast, x: &CollectionElement) -> Option<usize> {
    match x {
        CollectionElement::ForIn { iterable, .. } => Some(a.expr(*iterable).span.start),
        CollectionElement::For { init, condition, updates, body, .. } => match init {
            Some(ast::ForInit::Expression(e)) => Some(a.expr(*e).span.start),
            Some(ast::ForInit::Variables(l)) => l.variables.first().map(|v| v.name.span.start),
            Some(ast::ForInit::Pattern { pattern, .. }) => Some(a.pattern(*pattern).span.start),
            None => condition.map(|c| a.expr(c).span.start).or_else(|| updates.first().map(|u| a.expr(*u).span.start)).or_else(|| element_end(a, body)),
        },
        _ => None,
    }
}

/// The end of a collection element.
fn element_end(a: &Ast, x: &CollectionElement) -> Option<usize> {
    Some(match x {
        CollectionElement::Expression(e) | CollectionElement::NullAwareExpression(e) => a.expr(*e).span.end,
        CollectionElement::MapEntry { value, .. } => a.expr(*value).span.end,
        CollectionElement::Spread { value, .. } => a.expr(*value).span.end,
        CollectionElement::If { then, else_, .. } => match else_ {
            Some(x) => element_end(a, x)?,
            None => element_end(a, then)?,
        },
        CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => element_end(a, body)?,
    })
}
