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
    pub fn novo(interner: &Interner, biblioteca: LibraryId, chave: SymbolId) -> Nome {
        let privado = interner.resolve(chave).starts_with('_');
        Nome { chave, biblioteca: privado.then_some(biblioteca) }
    }

    /// `forSetter`.
    pub fn de_setter(self, interner: &Interner) -> Option<Nome> {
        let texto = interner.resolve(self.chave);
        let chave = interner.lookup(&format!("{texto}_="))?;
        Some(Nome { chave, biblioteca: self.biblioteca })
    }

    /// `forGetter`.
    pub fn de_getter(self, interner: &Interner) -> Option<Nome> {
        let texto = interner.resolve(self.chave);
        match texto.strip_suffix("_=") {
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
    pub abstrato: bool,
    /// Os parâmetros covariantes (`isCovariant`: escrito ou herdado).
    pub covariantes: Rc<[Param]>,
    pub sintetico: bool,
}

/// `Conflict`.
#[derive(Clone, Debug)]
pub enum Conflito {
    /// `CandidatesConflict`.
    Candidatos { nome: Nome, candidatos: Vec<Membro> },
    /// `GetterMethodConflict`.
    GetterMetodo { nome: Nome, getter: Membro, metodo: Membro },
    /// `HasNonExtensionAndExtensionMemberConflict`.
    ExtensaoENaoExtensao { nome: Nome, nao_extensao: Vec<Membro>, extensao: Vec<Membro> },
    /// `NotUniqueExtensionMemberConflict`.
    ExtensaoNaoUnica { nome: Nome, candidatos: Vec<Membro> },
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
    pub conflitos: Vec<Conflito>,
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
    fn dados_da_classe(&self, c: ClassId) -> ClassTypeData;
    /// `ExecutableElement.type` do membro `f`, nos parâmetros de tipo da
    /// classe que o declara.
    fn tipo_do_membro(&mut self, f: FunctionElementId) -> TypeId;
    fn sub(&mut self, a: TypeId, b: TypeId) -> bool;
    /// `TypeSystem.normalize`.
    fn normalizar(&mut self, t: TypeId) -> TypeId;
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
pub fn tipo_de_funcao_do_membro(
    program: &Program,
    outline: &OutlineTypes,
    core: &CoreTypes,
    table: &mut TypeTable,
    f: FunctionElementId,
    tipo_da_variavel: TypeId,
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
                positional: Box::new([tipo_da_variavel]),
                optional: Box::new([]),
                named: Box::new([]),
                nullable: false,
            })
        } else {
            table.intern(Type::Function {
                type_params: Box::new([]),
                ret: tipo_da_variavel,
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
    fn dados_da_classe(&self, c: ClassId) -> ClassTypeData {
        self.outline.classes[c.0 as usize].clone()
    }
    fn tipo_do_membro(&mut self, f: FunctionElementId) -> TypeId {
        let tv = match self.program.function(f).variable {
            Some(v) => self.outline.variables.get(v.0 as usize).and_then(|d| d.declared_type.or(d.inferred)).unwrap_or(self.core.dynamic_),
            None => self.core.dynamic_,
        };
        tipo_de_funcao_do_membro(self.program, self.outline, self.core, &mut *self.table, f, tv)
    }
    fn sub(&mut self, a: TypeId, b: TypeId) -> bool {
        let mut env = SubtypeEnv::new(&mut *self.table, &self.outline.hierarchy, self.core);
        is_subtype(a, b, &mut env)
    }
    fn normalizar(&mut self, t: TypeId) -> TypeId {
        ops::normalize(t, &mut *self.table, self.core)
    }
}

/// `InheritanceManager3`.
#[derive(Default)]
pub struct Heranca {
    interfaces: HashMap<ClassId, Rc<Interface>>,
    processando: HashSet<ClassId>,
    herdados: HashMap<ClassId, Rc<Mapa<Membro>>>,
}

/// A espécie de um membro declarado.
fn especie_de(program: &Program, f: FunctionElementId) -> Especie {
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
fn lista_do_campo(program: &Program, v: dartforge_elements::model::VariableId) -> Option<&ast::VariableList> {
    match program.variable(v).node {
        VariableRef::Field { unit, member, .. } => match &program.unit(unit).ast.member(member).kind {
            MemberKind::Field(l) => Some(l),
            _ => None,
        },
        _ => None,
    }
}

/// `isAbstract`.
fn abstrato(program: &Program, f: FunctionElementId) -> bool {
    let e = program.function(f);
    if e.kind == FunctionKind::ImplicitAccessor {
        return e.variable.and_then(|v| lista_do_campo(program, v)).is_some_and(|l| l.abstract_);
    }
    e.abstract_
}

/// Os parâmetros escritos `covariant` de um membro declarado (o setter
/// implícito de um campo `covariant`).
fn covariancia_escrita(program: &Program, f: FunctionElementId) -> Vec<Param> {
    let e = program.function(f);
    if e.kind == FunctionKind::ImplicitAccessor {
        let setter = e.variable.is_some_and(|v| program.variable(v).setter == Some(f));
        let covariante = e.variable.and_then(|v| lista_do_campo(program, v)).is_some_and(|l| l.covariant);
        return if setter && covariante { vec![Param::Indice(0)] } else { Vec::new() };
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
            ClassKind::ExtensionType => self.de_tipo_de_extensao(p, c),
            ClassKind::Mixin => self.de_mixin(p, c),
            _ => self.de_classe(p, c),
        };
        self.processando.remove(&c);
        let r = Rc::new(r);
        self.interfaces.insert(c, r.clone());
        r
    }

    /// Tira do cache a interface de `c` (os tipos dos membros dela mudaram).
    pub fn esquecer(&mut self, program: &Program, c: ClassId) {
        let c = program.dono_da_classe(c);
        self.interfaces.remove(&c);
        self.herdados.remove(&c);
    }

    /// `getMember2`.
    pub fn membro<'p>(
        &mut self,
        p: &mut dyn Provedor<'p>,
        c: ClassId,
        nome: Nome,
        concreto: bool,
        indice_mixin: Option<usize>,
        para_super: bool,
    ) -> Option<Membro> {
        let i = self.interface(p, c);
        if para_super {
            if p.programa().class(c).kind == ClassKind::ExtensionType {
                return None;
            }
            if let Some(k) = indice_mixin {
                return i.super_implemented.get(k).and_then(|m| m.get(&nome)).cloned();
            }
            return i.super_implemented.last().and_then(|m| m.get(&nome)).cloned();
        }
        if concreto {
            return i.implemented.get(&nome).cloned();
        }
        i.map.get(&nome).cloned()
    }

    /// `getOverridden2`.
    pub fn sobrescritos<'p>(&mut self, p: &mut dyn Provedor<'p>, c: ClassId, nome: Nome) -> Option<Vec<Membro>> {
        self.interface(p, c).overridden.get(&nome).cloned()
    }

    /// `getInheritedMap2`.
    pub fn mapa_herdado<'p>(&mut self, p: &mut dyn Provedor<'p>, c: ClassId) -> Rc<Mapa<Membro>> {
        let c = p.programa().dono_da_classe(c);
        if let Some(m) = self.herdados.get(&c) {
            return m.clone();
        }
        let i = self.interface(p, c);
        let mut mapa = Mapa::default();
        let fonte = if p.programa().class(c).kind == ClassKind::ExtensionType { &i.redeclared } else { &i.overridden };
        mais_especificos(p, c, &mut mapa, fonte, false);
        let mapa = Rc::new(mapa);
        self.herdados.insert(c, mapa.clone());
        mapa
    }

    /// `getInherited2`.
    pub fn herdado<'p>(&mut self, p: &mut dyn Provedor<'p>, c: ClassId, nome: Nome) -> Option<Membro> {
        self.mapa_herdado(p, c).get(&nome).cloned()
    }

    /// `getInheritedConcreteMap2`.
    pub fn concretos_herdados<'p>(&mut self, p: &mut dyn Provedor<'p>, c: ClassId) -> Mapa<Membro> {
        if p.programa().class(c).kind == ClassKind::ExtensionType {
            return Mapa::default();
        }
        self.interface(p, c).super_implemented.last().cloned().unwrap_or_default()
    }

    /// A classe e a substituição (`Substitution.fromInterfaceType`) de um
    /// supertipo.
    fn substituicao<'p>(p: &mut dyn Provedor<'p>, t: TypeId) -> Option<(ClassId, HashMap<TypeParamId, TypeId>)> {
        let (c, args) = match p.tabela().get(t) {
            Type::Interface { class, args, .. } => (*class, args.clone()),
            Type::ExtensionType { decl, args, .. } => (*decl, args.clone()),
            _ => return None,
        };
        let formais = p.dados_da_classe(c).type_params;
        Some((c, formais.iter().copied().zip(args.iter().copied()).collect()))
    }

    fn substituir<'p>(p: &mut dyn Provedor<'p>, m: &Membro, s: &HashMap<TypeParamId, TypeId>) -> Membro {
        if s.is_empty() {
            return m.clone();
        }
        Membro { tipo: ops::substitute(m.tipo, s, p.tabela()), ..m.clone() }
    }

    /// `_addCandidates`.
    fn adicionar_candidatos<'p>(p: &mut dyn Provedor<'p>, candidatos: &mut Mapa<Vec<Membro>>, s: &HashMap<TypeParamId, TypeId>, i: &Interface) {
        for (n, m) in i.map.iter() {
            let c = Self::substituir(p, m, s);
            match candidatos.get_mut(n) {
                Some(l) => l.push(c),
                None => candidatos.insert(*n, vec![c]),
            }
        }
    }

    /// O tipo de `Object`.
    fn tipo_object<'p>(p: &mut dyn Provedor<'p>) -> Option<TypeId> {
        let o = p.core().object_class?;
        Some(p.tabela().intern(Type::Interface { class: o, args: Box::new([]), nullable: false }))
    }

    /// `element.supertype`, com o padrão do `setDefaultSupertypes` e do
    /// `buildEnumChildren`.
    fn supertipo<'p>(p: &mut dyn Provedor<'p>, c: ClassId, dados: &ClassTypeData) -> Option<TypeId> {
        let prog = p.programa();
        let k = prog.class(c);
        match k.kind {
            ClassKind::Class | ClassKind::MixinApplication => {
                if Some(c) == p.core().object_class {
                    return None;
                }
                // Só um tipo de interface de classe vale como superclasse
                // (`_isInterfaceTypeClass`); o resto cai em `Object`.
                let escrito = dados.supertype.filter(|&t| match p.tabela().get(t) {
                    Type::Interface { class, .. } => matches!(prog.class(*class).kind, ClassKind::Class | ClassKind::MixinApplication),
                    _ => false,
                });
                escrito.or_else(|| Self::tipo_object(p))
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
                    None => Self::tipo_object(p),
                }
            }
            _ => None,
        }
    }

    /// As classes que `InterfaceElementImpl._implementationsOfGetter` (e os
    /// de método e setter) percorre: a própria, os mixins de trás para a
    /// frente e então a superclasse, até `Object`
    /// (an362:src/dart/element/element.dart:5423-5470).
    pub fn cadeia_de_implementacoes<'p>(p: &mut dyn Provedor<'p>, c: ClassId) -> Vec<ClassId> {
        let mut cadeia = Vec::new();
        let mut vistas = std::collections::HashSet::new();
        let mut atual = Some(c);
        while let Some(k) = atual {
            if !vistas.insert(k) {
                break;
            }
            cadeia.push(k);
            let dados = p.dados_da_classe(k);
            for &mt in dados.mixins.iter().rev() {
                if let Some((m, _)) = Self::substituicao(p, mt) {
                    cadeia.push(m);
                }
            }
            atual = Self::supertipo(p, k, &dados).and_then(|st| Self::substituicao(p, st)).map(|(s, _)| s);
        }
        cadeia
    }

    /// Os membros de instância declarados (`_getTypeMembers`): métodos,
    /// depois acessores, na ordem da fonte, com a covariância escrita e a
    /// herdada dos `candidatos` (`_inferParameterCovariance`).
    fn declarados<'p>(p: &mut dyn Provedor<'p>, c: ClassId, candidatos: &Mapa<Vec<Membro>>) -> Mapa<Membro> {
        let prog = p.programa();
        let interner = p.interner();
        let k = prog.class(c);
        let mut membros: Vec<(SymbolId, FunctionElementId)> = k.instance_members.iter().map(|(&s, &f)| (s, f)).collect();
        membros.sort_by_key(|&(_, f)| (especie_de(prog, f) != Especie::Metodo, f));
        let mut mapa = Mapa::default();
        for (chave, f) in membros {
            let nome = Nome::novo(interner, k.library, chave);
            let tipo = p.tipo_do_membro(f);
            let mut covariantes = covariancia_escrita(prog, f);
            if let Some(sobrescritos) = candidatos.get(&nome) {
                for d in parametros_do_tipo(p.tabela(), tipo) {
                    if covariantes.contains(&d) {
                        continue;
                    }
                    let herda = sobrescritos.iter().any(|o| {
                        // `_getCorrespondingParameter`: o nomeado pelo nome, o
                        // posicional pela posição (se lá for posicional).
                        o.covariantes.contains(&d) && parametros_do_tipo(p.tabela(), o.tipo).contains(&d)
                    });
                    if herda {
                        covariantes.push(d);
                    }
                }
            }
            mapa.insert(
                nome,
                Membro {
                    funcao: f,
                    classe: c,
                    tipo,
                    especie: especie_de(prog, f),
                    abstrato: abstrato(prog, f),
                    covariantes: covariantes.into(),
                    sintetico: false,
                },
            );
        }
        mapa
    }

    /// `_getInterfaceClass`.
    fn de_classe<'p>(&mut self, p: &mut dyn Provedor<'p>, c: ClassId) -> Interface {
        let prog = p.programa();
        let dados = p.dados_da_classe(c);
        let mut candidatos: Mapa<Vec<Membro>> = Mapa::default();
        let mut super_impl: Vec<Mapa<Membro>> = Vec::new();
        let mut implementados: Mapa<Membro> = Mapa::default();
        let mut interface_do_super: Option<Rc<Interface>> = None;
        if let Some(st) = Self::supertipo(p, c, &dados)
            && let Some((sc, s)) = Self::substituicao(p, st)
        {
            let si = self.interface(p, sc);
            Self::adicionar_candidatos(p, &mut candidatos, &s, &si);
            for (n, m) in si.implemented.iter() {
                let m = Self::substituir(p, m, &s);
                implementados.insert(*n, m);
            }
            super_impl.push(implementados.clone());
            interface_do_super = Some(si);
        }
        let mut conflitos_de_mixins: Vec<Vec<Conflito>> = Vec::new();
        for &mt in dados.mixins.iter() {
            let Some((mc, s)) = Self::substituicao(p, mt) else { continue };
            let mi = self.interface(p, mc);
            let mut de_super_e_mixin: Mapa<Vec<Membro>> = Mapa::default();
            let mut conflitos_do_mixin: Vec<Conflito> = Vec::new();
            for (n, m0) in mi.map.iter() {
                let candidato = Self::substituir(p, m0, &s);
                let atual = match candidatos.get(n) {
                    None => {
                        candidatos.insert(*n, vec![candidato]);
                        continue;
                    }
                    Some(l) => l[0].clone(),
                };
                if candidato.classe == mc {
                    if atual.especie != candidato.especie {
                        let (getter, metodo) = if atual.especie == Especie::Getter { (atual, candidato.clone()) } else { (candidato.clone(), atual) };
                        conflitos_do_mixin.push(Conflito::GetterMetodo { nome: *n, getter, metodo });
                    }
                    candidatos.insert(*n, vec![candidato]);
                } else {
                    de_super_e_mixin.insert(*n, vec![atual, candidato]);
                }
            }
            let mut mapa = Mapa::default();
            mais_especificos(p, c, &mut mapa, &de_super_e_mixin, true);
            for (n, m) in mapa.iter() {
                candidatos.insert(*n, vec![m.clone()]);
            }
            conflitos_de_mixins.push(conflitos_do_mixin);
            // `_addMixinMembers`.
            let object = p.core().object_class;
            for (n, m) in mi.implemented.iter() {
                if m.abstrato || Some(m.classe) == object {
                    continue;
                }
                let m = Self::substituir(p, m, &s);
                implementados.insert(*n, m);
            }
            super_impl.push(implementados.clone());
        }
        for &it in dados.interfaces.iter() {
            let Some((ic, s)) = Self::substituicao(p, it) else { continue };
            let ii = self.interface(p, ic);
            Self::adicionar_candidatos(p, &mut candidatos, &s, &ii);
        }
        let declarados = Self::declarados(p, c, &candidatos);
        // `_addImplemented`.
        for (n, m) in declarados.iter() {
            if !m.abstrato {
                implementados.insert(*n, m.clone());
            }
        }
        let mut mapa = declarados.clone();
        let mut conflitos = mais_especificos(p, c, &mut mapa, &candidatos, true);
        // Os encaminhadores de `noSuchMethod`.
        let k = prog.class(c);
        let mut encaminhados: HashSet<Nome> = HashSet::new();
        let classe_abstrata =
            matches!(k.kind, ClassKind::Class | ClassKind::MixinApplication) && (k.modifiers.abstract_ || k.modifiers.sealed);
        if classe_abstrata {
            if let Some(si) = &interface_do_super {
                encaminhados = si.encaminhados.clone();
            }
        } else if let Some(nsm) = p.interner().lookup("noSuchMethod") {
            let nome = Nome { chave: nsm, biblioteca: None };
            let object = p.core().object_class;
            if implementados.get(&nome).is_some_and(|m| Some(m.classe) != object) {
                let do_super = interface_do_super.as_ref().map(|s| &s.encaminhados);
                for (n, m) in mapa.iter() {
                    if !implementados.contains_key(n) || do_super.is_some_and(|s| s.contains(n)) {
                        implementados.insert(*n, m.clone());
                        encaminhados.insert(*n);
                    }
                }
            }
        }
        for cs in conflitos_de_mixins {
            conflitos.extend(cs);
        }
        // `_inheritCovariance`.
        let mut com_covariancia: Mapa<Membro> = Mapa::default();
        for (n, m) in implementados.iter() {
            let r = herdar_covariancia(p, c, &candidatos, n, m);
            com_covariancia.insert(*n, r);
        }
        Interface {
            map: mapa,
            declared: declarados,
            implemented: com_covariancia,
            encaminhados,
            overridden: candidatos,
            redeclared: Mapa::default(),
            super_implemented: super_impl,
            conflitos,
        }
    }

    /// `_getInterfaceMixin`.
    fn de_mixin<'p>(&mut self, p: &mut dyn Provedor<'p>, c: ClassId) -> Interface {
        let dados = p.dados_da_classe(c);
        let mut restricoes: Vec<TypeId> = dados.on.to_vec();
        if restricoes.is_empty()
            && let Some(o) = Self::tipo_object(p)
        {
            restricoes.push(o);
        }
        let mut candidatos_do_super: Mapa<Vec<Membro>> = Mapa::default();
        for t in restricoes {
            let Some((rc, s)) = Self::substituicao(p, t) else { continue };
            let ri = self.interface(p, rc);
            Self::adicionar_candidatos(p, &mut candidatos_do_super, &s, &ri);
        }
        let mut interface_do_super: Mapa<Membro> = Mapa::default();
        let conflitos_do_super = mais_especificos(p, c, &mut interface_do_super, &candidatos_do_super, true);
        let mut candidatos = candidatos_do_super.clone();
        for &it in dados.interfaces.iter() {
            let Some((ic, s)) = Self::substituicao(p, it) else { continue };
            let ii = self.interface(p, ic);
            Self::adicionar_candidatos(p, &mut candidatos, &s, &ii);
        }
        let declarados = Self::declarados(p, c, &candidatos);
        let mut mapa = declarados.clone();
        let conflitos_da_interface = mais_especificos(p, c, &mut mapa, &candidatos, true);
        let mut implementados: Mapa<Membro> = Mapa::default();
        for (n, m) in declarados.iter() {
            if !m.abstrato {
                implementados.insert(*n, m.clone());
            }
        }
        let mut conflitos = conflitos_do_super;
        conflitos.extend(conflitos_da_interface);
        Interface {
            map: mapa,
            declared: declarados,
            implemented: implementados,
            encaminhados: HashSet::new(),
            overridden: candidatos,
            redeclared: Mapa::default(),
            super_implemented: vec![interface_do_super],
            conflitos,
        }
    }

    /// `_getInterfaceExtensionType`.
    fn de_tipo_de_extensao<'p>(&mut self, p: &mut dyn Provedor<'p>, c: ClassId) -> Interface {
        let prog = p.programa();
        let interner = p.interner();
        let dados = p.dados_da_classe(c);
        let todos = Self::declarados(p, c, &Mapa::default());
        let mut declarados: Mapa<Membro> = Mapa::default();
        for (n, m) in todos.iter() {
            if !m.abstrato {
                declarados.insert(*n, m.clone());
            }
        }
        // Os nomes excluídos.
        let mut nomes_excluidos: HashSet<Nome> = HashSet::new();
        let mut metodos_excluidos: HashSet<Nome> = HashSet::new();
        let mut setters_excluidos: HashSet<Nome> = HashSet::new();
        for (n, m) in declarados.iter() {
            nomes_excluidos.insert(*n);
            match m.especie {
                Especie::Metodo => {
                    if let Some(s) = n.de_setter(interner) {
                        setters_excluidos.insert(s);
                    }
                }
                Especie::Setter => {
                    if let Some(g) = n.de_getter(interner) {
                        metodos_excluidos.insert(g);
                    }
                }
                Especie::Getter => {}
            }
        }
        let mut implementados = declarados.clone();
        // Os candidatos, separados entre membros de tipo de extensão e os
        // outros (`_ExtensionTypeCandidates`: métodos, getters, setters).
        #[derive(Default, Clone)]
        struct Candidatos {
            metodos: Vec<Membro>,
            getters: Vec<Membro>,
            setters: Vec<Membro>,
        }
        impl Candidatos {
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
        let nao_excluidos = |n: &Nome, cs: &Candidatos| -> Vec<Membro> {
            if nomes_excluidos.contains(n) {
                return Vec::new();
            }
            let mut v = Vec::new();
            if !metodos_excluidos.contains(n) {
                v.extend(cs.metodos.iter().cloned());
            }
            v.extend(cs.getters.iter().cloned());
            if !setters_excluidos.contains(n) {
                v.extend(cs.setters.iter().cloned());
            }
            v
        };
        let mut de_extensao: Mapa<Candidatos> = Mapa::default();
        let mut outros: Mapa<Candidatos> = Mapa::default();
        for &it in dados.interfaces.iter() {
            let Some((ic, s)) = Self::substituicao(p, it) else { continue };
            let ii = self.interface(p, ic);
            for (n, m) in ii.map.iter() {
                let m = Self::substituir(p, m, &s);
                let alvo = if prog.class(m.classe).kind == ClassKind::ExtensionType { &mut de_extensao } else { &mut outros };
                match alvo.get_mut(n) {
                    Some(cs) => cs.adicionar(m),
                    None => {
                        let mut cs = Candidatos::default();
                        cs.adicionar(m);
                        alvo.insert(*n, cs);
                    }
                }
            }
        }
        let mut redeclarados: Mapa<Vec<Membro>> = Mapa::default();
        let mut conflitos: Vec<Conflito> = Vec::new();
        for (n, cs) in de_extensao.iter() {
            match redeclarados.get_mut(n) {
                Some(l) => l.extend(cs.todos()),
                None => redeclarados.insert(*n, cs.todos()),
            }
            let livres = nao_excluidos(n, cs);
            if livres.is_empty() {
                continue;
            }
            if let Some(ne) = outros.get(n) {
                let ne_livres = nao_excluidos(n, ne);
                if !ne_livres.is_empty() {
                    conflitos.push(Conflito::ExtensaoENaoExtensao { nome: *n, nao_extensao: ne_livres, extensao: livres });
                }
                continue;
            }
            // O herdado tem de ser um só.
            let unico = livres.iter().all(|m| m.funcao == livres[0].funcao);
            if !unico {
                conflitos.push(Conflito::ExtensaoNaoUnica { nome: *n, candidatos: livres });
                continue;
            }
            implementados.insert(*n, livres[0].clone());
        }
        for (n, cs) in outros.iter() {
            match redeclarados.get_mut(n) {
                Some(l) => l.extend(cs.todos()),
                None => redeclarados.insert(*n, cs.todos()),
            }
            let livres = nao_excluidos(n, cs);
            if livres.is_empty() || de_extensao.contains_key(n) {
                continue;
            }
            match combinar(p, c, &livres, true, *n, None) {
                Some(m) => implementados.insert(*n, m),
                None => conflitos.push(Conflito::Candidatos { nome: *n, candidatos: livres }),
            }
        }
        // Os redeclarados sem repetição.
        let mut unicos: Mapa<Vec<Membro>> = Mapa::default();
        for (n, l) in redeclarados.iter() {
            if l.len() <= 1 {
                unicos.insert(*n, l.clone());
            } else {
                let mut vistos: Vec<FunctionElementId> = Vec::new();
                let mut v = Vec::new();
                for m in l {
                    if !vistos.contains(&m.funcao) {
                        vistos.push(m.funcao);
                        v.push(m.clone());
                    }
                }
                unicos.insert(*n, v);
            }
        }
        Interface {
            map: implementados.clone(),
            declared: declarados,
            implemented: implementados,
            encaminhados: HashSet::new(),
            overridden: Mapa::default(),
            redeclared: unicos,
            super_implemented: Vec::new(),
            conflitos,
        }
    }
}

