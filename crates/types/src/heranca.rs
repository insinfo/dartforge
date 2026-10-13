//! O `InheritanceManager3` do analyzer 3.6.2
//! (`analyzer/lib/src/dart/element/inheritance_manager3.dart`) e o
//! `NNBD_TOP_MERGE` (`top_merge.dart`) que ele usa: a interface de cada
//! classe, mixin, enum e tipo de extensão, com os mesmos mapas do original:
//!
//! * `map`: a assinatura de cada nome na interface (as declaradas, e as
//!   herdadas pela assinatura mais específica, com o `topMerge`);
//! * `declared`, `implemented` (as implementações concretas, com os
//!   encaminhadores de `noSuchMethod` e a covariância herdada);
//! * `overridden` (os candidatos de cada nome nos supertipos diretos, na
//!   ordem: superclasse, mixins, interfaces), `redeclared` (tipos de
//!   extensão), `superImplemented` (`[S, S&M1, S&M1&M2]`) e os conflitos.
//!
//! Os nomes são os `Name` do analyzer: o privado é qualificado pela
//! biblioteca. A chave do texto é a do modelo de elementos (setter `x_=`).
//!
//! O tipo de cada membro (`ExecutableElement.type`) e a subtipagem vêm do
//! [`Provedor`]: a inferência fornece os tipos de campos inferidos sob
//! demanda; as fases depois dela usam [`ProvedorDoOutline`].
//! Escrito sem compilar nem executar (2026-10-05).

use crate::ops;
use crate::resolve::{ClassTypeData, OutlineTypes};
use crate::subtyping::{SubtypeEnv, is_subtype};
use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeParamOwner, TypeTable, Variance};
use dartforge_elements::model::{ClassId, ClassKind, FunctionElementId, FunctionKind, FunctionRef, LibraryId, Program, VariableRef};
use dartforge_frontend::ast::{self, MemberKind};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// `Name`: a chave do membro e, se privado, a biblioteca.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Nome {
    pub chave: SymbolId,
    pub biblioteca: Option<LibraryId>,
}

impl Nome {
    /// `Name(libraryUri, name)`.
    pub fn novo(interner: &Interner, library: LibraryId, key: SymbolId) -> Nome {
        let private = interner.resolve(key).starts_with('_');
        Nome { chave: key, biblioteca: private.then_some(library) }
    }

    /// `forSetter`.
    pub fn de_setter(self, interner: &Interner) -> Option<Nome> {
        let text = interner.resolve(self.chave);
        let key = interner.lookup(&format!("{text}_="))?;
        Some(Nome { chave: key, biblioteca: self.biblioteca })
    }

    /// `forGetter`.
    pub fn of_getter(self, interner: &Interner) -> Option<Nome> {
        let text = interner.resolve(self.chave);
        match text.strip_suffix("_=") {
            Some(base) => Some(Nome { chave: interner.lookup(base)?, biblioteca: self.biblioteca }),
            None => Some(self),
        }
    }
}

/// `ElementKind` de um membro.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Especie {
    Metodo,
    Getter,
    Setter,
}

/// `_ParameterDesc`: o parâmetro nomeado pelo nome, o posicional pela
/// posição.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Param {
    Indice(usize),
    Nome(SymbolId),
}

/// Um membro da interface (`ExecutableElement`, já com a substituição do
/// supertipo de onde veio).
#[derive(Clone, Debug)]
pub struct Membro {
    /// O elemento declarado; no sintético (`topMerge`, covariância herdada),
    /// o primeiro dos candidatos (o `prototype`).
    pub funcao: FunctionElementId,
    /// `enclosingElement3`: a classe que declara; no sintético, a classe da
    /// interface.
    pub classe: ClassId,
    /// `type`, nos parâmetros de tipo da classe cuja interface o contém.
    pub tipo: TypeId,
    pub especie: Especie,
    pub the_abstract_: bool,
    /// Os parâmetros covariantes (`isCovariant`: escrito ou herdado).
    pub the_covariant: Rc<[Param]>,
    pub sintetico: bool,
}

/// `Conflict`.
#[derive(Clone, Debug)]
pub enum Conflict {
    /// `CandidatesConflict`.
    Candidates { nome: Nome, candidatos: Vec<Membro> },
    /// `GetterMethodConflict`.
    GetterMethod { nome: Nome, getter: Membro, metodo: Membro },
    /// `HasNonExtensionAndExtensionMemberConflict`.
    ExtensionAndNonExtension { nome: Nome, non_extension: Vec<Membro>, extensao: Vec<Membro> },
    /// `NotUniqueExtensionMemberConflict`.
    NonUniqueExtension { nome: Nome, candidatos: Vec<Membro> },
}

/// Um mapa na ordem de inserção (o `LinkedHashMap` do Dart: reatribuir
/// mantém a posição).
#[derive(Clone, Debug)]
pub struct Mapa<V> {
    entradas: Vec<(Nome, V)>,
    indice: HashMap<Nome, usize>,
}

impl<V> Default for Mapa<V> {
    fn default() -> Self {
        Mapa { entradas: Vec::new(), indice: HashMap::new() }
    }
}

impl<V> Mapa<V> {
    pub fn get(&self, n: &Nome) -> Option<&V> {
        self.indice.get(n).map(|&i| &self.entradas[i].1)
    }

    pub fn get_mut(&mut self, n: &Nome) -> Option<&mut V> {
        match self.indice.get(n) {
            Some(&i) => Some(&mut self.entradas[i].1),
            None => None,
        }
    }

    pub fn contains_key(&self, n: &Nome) -> bool {
        self.indice.contains_key(n)
    }

