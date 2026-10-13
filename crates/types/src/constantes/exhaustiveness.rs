//! Exaustividade de `switch` como o analyzer 6.11: o algoritmo de espaços
//! do `_fe_analyzer_shared-76.0.0` (`lib/src/exhaustiveness/`, citado como
//! `fe76:`) com a cola do analyzer (`an611:src/generated/exhaustiveness.dart`)
//! e o `_validateSwitchExhaustiveness` do `ConstantVerifier`
//! (`an611:src/dart/constant/constant_verifier.dart:883-1000`).
//!
//! Os tipos estáticos do algoritmo (`StaticType`) vivem numa arena com
//! internação pela igualdade do Dart (`TypeBasedStaticType ==` compara o
//! tipo e a restrição; `NullableStaticType ==` o subjacente), então a
//! igualdade é a do índice. Os espaços (`Space`) comparam por identidade,
//! como no Dart (a classe não define `==`), e por isso são `Rc`.

use super::avaliador::Motor;
use super::valor::{double_as_dart, Estado, Valor};
use crate::table::{Type, TypeId, TypeParamId};
use dartforge_elements::model::{ClassId, ClassKind, FunctionKind, LibraryId, VariableId};
use dartforge_frontend::ast::{self, ExprId, ListPatternElement, PatternId, PatternKind};
use std::collections::HashMap;
use std::rc::Rc;

// -- Tipos estáticos (`fe76:static_type.dart`, `fe76:types.dart`) ------------

type St = usize;
const OBJ_NULLABLE: St = 0;
const OBJ: St = 1;
const NULL: St = 2;
const NEVER: St = 3;

/// Identidade de uma `IdentityRestriction`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Ident {
    Bool(bool),
    Enum(VariableId),
    /// Valor constante de igualdade primitiva (índice em `Exaustividade::valores`).
    Valor(usize),
    /// `getUnknownStaticType`: único, não casa nem é casado por nada.
    Unique(usize),
}

/// `Restriction` (a parte da igualdade de um `TypeBasedStaticType`).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum Restr {
    Livre,
    Id(Ident),
    /// `ListTypeRestriction` (o texto dos argumentos não entra na igualdade).
    Lista { el: TypeId, tam: usize, resto: bool },
    /// `MapTypeRestriction`: as chaves pelos valores (índices ordenados).
    Mapa { k: TypeId, v: TypeId, chaves: Vec<usize> },
}

impl Restr {
    fn free(&self) -> bool {
        match self {
            Restr::Livre => true,
            Restr::Id(_) => false,
            Restr::Lista { tam, resto: rest, .. } => *rest && *tam == 0,
            Restr::Mapa { chaves: keys, .. } => keys.is_empty(),
        }
    }
}

#[derive(Clone, Debug)]
enum Especie {
    Common,
    Bool,
    Enum(ClassId),
    Sealed(ClassId),
    FutureOr { arg: St, fut: St },
    Registro,
    ListType,
    /// `ListPatternStaticType` (o nome já tem o texto dos argumentos).
    ListPattern,
    /// `MapPatternStaticType`: as chaves na ordem escrita, com o texto.
    MapPattern { chaves: Vec<(usize, String)> },
    BoolValue,
    EnumElement,
    /// `GeneralValueStaticType` (constante comum ou o desconhecido `?`).
    General,
}

#[derive(Clone, Debug)]
enum Tipo {
    /// `NullableStaticType(u)`; `OBJ_ANUL` e `NULL` são deste formato.
    Anulavel(St),
    Objeto,
    Never,
    Base { ty: TypeId, especie: Especie, the_restr: Restr, nome: String, implicitly_nullable: bool },
    Wrapped(St, St),
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum TypeKey {
    Anulavel(St),
    Base(TypeId, Restr),
    Wrapped(St, St),
}

// -- Chaves (`fe76:key.dart`) --------------------------------------------------

#[derive(Clone, Debug)]
enum Chave {
    Mapa { id: usize, texto: String },
    Cabeca(usize),
    Cauda(usize),
    RestPart(usize, usize),
    /// `NameKey`; `registro` é `RecordNameKey`.
    Nome { nome: Rc<str>, registro: bool },
    /// `RecordIndexKey` (nome `$i+1`).
    Indice(usize),
    Extensao { recv: St, nome: Rc<str>, tipo: St },
}

impl Chave {
    fn nome(&self) -> String {
        match self {
            Chave::Mapa { texto: text, .. } => format!("[{text}]"),
            Chave::Cabeca(i) => format!("[{i}]"),
            Chave::Cauda(i) => format!("[{}]", -(*i as i64 + 1)),
            Chave::RestPart(h, t) => format!("[{h}:{}]", -(*t as i64)),
            Chave::Nome { nome: the_name, .. } | Chave::Extensao { nome: the_name, .. } => the_name.to_string(),
            Chave::Indice(i) => format!("${}", i + 1),
        }
    }

    fn of_record(&self) -> bool {
        matches!(self, Chave::Indice(_) | Chave::Nome { registro: true, .. })
    }

    fn de_lista(&self) -> bool {
        matches!(self, Chave::Cabeca(_) | Chave::Cauda(_) | Chave::RestPart(..))
    }

    /// A posição na ordem total de `compareTo`.
    fn grupo(&self) -> u8 {
        match self {
            Chave::Cabeca(_) => 0,
            Chave::RestPart(..) => 1,
            Chave::Cauda(_) => 2,
            Chave::Mapa { .. } => 3,
            Chave::Indice(_) => 4,
            Chave::Nome { .. } => 5,
            Chave::Extensao { .. } => 6,
        }
    }
}

impl PartialEq for Chave {
    fn eq(&self, o: &Chave) -> bool {
        match (self, o) {
            (Chave::Mapa { id: a, .. }, Chave::Mapa { id: b, .. }) => a == b,
            (Chave::Cabeca(a), Chave::Cabeca(b)) | (Chave::Cauda(a), Chave::Cauda(b)) => a == b,
            (Chave::RestPart(a, b), Chave::RestPart(c, d)) => a == c && b == d,
            // `NameKey ==` só olha o nome: `RecordIndexKey(0)` é `NameKey('$1')`.
            (Chave::Nome { .. } | Chave::Indice(_), Chave::Nome { .. } | Chave::Indice(_)) => self.nome() == o.nome(),
            (Chave::Extensao { recv: a, nome: n, .. }, Chave::Extensao { recv: b, nome: m, .. }) => a == b && n == m,
            _ => false,
        }
    }
}

/// `Key.compareTo` (ordem só para escolher a testemunha de forma estável).
fn compare_keys(ex: &Exhaustiveness, a: &Chave, b: &Chave) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (ga, gb) = (a.grupo(), b.grupo());
    if ga != gb {
        return ga.cmp(&gb);
    }
    match (a, b) {
        (Chave::Cabeca(x), Chave::Cabeca(y)) => x.cmp(y),
        (Chave::Cauda(x), Chave::Cauda(y)) => y.cmp(x),
        (Chave::RestPart(h1, t1), Chave::RestPart(h2, t2)) => h1.cmp(h2).then(t2.cmp(t1)),
        (Chave::Mapa { texto: x, .. }, Chave::Mapa { texto: y, .. }) => x.cmp(y),
        (Chave::Indice(x), Chave::Indice(y)) => x.cmp(y),
        (Chave::Nome { nome: x, .. }, Chave::Nome { nome: y, .. }) => x.cmp(y),
        (Chave::Extensao { recv: r1, nome: n1, .. }, Chave::Extensao { recv: r2, nome: n2, .. }) => {
            ex.nome(*r1).cmp(&ex.nome(*r2)).then(n1.cmp(n2))
        }
        _ => Ordering::Equal,
    }
}