/// `_checkForGetterMethodConflict`.
fn conflito_getter_metodo(nome: Nome, candidatos: &[Membro]) -> Option<Conflito> {
    let getter = candidatos.iter().find(|m| m.especie == Especie::Getter)?;
    let metodo = candidatos.iter().find(|m| m.especie == Especie::Metodo)?;
    Some(Conflito::GetterMetodo { nome, getter: getter.clone(), metodo: metodo.clone() })
}

/// `_findMostSpecificFromNamedCandidates`.
fn mais_especificos<'p>(p: &mut dyn Provedor<'p>, alvo: ClassId, mapa: &mut Mapa<Membro>, candidatos: &Mapa<Vec<Membro>>, top_merge: bool) -> Vec<Conflito> {
    let mut conflitos = Vec::new();
    for (n, lista) in candidatos.iter() {
        if lista.len() > 1
            && let Some(c) = conflito_getter_metodo(*n, lista)
        {
            conflitos.push(c);
            continue;
        }
        if mapa.contains_key(n) {
            continue;
        }
        if let Some(m) = combinar(p, alvo, lista, top_merge, *n, Some(&mut conflitos)) {
            mapa.insert(*n, m);
        }
    }
    conflitos
}

/// `combineSignatures`.
pub fn combinar<'p>(
    p: &mut dyn Provedor<'p>,
    alvo: ClassId,
    candidatos: &[Membro],
    top_merge: bool,
    nome: Nome,
    conflitos: Option<&mut Vec<Conflito>>,
) -> Option<Membro> {
    if candidatos.len() == 1 {
        return Some(candidatos[0].clone());
    }
    let mut validos: Vec<Membro> = Vec::new();
    for (i, c) in candidatos.iter().enumerate() {
        let _ = i;
        let ok = candidatos.iter().all(|d| p.sub(c.tipo, d.tipo));
        if ok {
            validos.push(c.clone());
        }
    }
    if validos.is_empty() {
        if let Some(cs) = conflitos {
            cs.push(Conflito::Candidatos { nome, candidatos: candidatos.to_vec() });
        }
        return None;
    }
    if top_merge { Some(topo(p, alvo, &validos)) } else { Some(validos[0].clone()) }
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
            let (Type::Function { ret: ra, positional: pa, optional: oa, named: na, nullable: la, .. }, Type::Function { ret: rr, positional: pb, optional: ob, named: nb, nullable: lb, .. }) =
                (table.get(ca).clone(), table.get(rb).clone())
            else {
                return false;
            };
            let limites = ta.iter().zip(tb.iter()).all(|(x, y)| {
                let lx = table.param(*x).bound;
                let ly = ops::substitute(table.param(*y).bound, &s, table);
                table.canonico(lx) == table.canonico(ly)
            });
            limites && la == lb && table.canonico(ra) == table.canonico(rr) && pa == pb && oa == ob && na == nb
        }
        _ => false,
    }
}

