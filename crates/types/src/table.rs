//! Type representation and hash-consed table (`TypeTable`).
//!
//! Memory discipline (docs/FRONTEND-ARQUITETURA.md §2):
//! - Semantic types are hash-consed: a single [`TypeId`] per structurally equal type.
//! - Nullability is part of the structural identity (`int` ≠ `int?`).
//! - No cyclic heap pointers (`Rc`/`RefCell` forbidden); nodes reference other
//!   types exclusively through [`TypeId`] and [`TypeParamId`].
//! - Continuous tracking of `payload_bytes` and the type count for plateau
//!   monitoring in the compiler and the LSP.

use dartforge_elements::model::{
    ClassId, ExtensionId, FunctionElementId, LibraryId, Program, TypedefId,
};
use dartforge_intern::{Interner, SymbolId};
use std::collections::HashMap;

/// Identifier of a type interned in the [`TypeTable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TypeId(pub u32);

/// Identifier of a type parameter in the [`TypeTable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TypeParamId(pub u32);

/// Owner of a type parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeParamOwner {
    Class(ClassId),
    Extension(ExtensionId),
    Typedef(TypedefId),
    Function(FunctionElementId),
    GenericFunctionType,
}

/// Variance of a type parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Variance {
    #[default]
    Unspecified,
    Covariant,
    Contravariant,
    Invariant,
}

/// Metadata of a type parameter registered in the table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParameterData {
    pub name: SymbolId,
    pub owner: TypeParamOwner,
    pub bound: TypeId,
    pub variance: Variance,
    /// The bound was written (`T extends Object?`), and is not the implicit
    /// `Object?`: instantiation to bounds and display tell them apart.
    pub explicito: bool,
}

/// Semantic representation of a Dart 3 type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    /// The `dynamic` type.
    Dynamic,
    /// The `void` type.
    Void,
    /// The `Never` type (bottom of the type system).
    Never,
    /// The `Null` type.
    Null,
    /// Interface type of a class, mixin or enum: `C<T1, ..., Tn>` or `C<T1, ..., Tn>?`.
    Interface {
        class: ClassId,
        args: Box<[TypeId]>,
        nullable: bool,
    },
    /// Structural function type: `U Function<X extends B>(P, [O], {N})`.
    Function {
        type_params: Box<[TypeParamId]>,
        ret: TypeId,
        positional: Box<[TypeId]>,
        optional: Box<[TypeId]>,
        /// Named parameters canonically ordered by `SymbolId`: `(the_name, tipo, required)`.
        named: Box<[(SymbolId, TypeId, bool)]>,
        nullable: bool,
    },
    /// Tuple/record type: `(T1, ..., Tn, {N1: U1, ...})`.
    Record {
        positional: Box<[TypeId]>,
        /// Named fields canonically ordered by `SymbolId`: `(the_name, tipo)`.
        named: Box<[(SymbolId, TypeId)]>,
        nullable: bool,
    },
    /// Type parameter in scope: `X` or `X?`.
    TypeParameter { param: TypeParamId, nullable: bool },
    /// Union type `FutureOr<T>` or `FutureOr<T>?`.
    FutureOr { arg: TypeId, nullable: bool },
    /// Extension type (Dart 3.3+): keeps its static identity and erases on demand.
    ExtensionType {
        decl: ClassId,
        args: Box<[TypeId]>,
        nullable: bool,
    },
    /// Promoted type variable `X & B` (`flow-analysis.md`, "promote"): only
    /// exists as the static type of a promoted read; never nullable.
    Intersection { param: TypeParamId, bound: TypeId },
}

impl Type {
    /// Tells whether the type was explicitly annotated as nullable with `?`.
    pub fn is_declared_nullable(&self) -> bool {
        match self {
            Type::Dynamic | Type::Void | Type::Null | Type::Never | Type::Intersection { .. } => false,
            Type::Interface { nullable, .. }
            | Type::Function { nullable, .. }
            | Type::Record { nullable, .. }
            | Type::TypeParameter { nullable, .. }
            | Type::FutureOr { nullable, .. }
            | Type::ExtensionType { nullable, .. } => *nullable,
        }
    }
}

