//! Os ciclos de constantes de uma biblioteca, como o `computeConstants` do
//! analyzer os acha (`compute.dart:13-96`): os alvos do `ConstantFinder`
//! (`utilities.dart:107-210`) e as dependências soltas do
//! `ConstantExpressionsDependenciesFinder` (`:20-101`), percorridos na ordem
//! pelo caminhante de [`super::grafo`], com as dependências de
//! `computeDependencies` (`evaluation.dart:223-326`) e do `ReferenceFinder`
//! (`utilities.dart:214-273`).
//!
//! O grafo só **acha os componentes** (docs/ANALYZER-ESPECIFICACAO.md §G,
//! T8.5): o valor de cada constante continua calculado sob demanda pelo
//! [`Motor`]. O que sai daqui é quem está em ciclo:
//!
//! * cada variável de um componente recebe `recursive_compile_time_constant`
//!   no nome ([`Estado::variaveis_em_ciclo`], [`Estado::locais_em_ciclo`]);
//! * cada construtor de um componente deixa de ser livre de ciclo
//!   ([`Estado::construtores_em_ciclo`]): toda criação por ele vale um
//!   desconhecido do tipo, e o verificador relata
//!   `recursive_constant_constructor` no nome da classe do cabeçalho.
//!
//! O estado do caminhante (índices dos nós) persiste entre as bibliotecas
//! de uma mesma verificação, como os elementos persistem na sessão do
//! analyzer. Elementos de bibliotecas cujos corpos não foram inferidos
//! contam como já avaliados: não viram nó.

use super::avaliador::{locais_constantes, Motor};
use super::grafo::{Caminhante, Grafo};
use crate::resolved::{MemberRef, Resolved};
use crate::table::Type;
use dartforge_elements::model::{
    ClassId, ClassKind, Element, FunctionElementId, FunctionKind, FunctionRef, LibraryId, Program, UnitId, UnitRole,
    VariableId, VariableRef,
};
use dartforge_frontend::ast::{
    self, CollectionElement, DeclKind, ExprId, ExprKind, MemberKind, PatternId, PatternKind, StmtId, StmtKind,
};
use dartforge_frontend::features::LanguageVersion;
use dartforge_intern::SymbolId;
use std::collections::{HashMap, HashSet};

/// Um alvo de avaliação constante (`ConstantEvaluationTarget`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Alvo {
    /// Variável de topo, campo ou constante de enum.
    Var(VariableId),
    Ctor(FunctionElementId),
    /// Parâmetro opcional (o `DefaultFormalParameter` do analyzer, com ou
    /// sem valor padrão): a unidade, o início do parâmetro e o padrão. Um
    /// parâmetro obrigatório está sempre avaliado e não vira alvo.
    Param(UnitId, usize, Option<ExprId>),
    /// Constante local: a unidade e o offset do nome.
    Local(UnitId, usize),
    /// Anotação: o índice em [`Estado::anotacoes`].
    Anot(u32),
    /// O campo `const` sintético `values` de um enum, cujo inicializador é
    /// a lista das constantes (`LibraryBuilder.buildEnumChildren`): depende
    /// de todas elas, e `e1(values)` fecha um ciclo.
    Values(ClassId),
}

/// Os índices das declarações do programa, pelo nó da árvore.
#[derive(Debug, Default)]
struct Indices {
    topo: HashMap<(UnitId, ast::DeclId, usize), VariableId>,
    campos: HashMap<(UnitId, ast::MemberId, usize), VariableId>,
    construtores: HashMap<(UnitId, ast::MemberId), FunctionElementId>,
    classes: HashMap<(UnitId, ast::DeclId), ClassId>,
}

impl Indices {
    fn novo(program: &Program) -> Self {
        let mut x = Indices::default();
        for (i, v) in program.variables.iter().enumerate() {
            match v.node {
                VariableRef::TopLevel { unit, decl, index } => {
                    x.topo.insert((unit, decl, index), VariableId(i as u32));
                }
                VariableRef::Field { unit, member, index } => {
                    x.campos.insert((unit, member, index), VariableId(i as u32));
                }
                _ => {}
            }
        }
        for (i, f) in program.functions.iter().enumerate() {
            if let FunctionRef::Constructor { unit, member } = f.node {
                x.construtores.insert((unit, member), FunctionElementId(i as u32));
            }
        }
        for (i, k) in program.classes.iter().enumerate() {
            if let Some(d) = k.decl {
                x.classes.insert((d.unit, d.decl), ClassId(i as u32));
            }
        }
        x
    }
}

/// O que persiste no [`Motor`] entre as bibliotecas.
#[derive(Debug, Default)]
pub struct Estado {
    caminhante: Caminhante,
    nos: Vec<Alvo>,
    por_alvo: HashMap<Alvo, usize>,
    /// `isConstantEvaluated` de cada nó.
    avaliados: Vec<bool>,
    /// As dependências de cada anotação, calculadas quando ela é achada
    /// (nada depende de uma anotação: ela só começa percursos).
    anotacoes: Vec<Vec<Alvo>>,
    /// As constantes locais de cada unidade, pelo offset do nome.
    locais: HashMap<UnitId, HashMap<usize, (StmtId, usize)>>,
    indices: Option<Indices>,
    /// Construtores com `isCycleFree == false`.
    pub construtores_em_ciclo: HashSet<FunctionElementId>,
    /// Variáveis cujo resultado é o inválido de ciclo.
    pub variaveis_em_ciclo: HashSet<VariableId>,
    /// Idem, constantes locais (unidade e offset do nome).
    pub locais_em_ciclo: HashSet<(UnitId, usize)>,
}

