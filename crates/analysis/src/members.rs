//! Local checks of declarations and members from the `ErrorVerifier` of
//! analyzer 6.11.0 (`src/generated/error_verifier.dart`), those that depend
//! only on the written form and on the library's scope:
//!
//! * `_checkForConflicting*TypeVariableErrorCodes`: a type parameter with the
//!   name of the declaration or of one of its members
//!   (`conflicting_type_variable_and_container`/`_and_member`);
//! * `visitFieldDeclaration`: `const_instance_field`,
//!   `extension_type_declares_instance_field`, `non_final_field_in_enum`;
//! * `visitMethodDeclaration`/`visitFunctionDeclaration`: setters
//!   (`wrong_number_of_parameters_for_setter`, `non_void_return_for_setter`),
//!   `Object` members in extensions and extension types, abstract members in
//!   an extension type;
//! * `visitConstructorDeclaration` and the `ReturnTypeVerifier`:
//!   `return_in_generative_constructor`;
//! * `visitFieldFormalParameter`: `this.x` outside a constructor, in a
//!   `factory` or in a redirecting constructor (`field_initializer_*`).
//!
//! The positions are the analyzer's: the name (`atToken`) or the whole node
//! (`atNode`, which starts at the documentation comment or at the annotation).

use dartforge_diagnostics::{Code, Diagnostic, Span, codes::compile_time_error as c};
use dartforge_elements::model::{DeclRef, Element, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{
    self, DeclKind, FunctionBody, FunctionKind, Initializer, MemberKind, Parameter, StmtId,
    StmtKind, TypeKind, TypedefKind,
};
use dartforge_frontend::features::Feature;
use dartforge_intern::{Interner, SymbolId};

/// The context of a unit of the library.
struct Ctx<'a> {
    program: &'a Program,
    names: &'a Interner,
    u: UnitId,
    ast: &'a ast::Ast,
    source: &'a str,
    out: Vec<(UnitId, Diagnostic)>,
    /// The initializer list being checked is the primary constructor's.
    super_in_primary: bool,
}

fn diag(code: Code, span: Span, args: &[&str]) -> Diagnostic {
    Diagnostic::with_code(code, span, args.iter().copied())
}

/// The names of `Object` members (`MethodDeclarationExtension.hasObjectMemberName`).
fn is_object_member(name: &str) -> bool {
    matches!(
        name,
        "==" | "hashCode" | "toString" | "runtimeType" | "noSuchMethod"
    )
}

/// The lexeme of a member's name (`unary-` is written `-`).
fn lexeme<'a>(names: &'a Interner, f: &ast::Function) -> Option<&'a str> {
    let n = names.resolve(f.name?.sym);
    Some(if n == "unary-" { "-" } else { n })
}

/// The start of an annotated node (`AnnotatedNodeImpl.beginToken`): the
/// documentation comment right before, if any, otherwise the start itself.
fn start_with_docs(source: &str, start: usize) -> usize {
    let before = &source[..start];
    let without_space = before.trim_end();
    // `/** ... */` immediately before.
    if let Some(without_end) = without_space.strip_suffix("*/") {
        if let Some(i) = without_end.rfind("/**")
            && !without_end[i..].contains("*/")
        {
            return i;
        }
        return start;
    }
    // Linhas `///` consecutivas.
    let mut result = start;
    let mut end = without_space.len();
    loop {
        let line_start = without_space[..end].rfind('\n').map_or(0, |i| i + 1);
        let line = without_space[line_start..end].trim_start();
        if line.starts_with("///") && !line.starts_with("////") {
            result = end - line.len();
            if line_start == 0 {
                break;
            }
            end = without_space[..line_start].trim_end().len();
            if !without_space[end..line_start]
                .chars()
                .all(char::is_whitespace)
            {
                break;
            }
        } else {
            break;
        }
    }
    result
}

/// Does the member starting at `start` come right after the end of another
/// declaration (`;`, `}`), the opening of the body (`{`) or an annotation?
/// During error recovery, our parser may drop words before the member
/// (`augment factory A()`, `int new()`) that the analyzer's uses; then the
/// member's shape is not the same and nothing is reported about it.
fn preceded_by_boundary(source: &str, start: usize) -> bool {
    let mut before = &source[..start];
    loop {
        let t = before.trim_end();
        if let Some(without) = t.strip_suffix("*/") {
            match without.rfind("/*") {
                Some(i) => {
                    before = &without[..i];
                    continue;
                }
                None => return false,
            }
        }
        // Line comment: the last line starting (after spaces) with `//`.
        let line_start = t.rfind('\n').map_or(0, |i| i + 1);
        let line = &t[line_start..];
        if let Some(j) = line.find("//")
            && !line[..j].contains(['\'', '"'])
        {
            before = &t[..line_start + j];
            continue;
        }
        return t.is_empty() || t.ends_with([';', '}', '{']);
    }
}

