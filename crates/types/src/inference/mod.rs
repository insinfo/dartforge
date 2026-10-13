//! Inferência de tipos dos corpos: expressões, instruções, padrões e fluxo.
//!
//! Segue a especificação de inferência (`references/dart-language/resources/
//! type-system/inference.md`), a análise de fluxo (`flow-analysis.md`) e, onde
//! a especificação é omissa, a implementação oficial (`pkg/analyzer/lib/src/
//! dart/resolver`, `_fe_analyzer_shared/lib/src/type_inference`).
//!
//! Organização:
//!
//! * `corpo` — estado de um corpo (locais, escopos, contexto de função);
//! * `fluxo` — modelo de fluxo: promoção, atribuição definitiva, alcance;
//! * `membros` — busca de membros (interface, estáticos, extensões);
//! * `expr` — expressões, com contexto (esquema) para baixo e tipo para cima;
//! * `chamadas` — invocações e inferência de argumentos de tipo;
//! * `colecoes` — literais de lista, conjunto e mapa;
//! * `funcoes` — corpos, expressões de função e funções locais;
//! * `instrucoes` — instruções;
//! * `padroes` — padrões (Dart 3);
//! * `tipos` — operações de tipo usadas por todos (subtipo, UP, flatten…).
//!
//! A inferência de topo (variáveis e campos sem tipo) é sob demanda
//! (`BodyInferrer::tipo_variavel`): quem lê a variável dispara a inferência
//! do inicializador, com detecção de ciclo (`inference.md`, "Top-level
//! inference procedure").

mod shorthands;
mod calls;
mod collections;
mod body;
mod expr;
mod flow;
mod integers;
mod functions;
mod statements;
mod members;
mod non_promotion;
mod patterns;
mod overrides;
mod types;

use crate::resolved::{BodyTypes, UnitBodyTypes};
use crate::table::{CoreTypes, TypeId, TypeTable};
use body::Body;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{ExtensionId, FunctionElementId, LibraryId, Program, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast;
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Estado da inferência de topo de uma variável.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VarState {
    Pendente,
    InProgress,
    Ready,
}

/// Símbolos usados com frequência, resolvidos uma vez.
pub(crate) struct Symbols {
    pub vazio: Option<SymbolId>,
    pub call: Option<SymbolId>,
    pub new_: Option<SymbolId>,
    pub indice: Option<SymbolId>,
    pub set_index: Option<SymbolId>,
    pub igual: Option<SymbolId>,
    pub unary_minus: Option<SymbolId>,
    pub til: Option<SymbolId>,
    pub remainder: Option<SymbolId>,
    pub clamp: Option<SymbolId>,
    pub this_: Option<SymbolId>,
}

impl Symbols {
    fn new(i: &Interner) -> Self {
        Self {
            vazio: i.lookup(""),
            call: i.lookup("call"),
            new_: i.lookup("new"),
            indice: i.lookup("[]"),
            set_index: i.lookup("[]="),
            igual: i.lookup("=="),
            unary_minus: i.lookup("unary-"),
            til: i.lookup("~"),
            remainder: i.lookup("remainder"),
            clamp: i.lookup("clamp"),
            this_: i.lookup("this"),
        }
    }
}

/// Um corpo inferível isoladamente (ver [`BodyInferrer::apenas_corpos`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodyRef {
    /// Função de topo, método, construtor, getter/setter ou função local
    /// declarada como elemento.
    Funcao(FunctionElementId),
    /// O inicializador de uma variável de topo ou de um campo.
    Variavel(VariableId),
}

/// O corpo que contém o offset `pos` da unidade `u`: o executável ou o
/// inicializador de variável cujo nó o envolve (o menor).
pub fn corpo_no_offset(program: &Program, u: UnitId, pos: usize) -> Option<BodyRef> {
    use dartforge_elements::model::FunctionRef;
    let a = &program.unit(u).ast;
    let mut best: Option<(usize, BodyRef)> = None;
    for (i, fe) in program.functions.iter().enumerate() {
        let span = match fe.node {
            FunctionRef::Function { unit, function } if unit == u => a.function(function).span,
            FunctionRef::Constructor { unit, member } if unit == u => a.member(member).span,
            _ => continue,
        };
        if span.start <= pos && pos <= span.end && best.is_none_or(|(t, _)| span.end - span.start < t) {
            best = Some((span.end - span.start, BodyRef::Funcao(FunctionElementId(i as u32))));
        }
    }
    for (i, v) in program.variables.iter().enumerate() {
        let span = match v.node {
            VariableRef::TopLevel { unit, decl, .. } if unit == u => a.decl(decl).span,
            VariableRef::Field { unit, member, .. } if unit == u => a.member(member).span,
            _ => continue,
        };
        if span.start <= pos && pos <= span.end && best.is_none_or(|(t, _)| span.end - span.start < t) {
            best = Some((span.end - span.start, BodyRef::Variavel(VariableId(i as u32))));
        }
    }
    best.map(|(_, c)| c)
}