impl Estado {
    /// O construtor declarado no membro `mid` da unidade `u`.
    pub fn construtor_de(&self, u: UnitId, mid: ast::MemberId) -> Option<FunctionElementId> {
        self.indices.as_ref()?.construtores.get(&(u, mid)).copied()
    }

    /// O inicializador da constante local declarada em `offset`.
    fn inicializador_local(&mut self, m: &Motor<'_>, u: UnitId, offset: usize) -> Option<Option<ExprId>> {
        let a = m.ast(u);
        let mapa = self.locais.entry(u).or_insert_with(|| locais_constantes(a));
        let &(stmt, i) = mapa.get(&offset)?;
        let lista = match &a.stmt(stmt).kind {
            StmtKind::Variables(l) => l,
            StmtKind::For { init: Some(ast::ForInit::Variables(l)), .. } => l,
            _ => return None,
        };
        Some(lista.variables.get(i).and_then(|x| x.initializer))
    }
}

/// Acha os ciclos de constantes de `lib` e os guarda em `m.grafo`. Roda
/// antes do verificador, que lê o resultado.
pub fn computar(m: &mut Motor<'_>, lib: LibraryId) {
    if !m.inferidas.contains(&lib) {
        return;
    }
    let mut est = std::mem::take(&mut m.grafo);
    if est.indices.is_none() {
        est.indices = Some(Indices::novo(m.program));
    }
    let mut caminhante = std::mem::take(&mut est.caminhante);
    {
        let motor: &Motor<'_> = m;
        let program = motor.program;
        // Os alvos, unidade por unidade: os do `ConstantFinder` e depois
        // as dependências das expressões constantes soltas.
        let mut ordem: Vec<Alvo> = Vec::new();
        for &u in &program.library(lib).units {
            if program.unit(u).role == UnitRole::Patch {
                continue;
            }
            let mut p = Passeio::novo(motor, &mut est, u, false);
            p.unidade();
            let Passeio { alvos, soltas, .. } = p;
            ordem.extend(alvos);
            ordem.extend(soltas);
        }
        let mut g = G { m: motor, est: &mut est };
        for alvo in ordem {
            if let Some(no) = g.no(alvo) {
                caminhante.percorrer(&mut g, no);
            }
        }
    }
    est.caminhante = caminhante;
    m.grafo = est;
}

/// O inicializador escrito de uma variável de topo ou de um campo.
fn inicializador(m: &Motor<'_>, v: VariableId) -> Option<(UnitId, ExprId)> {
    match m.program.variable(v).node {
        VariableRef::TopLevel { unit, decl, index } => match &m.ast(unit).decl(decl).kind {
            DeclKind::Variables(l) => l.variables.get(index).and_then(|x| x.initializer).map(|x| (unit, x)),
            _ => None,
        },
        VariableRef::Field { unit, member, index } => match &m.ast(unit).member(member).kind {
            MemberKind::Field(l) => l.variables.get(index).and_then(|x| x.initializer).map(|x| (unit, x)),
            _ => None,
        },
        _ => None,
    }
}

/// A variável é um `ConstVariableElement`: `const`, constante de enum, ou
/// `final` de instância de um enum ou de uma classe com algum construtor
/// `const` (`constFieldsForFinalInstance`). As outras estão sempre avaliadas.
fn elemento_constante(m: &Motor<'_>, v: VariableId) -> bool {
    let program = m.program;
    let var = program.variable(v);
    if var.const_ || matches!(var.node, VariableRef::EnumConstant { .. }) {
        return true;
    }
    if !var.final_ || var.static_ {
        return false;
    }
    let Some(k) = var.class else { return false };
    let classe = program.class(k);
    match classe.kind {
        ClassKind::Enum => true,
        ClassKind::Class => classe.constructors.values().any(|f| program.function(program.publico(*f)).const_),
        _ => false,
    }
}

/// A superclasse declarada de `k`: as classes sintéticas `S&M` de uma
/// cláusula `with` não contam (o `superclass` do analyzer é `S`).
pub(super) fn superclasse_declarada(program: &Program, k: ClassId) -> Option<ClassId> {
    let mut sup = program.class(k).supertype_class?;
    let mut passos = 0;
    while program.class(sup).kind == ClassKind::MixinApplication && program.class(sup).decl.is_none() && passos < 64 {
        sup = program.class(sup).supertype_class?;
        passos += 1;
    }
    Some(sup)
}

// -- O grafo -------------------------------------------------------------------

struct G<'p, 'a> {
    m: &'p Motor<'a>,
    est: &'p mut Estado,
}