    pub fn insert(&mut self, n: Nome, v: V) {
        match self.indice.get(&n) {
            Some(&i) => self.entradas[i].1 = v,
            None => {
                self.indice.insert(n, self.entradas.len());
                self.entradas.push((n, v));
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Nome, &V)> {
        self.entradas.iter().map(|(n, v)| (n, v))
    }

    pub fn len(&self) -> usize {
        self.entradas.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entradas.is_empty()
    }
}

/// `Interface`.
#[derive(Clone, Debug, Default)]
pub struct Interface {
    pub map: Mapa<Membro>,
    pub declared: Mapa<Membro>,
    pub implemented: Mapa<Membro>,
    /// `noSuchMethodForwarders`.
    pub encaminhados: HashSet<Nome>,
    pub overridden: Mapa<Vec<Membro>>,
    pub redeclared: Mapa<Vec<Membro>>,
    pub super_implemented: Vec<Mapa<Membro>>,
    pub conflicts: Vec<Conflict>,
}

impl Interface {
    /// `Interface._empty`.
    fn vazia() -> Interface {
        Interface { super_implemented: vec![Mapa::default()], ..Interface::default() }
    }

    /// `isSuperImplemented`.
    pub fn super_implementado(&self, n: &Nome) -> bool {
        self.super_implemented.last().is_some_and(|m| m.contains_key(n))
    }
}

/// O que a herança precisa do resto do analisador.
pub trait Provedor<'p> {
    fn programa(&self) -> &'p Program;
    fn interner(&self) -> &'p Interner;
    fn core(&self) -> &'p CoreTypes;
    fn tabela(&mut self) -> &mut TypeTable;
    fn class_data(&self, c: ClassId) -> ClassTypeData;
    /// `ExecutableElement.type` do membro `f`, nos parâmetros de tipo da
    /// classe que o declara.
    fn tipo_do_membro(&mut self, f: FunctionElementId) -> TypeId;
    fn sub(&mut self, a: TypeId, b: TypeId) -> bool;
    /// `TypeSystem.normalize`.
    fn normalize_(&mut self, t: TypeId) -> TypeId;
}

/// O provedor das fases que rodam depois da inferência: os tipos do
/// outline (com os das variáveis já inferidas).
pub struct ProvedorDoOutline<'p, 't> {
    pub program: &'p Program,
    pub interner: &'p Interner,
    pub core: &'p CoreTypes,
    pub outline: &'p OutlineTypes,
    pub table: &'t mut TypeTable,
}

/// O tipo de função de um membro (o de um acessor implícito, montado pelo
/// tipo da variável `tipo_da_variavel`).
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
    outline.functions.get(f.0 as usize).map_or(core.dynamic_, |d| d.signature)
}

impl<'p> Provedor<'p> for ProvedorDoOutline<'p, '_> {
    fn programa(&self) -> &'p Program {
        self.program
    }
    fn interner(&self) -> &'p Interner {
        self.interner
    }
    fn core(&self) -> &'p CoreTypes {
        self.core
    }
    fn tabela(&mut self) -> &mut TypeTable {
        &mut *self.table
    }
    fn class_data(&self, c: ClassId) -> ClassTypeData {
        self.outline.classes[c.0 as usize].clone()
    }
    fn tipo_do_membro(&mut self, f: FunctionElementId) -> TypeId {
        let tv = match self.program.function(f).variable {
            Some(v) => self.outline.variables.get(v.0 as usize).and_then(|d| d.declared_type.or(d.inferred)).unwrap_or(self.core.dynamic_),
            None => self.core.dynamic_,
        };
        member_function_type(self.program, self.outline, self.core, &mut *self.table, f, tv)
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
pub struct Heranca {
    interfaces: HashMap<ClassId, Rc<Interface>>,
    processando: HashSet<ClassId>,
    the_inherited: HashMap<ClassId, Rc<Mapa<Membro>>>,
}

/// A espécie de um membro declarado.
fn kind_of(program: &Program, f: FunctionElementId) -> Especie {
    let e = program.function(f);
    match e.kind {
        FunctionKind::Getter => Especie::Getter,
        FunctionKind::Setter => Especie::Setter,
        FunctionKind::ImplicitAccessor => match e.variable {
            Some(v) if program.variable(v).setter == Some(f) => Especie::Setter,
            _ => Especie::Getter,
        },
        _ => Especie::Metodo,
    }
}

/// A lista de campos da declaração de uma variável (para `abstract` e
/// `covariant`).
fn field_list(program: &Program, v: dartforge_elements::model::VariableId) -> Option<&ast::VariableList> {
    match program.variable(v).node {
        VariableRef::Field { unit, member, .. } => match &program.unit(unit).ast.member(member).kind {
            MemberKind::Field(l) => Some(l),
            _ => None,
        },
        _ => None,
    }
}

/// `isAbstract`. O `int get index;` do `Enum` de `dart:core` conta como
/// concreto (3.6.2 `element_builder.dart:968-974`: a especificação diz que a
/// superclasse de fato é outra classe que implementa `Enum`).
fn the_abstract_(program: &Program, interner: &dartforge_intern::Interner, f: FunctionElementId) -> bool {
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
        return e.variable.and_then(|v| field_list(program, v)).is_some_and(|l| l.abstract_);
    }
    e.abstract_
}

/// Os parâmetros escritos `covariant` de um membro declarado (o setter
/// implícito de um campo `covariant`).
fn written_covariance(program: &Program, f: FunctionElementId) -> Vec<Param> {
    let e = program.function(f);
    if e.kind == FunctionKind::ImplicitAccessor {
        let setter = e.variable.is_some_and(|v| program.variable(v).setter == Some(f));
        let covariant = e.variable.and_then(|v| field_list(program, v)).is_some_and(|l| l.covariant);
        return if setter && covariant { vec![Param::Indice(0)] } else { Vec::new() };
    }
    let FunctionRef::Function { unit, function } = e.node else { return Vec::new() };
    let Some(ps) = &program.unit(unit).ast.function(function).parameters else { return Vec::new() };
    ps.iter()
        .enumerate()
        .filter(|(_, p)| p.covariant)
        .filter_map(|(i, p)| match p.kind {
            ast::ParameterKind::Named => p.public_name.or(p.name).map(|n| Param::Nome(n.sym)),
            _ => Some(Param::Indice(i)),
        })
        .collect()
}

/// Os descritores dos parâmetros de um tipo de função, na ordem.
fn parametros_do_tipo(table: &TypeTable, t: TypeId) -> Vec<Param> {
    match table.get(t) {
        Type::Function { positional, optional, named, .. } => {
            let mut v: Vec<Param> = (0..positional.len() + optional.len()).map(Param::Indice).collect();
            v.extend(named.iter().map(|(n, _, _)| Param::Nome(*n)));
            v
        }
        _ => Vec::new(),
    }
}

