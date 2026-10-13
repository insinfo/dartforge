//! `switch` exhaustiveness like analyzer 6.11: the space algorithm of
//! `_fe_analyzer_shared-76.0.0` (`lib/src/exhaustiveness/`, cited as `fe76:`)
//! with the analyzer's glue (`an611:src/generated/exhaustiveness.dart`) and
//! `ConstantVerifier`'s `_validateSwitchExhaustiveness`
//! (`an611:src/dart/constant/constant_verifier.dart:883-1000`).
//!
//! The algorithm's static types (`StaticType`) live in an arena interned by
//! Dart equality (`TypeBasedStaticType ==` compares the type and the
//! restriction; `NullableStaticType ==` the underlying one), so equality is
//! index equality. Spaces (`Space`) compare by identity, as in Dart (the
//! class does not define `==`), and that is why they are `Rc`.

use super::avaliador::Motor;
use super::valor::{double_as_dart, Estado, Valor};
use crate::table::{Type, TypeId, TypeParamId};
use dartforge_elements::model::{ClassId, ClassKind, FunctionKind, LibraryId, VariableId};
use dartforge_frontend::ast::{self, ExprId, ListPatternElement, PatternId, PatternKind};
use std::collections::HashMap;
use std::rc::Rc;

// -- Static types (`fe76:static_type.dart`, `fe76:types.dart`) ----------------

type St = usize;
const OBJ_NULLABLE: St = 0;
const OBJ: St = 1;
const NULL: St = 2;
const NEVER: St = 3;

/// Identity of an `IdentityRestriction`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Ident {
    Bool(bool),
    Enum(VariableId),
    /// Constant value with primitive equality (index into `Exhaustiveness::values`).
    Value(usize),
    /// `getUnknownStaticType`: unique, matches nothing and is matched by nothing.
    Unique(usize),
}

/// `Restriction` (the equality part of a `TypeBasedStaticType`).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum Restr {
    Free,
    Id(Ident),
    /// `ListTypeRestriction` (the arguments' text does not take part in
    /// equality).
    Lista { el: TypeId, tam: usize, resto: bool },
    /// `MapTypeRestriction`: the keys by their values (sorted indices).
    Mapa { k: TypeId, v: TypeId, chaves: Vec<usize> },
}

impl Restr {
    fn free(&self) -> bool {
        match self {
            Restr::Free => true,
            Restr::Id(_) => false,
            Restr::Lista { tam, resto: rest, .. } => *rest && *tam == 0,
            Restr::Mapa { chaves: keys, .. } => keys.is_empty(),
        }
    }
}

#[derive(Clone, Debug)]
enum Kind {
    Common,
    Bool,
    Enum(ClassId),
    Sealed(ClassId),
    FutureOr { arg: St, fut: St },
    Registro,
    ListType,
    /// `ListPatternStaticType` (the name already has the arguments' text).
    ListPattern,
    /// `MapPatternStaticType`: the keys in written order, with the text.
    MapPattern { chaves: Vec<(usize, String)> },
    BoolValue,
    EnumElement,
    /// `GeneralValueStaticType` (an ordinary constant or the unknown `?`).
    General,
}

#[derive(Clone, Debug)]
enum Type_ {
    /// `NullableStaticType(u)`; `OBJ_NULLABLE` and `NULL` have this shape.
    Nullable(St),
    Objeto,
    Never,
    Base { ty: TypeId, especie: Kind, the_restr: Restr, nome: String, implicitly_nullable: bool },
    Wrapped(St, St),
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum TypeKey {
    Anulavel(St),
    Base(TypeId, Restr),
    Wrapped(St, St),
}

// -- Keys (`fe76:key.dart`) ----------------------------------------------------

#[derive(Clone, Debug)]
enum Key {
    Mapa { id: usize, texto: String },
    Head(usize),
    Tail(usize),
    RestPart(usize, usize),
    /// `NameKey`; `registro` is `RecordNameKey`.
    Name { nome: Rc<str>, registro: bool },
    /// `RecordIndexKey` (name `$i+1`).
    Index(usize),
    Extension { recv: St, nome: Rc<str>, tipo: St },
}

impl Key {
    fn name(&self) -> String {
        match self {
            Key::Mapa { texto: text, .. } => format!("[{text}]"),
            Key::Head(i) => format!("[{i}]"),
            Key::Tail(i) => format!("[{}]", -(*i as i64 + 1)),
            Key::RestPart(h, t) => format!("[{h}:{}]", -(*t as i64)),
            Key::Name { nome: the_name, .. } | Key::Extension { nome: the_name, .. } => the_name.to_string(),
            Key::Index(i) => format!("${}", i + 1),
        }
    }

    fn of_record(&self) -> bool {
        matches!(self, Key::Index(_) | Key::Name { registro: true, .. })
    }

    fn of_list(&self) -> bool {
        matches!(self, Key::Head(_) | Key::Tail(_) | Key::RestPart(..))
    }

    /// The position in the total order of `compareTo`.
    fn group(&self) -> u8 {
        match self {
            Key::Head(_) => 0,
            Key::RestPart(..) => 1,
            Key::Tail(_) => 2,
            Key::Mapa { .. } => 3,
            Key::Index(_) => 4,
            Key::Name { .. } => 5,
            Key::Extension { .. } => 6,
        }
    }
}

impl PartialEq for Key {
    fn eq(&self, o: &Key) -> bool {
        match (self, o) {
            (Key::Mapa { id: a, .. }, Key::Mapa { id: b, .. }) => a == b,
            (Key::Head(a), Key::Head(b)) | (Key::Tail(a), Key::Tail(b)) => a == b,
            (Key::RestPart(a, b), Key::RestPart(c, d)) => a == c && b == d,
            // `NameKey ==` only looks at the name: `RecordIndexKey(0)` is `NameKey('$1')`.
            (Key::Name { .. } | Key::Index(_), Key::Name { .. } | Key::Index(_)) => self.name() == o.name(),
            (Key::Extension { recv: a, nome: n, .. }, Key::Extension { recv: b, nome: m, .. }) => a == b && n == m,
            _ => false,
        }
    }
}

/// `Key.compareTo` (an order only for choosing the witness stably).
fn compare_keys(ex: &Exhaustiveness, a: &Key, b: &Key) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (ga, gb) = (a.group(), b.group());
    if ga != gb {
        return ga.cmp(&gb);
    }
    match (a, b) {
        (Key::Head(x), Key::Head(y)) => x.cmp(y),
        (Key::Tail(x), Key::Tail(y)) => y.cmp(x),
        (Key::RestPart(h1, t1), Key::RestPart(h2, t2)) => h1.cmp(h2).then(t2.cmp(t1)),
        (Key::Mapa { texto: x, .. }, Key::Mapa { texto: y, .. }) => x.cmp(y),
        (Key::Index(x), Key::Index(y)) => x.cmp(y),
        (Key::Name { nome: x, .. }, Key::Name { nome: y, .. }) => x.cmp(y),
        (Key::Extension { recv: r1, nome: n1, .. }, Key::Extension { recv: r2, nome: n2, .. }) => {
            ex.nome(*r1).cmp(&ex.nome(*r2)).then(n1.cmp(n2))
        }
        _ => Ordering::Equal,
    }
}

// -- Paths and spaces (`fe76:path.dart`, `fe76:space.dart`) -------------------

#[derive(Clone, Default)]
struct Path(Option<Rc<(Path, Key)>>);

impl Path {
    fn plus(&self, k: Key) -> Path {
        Path(Some(Rc::new((self.clone(), k))))
    }

    fn list(&self) -> Vec<Key> {
        let mut v = Vec::new();
        let mut c = self;
        while let Some(rc) = &c.0 {
            v.push(rc.1.clone());
            c = &rc.0;
        }
        v.reverse();
        v
    }
}

struct SimpleD {
    ty: St,
    props: Vec<(Key, Space)>,
    additional: Vec<(Key, Space)>,
}

#[derive(Clone)]
struct Simple(Rc<SimpleD>);

impl Simple {
    fn new(typ: St) -> Simple {
        Simple(Rc::new(SimpleD { ty: typ, props: Vec::new(), additional: Vec::new() }))
    }

    /// `SingleSpace ==`: the type and the properties (spaces by identity).
    fn equal(&self, o: &Simple) -> bool {
        if Rc::ptr_eq(&self.0, &o.0) {
            return true;
        }
        if self.0.ty != o.0.ty || self.0.props.len() != o.0.props.len() {
            return false;
        }
        self.0.props.iter().all(|(k, v)| o.0.props.iter().any(|(k2, v2)| k == k2 && Rc::ptr_eq(&v.0, &v2.0)))
    }

    fn prop(&self, k: &Key) -> Option<&Space> {
        self.0.props.iter().find(|(c, _)| c == k).map(|(_, e)| e)
    }

    fn the_additional(&self, k: &Key) -> Option<&Space> {
        self.0.additional.iter().find(|(c, _)| c == k).map(|(_, e)| e)
    }
}

struct SpaceD {
    path: Path,
    simple: Vec<Simple>,
}

#[derive(Clone)]
struct Space(Rc<SpaceD>);

impl Space {
    fn novo(path: Path, typ: St) -> Space {
        Space(Rc::new(SpaceD { path, simple: vec![Simple::new(typ)] }))
    }

    fn the_with(path: Path, typ: St, props: Vec<(Key, Space)>, additional: Vec<(Key, Space)>) -> Space {
        Space(Rc::new(SpaceD { path, simple: vec![Simple(Rc::new(SimpleD { ty: typ, props, additional }))] }))
    }
}

/// A predicate of the witness (`fe76:witness.dart`).
#[derive(Clone)]
struct Predicate {
    caminho: Path,
    the_static: St,
    value: St,
}

/// `PropertyWitness`.
struct Witness {
    estatico: St,
    valor: St,
    props: Vec<(Key, Witness)>,
}

// -- The state: types, values, type system queries ----------------------------

