//! A walk over a unit's tree with the stack of ancestors, for the lint rules
//! that ask "inside what" (the analyzer's `thisOrAncestorMatching`): the
//! DartForge tree lives in arenas and the nodes do not know their parent.
//!
//! The visit is pre-order. It covers the declarations, the members, the
//! functions (declared, local and literal), the statements, the expressions
//! and the patterns; also the annotations' arguments and the parameters'
//! default values. Types are not nodes of the walk. Annotations and the
//! arguments of enum constants are nodes only to mark the constant context of
//! the arguments.
//!
//! Written without compiling or running (2026-10-05).

use crate::Unit;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, Ast, CollectionElement, DeclId, DeclKind, ExprId, ExprKind, ForInTarget, ForInit,
    FunctionBody, FunctionId, Initializer, ListPatternElement, MemberId, MemberKind, PatternId,
    PatternKind, StmtId, StmtKind, StringPart,
};

/// A node of the walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Node {
    Decl(DeclId),
    Member(MemberId),
    Function(FunctionId),
    Stmt(StmtId),
    Expr(ExprId),
    Pattern(PatternId),
    /// An annotation with arguments (its range): the arguments are the
    /// children. Constant context.
    Annotation(Span),
    /// The arguments of an enum constant (the constant's range). Constant
    /// context.
    EnumArguments(Span),
}

struct Walker<'a, 'f> {
    a: &'a Ast,
    stack: Vec<Node>,
    f: &'f mut dyn FnMut(Node, &[Node]),
}

/// Calls `f(node, ancestors)` for each node of the unit; `ancestors` goes from
/// the top-level declaration to the node's parent.
pub fn walk(u: Unit<'_>, f: &mut dyn FnMut(Node, &[Node])) {
    let mut x = Walker {
        a: u.ast,
        stack: Vec::new(),
        f,
    };
    for &d in u.unit.declarations.iter() {
        x.decl(d);
    }
}