impl Heranca {
    /// `getInterface`.
    pub fn interface<'p>(&mut self, p: &mut dyn Provedor<'p>, c: ClassId) -> Rc<Interface> {
        // T1.1: os homônimos da mesma espécie são uma chave só.
        let c = p.programa().dono_da_classe(c);
        if let Some(i) = self.interfaces.get(&c) {
            return i.clone();
        }
        self.interfaces.insert(c, Rc::new(Interface::vazia()));
        if !self.processando.insert(c) {
            return Rc::new(Interface::vazia());
        }
        let r = match p.programa().class(c).kind {
            ClassKind::ExtensionType => self.of_extension_type(p, c),
            ClassKind::Mixin => self.of_mixin(p, c),
            _ => self.of_class(p, c),
        };
        self.processando.remove(&c);
        let r = Rc::new(r);
        self.interfaces.insert(c, r.clone());
        r
    }

    /// Tira do cache a interface de `c` (os tipos dos membros dela mudaram).
    pub fn forget(&mut self, program: &Program, c: ClassId) {
        let c = program.dono_da_classe(c);
        self.interfaces.remove(&c);
        self.the_inherited.remove(&c);
    }

    /// `getMember2`.
    pub fn membro<'p>(
        &mut self,
        p: &mut dyn Provedor<'p>,
        c: ClassId,
        name: Nome,
        concrete: bool,
        mixin_index: Option<usize>,
        to_super: bool,
    ) -> Option<Membro> {
        let i = self.interface(p, c);
        if to_super {
            if p.programa().class(c).kind == ClassKind::ExtensionType {
                return None;
            }
            if let Some(k) = mixin_index {
                return i.super_implemented.get(k).and_then(|m| m.get(&name)).cloned();
            }
            return i.super_implemented.last().and_then(|m| m.get(&name)).cloned();
        }
        if concrete {
            return i.implemented.get(&name).cloned();
        }
        i.map.get(&name).cloned()
    }

    /// `getOverridden2`.
    pub fn sobrescritos<'p>(&mut self, p: &mut dyn Provedor<'p>, c: ClassId, name: Nome) -> Option<Vec<Membro>> {
        self.interface(p, c).overridden.get(&name).cloned()
    }

    /// `getInheritedMap2`.
    pub fn inherited_map<'p>(&mut self, p: &mut dyn Provedor<'p>, c: ClassId) -> Rc<Mapa<Membro>> {
        let c = p.programa().dono_da_classe(c);
        if let Some(m) = self.the_inherited.get(&c) {
            return m.clone();
        }
        let i = self.interface(p, c);
        let mut map = Mapa::default();
        let source = if p.programa().class(c).kind == ClassKind::ExtensionType { &i.redeclared } else { &i.overridden };
        most_specific(p, c, &mut map, source, false);
        let map = Rc::new(map);
        self.the_inherited.insert(c, map.clone());
        map
    }

    /// `getInherited2`.
    pub fn herdado<'p>(&mut self, p: &mut dyn Provedor<'p>, c: ClassId, name: Nome) -> Option<Membro> {
        self.inherited_map(p, c).get(&name).cloned()
    }

    /// `getInheritedConcreteMap2`.
    pub fn inherited_concrete<'p>(&mut self, p: &mut dyn Provedor<'p>, c: ClassId) -> Mapa<Membro> {
        if p.programa().class(c).kind == ClassKind::ExtensionType {
            return Mapa::default();
        }
        self.interface(p, c).super_implemented.last().cloned().unwrap_or_default()
    }

    /// A classe e a substituição (`Substitution.fromInterfaceType`) de um
    /// supertipo.
    fn substitution<'p>(p: &mut dyn Provedor<'p>, t: TypeId) -> Option<(ClassId, HashMap<TypeParamId, TypeId>)> {
        let (c, args) = match p.tabela().get(t) {
            Type::Interface { class, args, .. } => (*class, args.clone()),
            Type::ExtensionType { decl, args, .. } => (*decl, args.clone()),
            _ => return None,
        };
        let formals = p.class_data(c).type_params;
        Some((c, formals.iter().copied().zip(args.iter().copied()).collect()))
    }

    fn substituir<'p>(p: &mut dyn Provedor<'p>, m: &Membro, s: &HashMap<TypeParamId, TypeId>) -> Membro {
        if s.is_empty() {
            return m.clone();
        }
        Membro { tipo: ops::substitute(m.tipo, s, p.tabela()), ..m.clone() }
    }

    /// `_addCandidates`.
    fn add_candidates<'p>(p: &mut dyn Provedor<'p>, candidates: &mut Mapa<Vec<Membro>>, s: &HashMap<TypeParamId, TypeId>, i: &Interface) {
        for (n, m) in i.map.iter() {
            let c = Self::substituir(p, m, s);
            match candidates.get_mut(n) {
                Some(l) => l.push(c),
                None => candidates.insert(*n, vec![c]),
            }
        }
    }

    /// O tipo de `Object`.
    fn object_type<'p>(p: &mut dyn Provedor<'p>) -> Option<TypeId> {
        let o = p.core().object_class?;
        Some(p.tabela().intern(Type::Interface { class: o, args: Box::new([]), nullable: false }))
    }

    /// `element.supertype`, com o padrão do `setDefaultSupertypes` e do
    /// `buildEnumChildren`.
    fn the_supertype<'p>(p: &mut dyn Provedor<'p>, c: ClassId, data: &ClassTypeData) -> Option<TypeId> {
        let prog = p.programa();
        let k = prog.class(c);
        match k.kind {
            ClassKind::Class | ClassKind::MixinApplication => {
                if Some(c) == p.core().object_class {
                    return None;
                }
                // Só um tipo de interface de classe vale como superclasse
                // (`_isInterfaceTypeClass`); o resto cai em `Object`.
                let written = data.supertype.filter(|&t| match p.tabela().get(t) {
                    Type::Interface { class, .. } => matches!(prog.class(*class).kind, ClassKind::Class | ClassKind::MixinApplication),
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
                    Some(e) => Some(p.tabela().intern(Type::Interface { class: e, args: Box::new([]), nullable: false })),
                    None => Self::object_type(p),
                }
            }
            _ => None,
        }
    }

    /// As classes que `InterfaceElementImpl._implementationsOfGetter` (e os
    /// de método e setter) percorre: a própria, os mixins de trás para a
    /// frente e então a superclasse, até `Object`
    /// (an362:src/dart/element/element.dart:5423-5470).
    pub fn implementation_chain<'p>(p: &mut dyn Provedor<'p>, c: ClassId) -> Vec<ClassId> {
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
            current = Self::the_supertype(p, k, &data).and_then(|st| Self::substitution(p, st)).map(|(s, _)| s);
        }
        chain
    }

    /// Os membros de instância concretos da declaração de `c` na ordem do
    /// `_addImplemented` (métodos, depois acessores, cada grupo na ordem dos
    /// elementos), quando algum deles ficou fora do mapa `instance_members`
    /// (homônimo); vazio quando não há homônimo.
    fn concrete_same_named<'p>(p: &mut dyn Provedor<'p>, c: ClassId) -> Vec<FunctionElementId> {
        let prog = p.programa();
        let interner = p.interner();
        let k = prog.class(c);
        let ids = k.instance_members.values().chain(k.static_members.values()).chain(k.constructors.values()).map(|f| f.0);
        let (Some(mut min), Some(mut max)) = (ids.clone().min(), ids.max()) else { return Vec::new() };
        // Os homônimos sobrescritos no mapa podem ficar antes do menor id
        // que restou (ou depois do maior): a faixa cresce enquanto os
        // elementos vizinhos forem da classe.
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
                    && (!matches!(e.node, FunctionRef::None) || e.kind == FunctionKind::ImplicitAccessor)
                    && matches!(e.kind, FunctionKind::Function | FunctionKind::Getter | FunctionKind::Setter | FunctionKind::Operator | FunctionKind::ImplicitAccessor)
            })
            .collect();
        if all.iter().all(|f| in_map.contains(f)) {
            return Vec::new();
        }
        all.retain(|&f| !the_abstract_(prog, interner, f));
        all.sort_by_key(|&f| member_order(prog, f));
        all
    }

    /// Os membros de instância declarados (`_getTypeMembers`): métodos,
    /// depois acessores, na ordem da fonte, com a covariância escrita e a
    /// herdada dos `candidatos` (`_inferParameterCovariance`).
    fn declarados<'p>(p: &mut dyn Provedor<'p>, c: ClassId, candidates: &Mapa<Vec<Membro>>) -> Mapa<Membro> {
        let prog = p.programa();
        let interner = p.interner();
        let k = prog.class(c);
        // O `index` sintético que o outline põe num enum não é membro do
        // elemento do analyzer: vem de `Enum` (concreto, ver `abstrato`).
        let e_enum = k.kind == ClassKind::Enum;
        let mut members: Vec<(SymbolId, FunctionElementId)> = k
            .instance_members
            .iter()
            .filter(|&(&s, &f)| !(e_enum && matches!(prog.function(f).node, FunctionRef::None) && interner.resolve(s) == "index"))
            .map(|(&s, &f)| (s, f))
            .collect();
        // Declaração duplicada: vence o último na ordem do `_getTypeMembers`
        // (métodos; acessores dos campos; acessores escritos), não o da fonte
        // (`int get foo` e depois o campo `foo = 1`: o getter escrito).
        if !k.instancia_sobrepostos.is_empty() {
            for &(key, old) in k.instancia_sobrepostos.iter() {
                if let Some(current) = members.iter_mut().find(|(s, _)| *s == key)
                    && member_order(prog, old) > member_order(prog, current.1)
                {
                    current.1 = old;
                }
            }
        }
        members.sort_by_key(|&(_, f)| (kind_of(prog, f) != Especie::Metodo, f));
        let mut map = Mapa::default();
        for (key, f) in members {
            let name = Nome::novo(interner, k.library, key);
            let typ = p.tipo_do_membro(f);
            let mut the_covariant = written_covariance(prog, f);
            if let Some(overridden) = candidates.get(&name) {
                for d in parametros_do_tipo(p.tabela(), typ) {
                    if the_covariant.contains(&d) {
                        continue;
                    }
                    let inherits = overridden.iter().any(|o| {
                        // `_getCorrespondingParameter`: o nomeado pelo nome, o
                        // posicional pela posição (se lá for posicional).
                        o.the_covariant.contains(&d) && parametros_do_tipo(p.tabela(), o.tipo).contains(&d)
                    });
                    if inherits {
                        the_covariant.push(d);
                    }
                }
            }
            map.insert(
                name,
                Membro {
                    funcao: f,
                    classe: c,
                    tipo: typ,
                    especie: kind_of(prog, f),
                    the_abstract_: the_abstract_(prog, interner, f),
                    the_covariant: the_covariant.into(),
                    sintetico: false,
                },
            );
        }
        map
    }

    /// `_getInterfaceClass`.
    fn of_class<'p>(&mut self, p: &mut dyn Provedor<'p>, c: ClassId) -> Interface {
        let prog = p.programa();
        let data = p.class_data(c);
        let mut candidates: Mapa<Vec<Membro>> = Mapa::default();
        let mut super_impl: Vec<Mapa<Membro>> = Vec::new();
        let mut implemented: Mapa<Membro> = Mapa::default();
        let mut interface_do_super: Option<Rc<Interface>> = None;
        if let Some(st) = Self::the_supertype(p, c, &data)
            && let Some((sc, s)) = Self::substitution(p, st)
        {
            let si = self.interface(p, sc);
            Self::add_candidates(p, &mut candidates, &s, &si);
            for (n, m) in si.implemented.iter() {
                let m = Self::substituir(p, m, &s);
                implemented.insert(*n, m);
            }
            super_impl.push(implemented.clone());
            interface_do_super = Some(si);
        }
        let mut mixin_conflicts: Vec<Vec<Conflict>> = Vec::new();
        for &mt in data.mixins.iter() {
            let Some((mc, s)) = Self::substitution(p, mt) else { continue };
            let mi = self.interface(p, mc);
            let mut of_super_and_mixin: Mapa<Vec<Membro>> = Mapa::default();
            let mut mixin_conflicts_of: Vec<Conflict> = Vec::new();
            for (n, m0) in mi.map.iter() {
                let candidate = Self::substituir(p, m0, &s);
                let current = match candidates.get(n) {
                    None => {
                        candidates.insert(*n, vec![candidate]);
                        continue;
                    }
                    Some(l) => l[0].clone(),
                };
                if candidate.classe == mc {
                    if current.especie != candidate.especie {
                        let (getter, method) = if current.especie == Especie::Getter { (current, candidate.clone()) } else { (candidate.clone(), current) };
                        mixin_conflicts_of.push(Conflict::GetterMethod { nome: *n, getter, metodo: method });
                    }
                    candidates.insert(*n, vec![candidate]);
                } else {
                    of_super_and_mixin.insert(*n, vec![current, candidate]);
                }
            }
            let mut map = Mapa::default();
            most_specific(p, c, &mut map, &of_super_and_mixin, true);
            for (n, m) in map.iter() {
                candidates.insert(*n, vec![m.clone()]);
            }
            mixin_conflicts.push(mixin_conflicts_of);
            // `_addMixinMembers`.
            let object = p.core().object_class;
            for (n, m) in mi.implemented.iter() {
                if m.the_abstract_ || Some(m.classe) == object {
                    continue;
                }
                let m = Self::substituir(p, m, &s);
                implemented.insert(*n, m);
            }
            super_impl.push(implemented.clone());
        }
        for &it in data.interfaces.iter() {
            let Some((ic, s)) = Self::substitution(p, it) else { continue };
            let ii = self.interface(p, ic);
            Self::add_candidates(p, &mut candidates, &s, &ii);
        }
        let declared = Self::declarados(p, c, &candidates);
        // `_addImplemented` (3.6.2 `inheritance_manager3.dart:455-470`): todo
        // método e acessor de instância concreto da declaração, na ordem
        // (métodos, depois acessores), o último vencendo. Com homônimos (o
        // `augment` que o 3.6.2 lê como outra declaração), o mapa do outline
        // guarda só o último; os outros elementos estão na faixa de ids dos
        // membros da classe.
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
                let key = if kind == Especie::Setter && !text.ends_with("_=") {
                    match interner.lookup(&format!("{text}_=")) {
                        Some(s) => s,
                        None => continue,
                    }
                } else {
                    e.name
                };
                let name = Nome::novo(interner, lib, key);
                let m = match declared.get(&name) {
                    Some(d) if d.funcao == f => d.clone(),
                    _ => Membro {
                        funcao: f,
                        classe: c,
                        tipo: p.tipo_do_membro(f),
                        especie: kind,
                        the_abstract_: false,
                        the_covariant: written_covariance(prog, f).into(),
                        sintetico: false,
                    },
                };
                implemented.insert(name, m);
            }
        }
        let mut map = declared.clone();
        let mut conflicts = most_specific(p, c, &mut map, &candidates, true);
        // Os encaminhadores de `noSuchMethod`.
        let k = prog.class(c);
        let mut forwarded: HashSet<Nome> = HashSet::new();
        let abstract_class =
            matches!(k.kind, ClassKind::Class | ClassKind::MixinApplication) && (k.modifiers.abstract_ || k.modifiers.sealed);
        if abstract_class {
            if let Some(si) = &interface_do_super {
                forwarded = si.encaminhados.clone();
            }
        } else if let Some(nsm) = p.interner().lookup("noSuchMethod") {
            let name = Nome { chave: nsm, biblioteca: None };
            let object = p.core().object_class;
            if implemented.get(&name).is_some_and(|m| Some(m.classe) != object) {
                let do_super = interface_do_super.as_ref().map(|s| &s.encaminhados);
                for (n, m) in map.iter() {
                    if !implemented.contains_key(n) || do_super.is_some_and(|s| s.contains(n)) {
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
        let mut with_covariance: Mapa<Membro> = Mapa::default();
        for (n, m) in implemented.iter() {
            let r = inherit_covariance(p, c, &candidates, n, m);
            with_covariance.insert(*n, r);
        }
        Interface {
            map: map,
            declared: declared,
            implemented: with_covariance,
            encaminhados: forwarded,
            overridden: candidates,
            redeclared: Mapa::default(),
            super_implemented: super_impl,
            conflicts,
        }
    }

    /// `_getInterfaceMixin`.
    fn of_mixin<'p>(&mut self, p: &mut dyn Provedor<'p>, c: ClassId) -> Interface {
        let data = p.class_data(c);
        let mut constraints: Vec<TypeId> = data.on.to_vec();
        if constraints.is_empty()
            && let Some(o) = Self::object_type(p)
        {
            constraints.push(o);
        }
        let mut super_candidates: Mapa<Vec<Membro>> = Mapa::default();
        for t in constraints {
            let Some((rc, s)) = Self::substitution(p, t) else { continue };
            let ri = self.interface(p, rc);
            Self::add_candidates(p, &mut super_candidates, &s, &ri);
        }
        let mut interface_do_super: Mapa<Membro> = Mapa::default();
        let super_conflicts = most_specific(p, c, &mut interface_do_super, &super_candidates, true);
        let mut candidates = super_candidates.clone();
        for &it in data.interfaces.iter() {
            let Some((ic, s)) = Self::substitution(p, it) else { continue };
            let ii = self.interface(p, ic);
            Self::add_candidates(p, &mut candidates, &s, &ii);
        }
        let declared = Self::declarados(p, c, &candidates);
        let mut map = declared.clone();
        let interface_conflicts = most_specific(p, c, &mut map, &candidates, true);
        let mut implemented: Mapa<Membro> = Mapa::default();
        for (n, m) in declared.iter() {
            if !m.the_abstract_ {
                implemented.insert(*n, m.clone());
            }
        }
        let mut conflicts = super_conflicts;
        conflicts.extend(interface_conflicts);
        Interface {
            map: map,
            declared: declared,
            implemented: implemented,
            encaminhados: HashSet::new(),
            overridden: candidates,
            redeclared: Mapa::default(),
            super_implemented: vec![interface_do_super],
            conflicts,
        }
    }

    /// `_getInterfaceExtensionType`.
    fn of_extension_type<'p>(&mut self, p: &mut dyn Provedor<'p>, c: ClassId) -> Interface {
        let prog = p.programa();
        let interner = p.interner();
        let data = p.class_data(c);
        let all = Self::declarados(p, c, &Mapa::default());
        let mut declared: Mapa<Membro> = Mapa::default();
        for (n, m) in all.iter() {
            if !m.the_abstract_ {
                declared.insert(*n, m.clone());
            }
        }
        // Os nomes excluídos.
        let mut excluded_names: HashSet<Nome> = HashSet::new();
        let mut excluded_methods: HashSet<Nome> = HashSet::new();
        let mut excluded_setters: HashSet<Nome> = HashSet::new();
        for (n, m) in declared.iter() {
            excluded_names.insert(*n);
            match m.especie {
                Especie::Metodo => {
                    if let Some(s) = n.de_setter(interner) {
                        excluded_setters.insert(s);
                    }
                }
                Especie::Setter => {
                    if let Some(g) = n.of_getter(interner) {
                        excluded_methods.insert(g);
                    }
                }
                Especie::Getter => {}
            }
        }
        let mut implemented = declared.clone();
        // Os candidatos, separados entre membros de tipo de extensão e os
        // outros (`_ExtensionTypeCandidates`: métodos, getters, setters).
        #[derive(Default, Clone)]
        struct Candidates {
            metodos: Vec<Membro>,
            getters: Vec<Membro>,
            setters: Vec<Membro>,
        }
        impl Candidates {
            fn adicionar(&mut self, m: Membro) {
                match m.especie {
                    Especie::Metodo => self.metodos.push(m),
                    Especie::Getter => self.getters.push(m),
                    Especie::Setter => self.setters.push(m),
                }
            }
            fn todos(&self) -> Vec<Membro> {
                self.metodos.iter().chain(self.getters.iter()).chain(self.setters.iter()).cloned().collect()
            }
        }
        let not_excluded = |n: &Nome, cs: &Candidates| -> Vec<Membro> {
            if excluded_names.contains(n) {
                return Vec::new();
            }
            let mut v = Vec::new();
            if !excluded_methods.contains(n) {
                v.extend(cs.metodos.iter().cloned());
            }
            v.extend(cs.getters.iter().cloned());
            if !excluded_setters.contains(n) {
                v.extend(cs.setters.iter().cloned());
            }
            v
        };
        let mut of_extension: Mapa<Candidates> = Mapa::default();
        let mut others: Mapa<Candidates> = Mapa::default();
        for &it in data.interfaces.iter() {
            let Some((ic, s)) = Self::substitution(p, it) else { continue };
            let ii = self.interface(p, ic);
            for (n, m) in ii.map.iter() {
                let m = Self::substituir(p, m, &s);
                // `isExtensionTypeMember` é um bit do elemento declarado no
                // extension type; o membro sintético do `_topMerge` (dono =
                // o extension type que combinou) nasce sem ele e conta como
                // não-extensão (`G implements F, D` com o `method` combinado
                // de `A`/`B` em `F`: sem conflito).
                let of_ext = !m.sintetico && prog.class(m.classe).kind == ClassKind::ExtensionType;
                let target = if of_ext { &mut of_extension } else { &mut others };
                match target.get_mut(n) {
                    Some(cs) => cs.adicionar(m),
                    None => {
                        let mut cs = Candidates::default();
                        cs.adicionar(m);
                        target.insert(*n, cs);
                    }
                }
            }
        }
        let mut redeclared: Mapa<Vec<Membro>> = Mapa::default();
        let mut conflicts: Vec<Conflict> = Vec::new();
        for (n, cs) in of_extension.iter() {
            match redeclared.get_mut(n) {
                Some(l) => l.extend(cs.todos()),
                None => redeclared.insert(*n, cs.todos()),
            }
            let free = not_excluded(n, cs);
            if free.is_empty() {
                continue;
            }
            if let Some(ne) = others.get(n) {
                let ne_free = not_excluded(n, ne);
                if !ne_free.is_empty() {
                    conflicts.push(Conflict::ExtensionAndNonExtension { nome: *n, non_extension: ne_free, extensao: free });
                }
                continue;
            }
            // O herdado tem de ser um só.
            let unique = free.iter().all(|m| m.funcao == free[0].funcao);
            if !unique {
                conflicts.push(Conflict::NonUniqueExtension { nome: *n, candidatos: free });
                continue;
            }
            implemented.insert(*n, free[0].clone());
        }
        for (n, cs) in others.iter() {
            match redeclared.get_mut(n) {
                Some(l) => l.extend(cs.todos()),
                None => redeclared.insert(*n, cs.todos()),
            }
            let free = not_excluded(n, cs);
            if free.is_empty() || of_extension.contains_key(n) {
                continue;
            }
            match combinar(p, c, &free, true, *n, None) {
                Some(m) => implemented.insert(*n, m),
                None => conflicts.push(Conflict::Candidates { nome: *n, candidatos: free }),
            }
        }
        // Os redeclarados sem repetição.
        let mut the_unique: Mapa<Vec<Membro>> = Mapa::default();
        for (n, l) in redeclared.iter() {
            if l.len() <= 1 {
                the_unique.insert(*n, l.clone());
            } else {
                let mut seen: Vec<FunctionElementId> = Vec::new();
                let mut v = Vec::new();
                for m in l {
                    if !seen.contains(&m.funcao) {
                        seen.push(m.funcao);
                        v.push(m.clone());
                    }
                }
                the_unique.insert(*n, v);
            }
        }
        Interface {
            map: implemented.clone(),
            declared: declared,
            implemented: implemented,
            encaminhados: HashSet::new(),
            overridden: Mapa::default(),
            redeclared: the_unique,
            super_implemented: Vec::new(),
            conflicts,
        }
    }
}

