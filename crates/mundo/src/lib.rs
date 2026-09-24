//! Mundo fechado sobre o modelo de elementos: o que do programa do usuário e
//! dos pacotes é alcançável a partir das raízes.
//!
//! É o RTA de Bacon na forma do `ResolutionWorldBuilder` do dart2js
//! (`pkg/compiler/lib/src/universe/resolution_world_builder.dart:196-240`):
//! dois conjuntos que só crescem — classes **instanciadas** e **seletores**
//! invocados — e a regra "um membro de instância vive se a classe dele é
//! instanciada **e** o nome dele é seletor vivo". O impacto de cada elemento é
//! calculado quando ele entra na fila (`resolution/enqueuer.dart:283-302`),
//! então código morto nunca é percorrido.
//!
//! O crate não conhece nenhum backend. O que só o backend sabe — o que o
//! runtime chama por nome, o que a interop expõe — entra pelas [`Raizes`].
//!
//! Decisões de conservadorismo (o contrato está em `docs/JS-PRODUCAO.md` §1.7):
//!
//! * seletor **por nome**, sem restrição pelo tipo do receptor: a inferência
//!   ainda tem lacunas e o emissor tem busca própria de membros, então casar
//!   por tipo seria podar pelo palpite de uma das duas partes;
//! * seletor **por espécie**: leitura/chamada (`foo`) e escrita (`foo=`) são
//!   seletores distintos, como as chaves do `instance_members`. Nomes vindos
//!   de fora do programa (o runtime chamando por string) valem para as duas
//!   espécies; usos no programa registram a espécie exata;
//! * operadores, membros de `Object` e `call` vivem em toda classe
//!   instanciada, e também os membros que implementam um supertipo do SDK
//!   (o runtime pré-compilado chama por esses nomes sem que o programa veja);
//! * três níveis de classe — morta, só tipo (identidade e rti) e instanciada —
//!   que são as três alcançabilidades de `docs/PESQUISA-OTIMIZACAO.md` §5.

mod impacto;

use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionElementId, FunctionKind, FunctionRef, LibraryId, Program, VariableId, VariableRef};
use dartforge_frontend::ast;
use dartforge_intern::{Interner, SymbolId};
use dartforge_types::resolve::OutlineTypes;
use dartforge_types::resolved::BodyTypes;
use dartforge_types::table::{Type, TypeTable};
use std::collections::{HashMap, HashSet, VecDeque};

/// O programa analisado, emprestado de quem compilou.
#[derive(Clone, Copy)]
pub struct Entrada<'a> {
    pub program: &'a Program,
    pub interner: &'a Interner,
    pub table: &'a TypeTable,
    pub outline: &'a OutlineTypes,
    pub bodies: &'a BodyTypes,
}

/// O que o programa Dart não enxerga, mas que chama ou cria código dele.
#[derive(Default, Clone)]
pub struct Raizes {
    /// `main` e o que mais for chamado de fora.
    pub funcoes: Vec<FunctionElementId>,
    pub variaveis: Vec<VariableId>,
    /// Instanciadas por código que não vemos.
    pub classes_instanciadas: Vec<ClassId>,
    /// Pelo menos identidade de tipo (casca e regras rti).
    pub classes_tipo: Vec<ClassId>,
    /// Instanciadas e com **todos** os membros vivos (`@JSExport`).
    pub classes_todos_os_membros: Vec<ClassId>,
    /// Nomes chamados por fora do programa (`dart.dsend(o, "toJson")` no
    /// runtime): valem para toda classe instanciada.
    pub seletores: Vec<String>,
    /// Construtores cujo tearoff estático é citado por fora.
    pub tearoffs: Vec<FunctionElementId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NivelClasse {
    Morta,
    /// Identidade: casca, `addRtiResources`, regras rti, estáticos vivos.
    Tipo,
    /// Construída: membros de instância elegíveis por seletor.
    Instanciada,
}

/// Quem puxou um elemento para o mundo (o primeiro que o fez).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Causa {
    Raiz,
    Funcao(FunctionElementId),
    Variavel(VariableId),
    Classe(ClassId),
}

#[derive(Default, Debug, Clone)]
pub struct Estatisticas {
    pub classes_usuario: usize,
    pub classes_instanciadas: usize,
    pub classes_tipo: usize,
    pub funcoes_usuario: usize,
    pub funcoes_vivas: usize,
    pub variaveis_vivas: usize,
    pub seletores: usize,
    pub itens_processados: usize,
}

/// O resultado: o que vive.
pub struct Mundo {
    classes: Vec<NivelClasse>,
    funcoes: Vec<bool>,
    variaveis: Vec<bool>,
    tearoffs: Vec<bool>,
    seletores: HashSet<String>,
    causa_fn: Vec<Option<Causa>>,
    causa_classe: Vec<Option<Causa>>,
    causa_var: Vec<Option<Causa>>,
    pub estat: Estatisticas,
}