impl<'a> Walker<'a, '_> {
    fn enter(&mut self, node: Node) {
        (self.f)(node, &self.stack);
        self.stack.push(node);
    }

    fn exit(&mut self) {
        self.stack.pop();
    }

    fn annotations(&mut self, annotations: &[ast::Annotation]) {
        for m in annotations {
            if let Some(args) = &m.arguments {
                self.enter(Node::Annotation(m.span));
                self.arguments(args);
                self.exit();
            }
        }
    }

    fn arguments(&mut self, args: &ast::Arguments) {
        for x in args.args.iter() {
            self.expr(x.value);
        }
    }

    fn params(&mut self, ps: &[ast::Parameter]) {
        for p in ps {
            self.annotations(&p.metadata);
            if let Some(d) = p.default_value {
                self.expr(d);
            }
            if let Some(inner) = &p.function_parameters {
                self.params(inner);
            }
        }
    }

    fn list(&mut self, l: &ast::VariableList) {
        for v in l.variables.iter() {
            if let Some(i) = v.initializer {
                self.expr(i);
            }
        }
    }

    fn body(&mut self, b: &FunctionBody) {
        match b {
            FunctionBody::Block(s) => self.stmt(*s),
            FunctionBody::Expression(e) => self.expr(*e),
            FunctionBody::Empty | FunctionBody::Native(_) => {}
        }
    }

    fn members(&mut self, primary: Option<MemberId>, members: &[MemberId]) {
        if let Some(p) = primary
            && !members.contains(&p)
        {
            self.member(p);
        }
        for &m in members {
            self.member(m);
        }
    }

    fn decl(&mut self, d: DeclId) {
        let a: &'a Ast = self.a;
        let n = a.decl(d);
        self.enter(Node::Decl(d));
        self.annotations(&n.metadata);
        match &n.kind {
            DeclKind::Class(x) => self.members(x.primary_constructor, &x.members),
            DeclKind::Mixin(x) => self.members(None, &x.members),
            DeclKind::Enum(x) => {
                for k in x.constants.iter() {
                    self.annotations(&k.metadata);
                    if let Some(args) = &k.arguments {
                        self.enter(Node::EnumArguments(k.span));
                        self.arguments(args);
                        self.exit();
                    }
                }
                self.members(x.primary_constructor, &x.members);
            }
            DeclKind::Extension(x) => self.members(None, &x.members),
            DeclKind::ExtensionType(x) => {
                self.annotations(&x.representation_metadata);
                self.members(None, &x.members);
            }
            DeclKind::Typedef(_) => {}
            DeclKind::Function(f) => self.function(*f),
            DeclKind::Variables(l) => self.list(l),
        }
        self.exit();
    }

    fn member(&mut self, m: MemberId) {
        let a: &'a Ast = self.a;
        let n = a.member(m);
        self.enter(Node::Member(m));
        self.annotations(&n.metadata);
        match &n.kind {
            MemberKind::Field(l) => self.list(l),
            MemberKind::Method(f) => self.function(*f),
            MemberKind::Constructor(k) => {
                self.params(&k.parameters);
                for i in k.initializers.iter() {
                    match i {
                        Initializer::Field { value, .. } => self.expr(*value),
                        Initializer::Super { arguments, .. }
                        | Initializer::Redirect { arguments, .. } => self.arguments(arguments),
                        Initializer::Assert {
                            condition, message, ..
                        } => {
                            self.expr(*condition);
                            if let Some(m) = message {
                                self.expr(*m);
                            }
                        }
                    }
                }
                self.body(&k.body);
            }
        }
        self.exit();
    }

    fn function(&mut self, f: FunctionId) {
        let a: &'a Ast = self.a;
        let n = a.function(f);
        self.enter(Node::Function(f));
        if let Some(ps) = &n.parameters {
            self.params(ps);
        }
        self.body(&n.body);
        self.exit();
    }

    fn for_init(&mut self, i: &ForInit) {
        match i {
            ForInit::Variables(l) => self.list(l),
            ForInit::Expression(e) => self.expr(*e),
            ForInit::Pattern { pattern, value, .. } => {
                self.pattern(*pattern);
                self.expr(*value);
            }
        }
    }

    fn for_target(&mut self, t: &ForInTarget) {
        match t {
            ForInTarget::Declared { .. } => {}
            ForInTarget::Pattern { pattern, .. } => self.pattern(*pattern),
            ForInTarget::Expression(e) => self.expr(*e),
        }
    }

    fn stmt(&mut self, s: StmtId) {
        let a: &'a Ast = self.a;
        let n = a.stmt(s);
        self.enter(Node::Stmt(s));
        match &n.kind {
            StmtKind::Block(ss) => {
                for &x in ss.iter() {
                    self.stmt(x);
                }
            }
            StmtKind::Variables(l) => self.list(l),
            StmtKind::PatternVariables { pattern, value, .. } => {
                self.pattern(*pattern);
                self.expr(*value);
            }
            StmtKind::Function(f) => self.function(*f),
            StmtKind::Expression(e) => self.expr(*e),
            StmtKind::If {
                condition,
                case_pattern,
                guard,
                then,
                else_,
            } => {
                self.expr(*condition);
                if let Some(p) = case_pattern {
                    self.pattern(*p);
                }
                if let Some(g) = guard {
                    self.expr(*g);
                }
                self.stmt(*then);
                if let Some(e) = else_ {
                    self.stmt(*e);
                }
            }
            StmtKind::For {
                init,
                condition,
                updates,
                body,
                ..
            } => {
                if let Some(i) = init {
                    self.for_init(i);
                }
                if let Some(c) = condition {
                    self.expr(*c);
                }
                for &x in updates.iter() {
                    self.expr(x);
                }
                self.stmt(*body);
            }
            StmtKind::ForIn {
                target,
                iterable,
                body,
                ..
            } => {
                self.for_target(target);
                self.expr(*iterable);
                self.stmt(*body);
            }
            StmtKind::While { condition, body } => {
                self.expr(*condition);
                self.stmt(*body);
            }
            StmtKind::DoWhile { body, condition } => {
                self.stmt(*body);
                self.expr(*condition);
            }
            StmtKind::Switch { value, cases } => {
                self.expr(*value);
                for k in cases.iter() {
                    if let Some(p) = k.pattern {
                        self.pattern(p);
                    }
                    if let Some(g) = k.guard {
                        self.expr(g);
                    }
                    for &x in k.body.iter() {
                        self.stmt(x);
                    }
                }
            }
            StmtKind::Break(_) | StmtKind::Continue(_) | StmtKind::Empty => {}
            StmtKind::Return(e) => {
                if let Some(e) = e {
                    self.expr(*e);
                }
            }
            StmtKind::Yield { value, .. } => self.expr(*value),
            StmtKind::Try {
                body,
                catches,
                finally_,
            } => {
                self.stmt(*body);
                for k in catches.iter() {
                    self.stmt(k.body);
                }
                if let Some(f) = finally_ {
                    self.stmt(*f);
                }
            }
            StmtKind::Labeled { body, .. } => self.stmt(*body),
            StmtKind::Assert { condition, message } => {
                self.expr(*condition);
                if let Some(m) = message {
                    self.expr(*m);
                }
            }
        }
        self.exit();
    }

    fn element(&mut self, e: &CollectionElement) {
        match e {
            CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => {
                self.expr(*x)
            }
            CollectionElement::MapEntry { key, value, .. } => {
                self.expr(*key);
                self.expr(*value);
            }
            CollectionElement::Spread { value, .. } => self.expr(*value),
            CollectionElement::If {
                condition,
                case_pattern,
                guard,
                then,
                else_,
            } => {
                self.expr(*condition);
                if let Some(p) = case_pattern {
                    self.pattern(*p);
                }
                if let Some(g) = guard {
                    self.expr(*g);
                }
                self.element(then);
                if let Some(x) = else_ {
                    self.element(x);
                }
            }
            CollectionElement::For {
                init,
                condition,
                updates,
                body,
                ..
            } => {
                if let Some(i) = init {
                    self.for_init(i);
                }
                if let Some(c) = condition {
                    self.expr(*c);
                }
                for &x in updates.iter() {
                    self.expr(x);
                }
                self.element(body);
            }
            CollectionElement::ForIn {
                target,
                iterable,
                body,
                ..
            } => {
                self.for_target(target);
                self.expr(*iterable);
                self.element(body);
            }
        }
    }

    fn expr(&mut self, e: ExprId) {
        let a: &'a Ast = self.a;
        let n = a.expr(e);
        self.enter(Node::Expr(e));
        match &n.kind {
            ExprKind::Int(_)
            | ExprKind::Double(_)
            | ExprKind::Bool(_)
            | ExprKind::Null
            | ExprKind::Symbol(_)
            | ExprKind::Identifier(_)
            | ExprKind::This
            | ExprKind::Super
            | ExprKind::CascadeTarget
            | ExprKind::Rethrow
            | ExprKind::DotShorthand { .. } => {}
            ExprKind::String(lit) => {
                for part in lit.parts.iter() {
                    if let StringPart::Interpolation(x) = part {
                        self.expr(*x);
                    }
                }
            }
            ExprKind::Parenthesized(x) | ExprKind::Await(x) | ExprKind::Throw(x) => self.expr(*x),
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                for el in elements.iter() {
                    self.element(el);
                }
            }
            ExprKind::Record {
                positional, named, ..
            } => {
                for &x in positional.iter() {
                    self.expr(x);
                }
                for (_, x) in named.iter() {
                    self.expr(*x);
                }
            }
            ExprKind::InstanceCreation { arguments, .. } => self.arguments(arguments),
            ExprKind::FunctionExpression(f) => self.function(*f),
            ExprKind::Property { target, .. } | ExprKind::TypeArguments { target, .. } => {
                self.expr(*target)
            }
            ExprKind::Index { target, index, .. } => {
                self.expr(*target);
                self.expr(*index);
            }
            ExprKind::Call { target, arguments } => {
                self.expr(*target);
                self.arguments(arguments);
            }
            ExprKind::Unary { operand, .. } => self.expr(*operand),
            ExprKind::Binary { left, right, .. } => {
                self.expr(*left);
                self.expr(*right);
            }
            ExprKind::Conditional {
                condition,
                then,
                else_,
            } => {
                self.expr(*condition);
                self.expr(*then);
                self.expr(*else_);
            }
            ExprKind::Is { value, .. } | ExprKind::As { value, .. } => self.expr(*value),
            ExprKind::Assign { target, value, .. } => {
                self.expr(*target);
                self.expr(*value);
            }
            ExprKind::PatternAssign { pattern, value } => {
                self.pattern(*pattern);
                self.expr(*value);
            }
            ExprKind::Cascade {
                target, sections, ..
            } => {
                self.expr(*target);
                for &x in sections.iter() {
                    self.expr(x);
                }
            }
            ExprKind::Switch { value, cases } => {
                self.expr(*value);
                for k in cases.iter() {
                    self.pattern(k.pattern);
                    if let Some(g) = k.guard {
                        self.expr(g);
                    }
                    self.expr(k.body);
                }
            }
        }
        self.exit();
    }

    fn pattern(&mut self, p: PatternId) {
        let a: &'a Ast = self.a;
        let n = a.pattern(p);
        self.enter(Node::Pattern(p));
        match &n.kind {
            PatternKind::Wildcard { .. } | PatternKind::Variable { .. } => {}
            PatternKind::Constant(e) | PatternKind::Relational { value: e, .. } => self.expr(*e),
            PatternKind::Or(x, y) | PatternKind::And(x, y) => {
                self.pattern(*x);
                self.pattern(*y);
            }
            PatternKind::NullCheck(x)
            | PatternKind::NullAssert(x)
            | PatternKind::Parenthesized(x)
            | PatternKind::Cast { pattern: x, .. } => self.pattern(*x),
            PatternKind::List { elements, .. } => {
                for el in elements.iter() {
                    match el {
                        ListPatternElement::Pattern(x) | ListPatternElement::Rest(Some(x)) => {
                            self.pattern(*x)
                        }
                        ListPatternElement::Rest(None) => {}
                    }
                }
            }
            PatternKind::Map { entries, .. } => {
                for en in entries.iter() {
                    self.expr(en.key);
                    self.pattern(en.value);
                }
            }
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                for c in fields.iter() {
                    self.pattern(c.pattern);
                }
            }
        }
        self.exit();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dartforge_intern::Interner;

    #[test]
    fn ancestor_stack() {
        let source = "void f() {\n  try {\n  } finally {\n    g(() => 1);\n  }\n}\n";
        let mut names = Interner::new();
        let p = dartforge_frontend::parser::parse(source, &mut names);
        let u = Unit {
            ast: &p.ast,
            unit: &p.unit,
            source,
        };
        let mut ints = Vec::new();
        walk(u, &mut |node, stack| {
            if let Node::Expr(e) = node
                && matches!(p.ast.expr(e).kind, ExprKind::Int(_))
            {
                ints.push(
                    stack
                        .iter()
                        .filter(|n| matches!(n, Node::Function(_)))
                        .count(),
                );
                assert!(matches!(stack[0], Node::Decl(_)));
            }
        });
        // The `1` is inside `f` and the function literal.
        assert_eq!(ints, vec![2]);
    }
}
