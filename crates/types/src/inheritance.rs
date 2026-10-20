//! The `InheritanceManager3` of analyzer 3.6.2
//! (`analyzer/lib/src/dart/element/inheritance_manager3.dart`) and the
//! `NNBD_TOP_MERGE` (`top_merge.dart`) it uses: the interface of each class,
//! mixin, enum and extension type, with the same maps as the original:
//!
//! * `map`: the signature of each name in the interface (the declared ones,
//!   and the inherited ones by the most specific signature, with
//!   `topMerge`);
//! * `declared`, `implemented` (the concrete implementations, with the
//!   `noSuchMethod` forwarders and inherited covariance);
//! * `overridden` (the candidates of each name in the direct supertypes, in
//!   order: superclass, mixins, interfaces), `redeclared` (extension types),
//!   `superImplemented` (`[S, S&M1, S&M1&M2]`) and the conflicts.
//!
//! Names are the analyzer's `Name`: a private one is qualified by the
//! library. The text key is the element model's (setter `x_=`).
//!
//! Each member's type (`ExecutableElement.type`) and subtyping come from the
//! [`Provider`]: inference provides the types of inferred fields on demand;
//! the passes after it use [`OutlineProvider`].
//! Written without compiling or running (2026-10-05).

use crate::ops;
use crate::resolve::{ClassTypeData, OutlineTypes};
use crate::subtyping::{SubtypeEnv, is_subtype};
use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeParamOwner, TypeTable, Variance};
use dartforge_elements::model::{
    ClassId, ClassKind, FunctionElementId, FunctionKind, FunctionRef, LibraryId, Program,
    VariableRef,
};
use dartforge_frontend::ast::{self, MemberKind};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};
// Fx hashing for the interface maps: they are rebuilt on every inference pass
// and their keys are small integers.
use rustc_hash::{FxHashMap, FxHashSet};
use std::rc::Rc;

/// `Name`: the member key and, if private, the library.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct MemberName {
    pub key: SymbolId,
    pub library: Option<LibraryId>,
}

impl MemberName {
    /// `Name(libraryUri, name)`.
    pub fn new(interner: &Interner, library: LibraryId, key: SymbolId) -> MemberName {
        let private = interner.resolve(key).starts_with('_');
        MemberName {
            key,
            library: private.then_some(library),
        }
    }

    /// `forSetter`.
    pub fn from_setter(self, interner: &Interner) -> Option<MemberName> {
        let text = interner.resolve(self.key);
        let key = interner.lookup(&format!("{text}_="))?;
        Some(MemberName {
            key,
            library: self.library,
        })
    }

    /// `forGetter`.
    pub fn of_getter(self, interner: &Interner) -> Option<MemberName> {
        let text = interner.resolve(self.key);
        match text.strip_suffix("_=") {
            Some(base) => Some(MemberName {
                key: interner.lookup(base)?,
                library: self.library,
            }),
            None => Some(self),
        }
    }
}

/// `ElementKind` of a member.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Method,
    Getter,
    Setter,
}

/// `_ParameterDesc`: a named parameter by its name, a positional one by its
/// position.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Param {
    Index(usize),
    MemberName(SymbolId),
}

/// A member of the interface (`ExecutableElement`, already with the
/// substitution of the supertype it came from).
#[derive(Clone, Debug)]
pub struct InterfaceMember {
    /// The declared element; for a synthetic one (`topMerge`, inherited
    /// covariance), the first of the candidates (the `prototype`).
    pub function: FunctionElementId,
    /// `enclosingElement3`: the declaring class; for a synthetic one, the class
    /// of the interface.
    pub class: ClassId,
    /// `type`, in the type parameters of the class whose interface contains it.
    pub ty: TypeId,
    pub kind: Kind,
    pub the_abstract_: bool,
    /// The covariant parameters (`isCovariant`: written or inherited).
    pub the_covariant: Rc<[Param]>,
    pub synthetic: bool,
}

/// `Conflict`.
#[derive(Clone, Debug)]
pub enum Conflict {
    /// `CandidatesConflict`.
    Candidates {
        name: MemberName,
        candidates: Vec<InterfaceMember>,
    },
    /// `GetterMethodConflict`.
    GetterMethod {
        name: MemberName,
        getter: InterfaceMember,
        method: InterfaceMember,
    },
    /// `HasNonExtensionAndExtensionMemberConflict`.
    ExtensionAndNonExtension {
        name: MemberName,
        non_extension: Vec<InterfaceMember>,
        extension: Vec<InterfaceMember>,
    },
    /// `NotUniqueExtensionMemberConflict`.
    NonUniqueExtension {
        name: MemberName,
        candidates: Vec<InterfaceMember>,
    },
}

/// A map in insertion order (Dart's `LinkedHashMap`: reassigning keeps the
/// position).
#[derive(Clone, Debug)]
pub struct Map<V> {
    entries: Vec<(MemberName, V)>,
    index: FxHashMap<MemberName, usize>,
}

impl<V> Default for Map<V> {
    fn default() -> Self {
        Map {
            entries: Vec::new(),
            index: FxHashMap::default(),
        }
    }
}

impl<V> Map<V> {
    pub fn get(&self, n: &MemberName) -> Option<&V> {
        self.index.get(n).map(|&i| &self.entries[i].1)
    }

    pub fn get_mut(&mut self, n: &MemberName) -> Option<&mut V> {
        match self.index.get(n) {
            Some(&i) => Some(&mut self.entries[i].1),
            None => None,
        }
    }

    pub fn contains_key(&self, n: &MemberName) -> bool {
        self.index.contains_key(n)
    }