impl Mundo {
    pub fn classe(&self, c: ClassId) -> NivelClasse {
        self.classes.get(c.0 as usize).copied().unwrap_or(NivelClasse::Morta)
    }
    pub fn funcao(&self, f: FunctionElementId) -> bool {
        self.funcoes.get(f.0 as usize).copied().unwrap_or(false)
    }
    pub fn variavel(&self, v: VariableId) -> bool {
        self.variaveis.get(v.0 as usize).copied().unwrap_or(false)
    }
    pub fn seletor(&self, nome: &str) -> bool {
        self.seletores.contains(nome)
    }
    /// Os seletores vivos, em ordem alfabética (o conjunto é um `HashSet`).
    pub fn seletores(&self) -> impl Iterator<Item = &str> {
        let mut v: Vec<&str> = self.seletores.iter().map(String::as_str).collect();
        v.sort_unstable();
        v.into_iter()
    }
    pub fn tearoff_de_construtor(&self, f: FunctionElementId) -> bool {
        self.tearoffs.get(f.0 as usize).copied().unwrap_or(false)
    }
    pub fn causa_funcao(&self, f: FunctionElementId) -> Option<Causa> {
        self.causa_fn.get(f.0 as usize).copied().flatten()
    }
    pub fn causa_classe(&self, c: ClassId) -> Option<Causa> {
        self.causa_classe.get(c.0 as usize).copied().flatten()
    }
    pub fn causa_variavel(&self, v: VariableId) -> Option<Causa> {
        self.causa_var.get(v.0 as usize).copied().flatten()
    }
}

/// Um elemento esperando para ter o impacto calculado.
#[derive(Clone, Copy, Debug)]
enum Item {
    Funcao(FunctionElementId),
    Variavel(VariableId),
    /// Inicializadores dos campos de instância de uma classe instanciada.
    Campos(ClassId),
}

/// Calcula o mundo. Determinístico: a fila é FIFO, as raízes são ordenadas, e
/// o resultado (os conjuntos) é o menor ponto fixo, que não depende da ordem.
pub fn calcular(e: Entrada<'_>, r: &Raizes) -> Mundo {
    let mut m = Motor::novo(e);
    m.causa = Causa::Raiz;
    for s in {
        let mut v = r.seletores.clone();
        v.sort();
        v
    } {
        m.novo_seletor_externo(&s);
    }
    let mut todos: Vec<ClassId> = r.classes_todos_os_membros.clone();
    todos.sort();
    for c in todos {
        m.todos_membros.insert(c);
        m.instanciar(c);
    }
    let mut cs = r.classes_instanciadas.clone();
    cs.sort();
    for c in cs {
        m.instanciar(c);
    }
    let mut ts = r.classes_tipo.clone();
    ts.sort();
    for c in ts {
        m.marcar_tipo(c);
    }
    let mut fs = r.funcoes.clone();
    fs.sort();
    for f in fs {
        m.usar_funcao_direta(f);
    }
    let mut vs = r.variaveis.clone();
    vs.sort();
    for v in vs {
        m.usar_variavel(v);
    }
    let mut to = r.tearoffs.clone();
    to.sort();
    for f in to {
        m.usar_construtor(f, true);
    }
    m.inicializar_extensoes();
    m.rodar();
    m.finalizar()
}

/// Uma inconsistência achada pela conferência a seco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inconsistencia {
    Funcao(FunctionElementId),
    Variavel(VariableId),
    Classe(ClassId),
    Seletor(String),
}