/// Hash-consed type table.
///
/// Survives the whole compiler session and grows as the SDK and packages
/// are loaded. Guarantees that each structurally identical type gets
/// exactly the same [`TypeId`].
#[derive(Debug, Clone)]
pub struct TypeTable {
    types: Vec<Type>,
    lookup: HashMap<Type, TypeId>,
    type_params: Vec<TypeParameterData>,
    payload_bytes: usize,
    /// Types decorated for display (C9): the `typedef` alias with which the type
    /// was written (the analyzer's `TypeImpl.alias`) and `Never?` (which is
    /// `Null` semantically). Each decorated type is its own `TypeId` with the
    /// same structure as the target (`get` returns the target's), outside
    /// hash-consing; they only exist with [`TypeTable::preservar_exibicao`] on
    /// (analysis: parity, LSP), never in compilation.
    display: HashMap<TypeId, Exibicao>,
    decorated: HashMap<(TypeId, Exibicao), TypeId>,
    canonical: HashMap<TypeId, TypeId>,
    /// Enables the creation of decorated types ([`TypeTable::decorate`]).
    pub preservar_exibicao: bool,
    /// The sentinel parameter of the unknown `_` ([`CoreTypes::unknown_param`]),
    /// displayed as `_` like in the analyzer (`B<_>`).
    pub the_unknown_param: Option<TypeParamId>,
    /// The fresh formals of a generic function type whose bounds change in a
    /// substitution (`crate::ops::substitute`), keyed by the original formals
    /// and the substituted bounds: the same substitution gives the same
    /// formals.
    pub(crate) fresh_formals: HashMap<(Box<[TypeParamId]>, Box<[TypeId]>), Box<[TypeParamId]>>,
    /// The type parameters without a `defaultType` in the analyzer: those of
    /// function literals and of function types written in the body
    /// (`DefaultTypesBuilder` only runs on declarations). The exhaustiveness
    /// `TypeParameterReplacer` of 3.6.2 throws on one of them
    /// (`exhaustiveness.dart:771`).
    pub without_default_type: std::collections::HashSet<TypeParamId>,
}

/// The display decoration of a type (see [`TypeTable`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Exibicao {
    /// Written as the `typedef` alias with these arguments.
    Alias { typedef: dartforge_elements::model::TypedefId, args: Box<[TypeId]> },
    /// `Never?` (estrutura `Null`).
    NeverAnulavel,
    /// The analyzer's `InvalidType` (`dynamic` structure): the recovery type of
    /// a name or type that does not resolve. Behaves like `dynamic`, suppresses
    /// type diagnostics and is displayed as `InvalidType`.
    Invalido,
}

impl Default for TypeTable {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeTable {
    /// Creates a new empty table.
    pub fn new() -> Self {
        Self {
            types: Vec::with_capacity(1024),
            lookup: HashMap::with_capacity(1024),
            type_params: Vec::with_capacity(256),
            payload_bytes: 0,
            display: HashMap::new(),
            decorated: HashMap::new(),
            canonical: HashMap::new(),
            preservar_exibicao: false,
            the_unknown_param: None,
            fresh_formals: HashMap::new(),
            without_default_type: std::collections::HashSet::new(),
        }
    }

    /// The type `target` with the display decoration `ex` (a new `TypeId` with the
    /// same structure); without [`TypeTable::preservar_exibicao`], `target` itself.
    pub fn decorate(&mut self, target: TypeId, ex: Exibicao) -> TypeId {
        if !self.preservar_exibicao {
            return target;
        }
        let c = self.canonico(target);
        if let Some(&d) = self.decorated.get(&(c, ex.clone())) {
            return d;
        }
        let id = TypeId(self.types.len() as u32);
        let structure = self.types[c.0 as usize].clone();
        self.types.push(structure);
        self.display.insert(id, ex.clone());
        self.canonical.insert(id, c);
        self.decorated.insert((c, ex), id);
        id
    }