/// Os corpos da unidade `u`, na ordem da inferência completa (variáveis,
/// depois funções). Com `todos = false`, só os inicializadores de variáveis
/// sem tipo escrito que o esboço já inferiu: o que a inferência de uma
/// biblioteca cujos corpos não foram pedidos grava nas tabelas desta unidade
/// (a inferência de topo sob demanda).
pub fn corpos_da_unidade(program: &Program, outline: &crate::resolve::OutlineTypes, u: UnitId, all: bool) -> Vec<BodyRef> {
    use dartforge_elements::model::FunctionRef;
    let mut v = Vec::new();
    for (i, var) in program.variables.iter().enumerate() {
        let unit_id = match var.node {
            VariableRef::TopLevel { unit, .. } | VariableRef::Field { unit, .. } | VariableRef::EnumConstant { unit, .. } | VariableRef::Representation { unit, .. } => unit,
            VariableRef::None => continue,
        };
        if unit_id != u {
            continue;
        }
        let d = &outline.variables[i];
        if all || (d.declared_type.is_none() && d.inferred.is_some()) {
            v.push(BodyRef::Variavel(VariableId(i as u32)));
        }
    }
    if !all {
        return v;
    }
    for (i, f) in program.functions.iter().enumerate() {
        let its = match f.node {
            FunctionRef::Function { unit, .. } | FunctionRef::Constructor { unit, .. } => unit == u,
            // Sintéticas (construtor implícito, acessores de campo): pela
            // declaração da classe ou pela variável de origem.
            FunctionRef::None => {
                f.class.and_then(|c| program.class(c).decl).is_some_and(|d| d.unit == u && f.variable.is_none())
                    || f.variable.is_some_and(|x| match program.variable(x).node {
                        VariableRef::TopLevel { unit, .. } | VariableRef::Field { unit, .. } | VariableRef::EnumConstant { unit, .. } | VariableRef::Representation { unit, .. } => unit == u,
                        VariableRef::None => false,
                    })
            }
        };
        if its {
            v.push(BodyRef::Funcao(FunctionElementId(i as u32)));
        }
    }
    v
}

/// O resultado de [`inferir_corpos`].
pub struct InferredBodies {
    /// As tabelas da unidade, preenchidas pelos corpos pedidos.
    pub tabelas: UnitBodyTypes,
    /// Os diagnósticos desses corpos.
    pub diagnosticos: Vec<Diagnostic>,
    /// O escopo capturado pela sonda, quando pedida e alcançada.
    pub escopo: Option<crate::resolved::EscopoSondado>,
}

/// Infere só `corpos` (todos da unidade `unidade`) sobre um esboço e uma
/// tabela de tipos já povoados por uma inferência anterior do mesmo
/// programa (docs/LSP-ESPECIFICACAO.md §16.5, E1): os tipos de variáveis já
/// inferidos valem, as sobrescritas de campo já foram completadas (a lista
/// do esboço está vazia) e a inferência de sobrescritas já rodou. Com
/// `metadados`, as anotações da unidade também (a recriação das tabelas de
/// uma unidade inteira; o completar não as pede).
///
/// Invariante: para todo corpo `c`, as tabelas e os diagnósticos de `c` são
/// os que a inferência completa do programa dá a `c`.
#[allow(clippy::too_many_arguments)]
pub fn inferir_corpos(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &mut crate::resolve::OutlineTypes,
    unit_id: UnitId,
    bodies: &[BodyRef],
    metadata: bool,
    probe: Option<(UnitId, usize)>,
    record_locals: bool,
) -> InferredBodies {
    let mut inf = BodyInferrer::with_tables(program, interner, table, core, outline, Some(unit_id));
    inf.apenas_bibliotecas = Some(HashSet::from([program.unit(unit_id).library.0]));
    inf.registrar_locais = record_locals;
    inf.sonda_escopo = probe;
    inf.use_retained_sketch();
    inf.complete_field_overrides();
    inf.infer_overrides();
    for &c in bodies {
        match c {
            BodyRef::Variavel(v) => inf.reinfer_variable(v),
            BodyRef::Funcao(f) => functions::infer_declared_function(&mut inf, f),
        }
    }
    if metadata {
        inf.current_unit = Some(unit_id);
        functions::infer_unit_metadata(&mut inf, unit_id);
    }
    InferredBodies {
        tabelas: std::mem::take(&mut inf.body_types.units[unit_id.0 as usize]),
        diagnosticos: std::mem::take(&mut inf.diagnostics),
        escopo: inf.probed_scope.take(),
    }
}