/// `checkEnqueuerConsistency` (`pkg/compiler/lib/src/enqueue.dart:142-156`):
/// refaz o impacto de **todos** os elementos vivos, sem reaproveitar o estado
/// do ponto fixo, e acusa todo uso que aponte para algo que o mundo diz estar
/// morto. Um resultado não vazio é defeito do ponto fixo.
pub fn conferir(e: Entrada<'_>, r: &Raizes, mundo: &Mundo) -> Vec<Inconsistencia> {
    // Recalcula num motor novo, semeado com o mundo pronto, e compara: se o
    // motor achar qualquer coisa nova, o mundo não era um ponto fixo.
    let mut m = Motor::novo(e);
    m.classes = mundo.classes.clone();
    m.f_vivo = mundo.funcoes.clone();
    m.v_vivo = mundo.variaveis.clone();
    m.tearoff = mundo.tearoffs.clone();
    m.sel = mundo.seletores.clone();
    for c in &r.classes_todos_os_membros {
        m.todos_membros.insert(*c);
    }
    // Pendentes: membros de classes instanciadas cujo nome ainda não vive.
    for (i, n) in mundo.classes.iter().enumerate() {
        if *n == NivelClasse::Instanciada {
            m.processar_membros_da_cadeia(ClassId(i as u32), true);
        }
    }
    m.inicializar_extensoes();
    // Refaz o impacto de tudo que vive.
    for (i, v) in mundo.funcoes.iter().enumerate() {
        if *v {
            m.fila.push_back(Item::Funcao(FunctionElementId(i as u32)));
        }
    }
    for (i, v) in mundo.variaveis.iter().enumerate() {
        if *v {
            m.fila.push_back(Item::Variavel(VariableId(i as u32)));
        }
    }
    for (i, n) in mundo.classes.iter().enumerate() {
        if *n == NivelClasse::Instanciada {
            m.fila.push_back(Item::Campos(ClassId(i as u32)));
        }
    }
    let base = m.fila.len();
    m.rodar_itens(base);
    let mut out = Vec::new();
    for (i, v) in m.f_vivo.iter().enumerate() {
        if *v && !mundo.funcoes[i] {
            out.push(Inconsistencia::Funcao(FunctionElementId(i as u32)));
        }
    }
    for (i, v) in m.v_vivo.iter().enumerate() {
        if *v && !mundo.variaveis[i] {
            out.push(Inconsistencia::Variavel(VariableId(i as u32)));
        }
    }
    for (i, n) in m.classes.iter().enumerate() {
        if *n > mundo.classes[i] {
            out.push(Inconsistencia::Classe(ClassId(i as u32)));
        }
    }
    let mut novos: Vec<&String> = m.sel.iter().filter(|s| !mundo.seletores.contains(*s)).collect();
    novos.sort();
    out.extend(novos.into_iter().map(|s| Inconsistencia::Seletor(s.clone())));
    out
}

pub(crate) struct Motor<'a> {
    pub(crate) e: Entrada<'a>,
    classes: Vec<NivelClasse>,
    f_vivo: Vec<bool>,
    v_vivo: Vec<bool>,
    tearoff: Vec<bool>,
    sel: HashSet<String>,
    /// Membros de classes instanciadas cujo nome ainda não é seletor vivo
    /// (`_invokableInstanceMembersByName`, `resolution_world_builder.dart:232`).
    pendentes: HashMap<String, Vec<FunctionElementId>>,
    todos_membros: HashSet<ClassId>,
    fila: VecDeque<Item>,
    /// Tipos (da `TypeTable`) já varridos atrás de classes.
    tipos_vistos: HashSet<dartforge_types::table::TypeId>,
    /// Nomes que o runtime chama em qualquer objeto: membros de `Object` e `call`.
    nomes_universais: HashSet<String>,
    /// Classes instanciadas cuja regra de protocolo do SDK já foi aplicada.
    protocolo_feito: HashSet<ClassId>,
    causa: Causa,
    causa_fn: Vec<Option<Causa>>,
    causa_classe: Vec<Option<Causa>>,
    causa_var: Vec<Option<Causa>>,
    itens: usize,
    extensoes_prontas: bool,
    /// O alvo de um `Assign` simples sendo percorrido: o membro mais externo
    /// dele é escrita (`foo=`), não leitura. Consumido (e zerado) pelo braço
    /// `Property`/`Identifier`; o resto do alvo continua sendo leitura.
    pub(crate) alvo_de_escrita: Option<dartforge_frontend::ast::ExprId>,
}

impl<'a> Motor<'a> {
    fn novo(e: Entrada<'a>) -> Motor<'a> {
        let p = e.program;
        let mut universais: HashSet<String> = HashSet::new();
        universais.insert("call".into());
        // Os membros de `Object` saem do modelo de elementos, não de uma lista.
        if let Some(core) = p.core {
            if let Some(sym) = e.interner.lookup("Object") {
                if let Some(Element::Class(obj)) = p.library(core).declared.get(&sym).and_then(|b| b.getter) {
                    for f in p.class(obj).instance_members.values() {
                        universais.insert(e.interner.resolve(p.function(*f).name).to_string());
                    }
                }
            }
        }
        Motor {
            e,
            classes: vec![NivelClasse::Morta; p.classes.len()],
            f_vivo: vec![false; p.functions.len()],
            v_vivo: vec![false; p.variables.len()],
            tearoff: vec![false; p.functions.len()],
            sel: HashSet::new(),
            pendentes: HashMap::new(),
            todos_membros: HashSet::new(),
            fila: VecDeque::new(),
            tipos_vistos: HashSet::new(),
            nomes_universais: universais,
            protocolo_feito: HashSet::new(),
            causa: Causa::Raiz,
            causa_fn: vec![None; p.functions.len()],
            causa_classe: vec![None; p.classes.len()],
            causa_var: vec![None; p.variables.len()],
            itens: 0,
            extensoes_prontas: false,
            alvo_de_escrita: None,
        }
    }

    fn e_usuario_lib(&self, l: LibraryId) -> bool {
        !self.e.program.library(l).is_sdk
    }
    fn e_usuario_classe(&self, c: ClassId) -> bool {
        self.e_usuario_lib(self.e.program.class(c).library)
    }

    fn nome(&self, s: SymbolId) -> &'a str {
        self.e.interner.resolve(s)
    }

