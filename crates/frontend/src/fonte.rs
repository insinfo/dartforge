//! A node's text like the analyzer 3.6.2 `toSource()` (and `toString()`):
//! the `ToSourceVisitor` (`analyzer/lib/src/dart/ast/to_source_visitor.dart`),
//! node by node, with the same separators. Leaf tokens (numbers, strings,
//! identifiers) come out with the source lexeme; the rest is recomposed, so
//! the source's whitespace and comments do not appear.
//!
//! What the tree here does not store and the visitor prints is read from
//! the source, at the exact place the parser consumed it: the name of a
//! record type's positional field, the `var`/`final` of a wildcard pattern,
//! the `const` of a constant pattern, the separator (`=` or `:`) of a
//! parameter's default value, the bounds of adjacent strings and of
//! interpolations. The metadata of local declarations comes from
//! `Ast::metadados_locais`.
//! Written without compiling or running (2026-10-05).

use crate::ast::{
    Annotation, Arguments, AssignOp, Ast, BinaryOp, CatchClause, CollectionElement, CreationKeyword, ExprId, ExprKind, ForInTarget, ForInit, FunctionBody,
    FunctionId, AsyncModifier, ListPatternElement, Parameter, ParameterKind, PatternId, PatternKind, StmtId, StmtKind, StringLit, TypeId, TypeKind,
    TypeParameter, UnaryOp, VariableList,
};
use dartforge_intern::Interner;

/// Skips whitespace and comments (block comments nest) from `i`.
pub fn pular_brancos(b: &[u8], mut i: usize) -> usize {
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if b[i.min(b.len())..].starts_with(b"//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if b[i.min(b.len())..].starts_with(b"/*") {
            let mut level = 0usize;
            while i < b.len() {
                if b[i..].starts_with(b"/*") {
                    level += 1;
                    i += 2;
                } else if b[i..].starts_with(b"*/") {
                    level -= 1;
                    i += 2;
                    if level == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else {
            return i;
        }
    }
}

fn identifier_part(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$'
}

/// The source, from `i`, starts with the whole word `p`.
pub fn palavra_em(fonte: &str, i: usize, p: &str) -> bool {
    let b = fonte.as_bytes();
    b.get(i..).is_some_and(|r| r.starts_with(p.as_bytes())) && !b.get(i + p.len()).is_some_and(|&c| identifier_part(c))
}

