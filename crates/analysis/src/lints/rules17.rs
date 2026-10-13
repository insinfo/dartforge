//! The seventeenth batch of lint rules (docs/ANALYZER-ESPECIFICACAO-INFRA.md
//! §8), written directly from the 3.6.2 emitters
//! (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//!
//! * `library_private_types_in_public_api` (`recommended`): the emitter's
//!   `Validator` on the public declarations of a unit whose file is not
//!   private; the element of a named type is that of the written name (a type
//!   parameter in scope, or the unit's), and a record type is not visited.
//! * `package_prefixed_library_names`: the 3.6.2 emitter reports nothing
//!   (`visitLibraryDirective` returns right away).
//! * `avoid_js_rounded_ints`: the integer literal that a `double` cannot hold.
//! * `cancel_subscriptions` and `close_sinks`: the `LeakDetectorProcessors`
//!   (fields in the unit, locals in the function body) with the valid uses of
//!   the `_ValidUseVisitor`.
//! * `erase_dart_type_extension_types`: `is` with `kernel.ast`'s `DartType`.
//! * `conditional_uri_does_not_exist`: the URI of each configuration that
//!   does not point to an existing file (relative, `package:` through the
//!   package's `package_config.json`, `dart:` through the list of the 3.6.2
//!   SDK's `libraries.dart`).
//! * `missing_code_block_language_in_doc_comment`: a fenced block without a
//!   language (the opening line).
//! * `prefer_mixin`: the `with` of a class that is not a `mixin class`.
//! * `avoid_catching_errors`: the `on` that is or implements `Error`.
//! * `avoid_web_libraries_in_flutter`: the import of `dart:html`, `dart:js` or
//!   `dart:js_util` in a package that depends on `flutter` (and is not a web
//!   plugin).
//! * `cast_nullable_to_non_nullable`: `as` from nullable to non-nullable.
//!
//! Written without compiling or running (2026-10-05).

use super::codigos_g as c;
use super::rules::LintReport;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, VariableId, VariableRef};
use dartforge_frontend::ast::{
    Ast, DeclKind, DirectiveKind, ExprId, ExprKind, FunctionBody, Initializer, MemberKind, Parameter, ParameterKind, StmtKind, TypeId, TypeKind,
    TypeParameter, TypedefKind, UnaryOp,
};
use dartforge_intern::{Interner, SymbolId};
use dartforge_types::resolved::{MemberRef, Resolved};
use dartforge_types::table::{Type, TypeTable};

/// The libraries of the 3.6.2 SDK's `libraries.dart`.
const SDK_LIBRARIES: [&str; 46] = [
    "async", "collection", "concurrent", "convert", "core", "developer", "ffi", "html", "html_common", "indexed_db", "_http", "io", "isolate", "js",
    "_js", "js_interop", "js_interop_unsafe", "js_util", "math", "mirrors", "nativewrappers", "typed_data", "_native_typed_data", "cli", "svg",
    "web_audio", "web_gl", "_internal", "_js_helper", "_late_helper", "_rti", "_interceptors", "_foreign_helper", "_js_names", "_js_primitives",
    "_js_embedded_names", "_js_shared_embedded_names", "_js_types", "_async_status_codes", "_invocation_mirror_constants", "_recipe_syntax",
    "_load_library_priority", "_metadata", "_js_annotations", "_wasm", "_macros",
];

fn is_private(s: &str) -> bool {
    s.starts_with('_')
}

fn is_nullable(table: &TypeTable, t: dartforge_types::table::TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::Void | Type::Null => true,
        Type::Intersection { bound, .. } => is_nullable(table, *bound),
        Type::Interface { nullable, .. } | Type::TypeParameter { nullable, .. } | Type::ExtensionType { nullable, .. } => *nullable,
        Type::Function { nullable, .. } | Type::Record { nullable, .. } => *nullable,
        Type::FutureOr { arg, nullable } => *nullable || is_nullable(table, *arg),
        Type::Never => false,
    }
}

fn is_non_nullable(table: &TypeTable, t: dartforge_types::table::TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::Void | Type::Null => false,
        Type::Intersection { bound, .. } => is_non_nullable(table, *bound),
        Type::TypeParameter { param, nullable } => !*nullable && is_non_nullable(table, table.param(*param).bound),
        Type::FutureOr { arg, nullable } => !*nullable && is_non_nullable(table, *arg),
        Type::Interface { nullable, .. } | Type::ExtensionType { nullable, .. } => !*nullable,
        Type::Function { nullable, .. } | Type::Record { nullable, .. } => !*nullable,
        Type::Never => true,
    }
}

fn library_name(s: &super::Semantics<'_>, interner: &Interner, l: dartforge_elements::model::LibraryId) -> Option<String> {
    s.program.library(l).name.as_ref().map(|n| n.iter().map(|x| interner.resolve(*x)).collect::<Vec<_>>().join("."))
}