impl Ctx<'_> {
    fn name(&self, s: SymbolId) -> &str {
        self.names.resolve(s)
    }

    fn report(&mut self, code: Code, span: Span, args: &[&str]) {
        self.out.push((self.u, diag(code, span, args)));
    }

    /// `type is VoidType` for a written annotation: `void`, or an alias
    /// (`typedef V = void;`) that expands to it.
    fn is_void(&self, u: UnitId, ast_: &ast::Ast, t: ast::TypeId, depth: u32) -> bool {
        let node = ast_.ty(t);
        match &node.kind {
            TypeKind::Void => true,
            TypeKind::Named { name, args } if args.is_empty() && depth < 8 => {
                let b = match &name[..] {
                    [n] => self.program.lookup_in_unit(u, n.sym),
                    [p, n] => self.program.lookup_prefixed_in_unit(u, p.sym, n.sym),
                    _ => None,
                };
                let Some(Element::Typedef(tid)) = b.and_then(|b| b.getter) else {
                    return false;
                };
                let td = self.program.typedef(tid);
                let ast_td = &self.program.unit(td.decl.unit).ast;
                let DeclKind::Typedef(d) = &ast_td.decl(td.decl.decl).kind else {
                    return false;
                };
                match d.kind {
                    TypedefKind::Alias(body) => self.is_void(td.decl.unit, ast_td, body, depth + 1),
                    TypedefKind::Legacy { .. } => false,
                }
            }
            _ => false,
        }
    }

    /// `_checkForWrongNumberOfParametersForSetter` and
    /// `_checkForNonVoidReturnTypeForSetter`. A setter without a parameter
    /// list gets an empty one in the parser's recovery.
    fn setter(&mut self, f: &ast::Function) {
        let Some(name) = f.name else { return };
        let params = f.parameters.as_deref().unwrap_or(&[]);
        if params.len() != 1 || params[0].kind != ast::ParameterKind::Required {
            self.report(c::WRONG_NUMBER_OF_PARAMETERS_FOR_SETTER, name.span, &[]);
        }
        if let Some(t) = f.return_type
            && !self.is_void(self.u, self.ast, t, 0)
        {
            self.report(c::NON_VOID_RETURN_FOR_SETTER, self.ast.ty(t).span, &[]);
        }
    }

    /// The `FieldFormalParameter` without the default value (the
    /// `DefaultFormalParameter` wraps it).
    fn span_without_default(&self, p: &Parameter) -> Span {
        match p.default_value {
            None => p.span,
            Some(v) => {
                let start = self.ast.expr(v).span.start;
                let end = p.name.map_or(p.span.start, |n| n.span.end);
                let fragment = self.source.get(end..start).unwrap_or("");
                let t = fragment.trim_end();
                let t = t
                    .strip_suffix('=')
                    .or_else(|| t.strip_suffix(':'))
                    .unwrap_or(t)
                    .trim_end();
                Span {
                    start: p.span.start,
                    end: end + t.len(),
                }
            }
        }
    }

    /// `this.x` outside a constructor: in the parameters of `ps` and in those of
    /// the function-typed parameters inside them.
    fn formals_outside(&mut self, ps: &[Parameter]) {
        for p in ps {
            if p.this_ {
                let s = self.span_without_default(p);
                self.report(c::FIELD_INITIALIZER_OUTSIDE_CONSTRUCTOR, s, &[]);
            }
            if let Some(inner) = &p.function_parameters {
                self.formals_outside(inner);
            }
        }
    }

    /// `ReturnTypeVerifier.verifyReturnStatement` in a generative constructor:
    /// `return e;` in the body (not in nested functions).
    fn returns(&mut self, s: StmtId) {
        let ast_ = self.ast;
        match &ast_.stmt(s).kind {
            StmtKind::Return(Some(e)) => {
                let span = ast_.expr(*e).span;
                self.report(c::RETURN_IN_GENERATIVE_CONSTRUCTOR, span, &[]);
            }
            StmtKind::Block(v) => v.iter().for_each(|&x| self.returns(x)),
            StmtKind::If { then, else_, .. } => {
                self.returns(*then);
                if let Some(e) = else_ {
                    self.returns(*e);
                }
            }
            StmtKind::For { body, .. }
            | StmtKind::ForIn { body, .. }
            | StmtKind::While { body, .. }
            | StmtKind::DoWhile { body, .. }
            | StmtKind::Labeled { body, .. } => self.returns(*body),
            StmtKind::Switch { cases, .. } => {
                for case in cases.iter() {
                    case.body.iter().for_each(|&x| self.returns(x));
                }
            }
            StmtKind::Try {
                body,
                catches,
                finally_,
            } => {
                self.returns(*body);
                for cc in catches.iter() {
                    self.returns(cc.body);
                }
                if let Some(f) = finally_ {
                    self.returns(*f);
                }
            }
            _ => {}
        }
    }

    /// The `=> e;` body (`ExpressionFunctionBody`): from the arrow to the `;`.
    fn expression_body(&self, e: ast::ExprId) -> Span {
        let s = self.ast.expr(e).span;
        let before = &self.source[..s.start];
        let start = before.rfind("=>").unwrap_or(s.start);
        let after = &self.source[s.end..];
        let end = match after.trim_start().strip_prefix(';') {
            Some(_) => s.end + (after.len() - after.trim_start().len()) + 1,
            None => s.end,
        };
        Span { start, end }
    }

    /// `_checkForConflictingInitializerErrorCodes`: redirection and
    /// `super(...)` in the initializer list. `superclass` is the name the
    /// analyzer displays for the supertype (`None` where it is not known here).
    fn initializers(
        &mut self,
        k: &ast::Constructor,
        container: Container,
        superclass: Option<&str>,
    ) {
        let is_enum = container == Container::Enum;
        let mut redirections = 0;
        let mut supers = 0;
        let mut last_super = None;
        for (i, start) in k.initializers.iter().enumerate() {
            match start {
                Initializer::Redirect { span, .. } => {
                    if redirections > 0 && !self.super_in_primary {
                        self.report(c::MULTIPLE_REDIRECTING_CONSTRUCTOR_INVOCATIONS, *span, &[]);
                    }
                    redirections += 1;
                }
                Initializer::Super { span, .. } => {
                    if is_enum {
                        self.report(
                            c::SUPER_IN_ENUM_CONSTRUCTOR,
                            Span {
                                start: span.start,
                                end: span.start + 5,
                            },
                            &[],
                        );
                    } else if supers == 1 {
                        // In the primary (3.13.4), on the `super`.
                        let sp = if self.super_in_primary {
                            Span {
                                start: span.start,
                                end: span.start + 5,
                            }
                        } else {
                            *span
                        };
                        self.report(c::MULTIPLE_SUPER_INITIALIZERS, sp, &[]);
                    }
                    last_super = Some(i);
                    supers += 1;
                }
                _ => {}
            }
        }
        // A primary that redirects is `primary_constructor_cannot_redirect`
        // (reported elsewhere); the redirection rules do not apply.
        if redirections > 0 && !self.super_in_primary {
            for start in k.initializers.iter() {
                match start {
                    Initializer::Super { span, .. } if !is_enum => {
                        self.report(c::SUPER_IN_REDIRECTING_CONSTRUCTOR, *span, &[]);
                    }
                    Initializer::Field { span, .. } => {
                        self.report(c::FIELD_INITIALIZER_REDIRECTING_CONSTRUCTOR, *span, &[]);
                    }
                    Initializer::Assert { span, .. } => {
                        self.report(c::ASSERT_IN_REDIRECTING_CONSTRUCTOR, *span, &[]);
                    }
                    _ => {}
                }
            }
        }
        if !is_enum
            && redirections == 0
            && supers == 1
            && let Some(i) = last_super
            && i + 1 != k.initializers.len()
            && let Initializer::Super {
                span, constructor, ..
            } = &k.initializers[i]
            && let Some(sup) = superclass
        {
            let name = match constructor {
                Some(n) => format!("{sup}.{}", self.name(n.sym)),
                None => sup.to_string(),
            };
            self.report(
                c::SUPER_INVOCATION_NOT_LAST,
                Span {
                    start: span.start,
                    end: span.start + 5,
                },
                &[&name],
            );
        }
    }

    fn constructor(&mut self, k: &ast::Constructor) {
        // `_checkForRedirectingConstructorErrorCodes`
        // (`error_verifier.dart:5094-5107`): a default value in a parameter of
        // a constructor with `= target`, on the parameter's name.
        if k.redirect.is_some() {
            for p in k.parameters.iter() {
                if p.default_value.is_some()
                    && let Some(n) = p.name
                {
                    self.report(
                        c::DEFAULT_VALUE_IN_REDIRECTING_FACTORY_CONSTRUCTOR,
                        n.span,
                        &[],
                    );
                }
            }
        }
        if !k.factory {
            match &k.body {
                FunctionBody::Expression(e) => {
                    let s = self.expression_body(*e);
                    self.report(c::RETURN_IN_GENERATIVE_CONSTRUCTOR, s, &[]);
                }
                FunctionBody::Block(b) => self.returns(*b),
                _ => {}
            }
        }
        let redirects = k
            .initializers
            .iter()
            .any(|i| matches!(i, Initializer::Redirect { .. }));
        for p in k.parameters.iter() {
            if p.this_ {
                let s = self.span_without_default(p);
                if k.factory {
                    self.report(c::FIELD_INITIALIZER_FACTORY_CONSTRUCTOR, s, &[]);
                } else if redirects {
                    self.report(c::FIELD_INITIALIZER_REDIRECTING_CONSTRUCTOR, s, &[]);
                }
            }
            if let Some(inner) = &p.function_parameters {
                self.formals_outside(inner);
            }
        }
    }
}