/// A ordem dos membros de instância de uma declaração no elemento do
/// analyzer (`_getTypeMembers`, `_addImplemented`): métodos; depois os
/// acessores, os dos campos antes dos escritos (`_visitPropertyFirst`); em
/// cada grupo, a ordem da fonte.
fn member_order(prog: &Program, f: FunctionElementId) -> (u8, FunctionElementId) {
    let category = match kind_of(prog, f) {
        Especie::Metodo => 0u8,
        _ if matches!(prog.function(f).node, FunctionRef::None) => 1,
        _ => 2,
    };
    (category, f)
}

/// `_checkForGetterMethodConflict`.
fn getter_method_conflict(name: Nome, candidates: &[Membro]) -> Option<Conflict> {
    let getter = candidates.iter().find(|m| m.especie == Especie::Getter)?;
    let method = candidates.iter().find(|m| m.especie == Especie::Metodo)?;
    Some(Conflict::GetterMethod { nome: name, getter: getter.clone(), metodo: method.clone() })
}

/// `_findMostSpecificFromNamedCandidates`.
fn most_specific<'p>(p: &mut dyn Provedor<'p>, target: ClassId, map: &mut Mapa<Membro>, candidates: &Mapa<Vec<Membro>>, top_merge: bool) -> Vec<Conflict> {
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
        if let Some(m) = combinar(p, target, list, top_merge, *n, Some(&mut conflicts)) {
            map.insert(*n, m);
        }
    }
    conflicts
}

