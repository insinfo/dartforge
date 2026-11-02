//! Replacing a unit's text without reloading the program
//! (docs/LSP-ESPECIFICACAO.md §16.6, stage I1): reanalyzes the unit, compares
//! the **API signature** of the old version with the new one's and, when only
//! the inside of bodies changed (class **Body**), swaps the tree and the text
//! in place.
//!
//! The signature is the file's token sequence outside the bodies (the
//! analyzer's `apiSignature`, which is also made of tokens): left out are the
//! inside of blocks and of `=>` expressions of top-level functions, methods
//! and constructors, and the initializers of variables and fields with a
//! written type that are neither `const` nor instance `final` in a class
//! with a `const` constructor. The directives' tokens form a separate
//! signature (class **Directives**: the library graph may change). Comments
//! and whitespace do not count; the gluing of `>` (which composes `>>`)
//! does.
//!
//! `mixin` members have no body outside the signature: the names called via
//! `super` inside them belong to the signature in the analyzer, and an edit
//! inside one of them refuses the swap (a reload), which is conservative.
//!
//! The elements keep ids from the declaration, member and function arenas
//! (`DeclRef`, `FunctionRef`, `VariableRef`), checked one by one; and
//! `(UnitId, ast::TypeId)` pairs of clauses and bounds, which are
//! **remapped** by walking the signatures' type annotations in parallel (the
//! type arena is filled in parse order, by signatures and bodies: a new type
//! in a body shifts the ids of what comes after). The types outline keeps
//! the same pairs (`OutlineTypes::written_types`); whoever keeps it applies
//! [`Previous::type_map`] (and the inverse, when undoing).

use crate::model::{Program, UnitId};
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, Ast, CompilationUnit, DeclKind, FunctionBody, MemberKind, TypeId,
};
use dartforge_frontend::token::{Kind, Op, Token};
use dartforge_intern::Interner;
use std::collections::HashMap;

/// Why the swap was refused (the caller reloads the program).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The declaration, member or function arenas do not correspond one to one,
    /// or the signatures' type annotations do not line up.
    ShapeChanged(&'static str),
    /// One of the versions has a lexical error: the signature is not
    /// comparable.
    Lexical,
    /// The declarations' API signature changed.
    Signature,
    /// The directives changed.
    Directives,
}

/// A body of the unit, by arena ids (stable in an accepted swap).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnitBody {
    /// Top-level function, method, getter, setter or operator.
    Function(ast::FunctionId),
    /// Constructor body (the initializers are signature).
    Constructor(ast::MemberId),
    /// Top-level variable initializer.
    TopLevelVariable { decl: ast::DeclId, index: usize },
    /// Field initializer.
    Field { member: ast::MemberId, index: usize },
}

/// What the swap replaced, to undo it with [`restore_unit`].
pub struct Previous {
    unit_id: UnitId,
    source: String,
    ast: Ast,
    unit: CompilationUnit,
    skipped: Vec<Span>,
    reference: dartforge_diagnostics::AnalyzerReference,
    /// Old → new `TypeId` of the signatures' annotations.
    map: HashMap<TypeId, TypeId>,
    changed: Vec<UnitBody>,
}

impl Previous {
    /// The swapped unit.
    pub fn unit(&self) -> UnitId {
        self.unit_id
    }

    /// The bodies whose text changed.
    pub fn changed_bodies(&self) -> &[UnitBody] {
        &self.changed
    }

    /// The old tree's `TypeId` → the new one's, for the signatures' annotations
    /// (the only ones the outline keeps).
    pub fn type_map(&self) -> &HashMap<TypeId, TypeId> {
        &self.map
    }

    /// The inverse of [`Previous::type_map`] (the map is a bijection).
    pub fn inverse_map(&self) -> HashMap<TypeId, TypeId> {
        self.map.iter().map(|(&a, &b)| (b, a)).collect()
    }
}