/// Contexto de inferência de corpos para o programa inteiro.
pub struct BodyInferrer<'a> {
    pub program: &'a Program,
    pub interner: &'a Interner,
    pub table: &'a mut TypeTable,
    pub core: &'a CoreTypes,
    pub outline: &'a mut crate::resolve::OutlineTypes,
    pub diagnostics: Vec<Diagnostic>,
    pub body_types: BodyTypes,
    /// Sessão residente: quando `Some`, só os corpos de funções destas
    /// bibliotecas são inferidos — as outras não serão reemitidas, e o
    /// emissor só consulta os corpos do que emite. Inicializadores de
    /// variáveis continuam sendo inferidos em todas as bibliotecas, porque o
    /// tipo inferido de uma variável (`var x = 1;`) é lido por quem a usa.
    pub apenas_bibliotecas: Option<HashSet<u32>>,
    /// Inferência incremental (o completar do LSP): quando `Some`, só estes
    /// corpos (funções/métodos/construtores e inicializadores de variáveis)
    /// são inferidos; os tipos de variáveis de topo lidos por eles seguem sob
    /// demanda. Os metadados das unidades não são visitados.
    pub only_bodies: Option<HashSet<BodyRef>>,
    vars_state: Vec<VarState>,
    /// Inicializadores já visitados para as tabelas laterais.
    visited_initializer: Vec<bool>,
    extensoes: HashMap<u32, Rc<[ExtensionId]>>,
    pub(crate) sym: Symbols,
    /// Profundidade de inferências de topo aninhadas (proteção de pilha).
    top_depth: u32,
    /// Parâmetros auxiliares (E, K, V) da inferência de literais de coleção.
    pub(crate) collection_params_cache: Option<[crate::table::TypeParamId; 3]>,
    /// Parâmetros novos, por classe, da inferência de construtores.
    pub(crate) constructor_params: HashMap<u32, Vec<crate::table::TypeParamId>>,
    /// O `FieldPromotability` de cada biblioteca já consultada.
    pub(crate) promotion_by_library: HashMap<LibraryId, std::rc::Rc<crate::field_promotion::LibraryPromotion>>,
    /// O `whyNotPromoted` de cada leitura de referência (local ou
    /// propriedade) que tem algum motivo: `tipo -> motivo`, na ordem do mapa
    /// do fluxo.
    pub(crate) non_promotions: HashMap<(UnitId, ast::ExprId), Vec<(TypeId, flow::NonPromotionReason)>>,
    /// Contexto refinado dos argumentos de `clamp`/`remainder` para a
    /// próxima invocação.
    pub(crate) pending_numeric_context: Option<TypeId>,
    /// O `errorEntity` da próxima invocação genérica (onde vai o
    /// `COULD_NOT_INFER`) e os nomes dos parâmetros posicionais do alvo
    /// (para a mensagem); só quem conhece o alvo os define.
    pub(crate) inference_entity: Option<Span>,
    /// Parâmetros cujo tipo escrito não resolve (`InvalidType` no
    /// analyzer), pela unidade e o offset do nome: a leitura deles tem o tipo
    /// de recuperação.
    pub(crate) invalid_locals: HashSet<(UnitId, usize)>,
    pub(crate) nomes_posicionais: Option<Vec<String>>,
    /// Alvo da próxima verificação de aridade (`chamadas::verificar_aridade`):
    /// o nome citado nos `NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_*` e o
    /// intervalo do `MISSING_REQUIRED_ARGUMENT`; só quem conhece a chamada o define.
    pub(crate) arity_target: Option<calls::ArityTarget>,
    /// A próxima [`chamadas::invocar`] é a de um `call` implícito: a função
    /// invocada tem tipo de interface, e o `_checkInvocationTypeArguments`
    /// (`type_arguments_verifier.dart`) não confere os limites dos
    /// argumentos de tipo escritos (o tipo genérico não é `FunctionType`).
    pub(crate) implicit_call: bool,
    /// Unidade de cada diagnóstico (paralelo a `diagnostics`), para quem
    /// precisa do arquivo (ferramentas; o LSP).
    pub warning_units: Vec<Option<UnitId>>,
    /// Unidade do corpo em inferência.
    pub(crate) current_unit: Option<UnitId>,
    /// Tipos dos espalhamentos de um `{…}` só de espalhamentos, já
    /// inferidos sem contexto para decidir entre conjunto e mapa (a visita
    /// dos elementos os reaproveita em vez de inferir de novo).
    pub(crate) inferred_spreads: HashMap<dartforge_frontend::ast::ExprId, TypeId>,
    /// O tipo de cada espalhamento depois da visita do literal, para a
    /// forma pelos elementos (`colecoes::literal`).
    pub(crate) visited_spreads: HashMap<dartforge_frontend::ast::ExprId, TypeId>,
    /// LSP: registra em [`UnitBodyTypes::declaracoes_de_locais`] a
    /// declaração de cada local referido (renomear, referências). Desligado
    /// no compilador, que não paga pela tabela.
    pub registrar_locais: bool,
    /// LSP (completar): o identificador simples em `(unidade, offset do
    /// nome)` tem o escopo léxico capturado em [`BodyInferrer::escopo_sondado`]
    /// quando a inferência passa por ele.
    pub sonda_escopo: Option<(UnitId, usize)>,
    /// O escopo capturado pela [`BodyInferrer::sonda_escopo`].
    pub probed_scope: Option<crate::resolved::EscopoSondado>,
    /// O contexto da próxima anotação resolvida (lido e zerado por
    /// `resolver_anotacao`).
    pub(crate) type_context: crate::resolve::TypeContext,
    /// Os locais do corpo visíveis na anotação em resolução (ver
    /// [`Corpo::locais_visiveis`](corpo::Corpo::locais_visiveis)).
    pub(crate) locals_as_type: HashMap<SymbolId, (Span, bool)>,
    /// A classe e a extensão cujos membros estão em escopo na anotação.
    pub(crate) type_container: (Option<dartforge_elements::model::ClassId>, Option<ExtensionId>),
    /// Ambiguidade de extensão da última busca de membro: `(nome, lista)`.
    pub(crate) extension_ambiguity: Option<(String, String, Option<(String, String)>)>,
    /// A anotação em resolução está num método ou campo estático.
    pub(crate) in_static_member: bool,
    /// O `InheritanceManager3` (as interfaces das classes), com os tipos
    /// dos campos inferidos sob demanda.
    pub(crate) heranca: crate::heranca::Heranca,
    /// Os pais das expressões de cada unidade, sob demanda (as regras que
    /// dependem do lugar sintático de um nó).
    pais: HashMap<UnitId, Rc<dartforge_frontend::pais::Pais>>,
    /// `_inferring` da inferência de topo: as variáveis cujo inicializador
    /// está sendo inferido, na ordem.
    variable_stack: Vec<VariableId>,
    /// As variáveis num ciclo de inferência (`dependencyCycle`): o tipo é
    /// `dynamic`, e o relato já saiu.
    in_cycle: HashSet<VariableId>,
    /// Na busca dos sobrescritos de um campo sem tipo, a classe dele: os
    /// campos declarados nela não são tipados ao montar a interface
    /// ([`crate::heranca::Provedor::tipo_do_membro`]).
    pub(crate) declared_untyped: Option<dartforge_elements::model::ClassId>,
    /// Alguma interface do cache foi montada com um campo não tipado.
    pub(crate) provisional_inheritance: bool,
    /// O nome (sem o `=`) buscado na interface durante uma inferência de
    /// topo: os campos ainda não inferidos com outro nome ficam provisórios
    /// ([`crate::heranca::Provedor::tipo_do_membro`]).
    pub(crate) nome_da_busca: Option<SymbolId>,
}