/// `combineSignatures`.
pub fn combinar<'p>(
    p: &mut dyn Provedor<'p>,
    target: ClassId,
    candidates: &[Membro],
    top_merge: bool,
    name: Nome,
    conflicts: Option<&mut Vec<Conflict>>,
) -> Option<Membro> {
    if candidates.len() == 1 {
        return Some(candidates[0].clone());
    }
    let mut valid: Vec<Membro> = Vec::new();
    for (i, c) in candidates.iter().enumerate() {
        let _ = i;
        let ok = candidates.iter().all(|d| p.sub(c.tipo, d.tipo));
        if ok {
            valid.push(c.clone());
        }
    }
    if valid.is_empty() {
        if let Some(cs) = conflicts {
            cs.push(Conflict::Candidates { nome: name, candidatos: candidates.to_vec() });
        }
        return None;
    }
    if top_merge { Some(topo(p, target, &valid)) } else { Some(valid[0].clone()) }
}

/// A igualdade de tipos do Dart (`==`): estrutural, sem o alias, e a menos
/// do nome dos parâmetros de tipo de uma função genérica.
fn iguais(table: &mut TypeTable, a: TypeId, b: TypeId) -> bool {
    let (ca, cb) = (table.canonico(a), table.canonico(b));
    if ca == cb {
        return true;
    }
    match (table.get(ca).clone(), table.get(cb).clone()) {
        (Type::Function { type_params: ta, .. }, Type::Function { type_params: tb, .. }) if !ta.is_empty() && ta.len() == tb.len() => {
            let mut s: HashMap<TypeParamId, TypeId> = HashMap::new();
            for (x, y) in tb.iter().zip(ta.iter()) {
                let ty = table.intern(Type::TypeParameter { param: *y, nullable: false });
                s.insert(*x, ty);
            }
            // Os parâmetros de `b` renomeados para os de `a`: o resto tem de
            // coincidir.
            let rb = ops::substitute(cb, &s, table);
            let (Type::Function { ret: ra, positional: pa, optional: oa, named: at, nullable: la, .. }, Type::Function { ret: rr, positional: pb, optional: ob, named: nb, nullable: lb, .. }) =
                (table.get(ca).clone(), table.get(rb).clone())
            else {
                return false;
            };
            let limites = ta.iter().zip(tb.iter()).all(|(x, y)| {
                let lx = table.param(*x).bound;
                let ly = ops::substitute(table.param(*y).bound, &s, table);
                table.canonico(lx) == table.canonico(ly)
            });
            limites && la == lb && table.canonico(ra) == table.canonico(rr) && pa == pb && oa == ob && at == nb
        }
        _ => false,
    }
}