/// `ErrorVerifier._checkUseOfCovariantInParameters`
/// (`error_verifier.dart:6143-6178`), in every parameter list: silent in the
/// methods of a class, mixin, enum or extension type, inside an extension and
/// in top-level functions (the parser reports those); elsewhere (function
/// expression, local function, constructor, function type, function-typed
/// parameter, `typedef`) each direct `covariant` reports, on the token.
fn misplaced_covariant(cx: &mut Ctx<'_>, ast_: &ast::Ast) {
    let extensions: Vec<Span> = ast_
        .decls
        .iter()
        .filter(|d| matches!(d.kind, DeclKind::Extension(_)))
        .map(|d| d.span)
        .collect();
    let extension_types: Vec<Span> = ast_
        .decls
        .iter()
        .filter(|d| matches!(d.kind, DeclKind::ExtensionType(_)))
        .map(|d| d.span)
        .collect();
    let inside = |v: &[Span], s: Span| v.iter().any(|x| x.start <= s.start && s.end <= x.end);
    fn report_list(cx: &mut Ctx<'_>, ps: &[Parameter]) {
        for p in ps {
            if !p.covariant {
                continue;
            }
            let end = p.name.map_or(p.span.end, |n| n.span.start);
            let Some(fragment) = cx.source.get(p.span.start..end) else {
                continue;
            };
            if let Some(i) = fragment.find("covariant") {
                let s = Span {
                    start: p.span.start + i,
                    end: p.span.start + i + "covariant".len(),
                };
                cx.report(c::INVALID_USE_OF_COVARIANT, s, &[]);
            }
        }
    }
    /// The nested lists of function-typed parameters (`void f(covariant int x)`).
    fn nested(cx: &mut Ctx<'_>, ps: &[Parameter]) {
        for p in ps {
            if let Some(inner) = &p.function_parameters {
                report_list(cx, inner);
                nested(cx, inner);
            }
        }
    }
    // The functions that are declaration methods and the top-level ones.
    let mut methods: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for m in &ast_.members {
        if let MemberKind::Method(fid) = &m.kind {
            methods.insert(fid.0);
        }
    }
    let mut top_level: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for d in &ast_.decls {
        if let DeclKind::Function(fid) = &d.kind {
            top_level.insert(fid.0);
        }
    }
    for (i, f) in ast_.functions.iter().enumerate() {
        let Some(ps) = &f.parameters else { continue };
        if inside(&extensions, f.span) {
            continue;
        }
        let i = i as u32;
        if !methods.contains(&i) && !top_level.contains(&i) {
            report_list(cx, ps);
        }
        nested(cx, ps);
    }
    // The primary constructors (3.13): the parser checks the header's list
    // (`INVALID_COVARIANT_MODIFIER_IN_PRIMARY_CONSTRUCTOR`/
    // `EXTRANEOUS_MODIFIER_IN_PRIMARY_CONSTRUCTOR`), and the verifier returns
    // (`parent is PrimaryConstructorDeclaration`).
    let primaries: std::collections::HashSet<u32> = ast_
        .decls
        .iter()
        .filter_map(|d| match &d.kind {
            DeclKind::Class(x) => x.primary_constructor,
            DeclKind::Enum(x) => x.primary_constructor,
            _ => None,
        })
        .map(|m| m.0)
        .collect();
    for (mi, m) in ast_.members.iter().enumerate() {
        let MemberKind::Constructor(k) = &m.kind else {
            continue;
        };
        if inside(&extensions, m.span) {
            continue;
        }
        // In an extension type the parser reports (`EXTRANEOUS_MODIFIER_IN_EXTENSION_TYPE`).
        if !inside(&extension_types, m.span) && !primaries.contains(&(mi as u32)) {
            report_list(cx, &k.parameters);
        }
        nested(cx, &k.parameters);
    }
    for ty in &ast_.types {
        let TypeKind::Function { parameters, .. } = &ty.kind else {
            continue;
        };
        if inside(&extensions, ty.span) {
            continue;
        }
        report_list(cx, parameters);
        nested(cx, parameters);
    }
    for d in &ast_.decls {
        if let DeclKind::Typedef(x) = &d.kind
            && let TypedefKind::Legacy { parameters, .. } = &x.kind
        {
            report_list(cx, parameters);
            nested(cx, parameters);
        }
    }
}

/// `ErrorVerifier.visitSuperFormalParameter`
/// (`error_verifier.dart:1457-1478`) outside a generative, non-redirecting,
/// non-`external` constructor: in functions, methods, nested function-typed
/// parameters (even inside a constructor), `factory`, an `external`
/// constructor or one with `this(...)` — `invalid_super_formal_parameter_location`;
/// inside an extension type the code is always
/// `extension_type_constructor_with_super_formal_parameter` (the one of the
/// constructor's list comes out in `a_context`). On the `super` token.
fn misplaced_super(cx: &mut Ctx<'_>, ast_: &ast::Ast) {
    let extension_types: Vec<Span> = ast_
        .decls
        .iter()
        .filter(|d| matches!(d.kind, DeclKind::ExtensionType(_)))
        .map(|d| d.span)
        .collect();
    let in_extension_type = |s: Span| {
        extension_types
            .iter()
            .any(|x| x.start <= s.start && s.end <= x.end)
    };
    fn report_super(cx: &mut Ctx<'_>, p: &Parameter, extension: bool) {
        let end = p.name.map_or(p.span.end, |n| n.span.start);
        let Some(fragment) = cx.source.get(p.span.start..end) else {
            return;
        };
        let Some(i) = fragment.rfind("super") else {
            return;
        };
        let s = Span {
            start: p.span.start + i,
            end: p.span.start + i + 5,
        };
        let code = if extension {
            c::EXTENSION_TYPE_CONSTRUCTOR_WITH_SUPER_FORMAL_PARAMETER
        } else {
            c::INVALID_SUPER_FORMAL_PARAMETER_LOCATION
        };
        cx.report(code, s, &[]);
    }
    fn all(cx: &mut Ctx<'_>, ps: &[Parameter], extension: bool) {
        for p in ps {
            if p.super_ {
                report_super(cx, p, extension);
            }
            if let Some(inner) = &p.function_parameters {
                all(cx, inner, extension);
            }
        }
    }
    for f in &ast_.functions {
        if let Some(ps) = &f.parameters {
            let extension = in_extension_type(f.span);
            all(cx, ps, extension);
        }
    }
    for m in &ast_.members {
        let MemberKind::Constructor(k) = &m.kind else {
            continue;
        };
        let extension = in_extension_type(m.span);
        let direct_generative = !k.factory
            && !k.external
            && !k
                .initializers
                .iter()
                .any(|i| matches!(i, Initializer::Redirect { .. }));
        for p in k.parameters.iter() {
            // The constructor's list in an extension type already comes out in `a_context`.
            if p.super_ && !direct_generative && !extension {
                report_super(cx, p, false);
            }
            if let Some(inner) = &p.function_parameters {
                all(cx, inner, extension);
            }
        }
    }
}