/// As extensões acessíveis em `lib` (as da biblioteca e as que os imports
/// trazem, `BodyInferrer::extensoes_acessiveis`) que se aplicam a `recv`,
/// com os argumentos de tipo inferidos (`applicableTo` do analyzer, usado
/// pelo completar: `InstanceExtensionMembersOperation` e os membros de
/// extensão do `DeclarationHelper`). O receptor é tomado como está: quem
/// chama já o promoveu a não anulável (`promoteToNonNull`).
pub fn extensoes_aplicaveis(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &mut crate::resolve::OutlineTypes,
    lib: LibraryId,
    recv: TypeId,
) -> Vec<(ExtensionId, Vec<TypeId>)> {
    // Sem tabelas de corpo: só a aplicabilidade é consultada.
    let mut inf = BodyInferrer::with_tables(program, interner, table, core, outline, Some(UnitId(u32::MAX)));
    let exts = inf.accessible_extensions(lib);
    exts.iter().filter_map(|&e| inf.extensao_aplicavel(e, recv).map(|a| (e, a))).collect()
}

/// Como [`extensoes_aplicaveis`], para uma lista de extensões dada (as do
/// `exportNamespace` de uma biblioteca ainda não importada,
/// `addNotImportedExtensionMethods`).
pub fn extensoes_aplicaveis_dentre(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &mut crate::resolve::OutlineTypes,
    candidates: &[ExtensionId],
    recv: TypeId,
) -> Vec<(ExtensionId, Vec<TypeId>)> {
    let mut inf = BodyInferrer::with_tables(program, interner, table, core, outline, Some(UnitId(u32::MAX)));
    candidates.iter().filter_map(|&e| inf.extensao_aplicavel(e, recv).map(|a| (e, a))).collect()
}

impl<'a> BodyInferrer<'a> {
    pub fn new(
        program: &'a Program,
        interner: &'a Interner,
        table: &'a mut TypeTable,
        core: &'a CoreTypes,
        outline: &'a mut crate::resolve::OutlineTypes,
    ) -> Self {
        Self::with_tables(program, interner, table, core, outline, None)
    }

    /// Como [`BodyInferrer::new`]; com `so = Some(u)`, só a unidade `u` tem
    /// tabelas (a inferência de corpos isolados, [`inferir_corpos`]).
    fn with_tables(
        program: &'a Program,
        interner: &'a Interner,
        table: &'a mut TypeTable,
        core: &'a CoreTypes,
        outline: &'a mut crate::resolve::OutlineTypes,
        only: Option<UnitId>,
    ) -> Self {
        let mut units = Vec::with_capacity(program.units.len());
        for (ui, u) in program.units.iter().enumerate() {
            // Unidades do SDK não recebem inferência de corpos: tabela vazia
            // (`get_type`/`get_resolved` devolvem `None`, `set_*` ignoram).
            let allocate = match only {
                Some(s) => s.0 as usize == ui,
                None => !program.library(u.library).is_sdk,
            };
            if allocate {
                units.push(UnitBodyTypes::new(u.ast.exprs.len(), core.dynamic_));
            } else {
                units.push(UnitBodyTypes::default());
            }
        }
        let nvars = program.variables.len();
        Self {
            program,
            interner,
            table,
            core,
            outline,
            diagnostics: Vec::new(),
            body_types: BodyTypes { units, extensoes_usadas: std::collections::HashSet::new() },
            apenas_bibliotecas: None,
            only_bodies: None,
            vars_state: vec![VarState::Pendente; nvars],
            visited_initializer: vec![false; nvars],
            extensoes: HashMap::new(),
            sym: Symbols::new(interner),
            top_depth: 0,
            collection_params_cache: None,
            constructor_params: HashMap::new(),
            promotion_by_library: HashMap::new(),
            non_promotions: HashMap::new(),
            pending_numeric_context: None,
            inference_entity: None,
            invalid_locals: HashSet::new(),
            nomes_posicionais: None,
            arity_target: None,
            implicit_call: false,
            warning_units: Vec::new(),
            current_unit: None,
            heranca: crate::heranca::Heranca::default(),
            declared_untyped: None,
            provisional_inheritance: false,
            nome_da_busca: None,
            pais: HashMap::new(),
            variable_stack: Vec::new(),
            in_cycle: HashSet::new(),
            inferred_spreads: HashMap::new(),
            visited_spreads: HashMap::new(),
            registrar_locais: false,
            sonda_escopo: None,
            probed_scope: None,
            type_context: crate::resolve::TypeContext::Normal,
            locals_as_type: HashMap::new(),
            type_container: (None, None),
            extension_ambiguity: None,
            in_static_member: false,
        }
    }

    /// Dá tabelas de corpo às unidades do SDK das bibliotecas `libs` (que
    /// `new` deixa vazias): a busca de referências do LSP infere os corpos do
    /// SDK que citam o nome procurado (docs/LSP-ESPECIFICACAO.md §11.9 B).
    pub fn alocar_corpos_de(&mut self, libs: &[LibraryId]) {
        for (i, u) in self.program.units.iter().enumerate() {
            if libs.contains(&u.library) && self.program.library(u.library).is_sdk && self.body_types.units[i].static_types.is_empty() {
                self.body_types.units[i] = UnitBodyTypes::new(u.ast.exprs.len(), self.core.dynamic_);
            }
        }
    }

    /// Os tipos de variáveis já inferidos no esboço valem (um esboço retido
    /// de uma inferência anterior do mesmo programa, docs/LSP-ESPECIFICACAO.md
    /// §16.5): o estado delas começa pronto, e só as pedidas são reinferidas.
    fn use_retained_sketch(&mut self) {
        for (i, d) in self.outline.variables.iter().enumerate() {
            if d.declared_type.is_none() && d.inferred.is_some() {
                self.vars_state[i] = VarState::Ready;
            }
        }
    }