/// `_topMerge`.
fn topo<'p>(p: &mut dyn Provedor<'p>, alvo: ClassId, validos: &[Membro]) -> Membro {
    let primeiro = &validos[0];
    if validos.len() == 1 {
        return primeiro.clone();
    }
    let todos_iguais = validos.iter().all(|m| iguais(p.tabela(), m.tipo, primeiro.tipo));
    if todos_iguais {
        return primeiro.clone();
    }
    // `reduce` do `topMerge` sobre os tipos normalizados; os parâmetros do
    // resultado guardam a covariância do operando da esquerda.
    let mut resultado = p.normalizar(primeiro.tipo);
    let covariantes = primeiro.covariantes.clone();
    for m in &validos[1..] {
        let t = p.normalizar(m.tipo);
        resultado = top_merge_funcao(p, resultado, t, &covariantes, &m.covariantes);
    }
    for m in validos {
        if iguais(p.tabela(), m.tipo, resultado) {
            return m.clone();
        }
    }
    Membro {
        funcao: primeiro.funcao,
        classe: alvo,
        tipo: resultado,
        especie: primeiro.especie,
        abstrato: false,
        covariantes,
        sintetico: true,
    }
}

/// `_inheritCovariance`.
fn herdar_covariancia<'p>(p: &mut dyn Provedor<'p>, c: ClassId, candidatos: &Mapa<Vec<Membro>>, nome: &Nome, m: &Membro) -> Membro {
    if m.classe == c {
        return m.clone();
    }
    let parametros = parametros_do_tipo(p.tabela(), m.tipo);
    if parametros.is_empty() {
        return m.clone();
    }
    let Some(lista) = candidatos.get(nome) else { return m.clone() };
    let mut covariantes: HashSet<Param> = HashSet::new();
    for o in lista {
        for d in parametros_do_tipo(p.tabela(), o.tipo) {
            if o.covariantes.contains(&d) {
                covariantes.insert(d);
            }
        }
    }
    if covariantes.is_empty() {
        return m.clone();
    }
    let devem: Vec<Param> = parametros.iter().copied().filter(|d| covariantes.contains(d)).collect();
    let mudou = parametros.iter().any(|d| m.covariantes.contains(d) != devem.contains(d));
    if !mudou {
        return m.clone();
    }
    match m.especie {
        Especie::Metodo | Especie::Setter => Membro { classe: c, covariantes: devem.into(), sintetico: true, ..m.clone() },
        Especie::Getter => m.clone(),
    }
}