    // ------------------------------------------------------------ marcações

    fn viva_fn(&mut self, f: FunctionElementId) {
        let i = f.0 as usize;
        if i >= self.f_vivo.len() || self.f_vivo[i] {
            return;
        }
        let func = self.e.program.function(f);
        if !self.e_usuario_lib(func.library) {
            return;
        }
        self.f_vivo[i] = true;
        self.causa_fn[i] = Some(self.causa);
        self.fila.push_back(Item::Funcao(f));
    }

    pub(crate) fn usar_variavel(&mut self, v: VariableId) {
        let i = v.0 as usize;
        if i >= self.v_vivo.len() || self.v_vivo[i] {
            return;
        }
        let var = self.e.program.variable(v);
        if !self.e_usuario_lib(var.library) {
            return;
        }
        // Campo de instância é armazenamento: vive com a classe instanciada,
        // e o acesso é por seletor. Aqui só entram topo e estáticos. O
        // `Resolved` não diz se o uso é leitura ou escrita, então vale a
        // espécie de leitura (a escrita exata vem do alvo do `Assign`); o
        // pior caso é manter o getter, nunca podar o setter sem registro.
        if var.class.is_some() && !var.static_ && var.extension.is_none() {
            let n = self.nome(var.name).to_string();
            self.novo_seletor(&n);
            return;
        }
        self.v_vivo[i] = true;
        self.causa_var[i] = Some(self.causa);
        if let Some(g) = var.getter {
            self.viva_fn(g);
        }
        if let Some(s) = var.setter {
            self.viva_fn(s);
        }
        if let Some(c) = var.class {
            self.marcar_tipo(c);
        }
        self.fila.push_back(Item::Variavel(v));
    }

    pub(crate) fn novo_seletor(&mut self, nome: &str) {
        // Leitura e chamada (`x.foo`, `x.foo()`) valem para o getter e o
        // método; a escrita (`x.foo = v`) registra `foo=` à parte. Sem
        // sufixo não há atendimento entre espécies: um `foo` lido nunca
        // mantém um `set foo` vivo.
        if self.sel.contains(nome) {
            return;
        }
        self.sel.insert(nome.to_string());
        if let Some(v) = self.pendentes.remove(nome) {
            for f in v {
                self.viva_fn(f);
            }
        }
    }

    /// Nome chamado por fora do programa (`dart.dsend`/`dput` no runtime):
    /// pode ser leitura ou escrita, então vale para as duas espécies.
    pub(crate) fn novo_seletor_externo(&mut self, nome: &str) {
        self.novo_seletor(nome);
        if !nome.ends_with('=') {
            self.novo_seletor(&format!("{nome}="));
        }
    }

    /// A chave do seletor de um membro de instância: `foo` para leitura e
    /// chamada, `foo=` para escrita — a mesma chave do `instance_members`.
    pub(crate) fn chave_membro_instancia(&self, f: FunctionElementId) -> String {
        let func = self.e.program.function(f);
        let nome = self.nome(func.name);
        if func.kind == FunctionKind::Setter {
            return format!("{nome}=");
        }
        if func.kind == FunctionKind::ImplicitAccessor {
            if let Some(v) = func.variable {
                if self.e.program.variable(v).setter == Some(f) {
                    return format!("{nome}=");
                }
            }
        }
        nome.to_string()
    }

    /// Uso de uma função pelo seu elemento: estática, de topo, de extensão ou
    /// construtor vão direto; membro de instância vira seletor.
    pub(crate) fn usar_funcao(&mut self, f: FunctionElementId) {
        let func = self.e.program.function(f);
        match func.kind {
            FunctionKind::Constructor | FunctionKind::SyntheticConstructor => self.usar_construtor(f, false),
            _ => {
                let instancia = func.class.is_some() && !func.static_ && func.extension.is_none();
                if instancia {
                    let n = self.chave_membro_instancia(f);
                    self.novo_seletor(&n);
                } else {
                    if let Some(v) = func.variable {
                        if func.kind == FunctionKind::ImplicitAccessor {
                            self.usar_variavel(v);
                            return;
                        }
                    }
                    if func.extension.is_some() {
                        let n = self.chave_membro_instancia(f);
                        self.novo_seletor(&n);
                    }
                    if let Some(c) = func.class {
                        self.marcar_tipo(c);
                    }
                    self.viva_fn(f);
                }
            }
        }
    }