/// The API signature of a unit's `text`, as a hash: the directives' tokens,
/// the declarations' signature tokens and the sequence of body owners — what
/// [`swap_unit`] compares. Two texts with the same value differ only inside
/// bodies (class **Body**, docs/LSP-ESPECIFICACAO.md §16.6), so the libraries
/// that depend on the unit see the same declarations. `None` when the text has
/// a lexical error (it has no comparable signature).
///
/// The text is parsed with the current language features: the comparison is
/// meaningful between two versions of the same unit, parsed the same way.
///
/// ```
/// use dartforge_elements::incremental::api_signature;
/// let a = api_signature("int f() { return 1; }\nclass C { void m() {} }\n");
/// let b = api_signature("int f() { return 2 + 3; }\nclass C { void m() { f(); } }\n");
/// let c = api_signature("int f(int x) { return 1; }\nclass C { void m() {} }\n");
/// assert!(a.is_some());
/// assert_eq!(a, b);
/// assert_ne!(a, c);
/// ```
pub fn api_signature(text: &str) -> Option<u64> {
    let mut names = Interner::new();
    let parsed = dartforge_frontend::parser::parse(text, &mut names);
    api_signature_of(text, &parsed.ast, &parsed.unit)
}

/// [`api_signature`] over a tree already parsed from `text`.
pub fn api_signature_of(text: &str, ast: &Ast, unit: &CompilationUnit) -> Option<u64> {
    let tokens = dartforge_frontend::lexer::lex(text).ok()?;
    Some(api_signature_with(text, &tokens, ast, unit))
}

/// [`api_signature_of`] and [`api_digest`] of a unit, with one lexing of
/// `text`.
pub fn api_signature_and_digest(
    text: &str,
    ast: &Ast,
    unit: &CompilationUnit,
) -> (Option<u64>, Option<ApiDigest>) {
    match dartforge_frontend::lexer::lex(text) {
        Ok(tokens) => (
            Some(api_signature_with(text, &tokens, ast, unit)),
            Some(api_digest_with(text, &tokens, ast, unit)),
        ),
        Err(_) => (None, None),
    }
}

fn api_signature_with(text: &str, tokens: &[Token], ast: &Ast, unit: &CompilationUnit) -> u64 {
    use std::hash::{Hash, Hasher};
    let bodies = regions(ast, false);
    let (directives, signature) = split(text, tokens, &directive_spans(unit), &bodies);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    directives.hash(&mut hasher);
    signature.hash(&mut hasher);
    for r in &bodies {
        std::mem::discriminant(&r.owner).hash(&mut hasher);
    }
    hasher.finish()
}