/// What the pattern conversion reads from inference and evaluation.
pub(crate) struct Entry<'x> {
    pub ast: &'x ast::Ast,
    pub source: &'x str,
    pub pattern_types: &'x HashMap<PatternId, TypeId>,
    pub extension_fields: &'x HashMap<PatternId, TypeId>,
    pub invalid_patterns: &'x std::collections::HashSet<PatternId>,
    pub the_default_values: &'x HashMap<PatternId, Valor>,
    pub key_values: &'x HashMap<ExprId, Valor>,
    /// The library has version 3.3 or later (`createCastSpace`).
    pub version_3_3: bool,
}

/// A `switch` case for the check.
pub(crate) struct Case {
    /// `None`: `default`.
    pub pattern: Option<PatternId>,
    pub stored: bool,
}

/// A witness part (the analyzer's `MissingPatternPart`): text, enum value
/// (`writeReference(enum).the_name`) or type (`writeType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParteDeTestemunha {
    Texto(String),
    ValorDeEnum { enumeracao: ClassId, valor: VariableId },
    Tipo(TypeId),
}

/// The part markers in the witness text (`\u{1}` around them).
const MARK: char = '\u{1}';

/// The marked-up text split into parts; `None` when there is a general
/// constant (the `isComplete = false` of `AnalyzerDartTemplateBuffer`).
fn text_parts(m: &Motor<'_>, s: &str) -> Option<Vec<ParteDeTestemunha>> {
    let mut v: Vec<ParteDeTestemunha> = Vec::new();
    let mut text = String::new();
    let mut rest = s;
    while let Some(i) = rest.find(MARK) {
        text.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let j = after.find(MARK)?;
        let mark = &after[..j];
        rest = &after[j + 1..];
        if !text.is_empty() {
            v.push(ParteDeTestemunha::Texto(std::mem::take(&mut text)));
        }
        let (typ, num) = mark.split_at(1);
        let n: u32 = num.parse().ok()?;
        match typ {
            "T" => v.push(ParteDeTestemunha::Tipo(TypeId(n))),
            "E" => {
                let value = VariableId(n);
                let enumeration = m.program.variable(value).class?;
                v.push(ParteDeTestemunha::ValorDeEnum { enumeracao: enumeration, valor: value });
            }
            _ => return None,
        }
    }
    text.push_str(rest);
    if !text.is_empty() {
        v.push(ParteDeTestemunha::Texto(text));
    }
    Some(v)
}

/// The result of `_validateSwitchExhaustiveness`.
pub(crate) struct Result_ {
    /// Indices (into `cases`) of the unreachable cases.
    pub unreachable: Vec<usize>,
    /// The first witness, when not exhaustive (without correction).
    pub witness: Option<String>,
    /// The first witness for the correction.
    pub fix: Option<String>,
    /// All the witnesses with parts (only with `collect_parts`).
    pub parts: Vec<Vec<ParteDeTestemunha>>,
}

pub(crate) struct Exhaustiveness<'m, 'a> {
    m: &'m mut Motor<'a>,
    types: Vec<Type_>,
    internal: HashMap<TypeKey, St>,
    the_unique: usize,
    values: Vec<Valor>,
    subtypes: HashMap<St, Vec<St>>,
    lib: LibraryId,
    invalid: bool,
    /// Writes the witnesses with part markers.
    mark: bool,
    /// Computes [`Result_::partes`].
    pub(crate) collect_parts: bool,
}

impl<'m, 'a> Exhaustiveness<'m, 'a> {
    pub(crate) fn new(m: &'m mut Motor<'a>, lib: LibraryId) -> Self {
        let types = vec![Type_::Nullable(OBJ), Type_::Objeto, Type_::Nullable(NEVER), Type_::Never];
        let mut internal = HashMap::new();
        internal.insert(TypeKey::Anulavel(OBJ), OBJ_NULLABLE);
        internal.insert(TypeKey::Anulavel(NEVER), NULL);
        Exhaustiveness { m, types, internal, the_unique: 0, values: Vec::new(), subtypes: HashMap::new(), lib, invalid: false, mark: false, collect_parts: false }
    }

    fn the_intern(&mut self, key: TypeKey, t: impl FnOnce() -> Type_) -> St {
        if let Some(&s) = self.internal.get(&key) {
            return s;
        }
        let s = self.types.len();
        self.types.push(t());
        self.internal.insert(key, s);
        s
    }

    fn base(&mut self, ty: TypeId, kind: Kind, the_restr: Restr, the_name: String, impl_nullable: bool) -> St {
        let key = TypeKey::Base(ty, the_restr.clone());
        self.the_intern(key, || Type_::Base { ty, especie: kind, the_restr, nome: the_name, implicitly_nullable: impl_nullable })
    }

    fn wrapped(&mut self, w: St, i: St) -> St {
        self.the_intern(TypeKey::Wrapped(w, i), || Type_::Wrapped(w, i))
    }

    fn unique(&mut self, ty: TypeId, the_name: &str) -> St {
        self.the_unique += 1;
        let id = self.the_unique;
        self.base(ty, Kind::General, Restr::Id(Ident::Unique(id)), the_name.to_string(), false)
    }

    /// `getUnknownStaticType`.
    fn unknown(&mut self) -> St {
        let o = self.m.core.object;
        self.unique(o, "?")
    }

    // ---- type system queries (`AnalyzerTypeOperations`) ----

    fn sub_dart(&mut self, a: TypeId, b: TypeId) -> bool {
        self.m.sub(a, b)
    }

    /// `typeToString` (`DartType.toString`, without the alias).
    fn text(&self, t: TypeId) -> String {
        self.m.table.format_sem_alias(t, self.m.interner, self.m.program)
    }

    fn dart_nullable(&self, t: TypeId) -> bool {
        self.m.table.get(t).is_declared_nullable()
    }

    /// `promoteToNonNull`.
    fn dart_non_null(&mut self, t: TypeId) -> TypeId {
        match self.m.table.get(t).clone() {
            Type::TypeParameter { param, nullable: false } => {
                let b = self.m.table.param(param).bound;
                let nb = self.dart_non_null(b);
                if nb == b {
                    t
                } else {
                    self.m.table.intern(Type::Intersection { param, bound: nb })
                }
            }
            Type::Dynamic | Type::Void => t,
            _ => crate::ops::non_nullable(t, self.m.table),
        }
    }

    fn as_instance_of(&mut self, t: TypeId, c: Option<ClassId>) -> Option<TypeId> {
        let c = c?;
        let t = crate::ops::non_nullable(t, self.m.table);
        self.m.outline.hierarchy.supertype_of(t, c, self.m.table, self.m.core)
    }

    fn args_of(&self, t: TypeId) -> Vec<TypeId> {
        match self.m.table.get(t) {
            Type::Interface { args, .. } => args.to_vec(),
            _ => Vec::new(),
        }
    }

    /// `hasSimpleName`.
    fn simple_name(&self, t: TypeId) -> bool {
        matches!(
            self.m.table.get(t),
            Type::Interface { .. }
                | Type::Dynamic
                | Type::Void
                | Type::Never
                | Type::TypeParameter { .. }
                | Type::Intersection { .. }
                | Type::FutureOr { .. }
                | Type::Null
        )
    }

    /// `TypeParameterReplacer.replaceTypeVariables` (`overapproximate`).
    fn overapproximate(&mut self, t: TypeId) -> TypeId {
        self.substitute_parameters(t, 1, 0)
    }

    /// `variance`: 1 covariant, -1 contravariant, 0 invariant.
    fn substitute_parameters(&mut self, t: TypeId, variance: i8, prof: u32) -> TypeId {
        if prof > 16 {
            return t;
        }
        match self.m.table.get(t).clone() {
            Type::TypeParameter { param, .. } | Type::Intersection { param, .. } => {
                if variance == -1 {
                    self.m.core.never
                } else {
                    // `element.defaultType!` null: 3.6.2 throws.
                    if self.m.table.without_default_type.contains(&param) {
                        self.m.analyzer_break = true;
                    }
                    let d = self.parameter_default(param);
                    self.substitute_parameters(d, variance, prof + 1)
                }
            }
            Type::Interface { class, args, nullable } => {
                let args: Vec<TypeId> = args.iter().map(|&a| self.substitute_parameters(a, variance, prof + 1)).collect();
                self.m.table.intern(Type::Interface { class, args: args.into_boxed_slice(), nullable })
            }
            Type::FutureOr { arg, nullable } => {
                let arg = self.substitute_parameters(arg, variance, prof + 1);
                self.m.table.intern(Type::FutureOr { arg, nullable })
            }
            Type::Record { positional, named, nullable } => {
                let positional: Vec<TypeId> = positional.iter().map(|&a| self.substitute_parameters(a, variance, prof + 1)).collect();
                let named: Vec<_> = named.iter().map(|&(n, a)| (n, self.substitute_parameters(a, variance, prof + 1))).collect();
                self.m.table.intern(Type::Record { positional: positional.into_boxed_slice(), named: named.into_boxed_slice(), nullable })
            }
            Type::Function { type_params, ret, positional, optional, named, nullable } => {
                let ret = self.substitute_parameters(ret, variance, prof + 1);
                let inv = -variance;
                let positional: Vec<TypeId> = positional.iter().map(|&a| self.substitute_parameters(a, inv, prof + 1)).collect();
                let optional: Vec<TypeId> = optional.iter().map(|&a| self.substitute_parameters(a, inv, prof + 1)).collect();
                let named: Vec<_> = named.iter().map(|&(n, a, r)| (n, self.substitute_parameters(a, inv, prof + 1), r)).collect();
                self.m.table.intern(Type::Function {
                    type_params,
                    ret,
                    positional: positional.into_boxed_slice(),
                    optional: optional.into_boxed_slice(),
                    named: named.into_boxed_slice(),
                    nullable,
                })
            }
            _ => t,
        }
    }

    /// `TypeParameterElement.defaultType` (instantiation to the bound).
    fn parameter_default(&mut self, p: TypeParamId) -> TypeId {
        crate::ops::instantiate_to_bounds(&[p], &[], self.m.table, self.m.core)[0]
    }