    /// The `InvalidType` (see [`Exibicao::Invalido`]); without
    /// [`TypeTable::preservar_exibicao`], `dynamic` itself.
    pub fn invalid(&mut self, dynamic_: TypeId) -> TypeId {
        self.decorate(dynamic_, Exibicao::Invalido)
    }

    /// Whether `t` is the `InvalidType` (at the top).
    pub fn e_invalido(&self, t: TypeId) -> bool {
        matches!(self.exibicao(t), Some(Exibicao::Invalido))
    }

    /// Whether `t` is the `InvalidType` or contains it in some position (type
    /// argument, parameter, return, record field): the analyzer's
    /// `hasInvalidType` (`dart/constant/has_invalid_type.dart:9-28`), which
    /// silences exhaustiveness, type argument bounds and unnecessary type
    /// warnings.
    pub fn contains_invalid(&self, t: TypeId) -> bool {
        self.contains_invalid_up_to(t, 0)
    }

    fn contains_invalid_up_to(&self, t: TypeId, prof: u32) -> bool {
        if self.e_invalido(t) {
            return true;
        }
        if prof > 32 {
            return false;
        }
        match self.get(t) {
            Type::Interface { args, .. } | Type::ExtensionType { args, .. } => {
                args.iter().any(|&a| self.contains_invalid_up_to(a, prof + 1))
            }
            Type::Function { ret, positional, optional, named, .. } => {
                self.contains_invalid_up_to(*ret, prof + 1)
                    || positional.iter().chain(optional.iter()).any(|&a| self.contains_invalid_up_to(a, prof + 1))
                    || named.iter().any(|&(_, a, _)| self.contains_invalid_up_to(a, prof + 1))
            }
            Type::Record { positional, named, .. } => {
                positional.iter().any(|&a| self.contains_invalid_up_to(a, prof + 1))
                    || named.iter().any(|&(_, a)| self.contains_invalid_up_to(a, prof + 1))
            }
            Type::FutureOr { arg, .. } => self.contains_invalid_up_to(*arg, prof + 1),
            _ => false,
        }
    }

    /// The type without the top-level decoration (the interned one).
    pub fn canonico(&self, t: TypeId) -> TypeId {
        self.canonical.get(&t).copied().unwrap_or(t)
    }

    /// Whether no decorated type was created.
    pub fn empty_display(&self) -> bool {
        self.display.is_empty()
    }

    /// The display decoration of `t`, if any.
    pub fn exibicao(&self, t: TypeId) -> Option<&Exibicao> {
        if self.display.is_empty() {
            return None;
        }
        self.display.get(&t)
    }

    /// Returns the number of unique types interned in the table.
    pub fn len(&self) -> usize {
        self.types.len()
    }

    /// Tells whether the table is empty.
    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    /// Returns the size in bytes taken by the structures interned in the table.
    pub fn payload_bytes(&self) -> usize {
        self.payload_bytes
    }

    /// Gets a reference to the type definition by its [`TypeId`].
    pub fn get(&self, id: TypeId) -> &Type {
        &self.types[id.0 as usize]
    }

    /// Gets the data of a type parameter by its [`TypeParamId`].
    pub fn param(&self, id: TypeParamId) -> &TypeParameterData {
        &self.type_params[id.0 as usize]
    }

    /// Gets a mutable reference to the data of a type parameter.
    pub fn param_mut(&mut self, id: TypeParamId) -> &mut TypeParameterData {
        &mut self.type_params[id.0 as usize]
    }