/// Where a piece of code has access to `this` (`ErrorVerifier._hasAccessToThis`).
#[derive(Clone, Copy)]
enum Access {
    /// The body of an instance method or of a generative constructor, or the
    /// initializer of a `late` instance field.
    Has,
    /// The body of a static method, of a `factory` or of a top-level function,
    /// an initializer list, the initializer of a non-`late` or static field.
    HasNot,
    /// A local function or function expression: inherits the outer one.
    Inherits,
}

/// `_checkForInvalidReferenceToThis`: each `this` by the innermost context
/// that contains it. The bodies that decide the access
/// (`_computeThisAccessForFunctionBody`) and the fields
/// (`visitFieldDeclaration`) are source ranges; the narrowest one containing
/// the `this` decides, and a local function passes the question outward.
fn this_without_access(cx: &mut Ctx<'_>) {
    let ast_ = cx.ast;
    let sites: Vec<Span> = ast_
        .exprs
        .iter()
        .filter(|e| matches!(e.kind, ast::ExprKind::This))
        .map(|e| e.span)
        .collect();
    if sites.is_empty() {
        return;
    }
    let body = |b: &FunctionBody| match b {
        FunctionBody::Block(s) => Some(ast_.stmt(*s).span),
        FunctionBody::Expression(e) => Some(ast_.expr(*e).span),
        _ => None,
    };
    // The roles of the declared functions; the others in the arena are local.
    let mut role: std::collections::HashMap<u32, Access> = std::collections::HashMap::new();
    let mut regions: Vec<(Span, Access)> = Vec::new();
    let program = cx.program;
    for &id in &program.unit(cx.u).unit.declarations {
        match &ast_.decl(id).kind {
            DeclKind::Function(f) => {
                role.insert(f.0, Access::HasNot);
            }
            DeclKind::Variables(vl) => {
                for v in vl.variables.iter() {
                    if let Some(e) = v.initializer {
                        regions.push((ast_.expr(e).span, Access::HasNot));
                    }
                }
            }
            DeclKind::Class(_)
            | DeclKind::Mixin(_)
            | DeclKind::Enum(_)
            | DeclKind::Extension(_)
            | DeclKind::ExtensionType(_) => {
                let members: &[ast::MemberId] = match &ast_.decl(id).kind {
                    DeclKind::Class(d) => &d.members,
                    DeclKind::Mixin(d) => &d.members,
                    DeclKind::Enum(d) => &d.members,
                    DeclKind::Extension(d) => &d.members,
                    DeclKind::ExtensionType(d) => &d.members,
                    _ => &[],
                };
                for &m in members {
                    match &ast_.member(m).kind {
                        MemberKind::Method(f) => {
                            let is_static = ast_.function(*f).static_;
                            role.insert(
                                f.0,
                                if is_static {
                                    Access::HasNot
                                } else {
                                    Access::Has
                                },
                            );
                        }
                        MemberKind::Field(vl) => {
                            let access = if !vl.static_ && vl.late {
                                Access::Has
                            } else {
                                Access::HasNot
                            };
                            for v in vl.variables.iter() {
                                if let Some(e) = v.initializer {
                                    regions.push((ast_.expr(e).span, access));
                                }
                            }
                        }
                        MemberKind::Constructor(k) => {
                            if let Some(sp) = body(&k.body) {
                                regions.push((
                                    sp,
                                    if k.factory {
                                        Access::HasNot
                                    } else {
                                        Access::Has
                                    },
                                ));
                            }
                        }
                    }
                }
            }
            DeclKind::Typedef(_) => {}
        }
    }
    // The parameters that an extension type's representation drops do not
    // exist for the analyzer (`AstBuilder.endPrimaryConstructor`).
    let discarded: Vec<Span> = program
        .unit(cx.u)
        .unit
        .declarations
        .iter()
        .filter_map(|&id| match &ast_.decl(id).kind {
            DeclKind::ExtensionType(d) => Some(d.representation_span),
            _ => None,
        })
        .collect();
    let sites: Vec<Span> = sites
        .into_iter()
        .filter(|u| {
            !discarded
                .iter()
                .any(|d| d.start <= u.start && u.end <= d.end)
        })
        .collect();
    for (i, f) in ast_.functions.iter().enumerate() {
        if let Some(sp) = body(&f.body) {
            regions.push((
                sp,
                role.get(&(i as u32)).copied().unwrap_or(Access::Inherits),
            ));
        }
    }
    // From the narrowest to the widest.
    regions.sort_by_key(|(s, _)| s.end - s.start);
    for site in sites {
        let mut access = Access::HasNot;
        for &(sp, a) in &regions {
            if sp.start <= site.start && site.end <= sp.end {
                match a {
                    Access::Inherits => continue,
                    _ => {
                        access = a;
                        break;
                    }
                }
            }
        }
        if matches!(access, Access::HasNot) {
            cx.report(c::INVALID_REFERENCE_TO_THIS, site, &[]);
        }
    }
}

/// The scanner's built-in identifiers (`KeywordStyle.builtIn`), except
/// `augment`, which 3.6's scanner only makes a keyword with the experiment.
/// T1.1 a: the top-level name `name` designates, in the library's scope,
/// something other than declaration `decl` (the same-named one that enters
/// the scope first).
fn name_designates_another(
    program: &Program,
    lib: LibraryId,
    name: SymbolId,
    decl: DeclRef,
) -> bool {
    let Some(designated) = program
        .library(lib)
        .declared
        .get(&name)
        .and_then(|d| d.getter)
    else {
        return false;
    };
    match designated {
        Element::Class(c) => program
            .class(c)
            .decl
            .is_some_and(|d| d.unit != decl.unit || d.decl != decl.decl),
        Element::Extension(_) | Element::Prefix(..) => false,
        _ => true,
    }
}

fn is_built_in(name: &str) -> bool {
    matches!(
        name,
        "abstract"
            | "as"
            | "covariant"
            | "deferred"
            | "dynamic"
            | "export"
            | "extension"
            | "external"
            | "factory"
            | "Function"
            | "get"
            | "implements"
            | "import"
            | "interface"
            | "late"
            | "library"
            | "mixin"
            | "operator"
            | "part"
            | "required"
            | "set"
            | "static"
            | "typedef"
    )
}

