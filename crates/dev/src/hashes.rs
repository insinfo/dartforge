//! Two hashes per library: content and public API.
//!
//! PLANO fixes the invalidation rule: the body changed and the API did not →
//! reanalyze and reemit **only** that library; the API changed → it and
//! whoever imports it.
//!
//! The API hash is **structural**: it walks the syntax tree and mixes in the
//! names, modifiers and written types of the public declarations and members
//! — never ranges of text. One extra `print("x")` inside a method shifts all
//! the following spans; if the hash looked at positions or slices, every
//! keystroke in a body would invalidate the dependents, which is exactly
//! what this item exists to avoid. Comments, whitespace and the order of
//! bodies do not show up.
//!
//! A deliberate exception: the **value** of a `const` declaration enters the
//! hash, because a dependent may use it in a constant context (`case`,
//! annotation argument, literal size). It enters as the text of the
//! initializer itself — a range that only changes when the value changes.
//!
//! Private members (`_x`) do not enter: no dependent sees them.
//!
//! The short tags mixed into the API hash (`"t:nome"`, `"d:classe"`…) are
//! only hash input; they are kept as they are so that the digests stay
//! stable.
use dartforge_elements::model::{LibraryId, Program};
use dartforge_frontend::ast::{
    self, Ast, DeclKind, FunctionBody, MemberKind, Parameter, TypeKind, TypeParameter, VariableList,
};

/// 64-bit FNV-1a: cheap and stable across runs (Rust's `DefaultHasher`
/// promises no stability across versions).
#[derive(Clone, Copy)]
pub struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
}

impl Fnv {
    pub fn bytes(&mut self, b: &[u8]) {
        for x in b {
            self.0 ^= u64::from(*x);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    /// Separator mark, so that `ab` + `c` does not collide with `a` + `bc`.
    pub fn sep(&mut self) {
        self.bytes(&[0xff]);
    }
    pub fn bool(&mut self, b: bool) {
        self.bytes(&[u8::from(b)]);
    }
    pub fn num(&mut self, n: u64) {
        self.bytes(&n.to_le_bytes());
    }
    pub fn value(self) -> u64 {
        self.0
    }
}

/// The two hashes of a library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LibraryHashes {
    pub content: u64,
    pub api: u64,
}

/// Computes the two hashes of a library (main unit, parts and patches, in
/// the order they are in the `Program`).
pub fn library_hashes(program: &Program, lib: LibraryId) -> LibraryHashes {
    let mut content = Fnv::default();
    let mut api = Fnv::default();
    // The language version changes the meaning of the text (wildcards, dot
    // shorthands, primary constructors) and can change without touching the
    // source, through the `package_config`'s `languageVersion`: it enters
    // both hashes.
    let version = program.library(lib).features.version();
    for h in [&mut content, &mut api] {
        h.num(u64::from(version.major) << 16 | u64::from(version.minor));
        h.sep();
    }
    for &uid in &program.library(lib).units {
        let u = program.unit(uid);
        content.bytes(u.source.as_bytes());
        content.sep();
        let mut h = Hasher { source: &u.source, ast: &u.ast, f: &mut api };
        // Directives are part of the API: changing an import changes what the
        // library sees and what it reexports. They are short lines without a
        // body, so the text of the directive itself serves as canonical form.
        for d in &u.unit.directives {
            let t = h.slice(d.span.start, d.span.end).to_string();
            h.f.bytes(t.as_bytes());
            h.f.sep();
        }
        for &decl in &u.unit.declarations {
            h.decl(decl);
        }
    }
    LibraryHashes { content: content.value(), api: api.value() }
}

/// Walks the tree mixing in only what is visible from outside.
struct Hasher<'a> {
    source: &'a str,
    ast: &'a Ast,
    f: &'a mut Fnv,
}