    // ---- `getStaticType` (`fe76:shared.dart:161-234`) ----

    fn static_type(&mut self, t: TypeId) -> St {
        // `createStaticType(InvalidType)` liga `hasInvalidType`.
        if self.m.table.contains_invalid(t) {
            self.invalid = true;
        }
        let t = self.m.table.canonico(t);
        let t = self.m.apagar(t);
        match self.m.table.get(t).clone() {
            Type::Never => return NEVER,
            Type::Null => return NULL,
            Type::Dynamic => return OBJ_NULLABLE,
            Type::Interface { class, nullable, .. } if Some(class) == self.m.core.object_class => {
                return if nullable { OBJ_NULLABLE } else { OBJ };
            }
            _ => {}
        }
        let nn = self.dart_non_null(t);
        let core = self.m.core;
        let mut st = match self.m.table.get(nn).clone() {
            Type::Interface { class, .. } if Some(class) == core.bool_class => {
                let n = self.text(nn);
                self.base(nn, Kind::Bool, Restr::Free, n, false)
            }
            Type::Record { .. } => {
                let n = self.text(nn);
                self.base(nn, Kind::Registro, Restr::Free, n, false)
            }
            Type::FutureOr { arg, .. } => {
                let a = self.static_type(arg);
                let fut = match core.future_class {
                    Some(fc) => self.m.table.intern(Type::Interface { class: fc, args: vec![arg].into_boxed_slice(), nullable: false }),
                    None => core.object,
                };
                let f = self.static_type(fut);
                let n = self.text(nn);
                let ia = self.dart_nullable(arg) || matches!(self.m.table.get(arg), Type::Null);
                self.base(nn, Kind::FutureOr { arg: a, fut: f }, Restr::Free, n, ia)
            }
            Type::Interface { class, .. } if self.m.program.class(class).kind == ClassKind::Enum => {
                let n = self.text(nn);
                self.base(nn, Kind::Enum(class), Restr::Free, n, false)
            }
            Type::Interface { class, .. }
                if self.m.program.class(class).kind == ClassKind::Class && self.m.program.class(class).modifiers.sealed =>
            {
                let n = self.text(nn);
                self.base(nn, Kind::Sealed(class), Restr::Free, n, false)
            }
            _ => {
                if self.as_instance_of(nn, core.list_class).is_some() {
                    let n = self.text(nn);
                    self.base(nn, Kind::ListType, Restr::Free, n, false)
                } else {
                    let n = self.text(nn);
                    let s = self.base(nn, Kind::Common, Restr::Free, n, false);
                    let bnd = match self.m.table.get(t).clone() {
                        Type::TypeParameter { param, .. } => Some(self.m.table.param(param).bound),
                        Type::Intersection { bound, .. } => Some(bound),
                        _ => None,
                    };
                    match bnd {
                        Some(b) => {
                            let sb = self.static_type(b);
                            self.wrapped(sb, s)
                        }
                        None => s,
                    }
                }
            }
        };
        if self.dart_nullable(t) {
            st = self.nullable(st);
        }
        st
    }

    // ---- `StaticType` operations ----

    fn nullable(&mut self, s: St) -> St {
        match self.types[s].clone() {
            Type_::Never => NULL,
            Type_::Objeto => OBJ_NULLABLE,
            Type_::Nullable(_) => s,
            Type_::Base { .. } => self.the_intern(TypeKey::Anulavel(s), || Type_::Nullable(s)),
            Type_::Wrapped(w, i) => {
                let (nw, ni) = (self.nullable(w), self.nullable(i));
                if nw == w && ni == i {
                    s
                } else {
                    self.wrapped(nw, ni)
                }
            }
        }
    }

    fn non_null(&mut self, s: St) -> St {
        match self.types[s].clone() {
            Type_::Nullable(u) => u,
            Type_::Wrapped(w, i) => {
                let (nw, ni) = (self.non_null(w), self.non_null(i));
                if nw == w && ni == i {
                    s
                } else {
                    self.wrapped(nw, ni)
                }
            }
            _ => s,
        }
    }

    fn the_sealed(&self, s: St) -> bool {
        match &self.types[s] {
            Type_::Nullable(_) => s != NULL,
            Type_::Base { especie: kind, .. } => matches!(
                kind,
                Kind::Bool | Kind::Enum(_) | Kind::Sealed(_) | Kind::FutureOr { .. } | Kind::ListType
            ),
            Type_::Wrapped(w, _) => self.the_sealed(*w),
            _ => false,
        }
    }

    fn implicitly_nullable(&self, s: St) -> bool {
        match &self.types[s] {
            Type_::Nullable(_) => true,
            Type_::Objeto | Type_::Never => false,
            Type_::Base { implicitly_nullable, .. } => *implicitly_nullable,
            Type_::Wrapped(w, _) => self.implicitly_nullable(*w),
        }
    }

    fn nome(&self, s: St) -> String {
        match &self.types[s] {
            _ if s == NULL => "Null".to_string(),
            Type_::Nullable(u) => {
                if self.implicitly_nullable(*u) {
                    self.nome(*u)
                } else {
                    format!("{}?", self.nome(*u))
                }
            }
            Type_::Objeto => "Object".to_string(),
            Type_::Never => "Never".to_string(),
            Type_::Base { nome: the_name, .. } => the_name.clone(),
            Type_::Wrapped(w, _) => self.nome(*w),
        }
    }

    /// `isSubtypeOf`.
    fn sub(&mut self, a: St, b: St) -> bool {
        if a == b || b == OBJ_NULLABLE {
            return true;
        }
        if a == NEVER {
            return true;
        }
        if let Type_::Wrapped(w, i) = self.types[b].clone() {
            if self.sub(a, w) && self.sub(a, i) {
                return true;
            }
        }
        match self.types[a].clone() {
            Type_::Objeto => false,
            Type_::Never => true,
            Type_::Nullable(u) => match self.types[b].clone() {
                Type_::Nullable(u2) => self.sub(u, u2),
                _ => false,
            },
            Type_::Wrapped(w, i) => self.sub(w, b) || self.sub(i, b),
            Type_::Base { ty, the_restr, .. } => {
                if b == OBJ {
                    return true;
                }
                match self.types[b].clone() {
                    Type_::Nullable(u2) => self.sub(a, u2),
                    Type_::Base { ty: ty2, the_restr: r2, .. } => self.sub_dart(ty, ty2) && self.sub_constraint(&the_restr, &r2),
                    _ => false,
                }
            }
        }
    }

    fn sub_constraint(&mut self, a: &Restr, b: &Restr) -> bool {
        if b.free() {
            return true;
        }
        match (a, b) {
            (Restr::Free, _) => false,
            (Restr::Id(x), Restr::Id(y)) => x == y,
            (Restr::Lista { el, tam, resto: rest }, Restr::Lista { el: el2, tam: tam2, resto: rest2 }) => {
                if !self.sub_dart(*el, *el2) {
                    return false;
                }
                if *rest2 {
                    tam >= tam2
                } else if *rest {
                    false
                } else {
                    tam == tam2
                }
            }
            (Restr::Mapa { k, v, chaves: keys }, Restr::Mapa { k: k2, v: v2, chaves: c2 }) => {
                self.sub_dart(*k, *k2) && self.sub_dart(*v, *v2) && c2.iter().all(|c| keys.contains(c))
            }
            _ => false,
        }
    }

    /// `getSubtypes`.
    fn subtypes(&mut self, s: St, keys: &[Key]) -> Vec<St> {
        match self.types[s].clone() {
            Type_::Nullable(u) => {
                if s == NULL {
                    Vec::new()
                } else {
                    vec![u, NULL]
                }
            }
            Type_::Wrapped(w, i) => {
                if let (Type_::Nullable(wu), Type_::Nullable(iu)) = (self.types[w].clone(), self.types[i].clone()) {
                    let e = self.wrapped(wu, iu);
                    return vec![e, NULL];
                }
                let subs = self.subtypes(w, keys);
                subs.into_iter().map(|x| self.wrapped(x, i)).collect()
            }
            Type_::Base { ty, especie: kind, .. } => match kind {
                Kind::Bool => {
                    let t = self.base(ty, Kind::BoolValue, Restr::Id(Ident::Bool(true)), "true".into(), false);
                    let f = self.base(ty, Kind::BoolValue, Restr::Id(Ident::Bool(false)), "false".into(), false);
                    vec![t, f]
                }
                Kind::FutureOr { arg, fut } => vec![arg, fut],
                Kind::Enum(c) => {
                    if let Some(v) = self.subtypes.get(&s) {
                        return v.clone();
                    }
                    let v = self.enum_elements(s, ty, c);
                    self.subtypes.insert(s, v.clone());
                    v
                }
                Kind::Sealed(c) => {
                    if let Some(v) = self.subtypes.get(&s) {
                        return v.clone();
                    }
                    let v = self.sealed_subclasses(s, ty, c);
                    self.subtypes.insert(s, v.clone());
                    v
                }
                Kind::ListType => self.list_subtypes(ty, keys),
                _ => Vec::new(),
            },
            _ => Vec::new(),
        }
    }

    /// `EnumStaticType._createEnumElements` (`fe76:types/enum.dart`).
    fn enum_elements(&mut self, s: St, ty: TypeId, c: ClassId) -> Vec<St> {
        let constantes = self.m.program.class(c).enum_constants.clone();
        // `overapproximate(_type)` per element (`enum.dart:118-124`): none
        // without elements.
        let over = if constantes.is_empty() { ty } else { self.overapproximate(ty) };
        let mut v = Vec::new();
        for var in constantes {
            let el = self.enum_element(c, var);
            let Type_::Base { ty: tel, .. } = self.types[el].clone() else { continue };
            if self.sub_dart(tel, over) {
                v.push(self.wrapped(el, s));
            }
        }
        v
    }