    /// O inicializador de `vid` de novo, para as tabelas da unidade: com
    /// tipo escrito, a visita; sem, a inferência de topo, mantendo o tipo do
    /// esboço (o inicializador de variável sem tipo é assinatura: numa troca
    /// de classe Corpo ele não mudou; na troca especulativa do completar, o
    /// tipo novo não é gravado).
    fn reinfer_variable(&mut self, vid: VariableId) {
        let d = &self.outline.variables[vid.0 as usize];
        if d.declared_type.is_some() {
            self.visit_initializer(vid);
            return;
        }
        let before = d.inferred;
        self.vars_state[vid.0 as usize] = VarState::Pendente;
        let t = self.variable_type(vid);
        if let Some(a) = before
            && a != t
        {
            self.outline.variables[vid.0 as usize].inferred = Some(a);
            self.sync_accessors(vid, a);
        }
    }

    /// Ponto de entrada: infere inicializadores de variáveis e corpos.
    pub fn infer_all(self) -> (BodyTypes, Vec<Diagnostic>) {
        let (b, d, _) = self.infer_all_with_units();
        (b, d)
    }

    /// Como [`BodyInferrer::infer_all`], com a unidade de cada diagnóstico.
    pub fn infer_all_with_units(mut self) -> (BodyTypes, Vec<Diagnostic>, Vec<Option<UnitId>>) {
        self.infer_everything();
        (self.body_types, self.diagnostics, self.warning_units)
    }

    /// Como [`BodyInferrer::infer_all`], devolvendo também o escopo capturado
    /// pela [`BodyInferrer::sonda_escopo`] (o completar do LSP).
    pub fn infer_all_com_sonda(mut self) -> (BodyTypes, Vec<Diagnostic>, Option<crate::resolved::EscopoSondado>) {
        self.infer_everything();
        (self.body_types, self.diagnostics, self.probed_scope)
    }

    /// Infere inicializadores, corpos e metadados, enchendo as tabelas.
    fn infer_everything(&mut self) {
        // Bibliotecas do SDK pedidas explicitamente (o nativo compila o SDK
        // da fonte) ganham tabelas laterais como as do usuário.
        if let Some(requested) = self.apenas_bibliotecas.clone() {
            for (ui, u) in self.program.units.iter().enumerate() {
                if self.program.library(u.library).is_sdk && requested.contains(&u.library.0) {
                    self.body_types.units[ui] = UnitBodyTypes::new(u.ast.exprs.len(), self.core.dynamic_);
                }
            }
        }
        self.complete_field_overrides();
        // `_performOverrideInference`: a assinatura combinada dos
        // sobrescritos nos tipos omitidos dos membros de instância.
        self.infer_overrides();
        if let Some(bodies) = self.only_bodies.clone() {
            for c in bodies {
                match c {
                    BodyRef::Variavel(vid) => {
                        self.variable_type(vid);
                        self.visit_initializer(vid);
                    }
                    BodyRef::Funcao(f) => functions::infer_declared_function(self, f),
                }
            }
            return;
        }
        for v in 0..self.program.variables.len() {
            let vid = VariableId(v as u32);
            // Sessão residente: variáveis de bibliotecas que não serão
            // reemitidas só são inferidas sob demanda (quem as lê pede o tipo).
            let lib = self.program.variable(vid).library;
            if !self.infer_bodies_of(lib) {
                continue;
            }
            self.variable_type(vid);
            self.visit_initializer(vid);
        }
        for f in 0..self.program.functions.len() {
            let lib = self.program.functions[f].library;
            if !self.infer_bodies_of(lib) {
                continue;
            }
            functions::infer_declared_function(self, FunctionElementId(f as u32));
        }
        for ui in 0..self.program.units.len() {
            let lib = self.program.units[ui].library;
            if !self.infer_bodies_of(lib) {
                continue;
            }
            self.current_unit = Some(UnitId(ui as u32));
            functions::infer_unit_metadata(self, UnitId(ui as u32));
        }
    }

    /// Os corpos de `lib` são inferidos: os pedidos, quando há pedido
    /// (inclusive do SDK); senão todos menos os do SDK.
    pub(crate) fn infer_bodies_of(&self, lib: LibraryId) -> bool {
        match &self.apenas_bibliotecas {
            Some(s) => s.contains(&lib.0),
            None => !self.program.library(lib).is_sdk,
        }
    }

    /// Tipo de uma variável de topo ou campo, inferindo o inicializador sob
    /// demanda quando o tipo foi omitido.
    pub(crate) fn variable_type(&mut self, vid: VariableId) -> TypeId {
        let d = &self.outline.variables[vid.0 as usize];
        if let Some(t) = d.declared_type {
            return t;
        }
        match self.vars_state[vid.0 as usize] {
            VarState::Ready => return d.inferred.unwrap_or(self.core.dynamic_),
            // Ciclo de inferência: `TopLevelInference` (`top_level_inference.dart:228-240`).
            VarState::InProgress => {
                self.inference_cycle(vid);
                return self.core.dynamic_;
            }
            VarState::Pendente => {}
        }
        if self.top_depth > 200 {
            return self.core.dynamic_;
        }
        self.vars_state[vid.0 as usize] = VarState::InProgress;
        self.top_depth += 1;
        let diags_before = self.diagnostics.len();
        let saved_unit = self.current_unit;
        self.variable_stack.push(vid);
        let t = functions::infer_untyped_variable_type(self, vid);
        self.variable_stack.pop();
        // Num ciclo, o tipo já ficou `dynamic` (o resultado do inicializador
        // não conta).
        let t = if self.in_cycle.contains(&vid) { self.core.dynamic_ } else { t };
        self.current_unit = saved_unit;
        // Inferência sob demanda de uma variável cujos corpos não foram
        // pedidos (SDK, biblioteca não reemitida): os avisos não são deste
        // pedido.
        if !self.infer_bodies_of(self.program.variable(vid).library) {
            self.diagnostics.truncate(diags_before);
            self.warning_units.truncate(diags_before);
        }
        self.top_depth -= 1;
        self.outline.variables[vid.0 as usize].inferred = Some(t);
        self.vars_state[vid.0 as usize] = VarState::Ready;
        self.visited_initializer[vid.0 as usize] = true;
        self.sync_accessors(vid, t);
        t
    }