/// `_topMerge`.
fn topo<'p>(p: &mut dyn Provedor<'p>, target: ClassId, valid: &[Membro]) -> Membro {
    let first = &valid[0];
    if valid.len() == 1 {
        return first.clone();
    }
    let all_equal = valid.iter().all(|m| iguais(p.tabela(), m.tipo, first.tipo));
    if all_equal {
        return first.clone();
    }
    // `reduce` do `topMerge` sobre os tipos normalizados; os parâmetros do
    // resultado guardam a covariância do operando da esquerda.
    let mut result = p.normalize_(first.tipo);
    let the_covariant = first.the_covariant.clone();
    for m in &valid[1..] {
        let t = p.normalize_(m.tipo);
        result = top_merge_function(p, result, t, &the_covariant, &m.the_covariant);
    }
    for m in valid {
        if iguais(p.tabela(), m.tipo, result) {
            return m.clone();
        }
    }
    Membro {
        funcao: first.funcao,
        classe: target,
        tipo: result,
        especie: first.especie,
        the_abstract_: false,
        the_covariant,
        sintetico: true,
    }
}

/// `_inheritCovariance`.
fn inherit_covariance<'p>(p: &mut dyn Provedor<'p>, c: ClassId, candidates: &Mapa<Vec<Membro>>, name: &Nome, m: &Membro) -> Membro {
    if m.classe == c {
        return m.clone();
    }
    let parameters = parametros_do_tipo(p.tabela(), m.tipo);
    if parameters.is_empty() {
        return m.clone();
    }
    let Some(list) = candidates.get(name) else { return m.clone() };
    let mut the_covariant: HashSet<Param> = HashSet::new();
    for o in list {
        for d in parametros_do_tipo(p.tabela(), o.tipo) {
            if o.the_covariant.contains(&d) {
                the_covariant.insert(d);
            }
        }
    }
    if the_covariant.is_empty() {
        return m.clone();
    }
    let must: Vec<Param> = parameters.iter().copied().filter(|d| the_covariant.contains(d)).collect();
    let changed = parameters.iter().any(|d| m.the_covariant.contains(d) != must.contains(d));
    if !changed {
        return m.clone();
    }
    match m.especie {
        Especie::Metodo | Especie::Setter => Membro { classe: c, the_covariant: must.into(), sintetico: true, ..m.clone() },
        Especie::Getter => m.clone(),
    }
}