impl<'a> G<'_, 'a> {
    /// O nó de `alvo`; `None` quando o elemento é de uma biblioteca opaca
    /// (conta como avaliado).
    fn no(&mut self, alvo: Alvo) -> Option<usize> {
        let program = self.m.program;
        match alvo {
            Alvo::Var(v) if !self.m.inferidas.contains(&program.variable(v).library) => return None,
            Alvo::Ctor(f) if !self.m.inferidas.contains(&program.function(f).library) => return None,
            _ => {}
        }
        if let Some(&i) = self.est.por_alvo.get(&alvo) {
            return Some(i);
        }
        let sempre = match alvo {
            Alvo::Var(v) => !elemento_constante(self.m, v),
            _ => false,
        };
        let i = self.est.nos.len();
        self.est.nos.push(alvo);
        self.est.por_alvo.insert(alvo, i);
        self.est.avaliados.push(sempre);
        Some(i)
    }

    /// O `ReferenceFinder` sobre o que `f` visita, na unidade `u`.
    fn referencias(&mut self, u: UnitId, f: impl FnOnce(&mut Passeio<'_, 'a>)) -> Vec<Alvo> {
        let mut p = Passeio::novo(self.m, &mut *self.est, u, true);
        f(&mut p);
        p.saida
    }

    fn de_variavel(&mut self, v: VariableId) -> Vec<Alvo> {
        let m = self.m;
        let program = m.program;
        let var = program.variable(v);
        if let VariableRef::EnumConstant { unit, decl, index } = var.node {
            let DeclKind::Enum(en) = &m.ast(unit).decl(decl).kind else { return Vec::new() };
            let Some(cst) = en.constants.get(index) else { return Vec::new() };
            // O enum chamado `values`, ou a constante com o nome do enum: o
            // erro já saiu em outro lugar.
            if m.interner.resolve(en.name.sym) == "values" || cst.name.sym == en.name.sym {
                return Vec::new();
            }
            let mut deps = Vec::new();
            if let Some(k) = var.class {
                let chave = cst.constructor.map(|n| n.sym).or_else(|| m.interner.lookup(""));
                if let Some(g) = chave.and_then(|c| program.class(k).constructors.get(&c).copied()) {
                    if program.function(program.publico(g)).const_ {
                        deps.push(Alvo::Ctor(g));
                    }
                }
            }
            if let Some(args) = &cst.arguments {
                deps.extend(self.referencias(unit, |p| {
                    for x in args.args.iter() {
                        p.expr(x.value, true);
                    }
                }));
            }
            return deps;
        }
        match inicializador(m, v) {
            Some((unit, init)) => self.referencias(unit, |p| p.expr(init, var.const_)),
            None => Vec::new(),
        }
    }

    fn de_construtor(&mut self, f: FunctionElementId) -> Vec<Alvo> {
        let m = self.m;
        let program = m.program;
        let publico = program.function(program.publico(f));
        if !publico.const_ {
            return Vec::new();
        }
        let fe = program.function(f);
        let Some(classe) = fe.class else { return Vec::new() };
        let vazio = m.interner.lookup("");
        let no = match fe.node {
            FunctionRef::Constructor { unit, member } => match &m.ast(unit).member(member).kind {
                MemberKind::Constructor(k) => Some((unit, k)),
                _ => None,
            },
            _ => None,
        };
        if publico.factory || no.is_some_and(|(_, k)| k.factory) {
            // `getConstRedirectedConstructor`: só o alvo `const` do
            // redirecionamento; o `Symbol` de `dart:core` é exceção.
            let e_symbol = m.interner.resolve(program.class(classe).name) == "Symbol" && Some(fe.library) == m.core.core_library;
            if e_symbol {
                return Vec::new();
            }
            let alvo = no.and_then(|(unit, k)| alvo_de_redirecionamento(m, unit, k));
            return match alvo {
                Some(g) if program.function(program.publico(g)).const_ => vec![Alvo::Ctor(g)],
                _ => Vec::new(),
            };
        }
        let mut deps = Vec::new();
        let mut super_implicito = true;
        if let Some((unit, k)) = no {
            for init in k.initializers.iter() {
                match init {
                    ast::Initializer::Field { value, .. } => {
                        let value = *value;
                        deps.extend(self.referencias(unit, |p| p.expr(value, false)));
                    }
                    ast::Initializer::Assert { condition, message, .. } => {
                        let (condition, message) = (*condition, *message);
                        deps.extend(self.referencias(unit, |p| {
                            p.expr(condition, false);
                            if let Some(x) = message {
                                p.expr(x, false);
                            }
                        }));
                    }
                    ast::Initializer::Super { constructor, arguments, .. } => {
                        super_implicito = false;
                        // O alvo, `const` ou não.
                        let chave = constructor.map(|n| n.sym).or(vazio);
                        let sup = program.class(classe).supertype_class;
                        if let Some(g) = chave.and_then(|c| sup.and_then(|s| program.class(s).constructors.get(&c).copied())) {
                            deps.push(Alvo::Ctor(g));
                        }
                        deps.extend(self.referencias(unit, |p| {
                            for x in arguments.args.iter() {
                                p.expr(x.value, false);
                            }
                        }));
                    }
                    ast::Initializer::Redirect { constructor, arguments, .. } => {
                        super_implicito = false;
                        let chave = constructor.map(|n| n.sym).or(vazio);
                        if let Some(g) = chave.and_then(|c| program.class(classe).constructors.get(&c).copied()) {
                            deps.push(Alvo::Ctor(g));
                        }
                        deps.extend(self.referencias(unit, |p| {
                            for x in arguments.args.iter() {
                                p.expr(x.value, false);
                            }
                        }));
                    }
                }
            }
        }
        if super_implicito {
            // Sem `super(...)`/`this(...)` escrito: o construtor sem nome da
            // superclasse, se é `const` e a superclasse não é `Object`.
            if let Some(sup) = superclasse_declarada(program, classe) {
                if Some(sup) != m.core.object_class {
                    if let Some(g) = vazio.and_then(|c| program.class(sup).constructors.get(&c).copied()) {
                        if program.function(program.publico(g)).const_ {
                            deps.push(Alvo::Ctor(g));
                        }
                    }
                }
            }
        }
        // Todo campo de instância `final`/`const` com inicializador.
        for &v in program.class(classe).fields.iter() {
            let var = program.variable(v);
            if (var.final_ || var.const_) && !var.static_ && inicializador(m, v).is_some() {
                deps.push(Alvo::Var(v));
            }
        }
        // Todos os parâmetros.
        if let Some((unit, k)) = no {
            for p in k.parameters.iter() {
                if p.kind != ast::ParameterKind::Required {
                    deps.push(Alvo::Param(unit, p.span.start, p.default_value));
                }
            }
        }
        deps
    }
}