    fn enum_element(&mut self, c: ClassId, var: VariableId) -> St {
        let ty = self.m.outline.variables[var.0 as usize]
            .declared_type
            .or(self.m.outline.variables[var.0 as usize].inferred)
            .unwrap_or_else(|| {
                let args = Vec::new();
                self.m.table.intern(Type::Interface { class: c, args: args.into_boxed_slice(), nullable: false })
            });
        let the_name = format!(
            "{}.{}",
            self.m.interner.resolve(self.m.program.class(c).name),
            self.m.interner.resolve(self.m.program.variables[var.0 as usize].name)
        );
        self.base(ty, Kind::EnumElement, Restr::Id(Ident::Enum(var)), the_name, false)
    }

    /// `AnalyzerSealedClassOperations.getDirectSubclasses` and
    /// `SealedClassStaticType._createSubtypes` (`fe76:types/sealed.dart`).
    fn sealed_subclasses(&mut self, s: St, ty: TypeId, sealed: ClassId) -> Vec<St> {
        let program = self.m.program;
        let lib = program.class(sealed).library;
        let mut direct = Vec::new();
        for (i, k) in program.classes.iter().enumerate() {
            let id = ClassId(i as u32);
            if k.library != lib || id == sealed || k.decl.is_none() || k.kind == ClassKind::ExtensionType {
                continue;
            }
            let mut sup = k.supertype_class;
            // The synthetic chain of `extends B with M` leads to the written superclass.
            let mut mixins: Vec<ClassId> = k.mixin_classes.clone();
            let mut guard = 0;
            while let Some(sc) = sup {
                guard += 1;
                if program.class(sc).decl.is_some() || guard > 50 {
                    break;
                }
                mixins.extend(program.class(sc).mixin_classes.iter().copied());
                sup = program.class(sc).supertype_class;
            }
            if sup == Some(sealed)
                || mixins.contains(&sealed)
                || k.interface_classes.contains(&sealed)
                || (k.kind == ClassKind::Mixin && k.on_classes.contains(&sealed))
            {
                direct.push(id);
            }
        }
        // `overapproximate(_type)` only for a non-generic subclass
        // (`sealed.dart:76-96`).
        let mut over: Option<TypeId> = None;
        let mut v = Vec::new();
        for sub in direct {
            let Some(st) = self.subclass_as_instance(sub, ty) else { continue };
            if self.args_of(st).is_empty() {
                let over = match over {
                    Some(x) => x,
                    None => {
                        let x = self.overapproximate(ty);
                        over = Some(x);
                        x
                    }
                };
                if !self.sub_dart(st, over) {
                    continue;
                }
            }
            let e = self.static_type(st);
            v.push(self.wrapped(e, s));
        }
        v
    }