    pub fn insert(&mut self, n: MemberName, v: V) {
        match self.index.get(&n) {
            Some(&i) => self.entries[i].1 = v,
            None => {
                self.index.insert(n, self.entries.len());
                self.entries.push((n, v));
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&MemberName, &V)> {
        self.entries.iter().map(|(n, v)| (n, v))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// `Interface`.
#[derive(Clone, Debug, Default)]
pub struct Interface {
    pub map: Map<InterfaceMember>,
    pub declared: Map<InterfaceMember>,
    pub implemented: Map<InterfaceMember>,
    /// `noSuchMethodForwarders`.
    pub forwarded: HashSet<MemberName>,
    pub overridden: Map<Vec<InterfaceMember>>,
    pub redeclared: Map<Vec<InterfaceMember>>,
    pub super_implemented: Vec<Map<InterfaceMember>>,
    pub conflicts: Vec<Conflict>,
}

impl Interface {
    /// `Interface._empty`.
    fn empty() -> Interface {
        Interface {
            super_implemented: vec![Map::default()],
            ..Interface::default()
        }
    }

    /// `isSuperImplemented`.
    pub fn super_implemented(&self, n: &MemberName) -> bool {
        self.super_implemented
            .last()
            .is_some_and(|m| m.contains_key(n))
    }
}

/// What inheritance needs from the rest of the analyzer.
pub trait Provider<'p> {
    fn program(&self) -> &'p Program;
    fn interner(&self) -> &'p Interner;
    fn core(&self) -> &'p CoreTypes;
    fn table(&mut self) -> &mut TypeTable;
    fn class_data(&self, c: ClassId) -> ClassTypeData;
    /// `ExecutableElement.type` of member `f`, in the type parameters of the
    /// class that declares it.
    fn member_type(&mut self, f: FunctionElementId) -> TypeId;
    fn sub(&mut self, a: TypeId, b: TypeId) -> bool;
    /// `TypeSystem.normalize`.
    fn normalize_(&mut self, t: TypeId) -> TypeId;
    /// The interface of `c` kept by an earlier pass over the same program
    /// ([`crate::SharedInference`]).
    fn shared_interface(&self, _c: ClassId) -> Option<Rc<Interface>> {
        None
    }
    /// A counter that changes whenever a member type was given provisionally
    /// (an interface built while it changed must not be shared).
    fn provisional_events(&self) -> u64 {
        0
    }
    /// Offers the interface of `c`, built without provisional member types,
    /// to the later passes.
    fn share_interface(&mut self, _c: ClassId, _i: &Rc<Interface>) {}
}

/// The provider for the passes that run after inference: the outline types
/// (including those of already inferred variables).
pub struct OutlineProvider<'p, 't> {
    pub program: &'p Program,
    pub interner: &'p Interner,
    pub core: &'p CoreTypes,
    pub outline: &'p OutlineTypes,
    pub table: &'t mut TypeTable,
}

/// The function type of a member (that of an implicit accessor, built from
/// the variable type `variable_type`).
pub fn member_function_type(
    program: &Program,
    outline: &OutlineTypes,
    core: &CoreTypes,
    table: &mut TypeTable,
    f: FunctionElementId,
    variable_type: TypeId,
) -> TypeId {
    let e = program.function(f);
    if e.kind == FunctionKind::ImplicitAccessor
        && let Some(v) = e.variable
    {
        let setter = program.variable(v).setter == Some(f);
        return if setter {
            table.intern(Type::Function {
                type_params: Box::new([]),
                ret: core.void_,
                positional: Box::new([variable_type]),
                optional: Box::new([]),
                named: Box::new([]),
                nullable: false,
            })
        } else {
            table.intern(Type::Function {
                type_params: Box::new([]),
                ret: variable_type,
                positional: Box::new([]),
                optional: Box::new([]),
                named: Box::new([]),
                nullable: false,
            })
        };
    }
    outline
        .functions
        .get(f.0 as usize)
        .map_or(core.dynamic_, |d| d.signature)
}

impl<'p> Provider<'p> for OutlineProvider<'p, '_> {
    fn program(&self) -> &'p Program {
        self.program
    }
    fn interner(&self) -> &'p Interner {
        self.interner
    }
    fn core(&self) -> &'p CoreTypes {
        self.core
    }
    fn table(&mut self) -> &mut TypeTable {
        &mut *self.table
    }
    fn class_data(&self, c: ClassId) -> ClassTypeData {
        self.outline.classes[c.0 as usize].clone()
    }
    fn member_type(&mut self, f: FunctionElementId) -> TypeId {
        let tv = match self.program.function(f).variable {
            Some(v) => self
                .outline
                .variables
                .get(v.0 as usize)
                .and_then(|d| d.declared_type.or(d.inferred))
                .unwrap_or(self.core.dynamic_),
            None => self.core.dynamic_,
        };
        member_function_type(
            self.program,
            self.outline,
            self.core,
            &mut *self.table,
            f,
            tv,
        )
    }
    fn sub(&mut self, a: TypeId, b: TypeId) -> bool {
        let mut env = SubtypeEnv::new(&mut *self.table, &self.outline.hierarchy, self.core);
        is_subtype(a, b, &mut env)
    }
    fn normalize_(&mut self, t: TypeId) -> TypeId {
        ops::normalize(t, &mut *self.table, self.core)
    }
}

/// `InheritanceManager3`.
#[derive(Default)]
pub struct InheritanceManager {
    interfaces: FxHashMap<ClassId, Rc<Interface>>,
    processing: FxHashSet<ClassId>,
    the_inherited: FxHashMap<ClassId, Rc<Map<InterfaceMember>>>,
    /// How many times a class asked for its own interface while building it
    /// (and got the empty placeholder).
    cycle_cuts: u64,
}

/// The kind of a declared member.
fn kind_of(program: &Program, f: FunctionElementId) -> Kind {
    let e = program.function(f);
    match e.kind {
        FunctionKind::Getter => Kind::Getter,
        FunctionKind::Setter => Kind::Setter,
        FunctionKind::ImplicitAccessor => match e.variable {
            Some(v) if program.variable(v).setter == Some(f) => Kind::Setter,
            _ => Kind::Getter,
        },
        _ => Kind::Method,
    }
}

/// The field list of a variable declaration (for `abstract` and
/// `covariant`).
fn field_list(
    program: &Program,
    v: dartforge_elements::model::VariableId,
) -> Option<&ast::VariableList> {
    match program.variable(v).node {
        VariableRef::Field { unit, member, .. } => {
            match &program.unit(unit).ast.member(member).kind {
                MemberKind::Field(l) => Some(l),
                _ => None,
            }
        }
        _ => None,
    }
}

/// `isAbstract`. The `int get index;` of `dart:core`'s `Enum` counts as
/// concrete (3.6.2 `element_builder.dart:968-974`: the specification says the
/// actual superclass is another class that implements `Enum`).
fn the_abstract_(
    program: &Program,
    interner: &dartforge_intern::Interner,
    f: FunctionElementId,
) -> bool {
    let e = program.function(f);
    if e.kind == FunctionKind::Getter
        && interner.resolve(e.name) == "index"
        && let Some(k) = e.class
        && interner.resolve(program.class(k).name) == "Enum"
        && program.library(program.class(k).library).uri == "dart:core"
    {
        return false;
    }
    if e.kind == FunctionKind::ImplicitAccessor {
        return e
            .variable
            .and_then(|v| field_list(program, v))
            .is_some_and(|l| l.abstract_);
    }
    e.abstract_
}

/// The parameters written `covariant` in a declared member (the implicit
/// setter of a `covariant` field).
fn written_covariance(program: &Program, f: FunctionElementId) -> Vec<Param> {
    let e = program.function(f);
    if e.kind == FunctionKind::ImplicitAccessor {
        let setter = e
            .variable
            .is_some_and(|v| program.variable(v).setter == Some(f));
        let covariant = e
            .variable
            .and_then(|v| field_list(program, v))
            .is_some_and(|l| l.covariant);
        return if setter && covariant {
            vec![Param::Index(0)]
        } else {
            Vec::new()
        };
    }
    let FunctionRef::Function { unit, function } = e.node else {
        return Vec::new();
    };
    let Some(ps) = &program.unit(unit).ast.function(function).parameters else {
        return Vec::new();
    };
    ps.iter()
        .enumerate()
        .filter(|(_, p)| p.covariant)
        .filter_map(|(i, p)| match p.kind {
            ast::ParameterKind::Named => p.public_name.or(p.name).map(|n| Param::MemberName(n.sym)),
            _ => Some(Param::Index(i)),
        })
        .collect()
}

/// The parameter descriptors of a function type, in order.
fn parameters_of_type(table: &TypeTable, t: TypeId) -> Vec<Param> {
    match table.get(t) {
        Type::Function {
            positional,
            optional,
            named,
            ..
        } => {
            let mut v: Vec<Param> = (0..positional.len() + optional.len())
                .map(Param::Index)
                .collect();
            v.extend(named.iter().map(|(n, _, _)| Param::MemberName(*n)));
            v
        }
        _ => Vec::new(),
    }
}