impl<'a> Hasher<'a> {
    /// A safe slice of the source (spans come from the lexer and may run past
    /// the end during error recovery).
    fn slice(&self, start: usize, end: usize) -> &'a str {
        let end = end.min(self.source.len());
        let start = start.min(end);
        self.source.get(start..end).unwrap_or("")
    }

    fn name(&mut self, n: &ast::Name) {
        let t = self.slice(n.span.start, n.span.end);
        self.f.bytes(t.as_bytes());
        self.f.sep();
    }

    fn tag(&mut self, s: &str) {
        self.f.bytes(s.as_bytes());
        self.f.sep();
    }

    /// Name of a declaration, to decide whether it is public.
    fn name_text(&self, n: &ast::Name) -> &'a str {
        self.slice(n.span.start, n.span.end)
    }

    fn is_private(&self, n: &ast::Name) -> bool {
        self.name_text(n).starts_with('_')
    }

    /// A written type, in the form it appears in the source (name,
    /// arguments, nullability), without depending on position.
    fn ty(&mut self, id: ast::TypeId) {
        let t = self.ast.ty(id);
        self.f.bool(t.nullable);
        match &t.kind {
            TypeKind::Named { name, args } => {
                self.tag("t:nome");
                for n in name.iter() {
                    self.name(n);
                }
                self.f.num(args.len() as u64);
                for &a in args.iter() {
                    self.ty(a);
                }
            }
            TypeKind::Void => self.tag("t:void"),
            TypeKind::Function { return_type, type_params, parameters } => {
                self.tag("t:fn");
                self.opt_ty(*return_type);
                self.type_params(type_params);
                self.params(parameters);
            }
            TypeKind::Record { positional, named } => {
                self.tag("t:record");
                self.f.num(positional.len() as u64);
                for &p in positional.iter() {
                    self.ty(p);
                }
                for (n, ty) in named.iter() {
                    self.name(n);
                    self.ty(*ty);
                }
            }
        }
    }

    fn opt_ty(&mut self, id: Option<ast::TypeId>) {
        match id {
            Some(t) => self.ty(t),
            None => self.tag("t:ausente"),
        }
    }

    fn type_params(&mut self, ps: &[TypeParameter]) {
        self.f.num(ps.len() as u64);
        for p in ps {
            self.name(&p.name);
            self.opt_ty(p.bound);
        }
    }

    /// Formal parameters: what a caller needs to know. The default value
    /// does not enter — Dart evaluates it in the callee's own body, so
    /// changing it recompiles the library that declares it, not the
    /// callers.
    fn params(&mut self, ps: &[Parameter]) {
        self.f.num(ps.len() as u64);
        for p in ps {
            self.f.num(match p.kind {
                ast::ParameterKind::Required => 0,
                ast::ParameterKind::Optional => 1,
                ast::ParameterKind::Named => 2,
            });
            self.f.bool(p.required);
            self.f.bool(p.covariant);
            self.f.bool(p.this_);
            self.f.bool(p.super_);
            self.f.bool(p.default_value.is_some());
            match &p.name {
                Some(n) => self.name(n),
                None => self.tag("p:sem-nome"),
            }
            self.opt_ty(p.ty);
            self.type_params(&p.function_type_params);
            if let Some(fp) = &p.function_parameters {
                self.tag("p:fn");
                self.params(fp);
            }
        }
    }

    /// Signature of a function/method/getter/setter/operator.
    fn function(&mut self, fid: ast::FunctionId) {
        let f = self.ast.function(fid);
        self.f.num(match f.kind {
            ast::FunctionKind::Function => 0,
            ast::FunctionKind::Getter => 1,
            ast::FunctionKind::Setter => 2,
            ast::FunctionKind::Operator => 3,
        });
        self.f.bool(f.external);
        self.f.bool(f.static_);
        // The `async`/`sync*` modifier changes the return type seen from
        // outside (`Future`, `Stream`, `Iterable`), so it counts as
        // signature.
        self.f.num(match f.modifier {
            ast::AsyncModifier::None => 0,
            ast::AsyncModifier::Async => 1,
            ast::AsyncModifier::AsyncStar => 2,
            ast::AsyncModifier::SyncStar => 3,
        });
        // Having a body or not is visible (abstract/external vs concrete).
        self.f.bool(matches!(f.body, FunctionBody::Empty));
        self.opt_ty(f.return_type);
        self.type_params(&f.type_params);
        match &f.parameters {
            Some(ps) => self.params(ps),
            None => self.tag("f:sem-lista"),
        }
    }

    /// Variable/field declarations: modifiers, type and public names. For
    /// `const`, the value enters through the text of the initializer itself.
    fn variables(&mut self, list: &VariableList, prefix: &str) {
        let mut any = false;
        for v in list.variables.iter() {
            if self.is_private(&v.name) {
                continue;
            }
            if !any {
                self.tag(prefix);
                self.f.bool(list.external);
                self.f.bool(list.static_);
                self.f.bool(list.abstract_);
                self.f.bool(list.covariant);
                self.f.bool(list.late);
                self.f.bool(list.final_);
                self.f.bool(list.const_);
                self.f.bool(list.var_);
                self.opt_ty(list.ty);
                any = true;
            }
            self.name(&v.name);
            self.f.bool(v.initializer.is_some());
            if list.const_ {
                if let Some(e) = v.initializer {
                    let s = self.ast.expr(e).span;
                    let t = self.slice(s.start, s.end).to_string();
                    self.f.bytes(t.as_bytes());
                    self.f.sep();
                }
            }
        }
    }

    /// A top-level declaration.
    fn decl(&mut self, decl: ast::DeclId) {
        let d = self.ast.decl(decl);
        match &d.kind {
            DeclKind::Function(fid) => {
                let f = self.ast.function(*fid);
                let Some(name) = f.name else { return };
                if self.is_private(&name) {
                    return;
                }
                self.tag("d:fn");
                self.name(&name);
                self.function(*fid);
            }
            DeclKind::Variables(list) => self.variables(list, "d:var"),
            DeclKind::Class(c) => {
                if self.is_private(&c.name) {
                    return;
                }
                self.tag("d:classe");
                self.name(&c.name);
                let m = c.modifiers;
                for b in [m.abstract_, m.base, m.interface, m.final_, m.sealed, m.mixin, c.mixin_application] {
                    self.f.bool(b);
                }
                self.type_params(&c.type_params);
                self.opt_ty(c.extends);
                self.types(&c.with);
                self.types(&c.implements);
                self.members(&c.members);
            }
            DeclKind::Mixin(mx) => {
                if self.is_private(&mx.name) {
                    return;
                }
                self.tag("d:mixin");
                self.name(&mx.name);
                self.f.bool(mx.base);
                self.type_params(&mx.type_params);
                self.types(&mx.on);
                self.types(&mx.implements);
                self.members(&mx.members);
            }
            DeclKind::Enum(e) => {
                if self.is_private(&e.name) {
                    return;
                }
                self.tag("d:enum");
                self.name(&e.name);
                self.type_params(&e.type_params);
                self.types(&e.with);
                self.types(&e.implements);
                // The name and order of the constants are API (the index is observable).
                self.f.num(e.constants.len() as u64);
                for c in &e.constants {
                    self.name(&c.name);
                }
                self.members(&e.members);
            }
            DeclKind::Extension(e) => {
                if e.name.is_some_and(|n| self.is_private(&n)) {
                    return;
                }
                self.tag("d:extension");
                if let Some(n) = e.name {
                    self.name(&n);
                }
                self.type_params(&e.type_params);
                self.ty(e.on);
                self.members(&e.members);
            }
            DeclKind::ExtensionType(e) => {
                if self.is_private(&e.name) {
                    return;
                }
                self.tag("d:extension-type");
                self.name(&e.name);
                self.f.bool(e.const_);
                if let Some(c) = e.constructor {
                    self.name(&c);
                }
                self.type_params(&e.type_params);
                self.ty(e.representation_type);
                self.name(&e.representation_name);
                self.types(&e.implements);
                self.members(&e.members);
            }
            DeclKind::Typedef(t) => {
                if self.is_private(&t.name) {
                    return;
                }
                self.tag("d:typedef");
                self.name(&t.name);
                self.type_params(&t.type_params);
                match &t.kind {
                    ast::TypedefKind::Alias(ty) => self.ty(*ty),
                    ast::TypedefKind::Legacy { return_type, parameters } => {
                        self.opt_ty(*return_type);
                        self.params(parameters);
                    }
                }
            }
        }
    }

    fn types(&mut self, ts: &[ast::TypeId]) {
        self.f.num(ts.len() as u64);
        for &t in ts {
            self.ty(t);
        }
    }

    /// Public members of a class, mixin, enum, extension or extension type.
    fn members(&mut self, members: &[ast::MemberId]) {
        for &mid in members {
            let m = self.ast.member(mid);
            match &m.kind {
                MemberKind::Method(fid) => {
                    let f = self.ast.function(*fid);
                    let Some(name) = f.name else { continue };
                    if self.is_private(&name) {
                        continue;
                    }
                    self.tag("m:metodo");
                    self.name(&name);
                    self.function(*fid);
                }
                MemberKind::Constructor(c) => {
                    if c.name.is_some_and(|n| self.is_private(&n)) {
                        continue;
                    }
                    self.tag("m:ctor");
                    match &c.name {
                        Some(n) => self.name(n),
                        None => self.tag("m:sem-nome"),
                    }
                    self.f.bool(c.external);
                    self.f.bool(c.const_);
                    self.f.bool(c.factory);
                    self.f.bool(c.redirect.is_some());
                    if let Some(r) = &c.redirect {
                        self.ty(r.ty);
                        if let Some(n) = r.constructor {
                            self.name(&n);
                        }
                    }
                    self.params(&c.parameters);
                    // A `const` constructor is evaluated by its callers: the
                    // initializer list enters through the text of each one.
                    if c.const_ {
                        for i in c.initializers.iter() {
                            let s = match i {
                                ast::Initializer::Field { span, .. }
                                | ast::Initializer::Super { span, .. }
                                | ast::Initializer::Redirect { span, .. }
                                | ast::Initializer::Assert { span, .. } => *span,
                            };
                            let t = self.slice(s.start, s.end).to_string();
                            self.f.bytes(t.as_bytes());
                            self.f.sep();
                        }
                    }
                }
                MemberKind::Field(list) => self.variables(list, "m:campo"),
            }
        }
    }
}