// -- Caminhos e espaços (`fe76:path.dart`, `fe76:space.dart`) -----------------

#[derive(Clone, Default)]
struct Path(Option<Rc<(Path, Chave)>>);

impl Path {
    fn plus(&self, k: Chave) -> Path {
        Path(Some(Rc::new((self.clone(), k))))
    }

    fn lista(&self) -> Vec<Chave> {
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
    tipo: St,
    props: Vec<(Chave, Espaco)>,
    additional: Vec<(Chave, Espaco)>,
}

#[derive(Clone)]
struct Simples(Rc<SimpleD>);

impl Simples {
    fn novo(typ: St) -> Simples {
        Simples(Rc::new(SimpleD { tipo: typ, props: Vec::new(), additional: Vec::new() }))
    }

    /// `SingleSpace ==`: o tipo e as propriedades (espaços por identidade).
    fn igual(&self, o: &Simples) -> bool {
        if Rc::ptr_eq(&self.0, &o.0) {
            return true;
        }
        if self.0.tipo != o.0.tipo || self.0.props.len() != o.0.props.len() {
            return false;
        }
        self.0.props.iter().all(|(k, v)| o.0.props.iter().any(|(k2, v2)| k == k2 && Rc::ptr_eq(&v.0, &v2.0)))
    }

    fn prop(&self, k: &Chave) -> Option<&Espaco> {
        self.0.props.iter().find(|(c, _)| c == k).map(|(_, e)| e)
    }

    fn the_additional(&self, k: &Chave) -> Option<&Espaco> {
        self.0.additional.iter().find(|(c, _)| c == k).map(|(_, e)| e)
    }
}

struct SpaceD {
    caminho: Path,
    simples: Vec<Simples>,
}

#[derive(Clone)]
struct Espaco(Rc<SpaceD>);

impl Espaco {
    fn novo(path: Path, typ: St) -> Espaco {
        Espaco(Rc::new(SpaceD { caminho: path, simples: vec![Simples::novo(typ)] }))
    }

    fn the_with(path: Path, typ: St, props: Vec<(Chave, Espaco)>, additional: Vec<(Chave, Espaco)>) -> Espaco {
        Espaco(Rc::new(SpaceD { caminho: path, simples: vec![Simples(Rc::new(SimpleD { tipo: typ, props, additional }))] }))
    }
}

/// Um predicado da testemunha (`fe76:witness.dart`).
#[derive(Clone)]
struct Predicate {
    caminho: Path,
    estatico: St,
    valor: St,
}

/// `PropertyWitness`.
struct Witness {
    estatico: St,
    valor: St,
    props: Vec<(Chave, Witness)>,
}

// -- O estado: tipos, valores, consultas ao sistema de tipos ------------------

/// O que a conversão de padrões lê da inferência e da avaliação.
pub(crate) struct Entrada<'x> {
    pub ast: &'x ast::Ast,
    pub source: &'x str,
    pub tipos_de_padroes: &'x HashMap<PatternId, TypeId>,
    pub extension_fields: &'x HashMap<PatternId, TypeId>,
    pub invalid_patterns: &'x std::collections::HashSet<PatternId>,
    pub the_default_values: &'x HashMap<PatternId, Valor>,
    pub key_values: &'x HashMap<ExprId, Valor>,
    /// A biblioteca tem a versão 3.3 ou maior (`createCastSpace`).
    pub version_3_3: bool,
}

/// Um caso do `switch` para a verificação.
pub(crate) struct Caso {
    /// `None`: `default`.
    pub padrao: Option<PatternId>,
    pub stored: bool,
}

/// Uma parte de testemunha (`MissingPatternPart` do analyzer): texto, valor
/// de enum (`writeReference(enum).nome`) ou tipo (`writeType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParteDeTestemunha {
    Texto(String),
    ValorDeEnum { enumeracao: ClassId, valor: VariableId },
    Tipo(TypeId),
}

/// Os marcadores das partes no texto da testemunha (`\u{1}` em volta).
const MARCA: char = '\u{1}';