/// O construtor para o qual a factory `k` redireciona (`= C.nome`).
fn alvo_de_redirecionamento(m: &Motor<'_>, unit: UnitId, k: &ast::Constructor) -> Option<FunctionElementId> {
    let program = m.program;
    let r = k.redirect.as_ref()?;
    let (alvo, construtor) = crate::redirecionamento::classe_e_construtor(program, unit, r)?;
    let chave = match construtor {
        Some(n) => n.sym,
        None => m.interner.lookup("")?,
    };
    program.class(alvo).constructors.get(&chave).copied()
}

impl Grafo for G<'_, '_> {
    fn dependencias(&mut self, no: usize) -> Vec<usize> {
        let alvo = self.est.nos[no];
        let deps: Vec<Alvo> = match alvo {
            Alvo::Var(v) => self.de_variavel(v),
            Alvo::Ctor(f) => self.de_construtor(f),
            Alvo::Param(u, _, Some(d)) => self.referencias(u, |p| p.expr(d, false)),
            Alvo::Param(_, _, None) => Vec::new(),
            Alvo::Local(u, offset) => match self.est.inicializador_local(self.m, u, offset) {
                Some(Some(init)) => self.referencias(u, |p| p.expr(init, true)),
                _ => Vec::new(),
            },
            Alvo::Anot(i) => self.est.anotacoes[i as usize].clone(),
            Alvo::Values(k) => self.m.program.class(k).enum_constants.iter().map(|&v| Alvo::Var(v)).collect(),
        };
        deps.into_iter().filter_map(|d| self.no(d)).collect()
    }

    fn avaliado(&self, no: usize) -> bool {
        self.est.avaliados[no]
    }

    fn avaliar(&mut self, no: usize) {
        let m = self.m;
        let alvo = self.est.nos[no];
        let pronto = match alvo {
            // Sem inicializador não há resultado a guardar: o nó fica por
            // avaliar, como no analyzer.
            Alvo::Var(v) => {
                matches!(m.program.variable(v).node, VariableRef::EnumConstant { .. }) || inicializador(m, v).is_some()
            }
            // Só o construtor `const` fica computado.
            Alvo::Ctor(f) => m.program.function(m.program.publico(f)).const_,
            Alvo::Param(..) | Alvo::Anot(_) | Alvo::Values(_) => true,
            Alvo::Local(u, offset) => matches!(self.est.inicializador_local(m, u, offset), Some(Some(_))),
        };
        if pronto {
            self.est.avaliados[no] = true;
        }
    }

    fn avaliar_componente(&mut self, nos: &[usize]) {
        for &no in nos {
            let alvo = self.est.nos[no];
            match alvo {
                Alvo::Var(v) => {
                    self.est.variaveis_em_ciclo.insert(v);
                    self.est.avaliados[no] = true;
                }
                // O construtor de um ciclo nunca fica avaliado.
                Alvo::Ctor(f) => {
                    self.est.construtores_em_ciclo.insert(f);
                }
                Alvo::Local(u, offset) => {
                    self.est.locais_em_ciclo.insert((u, offset));
                    self.est.avaliados[no] = true;
                }
                // O parâmetro é uma variável para `generateCycleError`: fica
                // com o inválido guardado.
                // O `values` sintético não tem nome a relatar.
                Alvo::Param(..) | Alvo::Anot(_) | Alvo::Values(_) => self.est.avaliados[no] = true,
            }
        }
    }
}

// -- O passeio pela árvore -------------------------------------------------------

/// Um passeio em pré-ordem pela árvore de uma unidade, nos dois papéis:
/// com `referencias`, é o `ReferenceFinder` (o que acha vai a `saida`);
/// sem, é o `ConstantFinder` (`alvos`) junto com o
/// `ConstantExpressionsDependenciesFinder` (`soltas`).
struct Passeio<'p, 'a> {
    m: &'p Motor<'a>,
    est: &'p mut Estado,
    u: UnitId,
    a: &'a ast::Ast,
    referencias: bool,
    saida: Vec<Alvo>,
    alvos: Vec<Alvo>,
    soltas: Vec<Alvo>,
    /// As dependências soltas já vistas (o conjunto do original é ordenado
    /// pela inserção).
    vistas: HashSet<Alvo>,
    /// A classe (ou mixin, enum, extension type) do membro visitado.
    classe: Option<ClassId>,
    /// `treatFinalInstanceVarAsConst`: dentro de uma classe com algum
    /// construtor `const`.
    finais_como_const: bool,
    padroes_ligados: bool,
}

impl<'p, 'a> Passeio<'p, 'a> {
    fn novo(m: &'p Motor<'a>, est: &'p mut Estado, u: UnitId, referencias: bool) -> Self {
        let program = m.program;
        let lib = program.unit(u).library;
        let padroes_ligados = program.library(lib).features.versao() >= LanguageVersion::new(3, 0);
        Passeio {
            m,
            est,
            u,
            a: m.ast(u),
            referencias,
            saida: Vec::new(),
            alvos: Vec::new(),
            soltas: Vec::new(),
            vistas: HashSet::new(),
            classe: None,
            finais_como_const: false,
            padroes_ligados,
        }
    }

    /// `_find`: as referências do que `f` visita viram dependências soltas.
    fn soltar(&mut self, f: impl FnOnce(&mut Self)) {
        let guardada = std::mem::take(&mut self.saida);
        let antes = std::mem::replace(&mut self.referencias, true);
        f(self);
        self.referencias = antes;
        let achadas = std::mem::replace(&mut self.saida, guardada);
        for x in achadas {
            if self.vistas.insert(x) {
                self.soltas.push(x);
            }
        }
    }

    // -- Declarações -------------------------------------------------------------