impl InheritanceManager {
    /// `getInterface`.
    pub fn interface<'p>(&mut self, p: &mut dyn Provider<'p>, c: ClassId) -> Rc<Interface> {
        // T1.1: same-named members of the same kind are a single key.
        let c = p.program().class_owner(c);
        if let Some(i) = self.interfaces.get(&c) {
            if self.processing.contains(&c) {
                self.cycle_cuts += 1;
            }
            return i.clone();
        }
        if let Some(i) = p.shared_interface(c) {
            self.interfaces.insert(c, i.clone());
            return i;
        }
        self.interfaces.insert(c, Rc::new(Interface::empty()));
        if !self.processing.insert(c) {
            return Rc::new(Interface::empty());
        }
        let events = p.provisional_events();
        let cuts = self.cycle_cuts;
        let r = match p.program().class(c).kind {
            ClassKind::ExtensionType => self.of_extension_type(p, c),
            ClassKind::Mixin => self.of_mixin(p, c),
            _ => self.of_class(p, c),
        };
        self.processing.remove(&c);
        let r = Rc::new(r);
        self.interfaces.insert(c, r.clone());
        // Shared only when no member type was provisional and no cycle cut an
        // interface short during the computation.
        if self.cycle_cuts == cuts && p.provisional_events() == events {
            p.share_interface(c, &r);
        }
        r
    }

    /// Drops the interface of `c` from the cache (the types of its members
    /// changed).
    pub fn forget(&mut self, program: &Program, c: ClassId) {
        let c = program.class_owner(c);
        self.interfaces.remove(&c);
        self.the_inherited.remove(&c);
    }

    /// `getMember2`.
    pub fn member<'p>(
        &mut self,
        p: &mut dyn Provider<'p>,
        c: ClassId,
        name: MemberName,
        concrete: bool,
        mixin_index: Option<usize>,
        to_super: bool,
    ) -> Option<InterfaceMember> {
        let i = self.interface(p, c);
        if to_super {
            if p.program().class(c).kind == ClassKind::ExtensionType {
                return None;
            }
            if let Some(k) = mixin_index {
                return i
                    .super_implemented
                    .get(k)
                    .and_then(|m| m.get(&name))
                    .cloned();
            }
            return i
                .super_implemented
                .last()
                .and_then(|m| m.get(&name))
                .cloned();
        }
        if concrete {
            return i.implemented.get(&name).cloned();
        }
        i.map.get(&name).cloned()
    }

    /// `getOverridden2`.
    pub fn overridden<'p>(
        &mut self,
        p: &mut dyn Provider<'p>,
        c: ClassId,
        name: MemberName,
    ) -> Option<Vec<InterfaceMember>> {
        self.interface(p, c).overridden.get(&name).cloned()
    }

    /// `getInheritedMap2`.
    pub fn inherited_map<'p>(
        &mut self,
        p: &mut dyn Provider<'p>,
        c: ClassId,
    ) -> Rc<Map<InterfaceMember>> {
        let c = p.program().class_owner(c);
        if let Some(m) = self.the_inherited.get(&c) {
            return m.clone();
        }
        let i = self.interface(p, c);
        let mut map = Map::default();
        let source = if p.program().class(c).kind == ClassKind::ExtensionType {
            &i.redeclared
        } else {
            &i.overridden
        };
        most_specific(p, c, &mut map, source, false);
        let map = Rc::new(map);
        self.the_inherited.insert(c, map.clone());
        map
    }

    /// `getInherited2`.
    pub fn inherited<'p>(
        &mut self,
        p: &mut dyn Provider<'p>,
        c: ClassId,
        name: MemberName,
    ) -> Option<InterfaceMember> {
        self.inherited_map(p, c).get(&name).cloned()
    }

    /// `getInheritedConcreteMap2`.
    pub fn inherited_concrete<'p>(
        &mut self,
        p: &mut dyn Provider<'p>,
        c: ClassId,
    ) -> Map<InterfaceMember> {
        if p.program().class(c).kind == ClassKind::ExtensionType {
            return Map::default();
        }
        self.interface(p, c)
            .super_implemented
            .last()
            .cloned()
            .unwrap_or_default()
    }

    /// The class and the substitution (`Substitution.fromInterfaceType`) of a
    /// supertype.
    fn substitution<'p>(
        p: &mut dyn Provider<'p>,
        t: TypeId,
    ) -> Option<(ClassId, HashMap<TypeParamId, TypeId>)> {
        let (c, args) = match p.table().get(t) {
            Type::Interface { class, args, .. } => (*class, args.clone()),
            Type::ExtensionType { decl, args, .. } => (*decl, args.clone()),
            _ => return None,
        };
        let formals = p.class_data(c).type_params;
        Some((
            c,
            formals.iter().copied().zip(args.iter().copied()).collect(),
        ))
    }

    fn substitute<'p>(
        p: &mut dyn Provider<'p>,
        m: &InterfaceMember,
        s: &HashMap<TypeParamId, TypeId>,
    ) -> InterfaceMember {
        if s.is_empty() {
            return m.clone();
        }
        InterfaceMember {
            ty: ops::substitute(m.ty, s, p.table()),
            ..m.clone()
        }
    }

    /// `_addCandidates`.
    fn add_candidates<'p>(
        p: &mut dyn Provider<'p>,
        candidates: &mut Map<Vec<InterfaceMember>>,
        s: &HashMap<TypeParamId, TypeId>,
        i: &Interface,
    ) {
        for (n, m) in i.map.iter() {
            let c = Self::substitute(p, m, s);
            match candidates.get_mut(n) {
                Some(l) => l.push(c),
                None => candidates.insert(*n, vec![c]),
            }
        }
    }

    /// The type of `Object`.
    fn object_type<'p>(p: &mut dyn Provider<'p>) -> Option<TypeId> {
        let o = p.core().object_class?;
        Some(p.table().intern(Type::Interface {
            class: o,
            args: Box::new([]),
            nullable: false,
        }))
    }

    /// `element.supertype`, with the defaults of `setDefaultSupertypes` and
    /// `buildEnumChildren`.
    fn the_supertype<'p>(
        p: &mut dyn Provider<'p>,
        c: ClassId,
        data: &ClassTypeData,
    ) -> Option<TypeId> {
        let prog = p.program();
        let k = prog.class(c);
        match k.kind {
            ClassKind::Class | ClassKind::MixinApplication => {
                if Some(c) == p.core().object_class {
                    return None;
                }
                // Only a class interface type counts as a superclass
                // (`_isInterfaceTypeClass`); anything else falls back to `Object`.
                let written = data.supertype.filter(|&t| match p.table().get(t) {
                    Type::Interface { class, .. } => matches!(
                        prog.class(*class).kind,
                        ClassKind::Class | ClassKind::MixinApplication
                    ),
                    _ => false,
                });
                written.or_else(|| Self::object_type(p))
            }
            ClassKind::Enum => {
                let core_lib = p.core().core_library;
                let enum_ = core_lib.and_then(|l| {
                    let s = p.interner().lookup("Enum")?;
                    match prog.lookup(l, s)?.getter? {
                        dartforge_elements::model::Element::Class(e) => Some(e),
                        _ => None,
                    }
                });
                match enum_ {
                    Some(e) => Some(p.table().intern(Type::Interface {
                        class: e,
                        args: Box::new([]),
                        nullable: false,
                    })),
                    None => Self::object_type(p),
                }
            }
            _ => None,
        }
    }

    /// The classes that `InterfaceElementImpl._implementationsOfGetter` (and the
    /// method and setter variants) walks: the class itself, its mixins from back
    /// to front, and then the superclass, up to `Object`
    /// (an362:src/dart/element/element.dart:5423-5470).
    pub fn implementation_chain<'p>(p: &mut dyn Provider<'p>, c: ClassId) -> Vec<ClassId> {
        let mut chain = Vec::new();
        let mut the_seen = std::collections::HashSet::new();
        let mut current = Some(c);
        while let Some(k) = current {
            if !the_seen.insert(k) {
                break;
            }
            chain.push(k);
            let data = p.class_data(k);
            for &mt in data.mixins.iter().rev() {
                if let Some((m, _)) = Self::substitution(p, mt) {
                    chain.push(m);
                }
            }
            current = Self::the_supertype(p, k, &data)
                .and_then(|st| Self::substitution(p, st))
                .map(|(s, _)| s);
        }
        chain
    }

    /// The concrete instance members of the declaration of `c` in the order of
    /// `_addImplemented` (methods, then accessors, each group in element order),
    /// when one of them was left out of the `instance_members` map (same-named);
    /// empty when there is no same-named member.
    fn concrete_same_named<'p>(p: &mut dyn Provider<'p>, c: ClassId) -> Vec<FunctionElementId> {
        let prog = p.program();
        let interner = p.interner();
        let k = prog.class(c);
        let ids = k
            .instance_members
            .values()
            .chain(k.static_members.values())
            .chain(k.constructors.values())
            .map(|f| f.0);
        let (Some(mut min), Some(mut max)) = (ids.clone().min(), ids.max()) else {
            return Vec::new();
        };
        // Overridden same-named members in the map may come before the
        // smallest remaining id (or after the largest): the range grows
        // while the neighboring elements belong to the class.
        let total = prog.functions.len() as u32;
        while min > 0 && prog.function(FunctionElementId(min - 1)).class == Some(c) {
            min -= 1;
        }
        while max + 1 < total && prog.function(FunctionElementId(max + 1)).class == Some(c) {
            max += 1;
        }
        let in_map: HashSet<FunctionElementId> = k.instance_members.values().copied().collect();
        let mut all: Vec<FunctionElementId> = (min..=max)
            .map(FunctionElementId)
            .filter(|&f| {
                let e = prog.function(f);
                e.class == Some(c)
                    && !e.static_
                    && (!matches!(e.node, FunctionRef::None)
                        || e.kind == FunctionKind::ImplicitAccessor)
                    && matches!(
                        e.kind,
                        FunctionKind::Function
                            | FunctionKind::Getter
                            | FunctionKind::Setter
                            | FunctionKind::Operator
                            | FunctionKind::ImplicitAccessor
                    )
            })
            .collect();
        if all.iter().all(|f| in_map.contains(f)) {
            return Vec::new();
        }
        all.retain(|&f| !the_abstract_(prog, interner, f));
        all.sort_by_key(|&f| member_order(prog, f));
        all
    }

    /// The declared instance members (`_getTypeMembers`): methods, then
    /// accessors, in source order, with the written covariance and the one
    /// inherited from `candidates` (`_inferParameterCovariance`).
    fn declared<'p>(
        p: &mut dyn Provider<'p>,
        c: ClassId,
        candidates: &Map<Vec<InterfaceMember>>,
    ) -> Map<InterfaceMember> {
        let prog = p.program();
        let interner = p.interner();
        let k = prog.class(c);
        // The synthetic `index` that the outline puts in an enum is not a member
        // of the analyzer's element: it comes from `Enum` (concrete, see
        // `the_abstract_`).
        let is_enum = k.kind == ClassKind::Enum;
        let mut members: Vec<(SymbolId, FunctionElementId)> = k
            .instance_members
            .iter()
            .filter(|&(&s, &f)| {
                !(is_enum
                    && matches!(prog.function(f).node, FunctionRef::None)
                    && interner.resolve(s) == "index")
            })
            .map(|(&s, &f)| (s, f))
            .collect();
        // Duplicate declaration: the last one in `_getTypeMembers` order wins
        // (methods; field accessors; written accessors), not the source order
        // (`int get foo` followed by the field `foo = 1`: the written getter).
        if !k.shadowed_instance_members.is_empty() {
            for &(key, old) in k.shadowed_instance_members.iter() {
                if let Some(current) = members.iter_mut().find(|(s, _)| *s == key)
                    && member_order(prog, old) > member_order(prog, current.1)
                {
                    current.1 = old;
                }
            }
        }
        members.sort_by_key(|&(_, f)| (kind_of(prog, f) != Kind::Method, f));
        let mut map = Map::default();
        for (key, f) in members {
            let name = MemberName::new(interner, k.library, key);
            let typ = p.member_type(f);
            let mut the_covariant = written_covariance(prog, f);
            if let Some(overridden) = candidates.get(&name) {
                for d in parameters_of_type(p.table(), typ) {
                    if the_covariant.contains(&d) {
                        continue;
                    }
                    let inherits = overridden.iter().any(|o| {
                        // `_getCorrespondingParameter`: a named one by name, a
                        // positional one by position (if it is positional there).
                        o.the_covariant.contains(&d)
                            && parameters_of_type(p.table(), o.ty).contains(&d)
                    });
                    if inherits {
                        the_covariant.push(d);
                    }
                }
            }
            map.insert(
                name,
                InterfaceMember {
                    function: f,
                    class: c,
                    ty: typ,
                    kind: kind_of(prog, f),
                    the_abstract_: the_abstract_(prog, interner, f),
                    the_covariant: the_covariant.into(),
                    synthetic: false,
                },
            );
        }
        map
    }

    /// `_getInterfaceClass`.
    fn of_class<'p>(&mut self, p: &mut dyn Provider<'p>, c: ClassId) -> Interface {
        let prog = p.program();
        let data = p.class_data(c);
        let mut candidates: Map<Vec<InterfaceMember>> = Map::default();
        let mut super_impl: Vec<Map<InterfaceMember>> = Vec::new();
        let mut implemented: Map<InterfaceMember> = Map::default();
        let mut super_interface: Option<Rc<Interface>> = None;
        if let Some(st) = Self::the_supertype(p, c, &data)
            && let Some((sc, s)) = Self::substitution(p, st)
        {
            let si = self.interface(p, sc);
            Self::add_candidates(p, &mut candidates, &s, &si);
            for (n, m) in si.implemented.iter() {
                let m = Self::substitute(p, m, &s);
                implemented.insert(*n, m);
            }
            super_impl.push(implemented.clone());
            super_interface = Some(si);
        }
        let mut mixin_conflicts: Vec<Vec<Conflict>> = Vec::new();
        for &mt in data.mixins.iter() {
            let Some((mc, s)) = Self::substitution(p, mt) else {
                continue;
            };
            let mi = self.interface(p, mc);
            let mut of_super_and_mixin: Map<Vec<InterfaceMember>> = Map::default();
            let mut mixin_conflicts_of: Vec<Conflict> = Vec::new();
            for (n, m0) in mi.map.iter() {
                let candidate = Self::substitute(p, m0, &s);
                let current = match candidates.get(n) {
                    None => {
                        candidates.insert(*n, vec![candidate]);
                        continue;
                    }
                    Some(l) => l[0].clone(),
                };
                if candidate.class == mc {
                    if current.kind != candidate.kind {
                        let (getter, method) = if current.kind == Kind::Getter {
                            (current, candidate.clone())
                        } else {
                            (candidate.clone(), current)
                        };
                        mixin_conflicts_of.push(Conflict::GetterMethod {
                            name: *n,
                            getter,
                            method,
                        });
                    }
                    candidates.insert(*n, vec![candidate]);
                } else {
                    of_super_and_mixin.insert(*n, vec![current, candidate]);
                }
            }
            let mut map = Map::default();
            most_specific(p, c, &mut map, &of_super_and_mixin, true);
            for (n, m) in map.iter() {
                candidates.insert(*n, vec![m.clone()]);
            }
            mixin_conflicts.push(mixin_conflicts_of);
            // `_addMixinMembers`.
            let object = p.core().object_class;
            for (n, m) in mi.implemented.iter() {
                if m.the_abstract_ || Some(m.class) == object {
                    continue;
                }
                let m = Self::substitute(p, m, &s);
                implemented.insert(*n, m);
            }
            super_impl.push(implemented.clone());
        }
        for &it in data.interfaces.iter() {
            let Some((ic, s)) = Self::substitution(p, it) else {
                continue;
            };
            let ii = self.interface(p, ic);
            Self::add_candidates(p, &mut candidates, &s, &ii);
        }
        let declared = Self::declared(p, c, &candidates);
        // `_addImplemented` (3.6.2 `inheritance_manager3.dart:455-470`): every
        // concrete instance method and accessor of the declaration, in order
        // (methods, then accessors), the last one winning. With same-named
        // members (the `augment` that 3.6.2 reads as another declaration), the
        // outline map keeps only the last one; the other elements are in the
        // class's member id range.
        for (n, m) in declared.iter() {
            if !m.the_abstract_ {
                implemented.insert(*n, m.clone());
            }
        }
        let same_named = Self::concrete_same_named(p, c);
        if !same_named.is_empty() {
            let interner = p.interner();
            let lib = prog.class(c).library;
            for f in same_named {
                let e = prog.function(f);
                let kind = kind_of(prog, f);
                let text = interner.resolve(e.name);
                let key = if kind == Kind::Setter && !text.ends_with("_=") {
                    match interner.lookup(&format!("{text}_=")) {
                        Some(s) => s,
                        None => continue,
                    }
                } else {
                    e.name
                };
                let name = MemberName::new(interner, lib, key);
                let m = match declared.get(&name) {
                    Some(d) if d.function == f => d.clone(),
                    _ => InterfaceMember {
                        function: f,
                        class: c,
                        ty: p.member_type(f),
                        kind,
                        the_abstract_: false,
                        the_covariant: written_covariance(prog, f).into(),
                        synthetic: false,
                    },
                };
                implemented.insert(name, m);
            }
        }
        let mut map = declared.clone();
        let mut conflicts = most_specific(p, c, &mut map, &candidates, true);
        // The `noSuchMethod` forwarders.
        let k = prog.class(c);
        let mut forwarded: HashSet<MemberName> = HashSet::new();
        let abstract_class = matches!(k.kind, ClassKind::Class | ClassKind::MixinApplication)
            && (k.modifiers.abstract_ || k.modifiers.sealed);
        if abstract_class {
            if let Some(si) = &super_interface {
                forwarded = si.forwarded.clone();
            }
        } else if let Some(nsm) = p.interner().lookup("noSuchMethod") {
            let name = MemberName {
                key: nsm,
                library: None,
            };
            let object = p.core().object_class;
            if implemented
                .get(&name)
                .is_some_and(|m| Some(m.class) != object)
            {
                let from_super = super_interface.as_ref().map(|s| &s.forwarded);
                for (n, m) in map.iter() {
                    if !implemented.contains_key(n) || from_super.is_some_and(|s| s.contains(n)) {
                        implemented.insert(*n, m.clone());
                        forwarded.insert(*n);
                    }
                }
            }
        }
        for cs in mixin_conflicts {
            conflicts.extend(cs);
        }
        // `_inheritCovariance`.
        let mut with_covariance: Map<InterfaceMember> = Map::default();
        for (n, m) in implemented.iter() {
            let r = inherit_covariance(p, c, &candidates, n, m);
            with_covariance.insert(*n, r);
        }
        Interface {
            map,
            declared,
            implemented: with_covariance,
            forwarded,
            overridden: candidates,
            redeclared: Map::default(),
            super_implemented: super_impl,
            conflicts,
        }
    }

    /// `_getInterfaceMixin`.
    fn of_mixin<'p>(&mut self, p: &mut dyn Provider<'p>, c: ClassId) -> Interface {
        let data = p.class_data(c);
        let mut constraints: Vec<TypeId> = data.on.to_vec();
        if constraints.is_empty()
            && let Some(o) = Self::object_type(p)
        {
            constraints.push(o);
        }
        let mut super_candidates: Map<Vec<InterfaceMember>> = Map::default();
        for t in constraints {
            let Some((rc, s)) = Self::substitution(p, t) else {
                continue;
            };
            let ri = self.interface(p, rc);
            Self::add_candidates(p, &mut super_candidates, &s, &ri);
        }
        let mut super_interface: Map<InterfaceMember> = Map::default();
        let super_conflicts = most_specific(p, c, &mut super_interface, &super_candidates, true);
        let mut candidates = super_candidates.clone();
        for &it in data.interfaces.iter() {
            let Some((ic, s)) = Self::substitution(p, it) else {
                continue;
            };
            let ii = self.interface(p, ic);
            Self::add_candidates(p, &mut candidates, &s, &ii);
        }
        let declared = Self::declared(p, c, &candidates);
        let mut map = declared.clone();
        let interface_conflicts = most_specific(p, c, &mut map, &candidates, true);
        let mut implemented: Map<InterfaceMember> = Map::default();
        for (n, m) in declared.iter() {
            if !m.the_abstract_ {
                implemented.insert(*n, m.clone());
            }
        }
        let mut conflicts = super_conflicts;
        conflicts.extend(interface_conflicts);
        Interface {
            map,
            declared,
            implemented,
            forwarded: HashSet::new(),
            overridden: candidates,
            redeclared: Map::default(),
            super_implemented: vec![super_interface],
            conflicts,
        }
    }

    /// `_getInterfaceExtensionType`.
    fn of_extension_type<'p>(&mut self, p: &mut dyn Provider<'p>, c: ClassId) -> Interface {
        let prog = p.program();
        let interner = p.interner();
        let data = p.class_data(c);
        let all = Self::declared(p, c, &Map::default());
        let mut declared: Map<InterfaceMember> = Map::default();
        for (n, m) in all.iter() {
            if !m.the_abstract_ {
                declared.insert(*n, m.clone());
            }
        }
        // The excluded names.
        let mut excluded_names: HashSet<MemberName> = HashSet::new();
        let mut excluded_methods: HashSet<MemberName> = HashSet::new();
        let mut excluded_setters: HashSet<MemberName> = HashSet::new();
        for (n, m) in declared.iter() {
            excluded_names.insert(*n);
            match m.kind {
                Kind::Method => {
                    if let Some(s) = n.from_setter(interner) {
                        excluded_setters.insert(s);
                    }
                }
                Kind::Setter => {
                    if let Some(g) = n.of_getter(interner) {
                        excluded_methods.insert(g);
                    }
                }
                Kind::Getter => {}
            }
        }
        let mut implemented = declared.clone();
        // The candidates, split between extension type members and the
        // others (`_ExtensionTypeCandidates`: methods, getters, setters).
        #[derive(Default, Clone)]
        struct Candidates {
            methods: Vec<InterfaceMember>,
            getters: Vec<InterfaceMember>,
            setters: Vec<InterfaceMember>,
        }
        impl Candidates {
            fn add(&mut self, m: InterfaceMember) {
                match m.kind {
                    Kind::Method => self.methods.push(m),
                    Kind::Getter => self.getters.push(m),
                    Kind::Setter => self.setters.push(m),
                }
            }
            fn all(&self) -> Vec<InterfaceMember> {
                self.methods
                    .iter()
                    .chain(self.getters.iter())
                    .chain(self.setters.iter())
                    .cloned()
                    .collect()
            }
        }
        let not_excluded = |n: &MemberName, cs: &Candidates| -> Vec<InterfaceMember> {
            if excluded_names.contains(n) {
                return Vec::new();
            }
            let mut v = Vec::new();
            if !excluded_methods.contains(n) {
                v.extend(cs.methods.iter().cloned());
            }
            v.extend(cs.getters.iter().cloned());
            if !excluded_setters.contains(n) {
                v.extend(cs.setters.iter().cloned());
            }
            v
        };
        let mut of_extension: Map<Candidates> = Map::default();
        let mut others: Map<Candidates> = Map::default();
        for &it in data.interfaces.iter() {
            let Some((ic, s)) = Self::substitution(p, it) else {
                continue;
            };
            let ii = self.interface(p, ic);
            for (n, m) in ii.map.iter() {
                let m = Self::substitute(p, m, &s);
                // `isExtensionTypeMember` is a bit of the element declared in the
                // extension type; the synthetic member from `_topMerge` (owner =
                // the extension type that merged it) is born without it and counts
                // as non-extension (`G implements F, D` with the merged `method`
                // from `A`/`B` in `F`: no conflict).
                let of_ext = !m.synthetic && prog.class(m.class).kind == ClassKind::ExtensionType;
                let target = if of_ext {
                    &mut of_extension
                } else {
                    &mut others
                };
                match target.get_mut(n) {
                    Some(cs) => cs.add(m),
                    None => {
                        let mut cs = Candidates::default();
                        cs.add(m);
                        target.insert(*n, cs);
                    }
                }
            }
        }
        let mut redeclared: Map<Vec<InterfaceMember>> = Map::default();
        let mut conflicts: Vec<Conflict> = Vec::new();
        for (n, cs) in of_extension.iter() {
            match redeclared.get_mut(n) {
                Some(l) => l.extend(cs.all()),
                None => redeclared.insert(*n, cs.all()),
            }
            let free = not_excluded(n, cs);
            if free.is_empty() {
                continue;
            }
            if let Some(ne) = others.get(n) {
                let ne_free = not_excluded(n, ne);
                if !ne_free.is_empty() {
                    conflicts.push(Conflict::ExtensionAndNonExtension {
                        name: *n,
                        non_extension: ne_free,
                        extension: free,
                    });
                }
                continue;
            }
            // The inherited one must be unique.
            let unique = free.iter().all(|m| m.function == free[0].function);
            if !unique {
                conflicts.push(Conflict::NonUniqueExtension {
                    name: *n,
                    candidates: free,
                });
                continue;
            }
            implemented.insert(*n, free[0].clone());
        }
        for (n, cs) in others.iter() {
            match redeclared.get_mut(n) {
                Some(l) => l.extend(cs.all()),
                None => redeclared.insert(*n, cs.all()),
            }
            let free = not_excluded(n, cs);
            if free.is_empty() || of_extension.contains_key(n) {
                continue;
            }
            match combine(p, c, &free, true, *n, None) {
                Some(m) => implemented.insert(*n, m),
                None => conflicts.push(Conflict::Candidates {
                    name: *n,
                    candidates: free,
                }),
            }
        }
        // The redeclared ones, without repetition.
        let mut the_unique: Map<Vec<InterfaceMember>> = Map::default();
        for (n, l) in redeclared.iter() {
            if l.len() <= 1 {
                the_unique.insert(*n, l.clone());
            } else {
                let mut seen: Vec<FunctionElementId> = Vec::new();
                let mut v = Vec::new();
                for m in l {
                    if !seen.contains(&m.function) {
                        seen.push(m.function);
                        v.push(m.clone());
                    }
                }
                the_unique.insert(*n, v);
            }
        }
        Interface {
            map: implemented.clone(),
            declared,
            implemented,
            forwarded: HashSet::new(),
            overridden: Map::default(),
            redeclared: the_unique,
            super_implemented: Vec::new(),
            conflicts,
        }
    }
}