/// Replaces the text of unit `u` with `text` when the API signature did not
/// change; returns the previous state to undo it.
///
/// `initializers_as_body`: the initializers of variables and fields all
/// count as body (completion, §16.7 rule 5: a variable's inferred type does
/// not matter for the suggestions inside its own initializer, and nothing
/// the speculative swap produces is written).
pub fn swap_unit(
    program: &mut Program,
    names_: &mut Interner,
    u: UnitId,
    text: &str,
    initializers_as_body: bool,
) -> Result<Previous, Refusal> {
    let features = program.unit(u).features;
    let new_ = dartforge_frontend::parser::parse_with(text, names_, features);
    let (map, changed) = {
        let old_ = program.unit(u);
        verify_shape(&old_.ast, &old_.unit, &new_.ast, &new_.unit)?;
        let (Ok(tv), Ok(tn)) = (
            dartforge_frontend::lexer::lex(&old_.source),
            dartforge_frontend::lexer::lex(text),
        ) else {
            return Err(Refusal::Lexical);
        };
        let rv = regions(&old_.ast, initializers_as_body);
        let rn = regions(&new_.ast, initializers_as_body);
        if rv.len() != rn.len() || rv.iter().zip(rn.iter()).any(|(a, b)| a.owner != b.owner) {
            return Err(Refusal::Signature);
        }
        let (dir_v, sig_v) = split(&old_.source, &tv, &directive_spans(&old_.unit), &rv);
        let (dir_n, sig_n) = split(text, &tn, &directive_spans(&new_.unit), &rn);
        if dir_v != dir_n {
            return Err(Refusal::Directives);
        }
        if sig_v != sig_n {
            return Err(Refusal::Signature);
        }
        let changed: Vec<UnitBody> = rv
            .iter()
            .zip(rn.iter())
            .filter(|(a, b)| {
                old_.source.get(a.span.start..a.span.end) != text.get(b.span.start..b.span.end)
            })
            .map(|(a, _)| a.owner)
            .collect();
        let (lv, ln) = (signature_types(&old_.ast), signature_types(&new_.ast));
        if lv.len() != ln.len() {
            return Err(Refusal::ShapeChanged("signature types"));
        }
        for (a, b) in lv.iter().zip(ln.iter()) {
            let (ta, tb) = (old_.ast.ty(*a), new_.ast.ty(*b));
            if ta.nullable != tb.nullable
                || std::mem::discriminant(&ta.kind) != std::mem::discriminant(&tb.kind)
            {
                return Err(Refusal::ShapeChanged("signature types"));
            }
        }
        let map: HashMap<TypeId, TypeId> = lv.into_iter().zip(ln).collect();
        (map, changed)
    };
    {
        let mut ps = pairs(program);
        if ps.iter().any(|p| p.0 == u && !map.contains_key(&p.1)) {
            return Err(Refusal::ShapeChanged("element types"));
        }
        for p in ps.iter_mut() {
            if p.0 == u {
                p.1 = map[&p.1];
            }
        }
    }
    let un = &mut program.units[u.0 as usize];
    Ok(Previous {
        unit_id: u,
        source: std::mem::replace(&mut un.source, text.to_string()),
        ast: std::mem::replace(&mut un.ast, new_.ast),
        unit: std::mem::replace(&mut un.unit, new_.unit),
        skipped: std::mem::replace(&mut un.skipped, new_.skipped),
        reference: std::mem::replace(&mut un.reference, new_.reference),
        map,
        changed,
    })
}

/// Undoes a [`swap_unit`] (the elements' pairs go back to the old
/// tree's ids).
pub fn restore_unit(program: &mut Program, previous: Previous) {
    let u = previous.unit_id;
    let inverse = previous.inverse_map();
    for p in pairs(program) {
        if p.0 == u
            && let Some(&t) = inverse.get(&p.1)
        {
            p.1 = t;
        }
    }
    let un = &mut program.units[u.0 as usize];
    un.source = previous.source;
    un.ast = previous.ast;
    un.unit = previous.unit;
    un.skipped = previous.skipped;
    un.reference = previous.reference;
}

/// The one-to-one correspondence of the arenas the elements cite.
fn verify_shape(
    old_: &Ast,
    old_unit: &CompilationUnit,
    a: &Ast,
    new_unit: &CompilationUnit,
) -> Result<(), Refusal> {
    if old_.decls.len() != a.decls.len() {
        return Err(Refusal::ShapeChanged("declarations"));
    }
    if old_.members.len() != a.members.len() {
        return Err(Refusal::ShapeChanged("members"));
    }
    if old_.functions.len() != a.functions.len() {
        return Err(Refusal::ShapeChanged("functions"));
    }
    for (d0, d1) in old_.decls.iter().zip(a.decls.iter()) {
        if std::mem::discriminant(&d0.kind) != std::mem::discriminant(&d1.kind)
            || decl_name(&d0.kind) != decl_name(&d1.kind)
        {
            return Err(Refusal::ShapeChanged("declaration"));
        }
        if let (DeclKind::Variables(l0), DeclKind::Variables(l1)) = (&d0.kind, &d1.kind)
            && !same_names(l0, l1)
        {
            return Err(Refusal::ShapeChanged("variables"));
        }
    }
    for (m0, m1) in old_.members.iter().zip(a.members.iter()) {
        if std::mem::discriminant(&m0.kind) != std::mem::discriminant(&m1.kind)
            || member_name(&m0.kind) != member_name(&m1.kind)
        {
            return Err(Refusal::ShapeChanged("member"));
        }
        if let (MemberKind::Field(l0), MemberKind::Field(l1)) = (&m0.kind, &m1.kind)
            && !same_names(l0, l1)
        {
            return Err(Refusal::ShapeChanged("fields"));
        }
    }
    for (f0, f1) in old_.functions.iter().zip(a.functions.iter()) {
        if f0.name.map(|n| n.sym) != f1.name.map(|n| n.sym) {
            return Err(Refusal::ShapeChanged("function"));
        }
    }
    if old_unit.declarations != new_unit.declarations {
        return Err(Refusal::ShapeChanged("unit declarations"));
    }
    Ok(())
}