/// The end of a `FunctionTypedFormalParameter`/`FieldFormalParameter` node
/// with a list (without the default value of the enclosing
/// `DefaultFormalParameter`): the `)` that closes the list starting after
/// the name (and the type parameters), and the `?` that follows it.
pub fn fim_do_parametro_funcao(fonte: &str, after_name: usize) -> usize {
    let b = fonte.as_bytes();
    let mut i = after_name;
    let mut level = 0usize;
    while i < b.len() {
        let j = pular_brancos(b, i);
        if j != i {
            i = j;
            continue;
        }
        match b[i] {
            b'(' | b'[' | b'{' | b'<' => level += 1,
            b')' | b']' | b'}' | b'>' => {
                level = level.saturating_sub(1);
                if level == 0 && b[i] == b')' {
                    let k = pular_brancos(b, i + 1);
                    return if b.get(k) == Some(&b'?') { k + 1 } else { i + 1 };
                }
            }
            b'\'' | b'"' => {
                let q = b[i];
                i += 1;
                while i < b.len() && b[i] != q {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    b.len()
}

/// The text of type `t`.
pub fn de_tipo(a: &Ast, fonte: &str, interner: &Interner, t: TypeId) -> String {
    let mut f = Impressor::novo(a, fonte, interner);
    f.kind(t);
    f.output
}

/// The text of expression `e`.
pub fn de_expr(a: &Ast, fonte: &str, interner: &Interner, e: ExprId) -> String {
    let mut f = Impressor::novo(a, fonte, interner);
    f.expr(e);
    f.output
}

/// The text of statement `s`.
pub fn from_stmt(a: &Ast, fonte: &str, interner: &Interner, s: StmtId) -> String {
    let mut f = Impressor::novo(a, fonte, interner);
    f.stmt(s);
    f.output
}

/// The text of pattern `p`.
pub fn de_padrao(a: &Ast, fonte: &str, interner: &Interner, p: PatternId) -> String {
    let mut f = Impressor::novo(a, fonte, interner);
    f.pattern(p);
    f.output
}

/// The text of annotation `m`.
pub fn from_annotation(a: &Ast, fonte: &str, interner: &Interner, m: &Annotation) -> String {
    let mut f = Impressor::novo(a, fonte, interner);
    f.annotation(m);
    f.output
}

/// The text of the type parameter list `ps` (the `TypeParameterList`; empty
/// when there are none).
pub fn de_parametros_de_tipo(a: &Ast, fonte: &str, interner: &Interner, ps: &[TypeParameter]) -> String {
    let mut f = Impressor::novo(a, fonte, interner);
    f.type_parameters_(ps);
    f.output
}

/// The text of the parameter list `ps` (the `FormalParameterList`).
pub fn de_parametros(a: &Ast, fonte: &str, interner: &Interner, ps: &[Parameter]) -> String {
    let mut f = Impressor::novo(a, fonte, interner);
    f.parameters_(ps);
    f.output
}

fn binary_lexeme(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::TruncDiv => "~/",
        BinaryOp::Rem => "%",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
        BinaryOp::UShr => ">>>",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::Eq => "==",
        BinaryOp::NotEq => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Gt => ">",
        BinaryOp::LtEq => "<=",
        BinaryOp::GtEq => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
        BinaryOp::IfNull => "??",
        BinaryOp::NaoBinario => "~",
    }
}

struct Impressor<'a> {
    a: &'a Ast,
    fonte: &'a str,
    interner: &'a Interner,
    output: String,
    /// The operator of the cascade section being printed (`..` or `?..`): the
    /// `CascadeTarget` prints as it.
    cascade: &'static str,
}

impl<'a> Impressor<'a> {
    fn novo(a: &'a Ast, fonte: &'a str, interner: &'a Interner) -> Self {
        Impressor { a, fonte, interner, output: String::new(), cascade: ".." }
    }

    fn w(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn nome(&mut self, n: crate::ast::Name) {
        let s = self.interner.resolve(n.sym);
        self.output.push_str(s);
    }

    fn excerpt(&mut self, s: crate::Span) {
        let t = self.fonte.get(s.start..s.end).unwrap_or("");
        self.output.push_str(t);
    }

    /// `_visitNodeList(metadata, separator: ' ', suffix: ' ')`.
    fn metadata(&mut self, ms: &[Annotation]) {
        for m in ms {
            self.annotation(m);
            self.w(" ");
        }
    }

    /// The metadata of a local declaration (`Ast::metadados_locais`).
    fn metadata_local(&mut self, s: StmtId) {
        let a = self.a;
        if let Some((_, ms)) = a.metadados_locais.iter().find(|(x, _)| *x == s) {
            self.metadata(ms);
        }
    }

    // -----------------------------------------------------------------
    // Types
    // -----------------------------------------------------------------

    fn type_arguments_(&mut self, ts: &[TypeId]) {
        if ts.is_empty() {
            return;
        }
        self.w("<");
        for (k, &t) in ts.iter().enumerate() {
            if k > 0 {
                self.w(", ");
            }
            self.kind(t);
        }
        self.w(">");
    }

    fn type_parameters_(&mut self, ps: &[TypeParameter]) {
        if ps.is_empty() {
            return;
        }
        self.w("<");
        for (k, p) in ps.iter().enumerate() {
            if k > 0 {
                self.w(", ");
            }
            self.metadata(&p.metadata);
            if let Some((_, s)) = p.variance {
                self.excerpt(s);
                self.w(" ");
            }
            self.nome(p.name);
            if let Some(b) = p.bound {
                self.w(" extends ");
                self.kind(b);
            }
        }
        self.w(">");
    }

    fn kind(&mut self, t: TypeId) {
        let a = self.a;
        let ty = a.ty(t);
        match &ty.kind {
            TypeKind::Named { name, args } => {
                for (k, n) in name.iter().enumerate() {
                    if k > 0 {
                        self.w(".");
                    }
                    self.nome(*n);
                }
                self.type_arguments_(args);
            }
            TypeKind::Void => self.w("void"),
            TypeKind::Function { return_type, type_params, parameters } => {
                if let Some(r) = return_type {
                    self.kind(*r);
                }
                self.w(" Function");
                self.type_parameters_(type_params);
                self.parameters_(parameters);
            }
            TypeKind::Record { positional, named } => {
                self.w("(");
                for (k, &p) in positional.iter().enumerate() {
                    if k > 0 {
                        self.w(", ");
                    }
                    self.kind(p);
                    // The positional field's name: the identifier the parser consumed right
                    // after the type.
                    let b = self.fonte.as_bytes();
                    let i = pular_brancos(b, a.ty(p).span.end);
                    if b.get(i).is_some_and(|&c| identifier_part(c) && !c.is_ascii_digit()) {
                        let mut j = i;
                        while j < b.len() && identifier_part(b[j]) {
                            j += 1;
                        }
                        self.w(" ");
                        let nome = &self.fonte[i..j];
                        self.output.push_str(nome);
                    }
                }
                if !named.is_empty() {
                    if !positional.is_empty() {
                        self.w(", ");
                    }
                    self.w("{");
                    for (k, (n, t)) in named.iter().enumerate() {
                        if k > 0 {
                            self.w(", ");
                        }
                        self.kind(*t);
                        self.w(" ");
                        self.nome(*n);
                    }
                    self.w("}");
                }
                self.w(")");
            }
        }
        if ty.nullable {
            self.w("?");
        }
    }

    // -----------------------------------------------------------------
    // Parameters
    // -----------------------------------------------------------------

    /// `visitFormalParameterList`.
    fn parameters_(&mut self, ps: &[Parameter]) {
        self.w("(");
        let mut group_end: Option<&'static str> = None;
        for (k, p) in ps.iter().enumerate() {
            if k > 0 {
                self.w(", ");
            }
            if group_end.is_none() && p.kind != ParameterKind::Required {
                if p.kind == ParameterKind::Named {
                    group_end = Some("}");
                    self.w("{");
                } else {
                    group_end = Some("]");
                    self.w("[");
                }
            }
            self.parameter(p);
        }
        if let Some(g) = group_end {
            self.w(g);
        }
        self.w(")");
    }

    fn parameter(&mut self, p: &Parameter) {
        self.metadata(&p.metadata);
        if p.required {
            self.w("required ");
        }
        if p.covariant {
            self.w("covariant ");
        }
        let function = p.function_parameters.is_some();
        if p.this_ || p.super_ || !function {
            // `keyword`: the `FunctionTypedFormalParameter` does not have it.
            if p.final_ {
                self.w("final ");
            } else if p.const_ {
                self.w("const ");
            } else if p.var_ {
                self.w("var ");
            }
        }
        if p.this_ || p.super_ {
            if let Some(t) = p.ty {
                self.kind(t);
                self.w(" ");
            }
            self.w(if p.this_ { "this." } else { "super." });
            if let Some(n) = p.name {
                self.nome(n);
            }
            self.type_parameters_(&p.function_type_params);
            if let Some(fs) = &p.function_parameters {
                self.parameters_(fs);
            }
        } else if let Some(fs) = &p.function_parameters {
            if let Some(t) = p.ty {
                self.kind(t);
                self.w(" ");
            }
            if let Some(n) = p.name {
                self.nome(n);
            }
            self.type_parameters_(&p.function_type_params);
            self.parameters_(fs);
            if p.function_nullable {
                self.w("?");
            }
        } else {
            if let Some(t) = p.ty {
                self.kind(t);
            }
            if p.ty.is_some() && p.name.is_some() {
                self.w(" ");
            }
            if let Some(n) = p.name {
                self.nome(n);
            }
        }
        // `visitDefaultFormalParameter`: the separator as written.
        if let Some(v) = p.default_value {
            let start_offset = self.a.expr(v).span.start;
            let b = self.fonte.as_bytes();
            // The separator is the last `=` or `:` before the value.
            let mut i = start_offset;
            let mut sep = b'=';
            while i > p.span.start {
                i -= 1;
                if b[i] == b'=' || b[i] == b':' {
                    sep = b[i];
                    break;
                }
            }
            if sep == b':' {
                self.w(":");
            } else {
                self.w(" =");
            }
            self.w(" ");
            self.expr(v);
        }
    }

    /// `visitAnnotation`: `@`, the name (with the prefix), the type arguments,
    /// `.constructor` and the arguments.
    fn annotation(&mut self, m: &Annotation) {
        self.w("@");
        // The type arguments come after the parts written before them.
        let before = match m.type_args.first() {
            Some(&t) => {
                let start_ = self.a.ty(t).span.start;
                m.name.iter().filter(|n| n.span.end <= start_).count()
            }
            None => m.name.len(),
        };
        for (k, n) in m.name.iter().enumerate() {
            if k == before {
                self.type_arguments_(&m.type_args);
            }
            if k > 0 {
                self.w(".");
            }
            self.nome(*n);
        }
        if before >= m.name.len() {
            self.type_arguments_(&m.type_args);
        }
        if let Some(args) = &m.arguments {
            self.arguments_(args, false);
        }
    }

    /// `visitArgumentList`; `with_types` prints the invocation's type arguments
    /// first.
    fn arguments_(&mut self, args: &Arguments, with_types: bool) {
        if with_types {
            self.type_arguments_(&args.type_args);
        }
        self.w("(");
        for (k, x) in args.args.iter().enumerate() {
            if k > 0 {
                self.w(", ");
            }
            if let Some(n) = x.name {
                // `visitNamedExpression`: the `name:` label and ` value`.
                self.nome(n);
                self.w(": ");
            }
            self.expr(x.value);
        }
        self.w(")");
    }

    // -----------------------------------------------------------------
    // Expressions
    // -----------------------------------------------------------------

    fn expr(&mut self, e: ExprId) {
        let a = self.a;
        let x = a.expr(e);
        match &x.kind {
            ExprKind::Int(s) | ExprKind::Double(s) => self.excerpt(*s),
            ExprKind::Bool(v) => self.w(if *v { "true" } else { "false" }),
            ExprKind::Null => self.w("null"),
            ExprKind::String(s) => self.string(s),
            ExprKind::Symbol(partes) => {
                self.w("#");
                for (k, n) in partes.iter().enumerate() {
                    if k > 0 {
                        self.w(".");
                    }
                    self.nome(*n);
                }
            }
            ExprKind::Identifier(n) => self.nome(*n),
            ExprKind::This => self.w("this"),
            ExprKind::Super => self.w("super"),
            ExprKind::Parenthesized(i) => {
                self.w("(");
                self.expr(*i);
                self.w(")");
            }
            ExprKind::List { const_, type_args, elements } => {
                if *const_ {
                    self.w("const ");
                }
                self.type_arguments_(type_args);
                self.w("[");
                self.elements_(elements);
                self.w("]");
            }
            ExprKind::SetOrMap { const_, type_args, elements } => {
                if *const_ {
                    self.w("const ");
                }
                self.type_arguments_(type_args);
                self.w("{");
                self.elements_(elements);
                self.w("}");
            }
            ExprKind::Record { positional, named, .. } => {
                // `visitRecordLiteral`: the fields in written order, without the `const`
                // and without the trailing comma.
                let mut fields_: Vec<(usize, Option<crate::ast::Name>, ExprId)> = positional.iter().map(|&p| (a.expr(p).span.start, None, p)).collect();
                fields_.extend(named.iter().map(|&(n, v)| (n.span.start, Some(n), v)));
                fields_.sort_by_key(|c| c.0);
                self.w("(");
                for (k, (_, n, v)) in fields_.into_iter().enumerate() {
                    if k > 0 {
                        self.w(", ");
                    }
                    if let Some(n) = n {
                        self.nome(n);
                        self.w(": ");
                    }
                    self.expr(v);
                }
                self.w(")");
            }
            ExprKind::InstanceCreation { keyword, ty, constructor, arguments } => {
                match keyword {
                    Some(CreationKeyword::New) => self.w("new "),
                    Some(CreationKeyword::Const) => self.w("const "),
                    None => {}
                }
                self.kind(*ty);
                if let Some(c) = constructor {
                    self.w(".");
                    self.nome(*c);
                }
                self.arguments_(arguments, false);
            }
            ExprKind::FunctionExpression(f) => self.function(*f, false),
            ExprKind::Property { target, name, null_aware } => {
                if matches!(a.expr(*target).kind, ExprKind::CascadeTarget) {
                    self.w(self.cascade);
                } else {
                    self.expr(*target);
                    self.w(if *null_aware { "?." } else { "." });
                }
                self.nome(*name);
            }
            ExprKind::Index { target, index, null_aware } => {
                if matches!(a.expr(*target).kind, ExprKind::CascadeTarget) {
                    self.w(self.cascade);
                } else {
                    self.expr(*target);
                    if *null_aware {
                        self.w("?");
                    }
                }
                self.w("[");
                self.expr(*index);
                self.w("]");
            }
            ExprKind::Call { target, arguments } => {
                self.expr(*target);
                self.arguments_(arguments, true);
            }
            ExprKind::TypeArguments { target, type_args } => {
                self.expr(*target);
                self.type_arguments_(type_args);
            }
            ExprKind::Unary { op, operand } => {
                let (pre, lexeme) = match op {
                    UnaryOp::Neg => (true, "-"),
                    UnaryOp::Not => (true, "!"),
                    UnaryOp::BitNot => (true, "~"),
                    UnaryOp::PrefixInc => (true, "++"),
                    UnaryOp::PrefixDec => (true, "--"),
                    UnaryOp::PostfixInc => (false, "++"),
                    UnaryOp::PostfixDec => (false, "--"),
                    UnaryOp::NullAssert => (false, "!"),
                };
                // The written operands already respect precedence (the parentheses are in
                // the tree): `_writeOperand` adds nothing.
                if pre {
                    self.w(lexeme);
                    self.expr(*operand);
                } else {
                    self.expr(*operand);
                    self.w(lexeme);
                }
            }
            ExprKind::Binary { op, left, right } => {
                self.expr(*left);
                self.w(" ");
                self.w(binary_lexeme(*op));
                self.w(" ");
                self.expr(*right);
            }
            ExprKind::Conditional { condition, then, else_ } => {
                self.expr(*condition);
                self.w(" ? ");
                self.expr(*then);
                self.w(" : ");
                self.expr(*else_);
            }
            ExprKind::Is { value, ty, negated } => {
                self.expr(*value);
                self.w(if *negated { " is! " } else { " is " });
                self.kind(*ty);
            }
            ExprKind::As { value, ty } => {
                self.expr(*value);
                self.w(" as ");
                self.kind(*ty);
            }
            ExprKind::Assign { op, target, value } => {
                self.expr(*target);
                self.w(" ");
                match op {
                    AssignOp::Assign => self.w("="),
                    AssignOp::Compound(b) => {
                        self.w(binary_lexeme(*b));
                        self.w("=");
                    }
                }
                self.w(" ");
                self.expr(*value);
            }
            ExprKind::PatternAssign { pattern, value } => {
                self.pattern(*pattern);
                self.w(" = ");
                self.expr(*value);
            }
            ExprKind::Cascade { target, sections, null_aware } => {
                self.expr(*target);
                let before = self.cascade;
                for (k, &s) in sections.iter().enumerate() {
                    self.cascade = if k == 0 && *null_aware { "?.." } else { ".." };
                    self.expr(s);
                }
                self.cascade = before;
            }
            ExprKind::CascadeTarget => self.w(self.cascade),
            ExprKind::Await(i) => {
                self.w("await ");
                self.expr(*i);
            }
            ExprKind::Throw(i) => {
                self.w("throw ");
                self.expr(*i);
            }
            ExprKind::Rethrow => self.w("rethrow"),
            ExprKind::Switch { value, cases } => {
                self.w("switch (");
                self.expr(*value);
                self.w(") {");
                for (k, c) in cases.iter().enumerate() {
                    if k > 0 {
                        self.w(", ");
                    }
                    self.pattern(c.pattern);
                    if let Some(g) = c.guard {
                        self.w(" when ");
                        self.expr(g);
                    }
                    self.w(" => ");
                    self.expr(c.body);
                }
                self.w("}");
            }
            ExprKind::DotShorthand { name, const_ } => {
                if *const_ {
                    self.w("const ");
                }
                self.w(".");
                self.nome(*name);
            }
        }
    }

    /// The string, possibly adjacent and interpolated, from the source: each
    /// literal with its lexeme (`visitSimpleStringLiteral`,
    /// `visitInterpolationString`), adjacent ones separated by a space
    /// (`visitAdjacentStrings`), each `${e}` interpolation recomposed
    /// (`visitInterpolationExpression`).
    fn string(&mut self, s: &StringLit) {
        let a = self.a;
        let b = self.fonte.as_bytes();
        let end_offset = s.span.end.min(b.len());
        let mut interpolations = s.parts.iter().filter_map(|p| match p {
            crate::ast::StringPart::Interpolation(e) => Some(*e),
            crate::ast::StringPart::Text(_) => None,
        });
        let mut i = s.span.start;
        let mut first = true;
        while i < end_offset {
            let j = pular_brancos(b, i);
            if j >= end_offset {
                break;
            }
            i = j;
            if !first {
                self.w(" ");
            }
            first = false;
            let raw = b[i] == b'r';
            let mut start_ = i;
            if raw {
                i += 1;
            }
            let q = b[i];
            let triple = b[i..].starts_with(&[q, q, q]);
            let n = if triple { 3 } else { 1 };
            i += n;
            loop {
                if i >= end_offset {
                    self.output.push_str(&self.fonte[start_..end_offset]);
                    return;
                }
                if b[i] == q && (!triple || b[i..].starts_with(&[q, q, q])) {
                    i += n;
                    self.output.push_str(&self.fonte[start_..i]);
                    break;
                }
                if !raw && b[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if !raw && b[i] == b'$' && i + 1 < end_offset {
                    if b[i + 1] == b'{' {
                        self.output.push_str(&self.fonte[start_..i]);
                        self.w("${");
                        if let Some(e) = interpolations.next() {
                            self.expr(e);
                            // Up to the closing `}`, after the expression.
                            let k = pular_brancos(b, a.expr(e).span.end);
                            i = if b.get(k) == Some(&b'}') { k + 1 } else { k };
                        } else {
                            i += 2;
                        }
                        self.w("}");
                        start_ = i;
                        continue;
                    }
                    if identifier_part(b[i + 1]) && !b[i + 1].is_ascii_digit() && b[i + 1] != b'$' {
                        self.output.push_str(&self.fonte[start_..i]);
                        self.w("$");
                        if let Some(e) = interpolations.next() {
                            self.expr(e);
                            i = a.expr(e).span.end;
                        } else {
                            i += 1;
                        }
                        start_ = i;
                        continue;
                    }
                }
                i += 1;
            }
        }
    }

    fn elements_(&mut self, es: &[CollectionElement]) {
        for (k, e) in es.iter().enumerate() {
            if k > 0 {
                self.w(", ");
            }
            self.element(e);
        }
    }

    fn element(&mut self, e: &CollectionElement) {
        match e {
            CollectionElement::Expression(x) => self.expr(*x),
            CollectionElement::NullAwareExpression(x) => {
                self.w("?");
                self.expr(*x);
            }
            // `visitMapLiteralEntry`: 3.6.2 does not print the `?`.
            CollectionElement::MapEntry { key, value, .. } => {
                self.expr(*key);
                self.w(" : ");
                self.expr(*value);
            }
            CollectionElement::Spread { value, null_aware } => {
                self.w(if *null_aware { "...?" } else { "..." });
                self.expr(*value);
            }
            CollectionElement::If { condition, case_pattern, guard, then, else_ } => {
                self.w("if (");
                self.expr(*condition);
                if let Some(p) = case_pattern {
                    self.w(" case ");
                    self.pattern(*p);
                    if let Some(g) = guard {
                        self.w(" when ");
                        self.expr(*g);
                    }
                }
                self.w(") ");
                self.element(then);
                if let Some(x) = else_ {
                    self.w(" else ");
                    self.element(x);
                }
            }
            CollectionElement::For { await_, init, condition, updates, body } => {
                if *await_ {
                    self.w("await ");
                }
                self.w("for (");
                self.for_parts(init.as_ref(), *condition, updates);
                self.w(") ");
                self.element(body);
            }
            CollectionElement::ForIn { await_, target, iterable, body } => {
                if *await_ {
                    self.w("await ");
                }
                self.w("for (");
                self.for_in_parts(target, *iterable);
                self.w(") ");
                self.element(body);
            }
        }
    }

    fn for_parts(&mut self, init: Option<&ForInit>, condition: Option<ExprId>, updates: &[ExprId]) {
        if let Some(ForInit::Pattern { final_, pattern, value }) = init {
            // `visitForPartsWithPattern`.
            self.w(if *final_ { "final " } else { "var " });
            self.pattern(*pattern);
            self.w(" = ");
            self.expr(*value);
            self.w("; ");
            if let Some(c) = condition {
                self.expr(c);
            }
            self.w("; ");
            for (k, &u) in updates.iter().enumerate() {
                if k > 0 {
                    self.w(", ");
                }
                self.expr(u);
            }
            return;
        }
        match init {
            Some(ForInit::Variables(l)) => self.variable_list(l),
            Some(ForInit::Expression(e)) => self.expr(*e),
            _ => {}
        }
        self.w(";");
        if let Some(c) = condition {
            self.w(" ");
            self.expr(c);
        }
        self.w(";");
        for (k, &u) in updates.iter().enumerate() {
            self.w(if k == 0 { " " } else { ", " });
            self.expr(u);
        }
    }

    fn for_in_parts(&mut self, target: &ForInTarget, iterable: ExprId) {
        match target {
            ForInTarget::Declared { metadata, final_, var_, ty, name, const_ } => {
                self.metadata(metadata);
                if *final_ {
                    self.w("final ");
                } else if *var_ {
                    self.w("var ");
                } else if const_.is_some() {
                    self.w("const ");
                }
                if let Some(t) = ty {
                    self.kind(*t);
                    self.w(" ");
                }
                self.nome(*name);
            }
            ForInTarget::Pattern { final_, pattern } => {
                self.w(if *final_ { "final " } else { "var " });
                self.pattern(*pattern);
            }
            ForInTarget::Expression(e) => self.expr(*e),
        }
        self.w(" in ");
        self.expr(iterable);
    }

    /// `visitVariableDeclarationList` (without the metadata, which the caller
    /// adds).
    fn variable_list(&mut self, l: &VariableList) {
        if l.late {
            self.w("late ");
        }
        if l.final_ {
            self.w("final ");
        } else if l.const_ {
            self.w("const ");
        } else if l.var_ {
            self.w("var ");
        }
        if let Some(t) = l.ty {
            self.kind(t);
            self.w(" ");
        }
        for (k, v) in l.variables.iter().enumerate() {
            if k > 0 {
                self.w(", ");
            }
            self.nome(v.name);
            if let Some(i) = v.initializer {
                self.w(" = ");
                self.expr(i);
            }
        }
    }

    /// `visitFunctionExpression` (`declared`: the `=> e` body of a declaration
    /// ends with `;`).
    fn function(&mut self, f: FunctionId, declared: bool) {
        let a = self.a;
        let g = a.function(f);
        self.type_parameters_(&g.type_params);
        if let Some(ps) = &g.parameters {
            self.parameters_(ps);
        }
        // `_visitFunctionBody`.
        if !matches!(g.body, FunctionBody::Empty) {
            self.w(" ");
        }
        let modifier = match g.modifier {
            AsyncModifier::None => "",
            AsyncModifier::Async => "async ",
            AsyncModifier::AsyncStar => "async* ",
            AsyncModifier::SyncStar => "sync* ",
        };
        match &g.body {
            FunctionBody::Block(s) => {
                self.w(modifier);
                self.stmt(*s);
            }
            FunctionBody::Expression(e) => {
                self.w(modifier);
                self.w("=> ");
                self.expr(*e);
                if declared {
                    self.w(";");
                }
            }
            FunctionBody::Empty => self.w(";"),
            FunctionBody::Native(lit) => {
                self.w("native ");
                if let Some(l) = lit {
                    self.string(l);
                }
                self.w(";");
            }
        }
    }

    // -----------------------------------------------------------------
    // Statements
    // -----------------------------------------------------------------

    fn stmt(&mut self, s: StmtId) {
        let a = self.a;
        let st = a.stmt(s);
        match &st.kind {
            StmtKind::Block(ss) => {
                self.w("{");
                for (k, &x) in ss.iter().enumerate() {
                    if k > 0 {
                        self.w(" ");
                    }
                    self.stmt(x);
                }
                self.w("}");
            }
            StmtKind::Variables(l) => {
                self.metadata_local(s);
                self.variable_list(l);
                self.w(";");
            }
            StmtKind::PatternVariables { final_, pattern, value } => {
                self.metadata_local(s);
                self.w(if *final_ { "final " } else { "var " });
                self.pattern(*pattern);
                self.w(" = ");
                self.expr(*value);
                self.w(";");
            }
            StmtKind::Function(f) => {
                // `visitFunctionDeclaration` of a local function.
                self.metadata_local(s);
                let g = a.function(*f);
                if g.external {
                    self.w("external ");
                }
                if let Some(r) = g.return_type {
                    self.kind(r);
                    self.w(" ");
                }
                if let Some(n) = g.name {
                    self.nome(n);
                }
                self.function(*f, true);
            }
            StmtKind::Expression(e) => {
                self.expr(*e);
                self.w(";");
            }
            StmtKind::If { condition, case_pattern, guard, then, else_ } => {
                self.w("if (");
                self.expr(*condition);
                if let Some(p) = case_pattern {
                    self.w(" case ");
                    self.pattern(*p);
                    if let Some(g) = guard {
                        self.w(" when ");
                        self.expr(*g);
                    }
                }
                self.w(") ");
                self.stmt(*then);
                if let Some(x) = else_ {
                    self.w(" else ");
                    self.stmt(*x);
                }
            }
            StmtKind::For { await_, init, condition, updates, body } => {
                if *await_ {
                    self.w("await ");
                }
                self.w("for (");
                self.for_parts(init.as_ref(), *condition, updates);
                self.w(") ");
                self.stmt(*body);
            }
            StmtKind::ForIn { await_, target, iterable, body } => {
                if *await_ {
                    self.w("await ");
                }
                self.w("for (");
                self.for_in_parts(target, *iterable);
                self.w(") ");
                self.stmt(*body);
            }
            StmtKind::While { condition, body } => {
                self.w("while (");
                self.expr(*condition);
                self.w(") ");
                self.stmt(*body);
            }
            StmtKind::DoWhile { body, condition } => {
                self.w("do ");
                self.stmt(*body);
                self.w(" while (");
                self.expr(*condition);
                self.w(");");
            }
            StmtKind::Switch { value, cases } => {
                self.w("switch (");
                self.expr(*value);
                self.w(") {");
                for (k, c) in cases.iter().enumerate() {
                    if k > 0 {
                        self.w(" ");
                    }
                    for l in c.labels.iter() {
                        self.nome(*l);
                        self.w(": ");
                    }
                    match c.pattern {
                        Some(p) => {
                            self.w("case ");
                            self.pattern(p);
                            if let Some(g) = c.guard {
                                self.w(" when ");
                                self.expr(g);
                            }
                            self.w(": ");
                        }
                        None => self.w("default: "),
                    }
                    for (j, &x) in c.body.iter().enumerate() {
                        if j > 0 {
                            self.w(" ");
                        }
                        self.stmt(x);
                    }
                }
                self.w("}");
            }
            StmtKind::Break(l) | StmtKind::Continue(l) => {
                self.w(if matches!(st.kind, StmtKind::Break(_)) { "break" } else { "continue" });
                if let Some(n) = l {
                    self.w(" ");
                    self.nome(*n);
                }
                self.w(";");
            }
            StmtKind::Return(v) => match v {
                None => self.w("return;"),
                Some(e) => {
                    self.w("return ");
                    self.expr(*e);
                    self.w(";");
                }
            },
            StmtKind::Yield { star, value } => {
                self.w(if *star { "yield* " } else { "yield " });
                self.expr(*value);
                self.w(";");
            }
            StmtKind::Try { body, catches, finally_ } => {
                self.w("try ");
                self.stmt(*body);
                for c in catches.iter() {
                    self.w(" ");
                    self.catch(c);
                }
                if let Some(f) = finally_ {
                    self.w(" finally ");
                    self.stmt(*f);
                }
            }
            StmtKind::Labeled { labels, body } => {
                for l in labels.iter() {
                    self.nome(*l);
                    self.w(": ");
                }
                self.stmt(*body);
            }
            StmtKind::Assert { condition, message } => {
                self.w("assert (");
                self.expr(*condition);
                if let Some(m) = message {
                    self.w(", ");
                    self.expr(*m);
                }
                self.w(");");
            }
            StmtKind::Empty => self.w(";"),
        }
    }

    fn catch(&mut self, c: &CatchClause) {
        if let Some(t) = c.on_type {
            self.w("on ");
            self.kind(t);
        }
        if c.exception.is_some() {
            if c.on_type.is_some() {
                self.w(" ");
            }
            self.w("catch (");
            if let Some(e) = c.exception {
                self.nome(e);
            }
            if let Some(s) = c.stack_trace {
                self.w(", ");
                self.nome(s);
            }
            self.w(") ");
        } else {
            self.w(" ");
        }
        self.stmt(c.body);
    }

    // -----------------------------------------------------------------
    // Patterns
    // -----------------------------------------------------------------

    fn pattern(&mut self, p: PatternId) {
        let a = self.a;
        let pt = a.pattern(p);
        match &pt.kind {
            PatternKind::Wildcard { ty } => {
                // `visitWildcardPattern`: the `var`/`final` as written.
                if palavra_em(self.fonte, pt.span.start, "var") {
                    self.w("var ");
                } else if palavra_em(self.fonte, pt.span.start, "final") {
                    self.w("final ");
                }
                if let Some(t) = ty {
                    self.kind(*t);
                    self.w(" ");
                }
                self.w("_");
            }
            PatternKind::Variable { final_, var_, ty, name } => {
                if *final_ {
                    self.w("final ");
                } else if *var_ {
                    self.w("var ");
                }
                if let Some(t) = ty {
                    self.kind(*t);
                    self.w(" ");
                }
                self.nome(*name);
            }
            PatternKind::Constant(e) => {
                // `visitConstantPattern`: the pattern's `const`, when the expression does
                // not print it itself.
                let before = self.output.len();
                let written = palavra_em(self.fonte, pt.span.start, "const");
                self.expr(*e);
                if written && !self.output[before..].starts_with("const ") {
                    self.output.insert_str(before, "const ");
                }
            }
            PatternKind::Relational { op, value } => {
                self.w(binary_lexeme(*op));
                self.w(" ");
                self.expr(*value);
            }
            PatternKind::Or(l, r) => {
                self.pattern(*l);
                self.w(" || ");
                self.pattern(*r);
            }
            PatternKind::And(l, r) => {
                self.pattern(*l);
                self.w(" && ");
                self.pattern(*r);
            }
            PatternKind::NullCheck(i) => {
                self.pattern(*i);
                self.w("?");
            }
            PatternKind::NullAssert(i) => {
                self.pattern(*i);
                self.w("!");
            }
            PatternKind::Cast { pattern, ty } => {
                self.pattern(*pattern);
                self.w(" as ");
                self.kind(*ty);
            }
            PatternKind::Parenthesized(i) => {
                self.w("(");
                self.pattern(*i);
                self.w(")");
            }
            PatternKind::List { type_args, elements } => {
                self.type_arguments_(type_args);
                self.w("[");
                for (k, e) in elements.iter().enumerate() {
                    if k > 0 {
                        self.w(", ");
                    }
                    match e {
                        ListPatternElement::Pattern(x) => self.pattern(*x),
                        ListPatternElement::Rest(x) => {
                            self.w("...");
                            if let Some(x) = x {
                                self.pattern(*x);
                            }
                        }
                    }
                }
                self.w("]");
            }
            PatternKind::Map { type_args, entries, rest } => {
                self.type_arguments_(type_args);
                self.w("{");
                for (k, e) in entries.iter().enumerate() {
                    if k > 0 {
                        self.w(", ");
                    }
                    self.expr(e.key);
                    self.w(": ");
                    self.pattern(e.value);
                }
                if *rest {
                    if !entries.is_empty() {
                        self.w(", ");
                    }
                    self.w("...");
                }
                self.w("}");
            }
            PatternKind::Record { fields } => {
                self.w("(");
                self.fields_(fields);
                if fields.len() == 1 {
                    self.w(",");
                }
                self.w(")");
            }
            PatternKind::Object { ty, fields } => {
                self.kind(*ty);
                self.w("(");
                self.fields_(fields);
                self.w(")");
            }
        }
    }

    /// `visitPatternField`: `name: p`, `: p` (the name inferred from the
    /// pattern) or just `p`.
    fn fields_(&mut self, fields: &[crate::ast::PatternField]) {
        let a = self.a;
        for (k, f) in fields.iter().enumerate() {
            if k > 0 {
                self.w(", ");
            }
            if let Some(n) = f.name {
                let sp = a.pattern(f.pattern).span;
                let inferred = n.span.start >= sp.start && n.span.end <= sp.end;
                if !inferred {
                    self.nome(n);
                }
                self.w(": ");
            }
            self.pattern(f.pattern);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_and_comments() {
        let b = b"  /* a /* b */ c */ // x\n  y";
        assert_eq!(pular_brancos(b, 0), b.len() - 1);
    }

    #[test]
    fn whole_word() {
        assert!(palavra_em("var x", 0, "var"));
        assert!(!palavra_em("variable", 0, "var"));
    }
}
