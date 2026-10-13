//! Merging of `augment` declarations into the outline
//! (docs/AUGMENTATIONS.md).
//!
//! Normative source: `working/augmentations/feature-specification.md` v1.46,
//! with two divergences from the 3.6.2 CFE (D9): an `external` declaration
//! can be completed by an augmentation (`external C.fromJson(...)` followed
//! by `augment C.fromJson(...) : ...`); under the `macros` experiment, an
//! augmentation can replace the body of a function that already had one.
//! The 3.6.2 CFE does this both for macro output and for a manual
//! `import augment`.
//!
//! It is the generalization of the SDK's patch mechanism (the CFE reused the
//! same infrastructure): an `augment` declaration does not create a new
//! name; it attaches to the declaration of the same name that comes
//! **before** it in application order (the preorder of the parts tree,
//! which the loader leaves in [`Library::units`]).
//!
//! Invariant of a member's chain, the same as for patches: the entry in the
//! member map (and in the library's `declared`) always points to the
//! **effective** element — the last complete declaration of the chain, or
//! the introductory one if none is complete —, and every other element of
//! the chain has `patched_by` pointing to it. Emitters walk only the
//! elements with `patched_by == None`.
//!
//! What emitters see of an augmented class: the members of the chain's
//! declarations ([`Program::class_members`]), one element per member,
//! and the added supertypes (`implements`/`with`/`extends`).
use crate::model::*;
use crate::outline::{extract_members, ElementPools};
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_frontend::ast::{self, DeclKind, FunctionBody, MemberKind};
use dartforge_frontend::features::Feature;
use dartforge_intern::{Interner, SymbolId};
use std::collections::{BTreeMap, HashMap};

/// What the merge needs to read and write, besides the elements.
pub(crate) struct Merge<'a> {
    pub units: &'a [Unit],
    pub library: &'a mut Library,
    pub lib_id: LibraryId,
    pub empty_sym: SymbolId,
    pub interner: &'a mut Interner,
    pub diagnostics: &'a mut Vec<Diagnostic>,
    /// Augmentation declarations of each class, in application order.
    pub chains: &'a mut Vec<(ClassId, DeclRef)>,
}

impl Merge<'_> {
    fn error(&mut self, unit: UnitId, msg: impl Into<String>, span: Span) {
        let u = &self.units[unit.0 as usize];
        self.diagnostics.push(Diagnostic::new(format!("{}: {}", u.uri, msg.into()), span));
    }
}

