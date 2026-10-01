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
use super::valor::{double_como_dart, Estado, Valor};
use crate::table::{Type, TypeId, TypeParamId};
use dartforge_elements::model::{ClassId, ClassKind, FunctionKind, LibraryId, VariableId};
use dartforge_frontend::ast::{self, ExprId, ListPatternElement, PatternId, PatternKind};
use std::collections::HashMap;
use std::rc::Rc;

// -- Tipos estáticos (`fe76:static_type.dart`, `fe76:types.dart`) ------------

type St = usize;
const OBJ_ANUL: St = 0;
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
    Unico(usize),
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
    fn livre(&self) -> bool {
        match self {
            Restr::Livre => true,
            Restr::Id(_) => false,
            Restr::Lista { tam, resto, .. } => *resto && *tam == 0,
            Restr::Mapa { chaves, .. } => chaves.is_empty(),
        }
    }
}

#[derive(Clone, Debug)]
enum Especie {
    Comum,
    Bool,
    Enum(ClassId),
    Selado(ClassId),
    FutureOr { arg: St, fut: St },
    Registro,
    ListaTipo,
    /// `ListPatternStaticType` (o nome já tem o texto dos argumentos).
    ListaPadrao,
    /// `MapPatternStaticType`: as chaves na ordem escrita, com o texto.
    MapaPadrao { chaves: Vec<(usize, String)> },
    ValorBool,
    ElementoEnum,
    /// `GeneralValueStaticType` (constante comum ou o desconhecido `?`).
    Geral,
}

#[derive(Clone, Debug)]
enum Tipo {
    /// `NullableStaticType(u)`; `OBJ_ANUL` e `NULL` são deste formato.
    Anulavel(St),
    Objeto,
    Never,
    Base { ty: TypeId, especie: Especie, restr: Restr, nome: String, implicitamente_anulavel: bool },
    Embrulhado(St, St),
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum ChaveTipo {
    Anulavel(St),
    Base(TypeId, Restr),
    Embrulhado(St, St),
}

// -- Chaves (`fe76:key.dart`) --------------------------------------------------

#[derive(Clone, Debug)]
enum Chave {
    Mapa { id: usize, texto: String },
    Cabeca(usize),
    Cauda(usize),
    Resto(usize, usize),
    /// `NameKey`; `registro` é `RecordNameKey`.
    Nome { nome: Rc<str>, registro: bool },
    /// `RecordIndexKey` (nome `$i+1`).
    Indice(usize),
    Extensao { recv: St, nome: Rc<str>, tipo: St },
}

impl Chave {
    fn nome(&self) -> String {
        match self {
            Chave::Mapa { texto, .. } => format!("[{texto}]"),
            Chave::Cabeca(i) => format!("[{i}]"),
            Chave::Cauda(i) => format!("[{}]", -(*i as i64 + 1)),
            Chave::Resto(h, t) => format!("[{h}:{}]", -(*t as i64)),
            Chave::Nome { nome, .. } | Chave::Extensao { nome, .. } => nome.to_string(),
            Chave::Indice(i) => format!("${}", i + 1),
        }
    }

    fn de_registro(&self) -> bool {
        matches!(self, Chave::Indice(_) | Chave::Nome { registro: true, .. })
    }

    fn de_lista(&self) -> bool {
        matches!(self, Chave::Cabeca(_) | Chave::Cauda(_) | Chave::Resto(..))
    }

