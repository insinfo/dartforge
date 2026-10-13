//! Promoção de campos (Dart 3.2, `inference-update-2`): o
//! `FieldPromotability` de `_fe_analyzer_shared`
//! (`src/field_promotability.dart`) com o `_FieldPromotability` do analyzer
//! (`src/summary2/library_builder.dart:1466-1581`).
//!
//! Por biblioteca: cada classe, enum e mixin registra os campos e getters de
//! instância não sintéticos de nome privado. Um campo não final ou
//! `external`, ou um getter concreto, torna o nome não promovível; uma classe
//! concreta cuja interface tem o nome sem que a implementação o tenha (um
//! encaminhador de `noSuchMethod`) também. O resultado é o
//! `fieldNameNonPromotabilityInfo` da biblioteca (que o why-not-promoted
//! mostra) e os campos promovíveis (`isPromotable`).

use dartforge_elements::model::{ClassId, ClassKind, FunctionElementId, FunctionKind, LibraryId, Program, VariableId, VariableRef};
use dartforge_frontend::ast;
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};

/// `FieldNameNonPromotabilityInfo`: as declarações que impedem a promoção de
/// um nome, na ordem em que o analyzer as encontra.
#[derive(Debug, Default, Clone)]
pub struct NonPromotionInfo {
    /// Campos não finais ou `external` (`conflictingFields`).
    pub campos: Vec<VariableId>,
    /// Getters concretos (`conflictingGetters`).
    pub getters: Vec<FunctionElementId>,
    /// Classes concretas com encaminhador de `noSuchMethod`
    /// (`conflictingNsmClasses`).
    pub nsm_classes: Vec<ClassId>,
}

/// O resultado de uma biblioteca.
#[derive(Debug, Default, Clone)]
pub struct LibraryPromotion {
    /// `fieldNameNonPromotabilityInfo`, pelo nome.
    pub info: HashMap<SymbolId, NonPromotionInfo>,
    /// Campos com `isPromotable` (os de representação privados de tipos de
    /// extensão inclusive).
    pub campos: HashSet<VariableId>,
    /// Getters abstratos privados cujo campo sintético é promovível (o
    /// `accessor.variable2` do analyzer).
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

/// Se o recurso vale numa biblioteca (versão de linguagem 3.2 ou mais).
pub fn enabled(program: &Program, lib: LibraryId) -> bool {
    program.library(lib).features.versao() >= dartforge_frontend::LanguageVersion::new(3, 2)
}

/// As classes, enums e mixins de uma unidade na ordem do analyzer: as
/// classes (e aliases de classe), depois os enums, depois os mixins, cada
/// grupo na ordem de declaração.
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
                // `sealed` também é abstrata no elemento do analyzer.
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

/// `getSuperclasses`: o supertipo e os mixins; com `implements` (e o `on`
/// dos mixins) quando não os ignora.
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

/// Os nomes de `_directNames` de cada nó alcançável a partir de `c` (o
/// `_ClassHierarchyWalker`: a união transitiva).
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

/// O `abstract` escrito de um campo.
fn abstract_field(program: &Program, v: VariableId) -> bool {
    match program.variable(v).node {
        VariableRef::Field { unit, member, .. } => match &program.unit(unit).ast.member(member).kind {
            ast::MemberKind::Field(l) => l.abstract_,
            _ => false,
        },
        _ => false,
    }
}

/// `_FieldPromotability.perform` de uma biblioteca.
pub fn calcular(program: &Program, interner: &Interner, lib: LibraryId) -> LibraryPromotion {
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
            // Os campos de instância não sintéticos, na ordem de declaração.
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
                    info.entry(ve.name).or_default().campos.push(v);
                } else if the_enabled {
                    possible_fields.push(v);
                }
            }
            // Os getters de instância não sintéticos, na ordem de declaração.
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
        // Representação privada de tipo de extensão: sempre promovível.
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
    // `computeNonPromotabilityInfo`: as classes concretas cuja interface tem
    // um nome que a implementação não tem.
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