    fn usar_funcao_direta(&mut self, f: FunctionElementId) {
        self.usar_funcao(f);
        self.viva_fn(f);
    }

    /// `C(..)`, `new C.n(..)`, `const C()`, tearoff `C.new`.
    pub(crate) fn usar_construtor(&mut self, f: FunctionElementId, tearoff: bool) {
        let func = self.e.program.function(f);
        let Some(c) = func.class else { return };
        if !self.e_usuario_classe(c) {
            return;
        }
        if tearoff {
            self.tearoff[f.0 as usize] = true;
        }
        if func.factory {
            self.marcar_tipo(c);
        } else {
            self.instanciar(c);
        }
        self.viva_fn(f);
    }

    /// `C.nome(..)` / `C(..)` pela classe: o construtor declarado, ou — classe
    /// sem construtor nenhum (`class C = S with M`, abstrata) — a instanciação
    /// direta, com o repasse para o construtor da superclasse.
    pub(crate) fn criar(&mut self, c: ClassId, nome: &str, tearoff: bool) {
        if let Some(f) = self.construtor(c, nome) {
            self.usar_construtor(f, tearoff);
        } else if (nome.is_empty() || nome == "new") && self.e.program.class(c).constructors.is_empty() && self.e_usuario_classe(c) {
            self.instanciar(c);
            if let Some(sc) = self.construtor_super(c, "") {
                self.usar_construtor_por_inicializador(sc);
            }
        }
    }

    /// Chamada de construtor como inicializador (`super(..)`, `this.n(..)`,
    /// super implícito): não instancia a classe — a subclasse instanciada já
    /// instanciou a cadeia.
    pub(crate) fn usar_construtor_por_inicializador(&mut self, f: FunctionElementId) {
        let func = self.e.program.function(f);
        if let Some(c) = func.class {
            self.marcar_tipo(c);
        }
        self.viva_fn(f);
    }

    pub(crate) fn marcar_tipo(&mut self, c: ClassId) {
        let i = c.0 as usize;
        if i >= self.classes.len() || !self.e_usuario_classe(c) || self.classes[i] >= NivelClasse::Tipo {
            return;
        }
        self.classes[i] = NivelClasse::Tipo;
        self.causa_classe[i] = Some(self.causa);
        let p = self.e.program;
        let class = p.class(c);
        // Enum referenciado: as constantes existem com a classe.
        if class.kind == ClassKind::Enum {
            self.instanciar(c);
        }
        let mut supers: Vec<ClassId> = Vec::new();
        supers.extend(class.supertype_class);
        supers.extend(class.mixin_classes.iter().copied());
        supers.extend(class.interface_classes.iter().copied());
        supers.extend(class.on_classes.iter().copied());
        for s in supers {
            self.marcar_tipo(s);
        }
        // Argumentos de tipo dos supertipos entram nas regras rti da classe.
        if let Some(ct) = self.e.outline.classes.get(i) {
            let mut ts: Vec<dartforge_types::table::TypeId> = Vec::new();
            ts.extend(ct.supertype);
            ts.extend(ct.mixins.iter().copied());
            ts.extend(ct.interfaces.iter().copied());
            ts.extend(ct.on.iter().copied());
            for &tp in ct.type_params.iter() {
                ts.push(self.e.table.param(tp).bound);
            }
            for t in ts {
                self.tipo_de(t);
            }
        }
        // Tipos dos campos de instância: o emissor escreve `setFieldSignature`
        // de toda classe emitida, inclusive a só-tipo.
        for &vid in &class.fields {
            if p.variable(vid).static_ {
                continue;
            }
            if let Some(vt) = self.e.outline.variables.get(vid.0 as usize) {
                if let Some(t) = vt.declared_type.or(vt.inferred) {
                    self.tipo_de(t);
                }
            }
        }
        if let Some(rep) = class.representation {
            if let Some(vt) = self.e.outline.variables.get(rep.0 as usize) {
                if let Some(t) = vt.declared_type.or(vt.inferred) {
                    self.tipo_de(t);
                }
            }
        }
    }