/// The whole list of names (the `index` of a `VariableRef` points into it).
fn same_names(a: &ast::VariableList, b: &ast::VariableList) -> bool {
    a.variables.len() == b.variables.len()
        && a.variables
            .iter()
            .zip(b.variables.iter())
            .all(|(x, y)| x.name.sym == y.name.sym)
}

/// A body range: the tokens inside it stay out of the signature.
struct Region {
    owner: UnitBody,
    span: Span,
    /// Block: the braces stay in the signature ("empty body?"), only the inside
    /// leaves.
    strict: bool,
}

/// The unit's bodies, in arena order.
fn regions(a: &Ast, initializers_as_body: bool) -> Vec<Region> {
    let mut v = Vec::new();
    let body_node = |v: &mut Vec<Region>, owner: UnitBody, body: &FunctionBody| match body {
        FunctionBody::Block(s) => v.push(Region {
            owner,
            span: a.stmt(*s).span,
            strict: true,
        }),
        FunctionBody::Expression(e) => v.push(Region {
            owner,
            span: a.expr(*e).span,
            strict: false,
        }),
        FunctionBody::Empty | FunctionBody::Native(_) => {}
    };
    let members_ = |v: &mut Vec<Region>, members_: &[ast::MemberId], has_const: bool| {
        for &m in members_ {
            match &a.member(m).kind {
                MemberKind::Method(f) => body_node(v, UnitBody::Function(*f), &a.function(*f).body),
                MemberKind::Constructor(k) => body_node(v, UnitBody::Constructor(m), &k.body),
                MemberKind::Field(l) => {
                    let signature =
                        l.ty.is_none() || l.const_ || (l.final_ && !l.static_ && has_const);
                    if initializers_as_body || !signature {
                        for (k, x) in l.variables.iter().enumerate() {
                            if let Some(e) = x.initializer {
                                v.push(Region {
                                    owner: UnitBody::Field {
                                        member: m,
                                        index: k,
                                    },
                                    span: a.expr(e).span,
                                    strict: false,
                                });
                            }
                        }
                    }
                }
            }
        }
    };
    let has_const_constructor = |members_: &[ast::MemberId]| {
        members_
            .iter()
            .any(|&m| matches!(&a.member(m).kind, MemberKind::Constructor(k) if k.const_))
    };
    for (i, d) in a.decls.iter().enumerate() {
        let decl = ast::DeclId(i as u32);
        match &d.kind {
            DeclKind::Function(f) => {
                body_node(&mut v, UnitBody::Function(*f), &a.function(*f).body)
            }
            DeclKind::Variables(l) => {
                if initializers_as_body || (l.ty.is_some() && !l.const_) {
                    for (k, x) in l.variables.iter().enumerate() {
                        if let Some(e) = x.initializer {
                            v.push(Region {
                                owner: UnitBody::TopLevelVariable { decl, index: k },
                                span: a.expr(e).span,
                                strict: false,
                            });
                        }
                    }
                }
            }
            DeclKind::Class(c) => members_(&mut v, &c.members, has_const_constructor(&c.members)),
            DeclKind::Enum(e) => members_(&mut v, &e.members, true),
            DeclKind::ExtensionType(x) => members_(
                &mut v,
                &x.members,
                x.const_ || has_const_constructor(&x.members),
            ),
            DeclKind::Extension(x) => members_(&mut v, &x.members, false),
            DeclKind::Mixin(_) | DeclKind::Typedef(_) => {}
        }
    }
    v
}