// -- `NNBD_TOP_MERGE` -----------------------------------------------------------

fn anulavel(table: &TypeTable, t: TypeId) -> bool {
    table.get(t).is_declared_nullable()
}

fn with_nullability(table: &mut TypeTable, t: TypeId, nullable: bool) -> TypeId {
    let the_new_ = match table.get(t).clone() {
        Type::Interface { class, args, .. } => Type::Interface { class, args, nullable },
        Type::Function { type_params, ret, positional, optional, named, .. } => Type::Function { type_params, ret, positional, optional, named, nullable },
        Type::Record { positional, named, .. } => Type::Record { positional, named, nullable },
        Type::TypeParameter { param, .. } => Type::TypeParameter { param, nullable },
        Type::FutureOr { arg, .. } => Type::FutureOr { arg, nullable },
        Type::ExtensionType { decl, args, .. } => Type::ExtensionType { decl, args, nullable },
        _ => return t,
    };
    table.intern(the_new_)
}

/// `topMerge` de dois tipos quaisquer (os tipos de função aninhados não têm
/// parâmetros covariantes).
fn top_merge<'p>(p: &mut dyn Provedor<'p>, t: TypeId, s: TypeId) -> TypeId {
    top_merge_function(p, t, s, &[], &[])
}

/// `TopMergeHelper.topMerge`, com a covariância dos parâmetros quando `t` e
/// `s` são os tipos de dois membros.
fn top_merge_function<'p>(p: &mut dyn Provedor<'p>, t: TypeId, s: TypeId, cov_t: &[Param], cov_s: &[Param]) -> TypeId {
    let object = p.core().object_class;
    let (tt, st) = (p.tabela().get(t).clone(), p.tabela().get(s).clone());
    let t_obj_q = matches!(tt, Type::Interface { class, nullable: true, .. } if Some(class) == object);
    let s_obj_q = matches!(st, Type::Interface { class, nullable: true, .. } if Some(class) == object);
    if t_obj_q && s_obj_q {
        return t;
    }
    let invalid_t = p.tabela().e_invalido(t);
    let invalid_s = p.tabela().e_invalido(s);
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
    let (tq, sq) = (anulavel(p.tabela(), t), anulavel(p.tabela(), s));
    if tq && sq {
        let tn = with_nullability(p.tabela(), t, false);
        let sn = with_nullability(p.tabela(), s, false);
        let r = top_merge_function(p, tn, sn, cov_t, cov_s);
        return with_nullability(p.tabela(), r, true);
    } else if tq || sq {
        // `StateError` no original: não acontece com tipos normalizados que
        // são subtipos um do outro.
        return t;
    }
    match (tt, st) {
        (Type::Interface { class: a, args: xa, .. }, Type::Interface { class: b, args: xb, .. }) => {
            if a != b {
                return t;
            }
            if xa.is_empty() {
                return t;
            }
            let args: Vec<TypeId> = xa.iter().zip(xb.iter()).map(|(x, y)| top_merge(p, *x, *y)).collect();
            p.tabela().intern(Type::Interface { class: a, args: args.into_boxed_slice(), nullable: false })
        }
        (Type::ExtensionType { decl: a, args: xa, .. }, Type::ExtensionType { decl: b, args: xb, .. }) => {
            if a != b || xa.is_empty() {
                return t;
            }
            let args: Vec<TypeId> = xa.iter().zip(xb.iter()).map(|(x, y)| top_merge(p, *x, *y)).collect();
            p.tabela().intern(Type::ExtensionType { decl: a, args: args.into_boxed_slice(), nullable: false })
        }
        (Type::FutureOr { arg: a, .. }, Type::FutureOr { arg: b, .. }) => {
            let r = top_merge(p, a, b);
            p.tabela().intern(Type::FutureOr { arg: r, nullable: false })
        }
        (Type::Null, Type::Null) => t,
        (Type::Function { .. }, Type::Function { .. }) => funcoes(p, t, s, cov_t, cov_s),
        (Type::Record { positional: pa, named: at, .. }, Type::Record { positional: pb, named: nb, .. }) => {
            if pa.len() != pb.len() || at.len() != nb.len() {
                return t;
            }
            let pos: Vec<TypeId> = pa.iter().zip(pb.iter()).map(|(x, y)| top_merge(p, *x, *y)).collect();
            let mut named_args: Vec<(SymbolId, TypeId)> = Vec::new();
            for ((n1, x), (n2, y)) in at.iter().zip(nb.iter()) {
                if n1 != n2 {
                    return t;
                }
                named_args.push((*n1, top_merge(p, *x, *y)));
            }
            p.tabela().intern(Type::Record { positional: pos.into_boxed_slice(), named: named_args.into_boxed_slice(), nullable: false })
        }
        (Type::TypeParameter { param: a, .. }, Type::TypeParameter { param: b, .. }) if a == b => t,
        _ => t,
    }
}