// -- `NNBD_TOP_MERGE` -----------------------------------------------------------

fn anulavel(table: &TypeTable, t: TypeId) -> bool {
    table.get(t).is_declared_nullable()
}

fn com_anulabilidade(table: &mut TypeTable, t: TypeId, nullable: bool) -> TypeId {
    let novo = match table.get(t).clone() {
        Type::Interface { class, args, .. } => Type::Interface { class, args, nullable },
        Type::Function { type_params, ret, positional, optional, named, .. } => Type::Function { type_params, ret, positional, optional, named, nullable },
        Type::Record { positional, named, .. } => Type::Record { positional, named, nullable },
        Type::TypeParameter { param, .. } => Type::TypeParameter { param, nullable },
        Type::FutureOr { arg, .. } => Type::FutureOr { arg, nullable },
        Type::ExtensionType { decl, args, .. } => Type::ExtensionType { decl, args, nullable },
        _ => return t,
    };
    table.intern(novo)
}

/// `topMerge` de dois tipos quaisquer (os tipos de função aninhados não têm
/// parâmetros covariantes).
fn top_merge<'p>(p: &mut dyn Provedor<'p>, t: TypeId, s: TypeId) -> TypeId {
    top_merge_funcao(p, t, s, &[], &[])
}

/// `TopMergeHelper.topMerge`, com a covariância dos parâmetros quando `t` e
/// `s` são os tipos de dois membros.
fn top_merge_funcao<'p>(p: &mut dyn Provedor<'p>, t: TypeId, s: TypeId, cov_t: &[Param], cov_s: &[Param]) -> TypeId {
    let object = p.core().object_class;
    let (tt, st) = (p.tabela().get(t).clone(), p.tabela().get(s).clone());
    let t_obj_q = matches!(tt, Type::Interface { class, nullable: true, .. } if Some(class) == object);
    let s_obj_q = matches!(st, Type::Interface { class, nullable: true, .. } if Some(class) == object);
    if t_obj_q && s_obj_q {
        return t;
    }
    let invalido_t = p.tabela().e_invalido(t);
    let invalido_s = p.tabela().e_invalido(s);
    let t_dyn = matches!(tt, Type::Dynamic) && !invalido_t;
    let s_dyn = matches!(st, Type::Dynamic) && !invalido_s;
    if t_dyn && s_dyn {
        return p.core().dynamic_;
    }
    if invalido_t {
        return t;
    }
    if invalido_s {
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
        let tn = com_anulabilidade(p.tabela(), t, false);
        let sn = com_anulabilidade(p.tabela(), s, false);
        let r = top_merge_funcao(p, tn, sn, cov_t, cov_s);
        return com_anulabilidade(p.tabela(), r, true);
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
        (Type::Record { positional: pa, named: na, .. }, Type::Record { positional: pb, named: nb, .. }) => {
            if pa.len() != pb.len() || na.len() != nb.len() {
                return t;
            }
            let pos: Vec<TypeId> = pa.iter().zip(pb.iter()).map(|(x, y)| top_merge(p, *x, *y)).collect();
            let mut nomeados: Vec<(SymbolId, TypeId)> = Vec::new();
            for ((n1, x), (n2, y)) in na.iter().zip(nb.iter()) {
                if n1 != n2 {
                    return t;
                }
                nomeados.push((*n1, top_merge(p, *x, *y)));
            }
            p.tabela().intern(Type::Record { positional: pos.into_boxed_slice(), named: nomeados.into_boxed_slice(), nullable: false })
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
    let mut novos: Vec<TypeParamId> = Vec::new();
    let mut sub_t: HashMap<TypeParamId, TypeId> = HashMap::new();
    let mut sub_s: HashMap<TypeParamId, TypeId> = HashMap::new();
    if !tp_t.is_empty() {
        for (a, b) in tp_t.iter().zip(tp_s.iter()) {
            let nome = p.tabela().param(*a).name;
            let limite_padrao = p.core().object_nullable;
            let n = p.tabela().alloc_type_param(nome, TypeParamOwner::GenericFunctionType, limite_padrao, Variance::Unspecified);
            let tn = p.tabela().intern(Type::TypeParameter { param: n, nullable: false });
            sub_t.insert(*a, tn);
            sub_s.insert(*b, tn);
            novos.push(n);
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
            p.tabela().set_type_param_bound(novos[i], l);
        }
    }
    let ret = juntar(p, rt, rs, &sub_t, &sub_s);
    if pt.len() != ps.len() || ot.len() != os.len() || nt.len() != ns.len() {
        return t;
    }
    let mut posicionais = Vec::with_capacity(pt.len());
    for (i, (a, b)) in pt.iter().zip(ps.iter()).enumerate() {
        posicionais.push(parametro(p, Param::Indice(i), *a, *b, cov_t, cov_s, &sub_t, &sub_s));
    }
    let mut opcionais = Vec::with_capacity(ot.len());
    for (i, (a, b)) in ot.iter().zip(os.iter()).enumerate() {
        opcionais.push(parametro(p, Param::Indice(pt.len() + i), *a, *b, cov_t, cov_s, &sub_t, &sub_s));
    }
    let mut nomeados = Vec::with_capacity(nt.len());
    for ((n1, a, r1), (n2, b, r2)) in nt.iter().zip(ns.iter()) {
        if n1 != n2 {
            return t;
        }
        let ty = parametro(p, Param::Nome(*n1), *a, *b, cov_t, cov_s, &sub_t, &sub_s);
        nomeados.push((*n1, ty, *r1 || *r2));
    }
    p.tabela().intern(Type::Function {
        type_params: novos.into_boxed_slice(),
        ret,
        positional: posicionais.into_boxed_slice(),
        optional: opcionais.into_boxed_slice(),
        named: nomeados.into_boxed_slice(),
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