    /// `getSubclassAsInstanceOf`.
    fn subclass_as_instance(&mut self, sub: ClassId, the_sealed: TypeId) -> Option<TypeId> {
        let params = self.m.outline.classes[sub.0 as usize].type_params.to_vec();
        let args_this: Vec<TypeId> =
            params.iter().map(|&p| self.m.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
        let this = self.m.table.intern(Type::Interface { class: sub, args: args_this.clone().into_boxed_slice(), nullable: false });
        let Type::Interface { class: cs, args: selected_args, .. } = self.m.table.get(the_sealed).clone() else { return None };
        let as_ = self.m.outline.hierarchy.supertype_of(this, cs, self.m.table, self.m.core)?;
        if params.is_empty() {
            return Some(this);
        }
        let args_as = self.args_of(as_);
        let mut trivial = args_this.len() == args_as.len() && args_this.iter().zip(args_as.iter()).all(|(a, b)| a == b);
        if trivial && selected_args.len() == params.len() {
            let map: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(selected_args.iter().copied()).collect();
            for (i, &p) in params.iter().enumerate() {
                let d = self.m.table.param(p).clone();
                if d.explicito {
                    let b = crate::ops::substitute(d.bound, &map, self.m.table);
                    if !self.sub_dart(selected_args[i], b) {
                        trivial = false;
                        break;
                    }
                }
            }
        } else {
            trivial = false;
        }
        if trivial {
            Some(self.m.table.intern(Type::Interface { class: sub, args: selected_args.clone(), nullable: false }))
        } else {
            Some(self.overapproximate(this))
        }
    }

    /// `ListTypeStaticType.getSubtypes` (`fe76:types/list.dart`).
    fn list_subtypes(&mut self, ty: TypeId, keys: &[Key]) -> Vec<St> {
        let (mut cab, mut cau) = (0usize, 0usize);
        for k in keys {
            match k {
                Key::Head(i) if *i >= cab => cab = i + 1,
                Key::Tail(i) if *i >= cau => cau = i + 1,
                _ => {}
            }
        }
        let max = cab + cau;
        let el = self.as_instance_of(ty, self.m.core.list_class).and_then(|l| self.args_of(l).first().copied()).unwrap_or(self.m.core.dynamic_);
        let text = if matches!(self.m.table.get(el), Type::Dynamic) { String::new() } else { format!("<{}>", self.text(el)) };
        let mut v = Vec::new();
        for tam in 0..=max {
            let rest = tam == max;
            let the_name = list_text(&text, tam, rest);
            v.push(self.base(ty, Kind::ListPattern, Restr::Lista { el, tam, resto: rest }, the_name, false));
        }
        v
    }

    // ---- propriedades ----

    /// The (Dart) type of instance member `the_name` of `t`
    /// (`_getInterfaceFieldTypes`, `an611:src/generated/exhaustiveness.dart:389-417`).
    fn member(&mut self, t: TypeId, the_name: &str) -> Option<TypeId> {
        let t = crate::ops::non_nullable(t, self.m.table);
        let (cls, args) = match self.m.table.get(t).clone() {
            Type::Interface { class, args, .. } => (class, args.to_vec()),
            Type::Record { positional, named, .. } => {
                if let Some(i) = the_name.strip_prefix('$').and_then(|x| x.parse::<usize>().ok()) {
                    if i >= 1 && i <= positional.len() {
                        return Some(positional[i - 1]);
                    }
                }
                for (n, x) in named.iter() {
                    if self.m.interner.resolve(*n) == the_name {
                        return Some(*x);
                    }
                }
                let o = self.m.core.object;
                return self.member(o, the_name);
            }
            _ => {
                let o = self.m.core.object;
                if t == o {
                    return None;
                }
                return self.member(o, the_name);
            }
        };
        let sym = self.m.interner.lookup(the_name)?;
        let private = the_name.starts_with('_');
        let mut candidates: Vec<TypeId> = Vec::new();
        let mut supers = vec![t];
        if let Some(h) = self.m.outline.hierarchy.get(cls) {
            let map: HashMap<TypeParamId, TypeId> = h.type_params.iter().copied().zip(args.iter().copied()).collect();
            let all = h.all_supertypes.clone();
            for s in all {
                supers.push(crate::ops::substitute(s, &map, self.m.table));
            }
        }
        if let Some(o) = self.m.core.object_class {
            if cls != o {
                supers.push(self.m.core.object);
            }
        }
        for s in supers {
            let Type::Interface { class: k, args: ka, .. } = self.m.table.get(s).clone() else { continue };
            let Some(&f) = self.m.program.class(k).instance_members.get(&sym) else { continue };
            let fe = self.m.program.function(f);
            if private && fe.library != self.lib {
                continue;
            }
            let raw = match fe.kind {
                FunctionKind::Setter => continue,
                FunctionKind::Getter => self.m.outline.functions[f.0 as usize].return_type,
                FunctionKind::ImplicitAccessor => match fe.variable {
                    Some(v) => {
                        let vd = &self.m.outline.variables[v.0 as usize];
                        vd.declared_type.or(vd.inferred).unwrap_or(self.m.outline.functions[f.0 as usize].return_type)
                    }
                    None => self.m.outline.functions[f.0 as usize].return_type,
                },
                _ => self.m.outline.functions[f.0 as usize].signature,
            };
            let params = self.m.outline.classes[k.0 as usize].type_params.clone();
            let map: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(ka.iter().copied()).collect();
            candidates.push(crate::ops::substitute(raw, &map, self.m.table));
        }
        // The most specific one (the covariant overrider).
        let mut best = *candidates.first()?;
        for &c in &candidates[1..] {
            if c != best && self.sub_dart(c, best) && !self.sub_dart(best, c) {
                best = c;
            }
        }
        Some(best)
    }

    /// `getObjectFieldType`.
    fn object_field(&mut self, k: &Key) -> Option<St> {
        if !matches!(k, Key::Name { .. } | Key::Index(_)) {
            return None;
        }
        let o = self.m.core.object;
        let t = self.member(o, &k.name())?;
        Some(self.static_type(t))
    }

    /// `fields[key]` of a `TypeBasedStaticType` (`getFieldTypes`).
    fn type_field(&mut self, ty: TypeId, k: &Key) -> Option<St> {
        if !matches!(k, Key::Name { .. } | Key::Index(_)) {
            return None;
        }
        let t = self.member(ty, &k.name())?;
        Some(self.static_type(t))
    }

    /// `StaticType.fields[key]`.
    fn fields(&mut self, s: St, k: &Key) -> Option<St> {
        match self.types[s].clone() {
            Type_::Base { ty, .. } => self.type_field(ty, k),
            Type_::Wrapped(w, _) => self.fields(w, k),
            _ => None,
        }
    }

    /// `getPropertyType`.
    fn property(&mut self, s: St, k: &Key) -> Option<St> {
        match self.types[s].clone() {
            Type_::Base { ty, .. } => self.type_field(ty, k),
            Type_::Wrapped(w, _) => self.property(w, k),
            _ => self.object_field(k),
        }
    }

    /// `getAdditionalPropertyType`.
    fn the_additional(&mut self, s: St, k: &Key) -> Option<St> {
        let Type_::Base { ty, .. } = self.types[s].clone() else { return None };
        let core = self.m.core;
        match k {
            Key::Mapa { .. } => {
                let m = self.as_instance_of(ty, core.map_class)?;
                let v = *self.args_of(m).get(1)?;
                Some(self.static_type(v))
            }
            Key::Head(_) | Key::Tail(_) => {
                let l = self.as_instance_of(ty, core.list_class)?;
                let e = *self.args_of(l).first()?;
                Some(self.static_type(e))
            }
            Key::RestPart(..) => {
                let l = self.as_instance_of(ty, core.list_class)?;
                Some(self.static_type(l))
            }
            _ => self.object_field(k),
        }
    }

    // ---- o algoritmo (`fe76:exhaustive.dart`) ----

    fn unmatched(&mut self, lines: &[Vec<Space>], values: &[Space], preds: &[Predicate], several: bool) -> Option<Vec<Vec<Predicate>>> {
        if values.is_empty() {
            if !lines.is_empty() {
                return None;
            }
            return Some(vec![preds.to_vec()]);
        }
        let first = values[0].clone();
        let mut interesting: Vec<Key> = Vec::new();
        for l in lines {
            for s in &l[0].0.simple {
                for (k, _) in &s.0.additional {
                    if !interesting.contains(k) {
                        interesting.push(k.clone());
                    }
                }
            }
        }
        for value in first.0.simple.clone() {
            let context = value.0.ty;
            let mut stack = std::collections::VecDeque::from([context]);
            let mut witnesses: Option<Vec<Vec<Predicate>>> = None;
            while let Some(t) = stack.pop_front() {
                if self.sub(t, NEVER) {
                    continue;
                }
                if self.the_sealed(t) {
                    let r = self.filter_by_type(context, t, lines, &value, values, preds, &first.0.path, false);
                    if r.is_some() {
                        let subs = self.subtypes(t, &interesting);
                        stack.extend(subs);
                    }
                } else {
                    let r = self.filter_by_type(context, t, lines, &value, values, preds, &first.0.path, false);
                    if let Some(r) = r {
                        witnesses.get_or_insert_with(Vec::new).extend(r);
                        if !several {
                            return witnesses;
                        }
                    }
                }
            }
            if witnesses.is_some() {
                return witnesses;
            }
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    fn filter_by_type(
        &mut self,
        context: St,
        t: St,
        lines: &[Vec<Space>],
        value: &Simple,
        values: &[Space],
        preds: &[Predicate],
        path: &Path,
        several: bool,
    ) -> Option<Vec<Vec<Predicate>>> {
        let mut extended = preds.to_vec();
        extended.push(Predicate { caminho: path.clone(), the_static: context, value: t });
        let mut first_: Vec<Simple> = Vec::new();
        let mut remaining: Vec<Vec<Space>> = Vec::new();
        for l in lines {
            for s in &l[0].0.simple {
                if self.sub(t, s.0.ty) {
                    first_.push(s.clone());
                    remaining.push(l.clone());
                }
            }
        }
        let mut keys: Vec<Key> = Vec::new();
        let mut additional: Vec<Key> = Vec::new();
        for s in std::iter::once(value).chain(first_.iter()) {
            for (k, _) in &s.0.props {
                if !keys.contains(k) {
                    keys.push(k.clone());
                }
            }
            for (k, _) in &s.0.additional {
                if !additional.contains(k) {
                    additional.push(k.clone());
                }
            }
        }
        keys.sort_by(|a, b| compare_keys(self, a, b));
        additional.sort_by(|a, b| compare_keys(self, a, b));
        let mut new_values = self.expand(&keys, &additional, value, t, path);
        new_values.extend(values[1..].iter().cloned());
        let mut new_lines = Vec::with_capacity(remaining.len());
        for (i, l) in remaining.iter().enumerate() {
            let s = &first_[i];
            let mut nl = self.expand(&keys, &additional, s, s.0.ty, path);
            nl.extend(l[1..].iter().cloned());
            new_lines.push(nl);
        }
        self.unmatched(&new_lines, &new_values, &extended, several)
    }

    fn expand(&mut self, keys: &[Key], additional: &[Key], s: &Simple, t: St, path: &Path) -> Vec<Space> {
        let mut r = Vec::new();
        for k in keys {
            if let Some(e) = s.prop(k) {
                r.push(e.clone());
            } else {
                let mut pt = self.property(t, k);
                if pt.is_none() {
                    if let Key::Extension { tipo: typ, .. } = k {
                        pt = Some(*typ);
                    }
                }
                r.push(Space::novo(path.plus(k.clone()), pt.unwrap_or(OBJ_NULLABLE)));
            }
        }
        for k in additional {
            if let Some(e) = s.the_additional(k) {
                r.push(e.clone());
            } else {
                let pt = self.the_additional(t, k);
                r.push(Space::novo(path.plus(k.clone()), pt.unwrap_or(OBJ_NULLABLE)));
            }
        }
        r
    }

    /// `computeExhaustiveness`.
    fn compute(&mut self, value: St, stored: &[bool], spaces: &[Space]) -> (Vec<usize>, Option<Vec<Vec<Predicate>>>) {
        let mut lines: Vec<Vec<Space>> = Vec::new();
        let mut unreachable = Vec::new();
        for (i, e) in spaces.iter().enumerate() {
            let line = vec![e.clone()];
            if i > 0 && self.unmatched(&lines, &line, &[], false).is_none() {
                unreachable.push(i);
            }
            if !stored[i] {
                lines.push(line);
            }
        }
        let root = Space::novo(Path::default(), value);
        let t = self.unmatched(&lines, &[root], &[], true);
        (unreachable, t)
    }

    // ---- pattern spaces (`SpaceCreator`, `PatternConverter`) ----

    fn with_context(&mut self, context: St, t: TypeId, non_null: bool) -> St {
        let mut s = self.static_type(t);
        if self.sub(context, s) {
            s = context;
        }
        if non_null {
            s = self.non_null(s);
        }
        s
    }

    fn union(&mut self, a: &Space, b: &Space) -> Space {
        let mut all = a.0.simple.clone();
        all.extend(b.0.simple.iter().cloned());
        self.of_simple(a.0.path.clone(), all)
    }

    /// `Space.fromSingleSpaces`.
    fn of_simple(&mut self, path: Path, simple: Vec<Simple>) -> Space {
        let empty = Simple::new(NEVER);
        let mut set: Vec<Simple> = Vec::new();
        for s in simple {
            if s.equal(&empty) {
                continue;
            }
            if !set.iter().any(|x| x.equal(&s)) {
                set.push(s);
            }
        }
        if set.is_empty() {
            set.push(empty);
        } else if set.len() == 2 {
            let (a, b) = (set[0].clone(), set[1].clone());
            if a.0.ty == NULL && a.0.props.is_empty() && b.0.props.is_empty() {
                let n = self.nullable(b.0.ty);
                set = vec![Simple::new(n)];
            } else if b.0.ty == NULL && b.0.props.is_empty() && a.0.props.is_empty() {
                let n = self.nullable(a.0.ty);
                set = vec![Simple::new(n)];
            }
        }
        Space(Rc::new(SpaceD { path, simple: set }))
    }

    fn pattern(&mut self, e: &Entry<'_>, path: Path, context: St, p: PatternId, non_null: bool) -> Space {
        if e.invalid_patterns.contains(&p) {
            self.invalid = true;
        }
        match &e.ast.pattern(p).kind {
            PatternKind::Variable { final_, var_, ty, .. } => {
                let Some(&t) = e.pattern_types.get(&p) else {
                    // `case the_name:` is a constant pattern: the value of the top-level
                    // constant, if the verifier found it; otherwise the check is
                    // skipped, as with an invalid type.
                    let _ = (final_, var_, ty);
                    if let Some(v) = e.the_default_values.get(&p).cloned() {
                        return self.constant_value(&v, path);
                    }
                    self.invalid = true;
                    let d = self.unknown();
                    return Space::novo(path, d);
                };
                let s = self.with_context(context, t, non_null);
                Space::novo(path, s)
            }
            PatternKind::Wildcard { ty } => {
                let s = match ty.and_then(|_| e.pattern_types.get(&p).copied()) {
                    None => {
                        if non_null {
                            self.non_null(context)
                        } else {
                            context
                        }
                    }
                    Some(t) => self.with_context(context, t, non_null),
                };
                Space::novo(path, s)
            }
            PatternKind::Object { fields, .. } => {
                let Some(&t) = e.pattern_types.get(&p) else {
                    self.invalid = true;
                    let d = self.unknown();
                    return Space::novo(path, d);
                };
                let s = self.with_context(context, t, non_null);
                let mut the_fields: Vec<(Rc<str>, PatternId, Option<TypeId>)> = Vec::new();
                for f in fields.iter() {
                    let the_name = match f.name {
                        Some(n) => Some(n.sym),
                        None => implicit_name(e.ast, f.pattern),
                    };
                    let Some(the_name) = the_name else { continue };
                    let the_name: Rc<str> = self.m.interner.resolve(the_name).into();
                    let ext = e.extension_fields.get(&f.pattern).copied();
                    if let Some(x) = the_fields.iter_mut().find(|x| x.0 == the_name) {
                        x.1 = f.pattern;
                        x.2 = ext;
                    } else {
                        the_fields.push((the_name, f.pattern, ext));
                    }
                }
                let mut props: Vec<(Key, Space)> = Vec::new();
                for (the_name, sub, ext) in the_fields {
                    let (key, pt) = match ext {
                        Some(et) => {
                            let pt = self.static_type(et);
                            let recv = self.static_type(t);
                            (Key::Extension { recv, nome: the_name, tipo: pt }, pt)
                        }
                        None => {
                            let k = Key::Name { nome: the_name, registro: false };
                            let pt = self.property(s, &k).unwrap_or(OBJ_NULLABLE);
                            (k, pt)
                        }
                    };
                    let es = self.pattern(e, path.plus(key.clone()), pt, sub, false);
                    props.push((key, es));
                }
                Space::the_with(path, s, props, Vec::new())
            }
            PatternKind::Record { fields } => {
                let d = self.m.core.dynamic_;
                let mut pos: Vec<PatternId> = Vec::new();
                let mut named_args: Vec<(Rc<str>, PatternId)> = Vec::new();
                for f in fields.iter() {
                    let implicit = f.name.is_none() && e.source.as_bytes().get(f.span.start) == Some(&b':');
                    if f.name.is_none() && !implicit {
                        pos.push(f.pattern);
                    } else {
                        let the_name = f.name.map(|n| n.sym).or_else(|| implicit_name(e.ast, f.pattern));
                        let Some(the_name) = the_name else { continue };
                        let the_name: Rc<str> = self.m.interner.resolve(the_name).into();
                        if let Some(x) = named_args.iter_mut().find(|x| x.0 == the_name) {
                            x.1 = f.pattern;
                        } else {
                            named_args.push((the_name, f.pattern));
                        }
                    }
                }
                let mut named: Vec<(dartforge_intern::SymbolId, TypeId)> = Vec::new();
                for (n, _) in &named_args {
                    if let Some(sym) = self.m.interner.lookup(n) {
                        named.push((sym, d));
                    }
                }
                named.sort_by_key(|x| x.0);
                let rt = self.m.table.intern(Type::Record {
                    positional: vec![d; pos.len()].into_boxed_slice(),
                    named: named.into_boxed_slice(),
                    nullable: false,
                });
                let s = self.with_context(context, rt, true);
                let mut props = Vec::new();
                for (i, sub) in pos.into_iter().enumerate() {
                    let k = Key::Index(i);
                    let pt = self.property(s, &k).unwrap_or(OBJ_NULLABLE);
                    let es = self.pattern(e, path.plus(k.clone()), pt, sub, false);
                    props.push((k, es));
                }
                for (the_name, sub) in named_args {
                    let k = Key::Name { nome: the_name, registro: true };
                    let pt = self.property(s, &k).unwrap_or(OBJ_NULLABLE);
                    let es = self.pattern(e, path.plus(k.clone()), pt, sub, false);
                    props.push((k, es));
                }
                Space::the_with(path, s, props, Vec::new())
            }
            PatternKind::Or(x, y) => {
                let (x, y) = (*x, *y);
                let a = self.pattern(e, path.clone(), context, x, non_null);
                let b = self.pattern(e, path, context, y, non_null);
                self.union(&a, &b)
            }
            PatternKind::And(x, y) => {
                let (x, y) = (*x, *y);
                let a = self.pattern(e, path.clone(), context, x, non_null);
                let b = self.pattern(e, path.clone(), context, y, non_null);
                self.intersection(path, &a, &b)
            }
            PatternKind::NullCheck(x) => self.pattern(e, path, context, *x, true),
            PatternKind::Parenthesized(x) => self.pattern(e, path, context, *x, non_null),
            PatternKind::NullAssert(x) => {
                let s = self.pattern(e, path.clone(), context, *x, true);
                let n = Space::novo(path, NULL);
                self.union(&s, &n)
            }
            PatternKind::Cast { pattern, .. } => {
                let sub = *pattern;
                let Some(&t) = e.pattern_types.get(&p) else {
                    self.invalid = true;
                    let d = self.unknown();
                    return Space::novo(path, d);
                };
                self.cast(e, path, context, t, sub, non_null)
            }
            PatternKind::Relational { .. } => {
                let d = self.unknown();
                Space::novo(path, d)
            }
            PatternKind::List { type_args, elements } => {
                let Some(&t) = e.pattern_types.get(&p) else {
                    self.invalid = true;
                    let d = self.unknown();
                    return Space::novo(path, d);
                };
                let el = self.args_of(t).first().copied().unwrap_or(self.m.core.dynamic_);
                let mut head = Vec::new();
                let mut tail = Vec::new();
                let mut rest: Option<Option<PatternId>> = None;
                for x in elements.iter() {
                    match x {
                        ListPatternElement::Rest(r) => rest = Some(*r),
                        ListPatternElement::Pattern(q) if rest.is_some() => tail.push(*q),
                        ListPatternElement::Pattern(q) => head.push(*q),
                    }
                }
                let text = if type_args.is_empty() { String::new() } else { format!("<{}>", self.text(el)) };
                let (nc, nt) = (head.len(), tail.len());
                let has_rest = rest.is_some();
                let the_name = list_text(&text, nc + nt, has_rest);
                let tt = crate::ops::non_nullable(t, self.m.table);
                let mut s = self.base(tt, Kind::ListPattern, Restr::Lista { el, tam: nc + nt, resto: has_rest }, the_name, false);
                if self.dart_nullable(t) {
                    s = self.nullable(s);
                }
                let mut ad = Vec::new();
                for (i, q) in head.into_iter().enumerate() {
                    let k = Key::Head(i);
                    let pt = self.the_additional(s, &k).unwrap_or(OBJ_NULLABLE);
                    let es = self.pattern(e, path.plus(k.clone()), pt, q, false);
                    ad.push((k, es));
                }
                if let Some(r) = rest {
                    let k = Key::RestPart(nc, nt);
                    let pt = self.the_additional(s, &k).unwrap_or(OBJ_NULLABLE);
                    let es = match r {
                        Some(q) => self.pattern(e, path.plus(k.clone()), pt, q, false),
                        None => Space::novo(path.plus(k.clone()), pt),
                    };
                    ad.push((k, es));
                }
                for i in 0..nt {
                    let k = Key::Tail(i);
                    let pt = self.the_additional(s, &k).unwrap_or(OBJ_NULLABLE);
                    let es = self.pattern(e, path.plus(k.clone()), pt, tail[nt - i - 1], false);
                    ad.push((k, es));
                }
                Space::the_with(path, s, Vec::new(), ad)
            }
            PatternKind::Map { type_args, entries, .. } => {
                let Some(&t) = e.pattern_types.get(&p) else {
                    self.invalid = true;
                    let d = self.unknown();
                    return Space::novo(path, d);
                };
                let a = self.args_of(t);
                let (k, v) = (a.first().copied().unwrap_or(self.m.core.dynamic_), a.get(1).copied().unwrap_or(self.m.core.dynamic_));
                let mut keys: Vec<(usize, String, PatternId)> = Vec::new();
                for en in entries.iter() {
                    let Some(val) = e.key_values.get(&en.key).cloned() else {
                        let d = self.unknown();
                        return Space::novo(path, d);
                    };
                    let id = self.value_id(&val);
                    let text = self.value_text(&val);
                    if let Some(x) = keys.iter_mut().find(|x| x.0 == id) {
                        x.2 = en.value;
                    } else {
                        keys.push((id, text, en.value));
                    }
                }
                let text = if type_args.len() == 2 { format!("<{}, {}>", self.text(k), self.text(v)) } else { String::new() };
                let mut ids: Vec<usize> = keys.iter().map(|x| x.0).collect();
                ids.sort_unstable();
                ids.dedup();
                let mut the_name = format!("{text}{{");
                for (i, (_, tx, _)) in keys.iter().enumerate() {
                    if i > 0 {
                        the_name.push_str(", ");
                    }
                    the_name.push_str(&format!("{tx}: ()"));
                }
                the_name.push('}');
                let tt = crate::ops::non_nullable(t, self.m.table);
                let kind = Kind::MapPattern { chaves: keys.iter().map(|x| (x.0, x.1.clone())).collect() };
                let s = self.base(tt, kind, Restr::Mapa { k, v, chaves: ids }, the_name, false);
                let mut ad = Vec::new();
                for (id, text, q) in keys {
                    let ch = Key::Mapa { id, texto: text };
                    let pt = self.the_additional(s, &ch).unwrap_or(OBJ_NULLABLE);
                    let es = self.pattern(e, path.plus(ch.clone()), pt, q, false);
                    ad.push((ch, es));
                }
                Space::the_with(path, s, Vec::new(), ad)
            }
            PatternKind::Constant(_) => match e.the_default_values.get(&p).cloned() {
                Some(v) => self.constant_value(&v, path),
                None => {
                    self.invalid = true;
                    let d = self.unknown();
                    Space::novo(path, d)
                }
            },
        }
    }

    /// `createCastSpace` (`fe76:shared.dart:570-632`).
    fn cast(&mut self, e: &Entry<'_>, path: Path, context: St, t: TypeId, sub: PatternId, non_null: bool) -> Space {
        let mut space = self.pattern(e, path.clone(), context, sub, non_null);
        let ct = self.static_type(t);
        if e.version_3_3 {
            if self.contained_in(ct, &space) {
                space = Space::novo(path.clone(), context);
            }
            if !self.potentially_nullable(t) {
                let n = Space::novo(path, NULL);
                space = self.union(&space, &n);
            }
        } else if self.sub(ct, context) && self.the_sealed(context) {
            for s in self.expanded_sealed(context) {
                if !self.sub(ct, s) && !self.sub(s, ct) {
                    let n = Space::novo(path.clone(), s);
                    space = self.union(&space, &n);
                }
            }
        }
        space
    }

    fn potentially_nullable(&mut self, t: TypeId) -> bool {
        let n = self.m.core.null;
        self.sub_dart(n, t) || match self.m.table.get(t).clone() {
            Type::TypeParameter { param, .. } => {
                let b = self.m.table.param(param).bound;
                self.sub_dart(n, b)
            }
            _ => false,
        }
    }

    /// `expandSealedSubtypes`.
    fn expanded_sealed(&mut self, t: St) -> Vec<St> {
        if !self.the_sealed(t) {
            return vec![t];
        }
        let mut r = Vec::new();
        for s in self.subtypes(t, &[]) {
            for x in self.expanded_sealed(s) {
                if !r.contains(&x) {
                    r.push(x);
                }
            }
        }
        r
    }

    fn unrestricted(&mut self, s: &Simple) -> bool {
        for (k, e) in s.0.props.clone() {
            let ft = self.fields(s.0.ty, &k).unwrap_or(NEVER);
            if !self.contained_in(ft, &e) {
                return false;
            }
        }
        for (k, e) in s.0.additional.clone() {
            let ft = self.the_additional(s.0.ty, &k).unwrap_or(NEVER);
            if !self.contained_in(ft, &e) {
                return false;
            }
        }
        true
    }

    /// `_isContainedIn`.
    fn contained_in(&mut self, t: St, e: &Space) -> bool {
        if e.0.simple.len() == 1 {
            let s = e.0.simple[0].clone();
            if self.unrestricted(&s) && self.sub(t, s.0.ty) {
                return true;
            }
        }
        for sub in self.expanded_sealed(t) {
            let mut found = false;
            for s in e.0.simple.clone() {
                if self.unrestricted(&s) && self.sub(sub, s.0.ty) {
                    found = true;
                    break;
                }
            }
            if !found {
                return false;
            }
        }
        true
    }

    /// `_createSpaceIntersection`.
    fn intersection(&mut self, path: Path, a: &Space, b: &Space) -> Space {
        let mut simple = Vec::new();
        let mut unknown = false;
        for x in a.0.simple.clone() {
            for y in b.0.simple.clone() {
                match self.simple_intersection(&path, &x, &y) {
                    Some(s) => simple.push(s),
                    None => unknown = true,
                }
            }
        }
        if unknown {
            let d = self.unknown();
            simple.push(Simple::new(d));
        }
        self.of_simple(path, simple)
    }

    fn simple_intersection(&mut self, path: &Path, a: &Simple, b: &Simple) -> Option<Simple> {
        let typ = if self.sub(a.0.ty, b.0.ty) {
            a.0.ty
        } else if self.sub(b.0.ty, a.0.ty) {
            b.0.ty
        } else {
            return None;
        };
        let mut props: Vec<(Key, Space)> = Vec::new();
        for (k, ea) in a.0.props.clone() {
            match b.prop(&k).cloned() {
                Some(eb) => {
                    let i = self.intersection(path.plus(k.clone()), &ea, &eb);
                    props.push((k, i));
                }
                None => props.push((k, ea)),
            }
        }
        for (k, eb) in &b.0.props {
            if !props.iter().any(|(c, _)| c == k) {
                props.push((k.clone(), eb.clone()));
            }
        }
        Some(Simple(Rc::new(SimpleD { ty: typ, props, additional: Vec::new() })))
    }

    // ---- constantes (`_convertConstantValue`) ----

    fn value_id(&mut self, v: &Valor) -> usize {
        for i in 0..self.values.len() {
            let w = self.values[i].clone();
            if self.m.iguais(v, &w) {
                return i;
            }
        }
        self.values.push(v.clone());
        self.values.len() - 1
    }

    /// `InstanceState.toString`.
    fn value_text(&self, v: &Valor) -> String {
        match &v.estado {
            Estado::Bool(Some(b)) => b.to_string(),
            Estado::Int(Some(i)) => i.to_string(),
            Estado::Double(Some(d)) => double_as_dart(*d),
            Estado::Str(Some(s)) => format!("'{}'", String::from_utf16_lossy(s)),
            Estado::Null { .. } => "null".to_string(),
            Estado::Simbolo(Some(s)) => format!("#{s}"),
            Estado::Tipo(Some(t)) => self.text(*t),
            Estado::Lista { elementos: elems, .. } | Estado::Set { elementos: elems, .. } => {
                let open = if matches!(v.estado, Estado::Lista { .. }) { '[' } else { '{' };
                let closes = if open == '[' { ']' } else { '}' };
                let parts: Vec<String> = elems.iter().map(|x| format!("{} ({})", self.text(x.tipo), self.value_text(x))).collect();
                format!("{open}{}{closes}", parts.join(", "))
            }
            Estado::Registro { posicionais: positional_args, nomeados: named_args } => {
                let mut parts: Vec<String> = positional_args.iter().map(|x| format!("{} ({})", self.text(x.tipo), self.value_text(x))).collect();
                let mut n: Vec<(String, String)> = named_args
                    .iter()
                    .map(|(s, x)| (self.m.interner.resolve(*s).to_string(), format!("{} ({})", self.text(x.tipo), self.value_text(x))))
                    .collect();
                n.sort();
                if !n.is_empty() {
                    let named_args: Vec<String> = n.into_iter().map(|(a, b)| format!("{a}: {b}")).collect();
                    parts.push(format!("{{{}}}", named_args.join(", ")));
                }
                format!("({})", parts.join(", "))
            }
            _ => "-unknown-".to_string(),
        }
    }

    fn constant_value(&mut self, v: &Valor, path: Path) -> Space {
        if v.estado.unknown() {
            // An opaque value (from a library that was not inferred): the analyzer
            // knows it; without it, the whole check is skipped.
            self.invalid = true;
            let d = self.unknown();
            return Space::novo(path, d);
        }
        match &v.estado {
            Estado::Null { .. } => return Space::novo(path, NULL),
            // Type literal: the identity depends on the alias and on
            // normalization (`const (Alias)`); it stays an unknown space, which
            // covers nothing but itself (never over-reports).
            Estado::Tipo(_) => {
                let d = self.unknown();
                return Space::novo(path, d);
            }
            Estado::Bool(Some(b)) => {
                let bt = self.m.core.bool_;
                let base = self.static_type(bt);
                let subs = self.subtypes(base, &[]);
                return Space::novo(path, if *b { subs[0] } else { subs[1] });
            }
            Estado::Registro { posicionais: positional_args, nomeados: named_args } => {
                let mut props = Vec::new();
                for (i, x) in positional_args.iter().enumerate() {
                    let k = Key::Index(i);
                    let e = self.constant_value(x, path.plus(k.clone()));
                    props.push((k, e));
                }
                for (n, x) in named_args.iter() {
                    let k = Key::Name { nome: self.m.interner.resolve(*n).into(), registro: true };
                    let e = self.constant_value(x, path.plus(k.clone()));
                    props.push((k, e));
                }
                let t = self.static_type(v.tipo);
                return Space::the_with(path, t, props, Vec::new());
            }
            _ => {}
        }
        if let Type::Interface { class, .. } = self.m.table.get(v.tipo).clone() {
            if self.m.program.class(class).kind == ClassKind::Enum {
                if let Estado::Generico { campos: the_fields, .. } = &v.estado {
                    let index = the_fields.iter().find_map(|(c, x)| match (c, &x.estado) {
                        (super::valor::Campo::Indice, Estado::Int(Some(i))) => Some(*i as usize),
                        _ => None,
                    });
                    if let Some(var) = index.and_then(|i| self.m.program.class(class).enum_constants.get(i).copied()) {
                        let el = self.enum_element(class, var);
                        return Space::novo(path, el);
                    }
                }
            }
        }
        let lib = self.lib;
        let s = if self.m.primitive_equality(v, lib) {
            let id = self.value_id(v);
            let the_name = self.value_text(v);
            let nn = crate::ops::non_nullable(v.tipo, self.m.table);
            let s = self.base(nn, Kind::General, Restr::Id(Ident::Value(id)), the_name, false);
            if self.dart_nullable(v.tipo) {
                self.nullable(s)
            } else {
                s
            }
        } else {
            self.unknown()
        };
        Space::novo(path, s)
    }

    // ---- witnesses (`fe76:witness.dart`) ----

    fn build_witness(&self, preds: &[Predicate]) -> Witness {
        let mut root = Witness { estatico: OBJ_NULLABLE, valor: OBJ_NULLABLE, props: Vec::new() };
        for p in preds {
            let mut here = &mut root;
            for k in p.caminho.list() {
                let i = match here.props.iter().position(|(c, _)| *c == k) {
                    Some(i) => i,
                    None => {
                        here.props.push((k, Witness { estatico: OBJ_NULLABLE, valor: OBJ_NULLABLE, props: Vec::new() }));
                        here.props.len() - 1
                    }
                };
                here = &mut here.props[i].1;
            }
            here.estatico = p.the_static;
            here.valor = p.value;
        }
        root
    }

    fn trivial(&mut self, w: &Witness) -> bool {
        if !self.sub(w.estatico, w.valor) {
            return false;
        }
        w.props.iter().all(|(_, p)| self.trivial(p))
    }

    /// `PropertyWitness.witnessToDart`.
    fn write_witness(&mut self, buf: &mut String, w: &Witness, fix: bool) {
        if !w.props.is_empty() {
            let mut groups: Vec<(St, Vec<(Key, &Witness)>)> = Vec::new();
            for (k, p) in &w.props {
                if fix && self.trivial(p) {
                    continue;
                }
                let t = match k {
                    Key::Extension { recv, .. } => *recv,
                    _ => w.valor,
                };
                match groups.iter_mut().find(|g| g.0 == t) {
                    Some(g) => g.1.push((k.clone(), p)),
                    None => groups.push((t, vec![(k.clone(), p)])),
                }
            }
            if !groups.is_empty() {
                let mut e = "";
                for (t, the_fields) in groups {
                    buf.push_str(e);
                    e = " && ";
                    self.type_to_dart(buf, t, &the_fields, fix);
                }
                return;
            }
        }
        let v = w.valor;
        self.type_to_dart(buf, v, &[], fix);
    }

    /// `StaticType.typeToDart`.
    fn write_type(&self, buf: &mut String, s: St) {
        match &self.types[s] {
            _ if s == NULL => buf.push_str("Null"),
            // `writeGeneralType` of the base types.
            Type_::Base { ty, .. } if self.mark => {
                buf.push(MARK);
                buf.push_str(&format!("T{}", ty.0));
                buf.push(MARK);
            }
            Type_::Nullable(u) => {
                self.write_type(buf, *u);
                if !self.implicitly_nullable(*u) {
                    buf.push('?');
                }
            }
            Type_::Wrapped(w, _) => self.write_type(buf, *w),
            _ => buf.push_str(&self.nome(s)),
        }
    }

    fn extra_fields(&mut self, buf: &mut String, the_fields: &[(Key, &Witness)], fix: bool, skip: impl Fn(&Key) -> bool) {
        let mut start = " && Object(";
        let mut end = "";
        let mut comma = "";
        for (k, w) in the_fields {
            if skip(k) {
                continue;
            }
            buf.push_str(start);
            start = "";
            end = ")";
            buf.push_str(comma);
            comma = ", ";
            buf.push_str(&k.name());
            buf.push_str(": ");
            self.write_witness(buf, w, fix);
        }
        buf.push_str(end);
    }

    /// `StaticType.witnessToDart`.
    fn type_to_dart(&mut self, buf: &mut String, s: St, the_fields: &[(Key, &Witness)], fix: bool) {
        match self.types[s].clone() {
            Type_::Wrapped(w, _) => self.type_to_dart(buf, w, the_fields, fix),
            Type_::Base { ty, especie: kind, .. } => match kind {
                Kind::Registro => {
                    buf.push('(');
                    let mut comma = "";
                    let (pos, named_args): (usize, Vec<String>) = match self.m.table.get(ty).clone() {
                        Type::Record { positional, named, .. } => {
                            let mut n: Vec<String> = named.iter().map(|(s, _)| self.m.interner.resolve(*s).to_string()).collect();
                            n.sort();
                            (positional.len(), n)
                        }
                        _ => (0, Vec::new()),
                    };
                    for i in 0..pos {
                        buf.push_str(comma);
                        comma = ", ";
                        match the_fields.iter().find(|(k, _)| *k == Key::Index(i)) {
                            Some((_, w)) => self.write_witness(buf, w, fix),
                            None => buf.push('_'),
                        }
                    }
                    for n in named_args {
                        buf.push_str(comma);
                        comma = ", ";
                        buf.push_str(&n);
                        buf.push_str(": ");
                        let k = Key::Name { nome: n.as_str().into(), registro: true };
                        match the_fields.iter().find(|(c, _)| *c == k) {
                            Some((_, w)) => self.write_witness(buf, w, fix),
                            None => buf.push('_'),
                        }
                    }
                    buf.push(')');
                    self.extra_fields(buf, the_fields, fix, |k| k.of_record());
                }
                Kind::BoolValue | Kind::EnumElement | Kind::General => {
                    match (&self.types[s], self.mark) {
                        (Type_::Base { especie: Kind::EnumElement, the_restr: Restr::Id(Ident::Enum(v)), .. }, true) => {
                            buf.push(MARK);
                            buf.push_str(&format!("E{}", v.0));
                            buf.push(MARK);
                        }
                        // `writeGeneralConstantValue`: the witness cannot be
                        // represented.
                        (Type_::Base { especie: Kind::General, .. }, true) => {
                            buf.push(MARK);
                            buf.push('X');
                            buf.push(MARK);
                        }
                        _ => buf.push_str(&self.nome(s)),
                    }
                    self.extra_fields(buf, the_fields, fix, |k| k.of_record());
                }
                Kind::ListPattern => {
                    let Type_::Base { the_restr: Restr::Lista { tam, resto: has_rest, .. }, .. } = self.types[s].clone() else { return };
                    let (mut cab, mut cau) = (0usize, 0usize);
                    let mut rest: Option<&Witness> = None;
                    for (k, w) in the_fields {
                        match k {
                            Key::Head(i) if *i >= cab => cab = i + 1,
                            // `key.index >= maxHeadSize` (sic, `fe76:types/list.dart:93`).
                            Key::Tail(i) if *i >= cab => cau = i + 1,
                            Key::RestPart(..) => rest = Some(*w),
                            _ => {}
                        }
                    }
                    if cab + cau < tam {
                        cab = tam - cau;
                    }
                    buf.push('[');
                    let mut comma = "";
                    for i in 0..cab {
                        buf.push_str(comma);
                        match the_fields.iter().find(|(k, _)| *k == Key::Head(i)) {
                            Some((_, w)) => self.write_witness(buf, w, fix),
                            None => buf.push('_'),
                        }
                        comma = ", ";
                    }
                    if has_rest {
                        buf.push_str(comma);
                        buf.push_str("...");
                        if let Some(w) = rest {
                            self.write_witness(buf, w, fix);
                        }
                        comma = ", ";
                    }
                    for i in (0..cau).rev() {
                        buf.push_str(comma);
                        match the_fields.iter().find(|(k, _)| *k == Key::Tail(i)) {
                            Some((_, w)) => self.write_witness(buf, w, fix),
                            None => buf.push('_'),
                        }
                        comma = ", ";
                    }
                    buf.push(']');
                    self.extra_fields(buf, the_fields, fix, |k| k.of_list());
                }
                Kind::MapPattern { chaves: keys } => {
                    buf.push('{');
                    let mut comma = "";
                    for (id, text) in keys {
                        buf.push_str(comma);
                        buf.push_str(&text);
                        buf.push_str(": ");
                        let k = Key::Mapa { id, texto: text.clone() };
                        match the_fields.iter().find(|(c, _)| *c == k) {
                            Some((_, w)) => self.write_witness(buf, w, fix),
                            None => buf.push('_'),
                        }
                        comma = ", ";
                    }
                    buf.push('}');
                    self.extra_fields(buf, the_fields, fix, |k| matches!(k, Key::Mapa { .. }));
                }
                _ => {
                    if !self.simple_name(ty) {
                        buf.push_str(&self.nome(s));
                        buf.push_str(" _");
                        self.extra_fields(buf, the_fields, fix, |k| k.of_list());
                    } else {
                        self.base_for_dart(buf, s, the_fields, fix);
                    }
                }
            },
            _ => self.base_for_dart(buf, s, the_fields, fix),
        }
    }

    /// `_BaseStaticType.witnessToDart`.
    fn base_for_dart(&mut self, buf: &mut String, s: St, the_fields: &[(Key, &Witness)], fix: bool) {
        if s == OBJ_NULLABLE && the_fields.is_empty() {
            buf.push('_');
        } else if s == NULL && the_fields.is_empty() {
            buf.push_str("null");
        } else {
            self.write_type(buf, s);
            buf.push('(');
            let mut comma = "";
            for (k, w) in the_fields {
                buf.push_str(comma);
                comma = ", ";
                buf.push_str(&k.name());
                buf.push_str(": ");
                self.write_witness(buf, w, fix);
            }
            buf.push(')');
        }
    }

    /// `isAlwaysExhaustive` (`an611:src/dart/element/type_system.dart:832-874`).
    pub(crate) fn always_exhaustive(&mut self, t: TypeId, prof: u32) -> bool {
        if prof > 32 {
            return false;
        }
        match self.m.table.get(t).clone() {
            Type::Null => true,
            Type::Interface { class, .. } => {
                let c = self.m.program.class(class);
                c.kind == ClassKind::Enum || (c.kind == ClassKind::Class && c.modifiers.sealed) || Some(class) == self.m.core.bool_class
            }
            Type::ExtensionType { .. } => {
                let a = self.m.apagar(t);
                a != t && self.always_exhaustive(a, prof + 1)
            }
            Type::FutureOr { arg, .. } => self.always_exhaustive(arg, prof + 1),
            Type::Intersection { param, bound } => {
                self.always_exhaustive(bound, prof + 1) || {
                    let b = self.m.table.param(param).bound;
                    self.m.table.param(param).explicito && self.always_exhaustive(b, prof + 1)
                }
            }
            Type::TypeParameter { param, .. } => {
                let b = self.m.table.param(param).bound;
                self.m.table.param(param).explicito && self.always_exhaustive(b, prof + 1)
            }
            Type::Record { positional, named, .. } => {
                let all: Vec<TypeId> = positional.iter().copied().chain(named.iter().map(|(_, c)| *c)).collect();
                all.into_iter().all(|c| self.always_exhaustive(c, prof + 1))
            }
            _ => false,
        }
    }

    /// `_validateSwitchExhaustiveness` without the report: the unreachable cases
    /// and, if not exhaustive, the first witness. `None` when there is an
    /// invalid type (the analyzer does not check).
    pub(crate) fn verify(&mut self, e: &Entry<'_>, scrutinee: TypeId, cases: &[Case]) -> Option<Result_> {
        self.invalid = false;
        let value = self.static_type(scrutinee);
        let mut spaces = Vec::new();
        let mut stored = Vec::new();
        let mut indices = Vec::new();
        for (i, c) in cases.iter().enumerate() {
            let Some(p) = c.pattern else { continue };
            let es = self.pattern(e, Path::default(), value, p, false);
            spaces.push(es);
            stored.push(c.stored);
            indices.push(i);
        }
        if self.invalid {
            return None;
        }
        let (unreach, not_) = self.compute(value, &stored, &spaces);
        let mut r = Result_ { unreachable: unreach.into_iter().map(|i| indices[i]).collect(), witness: None, fix: None, parts: Vec::new() };
        if let Some(ts) = &not_
            && self.collect_parts
        {
            // `witness.toDart(buffer, forCorrection: true)` of each one.
            self.mark = true;
            for t in ts.iter() {
                let w = self.build_witness(t);
                let mut b = String::new();
                self.write_witness(&mut b, &w, true);
                if let Some(p) = text_parts(self.m, &b) {
                    r.parts.push(p);
                }
            }
            self.mark = false;
        }
        if let Some(ts) = not_ {
            if let Some(the_first) = ts.first() {
                let w = self.build_witness(the_first);
                let mut a = String::new();
                self.write_witness(&mut a, &w, false);
                let mut b = String::new();
                self.write_witness(&mut b, &w, true);
                r.witness = Some(a);
                r.fix = Some(b);
            }
        }
        Some(r)
    }
}

fn list_text(text: &str, tam: usize, rest: bool) -> String {
    let mut s = format!("{text}[");
    let mut comma = "";
    for _ in 0..tam {
        s.push_str(comma);
        s.push_str("()");
        comma = ", ";
    }
    if rest {
        s.push_str(comma);
        s.push_str("...");
    }
    s.push(']');
    s
}

/// `effectiveName` of a field without a written name (`:x`): the variable's.
fn implicit_name(a: &ast::Ast, p: PatternId) -> Option<dartforge_intern::SymbolId> {
    match &a.pattern(p).kind {
        PatternKind::Variable { name, .. } => Some(name.sym),
        PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) => implicit_name(a, *x),
        PatternKind::Cast { pattern, .. } => implicit_name(a, *pattern),
        _ => None,
    }
}