    /// Classes citadas por um tipo da `TypeTable`.
    pub(crate) fn tipo_de(&mut self, t: dartforge_types::table::TypeId) {
        if !self.tipos_vistos.insert(t) {
            return;
        }
        let table = self.e.table;
        if t.0 as usize >= table.len() {
            return;
        }
        match table.get(t) {
            Type::Dynamic | Type::Void | Type::Never | Type::Null => {}
            Type::Interface { class, args, .. } | Type::ExtensionType { decl: class, args, .. } => {
                let (class, args) = (*class, args.clone());
                self.marcar_tipo(class);
                for a in args.iter() {
                    self.tipo_de(*a);
                }
            }
            Type::Function { type_params, ret, positional, optional, named, .. } => {
                let mut ts: Vec<dartforge_types::table::TypeId> = vec![*ret];
                ts.extend(positional.iter().copied());
                ts.extend(optional.iter().copied());
                ts.extend(named.iter().map(|(_, t, _)| *t));
                ts.extend(type_params.iter().map(|p| table.param(*p).bound));
                for x in ts {
                    self.tipo_de(x);
                }
            }
            Type::Record { positional, named, .. } => {
                let mut ts: Vec<dartforge_types::table::TypeId> = positional.to_vec();
                ts.extend(named.iter().map(|(_, t)| *t));
                for x in ts {
                    self.tipo_de(x);
                }
            }
            Type::TypeParameter { param, .. } => {
                let b = table.param(*param).bound;
                self.tipo_de(b);
            }
            Type::FutureOr { arg, .. } => {
                let a = *arg;
                self.tipo_de(a);
            }
            Type::Intersection { param, bound } => {
                let (b, pb) = (*bound, table.param(*param).bound);
                self.tipo_de(b);
                self.tipo_de(pb);
            }
        }
    }

    /// Instancia `c` e a cadeia de superclasses (`_processInstantiatedClass`,
    /// `resolution_world_builder.dart:706-737`), com os mixins aplicados.
    pub(crate) fn instanciar(&mut self, c: ClassId) {
        if !self.e_usuario_classe(c) {
            return;
        }
        let p = self.e.program;
        let mut cadeia: Vec<ClassId> = Vec::new();
        let mut cur = Some(c);
        while let Some(k) = cur {
            if !self.e_usuario_classe(k) || cadeia.contains(&k) {
                break;
            }
            cadeia.push(k);
            cur = p.class(k).supertype_class;
        }
        for k in cadeia.clone() {
            let class = p.class(k);
            let mixins = class.mixin_classes.clone();
            self.subir_para_instanciada(k);
            for mx in mixins {
                if self.e_usuario_classe(mx) {
                    self.subir_para_instanciada(mx);
                }
            }
        }
        // O protocolo do SDK é relativo à classe construída: um `compareTo`
        // herdado de uma superclasse sem `Comparable` também é chamado.
        if self.protocolo_feito.insert(c) {
            self.aplicar_protocolo(c);
        }
    }

    fn subir_para_instanciada(&mut self, k: ClassId) {
        let i = k.0 as usize;
        if self.classes[i] == NivelClasse::Instanciada {
            return;
        }
        self.marcar_tipo(k);
        self.classes[i] = NivelClasse::Instanciada;
        if self.causa_classe[i].is_none() {
            self.causa_classe[i] = Some(self.causa);
        }
        self.fila.push_back(Item::Campos(k));
        self.processar_membros(k, false);
    }

    /// Membros de instância de `k` (já instanciada): vivos se a chave é
    /// seletor vivo ou de protocolo; senão, pendentes pela chave (`foo` e
    /// `foo=` têm pendências separadas).
    fn processar_membros(&mut self, k: ClassId, so_pendentes: bool) {
        let p = self.e.program;
        let class = p.class(k);
        let todos = self.todos_membros.contains(&k) || class.kind == ClassKind::Enum;
        let mut membros: Vec<(dartforge_intern::SymbolId, FunctionElementId)> =
            class.instance_members.iter().map(|(s, f)| (*s, *f)).collect();
        membros.sort_by_key(|(_, f)| *f);
        for (chave, f) in membros {
            let func = p.function(f);
            let nome = self.nome(chave).to_string();
            let vive = todos
                || func.kind == FunctionKind::Operator
                || self.nomes_universais.contains(nome.as_str())
                || self.sel.contains(nome.as_str());
            if vive && !so_pendentes {
                self.viva_fn(f);
            } else if !vive {
                self.pendentes.entry(nome).or_default().push(f);
            }
        }
    }

    fn processar_membros_da_cadeia(&mut self, k: ClassId, so_pendentes: bool) {
        self.processar_membros(k, so_pendentes);
    }