/// `_functionTypes`.
fn funcoes<'p>(p: &mut dyn Provedor<'p>, t: TypeId, s: TypeId, cov_t: &[Param], cov_s: &[Param]) -> TypeId {
    let (
        Type::Function { type_params: tp_t, ret: rt, positional: pt, optional: ot, named: nt, .. },
        Type::Function { type_params: tp_s, ret: rs, positional: ps, optional: os, named: ns, .. },
    ) = (p.tabela().get(t).clone(), p.tabela().get(s).clone())
    else {
        return t;
    };
    if tp_t.len() != tp_s.len() {
        return t;
    }
    // Os parâmetros de tipo novos (`_typeParameters`).
    let mut new_: Vec<TypeParamId> = Vec::new();
    let mut sub_t: HashMap<TypeParamId, TypeId> = HashMap::new();
    let mut sub_s: HashMap<TypeParamId, TypeId> = HashMap::new();
    if !tp_t.is_empty() {
        for (a, b) in tp_t.iter().zip(tp_s.iter()) {
            let name = p.tabela().param(*a).name;
            let default_bound = p.core().object_nullable;
            let n = p.tabela().alloc_type_param(name, TypeParamOwner::GenericFunctionType, default_bound, Variance::Unspecified);
            let tn = p.tabela().intern(Type::TypeParameter { param: n, nullable: false });
            sub_t.insert(*a, tn);
            sub_s.insert(*b, tn);
            new_.push(n);
        }
        for (i, (a, b)) in tp_t.iter().zip(tp_s.iter()).enumerate() {
            let (ea, eb) = (p.tabela().param(*a).explicito, p.tabela().param(*b).explicito);
            if !ea && !eb {
                continue;
            }
            if ea != eb {
                return t;
            }
            let la = p.tabela().param(*a).bound;
            let lb = p.tabela().param(*b).bound;
            let la = ops::substitute(la, &sub_t, p.tabela());
            let lb = ops::substitute(lb, &sub_s, p.tabela());
            let l = top_merge(p, la, lb);
            p.tabela().set_type_param_bound(new_[i], l);
        }
    }
    let ret = juntar(p, rt, rs, &sub_t, &sub_s);
    if pt.len() != ps.len() || ot.len() != os.len() || nt.len() != ns.len() {
        return t;
    }
    let mut positional_args = Vec::with_capacity(pt.len());
    for (i, (a, b)) in pt.iter().zip(ps.iter()).enumerate() {
        positional_args.push(parametro(p, Param::Indice(i), *a, *b, cov_t, cov_s, &sub_t, &sub_s));
    }
    let mut the_optional = Vec::with_capacity(ot.len());
    for (i, (a, b)) in ot.iter().zip(os.iter()).enumerate() {
        the_optional.push(parametro(p, Param::Indice(pt.len() + i), *a, *b, cov_t, cov_s, &sub_t, &sub_s));
    }
    let mut named_args = Vec::with_capacity(nt.len());
    for ((n1, a, r1), (n2, b, r2)) in nt.iter().zip(ns.iter()) {
        if n1 != n2 {
            return t;
        }
        let ty = parametro(p, Param::Nome(*n1), *a, *b, cov_t, cov_s, &sub_t, &sub_s);
        named_args.push((*n1, ty, *r1 || *r2));
    }
    p.tabela().intern(Type::Function {
        type_params: new_.into_boxed_slice(),
        ret,
        positional: positional_args.into_boxed_slice(),
        optional: the_optional.into_boxed_slice(),
        named: named_args.into_boxed_slice(),
        nullable: false,
    })
}

/// `mergeTypes` de `_functionTypes`: os dois lados com os parâmetros de tipo
/// novos.
fn juntar<'p>(p: &mut dyn Provedor<'p>, a: TypeId, b: TypeId, sub_t: &HashMap<TypeParamId, TypeId>, sub_s: &HashMap<TypeParamId, TypeId>) -> TypeId {
    let a = ops::substitute(a, sub_t, p.tabela());
    let b = ops::substitute(b, sub_s, p.tabela());
    top_merge(p, a, b)
}

/// Um parâmetro de `_functionTypes`: o tipo pelo `topMerge`, ou, se um dos
/// lados é covariante, o maior dos dois (os tipos crus, sem a substituição,
/// como no original).
#[allow(clippy::too_many_arguments)]
fn parametro<'p>(
    p: &mut dyn Provedor<'p>,
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
            juntar(p, a, b, sub_t, sub_s)
        } else if a_sub_b {
            b
        } else {
            a
        }
    } else {
        juntar(p, a, b, sub_t, sub_s)
    }
}