    /// Registers a new type parameter in the table.
    pub fn alloc_type_param(
        &mut self,
        name: SymbolId,
        owner: TypeParamOwner,
        bound: TypeId,
        variance: Variance,
    ) -> TypeParamId {
        let id = TypeParamId(self.type_params.len() as u32);
        self.type_params.push(TypeParameterData {
            name,
            owner,
            bound,
            variance,
            explicito: false,
        });
        self.payload_bytes += std::mem::size_of::<TypeParameterData>();
        id
    }

    /// Updates the bound (*bound*) of a type parameter (useful for mutual recursion / F-bounds).
    ///
    /// Marks the bound as written ([`TypeParameterData::explicito`]); copies of
    /// parameters restore the original's mark.
    pub fn set_type_param_bound(&mut self, id: TypeParamId, bound: TypeId) {
        let d = &mut self.type_params[id.0 as usize];
        d.bound = bound;
        d.explicito = true;
    }

    /// Interns a structural type in the table, guaranteeing uniqueness by hash-consing.
    ///
    /// Also guarantees canonicity by ordering named fields in records and function types.
    pub fn intern(&mut self, mut ty: Type) -> TypeId {
        // Canonical ordering for hash uniqueness and equality
        match &mut ty {
            Type::Function { named, .. } => {
                named.sort_by_key(|(sym, _, _)| sym.as_u32());
            }
            Type::Record { named, .. } => {
                named.sort_by_key(|(sym, _)| sym.as_u32());
            }
            _ => {}
        }

        if let Some(&existing) = self.lookup.get(&ty) {
            return existing;
        }

        let extra_bytes = match &ty {
            Type::Interface { args, .. } => args.len() * std::mem::size_of::<TypeId>(),
            Type::Function {
                type_params,
                positional,
                optional,
                named,
                ..
            } => {
                type_params.len() * std::mem::size_of::<TypeParamId>()
                    + (positional.len() + optional.len()) * std::mem::size_of::<TypeId>()
                    + named.len() * std::mem::size_of::<(SymbolId, TypeId, bool)>()
            }
            Type::Record {
                positional, named, ..
            } => {
                positional.len() * std::mem::size_of::<TypeId>()
                    + named.len() * std::mem::size_of::<(SymbolId, TypeId)>()
            }
            Type::ExtensionType { args, .. } => args.len() * std::mem::size_of::<TypeId>(),
            _ => 0,
        };

        let id = TypeId(self.types.len() as u32);
        self.payload_bytes +=
            std::mem::size_of::<Type>() + extra_bytes + std::mem::size_of::<(Type, TypeId)>() + 8;
        self.lookup.insert(ty.clone(), id);
        self.types.push(ty);
        id
    }

    /// The `TypeId` of an already interned type, without interning.
    pub fn intern_lookup(&self, ty: Type) -> Option<TypeId> {
        self.lookup.get(&ty).copied()
    }

    /// The display names of the formal parameters of function `ty`, besides
    /// those already renamed in `ren` (see `formatar`).
    fn rename_formals(&self, ty: TypeId, interner: &Interner, ren: &[(TypeParamId, String)]) -> Vec<(TypeParamId, String)> {
        let mut output = ren.to_vec();
        let Type::Function { type_params, .. } = self.get(ty) else { return output };
        if type_params.is_empty() {
            return output;
        }
        let mut referenced: Vec<TypeParamId> = Vec::new();
        self.referenced_parameters(ty, &mut referenced, 0);
        let name_of = |p: TypeParamId| -> String {
            ren.iter().rev().find(|(q, _)| *q == p).map(|(_, n)| n.clone()).unwrap_or_else(|| interner.resolve(self.param(p).name).to_string())
        };
        let mut avoid: std::collections::HashSet<String> =
            referenced.iter().filter(|p| !type_params.contains(p)).map(|&p| name_of(p)).collect();
        for &p in type_params.iter() {
            let base = interner.resolve(self.param(p).name).to_string();
            let mut the_name = base.clone();
            let mut counter = 0u32;
            while !avoid.insert(the_name.clone()) {
                let sub: String = counter.to_string().chars().map(|c| char::from_u32(0x2080 + (c as u32 - '0' as u32)).unwrap_or(c)).collect();
                the_name = format!("{base}{sub}");
                counter += 1;
            }
            output.push((p, the_name));
        }
        output
    }