/// The order of the instance members of a declaration in the analyzer's
/// element (`_getTypeMembers`, `_addImplemented`): methods; then accessors,
/// field accessors before written ones (`_visitPropertyFirst`); within each
/// group, source order.
fn member_order(prog: &Program, f: FunctionElementId) -> (u8, FunctionElementId) {
    let category = match kind_of(prog, f) {
        Kind::Method => 0u8,
        _ if matches!(prog.function(f).node, FunctionRef::None) => 1,
        _ => 2,
    };
    (category, f)
}

/// `_checkForGetterMethodConflict`.
fn getter_method_conflict(name: MemberName, candidates: &[InterfaceMember]) -> Option<Conflict> {
    let getter = candidates.iter().find(|m| m.kind == Kind::Getter)?;
    let method = candidates.iter().find(|m| m.kind == Kind::Method)?;
    Some(Conflict::GetterMethod {
        name,
        getter: getter.clone(),
        method: method.clone(),
    })
}

/// `_findMostSpecificFromNamedCandidates`.
fn most_specific<'p>(
    p: &mut dyn Provider<'p>,
    target: ClassId,
    map: &mut Map<InterfaceMember>,
    candidates: &Map<Vec<InterfaceMember>>,
    top_merge: bool,
) -> Vec<Conflict> {
    let mut conflicts = Vec::new();
    for (n, list) in candidates.iter() {
        if list.len() > 1
            && let Some(c) = getter_method_conflict(*n, list)
        {
            conflicts.push(c);
            continue;
        }
        if map.contains_key(n) {
            continue;
        }
        if let Some(m) = combine(p, target, list, top_merge, *n, Some(&mut conflicts)) {
            map.insert(*n, m);
        }
    }
    conflicts
}