    /// O ciclo achado ao pedir de novo o tipo de `vid`: as variáveis da pilha
    /// a partir dela, ainda em inferência, ficam `dynamic` e relatam
    /// `TOP_LEVEL_CYCLE` (`ResolverVisitor._checkTopLevelCycle`,
    /// `resolver.dart:4009-4030`, só as não `const`) no nome, com os nomes do
    /// ciclo em ordem.
    fn inference_cycle(&mut self, vid: VariableId) {
        let Some(start) = self.variable_stack.iter().position(|&v| v == vid) else { return };
        let cycle: Vec<VariableId> = self.variable_stack[start..].iter().copied().filter(|v| !self.in_cycle.contains(v)).collect();
        if cycle.is_empty() {
            return;
        }
        let mut names: Vec<String> = self.variable_stack[start..].iter().map(|&v| self.interner.resolve(self.program.variable(v).name).to_string()).collect();
        names.sort();
        let list = names.join(", ");
        let saved_unit = self.current_unit;
        for v in cycle {
            self.in_cycle.insert(v);
            let var = self.program.variable(v);
            if var.const_ {
                continue;
            }
            let (unit, span) = match var.node {
                VariableRef::TopLevel { unit, decl, index } => match &self.program.unit(unit).ast.decl(decl).kind {
                    ast::DeclKind::Variables(l) => match l.variables.get(index) {
                        Some(x) => (unit, x.name.span),
                        None => continue,
                    },
                    _ => continue,
                },
                VariableRef::Field { unit, member, index } => match &self.program.unit(unit).ast.member(member).kind {
                    ast::MemberKind::Field(l) => match l.variables.get(index) {
                        Some(x) => (unit, x.name.span),
                        None => continue,
                    },
                    _ => continue,
                },
                _ => continue,
            };
            let name = self.interner.resolve(var.name).to_string();
            self.current_unit = Some(unit);
            self.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::TOP_LEVEL_CYCLE, span, &[&name, &list]);
        }
        self.current_unit = saved_unit;
    }

    /// Override inference sobre campo sem tipo escrito: o getter (retorno) ou
    /// o setter (parâmetro) sobrescritor que omitiu o tipo herda o tipo
    /// inferido do campo sobreposto, instanciado na classe do sobrescritor.
    fn complete_field_overrides(&mut self) {
        let pendentes = std::mem::take(&mut self.outline.field_overrides);
        for p in &pendentes {
            let t = self.variable_type(p.campo);
            let subst: HashMap<_, _> = p.subst.iter().copied().collect();
            let t = crate::ops::substitute(t, &subst, self.table);
            let fd = &mut self.outline.functions[p.funcao.0 as usize];
            match p.parametro {
                None => fd.return_type = t,
                Some(i) => match fd.parameters.get_mut(i) {
                    Some(par) => par.ty = t,
                    None => continue,
                },
            }
            let fd = &self.outline.functions[p.funcao.0 as usize];
            let (mut pos, mut opt, mut the_named) = (Vec::new(), Vec::new(), Vec::new());
            for par in fd.parameters.iter() {
                match par.kind {
                    ast::ParameterKind::Required => pos.push(par.ty),
                    ast::ParameterKind::Optional => opt.push(par.ty),
                    ast::ParameterKind::Named => {
                        if let Some(n) = par.externo {
                            the_named.push((n, par.ty, par.required));
                        }
                    }
                }
            }
            let sig = self.table.intern(crate::table::Type::Function {
                type_params: fd.type_params.clone(),
                ret: fd.return_type,
                positional: pos.into_boxed_slice(),
                optional: opt.into_boxed_slice(),
                named: the_named.into_boxed_slice(),
                nullable: false,
            });
            self.outline.functions[p.funcao.0 as usize].signature = sig;
        }
        self.outline.field_overrides = pendentes;
    }

    /// Os acessores implícitos da variável passam a ter o tipo inferido.
    fn sync_accessors(&mut self, vid: VariableId, t: TypeId) {
        let v = self.program.variable(vid);
        if let Some(g) = v.getter {
            let sig = self.table.intern(crate::table::Type::Function {
                type_params: Box::new([]),
                ret: t,
                positional: Box::new([]),
                optional: Box::new([]),
                named: Box::new([]),
                nullable: false,
            });
            let fd = &mut self.outline.functions[g.0 as usize];
            fd.return_type = t;
            fd.signature = sig;
        }
        if let Some(s) = v.setter {
            let sig = self.table.intern(crate::table::Type::Function {
                type_params: Box::new([]),
                ret: self.core.void_,
                positional: Box::new([t]),
                optional: Box::new([]),
                named: Box::new([]),
                nullable: false,
            });
            let fd = &mut self.outline.functions[s.0 as usize];
            fd.signature = sig;
            if let Some(p) = fd.parameters.first_mut() {
                p.ty = t;
            }
        }
    }

    /// Visita (uma vez) o inicializador de uma variável com tipo escrito,
    /// para preencher as tabelas laterais das suas expressões.
    fn visit_initializer(&mut self, vid: VariableId) {
        if self.visited_initializer[vid.0 as usize] {
            return;
        }
        self.visited_initializer[vid.0 as usize] = true;
        let v = self.program.variable(vid);
        if !self.infer_bodies_of(v.library) {
            return;
        }
        let Some(declared) = self.outline.variables[vid.0 as usize].declared_type else { return };
        if let Some((unit, init)) = self.inicializador(vid) {
            let mut cx = Body::to_variable(self, vid, unit);
            let t = expr::inferir(self, &mut cx, init, declared);
            expr::verify_assignable_expr(self, &cx, init, t, declared, crate::codes::INVALID_ASSIGNMENT.template);
        }
    }

    /// Os inicializadores de campo não-`late` de uma classe com construtor
    /// primário (Dart 3.13) são avaliados no escopo dos parâmetros dele
    /// (spec, "primary initializer scope"): um parâmetro sombreia o que
    /// estiver fora da classe. Os valores padrão já foram inferidos no
    /// próprio construtor.
    pub(crate) fn declare_primary_parameters(&mut self, cx: &mut Body, c: dartforge_elements::model::ClassId) {
        let Some(d) = self.program.class(c).decl else { return };
        let ast_unit = &self.program.unit(d.unit).ast;
        let primary = match &ast_unit.decl(d.decl).kind {
            ast::DeclKind::Class(cd) => cd.primary_constructor,
            ast::DeclKind::Enum(ed) => ed.primary_constructor,
            _ => None,
        };
        let Some(mem) = primary else { return };
        let ast::MemberKind::Constructor(k) = &ast_unit.member(mem).kind else { return };
        let Some(f) = self.program.functions.iter().position(|f| {
            matches!(f.node, dartforge_elements::model::FunctionRef::Constructor { unit, member } if unit == d.unit && member == mem)
        }) else {
            return;
        };
        let types: Vec<TypeId> = self.outline.functions[f].parameters.iter().map(|p| p.ty).collect();
        cx.push_scope();
        for (i, p) in k.parameters.iter().enumerate() {
            let Some(n) = &p.name else { continue };
            let ty = types.get(i).copied().unwrap_or(self.core.dynamic_);
            let id = expr::declarar_local(
                self,
                cx,
                body::Local { nome: n.sym, tipo: ty, final_: p.final_, late: false, const_: false, offset: n.span.start, funcao_local: false },
                true,
            );
            cx.primary_parameters.insert(id);
        }
    }

    /// `(unidade, expressão)` do inicializador de uma variável, se houver.
    pub(crate) fn inicializador(&self, vid: VariableId) -> Option<(UnitId, ast::ExprId)> {
        match self.program.variable(vid).node {
            VariableRef::TopLevel { unit, decl, index } => match &self.program.unit(unit).ast.decl(decl).kind {
                ast::DeclKind::Variables(vl) => vl.variables.get(index)?.initializer.map(|e| (unit, e)),
                _ => None,
            },
            VariableRef::Field { unit, member, index } => match &self.program.unit(unit).ast.member(member).kind {
                ast::MemberKind::Field(vl) => vl.variables.get(index)?.initializer.map(|e| (unit, e)),
                _ => None,
            },
            _ => None,
        }
    }

    /// Extensões acessíveis numa biblioteca: as declaradas nela (inclusive as
    /// sem nome) e as que um import não adiado traz no seu espaço de nomes
    /// (com ou sem prefixo, depois de `show`/`hide`). Como na especificação
    /// de extensões, o nome não precisa estar visível no escopo: duas
    /// `IterableExtension` importadas (a do `package:collection` e a do
    /// próprio analyzer) conflitam como nome, mas as duas continuam
    /// aplicáveis implicitamente.
    pub(crate) fn accessible_extensions(&mut self, lib: LibraryId) -> Rc<[ExtensionId]> {
        if let Some(e) = self.extensoes.get(&lib.0) {
            return e.clone();
        }
        let l = self.program.library(lib);
        let mut v: Vec<ExtensionId> = Vec::new();
        fn empurrar(e: ExtensionId, v: &mut Vec<ExtensionId>) {
            if !v.contains(&e) {
                v.push(e);
            }
        }
        for (i, e) in self.program.extensions.iter().enumerate() {
            if e.library == lib {
                empurrar(ExtensionId(i as u32), &mut v);
            }
        }
        // O escopo traz também o `dart:core` implícito (que não é diretiva).
        let mut names: Vec<(&SymbolId, &dartforge_elements::model::Binding)> = l.scope.iter().collect();
        names.sort_by_key(|(s, _)| s.as_u32());
        for (_, b) in names {
            if let Some(dartforge_elements::model::Element::Extension(e)) = b.getter {
                empurrar(e, &mut v);
            }
        }
        for import in l.imports.iter().filter(|i| !i.deferred) {
            let exported = &self.program.library(import.library).exported;
            let mut names: Vec<_> = exported
                .iter()
                .filter(|(s, _)| {
                    import.combinators.iter().all(|c| match c {
                        ast::Combinator::Show(ns) => ns.iter().any(|n| n.sym == **s),
                        ast::Combinator::Hide(ns) => !ns.iter().any(|n| n.sym == **s),
                    })
                })
                .collect();
            names.sort_by_key(|(s, _)| s.as_u32());
            for (_, b) in names {
                if let Some(dartforge_elements::model::Element::Extension(e)) = b.getter {
                    empurrar(e, &mut v);
                }
            }
        }
        let rc: Rc<[ExtensionId]> = v.into();
        self.extensoes.insert(lib.0, rc.clone());
        rc
    }

    // ---------------------------------------------------------------
    // Diagnósticos
    // ---------------------------------------------------------------

    /// O pai sintático da expressão `e` da unidade `u`.
    pub(crate) fn parent_of(&mut self, u: UnitId, e: ast::ExprId) -> dartforge_frontend::pais::Pai {
        let program = self.program;
        let pais = self.pais.entry(u).or_insert_with(|| Rc::new(crate::lints_tipados::pais_da_unidade(program, u))).clone();
        pais.pai(e)
    }

    pub(crate) fn aviso(&mut self, msg: String, span: Span) {
        self.diagnostics.push(Diagnostic::new(msg, span));
        self.warning_units.push(self.current_unit);
    }

    /// Uso sem checagem de nulo de um receptor potencialmente anulável
    /// (`codigo`, um dos `UNCHECKED_…_OF_NULLABLE_VALUE`); com o receptor do
    /// tipo `Null`, é o `INVALID_USE_OF_NULL_VALUE` (sem argumentos), no
    /// mesmo lugar.
    pub(crate) fn null_warning(&mut self, recv: TypeId, code: dartforge_diagnostics::Codigo, span: Span, args: &[&str]) {
        // `Never?` não é `Null` para o analyzer (é o `NeverType` com `?`).
        let nullable_never = matches!(self.table.exibicao(recv), Some(crate::table::Exibicao::NeverAnulavel));
        if matches!(self.table.get(recv), crate::table::Type::Null) && !nullable_never {
            self.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::INVALID_USE_OF_NULL_VALUE, span, &[]);
        } else {
            self.warning_with_code(code, span, args);
        }
    }

    /// Aviso já com o código do analyzer, a mensagem oficial e os
    /// argumentos (o desenho do T1): a ponte da paridade o deixa como está.
    pub(crate) fn warning_with_code(&mut self, code: dartforge_diagnostics::Codigo, span: Span, args: &[&str]) {
        self.diagnostics.push(Diagnostic::com_codigo(code, span, args.iter().copied()));
        self.warning_units.push(self.current_unit);
    }

    /// Aviso com código e mensagens de contexto (`DiagnosticFactory`):
    /// `(arquivo, intervalo, texto)`, o arquivo quando não é o do aviso.
    pub(crate) fn warning_with_context(&mut self, code: dartforge_diagnostics::Codigo, span: Span, args: &[&str], context: Vec<(Option<String>, Span, String)>) {
        let mut d = Diagnostic::com_codigo(code, span, args.iter().copied());
        d.contexto.extend(context.into_iter().map(|(file, span, message)| dartforge_diagnostics::Contexto { arquivo: file.map(Into::into), span, mensagem: message.into() }));
        self.diagnostics.push(d);
        self.warning_units.push(self.current_unit);
    }

    /// Aviso com código cujos argumentos passam juntos pela conversão do
    /// `ErrorReporter` ([`crate::exibicao::Exibidor::argumentos`], T7): os
    /// tipos saem com alias, e dois tipos do mesmo relato com o mesmo texto
    /// ganham o `(where X is defined in …)`.
    pub(crate) fn warning_with_args(&mut self, code: dartforge_diagnostics::Codigo, span: Span, args: &[crate::exibicao::Arg<'_>]) {
        let (texts, context) = {
            let displayer = crate::exibicao::Exibidor { table: &*self.table, interner: self.interner, program: self.program };
            displayer.arguments_and_context(args)
        };
        let mut d = Diagnostic::com_codigo(code, span, texts);
        d.contexto.extend(context);
        self.diagnostics.push(d);
        self.warning_units.push(self.current_unit);
    }

    /// Registra um erro de linguagem (`codes::ERRO_DE_LINGUAGEM`,
    /// docs/VERSOES-LINGUAGEM.md §3): aborta a compilação, com o arquivo e o
    /// deslocamento na mensagem, como os erros de carga.
    pub(crate) fn erro_de_linguagem(&mut self, unit: UnitId, span: Span, msg: String) {
        let file = self.program.unit(unit).path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        let text = format!("{}{file}:{}: {msg}", crate::codes::ERRO_DE_LINGUAGEM, span.start);
        self.diagnostics.push(Diagnostic::new(text, span));
        self.warning_units.push(Some(unit));
    }

    pub(crate) fn span_expr(&self, unit: UnitId, e: ast::ExprId) -> Span {
        self.program.unit(unit).ast.expr(e).span
    }

    /// Diagnóstico de atribuibilidade: `dynamic` é atribuível a tudo (cast
    /// implícito); um objeto com `call` a um tipo de função também.
    pub(crate) fn verify_assignable(&mut self, of: TypeId, for_: TypeId, span: Span, template: &str) {
        if self.atribuivel(of, for_) {
            return;
        }
        // Os códigos com tipos nos argumentos passam pela conversão do
        // `ErrorReporter` (alias e a desambiguação `(where C is defined in …)`
        // de dois tipos com o mesmo nome).
        use crate::exibicao::Arg;
        use dartforge_diagnostics::codigos::compile_time_error as c;
        if template == crate::codes::INVALID_ASSIGNMENT.template {
            self.warning_with_args(c::INVALID_ASSIGNMENT, span, &[Arg::Tipo(of), Arg::Tipo(for_)]);
            return;
        }
        if template == crate::codes::ARGUMENT_TYPE_NOT_ASSIGNABLE.template {
            self.warning_with_args(c::ARGUMENT_TYPE_NOT_ASSIGNABLE, span, &[Arg::Tipo(of), Arg::Tipo(for_), Arg::Texto("".into())]);
            return;
        }
        let msg = format!(
            "{}: '{}' não é atribuível a '{}'",
            template,
            self.table.format(of, self.interner, self.program),
            self.table.format(for_, self.interner, self.program)
        );
        self.aviso(msg, span);
    }
}