    /// A posição na ordem total de `compareTo`.
    fn grupo(&self) -> u8 {
        match self {
            Chave::Cabeca(_) => 0,
            Chave::Resto(..) => 1,
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
            (Chave::Resto(a, b), Chave::Resto(c, d)) => a == c && b == d,
            // `NameKey ==` só olha o nome: `RecordIndexKey(0)` é `NameKey('$1')`.
            (Chave::Nome { .. } | Chave::Indice(_), Chave::Nome { .. } | Chave::Indice(_)) => self.nome() == o.nome(),
            (Chave::Extensao { recv: a, nome: n, .. }, Chave::Extensao { recv: b, nome: m, .. }) => a == b && n == m,
            _ => false,
        }
    }
}

/// `Key.compareTo` (ordem só para escolher a testemunha de forma estável).
fn comparar_chaves(ex: &Exaustividade, a: &Chave, b: &Chave) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (ga, gb) = (a.grupo(), b.grupo());
    if ga != gb {
        return ga.cmp(&gb);
    }
    match (a, b) {
        (Chave::Cabeca(x), Chave::Cabeca(y)) => x.cmp(y),
        (Chave::Cauda(x), Chave::Cauda(y)) => y.cmp(x),
        (Chave::Resto(h1, t1), Chave::Resto(h2, t2)) => h1.cmp(h2).then(t2.cmp(t1)),
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
struct Caminho(Option<Rc<(Caminho, Chave)>>);

impl Caminho {
    fn mais(&self, k: Chave) -> Caminho {
        Caminho(Some(Rc::new((self.clone(), k))))
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

struct SimplesD {
    tipo: St,
    props: Vec<(Chave, Espaco)>,
    adicionais: Vec<(Chave, Espaco)>,
}

#[derive(Clone)]
struct Simples(Rc<SimplesD>);

impl Simples {
    fn novo(tipo: St) -> Simples {
        Simples(Rc::new(SimplesD { tipo, props: Vec::new(), adicionais: Vec::new() }))
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

    fn adicional(&self, k: &Chave) -> Option<&Espaco> {
        self.0.adicionais.iter().find(|(c, _)| c == k).map(|(_, e)| e)
    }
}

struct EspacoD {
    caminho: Caminho,
    simples: Vec<Simples>,
}

#[derive(Clone)]
struct Espaco(Rc<EspacoD>);

impl Espaco {
    fn novo(caminho: Caminho, tipo: St) -> Espaco {
        Espaco(Rc::new(EspacoD { caminho, simples: vec![Simples::novo(tipo)] }))
    }

    fn com(caminho: Caminho, tipo: St, props: Vec<(Chave, Espaco)>, adicionais: Vec<(Chave, Espaco)>) -> Espaco {
        Espaco(Rc::new(EspacoD { caminho, simples: vec![Simples(Rc::new(SimplesD { tipo, props, adicionais }))] }))
    }
}

/// Um predicado da testemunha (`fe76:witness.dart`).
#[derive(Clone)]
struct Predicado {
    caminho: Caminho,
    estatico: St,
    valor: St,
}

/// `PropertyWitness`.
struct Testemunha {
    estatico: St,
    valor: St,
    props: Vec<(Chave, Testemunha)>,
}

// -- O estado: tipos, valores, consultas ao sistema de tipos ------------------

/// O que a conversão de padrões lê da inferência e da avaliação.
pub(crate) struct Entrada<'x> {
    pub ast: &'x ast::Ast,
    pub source: &'x str,
    pub tipos_de_padroes: &'x HashMap<PatternId, TypeId>,
    pub campos_de_extensao: &'x HashMap<PatternId, TypeId>,
    pub padroes_invalidos: &'x std::collections::HashSet<PatternId>,
    pub valores_de_padroes: &'x HashMap<PatternId, Valor>,
    pub valores_de_chaves: &'x HashMap<ExprId, Valor>,
    /// A biblioteca tem a versão 3.3 ou maior (`createCastSpace`).
    pub versao_3_3: bool,
}

/// Um caso do `switch` para a verificação.
pub(crate) struct Caso {
    /// `None`: `default`.
    pub padrao: Option<PatternId>,
    pub guardado: bool,
}

/// O resultado de `_validateSwitchExhaustiveness`.
pub(crate) struct Resultado {
    /// Índices (em `casos`) dos casos inalcançáveis.
    pub inalcancaveis: Vec<usize>,
    /// A primeira testemunha, quando não é exaustivo (sem correção).
    pub testemunha: Option<String>,
    /// A primeira testemunha para a correção.
    pub correcao: Option<String>,
}

pub(crate) struct Exaustividade<'m, 'a> {
    m: &'m mut Motor<'a>,
    tipos: Vec<Tipo>,
    internos: HashMap<ChaveTipo, St>,
    unicos: usize,
    valores: Vec<Valor>,
    subtipos: HashMap<St, Vec<St>>,
    lib: LibraryId,
    invalido: bool,
}

impl<'m, 'a> Exaustividade<'m, 'a> {
    pub(crate) fn nova(m: &'m mut Motor<'a>, lib: LibraryId) -> Self {
        let tipos = vec![Tipo::Anulavel(OBJ), Tipo::Objeto, Tipo::Anulavel(NEVER), Tipo::Never];
        let mut internos = HashMap::new();
        internos.insert(ChaveTipo::Anulavel(OBJ), OBJ_ANUL);
        internos.insert(ChaveTipo::Anulavel(NEVER), NULL);
        Exaustividade { m, tipos, internos, unicos: 0, valores: Vec::new(), subtipos: HashMap::new(), lib, invalido: false }
    }

    fn internar(&mut self, chave: ChaveTipo, t: impl FnOnce() -> Tipo) -> St {
        if let Some(&s) = self.internos.get(&chave) {
            return s;
        }
        let s = self.tipos.len();
        self.tipos.push(t());
        self.internos.insert(chave, s);
        s
    }

    fn base(&mut self, ty: TypeId, especie: Especie, restr: Restr, nome: String, impl_anul: bool) -> St {
        let chave = ChaveTipo::Base(ty, restr.clone());
        self.internar(chave, || Tipo::Base { ty, especie, restr, nome, implicitamente_anulavel: impl_anul })
    }

    fn embrulhado(&mut self, w: St, i: St) -> St {
        self.internar(ChaveTipo::Embrulhado(w, i), || Tipo::Embrulhado(w, i))
    }

    fn unico(&mut self, ty: TypeId, nome: &str) -> St {
        self.unicos += 1;
        let id = self.unicos;
        self.base(ty, Especie::Geral, Restr::Id(Ident::Unico(id)), nome.to_string(), false)
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

    fn anulavel_dart(&self, t: TypeId) -> bool {
        self.m.table.get(t).is_declared_nullable()
    }

    /// `promoteToNonNull`.
    fn nao_nulo_dart(&mut self, t: TypeId) -> TypeId {
        match self.m.table.get(t).clone() {
            Type::TypeParameter { param, nullable: false } => {
                let b = self.m.table.param(param).bound;
                let nb = self.nao_nulo_dart(b);
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

    fn como_instancia_de(&mut self, t: TypeId, c: Option<ClassId>) -> Option<TypeId> {
        let c = c?;
        let t = crate::ops::non_nullable(t, self.m.table);
        self.m.outline.hierarchy.supertype_of(t, c, self.m.table, self.m.core)
    }

    fn args_de(&self, t: TypeId) -> Vec<TypeId> {
        match self.m.table.get(t) {
            Type::Interface { args, .. } => args.to_vec(),
            _ => Vec::new(),
        }
    }

    /// `hasSimpleName`.
    fn nome_simples(&self, t: TypeId) -> bool {
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
    fn sobreaproximar(&mut self, t: TypeId) -> TypeId {
        self.substituir_parametros(t, 1, 0)
    }

    /// `variancia`: 1 covariante, -1 contravariante, 0 invariante.
    fn substituir_parametros(&mut self, t: TypeId, variancia: i8, prof: u32) -> TypeId {
        if prof > 16 {
            return t;
        }
        match self.m.table.get(t).clone() {
            Type::TypeParameter { param, .. } | Type::Intersection { param, .. } => {
                if variancia == -1 {
                    self.m.core.never
                } else {
                    let d = self.padrao_do_parametro(param);
                    self.substituir_parametros(d, variancia, prof + 1)
                }
            }
            Type::Interface { class, args, nullable } => {
                let args: Vec<TypeId> = args.iter().map(|&a| self.substituir_parametros(a, variancia, prof + 1)).collect();
                self.m.table.intern(Type::Interface { class, args: args.into_boxed_slice(), nullable })
            }
            Type::FutureOr { arg, nullable } => {
                let arg = self.substituir_parametros(arg, variancia, prof + 1);
                self.m.table.intern(Type::FutureOr { arg, nullable })
            }
            Type::Record { positional, named, nullable } => {
                let positional: Vec<TypeId> = positional.iter().map(|&a| self.substituir_parametros(a, variancia, prof + 1)).collect();
                let named: Vec<_> = named.iter().map(|&(n, a)| (n, self.substituir_parametros(a, variancia, prof + 1))).collect();
                self.m.table.intern(Type::Record { positional: positional.into_boxed_slice(), named: named.into_boxed_slice(), nullable })
            }
            Type::Function { type_params, ret, positional, optional, named, nullable } => {
                let ret = self.substituir_parametros(ret, variancia, prof + 1);
                let inv = -variancia;
                let positional: Vec<TypeId> = positional.iter().map(|&a| self.substituir_parametros(a, inv, prof + 1)).collect();
                let optional: Vec<TypeId> = optional.iter().map(|&a| self.substituir_parametros(a, inv, prof + 1)).collect();
                let named: Vec<_> = named.iter().map(|&(n, a, r)| (n, self.substituir_parametros(a, inv, prof + 1), r)).collect();
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
    fn padrao_do_parametro(&mut self, p: TypeParamId) -> TypeId {
        crate::ops::instanciar_para_limites(&[p], &[], self.m.table, self.m.core)[0]
    }

    // ---- `getStaticType` (`fe76:shared.dart:161-234`) ----

    fn tipo_estatico(&mut self, t: TypeId) -> St {
        let t = self.m.table.canonico(t);
        let t = self.m.apagar(t);
        match self.m.table.get(t).clone() {
            Type::Never => return NEVER,
            Type::Null => return NULL,
            Type::Dynamic => return OBJ_ANUL,
            Type::Interface { class, nullable, .. } if Some(class) == self.m.core.object_class => {
                return if nullable { OBJ_ANUL } else { OBJ };
            }
            _ => {}
        }
        let nn = self.nao_nulo_dart(t);
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
                let ia = self.anulavel_dart(arg) || matches!(self.m.table.get(arg), Type::Null);
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
                self.base(nn, Especie::Selado(class), Restr::Livre, n, false)
            }
            _ => {
                if self.como_instancia_de(nn, core.list_class).is_some() {
                    let n = self.texto(nn);
                    self.base(nn, Especie::ListaTipo, Restr::Livre, n, false)
                } else {
                    let n = self.texto(nn);
                    let s = self.base(nn, Especie::Comum, Restr::Livre, n, false);
                    let limite = match self.m.table.get(t).clone() {
                        Type::TypeParameter { param, .. } => Some(self.m.table.param(param).bound),
                        Type::Intersection { bound, .. } => Some(bound),
                        _ => None,
                    };
                    match limite {
                        Some(b) => {
                            let sb = self.tipo_estatico(b);
                            self.embrulhado(sb, s)
                        }
                        None => s,
                    }
                }
            }
        };
        if self.anulavel_dart(t) {
            st = self.anulavel(st);
        }
        st
    }

    // ---- operações de `StaticType` ----

    fn anulavel(&mut self, s: St) -> St {
        match self.tipos[s].clone() {
            Tipo::Never => NULL,
            Tipo::Objeto => OBJ_ANUL,
            Tipo::Anulavel(_) => s,
            Tipo::Base { .. } => self.internar(ChaveTipo::Anulavel(s), || Tipo::Anulavel(s)),
            Tipo::Embrulhado(w, i) => {
                let (nw, ni) = (self.anulavel(w), self.anulavel(i));
                if nw == w && ni == i {
                    s
                } else {
                    self.embrulhado(nw, ni)
                }
            }
        }
    }

    fn nao_nulo(&mut self, s: St) -> St {
        match self.tipos[s].clone() {
            Tipo::Anulavel(u) => u,
            Tipo::Embrulhado(w, i) => {
                let (nw, ni) = (self.nao_nulo(w), self.nao_nulo(i));
                if nw == w && ni == i {
                    s
                } else {
                    self.embrulhado(nw, ni)
                }
            }
            _ => s,
        }
    }

    fn selado(&self, s: St) -> bool {
        match &self.tipos[s] {
            Tipo::Anulavel(_) => s != NULL,
            Tipo::Base { especie, .. } => matches!(
                especie,
                Especie::Bool | Especie::Enum(_) | Especie::Selado(_) | Especie::FutureOr { .. } | Especie::ListaTipo
            ),
            Tipo::Embrulhado(w, _) => self.selado(*w),
            _ => false,
        }
    }

    fn implicitamente_anulavel(&self, s: St) -> bool {
        match &self.tipos[s] {
            Tipo::Anulavel(_) => true,
            Tipo::Objeto | Tipo::Never => false,
            Tipo::Base { implicitamente_anulavel, .. } => *implicitamente_anulavel,
            Tipo::Embrulhado(w, _) => self.implicitamente_anulavel(*w),
        }
    }

    fn nome(&self, s: St) -> String {
        match &self.tipos[s] {
            _ if s == NULL => "Null".to_string(),
            Tipo::Anulavel(u) => {
                if self.implicitamente_anulavel(*u) {
                    self.nome(*u)
                } else {
                    format!("{}?", self.nome(*u))
                }
            }
            Tipo::Objeto => "Object".to_string(),
            Tipo::Never => "Never".to_string(),
            Tipo::Base { nome, .. } => nome.clone(),
            Tipo::Embrulhado(w, _) => self.nome(*w),
        }
    }

    /// `isSubtypeOf`.
    fn sub(&mut self, a: St, b: St) -> bool {
        if a == b || b == OBJ_ANUL {
            return true;
        }
        if a == NEVER {
            return true;
        }
        if let Tipo::Embrulhado(w, i) = self.tipos[b].clone() {
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
            Tipo::Embrulhado(w, i) => self.sub(w, b) || self.sub(i, b),
            Tipo::Base { ty, restr, .. } => {
                if b == OBJ {
                    return true;
                }
                match self.tipos[b].clone() {
                    Tipo::Anulavel(u2) => self.sub(a, u2),
                    Tipo::Base { ty: ty2, restr: r2, .. } => self.sub_dart(ty, ty2) && self.restricao_sub(&restr, &r2),
                    _ => false,
                }
            }
        }
    }

    fn restricao_sub(&mut self, a: &Restr, b: &Restr) -> bool {
        if b.livre() {
            return true;
        }
        match (a, b) {
            (Restr::Livre, _) => false,
            (Restr::Id(x), Restr::Id(y)) => x == y,
            (Restr::Lista { el, tam, resto }, Restr::Lista { el: el2, tam: tam2, resto: resto2 }) => {
                if !self.sub_dart(*el, *el2) {
                    return false;
                }
                if *resto2 {
                    tam >= tam2
                } else if *resto {
                    false
                } else {
                    tam == tam2
                }
            }
            (Restr::Mapa { k, v, chaves }, Restr::Mapa { k: k2, v: v2, chaves: c2 }) => {
                self.sub_dart(*k, *k2) && self.sub_dart(*v, *v2) && c2.iter().all(|c| chaves.contains(c))
            }
            _ => false,
        }
    }

    /// `getSubtypes`.
    fn subtipos(&mut self, s: St, chaves: &[Chave]) -> Vec<St> {
        match self.tipos[s].clone() {
            Tipo::Anulavel(u) => {
                if s == NULL {
                    Vec::new()
                } else {
                    vec![u, NULL]
                }
            }
            Tipo::Embrulhado(w, i) => {
                if let (Tipo::Anulavel(wu), Tipo::Anulavel(iu)) = (self.tipos[w].clone(), self.tipos[i].clone()) {
                    let e = self.embrulhado(wu, iu);
                    return vec![e, NULL];
                }
                let subs = self.subtipos(w, chaves);
                subs.into_iter().map(|x| self.embrulhado(x, i)).collect()
            }
            Tipo::Base { ty, especie, .. } => match especie {
                Especie::Bool => {
                    let t = self.base(ty, Especie::ValorBool, Restr::Id(Ident::Bool(true)), "true".into(), false);
                    let f = self.base(ty, Especie::ValorBool, Restr::Id(Ident::Bool(false)), "false".into(), false);
                    vec![t, f]
                }
                Especie::FutureOr { arg, fut } => vec![arg, fut],
                Especie::Enum(c) => {
                    if let Some(v) = self.subtipos.get(&s) {
                        return v.clone();
                    }
                    let v = self.elementos_de_enum(s, ty, c);
                    self.subtipos.insert(s, v.clone());
                    v
                }
                Especie::Selado(c) => {
                    if let Some(v) = self.subtipos.get(&s) {
                        return v.clone();
                    }
                    let v = self.subclasses_seladas(s, ty, c);
                    self.subtipos.insert(s, v.clone());
                    v
                }
                Especie::ListaTipo => self.subtipos_de_lista(ty, chaves),
                _ => Vec::new(),
            },
            _ => Vec::new(),
        }
    }

    /// `EnumStaticType._createEnumElements` (`fe76:types/enum.dart`).
    fn elementos_de_enum(&mut self, s: St, ty: TypeId, c: ClassId) -> Vec<St> {
        let constantes = self.m.program.class(c).enum_constants.clone();
        let sobre = self.sobreaproximar(ty);
        let mut v = Vec::new();
        for var in constantes {
            let el = self.elemento_de_enum(c, var);
            let Tipo::Base { ty: tel, .. } = self.tipos[el].clone() else { continue };
            if self.sub_dart(tel, sobre) {
                v.push(self.embrulhado(el, s));
            }
        }
        v
    }

    fn elemento_de_enum(&mut self, c: ClassId, var: VariableId) -> St {
        let ty = self.m.outline.variables[var.0 as usize]
            .declared_type
            .or(self.m.outline.variables[var.0 as usize].inferred)
            .unwrap_or_else(|| {
                let args = Vec::new();
                self.m.table.intern(Type::Interface { class: c, args: args.into_boxed_slice(), nullable: false })
            });
        let nome = format!(
            "{}.{}",
            self.m.interner.resolve(self.m.program.class(c).name),
            self.m.interner.resolve(self.m.program.variables[var.0 as usize].name)
        );
        self.base(ty, Especie::ElementoEnum, Restr::Id(Ident::Enum(var)), nome, false)
    }

    /// `AnalyzerSealedClassOperations.getDirectSubclasses` e
    /// `SealedClassStaticType._createSubtypes` (`fe76:types/sealed.dart`).
    fn subclasses_seladas(&mut self, s: St, ty: TypeId, selada: ClassId) -> Vec<St> {
        let program = self.m.program;
        let lib = program.class(selada).library;
        let mut diretas = Vec::new();
        for (i, k) in program.classes.iter().enumerate() {
            let id = ClassId(i as u32);
            if k.library != lib || id == selada || k.decl.is_none() || k.kind == ClassKind::ExtensionType {
                continue;
            }
            let mut sup = k.supertype_class;
            // A cadeia sintética de `extends B with M` leva à superclasse escrita.
            let mut mixins: Vec<ClassId> = k.mixin_classes.clone();
            let mut guarda = 0;
            while let Some(sc) = sup {
                guarda += 1;
                if program.class(sc).decl.is_some() || guarda > 50 {
                    break;
                }
                mixins.extend(program.class(sc).mixin_classes.iter().copied());
                sup = program.class(sc).supertype_class;
            }
            if sup == Some(selada)
                || mixins.contains(&selada)
                || k.interface_classes.contains(&selada)
                || (k.kind == ClassKind::Mixin && k.on_classes.contains(&selada))
            {
                diretas.push(id);
            }
        }
        let sobre = self.sobreaproximar(ty);
        let mut v = Vec::new();
        for sub in diretas {
            let Some(st) = self.subclasse_como_instancia(sub, ty) else { continue };
            if self.args_de(st).is_empty() && !self.sub_dart(st, sobre) {
                continue;
            }
            let e = self.tipo_estatico(st);
            v.push(self.embrulhado(e, s));
        }
        v
    }

    /// `getSubclassAsInstanceOf`.
    fn subclasse_como_instancia(&mut self, sub: ClassId, selado: TypeId) -> Option<TypeId> {
        let params = self.m.outline.classes[sub.0 as usize].type_params.to_vec();
        let args_this: Vec<TypeId> =
            params.iter().map(|&p| self.m.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
        let this = self.m.table.intern(Type::Interface { class: sub, args: args_this.clone().into_boxed_slice(), nullable: false });
        let Type::Interface { class: cs, args: args_sel, .. } = self.m.table.get(selado).clone() else { return None };
        let como = self.m.outline.hierarchy.supertype_of(this, cs, self.m.table, self.m.core)?;
        if params.is_empty() {
            return Some(this);
        }
        let args_como = self.args_de(como);
        let mut trivial = args_this.len() == args_como.len() && args_this.iter().zip(args_como.iter()).all(|(a, b)| a == b);
        if trivial && args_sel.len() == params.len() {
            let mapa: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(args_sel.iter().copied()).collect();
            for (i, &p) in params.iter().enumerate() {
                let d = self.m.table.param(p).clone();
                if d.explicito {
                    let b = crate::ops::substitute(d.bound, &mapa, self.m.table);
                    if !self.sub_dart(args_sel[i], b) {
                        trivial = false;
                        break;
                    }
                }
            }
        } else {
            trivial = false;
        }
        if trivial {
            Some(self.m.table.intern(Type::Interface { class: sub, args: args_sel.clone(), nullable: false }))
        } else {
            Some(self.sobreaproximar(this))
        }
    }

    /// `ListTypeStaticType.getSubtypes` (`fe76:types/list.dart`).
    fn subtipos_de_lista(&mut self, ty: TypeId, chaves: &[Chave]) -> Vec<St> {
        let (mut cab, mut cau) = (0usize, 0usize);
        for k in chaves {
            match k {
                Chave::Cabeca(i) if *i >= cab => cab = i + 1,
                Chave::Cauda(i) if *i >= cau => cau = i + 1,
                _ => {}
            }
        }
        let max = cab + cau;
        let el = self.como_instancia_de(ty, self.m.core.list_class).and_then(|l| self.args_de(l).first().copied()).unwrap_or(self.m.core.dynamic_);
        let texto = if matches!(self.m.table.get(el), Type::Dynamic) { String::new() } else { format!("<{}>", self.texto(el)) };
        let mut v = Vec::new();
        for tam in 0..=max {
            let resto = tam == max;
            let nome = texto_de_lista(&texto, tam, resto);
            v.push(self.base(ty, Especie::ListaPadrao, Restr::Lista { el, tam, resto }, nome, false));
        }
        v
    }

    // ---- propriedades ----

    /// O tipo (Dart) do membro de instância `nome` de `t`
    /// (`_getInterfaceFieldTypes`, `an611:src/generated/exhaustiveness.dart:389-417`).
    fn membro(&mut self, t: TypeId, nome: &str) -> Option<TypeId> {
        let t = crate::ops::non_nullable(t, self.m.table);
        let (classe, args) = match self.m.table.get(t).clone() {
            Type::Interface { class, args, .. } => (class, args.to_vec()),
            Type::Record { positional, named, .. } => {
                if let Some(i) = nome.strip_prefix('$').and_then(|x| x.parse::<usize>().ok()) {
                    if i >= 1 && i <= positional.len() {
                        return Some(positional[i - 1]);
                    }
                }
                for (n, x) in named.iter() {
                    if self.m.interner.resolve(*n) == nome {
                        return Some(*x);
                    }
                }
                let o = self.m.core.object;
                return self.membro(o, nome);
            }
            _ => {
                let o = self.m.core.object;
                if t == o {
                    return None;
                }
                return self.membro(o, nome);
            }
        };
        let sym = self.m.interner.lookup(nome)?;
        let privado = nome.starts_with('_');
        let mut candidatos: Vec<TypeId> = Vec::new();
        let mut supers = vec![t];
        if let Some(h) = self.m.outline.hierarchy.get(classe) {
            let mapa: HashMap<TypeParamId, TypeId> = h.type_params.iter().copied().zip(args.iter().copied()).collect();
            let todos = h.all_supertypes.clone();
            for s in todos {
                supers.push(crate::ops::substitute(s, &mapa, self.m.table));
            }
        }
        if let Some(o) = self.m.core.object_class {
            if classe != o {
                supers.push(self.m.core.object);
            }
        }
        for s in supers {
            let Type::Interface { class: k, args: ka, .. } = self.m.table.get(s).clone() else { continue };
            let Some(&f) = self.m.program.class(k).instance_members.get(&sym) else { continue };
            let fe = self.m.program.function(f);
            if privado && fe.library != self.lib {
                continue;
            }
            let bruto = match fe.kind {
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
            let mapa: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(ka.iter().copied()).collect();
            candidatos.push(crate::ops::substitute(bruto, &mapa, self.m.table));
        }
        // O mais específico (o sobrescritor covariante).
        let mut melhor = *candidatos.first()?;
        for &c in &candidatos[1..] {
            if c != melhor && self.sub_dart(c, melhor) && !self.sub_dart(melhor, c) {
                melhor = c;
            }
        }
        Some(melhor)
    }

    /// `getObjectFieldType`.
    fn campo_de_object(&mut self, k: &Chave) -> Option<St> {
        if !matches!(k, Chave::Nome { .. } | Chave::Indice(_)) {
            return None;
        }
        let o = self.m.core.object;
        let t = self.membro(o, &k.nome())?;
        Some(self.tipo_estatico(t))
    }

    /// `fields[key]` de um `TypeBasedStaticType` (`getFieldTypes`).
    fn campo_de_tipo(&mut self, ty: TypeId, k: &Chave) -> Option<St> {
        if !matches!(k, Chave::Nome { .. } | Chave::Indice(_)) {
            return None;
        }
        let t = self.membro(ty, &k.nome())?;
        Some(self.tipo_estatico(t))
    }

    /// `StaticType.fields[key]`.
    fn campos(&mut self, s: St, k: &Chave) -> Option<St> {
        match self.tipos[s].clone() {
            Tipo::Base { ty, .. } => self.campo_de_tipo(ty, k),
            Tipo::Embrulhado(w, _) => self.campos(w, k),
            _ => None,
        }
    }

    /// `getPropertyType`.
    fn propriedade(&mut self, s: St, k: &Chave) -> Option<St> {
        match self.tipos[s].clone() {
            Tipo::Base { ty, .. } => self.campo_de_tipo(ty, k),
            Tipo::Embrulhado(w, _) => self.propriedade(w, k),
            _ => self.campo_de_object(k),
        }
    }

    /// `getAdditionalPropertyType`.
    fn adicional(&mut self, s: St, k: &Chave) -> Option<St> {
        let Tipo::Base { ty, .. } = self.tipos[s].clone() else { return None };
        let core = self.m.core;
        match k {
            Chave::Mapa { .. } => {
                let m = self.como_instancia_de(ty, core.map_class)?;
                let v = *self.args_de(m).get(1)?;
                Some(self.tipo_estatico(v))
            }
            Chave::Cabeca(_) | Chave::Cauda(_) => {
                let l = self.como_instancia_de(ty, core.list_class)?;
                let e = *self.args_de(l).first()?;
                Some(self.tipo_estatico(e))
            }
            Chave::Resto(..) => {
                let l = self.como_instancia_de(ty, core.list_class)?;
                Some(self.tipo_estatico(l))
            }
            _ => self.campo_de_object(k),
        }
    }

    // ---- o algoritmo (`fe76:exhaustive.dart`) ----

    fn nao_casados(&mut self, linhas: &[Vec<Espaco>], valores: &[Espaco], preds: &[Predicado], varias: bool) -> Option<Vec<Vec<Predicado>>> {
        if valores.is_empty() {
            if !linhas.is_empty() {
                return None;
            }
            return Some(vec![preds.to_vec()]);
        }
        let primeiro = valores[0].clone();
        let mut interesse: Vec<Chave> = Vec::new();
        for l in linhas {
            for s in &l[0].0.simples {
                for (k, _) in &s.0.adicionais {
                    if !interesse.contains(k) {
                        interesse.push(k.clone());
                    }
                }
            }
        }
        for valor in primeiro.0.simples.clone() {
            let contexto = valor.0.tipo;
            let mut pilha = std::collections::VecDeque::from([contexto]);
            let mut testemunhas: Option<Vec<Vec<Predicado>>> = None;
            while let Some(t) = pilha.pop_front() {
                if self.sub(t, NEVER) {
                    continue;
                }
                if self.selado(t) {
                    let r = self.filtrar_por_tipo(contexto, t, linhas, &valor, valores, preds, &primeiro.0.caminho, false);
                    if r.is_some() {
                        let subs = self.subtipos(t, &interesse);
                        pilha.extend(subs);
                    }
                } else {
                    let r = self.filtrar_por_tipo(contexto, t, linhas, &valor, valores, preds, &primeiro.0.caminho, false);
                    if let Some(r) = r {
                        testemunhas.get_or_insert_with(Vec::new).extend(r);
                        if !varias {
                            return testemunhas;
                        }
                    }
                }
            }
            if testemunhas.is_some() {
                return testemunhas;
            }
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    fn filtrar_por_tipo(
        &mut self,
        contexto: St,
        t: St,
        linhas: &[Vec<Espaco>],
        valor: &Simples,
        valores: &[Espaco],
        preds: &[Predicado],
        caminho: &Caminho,
        varias: bool,
    ) -> Option<Vec<Vec<Predicado>>> {
        let mut estendida = preds.to_vec();
        estendida.push(Predicado { caminho: caminho.clone(), estatico: contexto, valor: t });
        let mut primeiros: Vec<Simples> = Vec::new();
        let mut restantes: Vec<Vec<Espaco>> = Vec::new();
        for l in linhas {
            for s in &l[0].0.simples {
                if self.sub(t, s.0.tipo) {
                    primeiros.push(s.clone());
                    restantes.push(l.clone());
                }
            }
        }
        let mut chaves: Vec<Chave> = Vec::new();
        let mut adicionais: Vec<Chave> = Vec::new();
        for s in std::iter::once(valor).chain(primeiros.iter()) {
            for (k, _) in &s.0.props {
                if !chaves.contains(k) {
                    chaves.push(k.clone());
                }
            }
            for (k, _) in &s.0.adicionais {
                if !adicionais.contains(k) {
                    adicionais.push(k.clone());
                }
            }
        }
        chaves.sort_by(|a, b| comparar_chaves(self, a, b));
        adicionais.sort_by(|a, b| comparar_chaves(self, a, b));
        let mut novos_valores = self.expandir(&chaves, &adicionais, valor, t, caminho);
        novos_valores.extend(valores[1..].iter().cloned());
        let mut novas_linhas = Vec::with_capacity(restantes.len());
        for (i, l) in restantes.iter().enumerate() {
            let s = &primeiros[i];
            let mut nl = self.expandir(&chaves, &adicionais, s, s.0.tipo, caminho);
            nl.extend(l[1..].iter().cloned());
            novas_linhas.push(nl);
        }
        self.nao_casados(&novas_linhas, &novos_valores, &estendida, varias)
    }

    fn expandir(&mut self, chaves: &[Chave], adicionais: &[Chave], s: &Simples, t: St, caminho: &Caminho) -> Vec<Espaco> {
        let mut r = Vec::new();
        for k in chaves {
            if let Some(e) = s.prop(k) {
                r.push(e.clone());
            } else {
                let mut pt = self.propriedade(t, k);
                if pt.is_none() {
                    if let Chave::Extensao { tipo, .. } = k {
                        pt = Some(*tipo);
                    }
                }
                r.push(Espaco::novo(caminho.mais(k.clone()), pt.unwrap_or(OBJ_ANUL)));
            }
        }
        for k in adicionais {
            if let Some(e) = s.adicional(k) {
                r.push(e.clone());
            } else {
                let pt = self.adicional(t, k);
                r.push(Espaco::novo(caminho.mais(k.clone()), pt.unwrap_or(OBJ_ANUL)));
            }
        }
        r
    }

    /// `computeExhaustiveness`.
    fn calcular(&mut self, valor: St, guardados: &[bool], espacos: &[Espaco]) -> (Vec<usize>, Option<Vec<Vec<Predicado>>>) {
        let mut linhas: Vec<Vec<Espaco>> = Vec::new();
        let mut inalcancaveis = Vec::new();
        for (i, e) in espacos.iter().enumerate() {
            let linha = vec![e.clone()];
            if i > 0 && self.nao_casados(&linhas, &linha, &[], false).is_none() {
                inalcancaveis.push(i);
            }
            if !guardados[i] {
                linhas.push(linha);
            }
        }
        let raiz = Espaco::novo(Caminho::default(), valor);
        let t = self.nao_casados(&linhas, &[raiz], &[], true);
        (inalcancaveis, t)
    }

    // ---- espaços dos padrões (`SpaceCreator`, `PatternConverter`) ----

    fn com_contexto(&mut self, contexto: St, t: TypeId, nao_nulo: bool) -> St {
        let mut s = self.tipo_estatico(t);
        if self.sub(contexto, s) {
            s = contexto;
        }
        if nao_nulo {
            s = self.nao_nulo(s);
        }
        s
    }

    fn uniao(&mut self, a: &Espaco, b: &Espaco) -> Espaco {
        let mut todos = a.0.simples.clone();
        todos.extend(b.0.simples.iter().cloned());
        self.de_simples(a.0.caminho.clone(), todos)
    }

    /// `Space.fromSingleSpaces`.
    fn de_simples(&mut self, caminho: Caminho, simples: Vec<Simples>) -> Espaco {
        let vazio = Simples::novo(NEVER);
        let mut conjunto: Vec<Simples> = Vec::new();
        for s in simples {
            if s.igual(&vazio) {
                continue;
            }
            if !conjunto.iter().any(|x| x.igual(&s)) {
                conjunto.push(s);
            }
        }
        if conjunto.is_empty() {
            conjunto.push(vazio);
        } else if conjunto.len() == 2 {
            let (a, b) = (conjunto[0].clone(), conjunto[1].clone());
            if a.0.tipo == NULL && a.0.props.is_empty() && b.0.props.is_empty() {
                let n = self.anulavel(b.0.tipo);
                conjunto = vec![Simples::novo(n)];
            } else if b.0.tipo == NULL && b.0.props.is_empty() && a.0.props.is_empty() {
                let n = self.anulavel(a.0.tipo);
                conjunto = vec![Simples::novo(n)];
            }
        }
        Espaco(Rc::new(EspacoD { caminho, simples: conjunto }))
    }

    fn padrao(&mut self, e: &Entrada<'_>, caminho: Caminho, contexto: St, p: PatternId, nao_nulo: bool) -> Espaco {
        if e.padroes_invalidos.contains(&p) {
            self.invalido = true;
        }
        match &e.ast.pattern(p).kind {
            PatternKind::Variable { final_, var_, ty, .. } => {
                let Some(&t) = e.tipos_de_padroes.get(&p) else {
                    // `case nome:` é um padrão constante: o valor da
                    // constante de topo, se o verificador o achou; senão a
                    // verificação fica de fora, como com um tipo inválido.
                    let _ = (final_, var_, ty);
                    if let Some(v) = e.valores_de_padroes.get(&p).cloned() {
                        return self.valor_constante(&v, caminho);
                    }
                    self.invalido = true;
                    let d = self.desconhecido();
                    return Espaco::novo(caminho, d);
                };
                let s = self.com_contexto(contexto, t, nao_nulo);
                Espaco::novo(caminho, s)
            }
            PatternKind::Wildcard { ty } => {
                let s = match ty.and_then(|_| e.tipos_de_padroes.get(&p).copied()) {
                    None => {
                        if nao_nulo {
                            self.nao_nulo(contexto)
                        } else {
                            contexto
                        }
                    }
                    Some(t) => self.com_contexto(contexto, t, nao_nulo),
                };
                Espaco::novo(caminho, s)
            }
            PatternKind::Object { fields, .. } => {
                let Some(&t) = e.tipos_de_padroes.get(&p) else {
                    self.invalido = true;
                    let d = self.desconhecido();
                    return Espaco::novo(caminho, d);
                };
                let s = self.com_contexto(contexto, t, nao_nulo);
                let mut campos: Vec<(Rc<str>, PatternId, Option<TypeId>)> = Vec::new();
                for f in fields.iter() {
                    let nome = match f.name {
                        Some(n) => Some(n.sym),
                        None => nome_implicito(e.ast, f.pattern),
                    };
                    let Some(nome) = nome else { continue };
                    let nome: Rc<str> = self.m.interner.resolve(nome).into();
                    let ext = e.campos_de_extensao.get(&f.pattern).copied();
                    if let Some(x) = campos.iter_mut().find(|x| x.0 == nome) {
                        x.1 = f.pattern;
                        x.2 = ext;
                    } else {
                        campos.push((nome, f.pattern, ext));
                    }
                }
                let mut props: Vec<(Chave, Espaco)> = Vec::new();
                for (nome, sub, ext) in campos {
                    let (chave, pt) = match ext {
                        Some(et) => {
                            let pt = self.tipo_estatico(et);
                            let recv = self.tipo_estatico(t);
                            (Chave::Extensao { recv, nome, tipo: pt }, pt)
                        }
                        None => {
                            let k = Chave::Nome { nome, registro: false };
                            let pt = self.propriedade(s, &k).unwrap_or(OBJ_ANUL);
                            (k, pt)
                        }
                    };
                    let es = self.padrao(e, caminho.mais(chave.clone()), pt, sub, false);
                    props.push((chave, es));
                }
                Espaco::com(caminho, s, props, Vec::new())
            }
            PatternKind::Record { fields } => {
                let d = self.m.core.dynamic_;
                let mut pos: Vec<PatternId> = Vec::new();
                let mut nomeados: Vec<(Rc<str>, PatternId)> = Vec::new();
                for f in fields.iter() {
                    let implicito = f.name.is_none() && e.source.as_bytes().get(f.span.start) == Some(&b':');
                    if f.name.is_none() && !implicito {
                        pos.push(f.pattern);
                    } else {
                        let nome = f.name.map(|n| n.sym).or_else(|| nome_implicito(e.ast, f.pattern));
                        let Some(nome) = nome else { continue };
                        let nome: Rc<str> = self.m.interner.resolve(nome).into();
                        if let Some(x) = nomeados.iter_mut().find(|x| x.0 == nome) {
                            x.1 = f.pattern;
                        } else {
                            nomeados.push((nome, f.pattern));
                        }
                    }
                }
                let mut named: Vec<(dartforge_intern::SymbolId, TypeId)> = Vec::new();
                for (n, _) in &nomeados {
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
                let s = self.com_contexto(contexto, rt, true);
                let mut props = Vec::new();
                for (i, sub) in pos.into_iter().enumerate() {
                    let k = Chave::Indice(i);
                    let pt = self.propriedade(s, &k).unwrap_or(OBJ_ANUL);
                    let es = self.padrao(e, caminho.mais(k.clone()), pt, sub, false);
                    props.push((k, es));
                }
                for (nome, sub) in nomeados {
                    let k = Chave::Nome { nome, registro: true };
                    let pt = self.propriedade(s, &k).unwrap_or(OBJ_ANUL);
                    let es = self.padrao(e, caminho.mais(k.clone()), pt, sub, false);
                    props.push((k, es));
                }
                Espaco::com(caminho, s, props, Vec::new())
            }
            PatternKind::Or(x, y) => {
                let (x, y) = (*x, *y);
                let a = self.padrao(e, caminho.clone(), contexto, x, nao_nulo);
                let b = self.padrao(e, caminho, contexto, y, nao_nulo);
                self.uniao(&a, &b)
            }
            PatternKind::And(x, y) => {
                let (x, y) = (*x, *y);
                let a = self.padrao(e, caminho.clone(), contexto, x, nao_nulo);
                let b = self.padrao(e, caminho.clone(), contexto, y, nao_nulo);
                self.intersecao(caminho, &a, &b)
            }
            PatternKind::NullCheck(x) => self.padrao(e, caminho, contexto, *x, true),
            PatternKind::Parenthesized(x) => self.padrao(e, caminho, contexto, *x, nao_nulo),
            PatternKind::NullAssert(x) => {
                let s = self.padrao(e, caminho.clone(), contexto, *x, true);
                let n = Espaco::novo(caminho, NULL);
                self.uniao(&s, &n)
            }
            PatternKind::Cast { pattern, .. } => {
                let sub = *pattern;
                let Some(&t) = e.tipos_de_padroes.get(&p) else {
                    self.invalido = true;
                    let d = self.desconhecido();
                    return Espaco::novo(caminho, d);
                };
                self.cast(e, caminho, contexto, t, sub, nao_nulo)
            }
            PatternKind::Relational { .. } => {
                let d = self.desconhecido();
                Espaco::novo(caminho, d)
            }
            PatternKind::List { type_args, elements } => {
                let Some(&t) = e.tipos_de_padroes.get(&p) else {
                    self.invalido = true;
                    let d = self.desconhecido();
                    return Espaco::novo(caminho, d);
                };
                let el = self.args_de(t).first().copied().unwrap_or(self.m.core.dynamic_);
                let mut cabeca = Vec::new();
                let mut cauda = Vec::new();
                let mut resto: Option<Option<PatternId>> = None;
                for x in elements.iter() {
                    match x {
                        ListPatternElement::Rest(r) => resto = Some(*r),
                        ListPatternElement::Pattern(q) if resto.is_some() => cauda.push(*q),
                        ListPatternElement::Pattern(q) => cabeca.push(*q),
                    }
                }
                let texto = if type_args.is_empty() { String::new() } else { format!("<{}>", self.texto(el)) };
                let (nc, nt) = (cabeca.len(), cauda.len());
                let tem_resto = resto.is_some();
                let nome = texto_de_lista(&texto, nc + nt, tem_resto);
                let tt = crate::ops::non_nullable(t, self.m.table);
                let mut s = self.base(tt, Especie::ListaPadrao, Restr::Lista { el, tam: nc + nt, resto: tem_resto }, nome, false);
                if self.anulavel_dart(t) {
                    s = self.anulavel(s);
                }
                let mut ad = Vec::new();
                for (i, q) in cabeca.into_iter().enumerate() {
                    let k = Chave::Cabeca(i);
                    let pt = self.adicional(s, &k).unwrap_or(OBJ_ANUL);
                    let es = self.padrao(e, caminho.mais(k.clone()), pt, q, false);
                    ad.push((k, es));
                }
                if let Some(r) = resto {
                    let k = Chave::Resto(nc, nt);
                    let pt = self.adicional(s, &k).unwrap_or(OBJ_ANUL);
                    let es = match r {
                        Some(q) => self.padrao(e, caminho.mais(k.clone()), pt, q, false),
                        None => Espaco::novo(caminho.mais(k.clone()), pt),
                    };
                    ad.push((k, es));
                }
                for i in 0..nt {
                    let k = Chave::Cauda(i);
                    let pt = self.adicional(s, &k).unwrap_or(OBJ_ANUL);
                    let es = self.padrao(e, caminho.mais(k.clone()), pt, cauda[nt - i - 1], false);
                    ad.push((k, es));
                }
                Espaco::com(caminho, s, Vec::new(), ad)
            }
            PatternKind::Map { type_args, entries, .. } => {
                let Some(&t) = e.tipos_de_padroes.get(&p) else {
                    self.invalido = true;
                    let d = self.desconhecido();
                    return Espaco::novo(caminho, d);
                };
                let a = self.args_de(t);
                let (k, v) = (a.first().copied().unwrap_or(self.m.core.dynamic_), a.get(1).copied().unwrap_or(self.m.core.dynamic_));
                let mut chaves: Vec<(usize, String, PatternId)> = Vec::new();
                for en in entries.iter() {
                    let Some(val) = e.valores_de_chaves.get(&en.key).cloned() else {
                        let d = self.desconhecido();
                        return Espaco::novo(caminho, d);
                    };
                    let id = self.id_de_valor(&val);
                    let texto = self.texto_de_valor(&val);
                    if let Some(x) = chaves.iter_mut().find(|x| x.0 == id) {
                        x.2 = en.value;
                    } else {
                        chaves.push((id, texto, en.value));
                    }
                }
                let texto = if type_args.len() == 2 { format!("<{}, {}>", self.texto(k), self.texto(v)) } else { String::new() };
                let mut ids: Vec<usize> = chaves.iter().map(|x| x.0).collect();
                ids.sort_unstable();
                ids.dedup();
                let mut nome = format!("{texto}{{");
                for (i, (_, tx, _)) in chaves.iter().enumerate() {
                    if i > 0 {
                        nome.push_str(", ");
                    }
                    nome.push_str(&format!("{tx}: ()"));
                }
                nome.push('}');
                let tt = crate::ops::non_nullable(t, self.m.table);
                let especie = Especie::MapaPadrao { chaves: chaves.iter().map(|x| (x.0, x.1.clone())).collect() };
                let s = self.base(tt, especie, Restr::Mapa { k, v, chaves: ids }, nome, false);
                let mut ad = Vec::new();
                for (id, texto, q) in chaves {
                    let ch = Chave::Mapa { id, texto };
                    let pt = self.adicional(s, &ch).unwrap_or(OBJ_ANUL);
                    let es = self.padrao(e, caminho.mais(ch.clone()), pt, q, false);
                    ad.push((ch, es));
                }
                Espaco::com(caminho, s, Vec::new(), ad)
            }
            PatternKind::Constant(_) => match e.valores_de_padroes.get(&p).cloned() {
                Some(v) => self.valor_constante(&v, caminho),
                None => {
                    self.invalido = true;
                    let d = self.desconhecido();
                    Espaco::novo(caminho, d)
                }
            },
        }
    }

    /// `createCastSpace` (`fe76:shared.dart:570-632`).
    fn cast(&mut self, e: &Entrada<'_>, caminho: Caminho, contexto: St, t: TypeId, sub: PatternId, nao_nulo: bool) -> Espaco {
        let mut espaco = self.padrao(e, caminho.clone(), contexto, sub, nao_nulo);
        let ct = self.tipo_estatico(t);
        if e.versao_3_3 {
            if self.contido_em(ct, &espaco) {
                espaco = Espaco::novo(caminho.clone(), contexto);
            }
            if !self.potencialmente_anulavel(t) {
                let n = Espaco::novo(caminho, NULL);
                espaco = self.uniao(&espaco, &n);
            }
        } else if self.sub(ct, contexto) && self.selado(contexto) {
            for s in self.selados_expandidos(contexto) {
                if !self.sub(ct, s) && !self.sub(s, ct) {
                    let n = Espaco::novo(caminho.clone(), s);
                    espaco = self.uniao(&espaco, &n);
                }
            }
        }
        espaco
    }

    fn potencialmente_anulavel(&mut self, t: TypeId) -> bool {
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
    fn selados_expandidos(&mut self, t: St) -> Vec<St> {
        if !self.selado(t) {
            return vec![t];
        }
        let mut r = Vec::new();
        for s in self.subtipos(t, &[]) {
            for x in self.selados_expandidos(s) {
                if !r.contains(&x) {
                    r.push(x);
                }
            }
        }
        r
    }

    fn irrestrito(&mut self, s: &Simples) -> bool {
        for (k, e) in s.0.props.clone() {
            let ft = self.campos(s.0.tipo, &k).unwrap_or(NEVER);
            if !self.contido_em(ft, &e) {
                return false;
            }
        }
        for (k, e) in s.0.adicionais.clone() {
            let ft = self.adicional(s.0.tipo, &k).unwrap_or(NEVER);
            if !self.contido_em(ft, &e) {
                return false;
            }
        }
        true
    }

    /// `_isContainedIn`.
    fn contido_em(&mut self, t: St, e: &Espaco) -> bool {
        if e.0.simples.len() == 1 {
            let s = e.0.simples[0].clone();
            if self.irrestrito(&s) && self.sub(t, s.0.tipo) {
                return true;
            }
        }
        for sub in self.selados_expandidos(t) {
            let mut achou = false;
            for s in e.0.simples.clone() {
                if self.irrestrito(&s) && self.sub(sub, s.0.tipo) {
                    achou = true;
                    break;
                }
            }
            if !achou {
                return false;
            }
        }
        true
    }

    /// `_createSpaceIntersection`.
    fn intersecao(&mut self, caminho: Caminho, a: &Espaco, b: &Espaco) -> Espaco {
        let mut simples = Vec::new();
        let mut desconhecido = false;
        for x in a.0.simples.clone() {
            for y in b.0.simples.clone() {
                match self.intersecao_simples(&caminho, &x, &y) {
                    Some(s) => simples.push(s),
                    None => desconhecido = true,
                }
            }
        }
        if desconhecido {
            let d = self.desconhecido();
            simples.push(Simples::novo(d));
        }
        self.de_simples(caminho, simples)
    }

    fn intersecao_simples(&mut self, caminho: &Caminho, a: &Simples, b: &Simples) -> Option<Simples> {
        let tipo = if self.sub(a.0.tipo, b.0.tipo) {
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
                    let i = self.intersecao(caminho.mais(k.clone()), &ea, &eb);
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
        Some(Simples(Rc::new(SimplesD { tipo, props, adicionais: Vec::new() })))
    }

    // ---- constantes (`_convertConstantValue`) ----

    fn id_de_valor(&mut self, v: &Valor) -> usize {
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
    fn texto_de_valor(&self, v: &Valor) -> String {
        match &v.estado {
            Estado::Bool(Some(b)) => b.to_string(),
            Estado::Int(Some(i)) => i.to_string(),
            Estado::Double(Some(d)) => double_como_dart(*d),
            Estado::Str(Some(s)) => format!("'{}'", String::from_utf16_lossy(s)),
            Estado::Null { .. } => "null".to_string(),
            Estado::Simbolo(Some(s)) => format!("#{s}"),
            Estado::Tipo(Some(t)) => self.texto(*t),
            Estado::Lista { elementos, .. } | Estado::Conjunto { elementos, .. } => {
                let abre = if matches!(v.estado, Estado::Lista { .. }) { '[' } else { '{' };
                let fecha = if abre == '[' { ']' } else { '}' };
                let partes: Vec<String> = elementos.iter().map(|x| format!("{} ({})", self.texto(x.tipo), self.texto_de_valor(x))).collect();
                format!("{abre}{}{fecha}", partes.join(", "))
            }
            Estado::Registro { posicionais, nomeados } => {
                let mut partes: Vec<String> = posicionais.iter().map(|x| format!("{} ({})", self.texto(x.tipo), self.texto_de_valor(x))).collect();
                let mut n: Vec<(String, String)> = nomeados
                    .iter()
                    .map(|(s, x)| (self.m.interner.resolve(*s).to_string(), format!("{} ({})", self.texto(x.tipo), self.texto_de_valor(x))))
                    .collect();
                n.sort();
                if !n.is_empty() {
                    let nomeados: Vec<String> = n.into_iter().map(|(a, b)| format!("{a}: {b}")).collect();
                    partes.push(format!("{{{}}}", nomeados.join(", ")));
                }
                format!("({})", partes.join(", "))
            }
            _ => "-unknown-".to_string(),
        }
    }

    fn valor_constante(&mut self, v: &Valor, caminho: Caminho) -> Espaco {
        if v.estado.desconhecido() {
            // Valor opaco (de biblioteca não inferida): o analyzer o conhece;
            // sem ele, a verificação inteira fica de fora.
            self.invalido = true;
            let d = self.desconhecido();
            return Espaco::novo(caminho, d);
        }
        match &v.estado {
            Estado::Null { .. } => return Espaco::novo(caminho, NULL),
            // Literal de tipo: a identidade depende do alias e da
            // normalização (`const (Alias)`); fica como espaço desconhecido,
            // que não cobre nada além de si (nunca relata a mais).
            Estado::Tipo(_) => {
                let d = self.desconhecido();
                return Espaco::novo(caminho, d);
            }
            Estado::Bool(Some(b)) => {
                let bt = self.m.core.bool_;
                let base = self.tipo_estatico(bt);
                let subs = self.subtipos(base, &[]);
                return Espaco::novo(caminho, if *b { subs[0] } else { subs[1] });
            }
            Estado::Registro { posicionais, nomeados } => {
                let mut props = Vec::new();
                for (i, x) in posicionais.iter().enumerate() {
                    let k = Chave::Indice(i);
                    let e = self.valor_constante(x, caminho.mais(k.clone()));
                    props.push((k, e));
                }
                for (n, x) in nomeados.iter() {
                    let k = Chave::Nome { nome: self.m.interner.resolve(*n).into(), registro: true };
                    let e = self.valor_constante(x, caminho.mais(k.clone()));
                    props.push((k, e));
                }
                let t = self.tipo_estatico(v.tipo);
                return Espaco::com(caminho, t, props, Vec::new());
            }
            _ => {}
        }
        if let Type::Interface { class, .. } = self.m.table.get(v.tipo).clone() {
            if self.m.program.class(class).kind == ClassKind::Enum {
                if let Estado::Generico { campos, .. } = &v.estado {
                    let indice = campos.iter().find_map(|(c, x)| match (c, &x.estado) {
                        (super::valor::Campo::Indice, Estado::Int(Some(i))) => Some(*i as usize),
                        _ => None,
                    });
                    if let Some(var) = indice.and_then(|i| self.m.program.class(class).enum_constants.get(i).copied()) {
                        let el = self.elemento_de_enum(class, var);
                        return Espaco::novo(caminho, el);
                    }
                }
            }
        }
        let lib = self.lib;
        let s = if self.m.igualdade_primitiva(v, lib) {
            let id = self.id_de_valor(v);
            let nome = self.texto_de_valor(v);
            let nn = crate::ops::non_nullable(v.tipo, self.m.table);
            let s = self.base(nn, Especie::Geral, Restr::Id(Ident::Valor(id)), nome, false);
            if self.anulavel_dart(v.tipo) {
                self.anulavel(s)
            } else {
                s
            }
        } else {
            self.desconhecido()
        };
        Espaco::novo(caminho, s)
    }

    // ---- testemunhas (`fe76:witness.dart`) ----

    fn construir_testemunha(&self, preds: &[Predicado]) -> Testemunha {
        let mut raiz = Testemunha { estatico: OBJ_ANUL, valor: OBJ_ANUL, props: Vec::new() };
        for p in preds {
            let mut aqui = &mut raiz;
            for k in p.caminho.lista() {
                let i = match aqui.props.iter().position(|(c, _)| *c == k) {
                    Some(i) => i,
                    None => {
                        aqui.props.push((k, Testemunha { estatico: OBJ_ANUL, valor: OBJ_ANUL, props: Vec::new() }));
                        aqui.props.len() - 1
                    }
                };
                aqui = &mut aqui.props[i].1;
            }
            aqui.estatico = p.estatico;
            aqui.valor = p.valor;
        }
        raiz
    }

    fn trivial(&mut self, w: &Testemunha) -> bool {
        if !self.sub(w.estatico, w.valor) {
            return false;
        }
        w.props.iter().all(|(_, p)| self.trivial(p))
    }

    /// `PropertyWitness.witnessToDart`.
    fn escrever_testemunha(&mut self, buf: &mut String, w: &Testemunha, correcao: bool) {
        if !w.props.is_empty() {
            let mut grupos: Vec<(St, Vec<(Chave, &Testemunha)>)> = Vec::new();
            for (k, p) in &w.props {
                if correcao && self.trivial(p) {
                    continue;
                }
                let t = match k {
                    Chave::Extensao { recv, .. } => *recv,
                    _ => w.valor,
                };
                match grupos.iter_mut().find(|g| g.0 == t) {
                    Some(g) => g.1.push((k.clone(), p)),
                    None => grupos.push((t, vec![(k.clone(), p)])),
                }
            }
            if !grupos.is_empty() {
                let mut e = "";
                for (t, campos) in grupos {
                    buf.push_str(e);
                    e = " && ";
                    self.tipo_para_dart(buf, t, &campos, correcao);
                }
                return;
            }
        }
        let v = w.valor;
        self.tipo_para_dart(buf, v, &[], correcao);
    }

    /// `StaticType.typeToDart`.
    fn escrever_tipo(&self, buf: &mut String, s: St) {
        match &self.tipos[s] {
            _ if s == NULL => buf.push_str("Null"),
            Tipo::Anulavel(u) => {
                self.escrever_tipo(buf, *u);
                if !self.implicitamente_anulavel(*u) {
                    buf.push('?');
                }
            }
            Tipo::Embrulhado(w, _) => self.escrever_tipo(buf, *w),
            _ => buf.push_str(&self.nome(s)),
        }
    }

    fn campos_extras(&mut self, buf: &mut String, campos: &[(Chave, &Testemunha)], correcao: bool, pular: impl Fn(&Chave) -> bool) {
        let mut inicio = " && Object(";
        let mut fim = "";
        let mut virgula = "";
        for (k, w) in campos {
            if pular(k) {
                continue;
            }
            buf.push_str(inicio);
            inicio = "";
            fim = ")";
            buf.push_str(virgula);
            virgula = ", ";
            buf.push_str(&k.nome());
            buf.push_str(": ");
            self.escrever_testemunha(buf, w, correcao);
        }
        buf.push_str(fim);
    }

    /// `StaticType.witnessToDart`.
    fn tipo_para_dart(&mut self, buf: &mut String, s: St, campos: &[(Chave, &Testemunha)], correcao: bool) {
        match self.tipos[s].clone() {
            Tipo::Embrulhado(w, _) => self.tipo_para_dart(buf, w, campos, correcao),
            Tipo::Base { ty, especie, .. } => match especie {
                Especie::Registro => {
                    buf.push('(');
                    let mut virgula = "";
                    let (pos, nomeados): (usize, Vec<String>) = match self.m.table.get(ty).clone() {
                        Type::Record { positional, named, .. } => {
                            let mut n: Vec<String> = named.iter().map(|(s, _)| self.m.interner.resolve(*s).to_string()).collect();
                            n.sort();
                            (positional.len(), n)
                        }
                        _ => (0, Vec::new()),
                    };
                    for i in 0..pos {
                        buf.push_str(virgula);
                        virgula = ", ";
                        match campos.iter().find(|(k, _)| *k == Chave::Indice(i)) {
                            Some((_, w)) => self.escrever_testemunha(buf, w, correcao),
                            None => buf.push('_'),
                        }
                    }
                    for n in nomeados {
                        buf.push_str(virgula);
                        virgula = ", ";
                        buf.push_str(&n);
                        buf.push_str(": ");
                        let k = Chave::Nome { nome: n.as_str().into(), registro: true };
                        match campos.iter().find(|(c, _)| *c == k) {
                            Some((_, w)) => self.escrever_testemunha(buf, w, correcao),
                            None => buf.push('_'),
                        }
                    }
                    buf.push(')');
                    self.campos_extras(buf, campos, correcao, |k| k.de_registro());
                }
                Especie::ValorBool | Especie::ElementoEnum | Especie::Geral => {
                    buf.push_str(&self.nome(s));
                    self.campos_extras(buf, campos, correcao, |k| k.de_registro());
                }
                Especie::ListaPadrao => {
                    let Tipo::Base { restr: Restr::Lista { tam, resto: tem_resto, .. }, .. } = self.tipos[s].clone() else { return };
                    let (mut cab, mut cau) = (0usize, 0usize);
                    let mut resto: Option<&Testemunha> = None;
                    for (k, w) in campos {
                        match k {
                            Chave::Cabeca(i) if *i >= cab => cab = i + 1,
                            // `key.index >= maxHeadSize` (sic, `fe76:types/list.dart:93`).
                            Chave::Cauda(i) if *i >= cab => cau = i + 1,
                            Chave::Resto(..) => resto = Some(*w),
                            _ => {}
                        }
                    }
                    if cab + cau < tam {
                        cab = tam - cau;
                    }
                    buf.push('[');
                    let mut virgula = "";
                    for i in 0..cab {
                        buf.push_str(virgula);
                        match campos.iter().find(|(k, _)| *k == Chave::Cabeca(i)) {
                            Some((_, w)) => self.escrever_testemunha(buf, w, correcao),
                            None => buf.push('_'),
                        }
                        virgula = ", ";
                    }
                    if tem_resto {
                        buf.push_str(virgula);
                        buf.push_str("...");
                        if let Some(w) = resto {
                            self.escrever_testemunha(buf, w, correcao);
                        }
                        virgula = ", ";
                    }
                    for i in (0..cau).rev() {
                        buf.push_str(virgula);
                        match campos.iter().find(|(k, _)| *k == Chave::Cauda(i)) {
                            Some((_, w)) => self.escrever_testemunha(buf, w, correcao),
                            None => buf.push('_'),
                        }
                        virgula = ", ";
                    }
                    buf.push(']');
                    self.campos_extras(buf, campos, correcao, |k| k.de_lista());
                }
                Especie::MapaPadrao { chaves } => {
                    buf.push('{');
                    let mut virgula = "";
                    for (id, texto) in chaves {
                        buf.push_str(virgula);
                        buf.push_str(&texto);
                        buf.push_str(": ");
                        let k = Chave::Mapa { id, texto: texto.clone() };
                        match campos.iter().find(|(c, _)| *c == k) {
                            Some((_, w)) => self.escrever_testemunha(buf, w, correcao),
                            None => buf.push('_'),
                        }
                        virgula = ", ";
                    }
                    buf.push('}');
                    self.campos_extras(buf, campos, correcao, |k| matches!(k, Chave::Mapa { .. }));
                }
                _ => {
                    if !self.nome_simples(ty) {
                        buf.push_str(&self.nome(s));
                        buf.push_str(" _");
                        self.campos_extras(buf, campos, correcao, |k| k.de_lista());
                    } else {
                        self.base_para_dart(buf, s, campos, correcao);
                    }
                }
            },
            _ => self.base_para_dart(buf, s, campos, correcao),
        }
    }

    /// `_BaseStaticType.witnessToDart`.
    fn base_para_dart(&mut self, buf: &mut String, s: St, campos: &[(Chave, &Testemunha)], correcao: bool) {
        if s == OBJ_ANUL && campos.is_empty() {
            buf.push('_');
        } else if s == NULL && campos.is_empty() {
            buf.push_str("null");
        } else {
            self.escrever_tipo(buf, s);
            buf.push('(');
            let mut virgula = "";
            for (k, w) in campos {
                buf.push_str(virgula);
                virgula = ", ";
                buf.push_str(&k.nome());
                buf.push_str(": ");
                self.escrever_testemunha(buf, w, correcao);
            }
            buf.push(')');
        }
    }

    /// `isAlwaysExhaustive` (`an611:src/dart/element/type_system.dart:832-874`).
    pub(crate) fn sempre_exaustivo(&mut self, t: TypeId, prof: u32) -> bool {
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
                a != t && self.sempre_exaustivo(a, prof + 1)
            }
            Type::FutureOr { arg, .. } => self.sempre_exaustivo(arg, prof + 1),
            Type::Intersection { param, bound } => {
                self.sempre_exaustivo(bound, prof + 1) || {
                    let b = self.m.table.param(param).bound;
                    self.m.table.param(param).explicito && self.sempre_exaustivo(b, prof + 1)
                }
            }
            Type::TypeParameter { param, .. } => {
                let b = self.m.table.param(param).bound;
                self.m.table.param(param).explicito && self.sempre_exaustivo(b, prof + 1)
            }
            Type::Record { positional, named, .. } => {
                let todos: Vec<TypeId> = positional.iter().copied().chain(named.iter().map(|(_, c)| *c)).collect();
                todos.into_iter().all(|c| self.sempre_exaustivo(c, prof + 1))
            }
            _ => false,
        }
    }

    /// `_validateSwitchExhaustiveness` sem o relato: os casos inalcançáveis
    /// e, se não é exaustivo, a primeira testemunha. `None` quando há tipo
    /// inválido (o analyzer não verifica).
    pub(crate) fn verificar(&mut self, e: &Entrada<'_>, escrutinio: TypeId, casos: &[Caso]) -> Option<Resultado> {
        self.invalido = false;
        let valor = self.tipo_estatico(escrutinio);
        let mut espacos = Vec::new();
        let mut guardados = Vec::new();
        let mut indices = Vec::new();
        for (i, c) in casos.iter().enumerate() {
            let Some(p) = c.padrao else { continue };
            let es = self.padrao(e, Caminho::default(), valor, p, false);
            espacos.push(es);
            guardados.push(c.guardado);
            indices.push(i);
        }
        if self.invalido {
            return None;
        }
        let (inal, nao) = self.calcular(valor, &guardados, &espacos);
        let mut r = Resultado { inalcancaveis: inal.into_iter().map(|i| indices[i]).collect(), testemunha: None, correcao: None };
        if let Some(ts) = nao {
            if let Some(primeira) = ts.first() {
                let w = self.construir_testemunha(primeira);
                let mut a = String::new();
                self.escrever_testemunha(&mut a, &w, false);
                let mut b = String::new();
                self.escrever_testemunha(&mut b, &w, true);
                r.testemunha = Some(a);
                r.correcao = Some(b);
            }
        }
        Some(r)
    }
}

fn texto_de_lista(texto: &str, tam: usize, resto: bool) -> String {
    let mut s = format!("{texto}[");
    let mut virgula = "";
    for _ in 0..tam {
        s.push_str(virgula);
        s.push_str("()");
        virgula = ", ";
    }
    if resto {
        s.push_str(virgula);
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