/// `implementsInterface(nome, biblioteca)`.
fn implements_class(s: &super::Semantics<'_>, interner: &Interner, t: dartforge_types::table::TypeId, name: &str, library: &str) -> bool {
    let (Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. }) = s.table.get(t) else { return false };
    let e = |k: ClassId| {
        let x = s.program.class(k);
        interner.resolve(x.name) == name && library_name(s, interner, x.library).as_deref() == Some(library)
    };
    e(*class) || (s.program.class(*class).decl.is_some() && s.outline.hierarchy.get(*class).is_some_and(|d| d.supertypes.keys().any(|k| e(*k))))
}

/// The `Validator` of `library_private_types_in_public_api`.
struct PrivateTypeValidator<'a, 'b> {
    a: &'a Ast,
    s: &'a super::Semantics<'b>,
    interner: &'a Interner,
    /// The type parameters in scope (names).
    tps: Vec<SymbolId>,
    findings: Vec<Span>,
}

impl PrivateTypeValidator<'_, '_> {
    fn name_of(&self, s: SymbolId) -> &str {
        self.interner.resolve(s)
    }

    /// `visitNamedType` (and the other types).
    fn visit_type(&mut self, t: TypeId) {
        let a = self.a;
        match &a.ty(t).kind {
            TypeKind::Named { name, args } => {
                let last = name[name.len() - 1];
                let exists = match &name[..] {
                    [n] => self.tps.contains(&n.sym) || self.s.program.lookup_in_unit(self.s.unit, n.sym).is_some(),
                    [p, n] => self.s.program.lookup_prefixed_in_unit(self.s.unit, p.sym, n.sym).is_some(),
                    _ => false,
                };
                if exists && is_private(self.name_of(last.sym)) {
                    self.findings.push(last.span);
                }
                for x in args.iter() {
                    self.visit_type(*x);
                }
            }
            TypeKind::Function { return_type, type_params, parameters, .. } => {
                let before = self.tps.len();
                self.tps.extend(type_params.iter().map(|p| p.name.sym));
                if let Some(r) = return_type {
                    self.visit_type(*r);
                }
                self.visit_type_params(type_params);
                self.visit_params(parameters);
                self.tps.truncate(before);
            }
            _ => {}
        }
    }

    fn visit_type_params(&mut self, tps: &[TypeParameter]) {
        for p in tps {
            if let Some(b) = p.bound {
                self.visit_type(b);
            }
        }
    }

    /// `visitFormalParameterList` with the simple, field, `super` and function
    /// parameters.
    fn visit_params(&mut self, ps: &[Parameter]) {
        for p in ps {
            let named_private = p.kind == ParameterKind::Named && p.name.is_some_and(|n| is_private(self.name_of(n.sym)));
            if named_private {
                continue;
            }
            if let Some(inner) = &p.function_parameters {
                // `FunctionTypedFormalParameter`.
                let before = self.tps.len();
                self.tps.extend(p.function_type_params.iter().map(|t| t.name.sym));
                if let Some(r) = p.ty {
                    self.visit_type(r);
                }
                self.visit_type_params(&p.function_type_params);
                self.visit_params(inner);
                self.tps.truncate(before);
                continue;
            }
            match p.ty {
                Some(t) => self.visit_type(t),
                None if p.this_ || p.super_ => {
                    // The element's type: that of the field (or of the
                    // superclass constructor's parameter), a private interface.
                    if let Some(n) = p.name
                        && self.has_private_implicit_type(p, n.sym)
                    {
                        self.findings.push(n.span);
                    }
                }
                None => {}
            }
        }
    }