    /// Regra (i) do contrato: a implementação, na cadeia de `c`, de todo
    /// membro de instância declarado num supertipo do SDK de `c`. O casamento
    /// é pela chave (`foo=` casa com `set foo`), como nos pendentes.
    fn aplicar_protocolo(&mut self, c: ClassId) {
        let p = self.e.program;
        let mut vistos: HashSet<ClassId> = HashSet::new();
        let mut fila = vec![c];
        let mut nomes: HashSet<&'a str> = HashSet::new();
        while let Some(k) = fila.pop() {
            if !vistos.insert(k) {
                continue;
            }
            let class = p.class(k);
            if !self.e_usuario_classe(k) {
                for s in class.instance_members.keys() {
                    nomes.insert(self.nome(*s));
                }
            }
            fila.extend(class.supertype_class);
            fila.extend(class.mixin_classes.iter().copied());
            fila.extend(class.interface_classes.iter().copied());
            fila.extend(class.on_classes.iter().copied());
        }
        if nomes.is_empty() {
            return;
        }
        // Os membros com esses nomes, na cadeia e nos mixins aplicados.
        let mut alvo: Vec<ClassId> = Vec::new();
        let mut cur = Some(c);
        while let Some(k) = cur {
            if !self.e_usuario_classe(k) || alvo.contains(&k) {
                break;
            }
            alvo.push(k);
            alvo.extend(p.class(k).mixin_classes.iter().copied().filter(|m| self.e_usuario_classe(*m)));
            cur = p.class(k).supertype_class;
        }
        for k in alvo {
            let mut membros: Vec<(dartforge_intern::SymbolId, FunctionElementId)> =
                p.class(k).instance_members.iter().map(|(s, f)| (*s, *f)).collect();
            membros.sort_by_key(|(_, f)| *f);
            for (chave, f) in membros {
                if nomes.contains(self.nome(chave)) {
                    self.viva_fn(f);
                }
            }
        }
    }

    /// Membros de instância de extensões do usuário: donos "sempre
    /// instanciados", vivos por nome. O emissor resolve extensões por conta
    /// própria (`call.rs::try_extension_call`), então casar pelo `Resolved`
    /// seria divergir dele.
    fn inicializar_extensoes(&mut self) {
        if self.extensoes_prontas {
            return;
        }
        self.extensoes_prontas = true;
        let p = self.e.program;
        for ext in p.extensions.iter() {
            if !self.e_usuario_lib(ext.library) {
                continue;
            }
            let mut membros: Vec<(dartforge_intern::SymbolId, FunctionElementId)> =
                ext.instance_members.iter().map(|(s, f)| (*s, *f)).collect();
            membros.sort_by_key(|(_, f)| *f);
            for (chave, f) in membros {
                let nome = self.nome(chave).to_string();
                if self.sel.contains(nome.as_str()) || p.function(f).kind == FunctionKind::Operator {
                    self.viva_fn(f);
                } else {
                    self.pendentes.entry(nome).or_default().push(f);
                }
            }
        }
    }

    // ------------------------------------------------------------ o laço

    fn rodar(&mut self) {
        self.rodar_itens(usize::MAX);
    }

    fn rodar_itens(&mut self, _limite: usize) {
        while let Some(it) = self.fila.pop_front() {
            self.itens += 1;
            self.causa = match it {
                Item::Funcao(f) => Causa::Funcao(f),
                Item::Variavel(v) => Causa::Variavel(v),
                Item::Campos(c) => Causa::Classe(c),
            };
            match it {
                Item::Funcao(f) => impacto::de_funcao(self, f),
                Item::Variavel(v) => impacto::de_variavel(self, v),
                Item::Campos(c) => impacto::de_campos(self, c),
            }
        }
    }

    fn finalizar(self) -> Mundo {
        let p = self.e.program;
        let mut est = Estatisticas::default();
        for (i, c) in p.classes.iter().enumerate() {
            if p.library(c.library).is_sdk {
                continue;
            }
            est.classes_usuario += 1;
            match self.classes[i] {
                NivelClasse::Instanciada => est.classes_instanciadas += 1,
                NivelClasse::Tipo => est.classes_tipo += 1,
                NivelClasse::Morta => {}
            }
        }
        for (i, f) in p.functions.iter().enumerate() {
            if p.library(f.library).is_sdk {
                continue;
            }
            est.funcoes_usuario += 1;
            if self.f_vivo[i] {
                est.funcoes_vivas += 1;
            }
        }
        est.variaveis_vivas = self.v_vivo.iter().filter(|v| **v).count();
        est.seletores = self.sel.len();
        est.itens_processados = self.itens;
        Mundo {
            classes: self.classes,
            funcoes: self.f_vivo,
            variaveis: self.v_vivo,
            tearoffs: self.tearoff,
            seletores: self.sel,
            causa_fn: self.causa_fn,
            causa_classe: self.causa_classe,
            causa_var: self.causa_var,
            estat: est,
        }
    }

    // ------------------------------------------------------------ resolução por nome