/// `_checkForBuiltInIdentifierAsName`: a built-in identifier as the name of a
/// class, mixin, enum, alias, `typedef`, extension, extension type, import
/// prefix or type parameter.
fn built_in_identifiers(cx: &mut Ctx<'_>) {
    let ast_ = cx.ast;
    let program = cx.program;
    let unit = program.unit(cx.u);
    let sdk = program.library(unit.library).is_sdk;
    let mut found: Vec<(Code, ast::Name)> = Vec::new();
    for d in &unit.unit.directives {
        if let ast::DirectiveKind::Import {
            prefix: Some(p), ..
        } = &d.kind
        {
            found.push((c::BUILT_IN_IDENTIFIER_AS_PREFIX_NAME, *p));
        }
    }
    let mut params: Vec<ast::Name> = Vec::new();
    for &id in &unit.unit.declarations {
        let (code, name, tps): (Code, Option<ast::Name>, &[ast::TypeParameter]) =
            match &ast_.decl(id).kind {
                // `visitClassTypeAlias`: a class alias uses the `typedef` code.
                DeclKind::Class(d) if d.mixin_application => (
                    c::BUILT_IN_IDENTIFIER_AS_TYPEDEF_NAME,
                    Some(d.name),
                    &d.type_params,
                ),
                DeclKind::Class(d) => (
                    c::BUILT_IN_IDENTIFIER_AS_TYPE_NAME,
                    Some(d.name),
                    &d.type_params,
                ),
                DeclKind::Mixin(d) => (
                    c::BUILT_IN_IDENTIFIER_AS_TYPE_NAME,
                    Some(d.name),
                    &d.type_params,
                ),
                DeclKind::Enum(d) => (
                    c::BUILT_IN_IDENTIFIER_AS_TYPE_NAME,
                    Some(d.name),
                    &d.type_params,
                ),
                DeclKind::Typedef(d) => (
                    c::BUILT_IN_IDENTIFIER_AS_TYPEDEF_NAME,
                    Some(d.name),
                    &d.type_params,
                ),
                DeclKind::Extension(d) => (
                    c::BUILT_IN_IDENTIFIER_AS_EXTENSION_NAME,
                    d.name,
                    &d.type_params,
                ),
                DeclKind::ExtensionType(d) => (
                    c::BUILT_IN_IDENTIFIER_AS_EXTENSION_TYPE_NAME,
                    Some(d.name),
                    &d.type_params,
                ),
                _ => continue,
            };
        if let Some(n) = name {
            // The `Function` class of `dart:core` is the exception.
            if !(sdk && cx.name(n.sym) == "Function") {
                found.push((code, n));
            }
        }
        params.extend(tps.iter().map(|p| p.name));
    }
    for f in &ast_.functions {
        params.extend(f.type_params.iter().map(|p| p.name));
    }
    for t in &ast_.types {
        if let TypeKind::Function { type_params, .. } = &t.kind {
            params.extend(type_params.iter().map(|p| p.name));
        }
    }
    found.extend(
        params
            .into_iter()
            .map(|n| (c::BUILT_IN_IDENTIFIER_AS_TYPE_PARAMETER_NAME, n)),
    );
    for (code, n) in found {
        let text = cx.name(n.sym).to_string();
        // The written name (the parser's recovery may give a synthetic name).
        if is_built_in(&text) && cx.source.get(n.span.start..n.span.end) == Some(text.as_str()) {
            cx.report(code, n.span, &[&text]);
        }
    }
}

/// The kind of the declaration that contains the members.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Container {
    Class,
    Mixin,
    Enum,
    Extension,
    ExtensionType,
}

/// The names that `getNamedConstructor`/`getMethod`/`getGetter`/`getSetter`
/// find in the declaration.
fn member_names(ast_: &ast::Ast, members: &[ast::MemberId], constructors: bool) -> Vec<SymbolId> {
    let mut v = Vec::new();
    for &m in members {
        match &ast_.member(m).kind {
            MemberKind::Field(vl) => v.extend(vl.variables.iter().map(|x| x.name.sym)),
            MemberKind::Method(f) => {
                let f = ast_.function(*f);
                if f.kind != FunctionKind::Operator
                    && let Some(n) = f.name
                {
                    v.push(n.sym);
                }
            }
            MemberKind::Constructor(k) => {
                if constructors && let Some(n) = k.name {
                    v.push(n.sym);
                }
            }
        }
    }
    v
}

