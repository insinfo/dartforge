//! Field promotion (Dart 3.2, `inference-update-2`): `_fe_analyzer_shared`'s
//! `FieldPromotability` (`src/field_promotability.dart`) together with the
//! analyzer's `_FieldPromotability`
//! (`src/summary2/library_builder.dart:1466-1581`).
//!
//! Per library: each class, enum and mixin registers its non-synthetic
//! instance fields and getters with a private name. A non-final or
//! `external` field, or a concrete getter, makes the name non-promotable; so
//! does a concrete class whose interface has the name without the
//! implementation having it (a `noSuchMethod` forwarder). The result is the
//! library's `fieldNameNonPromotabilityInfo` (which why-not-promoted shows)
//! and the promotable fields (`isPromotable`).

use dartforge_elements::model::{ClassId, ClassKind, FunctionElementId, FunctionKind, LibraryId, Program, VariableId, VariableRef};
use dartforge_frontend::ast;
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};

/// `FieldNameNonPromotabilityInfo`: the declarations that prevent a name from
/// being promoted, in the order the analyzer finds them.
#[derive(Debug, Default, Clone)]
pub struct NonPromotionInfo {
    /// Non-final or `external` fields (`conflictingFields`).
    pub fields: Vec<VariableId>,
    /// Getters concretos (`conflictingGetters`).
    pub getters: Vec<FunctionElementId>,
    /// Concrete classes with a `noSuchMethod` forwarder
    /// (`conflictingNsmClasses`).
    pub nsm_classes: Vec<ClassId>,
}

/// The result for a library.
#[derive(Debug, Default, Clone)]
pub struct LibraryPromotion {
    /// `fieldNameNonPromotabilityInfo`, by name.
    pub info: HashMap<SymbolId, NonPromotionInfo>,
    /// Fields with `isPromotable` (including private representation fields of
    /// extension types).
    pub campos: HashSet<VariableId>,
    /// Private abstract getters whose synthetic field is promotable (the
    /// analyzer's `accessor.variable2`).
    pub getters: HashSet<FunctionElementId>,
}

/// `PropertyNonPromotabilityReason`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhyNotPromoted {
    NotAField,
    NotPrivate,
    External,
    NonFinal,
}

/// Whether the feature applies in a library (language version 3.2 or later).
pub fn enabled(program: &Program, lib: LibraryId) -> bool {
    program.library(lib).features.versao() >= dartforge_frontend::LanguageVersion::new(3, 2)
}

/// The classes, enums and mixins of a unit in the analyzer's order: the
/// classes (and class aliases), then the enums, then the mixins, each group
/// in declaration order.
fn unit_declarations(program: &Program, u: dartforge_elements::model::UnitId) -> Vec<(ClassId, bool)> {
    let mut classes = Vec::new();
    let mut enums = Vec::new();
    let mut mixins = Vec::new();
    for (i, c) in program.classes.iter().enumerate() {
        let Some(d) = c.decl else { continue };
        if d.unit != u {
            continue;
        }
        let id = ClassId(i as u32);
        match c.kind {
            ClassKind::Class | ClassKind::MixinApplication => {
                // `sealed` is also abstract in the analyzer's element.
                let abstract_ = c.modifiers.abstract_ || c.modifiers.sealed;
                classes.push((d.decl.0, id, abstract_));
            }
            ClassKind::Enum => enums.push((d.decl.0, id, false)),
            ClassKind::Mixin => mixins.push((d.decl.0, id, true)),
            ClassKind::ExtensionType => {}
        }
    }
    classes.sort_by_key(|x| x.0);
    enums.sort_by_key(|x| x.0);
    mixins.sort_by_key(|x| x.0);
    classes.into_iter().chain(enums).chain(mixins).map(|(_, id, a)| (id, a)).collect()
}

/// `getSuperclasses`: the supertype and the mixins; with `implements` (and
/// the mixins' `on`) when not ignoring them.
fn superclasses(program: &Program, c: ClassId, ignore_implements: bool) -> Vec<ClassId> {
    let ce = program.class(c);
    let mut r: Vec<ClassId> = ce.supertype_class.into_iter().collect();
    r.extend(ce.mixin_classes.iter().copied());
    if !ignore_implements {
        r.extend(ce.interface_classes.iter().copied());
        if ce.kind == ClassKind::Mixin {
            r.extend(ce.on_classes.iter().copied());
        }
    }
    r
}