    /// The element type of a `this.x`/`super.x` without a written type is an
    /// interface with a private name: the field's, or that of the
    /// corresponding parameter of the superclass constructor (by name, or by
    /// position among the positional `super.`s), seen in the supertype.
    fn has_private_implicit_type(&self, p: &Parameter, name: SymbolId) -> bool {
        let s = self.s;
        let program = s.program;
        let table = s.table;
        let private_iface = |t: dartforge_types::table::TypeId| match table.get(t) {
            Type::Interface { class, .. } => is_private(self.name_of(program.class(*class).name)),
            _ => false,
        };
        if p.this_ {
            let v = program.variables.iter().position(|x| {
                x.name == name && matches!(x.node, VariableRef::Field { unit, member, .. } if unit == s.unit && self.same_class(member, p.span))
            });
            let t = v.and_then(|v| s.outline.variables.get(v)).and_then(|d| d.declared_type.or(d.inferred));
            return t.is_some_and(private_iface);
        }
        // `super.x`: the constructor that has the parameter and its class.
        let a = self.a;
        let Some((decl_idx, k)) = a.decls.iter().enumerate().find_map(|(i, d)| {
            let members = match &d.kind {
                DeclKind::Class(x) => &x.members,
                DeclKind::Enum(x) => &x.members,
                _ => return None,
            };
            members.iter().find_map(|m| match &a.member(*m).kind {
                MemberKind::Constructor(k) if k.parameters.iter().any(|q| q.span == p.span) => Some((i, k)),
                _ => None,
            })
        }) else {
            return false;
        };
        let did = dartforge_frontend::ast::DeclId(decl_idx as u32);
        let Some(class_id) = (0..program.classes.len()).map(|i| ClassId(i as u32)).find(|c| program.class(*c).decl.is_some_and(|r| r.unit == s.unit && r.decl == did)) else {
            return false;
        };
        let Some(sup) = program.class(class_id).supertype_class else { return false };
        let super_name = k.initializers.iter().find_map(|i| match i {
            Initializer::Super { constructor, .. } => Some(*constructor),
            _ => None,
        });
        let key = match super_name.flatten() {
            Some(n) => Some(n.sym),
            None => self.interner.lookup(""),
        };
        let Some(&ksup) = key.and_then(|ch| program.class(sup).constructors.get(&ch)) else { return false };
        let data = match s.outline.functions.get(ksup.0 as usize) {
            Some(d) => d,
            None => return false,
        };
        let Type::Function { positional, optional, named, .. } = table.get(data.signature) else { return false };
        let declared = if p.kind == ParameterKind::Named {
            named.iter().find(|(n, _, _)| *n == name).map(|(_, t, _)| *t)
        } else {
            let pos = k.parameters.iter().take_while(|q| q.span != p.span).filter(|q| q.super_ && q.kind != ParameterKind::Named).count();
            positional.iter().chain(optional.iter()).nth(pos).copied()
        };
        let Some(declared) = declared else { return false };
        // A type parameter of the superclass becomes the supertype's argument.
        let seen = match table.get(declared) {
            Type::TypeParameter { param, .. } => {
                let i = s.outline.classes.get(sup.0 as usize).and_then(|d| d.type_params.iter().position(|q| q == param));
                let template = s.outline.hierarchy.get(class_id).and_then(|d| d.supertypes.get(&sup).copied());
                match (i, template.map(|m| table.get(m))) {
                    (Some(i), Some(Type::Interface { args, .. })) => args.get(i).copied(),
                    _ => None,
                }
            }
            _ => Some(declared),
        };
        seen.is_some_and(private_iface)
    }

    /// The field `member` belongs to the same declaration that contains `pos`.
    fn same_class(&self, member: dartforge_frontend::ast::MemberId, pos: Span) -> bool {
        self.a.decls.iter().any(|d| {
            let members: &[dartforge_frontend::ast::MemberId] = match &d.kind {
                DeclKind::Class(x) => &x.members,
                DeclKind::Enum(x) => &x.members,
                DeclKind::Mixin(x) => &x.members,
                DeclKind::ExtensionType(x) => &x.members,
                _ => return false,
            };
            members.contains(&member) && within(pos, d.span)
        })
    }

    /// The members of a public declaration.
    fn visit_members(&mut self, members: &[dartforge_frontend::ast::MemberId], owner: Option<&DeclKind>, effectively_private: bool) {
        let a = self.a;
        for &m in members {
            match &a.member(m).kind {
                MemberKind::Field(l) => {
                    // `isInvalidExtensionTypeField`.
                    if !l.static_ && matches!(owner, Some(DeclKind::ExtensionType(_))) {
                        continue;
                    }
                    if l.variables.iter().any(|v| !is_private(self.name_of(v.name.sym)))
                        && let Some(t) = l.ty
                    {
                        self.visit_type(t);
                    }
                }
                MemberKind::Method(f) => {
                    let f = a.function(*f);
                    let Some(n) = f.name else { continue };
                    if is_private(self.name_of(n.sym)) {
                        continue;
                    }
                    let before = self.tps.len();
                    self.tps.extend(f.type_params.iter().map(|t| t.name.sym));
                    if let Some(r) = f.return_type {
                        self.visit_type(r);
                    }
                    self.visit_type_params(&f.type_params);
                    if let Some(ps) = &f.parameters {
                        self.visit_params(ps);
                    }
                    self.tps.truncate(before);
                }
                MemberKind::Constructor(k) => {
                    if k.name.is_some_and(|n| is_private(self.name_of(n.sym))) || matches!(owner, Some(DeclKind::Enum(_))) || effectively_private {
                        continue;
                    }
                    self.visit_params(&k.parameters);
                }
            }
        }
    }
}