/// The diagnostics of this family in library `lib`.
pub fn check(program: &Program, lib: LibraryId, names: &Interner) -> Vec<(UnitId, Diagnostic)> {
    let library = program.library(lib);
    let wildcard = library.features.has(Feature::WildcardVariables);
    let mut out = Vec::new();
    for &u in &library.units {
        let unit = program.unit(u);
        if unit.role == dartforge_elements::model::UnitRole::Patch {
            continue;
        }
        let mut cx = Ctx {
            program,
            names,
            u,
            ast: &unit.ast,
            source: &unit.source,
            out: Vec::new(),
            super_in_primary: false,
        };
        let ast_ = &unit.ast;
        for &id in &unit.unit.declarations {
            let decl = ast_.decl(id);
            let (container, name, tps, members, extra): (
                Container,
                Option<ast::Name>,
                &[ast::TypeParameter],
                &[ast::MemberId],
                Vec<SymbolId>,
            ) = match &decl.kind {
                DeclKind::Class(d) => (
                    Container::Class,
                    Some(d.name),
                    &d.type_params,
                    &d.members,
                    Vec::new(),
                ),
                DeclKind::Mixin(d) => (
                    Container::Mixin,
                    Some(d.name),
                    &d.type_params,
                    &d.members,
                    Vec::new(),
                ),
                DeclKind::Enum(d) => (
                    Container::Enum,
                    Some(d.name),
                    &d.type_params,
                    &d.members,
                    // The implicit `values` is also a member
                    // (`ConflictTypeParameterValues<values>`).
                    d.constants
                        .iter()
                        .map(|k| k.name.sym)
                        .chain(names.lookup("values"))
                        .collect(),
                ),
                DeclKind::Extension(d) => (
                    Container::Extension,
                    d.name,
                    &d.type_params,
                    &d.members,
                    Vec::new(),
                ),
                DeclKind::ExtensionType(d) => (
                    Container::ExtensionType,
                    Some(d.name),
                    &d.type_params,
                    &d.members,
                    // The named primary constructor (`extension type A<T>.T(…)`)
                    // is a member like the other constructors.
                    std::iter::once(d.representation_name.sym)
                        .chain(d.constructor.map(|n| n.sym))
                        .collect(),
                ),
                DeclKind::Function(f) => {
                    let f = ast_.function(*f);
                    if f.kind == FunctionKind::Setter {
                        cx.setter(f);
                    }
                    continue;
                }
                DeclKind::Typedef(td) => {
                    // `_checkForTypeAliasCannotReferenceItself`.
                    let target = DeclRef { unit: u, decl: id };
                    if dartforge_types::self_reference::self_referencing_typedef(program, target) {
                        cx.report(c::TYPE_ALIAS_CANNOT_REFERENCE_ITSELF, td.name.span, &[]);
                    }
                    continue;
                }
                _ => continue,
            };

            let primary = match &decl.kind {
                DeclKind::Class(d) => d.primary_constructor,
                DeclKind::Enum(d) => d.primary_constructor,
                _ => None,
            };
            // `enclosingClass.supertype.element.displayName` of a class.
            let superclass: Option<String> = match &decl.kind {
                DeclKind::Class(d) => match d.extends {
                    None => Some("Object".to_string()),
                    Some(t) => match &ast_.ty(t).kind {
                        TypeKind::Named { name, .. } => {
                            let b = match &name[..] {
                                [n] => program.lookup_in_unit(u, n.sym),
                                [p, n] => program.lookup_prefixed_in_unit(u, p.sym, n.sym),
                                _ => None,
                            };
                            match b.filter(|b| !b.ambiguous).and_then(|b| b.getter) {
                                Some(Element::Class(id)) => {
                                    Some(names.resolve(program.class(id).name).to_string())
                                }
                                _ => None,
                            }
                        }
                        _ => None,
                    },
                },
                _ => None,
            };
            // Conflicting type parameters (only in the declaration, not in augmentations).
            if !decl.augment {
                let constructors = matches!(container, Container::Class | Container::ExtensionType);
                let mut member_name_set = member_names(ast_, members, constructors);
                member_name_set.extend(extra.iter().copied());
                for tp in tps {
                    let text = cx.name(tp.name.sym).to_string();
                    let skip_wildcard = wildcard
                        && text == "_"
                        && !matches!(container, Container::Enum | Container::Extension);
                    if skip_wildcard {
                        continue;
                    }
                    let (c_container, c_member) = match container {
                        Container::Class => (
                            c::CONFLICTING_TYPE_VARIABLE_AND_CLASS,
                            c::CONFLICTING_TYPE_VARIABLE_AND_MEMBER_CLASS,
                        ),
                        Container::Mixin => (
                            c::CONFLICTING_TYPE_VARIABLE_AND_MIXIN,
                            c::CONFLICTING_TYPE_VARIABLE_AND_MEMBER_MIXIN,
                        ),
                        Container::Enum => (
                            c::CONFLICTING_TYPE_VARIABLE_AND_ENUM,
                            c::CONFLICTING_TYPE_VARIABLE_AND_MEMBER_ENUM,
                        ),
                        Container::Extension => (
                            c::CONFLICTING_TYPE_VARIABLE_AND_EXTENSION,
                            c::CONFLICTING_TYPE_VARIABLE_AND_MEMBER_EXTENSION,
                        ),
                        Container::ExtensionType => (
                            c::CONFLICTING_TYPE_VARIABLE_AND_EXTENSION_TYPE,
                            c::CONFLICTING_TYPE_VARIABLE_AND_MEMBER_EXTENSION_TYPE,
                        ),
                    };
                    if name.is_some_and(|n| n.sym == tp.name.sym) {
                        cx.report(c_container, tp.name.span, &[&text]);
                    }
                    if member_name_set.contains(&tp.name.sym) {
                        cx.report(c_member, tp.name.span, &[&text]);
                    }
                }
            }

            for &m in members {
                let member = ast_.member(m);
                match &member.kind {
                    MemberKind::Field(vl) => {
                        if !vl.static_ && vl.const_ {
                            // The word `const` before the type or the first name.
                            let until = vl
                                .ty
                                .map(|t| ast_.ty(t).span.start)
                                .or(vl.variables.first().map(|v| v.name.span.start));
                            if let Some(until) = until {
                                let start = member
                                    .metadata
                                    .last()
                                    .map_or(member.span.start, |a| a.span.end);
                                if let Some(i) =
                                    cx.source.get(start..until).and_then(|t| t.rfind("const"))
                                {
                                    let s = Span {
                                        start: start + i,
                                        end: start + i + 5,
                                    };
                                    cx.report(c::CONST_INSTANCE_FIELD, s, &[]);
                                }
                            }
                        }
                        if container == Container::ExtensionType && !vl.static_ && !vl.external {
                            for v in vl.variables.iter() {
                                cx.report(
                                    c::EXTENSION_TYPE_DECLARES_INSTANCE_FIELD,
                                    v.name.span,
                                    &[],
                                );
                            }
                        }
                        if container == Container::Enum
                            && !vl.static_
                            && !vl.final_
                            && let Some(v) = vl.variables.first()
                        {
                            cx.report(c::NON_FINAL_FIELD_IN_ENUM, v.name.span, &[]);
                        }
                    }
                    MemberKind::Method(fid) => {
                        let f = ast_.function(*fid);
                        if f.kind == FunctionKind::Setter {
                            cx.setter(f);
                        }
                        let Some(m_name) = f.name else { continue };
                        let lex = lexeme(names, f).unwrap_or("");
                        if is_object_member(lex) {
                            match container {
                                Container::Extension => cx.report(
                                    c::EXTENSION_DECLARES_MEMBER_OF_OBJECT,
                                    m_name.span,
                                    &[],
                                ),
                                Container::ExtensionType => cx.report(
                                    c::EXTENSION_TYPE_DECLARES_MEMBER_OF_OBJECT,
                                    m_name.span,
                                    &[],
                                ),
                                _ => {}
                            }
                        }
                        if container == Container::ExtensionType
                            && !f.static_
                            && !f.external
                            && matches!(f.body, FunctionBody::Empty)
                            && let Some(n) = name
                        {
                            let start = start_with_docs(cx.source, member.span.start);
                            let s = Span {
                                start,
                                end: member.span.end,
                            };
                            let ty = cx.name(n.sym).to_string();
                            cx.report(c::EXTENSION_TYPE_WITH_ABSTRACT_MEMBER, s, &[lex, &ty]);
                        }
                    }
                    MemberKind::Constructor(k) => {
                        // The primary constructor (3.13): the initializer list
                        // of the `this : …` body goes through the same rules,
                        // on the `super`.
                        if (k.primary_body_part || primary == Some(m)) && !k.initializers.is_empty()
                        {
                            cx.super_in_primary = true;
                            cx.initializers(k, container, superclass.as_deref());
                            cx.super_in_primary = false;
                        }
                        if k.primary_body_part
                            || primary == Some(m)
                            || !preceded_by_boundary(cx.source, member.span.start)
                        {
                            continue;
                        }
                        // `_isFactoryConstructorReturnType`: the name before the
                        // dot of a `factory` must designate the declaration
                        // itself. With a same-named one, the name may designate
                        // another (T1.1 a and e, `c10`).
                        if k.factory
                            && let Some(n) = name
                            && (k.class_name.sym != n.sym
                                || name_designates_another(
                                    program,
                                    lib,
                                    n.sym,
                                    DeclRef { unit: u, decl: id },
                                ))
                            && cx
                                .source
                                .get(k.class_name.span.start..k.class_name.span.end)
                                == Some(cx.name(k.class_name.sym))
                        {
                            cx.report(c::INVALID_FACTORY_NAME_NOT_A_CLASS, k.class_name.span, &[]);
                        }
                        cx.constructor(k);
                        cx.initializers(k, container, superclass.as_deref());
                    }
                }
            }
        }
        // `this.x` in any function that is not a constructor (methods,
        // top-level and local functions, function expressions).
        for f in &ast_.functions {
            if let Some(ps) = &f.parameters {
                cx.formals_outside(ps);
            }
        }
        // The parser's copy (`AstBuilder.checkFieldFormalParameters`,
        // `ast_builder.dart:779-789`): only in member methods and local
        // functions, and only in direct required positional ones (the others
        // come wrapped in `DefaultFormalParameter`); on the `this` token.
        let mut parser_functions: Vec<ast::FunctionId> = Vec::new();
        for m in &ast_.members {
            if let MemberKind::Method(fid) = &m.kind {
                parser_functions.push(*fid);
            }
        }
        for s in &ast_.stmts {
            if let StmtKind::Function(fid) = &s.kind {
                parser_functions.push(*fid);
            }
        }
        for fid in parser_functions {
            let Some(ps) = &ast_.function(fid).parameters else {
                continue;
            };
            for p in ps
                .iter()
                .filter(|p| p.this_ && !p.super_ && p.kind == ast::ParameterKind::Required)
            {
                let end = p.name.map_or(p.span.end, |n| n.span.start);
                let Some(fragment) = cx.source.get(p.span.start..end) else {
                    continue;
                };
                let Some(i) = fragment.rfind("this") else {
                    continue;
                };
                let s = Span {
                    start: p.span.start + i,
                    end: p.span.start + i + 4,
                };
                cx.report(
                    dartforge_diagnostics::codes::parser::FIELD_INITIALIZER_OUTSIDE_CONSTRUCTOR,
                    s,
                    &[],
                );
            }
        }
        misplaced_super(&mut cx, ast_);
        misplaced_covariant(&mut cx, ast_);
        this_without_access(&mut cx);
        built_in_identifiers(&mut cx);
        out.append(&mut cx.out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use dartforge_elements::{load::load_lenient, sdk::SdkLayout};
    use std::fs;
    use std::path::PathBuf;

    /// A minimal SDK with `dart:core`.
    fn sdk(root: &std::path::Path) -> SdkLayout {
        let _ = fs::remove_dir_all(root);
        fs::create_dir_all(root.join("sdk/lib/core")).unwrap();
        fs::write(
            root.join("sdk/lib/libraries.json"),
            r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#,
        )
        .unwrap();
        fs::write(
            root.join("sdk/lib/core/core.dart"),
            "class Object {} class int {} class String {} class Null {} class bool {}",
        )
        .unwrap();
        SdkLayout::load(&root.join("sdk/lib"), "dartdevc").unwrap()
    }

    /// `(code, fragment, message)` of each diagnostic of `source`.
    fn run(name: &str, source: &str) -> Vec<(String, String, String)> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/tmp-agent/membros-{name}-{}",
            std::process::id()
        ));
        let sdk = sdk(&root);
        let entry = root.join("main.dart");
        fs::write(&entry, source).unwrap();
        let mut names = Interner::new();
        let (program, _) = load_lenient(&entry, &sdk, None, &mut names);
        let mut v: Vec<(String, String, String)> = check(&program, program.entry.unwrap(), &names)
            .into_iter()
            .map(|(_, d)| {
                (
                    d.code.map_or("", |c| c.info().name).to_string(),
                    source[d.span.start..d.span.end].to_string(),
                    d.message,
                )
            })
            .collect();
        v.sort();
        fs::remove_dir_all(&root).unwrap();
        v
    }

    fn t(c: &str, fragment: &str, m: &str) -> (String, String, String) {
        (c.to_string(), fragment.to_string(), m.to_string())
    }

    /// Cases from the analyzer corpus, with what `dart analyze` 3.6.2 reports
    /// (`corpus/diagnostics/analyzer/oracle.jsonl`).
    #[test]
    fn conflicting_type_parameters() {
        let v = run(
            "tv",
            "// @dart = 3.6\nclass A<_> {\n  _() {}\n}\nenum E<E> {\n  v\n}\nextension X<T> on int { int get T => 0; }\nmixin M<M> {}\n",
        );
        assert_eq!(
            v,
            vec![
                t(
                    "conflicting_type_variable_and_container",
                    "E",
                    "'E' can't be used to name both a type parameter and the enum in which the type parameter is defined."
                ),
                t(
                    "conflicting_type_variable_and_container",
                    "M",
                    "'M' can't be used to name both a type parameter and the mixin in which the type parameter is defined."
                ),
                t(
                    "conflicting_type_variable_and_member",
                    "T",
                    "'T' can't be used to name both a type parameter and a member in this extension."
                ),
                t(
                    "conflicting_type_variable_and_member",
                    "_",
                    "'_' can't be used to name both a type parameter and a member in this class."
                ),
            ]
        );
    }

    #[test]
    fn fields() {
        let v = run(
            "campos",
            "mixin C {\n  const int f = 0;\n}\nenum E { v; int a = 0, b = 1; static int s = 0; final int f = 0; }\nextension type T(int it) { int x = 0; external int y; static int z = 0; }\n",
        );
        assert_eq!(
            v,
            vec![
                t(
                    "const_instance_field",
                    "const",
                    "Only static fields can be declared as const."
                ),
                t(
                    "extension_type_declares_instance_field",
                    "x",
                    "Extension types can't declare instance fields."
                ),
                t(
                    "non_final_field_in_enum",
                    "a",
                    "Enums can only declare final fields."
                ),
            ]
        );
    }

    #[test]
    fn field_formals_outside_constructor() {
        let v = run(
            "formais",
            "class A {\n  int x = 0;\n  m([this.x = 0]) {}\n  A.named() {}\n  A(this.x) : this.named();\n  factory A.f(this.x) => throw 0;\n  A.g(void f(this.x));\n}\n",
        );
        assert_eq!(
            v,
            vec![
                t(
                    "field_initializer_factory_constructor",
                    "this.x",
                    "Initializing formal parameters can't be used in factory constructors."
                ),
                t(
                    "field_initializer_outside_constructor",
                    "this.x",
                    "Initializing formal parameters can only be used in constructors."
                ),
                t(
                    "field_initializer_outside_constructor",
                    "this.x",
                    "Initializing formal parameters can only be used in constructors."
                ),
                t(
                    "field_initializer_redirecting_constructor",
                    "this.x",
                    "The redirecting constructor can't have a field initializer."
                ),
            ]
        );
    }

    #[test]
    fn object_members_and_abstract_in_extensions() {
        let v = run(
            "object",
            "extension type E(int it) {\n  int get hashCode => 0;\n  /// doc\n  void foo();\n  static void bar() {}\n}\nextension X on int { String toString() => ''; }\n",
        );
        assert_eq!(
            v,
            vec![
                t(
                    "extension_declares_member_of_object",
                    "toString",
                    "Extensions can't declare members with the same name as a member declared by 'Object'."
                ),
                t(
                    "extension_type_declares_member_of_object",
                    "hashCode",
                    "Extension types can't declare members with the same name as a member declared by 'Object'."
                ),
                t(
                    "extension_type_with_abstract_member",
                    "/// doc\n  void foo();",
                    "'foo' must have a method body because 'E' is an extension type."
                ),
            ]
        );
    }

    /// The language's `setter/declaration_test.dart` and
    /// `getter/syntax_get_set_syntax_test.dart`: a return type that is not
    /// `void` (a `void` alias is fine) and the number of parameters.
    #[test]
    fn setters() {
        let v = run(
            "setters",
            "typedef V = void;\nint set a(int x) {}\nV set b(int x) {}\nset c(int x, int y) {}\nclass K { void set d([int? x]) {} dynamic set e(x) {} }\n",
        );
        assert_eq!(
            v,
            vec![
                t(
                    "non_void_return_for_setter",
                    "dynamic",
                    "The return type of the setter must be 'void' or absent."
                ),
                t(
                    "non_void_return_for_setter",
                    "int",
                    "The return type of the setter must be 'void' or absent."
                ),
                t(
                    "wrong_number_of_parameters_for_setter",
                    "c",
                    "Setters must declare exactly one required positional parameter."
                ),
                t(
                    "wrong_number_of_parameters_for_setter",
                    "d",
                    "Setters must declare exactly one required positional parameter."
                ),
            ]
        );
    }

    /// `return_in_generative_constructor` from the corpus: the whole `=> e;`,
    /// and the expression of `return e;` in the block (not in nested functions).
    #[test]
    fn return_in_generative_constructor() {
        let v = run(
            "retorno",
            "class A {\n  A() => A.b();\n  A.b() { if (true) return 0; return; }\n  A.c() { f() { return 1; } }\n  factory A.d() => A();\n}\n",
        );
        assert_eq!(
            v,
            vec![
                t(
                    "return_in_generative_constructor",
                    "0",
                    "Constructors can't return values."
                ),
                t(
                    "return_in_generative_constructor",
                    "=> A.b();",
                    "Constructors can't return values."
                ),
            ]
        );
    }

    /// The initializer list (`_checkForConflictingInitializerErrorCodes`), in
    /// the shapes of `variable/initializer_super_last_test.dart` and of the
    /// analyzer corpus (3.6.2 oracle).
    #[test]
    fn initializer_list() {
        let v = run(
            "inicializadores",
            "class S { S(); S.named(); }\nclass C extends S {\n  int x = 0;\n  C.a(int x) : super.named(), x = x;\n  C() : this.a(0), this.b();\n  C.b() : this(), x = 1, assert(true), super();\n  C.c() : super(), super();\n}\nclass D { int y; D() : super(), y = 0; }\nenum E { v; const E() : super(); }\n",
        );
        assert_eq!(
            v,
            vec![
                t(
                    "assert_in_redirecting_constructor",
                    "assert(true)",
                    "A redirecting constructor can't have an 'assert' initializer."
                ),
                t(
                    "field_initializer_redirecting_constructor",
                    "x = 1",
                    "The redirecting constructor can't have a field initializer."
                ),
                t(
                    "multiple_redirecting_constructor_invocations",
                    "this.b()",
                    "Constructors can have only one 'this' redirection, at most."
                ),
                t(
                    "multiple_super_initializers",
                    "super()",
                    "A constructor can have at most one 'super' initializer."
                ),
                t(
                    "super_in_enum_constructor",
                    "super",
                    "The enum constructor can't have a 'super' initializer."
                ),
                t(
                    "super_in_redirecting_constructor",
                    "super()",
                    "The redirecting constructor can't have a 'super' initializer."
                ),
                t(
                    "super_invocation_not_last",
                    "super",
                    "The superconstructor call must be last in an initializer list: 'Object'."
                ),
                t(
                    "super_invocation_not_last",
                    "super",
                    "The superconstructor call must be last in an initializer list: 'S.named'."
                ),
            ]
        );
    }

    /// A member our parser's recovery separated from the words before it
    /// (`int new() => 1;`, `augment factory A() => …`): the analyzer sees it
    /// with another shape, and nothing is reported.
    #[test]
    fn recovery_constructor_does_not_report() {
        assert!(preceded_by_boundary("class A {\n  // x\n  /* y */ A()", 27));
        assert!(!preceded_by_boundary("class A {\n  int new()", 16));
    }

    /// `type_alias_cannot_reference_itself` (corpus, 3.6.2 oracle): directly,
    /// through another `typedef`, through the bound of a class type
    /// parameter; the type parameter with the same name hides the `typedef`.
    #[test]
    fn typedef_referencing_itself() {
        let v = run(
            "typedef",
            "typedef F = void Function(F);\ntypedef G = List<H>;\ntypedef H = G Function();\nclass C<T extends K> {}\ntypedef K = C;\ntypedef L<L> = L Function();\ntypedef int M(M x);\ntypedef N = List<int>;\n",
        );
        let m = "Typedefs can't reference themselves directly or recursively via another typedef.";
        assert_eq!(
            v,
            vec![
                t("type_alias_cannot_reference_itself", "F", m),
                t("type_alias_cannot_reference_itself", "G", m),
                t("type_alias_cannot_reference_itself", "H", m),
                t("type_alias_cannot_reference_itself", "K", m),
                t("type_alias_cannot_reference_itself", "M", m),
            ]
        );
    }

    /// `invalid_reference_to_this` (corpus, 3.6.2 oracle): the innermost
    /// context decides; local functions and function expressions inherit.
    #[test]
    fn this_without_access() {
        let v = run(
            "this",
            "var t = this;\nf() => this;\nclass A {\n  var a = this;\n  late var b = this;\n  static var c = this;\n  int x;\n  A() : x = this.hashCode { this; () => this; }\n  factory A.f() { this; return A(); }\n  m([p = this]) { this; g() => this; }\n  static s() => this;\n}\n",
        );
        let n = v
            .iter()
            .filter(|x| x.0 == "invalid_reference_to_this")
            .count();
        assert_eq!(n, 8, "{v:?}");
    }

    /// `built_in_identifier_in_declaration` (corpus, 3.6.2 oracle): the code
    /// of each context; `augment` and the pseudo-keywords do not count.
    #[test]
    fn built_in_identifier_as_name() {
        let v = run(
            "embutidos",
            "import 'dart:core' as abstract;\nclass as {}\nclass B {}\nmixin M {}\nclass Function = B with M;\ntypedef interface = int;\nextension set on int {}\nclass C<implements> {}\nvoid f<dynamic>() {}\nclass on {}\nclass augment {}\n",
        );
        let names: Vec<(&str, &str)> = v.iter().map(|(_, t, m)| (t.as_str(), m.as_str())).collect();
        assert_eq!(
            names,
            vec![
                (
                    "Function",
                    "The built-in identifier 'Function' can't be used as a typedef name."
                ),
                (
                    "abstract",
                    "The built-in identifier 'abstract' can't be used as a prefix name."
                ),
                (
                    "as",
                    "The built-in identifier 'as' can't be used as a type name."
                ),
                (
                    "dynamic",
                    "The built-in identifier 'dynamic' can't be used as a type parameter name."
                ),
                (
                    "implements",
                    "The built-in identifier 'implements' can't be used as a type parameter name."
                ),
                (
                    "interface",
                    "The built-in identifier 'interface' can't be used as a typedef name."
                ),
                (
                    "set",
                    "The built-in identifier 'set' can't be used as an extension name."
                ),
            ]
        );
    }

    /// `invalid_factory_name_not_a_class` (corpus, 3.6.2 oracle): the name of
    /// a `factory` that is not the class's (in 3.13, `factory foo()` is the
    /// constructor `A.foo`, hence version 3.6).
    #[test]
    fn factory_with_another_name() {
        let v = run(
            "factory",
            "// @dart = 3.6\nclass A {\n  factory B() => throw 0;\n  factory A.c() => throw 0;\n  factory foo() => throw 0;\n}\n",
        );
        let v: Vec<&str> = v
            .iter()
            .filter(|x| x.0 == "invalid_factory_name_not_a_class")
            .map(|x| x.1.as_str())
            .collect();
        assert_eq!(v, vec!["B", "foo"]);
    }
}