fn directive_spans(unit: &CompilationUnit) -> Vec<(Span, bool)> {
    let mut v: Vec<(Span, bool)> = unit.directives.iter().map(|d| (d.span, false)).collect();
    v.sort_by_key(|(s, _)| s.start);
    v
}

/// Is the token in one of the ranges (sorted by start, non-overlapping)?
fn inside(excerpts: &[(Span, bool)], t: Span) -> bool {
    let i = excerpts.partition_point(|(s, _)| s.start <= t.start);
    if i == 0 {
        return false;
    }
    let (s, strict) = excerpts[i - 1];
    if strict {
        t.start > s.start && t.end < s.end
    } else {
        t.start >= s.start && t.end <= s.end
    }
}

/// A signature token: kind, text and the gluing of a `>`.
type Lexeme<'s> = (Kind, &'s str, bool);

/// The directives' tokens and the declarations' signature tokens.
fn split<'s>(
    source: &'s str,
    tokens: &[Token],
    directives: &[(Span, bool)],
    bodies: &[Region],
) -> (Vec<Lexeme<'s>>, Vec<Lexeme<'s>>) {
    let mut excerpts: Vec<(Span, bool)> = bodies.iter().map(|r| (r.span, r.strict)).collect();
    excerpts.sort_by_key(|(s, _)| s.start);
    let (mut d, mut s) = (Vec::new(), Vec::new());
    for t in tokens {
        let lexeme = (
            t.kind,
            source.get(t.span.start..t.span.end).unwrap_or(""),
            t.glued && matches!(t.kind, Kind::Op(Op::Gt)),
        );
        if inside(directives, t.span) {
            d.push(lexeme);
        } else if !inside(&excerpts, t.span) {
            s.push(lexeme);
        }
    }
    (d, s)
}

/// The signatures' type annotations, in an order determined by their shape
/// (the same in both trees when the signatures are equal).
fn signature_types(a: &Ast) -> Vec<TypeId> {
    let mut v = Vec::new();
    let mut declared: Vec<ast::FunctionId> = Vec::new();
    for d in &a.decls {
        annotations(a, &d.metadata, &mut v);
        match &d.kind {
            DeclKind::Class(c) => {
                type_parameters(a, &c.type_params, &mut v);
                if let Some(t) = c.extends {
                    kind(a, t, &mut v);
                }
                for t in c.with.iter().chain(c.implements.iter()) {
                    kind(a, *t, &mut v);
                }
            }
            DeclKind::Mixin(m) => {
                type_parameters(a, &m.type_params, &mut v);
                for t in m.on.iter().chain(m.implements.iter()) {
                    kind(a, *t, &mut v);
                }
            }
            DeclKind::Enum(e) => {
                type_parameters(a, &e.type_params, &mut v);
                for t in e.with.iter().chain(e.implements.iter()) {
                    kind(a, *t, &mut v);
                }
                for k in &e.constants {
                    annotations(a, &k.metadata, &mut v);
                    for t in k.type_args.iter() {
                        kind(a, *t, &mut v);
                    }
                }
            }
            DeclKind::Extension(x) => {
                type_parameters(a, &x.type_params, &mut v);
                kind(a, x.on, &mut v);
            }
            DeclKind::ExtensionType(x) => {
                type_parameters(a, &x.type_params, &mut v);
                annotations(a, &x.representation_metadata, &mut v);
                kind(a, x.representation_type, &mut v);
                for t in x.implements.iter() {
                    kind(a, *t, &mut v);
                }
            }
            DeclKind::Typedef(t) => {
                type_parameters(a, &t.type_params, &mut v);
                match &t.kind {
                    ast::TypedefKind::Alias(x) => kind(a, *x, &mut v),
                    ast::TypedefKind::Legacy {
                        return_type,
                        parameters,
                    } => {
                        if let Some(r) = return_type {
                            kind(a, *r, &mut v);
                        }
                        parameters_(a, parameters, &mut v);
                    }
                }
            }
            DeclKind::Function(f) => declared.push(*f),
            DeclKind::Variables(l) => {
                if let Some(t) = l.ty {
                    kind(a, t, &mut v);
                }
            }
        }
    }
    for m in &a.members {
        annotations(a, &m.metadata, &mut v);
        match &m.kind {
            MemberKind::Field(l) => {
                if let Some(t) = l.ty {
                    kind(a, t, &mut v);
                }
            }
            MemberKind::Method(f) => declared.push(*f),
            MemberKind::Constructor(k) => {
                parameters_(a, &k.parameters, &mut v);
                if let Some(r) = &k.redirect {
                    kind(a, r.ty, &mut v);
                }
            }
        }
    }
    for f in declared {
        let f = a.function(f);
        if let Some(r) = f.return_type {
            kind(a, r, &mut v);
        }
        type_parameters(a, &f.type_params, &mut v);
        if let Some(ps) = &f.parameters {
            parameters_(a, ps, &mut v);
        }
    }
    v
}