/// O texto com marcadores em partes; `None` quando há constante geral (o
/// `isComplete = false` do `AnalyzerDartTemplateBuffer`).
fn text_parts(m: &Motor<'_>, s: &str) -> Option<Vec<ParteDeTestemunha>> {
    let mut v: Vec<ParteDeTestemunha> = Vec::new();
    let mut text = String::new();
    let mut rest = s;
    while let Some(i) = rest.find(MARCA) {
        text.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let j = after.find(MARCA)?;
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

/// O resultado de `_validateSwitchExhaustiveness`.
pub(crate) struct Result_ {
    /// Índices (em `casos`) dos casos inalcançáveis.
    pub unreachable: Vec<usize>,
    /// A primeira testemunha, quando não é exaustivo (sem correção).
    pub witness: Option<String>,
    /// A primeira testemunha para a correção.
    pub correcao: Option<String>,
    /// Todas as testemunhas com partes (só com `coletar_partes`).
    pub partes: Vec<Vec<ParteDeTestemunha>>,
}

pub(crate) struct Exhaustiveness<'m, 'a> {
    m: &'m mut Motor<'a>,
    tipos: Vec<Tipo>,
    internal: HashMap<TypeKey, St>,
    the_unique: usize,
    valores: Vec<Valor>,
    subtipos: HashMap<St, Vec<St>>,
    lib: LibraryId,
    invalid: bool,
    /// Escreve as testemunhas com marcadores de partes.
    marcar: bool,
    /// Calcula [`Resultado::partes`].
    pub(crate) collect_parts: bool,
}

impl<'m, 'a> Exhaustiveness<'m, 'a> {
    pub(crate) fn nova(m: &'m mut Motor<'a>, lib: LibraryId) -> Self {
        let types = vec![Tipo::Anulavel(OBJ), Tipo::Objeto, Tipo::Anulavel(NEVER), Tipo::Never];
        let mut internal = HashMap::new();
        internal.insert(TypeKey::Anulavel(OBJ), OBJ_NULLABLE);
        internal.insert(TypeKey::Anulavel(NEVER), NULL);
        Exhaustiveness { m, tipos: types, internal, the_unique: 0, valores: Vec::new(), subtipos: HashMap::new(), lib, invalid: false, marcar: false, collect_parts: false }
    }

    fn the_intern(&mut self, key: TypeKey, t: impl FnOnce() -> Tipo) -> St {
        if let Some(&s) = self.internal.get(&key) {
            return s;
        }
        let s = self.tipos.len();
        self.tipos.push(t());
        self.internal.insert(key, s);
        s
    }

    fn base(&mut self, ty: TypeId, kind: Especie, the_restr: Restr, the_name: String, impl_nullable: bool) -> St {
        let key = TypeKey::Base(ty, the_restr.clone());
        self.the_intern(key, || Tipo::Base { ty, especie: kind, the_restr, nome: the_name, implicitly_nullable: impl_nullable })
    }

    fn wrapped(&mut self, w: St, i: St) -> St {
        self.the_intern(TypeKey::Wrapped(w, i), || Tipo::Wrapped(w, i))
    }

    fn unico(&mut self, ty: TypeId, the_name: &str) -> St {
        self.the_unique += 1;
        let id = self.the_unique;
        self.base(ty, Especie::General, Restr::Id(Ident::Unique(id)), the_name.to_string(), false)
    }

    /// `getUnknownStaticType`.
    fn desconhecido(&mut self) -> St {
        let o = self.m.core.object;
        self.unico(o, "?")
    }

    // ---- consultas ao sistema de tipos (`AnalyzerTypeOperations`) ----

    fn sub_dart(&mut self, a: TypeId, b: TypeId) -> bool {
        self.m.sub(a, b)
    }

    /// `typeToString` (`DartType.toString`, sem alias).
    fn texto(&self, t: TypeId) -> String {
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

    /// `variancia`: 1 covariante, -1 contravariante, 0 invariante.
    fn substitute_parameters(&mut self, t: TypeId, variance: i8, prof: u32) -> TypeId {
        if prof > 16 {
            return t;
        }
        match self.m.table.get(t).clone() {
            Type::TypeParameter { param, .. } | Type::Intersection { param, .. } => {
                if variance == -1 {
                    self.m.core.never
                } else {
                    // `element.defaultType!` nulo: o 3.6.2 lança.
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

    /// `TypeParameterElement.defaultType` (instanciação para o limite).
    fn parameter_default(&mut self, p: TypeParamId) -> TypeId {
        crate::ops::instantiate_to_bounds(&[p], &[], self.m.table, self.m.core)[0]
    }

    // ---- `getStaticType` (`fe76:shared.dart:161-234`) ----

    fn tipo_estatico(&mut self, t: TypeId) -> St {
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
                let n = self.texto(nn);
                self.base(nn, Especie::Bool, Restr::Livre, n, false)
            }
            Type::Record { .. } => {
                let n = self.texto(nn);
                self.base(nn, Especie::Registro, Restr::Livre, n, false)
            }
            Type::FutureOr { arg, .. } => {
                let a = self.tipo_estatico(arg);
                let fut = match core.future_class {
                    Some(fc) => self.m.table.intern(Type::Interface { class: fc, args: vec![arg].into_boxed_slice(), nullable: false }),
                    None => core.object,
                };
                let f = self.tipo_estatico(fut);
                let n = self.texto(nn);
                let ia = self.dart_nullable(arg) || matches!(self.m.table.get(arg), Type::Null);
                self.base(nn, Especie::FutureOr { arg: a, fut: f }, Restr::Livre, n, ia)
            }
            Type::Interface { class, .. } if self.m.program.class(class).kind == ClassKind::Enum => {
                let n = self.texto(nn);
                self.base(nn, Especie::Enum(class), Restr::Livre, n, false)
            }
            Type::Interface { class, .. }
                if self.m.program.class(class).kind == ClassKind::Class && self.m.program.class(class).modifiers.sealed =>
            {
                let n = self.texto(nn);
                self.base(nn, Especie::Sealed(class), Restr::Livre, n, false)
            }
            _ => {
                if self.as_instance_of(nn, core.list_class).is_some() {
                    let n = self.texto(nn);
                    self.base(nn, Especie::ListType, Restr::Livre, n, false)
                } else {
                    let n = self.texto(nn);
                    let s = self.base(nn, Especie::Common, Restr::Livre, n, false);
                    let bnd = match self.m.table.get(t).clone() {
                        Type::TypeParameter { param, .. } => Some(self.m.table.param(param).bound),
                        Type::Intersection { bound, .. } => Some(bound),
                        _ => None,
                    };
                    match bnd {
                        Some(b) => {
                            let sb = self.tipo_estatico(b);
                            self.wrapped(sb, s)
                        }
                        None => s,
                    }
                }
            }
        };
        if self.dart_nullable(t) {
            st = self.anulavel(st);
        }
        st
    }

    // ---- operações de `StaticType` ----

    fn anulavel(&mut self, s: St) -> St {
        match self.tipos[s].clone() {
            Tipo::Never => NULL,
            Tipo::Objeto => OBJ_NULLABLE,
            Tipo::Anulavel(_) => s,
            Tipo::Base { .. } => self.the_intern(TypeKey::Anulavel(s), || Tipo::Anulavel(s)),
            Tipo::Wrapped(w, i) => {
                let (nw, ni) = (self.anulavel(w), self.anulavel(i));
                if nw == w && ni == i {
                    s
                } else {
                    self.wrapped(nw, ni)
                }
            }
        }
    }

    fn non_null(&mut self, s: St) -> St {
        match self.tipos[s].clone() {
            Tipo::Anulavel(u) => u,
            Tipo::Wrapped(w, i) => {
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
        match &self.tipos[s] {
            Tipo::Anulavel(_) => s != NULL,
            Tipo::Base { especie: kind, .. } => matches!(
                kind,
                Especie::Bool | Especie::Enum(_) | Especie::Sealed(_) | Especie::FutureOr { .. } | Especie::ListType
            ),
            Tipo::Wrapped(w, _) => self.the_sealed(*w),
            _ => false,
        }
    }

    fn implicitly_nullable(&self, s: St) -> bool {
        match &self.tipos[s] {
            Tipo::Anulavel(_) => true,
            Tipo::Objeto | Tipo::Never => false,
            Tipo::Base { implicitly_nullable, .. } => *implicitly_nullable,
            Tipo::Wrapped(w, _) => self.implicitly_nullable(*w),
        }
    }

    fn nome(&self, s: St) -> String {
        match &self.tipos[s] {
            _ if s == NULL => "Null".to_string(),
            Tipo::Anulavel(u) => {
                if self.implicitly_nullable(*u) {
                    self.nome(*u)
                } else {
                    format!("{}?", self.nome(*u))
                }
            }
            Tipo::Objeto => "Object".to_string(),
            Tipo::Never => "Never".to_string(),
            Tipo::Base { nome: the_name, .. } => the_name.clone(),
            Tipo::Wrapped(w, _) => self.nome(*w),
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
        if let Tipo::Wrapped(w, i) = self.tipos[b].clone() {
            if self.sub(a, w) && self.sub(a, i) {
                return true;
            }
        }
        match self.tipos[a].clone() {
            Tipo::Objeto => false,
            Tipo::Never => true,
            Tipo::Anulavel(u) => match self.tipos[b].clone() {
                Tipo::Anulavel(u2) => self.sub(u, u2),
                _ => false,
            },
            Tipo::Wrapped(w, i) => self.sub(w, b) || self.sub(i, b),
            Tipo::Base { ty, the_restr, .. } => {
                if b == OBJ {
                    return true;
                }
                match self.tipos[b].clone() {
                    Tipo::Anulavel(u2) => self.sub(a, u2),
                    Tipo::Base { ty: ty2, the_restr: r2, .. } => self.sub_dart(ty, ty2) && self.sub_constraint(&the_restr, &r2),
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
            (Restr::Livre, _) => false,
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
    fn subtipos(&mut self, s: St, keys: &[Chave]) -> Vec<St> {
        match self.tipos[s].clone() {
            Tipo::Anulavel(u) => {
                if s == NULL {
                    Vec::new()
                } else {
                    vec![u, NULL]
                }
            }
            Tipo::Wrapped(w, i) => {
                if let (Tipo::Anulavel(wu), Tipo::Anulavel(iu)) = (self.tipos[w].clone(), self.tipos[i].clone()) {
                    let e = self.wrapped(wu, iu);
                    return vec![e, NULL];
                }
                let subs = self.subtipos(w, keys);
                subs.into_iter().map(|x| self.wrapped(x, i)).collect()
            }
            Tipo::Base { ty, especie: kind, .. } => match kind {
                Especie::Bool => {
                    let t = self.base(ty, Especie::BoolValue, Restr::Id(Ident::Bool(true)), "true".into(), false);
                    let f = self.base(ty, Especie::BoolValue, Restr::Id(Ident::Bool(false)), "false".into(), false);
                    vec![t, f]
                }
                Especie::FutureOr { arg, fut } => vec![arg, fut],
                Especie::Enum(c) => {
                    if let Some(v) = self.subtipos.get(&s) {
                        return v.clone();
                    }
                    let v = self.enum_elements(s, ty, c);
                    self.subtipos.insert(s, v.clone());
                    v
                }
                Especie::Sealed(c) => {
                    if let Some(v) = self.subtipos.get(&s) {
                        return v.clone();
                    }
                    let v = self.sealed_subclasses(s, ty, c);
                    self.subtipos.insert(s, v.clone());
                    v
                }
                Especie::ListType => self.list_subtypes(ty, keys),
                _ => Vec::new(),
            },
            _ => Vec::new(),
        }
    }

    /// `EnumStaticType._createEnumElements` (`fe76:types/enum.dart`).
    fn enum_elements(&mut self, s: St, ty: TypeId, c: ClassId) -> Vec<St> {
        let constantes = self.m.program.class(c).enum_constants.clone();
        // `overapproximate(_type)` por elemento (`enum.dart:118-124`): nenhum
        // sem elementos.
        let over = if constantes.is_empty() { ty } else { self.overapproximate(ty) };
        let mut v = Vec::new();
        for var in constantes {
            let el = self.enum_element(c, var);
            let Tipo::Base { ty: tel, .. } = self.tipos[el].clone() else { continue };
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
        self.base(ty, Especie::EnumElement, Restr::Id(Ident::Enum(var)), the_name, false)
    }

    /// `AnalyzerSealedClassOperations.getDirectSubclasses` e
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
            // A cadeia sintética de `extends B with M` leva à superclasse escrita.
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
        // `overapproximate(_type)` só para a subclasse não genérica
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
            let e = self.tipo_estatico(st);
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
    fn list_subtypes(&mut self, ty: TypeId, keys: &[Chave]) -> Vec<St> {
        let (mut cab, mut cau) = (0usize, 0usize);
        for k in keys {
            match k {
                Chave::Cabeca(i) if *i >= cab => cab = i + 1,
                Chave::Cauda(i) if *i >= cau => cau = i + 1,
                _ => {}
            }
        }
        let max = cab + cau;
        let el = self.as_instance_of(ty, self.m.core.list_class).and_then(|l| self.args_of(l).first().copied()).unwrap_or(self.m.core.dynamic_);
        let text = if matches!(self.m.table.get(el), Type::Dynamic) { String::new() } else { format!("<{}>", self.texto(el)) };
        let mut v = Vec::new();
        for tam in 0..=max {
            let rest = tam == max;
            let the_name = list_text(&text, tam, rest);
            v.push(self.base(ty, Especie::ListPattern, Restr::Lista { el, tam, resto: rest }, the_name, false));
        }
        v
    }

    // ---- propriedades ----

    /// O tipo (Dart) do membro de instância `nome` de `t`
    /// (`_getInterfaceFieldTypes`, `an611:src/generated/exhaustiveness.dart:389-417`).
    fn membro(&mut self, t: TypeId, the_name: &str) -> Option<TypeId> {
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
                return self.membro(o, the_name);
            }
            _ => {
                let o = self.m.core.object;
                if t == o {
                    return None;
                }
                return self.membro(o, the_name);
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
        // O mais específico (o sobrescritor covariante).
        let mut best = *candidates.first()?;
        for &c in &candidates[1..] {
            if c != best && self.sub_dart(c, best) && !self.sub_dart(best, c) {
                best = c;
            }
        }
        Some(best)
    }

    /// `getObjectFieldType`.
    fn object_field(&mut self, k: &Chave) -> Option<St> {
        if !matches!(k, Chave::Nome { .. } | Chave::Indice(_)) {
            return None;
        }
        let o = self.m.core.object;
        let t = self.membro(o, &k.nome())?;
        Some(self.tipo_estatico(t))
    }

    /// `fields[key]` de um `TypeBasedStaticType` (`getFieldTypes`).
    fn type_field(&mut self, ty: TypeId, k: &Chave) -> Option<St> {
        if !matches!(k, Chave::Nome { .. } | Chave::Indice(_)) {
            return None;
        }
        let t = self.membro(ty, &k.nome())?;
        Some(self.tipo_estatico(t))
    }

    /// `StaticType.fields[key]`.
    fn campos(&mut self, s: St, k: &Chave) -> Option<St> {
        match self.tipos[s].clone() {
            Tipo::Base { ty, .. } => self.type_field(ty, k),
            Tipo::Wrapped(w, _) => self.campos(w, k),
            _ => None,
        }
    }

    /// `getPropertyType`.
    fn propriedade(&mut self, s: St, k: &Chave) -> Option<St> {
        match self.tipos[s].clone() {
            Tipo::Base { ty, .. } => self.type_field(ty, k),
            Tipo::Wrapped(w, _) => self.propriedade(w, k),
            _ => self.object_field(k),
        }
    }

    /// `getAdditionalPropertyType`.
    fn the_additional(&mut self, s: St, k: &Chave) -> Option<St> {
        let Tipo::Base { ty, .. } = self.tipos[s].clone() else { return None };
        let core = self.m.core;
        match k {
            Chave::Mapa { .. } => {
                let m = self.as_instance_of(ty, core.map_class)?;
                let v = *self.args_of(m).get(1)?;
                Some(self.tipo_estatico(v))
            }
            Chave::Cabeca(_) | Chave::Cauda(_) => {
                let l = self.as_instance_of(ty, core.list_class)?;
                let e = *self.args_of(l).first()?;
                Some(self.tipo_estatico(e))
            }
            Chave::RestPart(..) => {
                let l = self.as_instance_of(ty, core.list_class)?;
                Some(self.tipo_estatico(l))
            }
            _ => self.object_field(k),
        }
    }

    // ---- o algoritmo (`fe76:exhaustive.dart`) ----

    fn unmatched(&mut self, lines: &[Vec<Espaco>], values: &[Espaco], preds: &[Predicate], several: bool) -> Option<Vec<Vec<Predicate>>> {
        if values.is_empty() {
            if !lines.is_empty() {
                return None;
            }
            return Some(vec![preds.to_vec()]);
        }
        let first = values[0].clone();
        let mut interesting: Vec<Chave> = Vec::new();
        for l in lines {
            for s in &l[0].0.simples {
                for (k, _) in &s.0.additional {
                    if !interesting.contains(k) {
                        interesting.push(k.clone());
                    }
                }
            }
        }
        for value in first.0.simples.clone() {
            let context = value.0.tipo;
            let mut stack = std::collections::VecDeque::from([context]);
            let mut witnesses: Option<Vec<Vec<Predicate>>> = None;
            while let Some(t) = stack.pop_front() {
                if self.sub(t, NEVER) {
                    continue;
                }
                if self.the_sealed(t) {
                    let r = self.filter_by_type(context, t, lines, &value, values, preds, &first.0.caminho, false);
                    if r.is_some() {
                        let subs = self.subtipos(t, &interesting);
                        stack.extend(subs);
                    }
                } else {
                    let r = self.filter_by_type(context, t, lines, &value, values, preds, &first.0.caminho, false);
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
        lines: &[Vec<Espaco>],
        value: &Simples,
        values: &[Espaco],
        preds: &[Predicate],
        path: &Path,
        several: bool,
    ) -> Option<Vec<Vec<Predicate>>> {
        let mut extended = preds.to_vec();
        extended.push(Predicate { caminho: path.clone(), estatico: context, valor: t });
        let mut first_: Vec<Simples> = Vec::new();
        let mut remaining: Vec<Vec<Espaco>> = Vec::new();
        for l in lines {
            for s in &l[0].0.simples {
                if self.sub(t, s.0.tipo) {
                    first_.push(s.clone());
                    remaining.push(l.clone());
                }
            }
        }
        let mut keys: Vec<Chave> = Vec::new();
        let mut additional: Vec<Chave> = Vec::new();
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
            let mut nl = self.expand(&keys, &additional, s, s.0.tipo, path);
            nl.extend(l[1..].iter().cloned());
            new_lines.push(nl);
        }
        self.unmatched(&new_lines, &new_values, &extended, several)
    }

    fn expand(&mut self, keys: &[Chave], additional: &[Chave], s: &Simples, t: St, path: &Path) -> Vec<Espaco> {
        let mut r = Vec::new();
        for k in keys {
            if let Some(e) = s.prop(k) {
                r.push(e.clone());
            } else {
                let mut pt = self.propriedade(t, k);
                if pt.is_none() {
                    if let Chave::Extensao { tipo: typ, .. } = k {
                        pt = Some(*typ);
                    }
                }
                r.push(Espaco::novo(path.plus(k.clone()), pt.unwrap_or(OBJ_NULLABLE)));
            }
        }
        for k in additional {
            if let Some(e) = s.the_additional(k) {
                r.push(e.clone());
            } else {
                let pt = self.the_additional(t, k);
                r.push(Espaco::novo(path.plus(k.clone()), pt.unwrap_or(OBJ_NULLABLE)));
            }
        }
        r
    }

    /// `computeExhaustiveness`.
    fn calcular(&mut self, value: St, stored: &[bool], spaces: &[Espaco]) -> (Vec<usize>, Option<Vec<Vec<Predicate>>>) {
        let mut lines: Vec<Vec<Espaco>> = Vec::new();
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
        let root = Espaco::novo(Path::default(), value);
        let t = self.unmatched(&lines, &[root], &[], true);
        (unreachable, t)
    }

    // ---- espaços dos padrões (`SpaceCreator`, `PatternConverter`) ----

    fn com_contexto(&mut self, context: St, t: TypeId, non_null: bool) -> St {
        let mut s = self.tipo_estatico(t);
        if self.sub(context, s) {
            s = context;
        }
        if non_null {
            s = self.non_null(s);
        }
        s
    }

    fn uniao(&mut self, a: &Espaco, b: &Espaco) -> Espaco {
        let mut all = a.0.simples.clone();
        all.extend(b.0.simples.iter().cloned());
        self.of_simple(a.0.caminho.clone(), all)
    }

    /// `Space.fromSingleSpaces`.
    fn of_simple(&mut self, path: Path, simple: Vec<Simples>) -> Espaco {
        let empty = Simples::novo(NEVER);
        let mut set: Vec<Simples> = Vec::new();
        for s in simple {
            if s.igual(&empty) {
                continue;
            }
            if !set.iter().any(|x| x.igual(&s)) {
                set.push(s);
            }
        }
        if set.is_empty() {
            set.push(empty);
        } else if set.len() == 2 {
            let (a, b) = (set[0].clone(), set[1].clone());
            if a.0.tipo == NULL && a.0.props.is_empty() && b.0.props.is_empty() {
                let n = self.anulavel(b.0.tipo);
                set = vec![Simples::novo(n)];
            } else if b.0.tipo == NULL && b.0.props.is_empty() && a.0.props.is_empty() {
                let n = self.anulavel(a.0.tipo);
                set = vec![Simples::novo(n)];
            }
        }
        Espaco(Rc::new(SpaceD { caminho: path, simples: set }))
    }

    fn padrao(&mut self, e: &Entrada<'_>, path: Path, context: St, p: PatternId, non_null: bool) -> Espaco {
        if e.invalid_patterns.contains(&p) {
            self.invalid = true;
        }
        match &e.ast.pattern(p).kind {
            PatternKind::Variable { final_, var_, ty, .. } => {
                let Some(&t) = e.tipos_de_padroes.get(&p) else {
                    // `case nome:` é um padrão constante: o valor da
                    // constante de topo, se o verificador o achou; senão a
                    // verificação fica de fora, como com um tipo inválido.
                    let _ = (final_, var_, ty);
                    if let Some(v) = e.the_default_values.get(&p).cloned() {
                        return self.constant_value(&v, path);
                    }
                    self.invalid = true;
                    let d = self.desconhecido();
                    return Espaco::novo(path, d);
                };
                let s = self.com_contexto(context, t, non_null);
                Espaco::novo(path, s)
            }
            PatternKind::Wildcard { ty } => {
                let s = match ty.and_then(|_| e.tipos_de_padroes.get(&p).copied()) {
                    None => {
                        if non_null {
                            self.non_null(context)
                        } else {
                            context
                        }
                    }
                    Some(t) => self.com_contexto(context, t, non_null),
                };
                Espaco::novo(path, s)
            }
            PatternKind::Object { fields, .. } => {
                let Some(&t) = e.tipos_de_padroes.get(&p) else {
                    self.invalid = true;
                    let d = self.desconhecido();
                    return Espaco::novo(path, d);
                };
                let s = self.com_contexto(context, t, non_null);
                let mut the_fields: Vec<(Rc<str>, PatternId, Option<TypeId>)> = Vec::new();
                for f in fields.iter() {
                    let the_name = match f.name {
                        Some(n) => Some(n.sym),
                        None => nome_implicito(e.ast, f.pattern),
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
                let mut props: Vec<(Chave, Espaco)> = Vec::new();
                for (the_name, sub, ext) in the_fields {
                    let (key, pt) = match ext {
                        Some(et) => {
                            let pt = self.tipo_estatico(et);
                            let recv = self.tipo_estatico(t);
                            (Chave::Extensao { recv, nome: the_name, tipo: pt }, pt)
                        }
                        None => {
                            let k = Chave::Nome { nome: the_name, registro: false };
                            let pt = self.propriedade(s, &k).unwrap_or(OBJ_NULLABLE);
                            (k, pt)
                        }
                    };
                    let es = self.padrao(e, path.plus(key.clone()), pt, sub, false);
                    props.push((key, es));
                }
                Espaco::the_with(path, s, props, Vec::new())
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
                        let the_name = f.name.map(|n| n.sym).or_else(|| nome_implicito(e.ast, f.pattern));
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
                let s = self.com_contexto(context, rt, true);
                let mut props = Vec::new();
                for (i, sub) in pos.into_iter().enumerate() {
                    let k = Chave::Indice(i);
                    let pt = self.propriedade(s, &k).unwrap_or(OBJ_NULLABLE);
                    let es = self.padrao(e, path.plus(k.clone()), pt, sub, false);
                    props.push((k, es));
                }
                for (the_name, sub) in named_args {
                    let k = Chave::Nome { nome: the_name, registro: true };
                    let pt = self.propriedade(s, &k).unwrap_or(OBJ_NULLABLE);
                    let es = self.padrao(e, path.plus(k.clone()), pt, sub, false);
                    props.push((k, es));
                }
                Espaco::the_with(path, s, props, Vec::new())
            }
            PatternKind::Or(x, y) => {
                let (x, y) = (*x, *y);
                let a = self.padrao(e, path.clone(), context, x, non_null);
                let b = self.padrao(e, path, context, y, non_null);
                self.uniao(&a, &b)
            }
            PatternKind::And(x, y) => {
                let (x, y) = (*x, *y);
                let a = self.padrao(e, path.clone(), context, x, non_null);
                let b = self.padrao(e, path.clone(), context, y, non_null);
                self.intersection(path, &a, &b)
            }
            PatternKind::NullCheck(x) => self.padrao(e, path, context, *x, true),
            PatternKind::Parenthesized(x) => self.padrao(e, path, context, *x, non_null),
            PatternKind::NullAssert(x) => {
                let s = self.padrao(e, path.clone(), context, *x, true);
                let n = Espaco::novo(path, NULL);
                self.uniao(&s, &n)
            }
            PatternKind::Cast { pattern, .. } => {
                let sub = *pattern;
                let Some(&t) = e.tipos_de_padroes.get(&p) else {
                    self.invalid = true;
                    let d = self.desconhecido();
                    return Espaco::novo(path, d);
                };
                self.cast(e, path, context, t, sub, non_null)
            }
            PatternKind::Relational { .. } => {
                let d = self.desconhecido();
                Espaco::novo(path, d)
            }
            PatternKind::List { type_args, elements } => {
                let Some(&t) = e.tipos_de_padroes.get(&p) else {
                    self.invalid = true;
                    let d = self.desconhecido();
                    return Espaco::novo(path, d);
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
                let text = if type_args.is_empty() { String::new() } else { format!("<{}>", self.texto(el)) };
                let (nc, nt) = (head.len(), tail.len());
                let has_rest = rest.is_some();
                let the_name = list_text(&text, nc + nt, has_rest);
                let tt = crate::ops::non_nullable(t, self.m.table);
                let mut s = self.base(tt, Especie::ListPattern, Restr::Lista { el, tam: nc + nt, resto: has_rest }, the_name, false);
                if self.dart_nullable(t) {
                    s = self.anulavel(s);
                }
                let mut ad = Vec::new();
                for (i, q) in head.into_iter().enumerate() {
                    let k = Chave::Cabeca(i);
                    let pt = self.the_additional(s, &k).unwrap_or(OBJ_NULLABLE);
                    let es = self.padrao(e, path.plus(k.clone()), pt, q, false);
                    ad.push((k, es));
                }
                if let Some(r) = rest {
                    let k = Chave::RestPart(nc, nt);
                    let pt = self.the_additional(s, &k).unwrap_or(OBJ_NULLABLE);
                    let es = match r {
                        Some(q) => self.padrao(e, path.plus(k.clone()), pt, q, false),
                        None => Espaco::novo(path.plus(k.clone()), pt),
                    };
                    ad.push((k, es));
                }
                for i in 0..nt {
                    let k = Chave::Cauda(i);
                    let pt = self.the_additional(s, &k).unwrap_or(OBJ_NULLABLE);
                    let es = self.padrao(e, path.plus(k.clone()), pt, tail[nt - i - 1], false);
                    ad.push((k, es));
                }
                Espaco::the_with(path, s, Vec::new(), ad)
            }
            PatternKind::Map { type_args, entries, .. } => {
                let Some(&t) = e.tipos_de_padroes.get(&p) else {
                    self.invalid = true;
                    let d = self.desconhecido();
                    return Espaco::novo(path, d);
                };
                let a = self.args_of(t);
                let (k, v) = (a.first().copied().unwrap_or(self.m.core.dynamic_), a.get(1).copied().unwrap_or(self.m.core.dynamic_));
                let mut keys: Vec<(usize, String, PatternId)> = Vec::new();
                for en in entries.iter() {
                    let Some(val) = e.key_values.get(&en.key).cloned() else {
                        let d = self.desconhecido();
                        return Espaco::novo(path, d);
                    };
                    let id = self.value_id(&val);
                    let text = self.value_text(&val);
                    if let Some(x) = keys.iter_mut().find(|x| x.0 == id) {
                        x.2 = en.value;
                    } else {
                        keys.push((id, text, en.value));
                    }
                }
                let text = if type_args.len() == 2 { format!("<{}, {}>", self.texto(k), self.texto(v)) } else { String::new() };
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
                let kind = Especie::MapPattern { chaves: keys.iter().map(|x| (x.0, x.1.clone())).collect() };
                let s = self.base(tt, kind, Restr::Mapa { k, v, chaves: ids }, the_name, false);
                let mut ad = Vec::new();
                for (id, text, q) in keys {
                    let ch = Chave::Mapa { id, texto: text };
                    let pt = self.the_additional(s, &ch).unwrap_or(OBJ_NULLABLE);
                    let es = self.padrao(e, path.plus(ch.clone()), pt, q, false);
                    ad.push((ch, es));
                }
                Espaco::the_with(path, s, Vec::new(), ad)
            }
            PatternKind::Constant(_) => match e.the_default_values.get(&p).cloned() {
                Some(v) => self.constant_value(&v, path),
                None => {
                    self.invalid = true;
                    let d = self.desconhecido();
                    Espaco::novo(path, d)
                }
            },
        }
    }

    /// `createCastSpace` (`fe76:shared.dart:570-632`).
    fn cast(&mut self, e: &Entrada<'_>, path: Path, context: St, t: TypeId, sub: PatternId, non_null: bool) -> Espaco {
        let mut space = self.padrao(e, path.clone(), context, sub, non_null);
        let ct = self.tipo_estatico(t);
        if e.version_3_3 {
            if self.contained_in(ct, &space) {
                space = Espaco::novo(path.clone(), context);
            }
            if !self.potentially_nullable(t) {
                let n = Espaco::novo(path, NULL);
                space = self.uniao(&space, &n);
            }
        } else if self.sub(ct, context) && self.the_sealed(context) {
            for s in self.expanded_sealed(context) {
                if !self.sub(ct, s) && !self.sub(s, ct) {
                    let n = Espaco::novo(path.clone(), s);
                    space = self.uniao(&space, &n);
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
        for s in self.subtipos(t, &[]) {
            for x in self.expanded_sealed(s) {
                if !r.contains(&x) {
                    r.push(x);
                }
            }
        }
        r
    }

    fn unrestricted(&mut self, s: &Simples) -> bool {
        for (k, e) in s.0.props.clone() {
            let ft = self.campos(s.0.tipo, &k).unwrap_or(NEVER);
            if !self.contained_in(ft, &e) {
                return false;
            }
        }
        for (k, e) in s.0.additional.clone() {
            let ft = self.the_additional(s.0.tipo, &k).unwrap_or(NEVER);
            if !self.contained_in(ft, &e) {
                return false;
            }
        }
        true
    }

    /// `_isContainedIn`.
    fn contained_in(&mut self, t: St, e: &Espaco) -> bool {
        if e.0.simples.len() == 1 {
            let s = e.0.simples[0].clone();
            if self.unrestricted(&s) && self.sub(t, s.0.tipo) {
                return true;
            }
        }
        for sub in self.expanded_sealed(t) {
            let mut found = false;
            for s in e.0.simples.clone() {
                if self.unrestricted(&s) && self.sub(sub, s.0.tipo) {
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
    fn intersection(&mut self, path: Path, a: &Espaco, b: &Espaco) -> Espaco {
        let mut simple = Vec::new();
        let mut unknown = false;
        for x in a.0.simples.clone() {
            for y in b.0.simples.clone() {
                match self.simple_intersection(&path, &x, &y) {
                    Some(s) => simple.push(s),
                    None => unknown = true,
                }
            }
        }
        if unknown {
            let d = self.desconhecido();
            simple.push(Simples::novo(d));
        }
        self.of_simple(path, simple)
    }

    fn simple_intersection(&mut self, path: &Path, a: &Simples, b: &Simples) -> Option<Simples> {
        let typ = if self.sub(a.0.tipo, b.0.tipo) {
            a.0.tipo
        } else if self.sub(b.0.tipo, a.0.tipo) {
            b.0.tipo
        } else {
            return None;
        };
        let mut props: Vec<(Chave, Espaco)> = Vec::new();
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
        Some(Simples(Rc::new(SimpleD { tipo: typ, props, additional: Vec::new() })))
    }

    // ---- constantes (`_convertConstantValue`) ----

    fn value_id(&mut self, v: &Valor) -> usize {
        for i in 0..self.valores.len() {
            let w = self.valores[i].clone();
            if self.m.iguais(v, &w) {
                return i;
            }
        }
        self.valores.push(v.clone());
        self.valores.len() - 1
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
            Estado::Tipo(Some(t)) => self.texto(*t),
            Estado::Lista { elementos: elems, .. } | Estado::Conjunto { elementos: elems, .. } => {
                let open = if matches!(v.estado, Estado::Lista { .. }) { '[' } else { '{' };
                let closes = if open == '[' { ']' } else { '}' };
                let parts: Vec<String> = elems.iter().map(|x| format!("{} ({})", self.texto(x.tipo), self.value_text(x))).collect();
                format!("{open}{}{closes}", parts.join(", "))
            }
            Estado::Registro { posicionais: positional_args, nomeados: named_args } => {
                let mut parts: Vec<String> = positional_args.iter().map(|x| format!("{} ({})", self.texto(x.tipo), self.value_text(x))).collect();
                let mut n: Vec<(String, String)> = named_args
                    .iter()
                    .map(|(s, x)| (self.m.interner.resolve(*s).to_string(), format!("{} ({})", self.texto(x.tipo), self.value_text(x))))
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

    fn constant_value(&mut self, v: &Valor, path: Path) -> Espaco {
        if v.estado.desconhecido() {
            // Valor opaco (de biblioteca não inferida): o analyzer o conhece;
            // sem ele, a verificação inteira fica de fora.
            self.invalid = true;
            let d = self.desconhecido();
            return Espaco::novo(path, d);
        }
        match &v.estado {
            Estado::Null { .. } => return Espaco::novo(path, NULL),
            // Literal de tipo: a identidade depende do alias e da
            // normalização (`const (Alias)`); fica como espaço desconhecido,
            // que não cobre nada além de si (nunca relata a mais).
            Estado::Tipo(_) => {
                let d = self.desconhecido();
                return Espaco::novo(path, d);
            }
            Estado::Bool(Some(b)) => {
                let bt = self.m.core.bool_;
                let base = self.tipo_estatico(bt);
                let subs = self.subtipos(base, &[]);
                return Espaco::novo(path, if *b { subs[0] } else { subs[1] });
            }
            Estado::Registro { posicionais: positional_args, nomeados: named_args } => {
                let mut props = Vec::new();
                for (i, x) in positional_args.iter().enumerate() {
                    let k = Chave::Indice(i);
                    let e = self.constant_value(x, path.plus(k.clone()));
                    props.push((k, e));
                }
                for (n, x) in named_args.iter() {
                    let k = Chave::Nome { nome: self.m.interner.resolve(*n).into(), registro: true };
                    let e = self.constant_value(x, path.plus(k.clone()));
                    props.push((k, e));
                }
                let t = self.tipo_estatico(v.tipo);
                return Espaco::the_with(path, t, props, Vec::new());
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
                        return Espaco::novo(path, el);
                    }
                }
            }
        }
        let lib = self.lib;
        let s = if self.m.primitive_equality(v, lib) {
            let id = self.value_id(v);
            let the_name = self.value_text(v);
            let nn = crate::ops::non_nullable(v.tipo, self.m.table);
            let s = self.base(nn, Especie::General, Restr::Id(Ident::Valor(id)), the_name, false);
            if self.dart_nullable(v.tipo) {
                self.anulavel(s)
            } else {
                s
            }
        } else {
            self.desconhecido()
        };
        Espaco::novo(path, s)
    }

    // ---- testemunhas (`fe76:witness.dart`) ----

    fn build_witness(&self, preds: &[Predicate]) -> Witness {
        let mut root = Witness { estatico: OBJ_NULLABLE, valor: OBJ_NULLABLE, props: Vec::new() };
        for p in preds {
            let mut here = &mut root;
            for k in p.caminho.lista() {
                let i = match here.props.iter().position(|(c, _)| *c == k) {
                    Some(i) => i,
                    None => {
                        here.props.push((k, Witness { estatico: OBJ_NULLABLE, valor: OBJ_NULLABLE, props: Vec::new() }));
                        here.props.len() - 1
                    }
                };
                here = &mut here.props[i].1;
            }
            here.estatico = p.estatico;
            here.valor = p.valor;
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
            let mut groups: Vec<(St, Vec<(Chave, &Witness)>)> = Vec::new();
            for (k, p) in &w.props {
                if fix && self.trivial(p) {
                    continue;
                }
                let t = match k {
                    Chave::Extensao { recv, .. } => *recv,
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
    fn escrever_tipo(&self, buf: &mut String, s: St) {
        match &self.tipos[s] {
            _ if s == NULL => buf.push_str("Null"),
            // `writeGeneralType` dos tipos de base.
            Tipo::Base { ty, .. } if self.marcar => {
                buf.push(MARCA);
                buf.push_str(&format!("T{}", ty.0));
                buf.push(MARCA);
            }
            Tipo::Anulavel(u) => {
                self.escrever_tipo(buf, *u);
                if !self.implicitly_nullable(*u) {
                    buf.push('?');
                }
            }
            Tipo::Wrapped(w, _) => self.escrever_tipo(buf, *w),
            _ => buf.push_str(&self.nome(s)),
        }
    }

    fn extra_fields(&mut self, buf: &mut String, the_fields: &[(Chave, &Witness)], fix: bool, skip: impl Fn(&Chave) -> bool) {
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
            buf.push_str(&k.nome());
            buf.push_str(": ");
            self.write_witness(buf, w, fix);
        }
        buf.push_str(end);
    }

    /// `StaticType.witnessToDart`.
    fn type_to_dart(&mut self, buf: &mut String, s: St, the_fields: &[(Chave, &Witness)], fix: bool) {
        match self.tipos[s].clone() {
            Tipo::Wrapped(w, _) => self.type_to_dart(buf, w, the_fields, fix),
            Tipo::Base { ty, especie: kind, .. } => match kind {
                Especie::Registro => {
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
                        match the_fields.iter().find(|(k, _)| *k == Chave::Indice(i)) {
                            Some((_, w)) => self.write_witness(buf, w, fix),
                            None => buf.push('_'),
                        }
                    }
                    for n in named_args {
                        buf.push_str(comma);
                        comma = ", ";
                        buf.push_str(&n);
                        buf.push_str(": ");
                        let k = Chave::Nome { nome: n.as_str().into(), registro: true };
                        match the_fields.iter().find(|(c, _)| *c == k) {
                            Some((_, w)) => self.write_witness(buf, w, fix),
                            None => buf.push('_'),
                        }
                    }
                    buf.push(')');
                    self.extra_fields(buf, the_fields, fix, |k| k.of_record());
                }
                Especie::BoolValue | Especie::EnumElement | Especie::General => {
                    match (&self.tipos[s], self.marcar) {
                        (Tipo::Base { especie: Especie::EnumElement, the_restr: Restr::Id(Ident::Enum(v)), .. }, true) => {
                            buf.push(MARCA);
                            buf.push_str(&format!("E{}", v.0));
                            buf.push(MARCA);
                        }
                        // `writeGeneralConstantValue`: a testemunha não é
                        // representável.
                        (Tipo::Base { especie: Especie::General, .. }, true) => {
                            buf.push(MARCA);
                            buf.push('X');
                            buf.push(MARCA);
                        }
                        _ => buf.push_str(&self.nome(s)),
                    }
                    self.extra_fields(buf, the_fields, fix, |k| k.of_record());
                }
                Especie::ListPattern => {
                    let Tipo::Base { the_restr: Restr::Lista { tam, resto: has_rest, .. }, .. } = self.tipos[s].clone() else { return };
                    let (mut cab, mut cau) = (0usize, 0usize);
                    let mut rest: Option<&Witness> = None;
                    for (k, w) in the_fields {
                        match k {
                            Chave::Cabeca(i) if *i >= cab => cab = i + 1,
                            // `key.index >= maxHeadSize` (sic, `fe76:types/list.dart:93`).
                            Chave::Cauda(i) if *i >= cab => cau = i + 1,
                            Chave::RestPart(..) => rest = Some(*w),
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
                        match the_fields.iter().find(|(k, _)| *k == Chave::Cabeca(i)) {
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
                        match the_fields.iter().find(|(k, _)| *k == Chave::Cauda(i)) {
                            Some((_, w)) => self.write_witness(buf, w, fix),
                            None => buf.push('_'),
                        }
                        comma = ", ";
                    }
                    buf.push(']');
                    self.extra_fields(buf, the_fields, fix, |k| k.de_lista());
                }
                Especie::MapPattern { chaves: keys } => {
                    buf.push('{');
                    let mut comma = "";
                    for (id, text) in keys {
                        buf.push_str(comma);
                        buf.push_str(&text);
                        buf.push_str(": ");
                        let k = Chave::Mapa { id, texto: text.clone() };
                        match the_fields.iter().find(|(c, _)| *c == k) {
                            Some((_, w)) => self.write_witness(buf, w, fix),
                            None => buf.push('_'),
                        }
                        comma = ", ";
                    }
                    buf.push('}');
                    self.extra_fields(buf, the_fields, fix, |k| matches!(k, Chave::Mapa { .. }));
                }
                _ => {
                    if !self.simple_name(ty) {
                        buf.push_str(&self.nome(s));
                        buf.push_str(" _");
                        self.extra_fields(buf, the_fields, fix, |k| k.de_lista());
                    } else {
                        self.base_for_dart(buf, s, the_fields, fix);
                    }
                }
            },
            _ => self.base_for_dart(buf, s, the_fields, fix),
        }
    }

    /// `_BaseStaticType.witnessToDart`.
    fn base_for_dart(&mut self, buf: &mut String, s: St, the_fields: &[(Chave, &Witness)], fix: bool) {
        if s == OBJ_NULLABLE && the_fields.is_empty() {
            buf.push('_');
        } else if s == NULL && the_fields.is_empty() {
            buf.push_str("null");
        } else {
            self.escrever_tipo(buf, s);
            buf.push('(');
            let mut comma = "";
            for (k, w) in the_fields {
                buf.push_str(comma);
                comma = ", ";
                buf.push_str(&k.nome());
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

    /// `_validateSwitchExhaustiveness` sem o relato: os casos inalcançáveis
    /// e, se não é exaustivo, a primeira testemunha. `None` quando há tipo
    /// inválido (o analyzer não verifica).
    pub(crate) fn verificar(&mut self, e: &Entrada<'_>, scrutinee: TypeId, cases: &[Caso]) -> Option<Result_> {
        self.invalid = false;
        let value = self.tipo_estatico(scrutinee);
        let mut spaces = Vec::new();
        let mut stored = Vec::new();
        let mut indices = Vec::new();
        for (i, c) in cases.iter().enumerate() {
            let Some(p) = c.padrao else { continue };
            let es = self.padrao(e, Path::default(), value, p, false);
            spaces.push(es);
            stored.push(c.stored);
            indices.push(i);
        }
        if self.invalid {
            return None;
        }
        let (unreach, not_) = self.calcular(value, &stored, &spaces);
        let mut r = Result_ { unreachable: unreach.into_iter().map(|i| indices[i]).collect(), witness: None, correcao: None, partes: Vec::new() };
        if let Some(ts) = &not_
            && self.collect_parts
        {
            // `witness.toDart(buffer, forCorrection: true)` de cada uma.
            self.marcar = true;
            for t in ts.iter() {
                let w = self.build_witness(t);
                let mut b = String::new();
                self.write_witness(&mut b, &w, true);
                if let Some(p) = text_parts(self.m, &b) {
                    r.partes.push(p);
                }
            }
            self.marcar = false;
        }
        if let Some(ts) = not_ {
            if let Some(the_first) = ts.first() {
                let w = self.build_witness(the_first);
                let mut a = String::new();
                self.write_witness(&mut a, &w, false);
                let mut b = String::new();
                self.write_witness(&mut b, &w, true);
                r.witness = Some(a);
                r.correcao = Some(b);
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

/// `effectiveName` de um campo sem nome escrito (`:x`): o da variável.
fn nome_implicito(a: &ast::Ast, p: PatternId) -> Option<dartforge_intern::SymbolId> {
    match &a.pattern(p).kind {
        PatternKind::Variable { name, .. } => Some(name.sym),
        PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) => nome_implicito(a, *x),
        PatternKind::Cast { pattern, .. } => nome_implicito(a, *pattern),
        _ => None,
    }
}