    fn unidade(&mut self) {
        let program = self.m.program;
        let cu = &program.unit(self.u).unit;
        for d in cu.directives.iter() {
            // As anotações de `part` e `part of` não têm elemento.
            if matches!(d.kind, ast::DirectiveKind::Part { .. } | ast::DirectiveKind::PartOf { .. }) {
                continue;
            }
            self.anotacoes(&d.metadata);
        }
        for &d in cu.declarations.iter() {
            self.declaracao(d);
        }
    }

    fn declaracao(&mut self, d: ast::DeclId) {
        let a = self.a;
        let program = self.m.program;
        let decl = a.decl(d);
        self.anotacoes(&decl.metadata);
        let classe = self.est.indices.as_ref().and_then(|x| x.classes.get(&(self.u, d)).copied());
        match &decl.kind {
            DeclKind::Variables(l) => {
                for (i, var) in l.variables.iter().enumerate() {
                    let Some(init) = var.initializer else { continue };
                    self.expr(init, l.const_);
                    if l.const_ {
                        let v = self.est.indices.as_ref().and_then(|x| x.topo.get(&(self.u, d, i)).copied());
                        if let Some(v) = v {
                            self.alvos.push(Alvo::Var(v));
                        }
                    }
                }
            }
            DeclKind::Function(f) => self.funcao(*f),
            DeclKind::Class(k) => {
                self.classe = classe;
                self.finais_como_const =
                    k.members.iter().any(|mid| matches!(&a.member(*mid).kind, MemberKind::Constructor(c) if c.const_));
                self.membros(&k.members);
                self.finais_como_const = false;
                self.classe = None;
            }
            DeclKind::Mixin(k) => {
                self.classe = classe;
                self.membros(&k.members);
                self.classe = None;
            }
            DeclKind::Enum(k) => {
                self.classe = classe;
                for (i, cst) in k.constants.iter().enumerate() {
                    if let Some(args) = &cst.arguments {
                        for x in args.args.iter() {
                            self.expr(x.value, true);
                        }
                    }
                    if let Some(v) = classe.and_then(|c| program.class(c).enum_constants.get(i).copied()) {
                        self.alvos.push(Alvo::Var(v));
                    }
                }
                self.membros(&k.members);
                self.classe = None;
            }
            DeclKind::Extension(k) => self.membros(&k.members),
            DeclKind::ExtensionType(k) => {
                self.classe = classe;
                self.membros(&k.members);
                self.classe = None;
            }
            DeclKind::Typedef(_) => {}
        }
    }