/// `combineSignatures`.
pub fn combine<'p>(
    p: &mut dyn Provider<'p>,
    target: ClassId,
    candidates: &[InterfaceMember],
    top_merge: bool,
    name: MemberName,
    conflicts: Option<&mut Vec<Conflict>>,
) -> Option<InterfaceMember> {
    if candidates.len() == 1 {
        return Some(candidates[0].clone());
    }
    let mut valid: Vec<InterfaceMember> = Vec::new();
    for (i, c) in candidates.iter().enumerate() {
        let _ = i;
        let ok = candidates.iter().all(|d| p.sub(c.ty, d.ty));
        if ok {
            valid.push(c.clone());
        }
    }
    if valid.is_empty() {
        if let Some(cs) = conflicts {
            cs.push(Conflict::Candidates {
                name,
                candidates: candidates.to_vec(),
            });
        }
        return None;
    }
    if top_merge {
        Some(top(p, target, &valid))
    } else {
        Some(valid[0].clone())
    }
}

/// Dart type equality (`==`): structural, ignoring the alias, and up to the
/// names of the type parameters of a generic function.
fn equal(table: &mut TypeTable, a: TypeId, b: TypeId) -> bool {
    let (ca, cb) = (table.canonical(a), table.canonical(b));
    if ca == cb {
        return true;
    }
    match (table.get(ca).clone(), table.get(cb).clone()) {
        (
            Type::Function {
                type_params: ta, ..
            },
            Type::Function {
                type_params: tb, ..
            },
        ) if !ta.is_empty() && ta.len() == tb.len() => {
            let mut s: HashMap<TypeParamId, TypeId> = HashMap::new();
            for (x, y) in tb.iter().zip(ta.iter()) {
                let ty = table.intern(Type::TypeParameter {
                    param: *y,
                    nullable: false,
                });
                s.insert(*x, ty);
            }
            // The parameters of `b` renamed to those of `a`: the rest must
            // match.
            let rb = ops::substitute(cb, &s, table);
            let (
                Type::Function {
                    ret: ra,
                    positional: pa,
                    optional: oa,
                    named: at,
                    nullable: la,
                    ..
                },
                Type::Function {
                    ret: rr,
                    positional: pb,
                    optional: ob,
                    named: nb,
                    nullable: lb,
                    ..
                },
            ) = (table.get(ca).clone(), table.get(rb).clone())
            else {
                return false;
            };
            let limits = ta.iter().zip(tb.iter()).all(|(x, y)| {
                let lx = table.param(*x).bound;
                let ly = ops::substitute(table.param(*y).bound, &s, table);
                table.canonical(lx) == table.canonical(ly)
            });
            limits
                && la == lb
                && table.canonical(ra) == table.canonical(rr)
                && pa == pb
                && oa == ob
                && at == nb
        }
        _ => false,
    }
}