/// Applies the top-level `augment` declaration `decl_id` of unit `unit_id`.
pub(crate) fn apply(f: &mut Merge<'_>, pools: &mut ElementPools, unit_id: UnitId, decl_id: ast::DeclId) {
    let ast = &f.units[unit_id.0 as usize].ast;
    let decl = ast.decl(decl_id);
    let span = decl.span;
    match &decl.kind {
        DeclKind::Class(c) => {
            if c.mixin_application {
                f.error(unit_id, "a mixin application ('class C = S with M') cannot be augmented", span);
                return;
            }
            let Some(cid) = introductory_class(f, pools, unit_id, c.name, ClassKind::Class) else { return };
            let class_elem = &mut pools.classes[cid.0 as usize];
            if let Some(t) = c.extends {
                if class_elem.supertype.is_some() {
                    f.error(unit_id, format!("class '{}' already has an 'extends' clause", name_(f, c.name.sym)), span);
                } else {
                    class_elem.supertype = Some((unit_id, t));
                }
            }
            class_elem.mixins.extend(c.with.iter().map(|t| (unit_id, *t)));
            class_elem.interfaces.extend(c.implements.iter().map(|t| (unit_id, *t)));
            f.chains.push((cid, DeclRef { unit: unit_id, decl: decl_id }));
            apply_members(f, pools, cid, unit_id, &c.members);
        }
        DeclKind::Mixin(m) => {
            let Some(cid) = introductory_class(f, pools, unit_id, m.name, ClassKind::Mixin) else { return };
            let class_elem = &mut pools.classes[cid.0 as usize];
            class_elem.on.extend(m.on.iter().map(|t| (unit_id, *t)));
            class_elem.interfaces.extend(m.implements.iter().map(|t| (unit_id, *t)));
            f.chains.push((cid, DeclRef { unit: unit_id, decl: decl_id }));
            apply_members(f, pools, cid, unit_id, &m.members);
        }
        DeclKind::Function(fid) => {
            let func = ast.function(*fid);
            let Some(n) = func.name else { return };
            let kind = function_type(func.kind);
            let entry_path = f.library.declared.get(&n.sym).copied().unwrap_or_default();
            let previous = match kind {
                FunctionKind::Setter => entry_path.setter,
                _ => entry_path.getter,
            };
            let Some(Element::Function(old)) = previous else {
                f.error(
                    unit_id,
                    format!("'augment' of '{}' without an introductory declaration before it", name_(f, n.sym)),
                    n.span,
                );
                return;
            };
            let new_ = FunctionElementId(pools.functions.len() as u32);
            pools.functions.push(FunctionElement {
                name: n.sym,
                library: f.lib_id,
                class: None,
                extension: None,
                kind,
                static_: func.static_,
                abstract_: false,
                external: func.external,
                const_: false,
                factory: false,
                node: FunctionRef::Function { unit: unit_id, function: *fid },
                variable: None,
                patched_by: None,
                public_declaration: None,
            });
            if link(f, pools, unit_id, old, new_, full_function(func), n.span) {
                let e = f.library.declared.entry(n.sym).or_default();
                match kind {
                    FunctionKind::Setter => e.setter = Some(Element::Function(new_)),
                    _ => e.getter = Some(Element::Function(new_)),
                }
            }
        }
        DeclKind::Enum(e) => {
            if e.primary_constructor.is_some() {
                f.error(unit_id, "augmenting an enum with a primary constructor is not supported yet", span);
                return;
            }
            let Some(cid) = introductory_class(f, pools, unit_id, e.name, ClassKind::Enum) else { return };
            let class_elem = &mut pools.classes[cid.0 as usize];
            class_elem.mixins.extend(e.with.iter().map(|t| (unit_id, *t)));
            class_elem.interfaces.extend(e.implements.iter().map(|t| (unit_id, *t)));
            // New values go after the existing ones, in application order (spec,
            // "Augmenting enum declarations": `values` in chain order).
            for (idx, k) in e.constants.iter().enumerate() {
                let c = &pools.classes[cid.0 as usize];
                let repeated = c.enum_constants.iter().any(|v| pools.variables[v.0 as usize].name == k.name.sym)
                    || c.instance_members.contains_key(&k.name.sym)
                    || c.static_members.contains_key(&k.name.sym);
                if repeated {
                    let n = name_(f, k.name.sym);
                    f.error(unit_id, format!("'{n}' is already declared; to augment it, use 'augment'"), k.name.span);
                    continue;
                }
                let id = VariableId(pools.variables.len() as u32);
                pools.variables.push(VariableElement {
                    name: k.name.sym,
                    library: f.lib_id,
                    class: Some(cid),
                    extension: None,
                    static_: true,
                    final_: true,
                    const_: true,
                    late: false,
                    external: false,
                    node: VariableRef::EnumConstant { unit: unit_id, decl: decl_id, index: idx },
                    getter: None,
                    setter: None,
                });
                pools.classes[cid.0 as usize].enum_constants.push(id);
            }
            f.chains.push((cid, DeclRef { unit: unit_id, decl: decl_id }));
            apply_members(f, pools, cid, unit_id, &e.members);
        }
        DeclKind::Extension(_) | DeclKind::ExtensionType(_) => {
            f.error(unit_id, "augmenting extensions and extension types is not supported yet", span);
        }
        DeclKind::Variables(_) => {
            f.error(unit_id, "augmenting top-level variables is not supported yet", span);
        }
        DeclKind::Typedef(_) => {
            f.error(unit_id, "a typedef cannot be augmented", span);
        }
    }
}

/// The introductory class (or mixin) of the same name, already declared.
fn introductory_class(
    f: &mut Merge<'_>,
    pools: &ElementPools,
    unit_id: UnitId,
    n: ast::Name,
    kind: ClassKind,
) -> Option<ClassId> {
    let found_one = f.library.declared.get(&n.sym).and_then(|b| b.getter);
    match found_one {
        Some(Element::Class(cid)) if pools.classes[cid.0 as usize].kind == kind => Some(cid),
        Some(_) => {
            f.error(unit_id, format!("'augment' of '{}' with a different kind of declaration", name_(f, n.sym)), n.span);
            None
        }
        None => {
            f.error(unit_id, format!("'augment' of '{}' without an introductory declaration before it", name_(f, n.sym)), n.span);
            None
        }
    }
}