    /// `collectTypeParameters`: the type parameters referenced in `ty` (bounds
    /// of function formals, parameters, return, arguments).
    fn referenced_parameters(&self, ty: TypeId, out: &mut Vec<TypeParamId>, prof: u32) {
        if prof > 32 {
            return;
        }
        match self.get(ty) {
            Type::TypeParameter { param, .. } | Type::Intersection { param, .. } => {
                if !out.contains(param) {
                    out.push(*param);
                }
            }
            Type::Function { type_params, ret, positional, optional, named, .. } => {
                for &p in type_params.iter() {
                    self.referenced_parameters(self.param(p).bound, out, prof + 1);
                }
                for &x in positional.iter().chain(optional.iter()) {
                    self.referenced_parameters(x, out, prof + 1);
                }
                for &(_, x, _) in named.iter() {
                    self.referenced_parameters(x, out, prof + 1);
                }
                self.referenced_parameters(*ret, out, prof + 1);
            }
            Type::Interface { args, .. } | Type::ExtensionType { args, .. } => {
                for &x in args.iter() {
                    self.referenced_parameters(x, out, prof + 1);
                }
            }
            Type::FutureOr { arg, .. } => self.referenced_parameters(*arg, out, prof + 1),
            _ => {}
        }
    }

    /// Returns a readable textual representation of a type for diagnostic
    /// messages: the `getDisplayString(preferTypeAlias: true)` with which the
    /// analyzer's `ErrorReporter` converts the argument types.
    pub fn format(&self, ty: TypeId, interner: &Interner, program: &Program) -> String {
        self.format_with(ty, interner, program, true, &[])
    }

    /// `getDisplayString()` without the alias (`DartType.toString`).
    pub fn format_sem_alias(&self, ty: TypeId, interner: &Interner, program: &Program) -> String {
        self.format_with(ty, interner, program, false, &[])
    }