fn within(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unidade<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let a = u.ast;
    let source = u.fonte;
    let mut report = |code: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(LintReport { code, span, args: args.iter().map(|x| x.to_string()).collect() });
    };

    // `package_prefixed_library_names`: the 3.6.2 emitter returns without reporting.
    let _ = enabled("package_prefixed_library_names");

    // `avoid_js_rounded_ints`: `IntegerLiteral.value` (with the `-` of the
    // unary parent, as the `AstBuilder` reads it) and the literal's range.
    if enabled("avoid_js_rounded_ints") {
        for e in a.exprs.iter() {
            let ExprKind::Int(sp) = &e.kind else { continue };
            let raw = &source[sp.start..sp.end];
            let negative = raw.starts_with('-');
            let start = if negative { sp.start + (raw.len() - raw[1..].trim_start().len()) } else { sp.start };
            let lexeme: String = source[start..sp.end].chars().filter(|c| *c != '_').collect();
            let value: Option<i64> = match lexeme.strip_prefix("0x").or_else(|| lexeme.strip_prefix("0X")) {
                Some(h) => u64::from_str_radix(h, 16).ok().map(|v| if negative { (v as i64).wrapping_neg() } else { v as i64 }),
                None => {
                    if negative {
                        format!("-{lexeme}").parse::<i64>().ok()
                    } else {
                        lexeme.parse::<i64>().ok()
                    }
                }
            };
            if let Some(v) = value
                && ((v as f64) as i64) != v
            {
                report(&c::AVOID_JS_ROUNDED_INTS, Span { start, end: sp.end }, &[]);
            }
        }
    }

    // `missing_code_block_language_in_doc_comment`.
    if enabled("missing_code_block_language_in_doc_comment") {
        let comments = dartforge_frontend::comments::Comments::scan(source);
        for doc in super::rules14::unit_docs(u, &comments) {
            let lines = super::rules14::doc_lines(source, &doc);
            for b in super::rules14::code_blocks(&lines) {
                if b.fenced && !b.has_info {
                    let (pos, tam) = b.lines[0];
                    report(&c::MISSING_CODE_BLOCK_LANGUAGE_IN_DOC_COMMENT, Span { start: pos, end: pos + tam }, &[]);
                }
            }
        }
    }

    let Some(s) = sem else { return out };
    let program = s.program;
    let table = s.table;

    // `library_private_types_in_public_api`.
    if enabled("library_private_types_in_public_api") {
        let private_file = program.unit(s.unit).path.as_ref().and_then(|p| p.file_name()).is_some_and(|n| n.to_string_lossy().starts_with('_'));
        if !private_file {
            let mut v = PrivateTypeValidator { a, s, interner, tps: Vec::new(), findings: Vec::new() };
            for &did in &u.unit.declarations {
                let d = a.decl(did);
                match &d.kind {
                    DeclKind::Class(x) => {
                        if is_private(interner.resolve(x.name.sym)) {
                            continue;
                        }
                        v.tps = x.type_params.iter().map(|t| t.name.sym).collect();
                        if x.mixin_application {
                            // `ClassTypeAlias`: the superclass and the type parameters.
                            if let Some(sup) = x.extends {
                                v.visit_type(sup);
                            }
                            v.visit_type_params(&x.type_params);
                            continue;
                        }
                        v.visit_type_params(&x.type_params);
                        // The constructor's `isEffectivelyPrivate`: `@internal`,
                        // `sealed`, `abstract final`, `abstract interface`.
                        let internal = d.metadata.iter().any(|m| dartforge_types::anotacoes::e_getter_de(program, interner, s.unit, m, "meta", "internal"));
                        let m = &x.modifiers;
                        let effectively = internal || m.sealed || (m.abstract_ && (m.final_ || m.interface));
                        v.visit_members(&x.members, Some(&d.kind), effectively);
                    }
                    DeclKind::Enum(x) => {
                        if is_private(interner.resolve(x.name.sym)) {
                            continue;
                        }
                        v.tps = x.type_params.iter().map(|t| t.name.sym).collect();
                        v.visit_type_params(&x.type_params);
                        v.visit_members(&x.members, Some(&d.kind), false);
                    }
                    DeclKind::Extension(x) => {
                        if x.name.is_none_or(|n| is_private(interner.resolve(n.sym))) {
                            continue;
                        }
                        v.tps = x.type_params.iter().map(|t| t.name.sym).collect();
                        v.visit_type_params(&x.type_params);
                        v.visit_type(x.on);
                        let internal = d.metadata.iter().any(|m| dartforge_types::anotacoes::e_getter_de(program, interner, s.unit, m, "meta", "internal"));
                        v.visit_members(&x.members, Some(&d.kind), internal);
                    }
                    DeclKind::ExtensionType(x) => {
                        if is_private(interner.resolve(x.name.sym)) {
                            continue;
                        }
                        v.tps = x.type_params.iter().map(|t| t.name.sym).collect();
                        v.visit_type_params(&x.type_params);
                        if !is_private(interner.resolve(x.representation_name.sym)) {
                            v.visit_type(x.representation_type);
                        }
                        let internal = d.metadata.iter().any(|m| dartforge_types::anotacoes::e_getter_de(program, interner, s.unit, m, "meta", "internal"));
                        v.visit_members(&x.members, Some(&d.kind), internal);
                    }
                    DeclKind::Mixin(x) => {
                        if is_private(interner.resolve(x.name.sym)) {
                            continue;
                        }
                        v.tps = x.type_params.iter().map(|t| t.name.sym).collect();
                        for t in x.on.iter() {
                            v.visit_type(*t);
                        }
                        v.visit_type_params(&x.type_params);
                        let internal = d.metadata.iter().any(|m| dartforge_types::anotacoes::e_getter_de(program, interner, s.unit, m, "meta", "internal"));
                        v.visit_members(&x.members, Some(&d.kind), internal);
                    }
                    DeclKind::Function(f) => {
                        let f = a.function(*f);
                        let Some(n) = f.name else { continue };
                        if is_private(interner.resolve(n.sym)) {
                            continue;
                        }
                        v.tps = f.type_params.iter().map(|t| t.name.sym).collect();
                        if let Some(r) = f.return_type {
                            v.visit_type(r);
                        }
                        v.visit_type_params(&f.type_params);
                        if let Some(ps) = &f.parameters {
                            v.visit_params(ps);
                        }
                    }
                    DeclKind::Typedef(x) => {
                        if is_private(interner.resolve(x.name.sym)) {
                            continue;
                        }
                        v.tps = x.type_params.iter().map(|t| t.name.sym).collect();
                        match &x.kind {
                            TypedefKind::Legacy { return_type, parameters } => {
                                if let Some(r) = return_type {
                                    v.visit_type(*r);
                                }
                                v.visit_type_params(&x.type_params);
                                v.visit_params(parameters);
                            }
                            TypedefKind::Alias(t) => {
                                // `functionType?.accept`: only a function type alias.
                                v.visit_type_params(&x.type_params);
                                if matches!(a.ty(*t).kind, TypeKind::Function { .. }) {
                                    v.visit_type(*t);
                                }
                            }
                        }
                    }
                    DeclKind::Variables(l) => {
                        v.tps.clear();
                        if l.variables.iter().any(|x| !is_private(interner.resolve(x.name.sym)))
                            && let Some(t) = l.ty
                        {
                            v.visit_type(t);
                        }
                    }
                }
            }
            for sp in v.findings {
                report(&c::LIBRARY_PRIVATE_TYPES_IN_PUBLIC_API, sp, &[]);
            }
        }
    }

    // `cancel_subscriptions` and `close_sinks`.
    for (rule, code) in [("cancel_subscriptions", &c::CANCEL_SUBSCRIPTIONS), ("close_sinks", &c::CLOSE_SINKS)] {
        if !enabled(rule) {
            continue;
        }
        // The predicates: (the type applies, the method that releases).
        let predicates: Vec<(&str, &str, &str)> = if rule == "cancel_subscriptions" {
            vec![("StreamSubscription", "dart.async", "cancel")]
        } else {
            vec![("Sink", "dart.core", "close"), ("Socket", "dart.io", "destroy")]
        };
        let matches_type = |t: dartforge_types::table::TypeId, method: Option<&str>| {
            predicates.iter().any(|(n, l, m)| method.is_none_or(|x| x == *m) && implements_class(s, interner, t, n, l))
        };
        for (candidate, container) in leak_candidates(s, a) {
            let (name, init, ty, field_id, local) = match candidate {
                LeakCandidate::Field { v, name, init } => {
                    let t = s.outline.variables.get(v.0 as usize).and_then(|d| d.declared_type.or(d.inferred));
                    (name, init, t, Some(v), None)
                }
                LeakCandidate::Local { name, init } => (name, init, s.body.tipo_local(name.span.start), None, Some(name.span.start)),
            };
            // `variable.equals != null && initializer is SimpleIdentifier`.
            if init.is_some_and(|i| matches!(a.expr(i).kind, ExprKind::Identifier(_))) {
                continue;
            }
            let Some(t) = ty else { continue };
            if !matches_type(t, None) {
                continue;
            }
            // The identifier's element is the variable (or its accessor).
            let is_the_variable = |e: ExprId| -> bool {
                match (local, field_id) {
                    (Some(pos), _) => matches!(s.body.get_resolved(e), Some(Resolved::Local(_))) && s.body.declaracao_local(e) == Some(pos),
                    (_, Some(v)) => match s.body.get_resolved(e) {
                        Some(Resolved::Member { member: MemberRef::Variable(x), .. }) => *x == v,
                        Some(Resolved::Member { member: MemberRef::Function(f), .. }) => program.function(*f).variable == Some(v),
                        _ => false,
                    },
                    _ => false,
                }
            };
            // The element is the variable itself (not the accessor).
            let is_the_element = |e: ExprId| -> bool {
                match (local, field_id) {
                    (Some(pos), _) => matches!(s.body.get_resolved(e), Some(Resolved::Local(_))) && s.body.declaracao_local(e) == Some(pos),
                    (_, Some(v)) => matches!(s.body.get_resolved(e), Some(Resolved::Member { member: MemberRef::Variable(x), .. }) if *x == v),
                    _ => false,
                }
            };
            let valid = has_valid_use(s, a, interner, container, name, init, field_id, local.is_some(), &is_the_variable, &is_the_element, &|m| matches_type(t, Some(m)));
            if !valid {
                let end = init.map_or(name.span.end, |i| a.expr(i).span.end);
                report(code, Span { start: name.span.start, end }, &[]);
            }
        }
    }

    // `erase_dart_type_extension_types`.
    if enabled("erase_dart_type_extension_types") {
        for e in a.exprs.iter() {
            let ExprKind::Is { ty, .. } = &e.kind else { continue };
            if let Some(&t) = s.body.tipos_de_anotacoes.get(ty)
                && implements_class(s, interner, t, "DartType", "kernel.ast")
            {
                report(&c::ERASE_DART_TYPE_EXTENSION_TYPES, e.span, &[]);
            }
        }
    }

    // `conditional_uri_does_not_exist`.
    if enabled("conditional_uri_does_not_exist") {
        let file = program.unit(s.unit).path.clone();
        let base = file.as_ref().and_then(|p| p.parent().map(std::path::Path::to_path_buf));
        let config = file.as_ref().and_then(|p| p.ancestors().skip(1).find(|d| d.join(".dart_tool").join("package_config.json").is_file()).map(|d| d.join(".dart_tool").join("package_config.json")));
        let packages = config.and_then(|c| dartforge_elements::PackageConfig::load(&c).ok());
        for d in &u.unit.directives {
            let configurations = match &d.kind {
                DirectiveKind::Import { configurations, .. } | DirectiveKind::Export { configurations, .. } => configurations,
                _ => continue,
            };
            for cfg in configurations {
                let Some(text) = cfg.uri.constant_value().map(|v| v.to_string_lossy()) else { continue };
                let exists = if let Some(name) = text.strip_prefix("dart:") {
                    let name = name.split('/').next().unwrap_or("");
                    !name.is_empty() && SDK_LIBRARIES.contains(&name)
                } else if let Some(rest) = text.strip_prefix("package:") {
                    match (rest.split_once('/'), &packages) {
                        (Some((package, rel)), Some(pc)) => pc.package_dirs.iter().find(|(n, _)| n == package).is_some_and(|(_, dir)| dir.join(rel).is_file()),
                        _ => false,
                    }
                } else if text.contains(':') {
                    // Another scheme: the source is not a known file.
                    true
                } else {
                    base.as_ref().is_some_and(|b| b.join(&text).is_file())
                };
                if !exists {
                    report(&c::CONDITIONAL_URI_DOES_NOT_EXIST, cfg.uri.span, &[text.as_str()]);
                }
            }
        }
    }

    // `prefer_mixin`.
    if enabled("prefer_mixin") {
        for d in a.decls.iter() {
            let with_types: &[TypeId] = match &d.kind {
                DeclKind::Class(x) => &x.with,
                DeclKind::Enum(x) => &x.with,
                _ => continue,
            };
            for &t in with_types {
                let Some(&ty) = s.outline.tipos_escritos.get(&(s.unit, t)) else { continue };
                let Type::Interface { class, .. } = table.get(ty) else { continue };
                let k = program.class(*class);
                if k.kind == ClassKind::Class && !k.modifiers.mixin {
                    let TypeKind::Named { name, .. } = &a.ty(t).kind else { continue };
                    let last_name = name[name.len() - 1];
                    report(&c::PREFER_MIXIN, a.ty(t).span, &[interner.resolve(last_name.sym)]);
                }
            }
        }
    }

    // `avoid_catching_errors`.
    if enabled("avoid_catching_errors") {
        for st in a.stmts.iter() {
            let StmtKind::Try { catches, .. } = &st.kind else { continue };
            for k in catches.iter() {
                let Some(on) = k.on_type else { continue };
                let Some(&t) = s.body.tipos_de_anotacoes.get(&on) else { continue };
                if !implements_class(s, interner, t, "Error", "dart.core") {
                    continue;
                }
                let exact = matches!(table.get(t), Type::Interface { class, .. } if interner.resolve(program.class(*class).name) == "Error" && library_name(s, interner, program.class(*class).library).as_deref() == Some("dart.core"));
                if exact {
                    report(&c::AVOID_CATCHING_ERRORS_CLASS, k.span, &[]);
                } else {
                    let text = dartforge_types::despejo::formatar(table, t, interner, program);
                    report(&c::AVOID_CATCHING_ERRORS_SUBCLASS, k.span, &[text.as_str()]);
                }
            }
        }
    }

    // `avoid_web_libraries_in_flutter`.
    if enabled("avoid_web_libraries_in_flutter") {
        let file = program.library(program.unit(s.unit).library).units.first().and_then(|x| program.unit(*x).path.clone());
        let pubspec = file.as_ref().and_then(|p| p.ancestors().skip(1).map(|d| d.join("pubspec.yaml")).find(|f| f.is_file()));
        let has_flutter = pubspec.and_then(|p| std::fs::read_to_string(p).ok()).is_some_and(|t| {
            let Ok(Some(root)) = crate::naodart::yaml::ler(&t) else { return false };
            let depends = root.campo("dependencies").and_then(|d| d.campo("flutter")).is_some_and(|f| !f.nulo());
            let web = root.campo("flutter").and_then(|f| f.campo("plugin")).and_then(|p| p.campo("platforms")).and_then(|p| p.campo("web")).is_some_and(|w| !w.nulo());
            depends && !web
        });
        if has_flutter {
            for d in &u.unit.directives {
                let DirectiveKind::Import { uri, .. } = &d.kind else { continue };
                let Some(v) = uri.constant_value().map(|v| v.to_string_lossy()) else { continue };
                if matches!(v.as_str(), "dart:html" | "dart:js" | "dart:js_util") {
                    report(&c::AVOID_WEB_LIBRARIES_IN_FLUTTER, d.span, &[]);
                }
            }
        }
    }

    // `cast_nullable_to_non_nullable`.
    if enabled("cast_nullable_to_non_nullable") {
        for e in a.exprs.iter() {
            let ExprKind::As { value, ty } = &e.kind else { continue };
            let (Some(te), Some(&tt)) = (s.body.get_type(*value), s.body.tipos_de_anotacoes.get(ty)) else { continue };
            if matches!(table.get(te), Type::Dynamic) || s.core.is_unknown(table, te) {
                continue;
            }
            if is_nullable(table, te) && is_non_nullable(table, tt) {
                report(&c::CAST_NULLABLE_TO_NON_NULLABLE, e.span, &[]);
            }
        }
    }

    out
}