fn kind(a: &Ast, t: TypeId, v: &mut Vec<TypeId>) {
    v.push(t);
    match &a.ty(t).kind {
        ast::TypeKind::Named { args, .. } => {
            for x in args.iter() {
                kind(a, *x, v);
            }
        }
        ast::TypeKind::Void => {}
        ast::TypeKind::Function {
            return_type,
            type_params,
            parameters,
        } => {
            if let Some(r) = return_type {
                kind(a, *r, v);
            }
            type_parameters(a, type_params, v);
            parameters_(a, parameters, v);
        }
        ast::TypeKind::Record { positional, named } => {
            for x in positional.iter() {
                kind(a, *x, v);
            }
            for (_, x) in named.iter() {
                kind(a, *x, v);
            }
        }
    }
}

fn type_parameters(a: &Ast, ps: &[ast::TypeParameter], v: &mut Vec<TypeId>) {
    for p in ps {
        annotations(a, &p.metadata, v);
        if let Some(b) = p.bound {
            kind(a, b, v);
        }
    }
}

fn parameters_(a: &Ast, ps: &[ast::Parameter], v: &mut Vec<TypeId>) {
    for p in ps {
        annotations(a, &p.metadata, v);
        if let Some(t) = p.ty {
            kind(a, t, v);
        }
        type_parameters(a, &p.function_type_params, v);
        if let Some(fp) = &p.function_parameters {
            parameters_(a, fp, v);
        }
    }
}

fn annotations(a: &Ast, list: &[ast::Annotation], v: &mut Vec<TypeId>) {
    for x in list {
        for t in &x.type_args {
            kind(a, *t, v);
        }
    }
}

/// The `(UnitId, ast::TypeId)` pairs stored in the elements.
fn pairs(program: &mut Program) -> Vec<&mut (UnitId, TypeId)> {
    let mut v: Vec<&mut (UnitId, TypeId)> = Vec::new();
    for c in program.classes.iter_mut() {
        let crate::model::ClassElement {
            type_params,
            supertype,
            mixins,
            interfaces,
            on,
            ..
        } = c;
        v.extend(type_params.iter_mut().filter_map(|p| p.bound.as_mut()));
        v.extend(supertype.as_mut());
        v.extend(mixins.iter_mut());
        v.extend(interfaces.iter_mut());
        v.extend(on.iter_mut());
    }
    for x in program.extensions.iter_mut() {
        let crate::model::ExtensionElement {
            type_params, on, ..
        } = x;
        v.extend(type_params.iter_mut().filter_map(|p| p.bound.as_mut()));
        v.push(on);
    }
    for t in program.typedefs.iter_mut() {
        v.extend(t.type_params.iter_mut().filter_map(|p| p.bound.as_mut()));
    }
    v
}

fn decl_name(k: &DeclKind) -> Option<dartforge_intern::SymbolId> {
    match k {
        DeclKind::Class(x) => Some(x.name.sym),
        DeclKind::Mixin(x) => Some(x.name.sym),
        DeclKind::Enum(x) => Some(x.name.sym),
        DeclKind::Extension(x) => x.name.map(|n| n.sym),
        DeclKind::ExtensionType(x) => Some(x.name.sym),
        DeclKind::Typedef(x) => Some(x.name.sym),
        DeclKind::Variables(l) => l.variables.first().map(|v| v.name.sym),
        _ => None,
    }
}