    fn format_with(&self, ty: TypeId, interner: &Interner, program: &Program, alias: bool, ren: &[(TypeParamId, String)]) -> String {
        match self.exibicao(ty) {
            Some(Exibicao::NeverAnulavel) => return "Never?".to_string(),
            Some(Exibicao::Invalido) => return "InvalidType".to_string(),
            Some(Exibicao::Alias { typedef, args }) if alias => {
                let the_name = interner.resolve(program.typedefs[typedef.0 as usize].name);
                let q = if self.get(ty).is_declared_nullable() { "?" } else { "" };
                if args.is_empty() {
                    return format!("{the_name}{q}");
                }
                let a: Vec<String> = args.iter().map(|&x| self.format_with(x, interner, program, alias, ren)).collect();
                return format!("{the_name}<{}>{q}", a.join(", "));
            }
            _ => {}
        }
        let t = self.get(ty);
        match t {
            Type::Dynamic => "dynamic".to_string(),
            Type::Void => "void".to_string(),
            Type::Never => "Never".to_string(),
            Type::Null => "Null".to_string(),
            Type::Interface {
                class,
                args,
                nullable,
            } => {
                let name = interner.resolve(program.classes[class.0 as usize].name);
                let q = if *nullable { "?" } else { "" };
                if args.is_empty() {
                    format!("{name}{q}")
                } else {
                    let formatted_args: Vec<String> = args
                        .iter()
                        .map(|&a| self.format_with(a, interner, program, alias, ren))
                        .collect();
                    format!("{name}<{}>{q}", formatted_args.join(", "))
                }
            }
            Type::Function {
                ret,
                positional,
                optional,
                named,
                nullable,
                ..
            } => {
                let q = if *nullable { "?" } else { "" };
                // `_uniqueTypeParameters` (`display_string_builder.dart:552-614`):
                // a type parameter of the function with the name of a free parameter
                // referenced inside it gets a subscript (`T₀`, `T₁`).
                let ren2 = self.rename_formals(ty, interner, ren);
                let ren: &[(TypeParamId, String)] = &ren2;
                let mut p_strs = Vec::new();
                for &p in positional.iter() {
                    p_strs.push(self.format_with(p, interner, program, alias, ren));
                }
                if !optional.is_empty() {
                    let opt_strs: Vec<String> = optional
                        .iter()
                        .map(|&p| self.format_with(p, interner, program, alias, ren))
                        .collect();
                    p_strs.push(format!("[{}]", opt_strs.join(", ")));
                }
                if !named.is_empty() {
                    // The analyzer's `FunctionType` keeps the named parameters in
                    // alphabetical order (`type.dart:157`); the table keeps them in
                    // the order the names were interned.
                    let mut order: Vec<&(SymbolId, TypeId, bool)> = named.iter().collect();
                    order.sort_by(|a, b| interner.resolve(a.0).cmp(interner.resolve(b.0)));
                    let named_strs: Vec<String> = order
                        .into_iter()
                        .map(|(n, t, req)| {
                            let r = if *req { "required " } else { "" };
                            format!("{r}{} {}", self.format_with(*t, interner, program, alias, ren), interner.resolve(*n))
                        })
                        .collect();
                    p_strs.push(format!("{{{}}}", named_strs.join(", ")));
                }
                let ret_str = self.format_with(*ret, interner, program, alias, ren);
                // Generic: `T Function<T>(T)`, with the bound when it is not `Object?`.
                let tps = match t {
                    Type::Function { type_params, .. } if !type_params.is_empty() => {
                        let ps: Vec<String> = type_params
                            .iter()
                            .map(|&p| {
                                let d = self.param(p);
                                let the_name = ren.iter().rev().find(|(q, _)| *q == p).map(|(_, n)| n.as_str()).unwrap_or(interner.resolve(d.name));
                                match self.get(d.bound) {
                                    Type::Interface { nullable: true, class, .. }
                                        if !d.explicito && interner.resolve(program.classes[class.0 as usize].name) == "Object" =>
                                    {
                                        the_name.to_string()
                                    }
                                    Type::Dynamic => the_name.to_string(),
                                    _ => format!("{the_name} extends {}", self.format_with(d.bound, interner, program, alias, ren)),
                                }
                            })
                            .collect();
                        format!("<{}>", ps.join(", "))
                    }
                    _ => String::new(),
                };
                format!("{ret_str} Function{tps}({}){q}", p_strs.join(", "))
            }
            Type::Record {
                positional,
                named,
                nullable,
            } => {
                let q = if *nullable { "?" } else { "" };
                let mut parts = Vec::new();
                for &p in positional.iter() {
                    parts.push(self.format_with(p, interner, program, alias, ren));
                }
                // Like the analyzer: named ones between braces, and `(int,)` with a
                // single positional one.
                if !named.is_empty() {
                    // The analyzer's `RecordType` sorts the named fields by name.
                    let mut order: Vec<&(SymbolId, TypeId)> = named.iter().collect();
                    order.sort_by(|a, b| interner.resolve(a.0).cmp(interner.resolve(b.0)));
                    let named_args: Vec<String> = order
                        .into_iter()
                        .map(|(n, t)| format!("{} {}", self.format_with(*t, interner, program, alias, ren), interner.resolve(*n)))
                        .collect();
                    parts.push(format!("{{{}}}", named_args.join(", ")));
                } else if positional.len() == 1 {
                    return format!("({},){q}", parts[0]);
                }
                format!("({}){q}", parts.join(", "))
            }
            Type::FutureOr { arg, nullable } => {
                let q = if *nullable { "?" } else { "" };
                format!("FutureOr<{}>{q}", self.format_with(*arg, interner, program, alias, ren))
            }
            Type::TypeParameter { param, nullable } => {
                let q = if *nullable { "?" } else { "" };
                if Some(*param) == self.the_unknown_param {
                    return format!("_{q}");
                }
                if let Some((_, n)) = ren.iter().rev().find(|(p, _)| p == param) {
                    return format!("{n}{q}");
                }
                let name = interner.resolve(self.param(*param).name);
                format!("{name}{q}")
            }
            Type::Intersection { param, bound } => {
                // The promoted parameter uses the renamed name, when there is one.
                let the_name = ren.iter().rev().find(|(p, _)| p == param).map(|(_, n)| n.as_str()).unwrap_or(interner.resolve(self.param(*param).name));
                format!("{the_name} & {}", self.format_with(*bound, interner, program, alias, ren))
            }
            Type::ExtensionType {
                decl,
                args,
                nullable,
            } => {
                let name = interner.resolve(program.classes[decl.0 as usize].name);
                let q = if *nullable { "?" } else { "" };
                if args.is_empty() {
                    format!("{name}{q}")
                } else {
                    let formatted_args: Vec<String> = args
                        .iter()
                        .map(|&a| self.format_with(a, interner, program, alias, ren))
                        .collect();
                    format!("{name}<{}>{q}", formatted_args.join(", "))
                }
            }
        }
    }
}

/// Fundamental Dart types (`dart:core` and `dart:async`) cached for fast lookups.
#[derive(Debug, Clone)]
pub struct CoreTypes {
    pub dynamic_: TypeId,
    pub void_: TypeId,
    pub never: TypeId,
    pub null: TypeId,
    pub object: TypeId,
    pub object_nullable: TypeId,
    pub int: TypeId,
    pub num: TypeId,
    pub string: TypeId,
    pub bool_: TypeId,
    pub function: TypeId,
    pub record: TypeId,
    pub object_class: Option<ClassId>,
    pub int_class: Option<ClassId>,
    pub num_class: Option<ClassId>,
    pub string_class: Option<ClassId>,
    pub bool_class: Option<ClassId>,
    pub function_class: Option<ClassId>,
    pub record_class: Option<ClassId>,
    pub future_class: Option<ClassId>,
    pub iterable_class: Option<ClassId>,
    pub list_class: Option<ClassId>,
    pub map_class: Option<ClassId>,
    pub core_library: Option<LibraryId>,
    pub async_library: Option<LibraryId>,
    /// `double`.
    pub double: TypeId,
    pub double_class: Option<ClassId>,
    /// `Type` (the type of a type literal).
    pub type_: TypeId,
    /// `Symbol`.
    pub symbol: TypeId,
    pub set_class: Option<ClassId>,
    pub stream_class: Option<ClassId>,
    pub map_entry_class: Option<ClassId>,
    pub null_class: Option<ClassId>,
    /// The unknown type `_` of context schemas (`inference.md`, "Type schemas").
    /// Represented by a sentinel type parameter that never appears in final
    /// types: no inferred type contains it.
    pub unknown: TypeId,
    pub unknown_param: TypeParamId,
}

impl CoreTypes {
    /// `t` is the unknown `_` (with or without `?`).
    pub fn is_unknown(&self, table: &TypeTable, t: TypeId) -> bool {
        matches!(table.get(t), Type::TypeParameter { param, .. } if *param == self.unknown_param)
    }
}

impl CoreTypes {
    /// Initializes the fundamental types by registering them in the [`TypeTable`] and
    /// resolving the symbols in `dart:core` and `dart:async` from the [`Program`].
    pub fn init(table: &mut TypeTable, program: &Program, interner: &Interner) -> Self {
        let dynamic_ = table.intern(Type::Dynamic);
        let void_ = table.intern(Type::Void);
        let never = table.intern(Type::Never);
        let null = table.intern(Type::Null);

        let core_lib = program.core;
        let mut async_lib = None;
        for (i, lib) in program.libraries.iter().enumerate() {
            if lib.uri == "dart:async" {
                async_lib = Some(LibraryId(i as u32));
                break;
            }
        }

        let find_class = |lib_id: Option<LibraryId>, name: &str| -> Option<ClassId> {
            let lib = lib_id?;
            let sym = interner.lookup(name)?;
            let binding = program.lookup(lib, sym)?;
            match binding.getter {
                Some(dartforge_elements::model::Element::Class(cid)) => Some(cid),
                _ => None,
            }
        };

        let object_class = find_class(core_lib, "Object");
        let int_class = find_class(core_lib, "int");
        let num_class = find_class(core_lib, "num");
        let string_class = find_class(core_lib, "String");
        let bool_class = find_class(core_lib, "bool");
        let function_class = find_class(core_lib, "Function");
        let record_class = find_class(core_lib, "Record");
        let iterable_class = find_class(core_lib, "Iterable");
        let list_class = find_class(core_lib, "List");
        let map_class = find_class(core_lib, "Map");
        let future_class = find_class(async_lib, "Future");

        let make_interface =
            |table: &mut TypeTable, class_opt: Option<ClassId>, nullable: bool| -> TypeId {
                if let Some(class) = class_opt {
                    table.intern(Type::Interface {
                        class,
                        args: Box::new([]),
                        nullable,
                    })
                } else {
                    // Fallback for when the SDK has not been loaded yet (isolated unit tests)
                    if nullable { dynamic_ } else { never }
                }
            };

        let object = make_interface(table, object_class, false);
        let object_nullable = make_interface(table, object_class, true);
        let int = make_interface(table, int_class, false);
        let num = make_interface(table, num_class, false);
        let string = make_interface(table, string_class, false);
        let bool_ = make_interface(table, bool_class, false);
        let function = make_interface(table, function_class, false);
        let record = make_interface(table, record_class, false);
        let double_class = find_class(core_lib, "double");
        let double = make_interface(table, double_class, false);
        let type_ = make_interface(table, find_class(core_lib, "Type"), false);
        let symbol = make_interface(table, find_class(core_lib, "Symbol"), false);
        let set_class = find_class(core_lib, "Set");
        let stream_class = find_class(async_lib, "Stream");
        let map_entry_class = find_class(core_lib, "MapEntry");
        let null_class = find_class(core_lib, "Null");
        // Sentinel of the unknown `_`: a type parameter without a real owner.
        let name_unknown = interner
            .lookup("_")
            .or_else(|| object_class.map(|c| program.classes[c.0 as usize].name))
            .or_else(|| program.classes.first().map(|c| c.name))
            .or_else(|| interner.lookup("dynamic"))
            .unwrap_or_else(|| interner.texts().next().and_then(|t| interner.lookup(t)).expect("empty interner"));
        let unknown_param = table.alloc_type_param(
            name_unknown,
            TypeParamOwner::GenericFunctionType,
            object_nullable,
            Variance::Unspecified,
        );
        table.the_unknown_param = Some(unknown_param);
        let unknown = table.intern(Type::TypeParameter { param: unknown_param, nullable: false });

        Self {
            dynamic_,
            void_,
            never,
            null,
            object,
            object_nullable,
            int,
            num,
            string,
            bool_,
            function,
            record,
            object_class,
            int_class,
            num_class,
            string_class,
            bool_class,
            function_class,
            record_class,
            future_class,
            iterable_class,
            list_class,
            map_class,
            core_library: core_lib,
            async_library: async_lib,
            double,
            double_class,
            type_,
            symbol,
            set_class,
            stream_class,
            map_entry_class,
            null_class,
            unknown,
            unknown_param,
        }
    }
}