/// `_topMerge`.
fn top<'p>(
    p: &mut dyn Provider<'p>,
    target: ClassId,
    valid: &[InterfaceMember],
) -> InterfaceMember {
    let first = &valid[0];
    if valid.len() == 1 {
        return first.clone();
    }
    let all_equal = valid.iter().all(|m| equal(p.table(), m.ty, first.ty));
    if all_equal {
        return first.clone();
    }
    // `reduce` of `topMerge` over the normalized types; the parameters of the
    // result keep the covariance of the left operand.
    let mut result = p.normalize_(first.ty);
    let the_covariant = first.the_covariant.clone();
    for m in &valid[1..] {
        let t = p.normalize_(m.ty);
        result = top_merge_function(p, result, t, &the_covariant, &m.the_covariant);
    }
    for m in valid {
        if equal(p.table(), m.ty, result) {
            return m.clone();
        }
    }
    InterfaceMember {
        function: first.function,
        class: target,
        ty: result,
        kind: first.kind,
        the_abstract_: false,
        the_covariant,
        synthetic: true,
    }
}

/// `_inheritCovariance`.
fn inherit_covariance<'p>(
    p: &mut dyn Provider<'p>,
    c: ClassId,
    candidates: &Map<Vec<InterfaceMember>>,
    name: &MemberName,
    m: &InterfaceMember,
) -> InterfaceMember {
    if m.class == c {
        return m.clone();
    }
    let parameters = parameters_of_type(p.table(), m.ty);
    if parameters.is_empty() {
        return m.clone();
    }
    let Some(list) = candidates.get(name) else {
        return m.clone();
    };
    let mut the_covariant: HashSet<Param> = HashSet::new();
    for o in list {
        for d in parameters_of_type(p.table(), o.ty) {
            if o.the_covariant.contains(&d) {
                the_covariant.insert(d);
            }
        }
    }
    if the_covariant.is_empty() {
        return m.clone();
    }
    let must: Vec<Param> = parameters
        .iter()
        .copied()
        .filter(|d| the_covariant.contains(d))
        .collect();
    let changed = parameters
        .iter()
        .any(|d| m.the_covariant.contains(d) != must.contains(d));
    if !changed {
        return m.clone();
    }
    match m.kind {
        Kind::Method | Kind::Setter => InterfaceMember {
            class: c,
            the_covariant: must.into(),
            synthetic: true,
            ..m.clone()
        },
        Kind::Getter => m.clone(),
    }
}