/// The `_directNames` names of each node reachable from `c` (the
/// `_ClassHierarchyWalker`: the transitive union).
fn transitive_names(program: &Program, c: ClassId, direct: &HashMap<ClassId, HashSet<SymbolId>>, ignore_implements: bool) -> HashSet<SymbolId> {
    let mut seen: HashSet<ClassId> = HashSet::new();
    let mut stack = vec![c];
    let mut names = HashSet::new();
    while let Some(x) = stack.pop() {
        if !seen.insert(x) {
            continue;
        }
        if let Some(d) = direct.get(&x) {
            names.extend(d.iter().copied());
        }
        stack.extend(superclasses(program, x, ignore_implements));
    }
    names
}

/// The written `abstract` of a field.
fn abstract_field(program: &Program, v: VariableId) -> bool {
    match program.variable(v).node {
        VariableRef::Field { unit, member, .. } => match &program.unit(unit).ast.member(member).kind {
            ast::MemberKind::Field(l) => l.abstract_,
            _ => false,
        },
        _ => false,
    }
}

/// `_FieldPromotability.perform` for a library.
pub fn compute(program: &Program, interner: &Interner, lib: LibraryId) -> LibraryPromotion {
    let the_enabled = enabled(program, lib);
    let mut info: HashMap<SymbolId, NonPromotionInfo> = HashMap::new();
    let mut interface: HashMap<ClassId, HashSet<SymbolId>> = HashMap::new();
    let mut implemented: HashMap<ClassId, HashSet<SymbolId>> = HashMap::new();
    let mut concrete: Vec<ClassId> = Vec::new();
    let mut possible_fields: Vec<VariableId> = Vec::new();
    let mut possible_getters: Vec<FunctionElementId> = Vec::new();
    let mut r = LibraryPromotion::default();
    let private = |s: SymbolId| interner.resolve(s).starts_with('_');
    for &u in &program.library(lib).units {
        for (c, abstract_) in unit_declarations(program, u) {
            if !abstract_ {
                concrete.push(c);
            }
            interface.entry(c).or_default();
            implemented.entry(c).or_default();
            let ce = program.class(c);
            // The non-synthetic instance fields, in declaration order.
            for &v in &ce.fields {
                let ve = program.variable(v);
                if ve.static_ {
                    continue;
                }
                if !private(ve.name) {
                    continue;
                }
                let the_abstract_ = abstract_field(program, v);
                interface.get_mut(&c).unwrap().insert(ve.name);
                if !the_abstract_ {
                    implemented.get_mut(&c).unwrap().insert(ve.name);
                }
                if ve.external || !ve.final_ {
                    info.entry(ve.name).or_default().fields.push(v);
                } else if the_enabled {
                    possible_fields.push(v);
                }
            }
            // The non-synthetic instance getters, in declaration order.
            let mut getters: Vec<FunctionElementId> = ce
                .instance_members
                .values()
                .copied()
                .filter(|&f| {
                    let fe = program.function(f);
                    fe.kind == FunctionKind::Getter && !fe.static_ && fe.class == Some(c)
                })
                .collect();
            getters.sort();
            for f in getters {
                let fe = program.function(f);
                if !private(fe.name) {
                    continue;
                }
                interface.get_mut(&c).unwrap().insert(fe.name);
                if !fe.abstract_ {
                    implemented.get_mut(&c).unwrap().insert(fe.name);
                    info.entry(fe.name).or_default().getters.push(f);
                } else if the_enabled {
                    possible_getters.push(f);
                }
            }
        }
        // Private extension type representation: always promotable.
        for c in program.classes.iter() {
            if c.kind != ClassKind::ExtensionType || c.decl.is_none_or(|d| d.unit != u) {
                continue;
            }
            if let Some(rep) = c.representation {
                if private(program.variable(rep).name) {
                    r.campos.insert(rep);
                }
            }
        }
    }
    // `computeNonPromotabilityInfo`: the concrete classes whose interface has
    // a name that the implementation does not have.
    for &c in &concrete {
        let interface_names = transitive_names(program, c, &interface, false);
        let implemented_names = transitive_names(program, c, &implemented, true);
        let mut missing: Vec<SymbolId> = interface_names.into_iter().filter(|n| !implemented_names.contains(n)).collect();
        missing.sort();
        for n in missing {
            info.entry(n).or_default().nsm_classes.push(c);
        }
    }
    for v in possible_fields {
        if !info.contains_key(&program.variable(v).name) {
            r.campos.insert(v);
        }
    }
    for f in possible_getters {
        if !info.contains_key(&program.function(f).name) {
            r.getters.insert(f);
        }
    }
    r.info = info;
    r
}