    /// Construtor `nome` da classe `c` (`""`/`new` é o sem nome).
    pub(crate) fn construtor(&self, c: ClassId, nome: &str) -> Option<FunctionElementId> {
        let n = if nome == "new" { "" } else { nome };
        let sym = self.e.interner.lookup(n)?;
        self.e.program.class(c).constructors.get(&sym).copied()
    }

    /// Construtor chamado por `super.nome(..)` a partir de `k`: sobe as
    /// aplicações de mixin sintéticas até achar quem o declara.
    pub(crate) fn construtor_super(&self, k: ClassId, nome: &str) -> Option<FunctionElementId> {
        let p = self.e.program;
        let mut cur = p.class(k).supertype_class;
        let mut n = 0;
        while let Some(s) = cur {
            if let Some(f) = self.construtor(s, nome) {
                return Some(f);
            }
            // Aplicação de mixin (sintética ou `class C = S with M`) e classe
            // abstrata sem construtor: o emissor escreve um `new` que só
            // repassa para a superclasse (`emit_super_call_default`).
            let sem_ctor = p.class(s).constructors.is_empty();
            if (p.class(s).decl.is_some() && !sem_ctor) || n > 32 {
                return None;
            }
            cur = p.class(s).supertype_class;
            n += 1;
        }
        None
    }

    /// Todas as ligações de `nome` visíveis sem prefixo em `lib`, mais os
    /// estáticos da classe e da extensão envolventes.
    pub(crate) fn usar_nome(&mut self, ctx: &impacto::Contexto, nome: SymbolId) {
        let p = self.e.program;
        let texto = self.nome(nome);
        let setter = self.e.interner.lookup(&format!("{texto}_="));
        if let Some(c) = ctx.classe {
            let class = p.class(c);
            let mut fs: Vec<FunctionElementId> = Vec::new();
            fs.extend(class.static_members.get(&nome).copied());
            if let Some(s) = setter {
                fs.extend(class.static_members.get(&s).copied());
            }
            for f in fs {
                self.usar_funcao(f);
            }
        }
        if let Some(x) = ctx.extensao {
            self.usar_estatico_de_extensao(x, nome);
        }
        if let Some(b) = p.lookup(ctx.biblioteca, nome) {
            for el in [b.getter, b.setter].into_iter().flatten() {
                self.usar_elemento(el);
            }
        }
    }

    /// `Ext.nome` / `nome` dentro da extensão: membro estático ou campo
    /// estático (os campos de extensão não têm acessor no modelo de elementos,
    /// só `ExtensionElement::fields`).
    pub(crate) fn usar_estatico_de_extensao(&mut self, x: dartforge_elements::model::ExtensionId, nome: SymbolId) {
        let p = self.e.program;
        let ext = p.extension(x);
        let setter = self.e.interner.lookup(&format!("{}_=", self.nome(nome)));
        let mut fs: Vec<FunctionElementId> = Vec::new();
        fs.extend(ext.static_members.get(&nome).copied());
        if let Some(s) = setter {
            fs.extend(ext.static_members.get(&s).copied());
        }
        for f in fs {
            self.usar_funcao(f);
        }
        let vs: Vec<VariableId> = ext.fields.iter().copied().filter(|v| p.variable(*v).name == nome).collect();
        for v in vs {
            self.usar_variavel(v);
        }
    }

    pub(crate) fn usar_elemento(&mut self, el: Element) {
        match el {
            Element::Class(c) => self.marcar_tipo(c),
            Element::Extension(_) | Element::Prefix(..) => {}
            Element::Typedef(t) => {
                if let Some(td) = self.e.outline.typedefs.get(t.0 as usize) {
                    let tt = td.target_type;
                    self.tipo_de(tt);
                }
            }
            Element::Function(f) => self.usar_funcao(f),
            Element::Variable(v) => self.usar_variavel(v),
        }
    }

    /// Nó da AST de uma variável (inicializador), para o impacto.
    pub(crate) fn no_da_variavel(&self, v: VariableId) -> Option<(dartforge_elements::model::UnitId, Option<ast::ExprId>)> {
        let p = self.e.program;
        match p.variable(v).node {
            VariableRef::TopLevel { unit, decl, index } => match &p.unit(unit).ast.decl(decl).kind {
                ast::DeclKind::Variables(l) => Some((unit, l.variables.get(index).and_then(|x| x.initializer))),
                _ => None,
            },
            VariableRef::Field { unit, member, index } => match &p.unit(unit).ast.member(member).kind {
                ast::MemberKind::Field(l) => Some((unit, l.variables.get(index).and_then(|x| x.initializer))),
                _ => None,
            },
            VariableRef::EnumConstant { .. } | VariableRef::Representation { .. } | VariableRef::None => None,
        }
    }

    pub(crate) fn no_da_funcao(&self, f: FunctionElementId) -> FunctionRef {
        self.e.program.function(f).node
    }
}