// -- `NNBD_TOP_MERGE` -----------------------------------------------------------

fn is_nullable(table: &TypeTable, t: TypeId) -> bool {
    table.get(t).is_declared_nullable()
}

fn with_nullability(table: &mut TypeTable, t: TypeId, nullable: bool) -> TypeId {
    let the_new_ = match table.get(t).clone() {
        Type::Interface { class, args, .. } => Type::Interface {
            class,
            args,
            nullable,
        },
        Type::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            ..
        } => Type::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            nullable,
        },
        Type::Record {
            positional, named, ..
        } => Type::Record {
            positional,
            named,
            nullable,
        },
        Type::TypeParameter { param, .. } => Type::TypeParameter { param, nullable },
        Type::FutureOr { arg, .. } => Type::FutureOr { arg, nullable },
        Type::ExtensionType { decl, args, .. } => Type::ExtensionType {
            decl,
            args,
            nullable,
        },
        _ => return t,
    };
    table.intern(the_new_)
}

/// `topMerge` of any two types (nested function types have no covariant
/// parameters).
fn top_merge<'p>(p: &mut dyn Provider<'p>, t: TypeId, s: TypeId) -> TypeId {
    top_merge_function(p, t, s, &[], &[])
}

/// `TopMergeHelper.topMerge`, with parameter covariance when `t` and `s` are
/// the types of two members.
fn top_merge_function<'p>(
    p: &mut dyn Provider<'p>,
    t: TypeId,
    s: TypeId,
    cov_t: &[Param],
    cov_s: &[Param],
) -> TypeId {
    let object = p.core().object_class;
    let (tt, st) = (p.table().get(t).clone(), p.table().get(s).clone());
    let t_obj_q =
        matches!(tt, Type::Interface { class, nullable: true, .. } if Some(class) == object);
    let s_obj_q =
        matches!(st, Type::Interface { class, nullable: true, .. } if Some(class) == object);
    if t_obj_q && s_obj_q {
        return t;
    }
    let invalid_t = p.table().is_invalid(t);
    let invalid_s = p.table().is_invalid(s);
    let t_dyn = matches!(tt, Type::Dynamic) && !invalid_t;
    let s_dyn = matches!(st, Type::Dynamic) && !invalid_s;
    if t_dyn && s_dyn {
        return p.core().dynamic_;
    }
    if invalid_t {
        return t;
    }
    if invalid_s {
        return s;
    }
    if matches!(tt, Type::Never) && matches!(st, Type::Never) {
        return p.core().never;
    }
    let t_void = matches!(tt, Type::Void);
    let s_void = matches!(st, Type::Void);
    if t_void && s_void {
        return p.core().void_;
    }
    if (t_obj_q && s_void) || (t_void && s_obj_q) || (t_dyn && s_void) || (t_void && s_dyn) {
        return p.core().object_nullable;
    }
    if t_obj_q && s_dyn {
        return t;
    }
    if t_dyn && s_obj_q {
        return s;
    }
    let (tq, sq) = (is_nullable(p.table(), t), is_nullable(p.table(), s));
    if tq && sq {
        let tn = with_nullability(p.table(), t, false);
        let sn = with_nullability(p.table(), s, false);
        let r = top_merge_function(p, tn, sn, cov_t, cov_s);
        return with_nullability(p.table(), r, true);
    } else if tq || sq {
        // `StateError` in the original: does not happen with normalized types
        // that are subtypes of each other.
        return t;
    }
    match (tt, st) {
        (
            Type::Interface {
                class: a, args: xa, ..
            },
            Type::Interface {
                class: b, args: xb, ..
            },
        ) => {
            if a != b {
                return t;
            }
            if xa.is_empty() {
                return t;
            }
            let args: Vec<TypeId> = xa
                .iter()
                .zip(xb.iter())
                .map(|(x, y)| top_merge(p, *x, *y))
                .collect();
            p.table().intern(Type::Interface {
                class: a,
                args: args.into_boxed_slice(),
                nullable: false,
            })
        }
        (
            Type::ExtensionType {
                decl: a, args: xa, ..
            },
            Type::ExtensionType {
                decl: b, args: xb, ..
            },
        ) => {
            if a != b || xa.is_empty() {
                return t;
            }
            let args: Vec<TypeId> = xa
                .iter()
                .zip(xb.iter())
                .map(|(x, y)| top_merge(p, *x, *y))
                .collect();
            p.table().intern(Type::ExtensionType {
                decl: a,
                args: args.into_boxed_slice(),
                nullable: false,
            })
        }
        (Type::FutureOr { arg: a, .. }, Type::FutureOr { arg: b, .. }) => {
            let r = top_merge(p, a, b);
            p.table().intern(Type::FutureOr {
                arg: r,
                nullable: false,
            })
        }
        (Type::Null, Type::Null) => t,
        (Type::Function { .. }, Type::Function { .. }) => the_functions(p, t, s, cov_t, cov_s),
        (
            Type::Record {
                positional: pa,
                named: at,
                ..
            },
            Type::Record {
                positional: pb,
                named: nb,
                ..
            },
        ) => {
            if pa.len() != pb.len() || at.len() != nb.len() {
                return t;
            }
            let pos: Vec<TypeId> = pa
                .iter()
                .zip(pb.iter())
                .map(|(x, y)| top_merge(p, *x, *y))
                .collect();
            let mut named_args: Vec<(SymbolId, TypeId)> = Vec::new();
            for ((n1, x), (n2, y)) in at.iter().zip(nb.iter()) {
                if n1 != n2 {
                    return t;
                }
                named_args.push((*n1, top_merge(p, *x, *y)));
            }
            p.table().intern(Type::Record {
                positional: pos.into_boxed_slice(),
                named: named_args.into_boxed_slice(),
                nullable: false,
            })
        }
        (Type::TypeParameter { param: a, .. }, Type::TypeParameter { param: b, .. }) if a == b => t,
        _ => t,
    }
}