    fn membros(&mut self, membros: &'a [ast::MemberId]) {
        let a = self.a;
        for &mid in membros {
            let membro = a.member(mid);
            self.anotacoes(&membro.metadata);
            match &membro.kind {
                MemberKind::Field(l) => {
                    for (i, var) in l.variables.iter().enumerate() {
                        let Some(init) = var.initializer else { continue };
                        self.expr(init, l.const_);
                        if l.const_ || self.finais_como_const && l.final_ && !l.static_ {
                            let v = self.est.indices.as_ref().and_then(|x| x.campos.get(&(self.u, mid, i)).copied());
                            if let Some(v) = v {
                                self.alvos.push(Alvo::Var(v));
                            }
                        }
                    }
                }
                MemberKind::Method(f) => self.funcao(*f),
                MemberKind::Constructor(k) => {
                    self.parametros(&k.parameters);
                    for init in k.initializers.iter() {
                        match init {
                            ast::Initializer::Field { value, .. } => self.expr(*value, false),
                            ast::Initializer::Assert { condition, message, .. } => {
                                self.expr(*condition, false);
                                if let Some(x) = message {
                                    self.expr(*x, false);
                                }
                            }
                            ast::Initializer::Super { arguments, .. } | ast::Initializer::Redirect { arguments, .. } => {
                                for x in arguments.args.iter() {
                                    self.expr(x.value, false);
                                }
                            }
                        }
                    }
                    self.corpo(&k.body);
                    // O construtor `const` e todos os parâmetros dele,
                    // depois dos filhos.
                    if k.const_ {
                        let f = self.est.indices.as_ref().and_then(|x| x.construtores.get(&(self.u, mid)).copied());
                        if let Some(f) = f {
                            self.alvos.push(Alvo::Ctor(f));
                            for p in k.parameters.iter() {
                                if p.kind != ast::ParameterKind::Required {
                                    self.alvos.push(Alvo::Param(self.u, p.span.start, p.default_value));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn funcao(&mut self, f: ast::FunctionId) {
        let a = self.a;
        let func = a.function(f);
        if let Some(ps) = &func.parameters {
            self.parametros(ps);
        }
        self.corpo(&func.body);
    }

    fn parametros(&mut self, ps: &'a [ast::Parameter]) {
        for p in ps {
            self.anotacoes(&p.metadata);
            if let Some(fp) = &p.function_parameters {
                self.parametros(fp);
            }
            if let Some(d) = p.default_value {
                self.expr(d, false);
                if !self.referencias {
                    self.alvos.push(Alvo::Param(self.u, p.span.start, Some(d)));
                }
            }
        }
    }

    fn corpo(&mut self, b: &ast::FunctionBody) {
        match b {
            ast::FunctionBody::Block(s) => self.stmt(*s),
            ast::FunctionBody::Expression(e) => self.expr(*e, false),
            _ => {}
        }
    }

    /// As anotações de uma declaração: cada uma é um alvo, com o elemento a
    /// que resolve (a variável do getter, ou o construtor) e as referências
    /// dos argumentos como dependências.
    fn anotacoes(&mut self, ms: &'a [ast::Annotation]) {
        if self.referencias {
            return;
        }
        for an in ms {
            let mut deps = Vec::new();
            if let Some(x) = self.elemento_da_anotacao(an) {
                deps.push(x);
            }
            if let Some(args) = &an.arguments {
                let guardada = std::mem::take(&mut self.saida);
                self.referencias = true;
                for x in args.args.iter() {
                    self.expr(x.value, true);
                }
                self.referencias = false;
                deps.extend(std::mem::replace(&mut self.saida, guardada));
            }
            let i = self.est.anotacoes.len() as u32;
            self.est.anotacoes.push(deps);
            self.alvos.push(Alvo::Anot(i));
        }
    }

    fn elemento_da_anotacao(&self, an: &ast::Annotation) -> Option<Alvo> {
        let program = self.m.program;
        let u = self.u;
        let vazio = self.m.interner.lookup("");
        let estatica = |k: ClassId, nome: SymbolId| -> Option<VariableId> {
            let classe = program.class(k);
            classe.enum_constants.iter().chain(classe.fields.iter()).copied().find(|v| {
                let x = program.variable(*v);
                x.name == nome && x.static_
            })
        };
        let de_classe = |k: ClassId, nome: Option<SymbolId>| -> Option<Alvo> {
            if let Some(v) = nome.and_then(|n| estatica(k, n)) {
                return Some(Alvo::Var(v));
            }
            let chave = nome.or(vazio)?;
            program.class(k).constructors.get(&chave).map(|f| Alvo::Ctor(*f))
        };
        let de_elemento = |el: Option<Element>, nome: Option<SymbolId>| -> Option<Alvo> {
            match el? {
                Element::Variable(v) if nome.is_none() => Some(Alvo::Var(v)),
                Element::Function(f) if nome.is_none() => program.function(f).variable.map(Alvo::Var),
                Element::Class(k) => de_classe(k, nome),
                _ => None,
            }
        };
        match &an.name[..] {
            [n] => {
                if let Some(v) = self.classe.and_then(|k| estatica(k, n.sym)) {
                    return Some(Alvo::Var(v));
                }
                de_elemento(program.lookup_na_unidade(u, n.sym).and_then(|b| b.getter), None)
            }
            [x, y] => match program.lookup_prefixed_na_unidade(u, x.sym, y.sym) {
                Some(b) => de_elemento(b.getter, None),
                None => de_elemento(program.lookup_na_unidade(u, x.sym).and_then(|b| b.getter), Some(y.sym)),
            },
            [p, k, n] => de_elemento(program.lookup_prefixed_na_unidade(u, p.sym, k.sym).and_then(|b| b.getter), Some(n.sym)),
            _ => None,
        }
    }

    // -- Comandos ------------------------------------------------------------------

    fn lista_local(&mut self, l: &'a ast::VariableList) {
        for var in l.variables.iter() {
            let Some(init) = var.initializer else { continue };
            self.expr(init, l.const_);
            if l.const_ && !self.referencias {
                self.alvos.push(Alvo::Local(self.u, var.name.span.start));
            }
        }
    }

    fn inicio_de_for(&mut self, init: &'a Option<ast::ForInit>) {
        match init {
            Some(ast::ForInit::Variables(l)) => self.lista_local(l),
            Some(ast::ForInit::Expression(e)) => self.expr(*e, false),
            Some(ast::ForInit::Pattern { pattern, value, .. }) => {
                self.padrao(*pattern);
                self.expr(*value, false);
            }
            None => {}
        }
    }

    fn alvo_de_for_in(&mut self, alvo: &'a ast::ForInTarget) {
        match alvo {
            ast::ForInTarget::Declared { metadata, .. } => self.anotacoes(metadata),
            ast::ForInTarget::Pattern { pattern, .. } => self.padrao(*pattern),
            ast::ForInTarget::Expression(e) => self.expr(*e, false),
        }
    }

    fn stmt(&mut self, s: StmtId) {
        let a = self.a;
        match &a.stmt(s).kind {
            StmtKind::Block(xs) => {
                for x in xs.iter() {
                    self.stmt(*x);
                }
            }
            StmtKind::Variables(l) => self.lista_local(l),
            StmtKind::PatternVariables { pattern, value, .. } => {
                self.padrao(*pattern);
                self.expr(*value, false);
            }
            StmtKind::Function(f) => self.funcao(*f),
            StmtKind::Expression(e) => self.expr(*e, false),
            StmtKind::If { condition, case_pattern, guard, then, else_ } => {
                self.expr(*condition, false);
                if let Some(p) = case_pattern {
                    self.padrao(*p);
                }
                if let Some(g) = guard {
                    self.expr(*g, false);
                }
                self.stmt(*then);
                if let Some(e) = else_ {
                    self.stmt(*e);
                }
            }
            StmtKind::For { init, condition, updates, body, .. } => {
                self.inicio_de_for(init);
                if let Some(c) = condition {
                    self.expr(*c, false);
                }
                for x in updates.iter() {
                    self.expr(*x, false);
                }
                self.stmt(*body);
            }
            StmtKind::ForIn { target, iterable, body, .. } => {
                self.alvo_de_for_in(target);
                self.expr(*iterable, false);
                self.stmt(*body);
            }
            StmtKind::While { condition, body } => {
                self.expr(*condition, false);
                self.stmt(*body);
            }
            StmtKind::DoWhile { body, condition } => {
                self.stmt(*body);
                self.expr(*condition, false);
            }
            StmtKind::Switch { value, cases } => {
                self.expr(*value, false);
                for caso in cases.iter() {
                    if let Some(p) = caso.pattern {
                        self.padrao(p);
                    }
                    if let Some(g) = caso.guard {
                        self.expr(g, false);
                    }
                    for x in caso.body.iter() {
                        self.stmt(*x);
                    }
                }
            }
            StmtKind::Return(Some(e)) => self.expr(*e, false),
            StmtKind::Yield { value, .. } => self.expr(*value, false),
            StmtKind::Try { body, catches, finally_ } => {
                self.stmt(*body);
                for c in catches.iter() {
                    self.stmt(c.body);
                }
                if let Some(f) = finally_ {
                    self.stmt(*f);
                }
            }
            StmtKind::Labeled { body, .. } => self.stmt(*body),
            StmtKind::Assert { condition, message } => {
                self.expr(*condition, false);
                if let Some(x) = message {
                    self.expr(*x, false);
                }
            }
            _ => {}
        }
    }

    // -- Padrões -------------------------------------------------------------------

    fn padrao(&mut self, p: PatternId) {
        let a = self.a;
        match &a.pattern(p).kind {
            // `ConstantPattern.expression` e `SwitchCase.expression`.
            PatternKind::Constant(e) => {
                let e = *e;
                if self.referencias {
                    self.expr(e, true);
                } else {
                    self.soltar(|s| s.expr(e, true));
                }
            }
            // `case nome:` (o parser guarda como variável): a constante de
            // topo com esse nome, como o verificador a lê.
            PatternKind::Variable { final_: false, var_: false, ty: None, name } if self.padroes_ligados => {
                let program = self.m.program;
                if let Some(Element::Variable(v)) = program.lookup_na_unidade(self.u, name.sym).and_then(|b| b.getter) {
                    if program.variable(v).const_ {
                        if self.referencias {
                            self.saida.push(Alvo::Var(v));
                        } else if self.vistas.insert(Alvo::Var(v)) {
                            self.soltas.push(Alvo::Var(v));
                        }
                    }
                }
            }
            PatternKind::Relational { value, .. } => {
                let e = *value;
                if self.referencias {
                    self.expr(e, true);
                } else {
                    self.soltar(|s| s.expr(e, true));
                }
            }
            PatternKind::Or(x, y) | PatternKind::And(x, y) => {
                self.padrao(*x);
                self.padrao(*y);
            }
            PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) => self.padrao(*x),
            PatternKind::Cast { pattern, .. } => self.padrao(*pattern),
            PatternKind::List { elements, .. } => {
                for el in elements.iter() {
                    match el {
                        ast::ListPatternElement::Pattern(x) | ast::ListPatternElement::Rest(Some(x)) => self.padrao(*x),
                        _ => {}
                    }
                }
            }
            PatternKind::Map { entries, .. } => {
                for en in entries.iter() {
                    let chave = en.key;
                    if self.referencias {
                        self.expr(chave, true);
                    } else {
                        self.soltar(|s| s.expr(chave, true));
                    }
                    self.padrao(en.value);
                }
            }
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                for f in fields.iter() {
                    self.padrao(f.pattern);
                }
            }
            _ => {}
        }
    }

    // -- Expressões ----------------------------------------------------------------

    /// A referência que o próprio nó `e` é: a variável `const` (direta ou
    /// pelo getter), de topo, estática ou local.
    fn referencia(&mut self, e: ExprId) {
        if !self.referencias {
            return;
        }
        let m = self.m;
        let program = m.program;
        let v = match m.resolvido(self.u, e) {
            Some(Resolved::Element(Element::Variable(v))) | Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => {
                Some(*v)
            }
            Some(Resolved::Element(Element::Function(f))) | Some(Resolved::Member { member: MemberRef::Function(f), .. }) => {
                let fe = program.function(*f);
                if fe.kind == FunctionKind::ImplicitAccessor {
                    fe.variable
                } else {
                    // O getter `values` sintético do enum.
                    if fe.node == FunctionRef::None
                        && fe.variable.is_none()
                        && fe.static_
                        && let Some(k) = fe.class
                        && program.class(k).kind == ClassKind::Enum
                        && m.interner.resolve(fe.name) == "values"
                    {
                        self.saida.push(Alvo::Values(k));
                    }
                    None
                }
            }
            // O local (ou o local declarado adiante, sem resolução na
            // inferência, mas com a declaração registrada).
            Some(Resolved::Local(_)) | None => {
                let offset = m.body.units.get(self.u.0 as usize).and_then(|b| b.declaracao_local(e));
                if let Some(offset) = offset {
                    if self.est.inicializador_local(m, self.u, offset).is_some() {
                        self.saida.push(Alvo::Local(self.u, offset));
                    }
                }
                None
            }
            _ => None,
        };
        if let Some(v) = v {
            if program.variable(v).const_ {
                self.saida.push(Alvo::Var(v));
            }
        }
    }

    /// Uma criação de instância: `e_const` é o `isConst` do nó (o `const`
    /// escrito, ou a criação implícita num contexto constante).
    fn criacao(&mut self, e: ExprId, e_const: bool, em_const: bool, args: &'a ast::Arguments) {
        if self.referencias {
            if e_const {
                if let Some(Resolved::Constructor(f)) = self.m.resolvido(self.u, e) {
                    let program = self.m.program;
                    if program.function(program.publico(*f)).const_ {
                        self.saida.push(Alvo::Ctor(*f));
                    }
                }
            }
            for x in args.args.iter() {
                self.expr(x.value, em_const || e_const);
            }
        } else if e_const {
            self.soltar(|s| s.expr(e, em_const));
        } else {
            for x in args.args.iter() {
                self.expr(x.value, em_const);
            }
        }
    }

    fn elemento(&mut self, el: &'a CollectionElement, c: bool) {
        match el {
            CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => self.expr(*x, c),
            CollectionElement::MapEntry { key, value, .. } => {
                self.expr(*key, c);
                self.expr(*value, c);
            }
            CollectionElement::Spread { value, .. } => self.expr(*value, c),
            CollectionElement::If { condition, case_pattern, guard, then, else_ } => {
                self.expr(*condition, c);
                if let Some(p) = case_pattern {
                    self.padrao(*p);
                }
                if let Some(g) = guard {
                    self.expr(*g, c);
                }
                self.elemento(then, c);
                if let Some(x) = else_ {
                    self.elemento(x, c);
                }
            }
            CollectionElement::For { init, condition, updates, body, .. } => {
                self.inicio_de_for(init);
                if let Some(x) = condition {
                    self.expr(*x, c);
                }
                for x in updates.iter() {
                    self.expr(*x, c);
                }
                self.elemento(body, c);
            }
            CollectionElement::ForIn { target, iterable, body, .. } => {
                self.alvo_de_for_in(target);
                self.expr(*iterable, c);
                self.elemento(body, c);
            }
        }
    }

    fn expr(&mut self, e: ExprId, em_const: bool) {
        let a = self.a;
        let u = self.u;
        match &a.expr(e).kind {
            ExprKind::Identifier(_) | ExprKind::DotShorthand { .. } => self.referencia(e),
            ExprKind::InstanceCreation { keyword, arguments, .. } => {
                let e_const = match keyword {
                    Some(ast::CreationKeyword::Const) => true,
                    Some(ast::CreationKeyword::New) => false,
                    None => em_const,
                };
                self.criacao(e, e_const, em_const, arguments);
            }
            ExprKind::Call { target, arguments } => {
                if matches!(self.m.resolvido(u, e), Some(Resolved::Constructor(_))) {
                    // Criação implícita: constante num contexto constante
                    // (ou com o `const` do atalho de ponto).
                    let atalho_const = matches!(a.expr(*target).kind, ExprKind::DotShorthand { const_: true, .. });
                    self.criacao(e, em_const || atalho_const, em_const, arguments);
                    return;
                }
                self.expr(*target, em_const);
                for x in arguments.args.iter() {
                    self.expr(x.value, em_const);
                }
            }
            ExprKind::List { const_, elements, .. } => {
                let c = *const_ || em_const;
                if c && !self.referencias {
                    self.soltar(|s| s.expr(e, em_const));
                    return;
                }
                for el in elements.iter() {
                    self.elemento(el, c);
                }
            }
            ExprKind::SetOrMap { const_, elements, .. } => {
                let c = *const_ || em_const;
                if !self.referencias {
                    if c {
                        self.soltar(|s| s.expr(e, em_const));
                        return;
                    }
                    // Sem `const`: as chaves de um mapa e os elementos de um
                    // conjunto são avaliados para a checagem de unicidade.
                    let classe = match self.m.table.get(self.m.estatico(u, e)) {
                        Type::Interface { class, .. } => Some(*class),
                        _ => None,
                    };
                    if classe.is_some() && classe == self.m.core.map_class {
                        for el in elements.iter() {
                            if let CollectionElement::MapEntry { key, .. } = el {
                                let chave = *key;
                                self.soltar(|s| s.expr(chave, false));
                            }
                        }
                    } else if classe.is_some() && classe == self.m.core.set_class {
                        for el in elements.iter() {
                            self.soltar(|s| s.elemento(el, false));
                        }
                    }
                }
                for el in elements.iter() {
                    self.elemento(el, c);
                }
            }
            ExprKind::Record { const_, positional, named } => {
                let c = *const_ || em_const;
                if c && !self.referencias {
                    self.soltar(|s| s.expr(e, em_const));
                    return;
                }
                for x in positional.iter().chain(named.iter().map(|(_, x)| x)) {
                    self.expr(*x, c);
                }
            }
            ExprKind::FunctionExpression(f) => self.funcao(*f),
            ExprKind::String(lit) => {
                for p in lit.parts.iter() {
                    if let ast::StringPart::Interpolation(x) = p {
                        self.expr(*x, em_const);
                    }
                }
            }
            ExprKind::Parenthesized(x) | ExprKind::Await(x) | ExprKind::Throw(x) => self.expr(*x, em_const),
            ExprKind::Property { target, .. } => {
                self.expr(*target, em_const);
                self.referencia(e);
            }
            ExprKind::Index { target, index, .. } => {
                self.expr(*target, em_const);
                self.expr(*index, em_const);
            }
            ExprKind::TypeArguments { target, .. } => self.expr(*target, em_const),
            ExprKind::Unary { operand, .. } => self.expr(*operand, em_const),
            ExprKind::Binary { left, right, .. } => {
                self.expr(*left, em_const);
                self.expr(*right, em_const);
            }
            ExprKind::Conditional { condition, then, else_ } => {
                self.expr(*condition, em_const);
                self.expr(*then, em_const);
                self.expr(*else_, em_const);
            }
            ExprKind::Is { value, .. } | ExprKind::As { value, .. } => self.expr(*value, em_const),
            ExprKind::Assign { target, value, .. } => {
                self.expr(*target, em_const);
                self.expr(*value, em_const);
            }
            ExprKind::PatternAssign { pattern, value } => {
                self.padrao(*pattern);
                self.expr(*value, em_const);
            }
            ExprKind::Cascade { target, sections, .. } => {
                self.expr(*target, em_const);
                for s in sections.iter() {
                    self.expr(*s, em_const);
                }
            }
            ExprKind::Switch { value, cases } => {
                self.expr(*value, em_const);
                for caso in cases.iter() {
                    self.padrao(caso.pattern);
                    if let Some(g) = caso.guard {
                        self.expr(g, false);
                    }
                    self.expr(caso.body, em_const);
                }
            }
            _ => {}
        }
    }
}