/// The members of the body of an `augment class`/`augment mixin`: plain ones
/// are added; `augment` ones attach to the member of the same name.
fn apply_members(f: &mut Merge<'_>, pools: &mut ElementPools, cid: ClassId, unit_id: UnitId, members_: &[ast::MemberId]) {
    let ast = &f.units[unit_id.0 as usize].ast;
    // In text order: a new member may be augmented right after it, in the same
    // body (that is what macro output does: `external C.x(...)` and
    // `augment C.x(...)` in the same `augment class`).
    for &mid in members_ {
        let m = ast.member(mid);
        if !m.augment {
            if let Some((sym, span)) = member_name(ast, m) {
                let c = &pools.classes[cid.0 as usize];
                let exists_ = match &m.kind {
                    MemberKind::Constructor(k) => {
                        let s = k.name.map(|n| n.sym).unwrap_or(f.empty_sym);
                        c.constructors
                            .get(&s)
                            .is_some_and(|id| pools.functions[id.0 as usize].kind != FunctionKind::SyntheticConstructor)
                    }
                    _ => c.instance_members.contains_key(&sym) || c.static_members.contains_key(&sym),
                };
                if exists_ {
                    let n = name_(f, sym);
                    f.error(unit_id, format!("'{n}' is already declared; to augment it, use 'augment'"), span);
                    continue;
                }
            }
            append_member(f, pools, cid, unit_id, mid);
            continue;
        }
        match &m.kind {
            MemberKind::Method(fid) => {
                let func = ast.function(*fid);
                let Some(n) = func.name else { continue };
                let kind = function_type(func.kind);
                let mut sym = n.sym;
                if kind == FunctionKind::Operator
                    && f.interner.resolve(sym) == "-"
                    && func.parameters.as_ref().is_some_and(|p| p.is_empty())
                {
                    sym = f.interner.intern("unary-");
                }
                let key = if kind == FunctionKind::Setter {
                    let s = format!("{}_=", f.interner.resolve(sym));
                    f.interner.intern(&s)
                } else {
                    sym
                };
                let c = &pools.classes[cid.0 as usize];
                let map = if func.static_ { &c.static_members } else { &c.instance_members };
                let Some(&old) = map.get(&key) else {
                    let t = name_(f, n.sym);
                    f.error(unit_id, format!("'augment' of '{t}' without an introductory member before it"), n.span);
                    continue;
                };
                let new_ = FunctionElementId(pools.functions.len() as u32);
                pools.functions.push(FunctionElement {
                    name: sym,
                    library: f.lib_id,
                    class: Some(cid),
                    extension: None,
                    kind,
                    static_: func.static_,
                    abstract_: matches!(func.body, FunctionBody::Empty) && !func.external,
                    external: func.external,
                    const_: false,
                    factory: false,
                    node: FunctionRef::Function { unit: unit_id, function: *fid },
                    variable: None,
                    patched_by: None,
                    public_declaration: None,
                });
                if link(f, pools, unit_id, old, new_, full_function(func), n.span) {
                    let c = &mut pools.classes[cid.0 as usize];
                    let map = if func.static_ { &mut c.static_members } else { &mut c.instance_members };
                    map.insert(key, new_);
                }
            }
            MemberKind::Constructor(k) => {
                let sym = k.name.map(|n| n.sym).unwrap_or(f.empty_sym);
                let span = k.name.map(|n| n.span).unwrap_or(k.class_name.span);
                let Some(&old) = pools.classes[cid.0 as usize].constructors.get(&sym) else {
                    f.error(unit_id, "constructor 'augment' without an introductory constructor before it", span);
                    continue;
                };
                let new_ = FunctionElementId(pools.functions.len() as u32);
                pools.functions.push(FunctionElement {
                    name: sym,
                    library: f.lib_id,
                    class: Some(cid),
                    extension: None,
                    kind: FunctionKind::Constructor,
                    static_: true,
                    abstract_: false,
                    external: k.external,
                    const_: k.const_,
                    factory: k.factory,
                    node: FunctionRef::Constructor { unit: unit_id, member: mid },
                    variable: None,
                    patched_by: None,
                    public_declaration: None,
                });
                if link(f, pools, unit_id, old, new_, full_constructor(k), span) {
                    pools.classes[cid.0 as usize].constructors.insert(sym, new_);
                }
            }
            MemberKind::Field(_) => {
                f.error(unit_id, "augmenting fields is not supported yet", m.span);
            }
        }
    }
}

/// A new member: the same elements as a plain declaration. A constructor
/// declared in the augmentation removes the synthetic constructor (the class
/// now declares a constructor).
fn append_member(f: &mut Merge<'_>, pools: &mut ElementPools, cid: ClassId, unit_id: UnitId, mid: ast::MemberId) {
    let ast = &f.units[unit_id.0 as usize].ast;
    let mut class_elem = std::mem::replace(&mut pools.classes[cid.0 as usize], empty_class(f.lib_id, f.empty_sym));
    if matches!(ast.member(mid).kind, MemberKind::Constructor(_)) {
        class_elem
            .constructors
            .retain(|_, id| pools.functions[id.0 as usize].kind != FunctionKind::SyntheticConstructor);
    }
    extract_members(pools, ast, &mut class_elem, cid, unit_id, &[mid], f.empty_sym, f.interner);
    pools.classes[cid.0 as usize] = class_elem;
}