/// `_functionTypes`.
fn the_functions<'p>(
    p: &mut dyn Provider<'p>,
    t: TypeId,
    s: TypeId,
    cov_t: &[Param],
    cov_s: &[Param],
) -> TypeId {
    let (
        Type::Function {
            type_params: tp_t,
            ret: rt,
            positional: pt,
            optional: ot,
            named: nt,
            ..
        },
        Type::Function {
            type_params: tp_s,
            ret: rs,
            positional: ps,
            optional: os,
            named: ns,
            ..
        },
    ) = (p.table().get(t).clone(), p.table().get(s).clone())
    else {
        return t;
    };
    if tp_t.len() != tp_s.len() {
        return t;
    }
    // The new type parameters (`_typeParameters`).
    let mut new_: Vec<TypeParamId> = Vec::new();
    let mut sub_t: HashMap<TypeParamId, TypeId> = HashMap::new();
    let mut sub_s: HashMap<TypeParamId, TypeId> = HashMap::new();
    if !tp_t.is_empty() {
        for (a, b) in tp_t.iter().zip(tp_s.iter()) {
            let name = p.table().param(*a).name;
            let default_bound = p.core().object_nullable;
            let n = p.table().alloc_type_param(
                name,
                TypeParamOwner::GenericFunctionType,
                default_bound,
                Variance::Unspecified,
            );
            let tn = p.table().intern(Type::TypeParameter {
                param: n,
                nullable: false,
            });
            sub_t.insert(*a, tn);
            sub_s.insert(*b, tn);
            new_.push(n);
        }
        for (i, (a, b)) in tp_t.iter().zip(tp_s.iter()).enumerate() {
            let (ea, eb) = (p.table().param(*a).explicit, p.table().param(*b).explicit);
            if !ea && !eb {
                continue;
            }
            if ea != eb {
                return t;
            }
            let la = p.table().param(*a).bound;
            let lb = p.table().param(*b).bound;
            let la = ops::substitute(la, &sub_t, p.table());
            let lb = ops::substitute(lb, &sub_s, p.table());
            let l = top_merge(p, la, lb);
            p.table().set_type_param_bound(new_[i], l);
        }
    }
    let ret = join(p, rt, rs, &sub_t, &sub_s);
    if pt.len() != ps.len() || ot.len() != os.len() || nt.len() != ns.len() {
        return t;
    }
    let mut positional_args = Vec::with_capacity(pt.len());
    for (i, (a, b)) in pt.iter().zip(ps.iter()).enumerate() {
        positional_args.push(parameter(
            p,
            Param::Index(i),
            *a,
            *b,
            cov_t,
            cov_s,
            &sub_t,
            &sub_s,
        ));
    }
    let mut the_optional = Vec::with_capacity(ot.len());
    for (i, (a, b)) in ot.iter().zip(os.iter()).enumerate() {
        the_optional.push(parameter(
            p,
            Param::Index(pt.len() + i),
            *a,
            *b,
            cov_t,
            cov_s,
            &sub_t,
            &sub_s,
        ));
    }
    let mut named_args = Vec::with_capacity(nt.len());
    for ((n1, a, r1), (n2, b, r2)) in nt.iter().zip(ns.iter()) {
        if n1 != n2 {
            return t;
        }
        let ty = parameter(
            p,
            Param::MemberName(*n1),
            *a,
            *b,
            cov_t,
            cov_s,
            &sub_t,
            &sub_s,
        );
        named_args.push((*n1, ty, *r1 || *r2));
    }
    p.table().intern(Type::Function {
        type_params: new_.into_boxed_slice(),
        ret,
        positional: positional_args.into_boxed_slice(),
        optional: the_optional.into_boxed_slice(),
        named: named_args.into_boxed_slice(),
        nullable: false,
    })
}

/// `mergeTypes` of `_functionTypes`: both sides with the new type
/// parameters.
fn join<'p>(
    p: &mut dyn Provider<'p>,
    a: TypeId,
    b: TypeId,
    sub_t: &HashMap<TypeParamId, TypeId>,
    sub_s: &HashMap<TypeParamId, TypeId>,
) -> TypeId {
    let a = ops::substitute(a, sub_t, p.table());
    let b = ops::substitute(b, sub_s, p.table());
    top_merge(p, a, b)
}

/// A parameter of `_functionTypes`: the type by `topMerge`, or, if one of the
/// sides is covariant, the greater of the two (the raw types, without the
/// substitution, as in the original).
#[allow(clippy::too_many_arguments)]
fn parameter<'p>(
    p: &mut dyn Provider<'p>,
    d: Param,
    a: TypeId,
    b: TypeId,
    cov_t: &[Param],
    cov_s: &[Param],
    sub_t: &HashMap<TypeParamId, TypeId>,
    sub_s: &HashMap<TypeParamId, TypeId>,
) -> TypeId {
    if cov_t.contains(&d) || cov_s.contains(&d) {
        let a_sub_b = p.sub(a, b);
        let b_sub_a = p.sub(b, a);
        if a_sub_b && b_sub_a {
            join(p, a, b, sub_t, sub_s)
        } else if a_sub_b {
            b
        } else {
            a
        }
    } else {
        join(p, a, b, sub_t, sub_s)
    }
}