fn member_name(k: &MemberKind) -> Option<dartforge_intern::SymbolId> {
    match k {
        MemberKind::Constructor(c) => c.name.map(|n| n.sym),
        MemberKind::Field(l) => l.variables.first().map(|v| v.name.sym),
        _ => None,
    }
}

/// One declaration of a unit, as [`api_digest`] sees it: a top-level
/// declaration (its name) or a member of one (with its `owner`), with the
/// digest of its signature tokens (those of [`api_signature`]: bodies left
/// out) and the identifier words in them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApiDeclaration {
    /// The name a reference elsewhere uses: the declaration's, or the
    /// member's (empty for an unnamed constructor or extension).
    pub name: String,
    /// The top-level declaration that owns a member.
    pub owner: Option<String>,
    /// Whether the declaration can be used without its name being written
    /// (an operator, an unnamed extension, a member that the language calls
    /// implicitly: `call`, `iterator`, `toString`…).
    pub implicit: bool,
    /// Whether it is a class-like declaration (class, mixin, enum, extension
    /// type), whose subtypes in the same library matter to exhaustiveness.
    pub class_like: bool,
    pub digest: u64,
    /// [`word_hash`]es of the identifiers of its signature, sorted.
    pub words: Vec<u64>,
}

/// The API of a unit, declaration by declaration ([`ApiDeclaration`]), and
/// the digest of its directives: what tells which names an API change of the
/// unit changed (the LSP reanalyzes only the dependents that use them).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApiDigest {
    pub directives: u64,
    pub declarations: Vec<ApiDeclaration>,
}

/// The hash of an identifier, as [`api_digest`] and [`text_words`] record it.
pub fn word_hash(word: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    word.hash(&mut h);
    h.finish()
}

/// The [`word_hash`]es of every identifier-like word of `text` (letters,
/// digits, `_` and `$`), comments and strings included, sorted and without
/// repetitions: an over-approximation of the names a file can refer to.
///
/// ```
/// use dartforge_elements::incremental::{text_words, word_hash};
/// let w = text_words("a.b(c); // d");
/// assert!(w.contains(&word_hash("d")));
/// ```
pub fn text_words(text: &str) -> Vec<u64> {
    let mut words: Vec<u64> = text
        .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
        .filter(|w| !w.is_empty())
        .map(word_hash)
        .collect();
    words.sort_unstable();
    words.dedup();
    words
}

/// Members the language uses without their name being written: a change to
/// one of them may reach code that never names it.
const IMPLICIT_MEMBERS: &[&str] = &[
    "call",
    "iterator",
    "moveNext",
    "current",
    "toString",
    "hashCode",
    "noSuchMethod",
    "runtimeType",
    "then",
];

/// [`ApiDigest`] of a unit already parsed from `text`; `None` when the text
/// does not lex.
pub fn api_digest(text: &str, ast: &Ast, unit: &CompilationUnit) -> Option<ApiDigest> {
    let tokens = dartforge_frontend::lexer::lex(text).ok()?;
    Some(api_digest_with(text, &tokens, ast, unit))
}