/// Attaches `new_` to the chain whose effective element is `old`. Returns
/// `true` if `new_` became the effective one (the caller swaps the map
/// entry).
fn link(
    f: &mut Merge<'_>,
    pools: &mut ElementPools,
    unit_id: UnitId,
    old: FunctionElementId,
    new_: FunctionElementId,
    new_full: bool,
    span: Span,
) -> bool {
    if !new_full {
        // Only metadata (or nothing): the body stays the effective one's.
        pools.functions[new_.0 as usize].patched_by = Some(old);
        return false;
    }
    if has_body(f.units, &pools.functions[old.0 as usize]) && !augmentation_with_macros(f.units, unit_id) {
        f.error(unit_id, "the augmented declaration already has a body; an augmentation cannot replace it", span);
        pools.functions[new_.0 as usize].patched_by = Some(old);
        return false;
    }
    for e in pools.functions.iter_mut() {
        if e.patched_by == Some(old) {
            e.patched_by = Some(new_);
        }
    }
    pools.functions[old.0 as usize].patched_by = Some(new_);
    true
}

/// The 3.6.2 CFE accepts the replacement under `macros`, including for a
/// hand-written augmentation; outside the experiment it follows the spec.
fn augmentation_with_macros(units: &[Unit], unit: UnitId) -> bool {
    let u = &units[unit.0 as usize];
    u.role == UnitRole::Augmentation && u.features.has(Feature::Macros)
}

/// Does the declaration have a body in Dart (not `external`, not `;`)?
fn has_body(units: &[Unit], e: &FunctionElement) -> bool {
    match e.node {
        FunctionRef::Function { unit, function } => {
            let func = units[unit.0 as usize].ast.function(function);
            !func.external && !matches!(func.body, FunctionBody::Empty)
        }
        FunctionRef::Constructor { unit, member } => {
            let MemberKind::Constructor(k) = &units[unit.0 as usize].ast.member(member).kind else { return false };
            !k.external && full_constructor(k)
        }
        // Implicit accessor of a non-abstract field: it has the synthetic body.
        FunctionRef::None => !e.abstract_ && !e.external,
    }
}

/// The spec's "Complete" (`:547-560`), with `external` counting as
/// complete.
fn full_function(func: &ast::Function) -> bool {
    func.external || !matches!(func.body, FunctionBody::Empty)
}

fn full_constructor(k: &ast::Constructor) -> bool {
    k.external
        || !matches!(k.body, FunctionBody::Empty)
        || !k.initializers.is_empty()
        || k.redirect.is_some()
        || k.parameters.iter().any(|p| p.this_ || p.super_)
}

fn function_type(k: ast::FunctionKind) -> FunctionKind {
    match k {
        ast::FunctionKind::Getter => FunctionKind::Getter,
        ast::FunctionKind::Setter => FunctionKind::Setter,
        ast::FunctionKind::Operator => FunctionKind::Operator,
        _ => FunctionKind::Function,
    }
}

fn member_name(ast: &ast::Ast, m: &ast::Member) -> Option<(SymbolId, Span)> {
    match &m.kind {
        MemberKind::Method(fid) => ast.function(*fid).name.map(|n| (n.sym, n.span)),
        MemberKind::Constructor(k) => Some(k.name.map(|n| (n.sym, n.span)).unwrap_or((k.class_name.sym, k.class_name.span))),
        MemberKind::Field(v) => v.variables.first().map(|x| (x.name.sym, x.name.span)),
    }
}

fn name_(f: &Merge<'_>, s: SymbolId) -> String {
    f.interner.resolve(s).to_string()
}

/// Marker that takes the place of a class while the new members are
/// extracted into it (extraction works on a loose `ClassElement`).
fn empty_class(library: LibraryId, name: SymbolId) -> ClassElement {
    ClassElement {
        name,
        library,
        decl: None,
        kind: ClassKind::Class,
        modifiers: ast::ClassModifiers::default(),
        type_params: Vec::new(),
        supertype: None,
        mixins: Vec::new(),
        interfaces: Vec::new(),
        on: Vec::new(),
        supertype_class: None,
        mixin_classes: Vec::new(),
        interface_classes: Vec::new(),
        on_classes: Vec::new(),
        instance_members: HashMap::new(),
        shadowed_instance_members: Vec::new(),
        static_members: HashMap::new(),
        constructors: BTreeMap::new(),
        fields: Vec::new(),
        enum_constants: Vec::new(),
        representation: None,
    }
}