/// A candidate variable of the `LeakDetectorProcessors`.
enum LeakCandidate {
    Field { v: VariableId, name: dartforge_frontend::ast::Name, init: Option<ExprId> },
    Local { name: dartforge_frontend::ast::Name, init: Option<ExprId> },
}

/// The fields (with the whole unit as the container) and the local variables
/// (with the nearest function body).
fn leak_candidates(s: &super::Semantics<'_>, a: &Ast) -> Vec<(LeakCandidate, Option<Span>)> {
    let mut v = Vec::new();
    for (k, m) in a.members.iter().enumerate() {
        let MemberKind::Field(l) = &m.kind else { continue };
        for (index, x) in l.variables.iter().enumerate() {
            let r = VariableRef::Field { unit: s.unit, member: dartforge_frontend::ast::MemberId(k as u32), index };
            if let Some(i) = s.program.variables.iter().position(|y| y.node == r) {
                v.push((LeakCandidate::Field { v: VariableId(i as u32), name: x.name, init: x.initializer }, None));
            }
        }
    }
    // The function bodies (the nearest `FunctionBody`).
    let mut bodies: Vec<Span> = Vec::new();
    for f in a.functions.iter() {
        match &f.body {
            FunctionBody::Block(s2) => bodies.push(a.stmt(*s2).span),
            FunctionBody::Expression(e) => bodies.push(a.expr(*e).span),
            _ => {}
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind
            && let FunctionBody::Block(s2) = &k.body
        {
            bodies.push(a.stmt(*s2).span);
        }
    }
    for st in a.stmts.iter() {
        let StmtKind::Variables(l) = &st.kind else { continue };
        let body = bodies.iter().filter(|c| within(st.span, **c)).min_by_key(|c| c.end - c.start).copied();
        let Some(body) = body else { continue };
        for x in l.variables.iter() {
            v.push((LeakCandidate::Local { name: x.name, init: x.initializer }, Some(body)));
        }
    }
    v
}

/// The `_ValidUseVisitor`: some valid use in the container (`None`: the unit).
#[allow(clippy::too_many_arguments)]
fn has_valid_use(
    s: &super::Semantics<'_>,
    a: &Ast,
    interner: &Interner,
    container: Option<Span>,
    var_name: dartforge_frontend::ast::Name,
    init: Option<ExprId>,
    field_id: Option<VariableId>,
    is_local: bool,
    is_the_variable: &dyn Fn(ExprId) -> bool,
    is_the_element: &dyn Fn(ExprId) -> bool,
    releases: &dyn Fn(&str) -> bool,
) -> bool {
    let in_container = |sp: Span| container.is_none_or(|c| within(sp, c));
    let decl_span = Span { start: var_name.span.start, end: init.map_or(var_name.span.end, |i| a.expr(i).span.end) };
    // The `realTarget` of cascade sections.
    let mut cascades: std::collections::HashMap<ExprId, ExprId> = std::collections::HashMap::new();
    for e in a.exprs.iter() {
        let ExprKind::Cascade { target, sections, .. } = &e.kind else { continue };
        for &sec in sections.iter() {
            let mut x = sec;
            loop {
                let next = match &a.expr(x).kind {
                    ExprKind::Property { target, .. } | ExprKind::Index { target, .. } | ExprKind::Call { target, .. } | ExprKind::Assign { target, .. } => *target,
                    ExprKind::CascadeTarget => {
                        cascades.insert(x, *target);
                        break;
                    }
                    _ => break,
                };
                x = next;
            }
        }
    }
    let real = |x: ExprId| if matches!(a.expr(x).kind, ExprKind::CascadeTarget) { cascades.get(&x).copied().unwrap_or(x) } else { x };
    for e in a.exprs.iter() {
        if !in_container(e.span) {
            continue;
        }
        match &e.kind {
            ExprKind::Assign { target, value, .. } => {
                if matches!(a.expr(*value).kind, ExprKind::Identifier(_)) {
                    if is_the_variable(*target) {
                        return true;
                    }
                    if let ExprKind::Property { name, target: t2, .. } = &a.expr(*target).kind
                        && !matches!(a.expr(*t2).kind, ExprKind::Identifier(_))
                        && name.sym == var_name.sym
                    {
                        return true;
                    }
                }
            }
            ExprKind::Call { target, arguments } => {
                let ExprKind::Property { target: candidate, name, .. } = &a.expr(*target).kind else { continue };
                let candidate = real(*candidate);
                let method = interner.resolve(name.sym);
                if releases(method) {
                    let by_identifier = matches!(a.expr(candidate).kind, ExprKind::Identifier(_)) && is_the_variable(candidate);
                    let by_postfix = match &a.expr(candidate).kind {
                        ExprKind::Unary { op: UnaryOp::PostfixInc | UnaryOp::PostfixDec | UnaryOp::NullAssert, operand } => {
                            matches!(a.expr(*operand).kind, ExprKind::Identifier(_)) && is_the_variable(*operand)
                        }
                        _ => false,
                    };
                    let by_this = match &a.expr(candidate).kind {
                        ExprKind::Property { target: t2, .. } => matches!(a.expr(*t2).kind, ExprKind::This) && is_the_variable(candidate),
                        _ => false,
                    };
                    let in_decl = within(e.span, decl_span);
                    if by_identifier || by_postfix || by_this || in_decl {
                        return true;
                    }
                }
                // Through the cascade: the identifier target whose element is
                // the field's accessor.
                if field_id.is_some()
                    && matches!(a.expr(candidate).kind, ExprKind::Identifier(_))
                    && matches!(s.body.get_resolved(candidate), Some(Resolved::Member { member: MemberRef::Function(_), .. }))
                    && is_the_variable(candidate)
                {
                    return true;
                }
                // Passed as an argument.
                if arguments.args.iter().any(|x| x.name.is_none() && matches!(a.expr(x.value).kind, ExprKind::Identifier(_)) && is_the_element(x.value)) {
                    return true;
                }
            }
            // `PrefixedIdentifier`: `x.close` (without calling it).
            ExprKind::Property { target, name, .. } => {
                if matches!(a.expr(*target).kind, ExprKind::Identifier(_)) && is_the_element(*target) && releases(interner.resolve(name.sym)) {
                    return true;
                }
            }
            _ => {}
        }
    }
    // The `return x;` of a local.
    if is_local {
        for st in a.stmts.iter() {
            if let StmtKind::Return(Some(v)) = &st.kind
                && in_container(st.span)
                && matches!(a.expr(*v).kind, ExprKind::Identifier(_))
                && is_the_element(*v)
            {
                return true;
            }
        }
    }
    // The fields: a constructor initializer and a `this.x` parameter.
    if let Some(v) = field_id {
        let class_id = s.program.variable(v).class;
        for m in a.members.iter() {
            let MemberKind::Constructor(k) = &m.kind else { continue };
            for i in k.initializers.iter() {
                if let Initializer::Field { name, .. } = i
                    && name.sym == var_name.sym
                    && class_id.is_some()
                {
                    return true;
                }
            }
            if k.parameters.iter().any(|p| p.this_ && p.name.is_some_and(|n| n.sym == var_name.sym)) {
                return true;
            }
        }
    }
    false
}