fn api_digest_with(text: &str, tokens: &[Token], ast: &Ast, unit: &CompilationUnit) -> ApiDigest {
    use std::hash::{Hash, Hasher};
    let mut excerpts: Vec<(Span, bool)> = regions(ast, false)
        .iter()
        .map(|r| (r.span, r.strict))
        .collect();
    excerpts.sort_by_key(|(s, _)| s.start);
    let directives = directive_spans(unit);
    let name_of = |n: &ast::Name| text.get(n.span.start..n.span.end).unwrap_or("").to_string();
    // The signature tokens of a span: their digest and their words.
    let signature = |span: Span| -> (u64, Vec<u64>) {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let mut words = Vec::new();
        let first = tokens.partition_point(|t| t.span.start < span.start);
        for t in &tokens[first..] {
            if t.span.start >= span.end {
                break;
            }
            if t.span.end > span.end || inside(&excerpts, t.span) {
                continue;
            }
            let lexeme = text.get(t.span.start..t.span.end).unwrap_or("");
            std::mem::discriminant(&t.kind).hash(&mut h);
            lexeme.hash(&mut h);
            (t.glued && matches!(t.kind, Kind::Op(Op::Gt))).hash(&mut h);
            if t.kind == Kind::Ident {
                words.push(word_hash(lexeme));
            }
        }
        words.sort_unstable();
        words.dedup();
        (h.finish(), words)
    };
    let mut d = std::collections::hash_map::DefaultHasher::new();
    for t in tokens {
        if inside(&directives, t.span) {
            text.get(t.span.start..t.span.end)
                .unwrap_or("")
                .hash(&mut d);
        }
    }
    let mut declarations = Vec::new();
    for decl in &ast.decls {
        let (members, names, class_like, unnamed): (&[ast::MemberId], Vec<String>, bool, bool) =
            match &decl.kind {
                DeclKind::Class(c) => (&c.members, vec![name_of(&c.name)], true, false),
                DeclKind::Mixin(m) => (&m.members, vec![name_of(&m.name)], true, false),
                DeclKind::Enum(e) => (&e.members, vec![name_of(&e.name)], true, false),
                DeclKind::ExtensionType(x) => (&x.members, vec![name_of(&x.name)], true, false),
                DeclKind::Extension(x) => (
                    &x.members,
                    vec![x.name.as_ref().map(name_of).unwrap_or_default()],
                    false,
                    x.name.is_none(),
                ),
                DeclKind::Typedef(t) => (&[], vec![name_of(&t.name)], false, false),
                DeclKind::Function(f) => (
                    &[],
                    vec![
                        ast.function(*f)
                            .name
                            .as_ref()
                            .map(name_of)
                            .unwrap_or_default(),
                    ],
                    false,
                    false,
                ),
                DeclKind::Variables(l) => (
                    &[],
                    l.variables.iter().map(|v| name_of(&v.name)).collect(),
                    false,
                    false,
                ),
            };
        let (digest, words) = signature(decl.span);
        let owner = names.first().cloned().unwrap_or_default();
        for name in names {
            declarations.push(ApiDeclaration {
                name,
                owner: None,
                implicit: unnamed,
                class_like,
                digest,
                words: words.clone(),
            });
        }
        // An enum's constants are members by name.
        if let DeclKind::Enum(e) = &decl.kind {
            for c in &e.constants {
                let (digest, words) = signature(c.span);
                declarations.push(ApiDeclaration {
                    name: name_of(&c.name),
                    owner: Some(owner.clone()),
                    implicit: false,
                    class_like: false,
                    digest,
                    words,
                });
            }
        }
        for &m in members {
            let member = ast.member(m);
            let (names, implicit): (Vec<String>, bool) = match &member.kind {
                MemberKind::Method(f) => {
                    let f = ast.function(*f);
                    let name = f.name.as_ref().map(name_of).unwrap_or_default();
                    let implicit = f.kind == ast::FunctionKind::Operator
                        || IMPLICIT_MEMBERS.contains(&name.as_str());
                    (vec![name], implicit)
                }
                MemberKind::Field(l) => {
                    let names: Vec<String> = l.variables.iter().map(|v| name_of(&v.name)).collect();
                    let implicit = names.iter().any(|n| IMPLICIT_MEMBERS.contains(&n.as_str()));
                    (names, implicit)
                }
                MemberKind::Constructor(k) => (
                    vec![k.name.as_ref().map(name_of).unwrap_or_default()],
                    false,
                ),
            };
            let (digest, words) = signature(member.span);
            for name in names {
                declarations.push(ApiDeclaration {
                    name,
                    owner: Some(owner.clone()),
                    implicit: implicit || unnamed,
                    class_like: false,
                    digest,
                    words: words.clone(),
                });
            }
        }
    }
    ApiDigest {
        directives: d.finish(),
        declarations,
    }
}
