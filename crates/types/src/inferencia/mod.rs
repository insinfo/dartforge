//! Inferência de tipos dos corpos: expressões, instruções, padrões e fluxo.
//!
//! Segue a especificação de inferência (`references/dart-language/resources/
//! type-system/inference.md`), a análise de fluxo (`flow-analysis.md`) e, onde
//! a especificação é omissa, a implementação oficial (`pkg/analyzer/lib/src/
//! dart/resolver`, `_fe_analyzer_shared/lib/src/type_inference`).
//!
//! Organização:
//!
//! * [`corpo`] — estado de um corpo (locais, escopos, contexto de função);
//! * [`fluxo`] — modelo de fluxo: promoção, atribuição definitiva, alcance;
//! * [`membros`] — busca de membros (interface, estáticos, extensões);
//! * [`expr`] — expressões, com contexto (esquema) para baixo e tipo para cima;
//! * [`chamadas`] — invocações e inferência de argumentos de tipo;
//! * [`colecoes`] — literais de lista, conjunto e mapa;
//! * [`funcoes`] — corpos, expressões de função e funções locais;
//! * [`instrucoes`] — instruções;
//! * [`padroes`] — padrões (Dart 3);
//! * [`tipos`] — operações de tipo usadas por todos (subtipo, UP, flatten…).
//!
//! A inferência de topo (variáveis e campos sem tipo) é sob demanda
//! ([`BodyInferrer::tipo_variavel`]): quem lê a variável dispara a inferência
//! do inicializador, com detecção de ciclo (`inference.md`, "Top-level
//! inference procedure").

mod chamadas;
mod colecoes;
mod corpo;
mod expr;
mod fluxo;
mod funcoes;
mod instrucoes;
mod membros;
mod padroes;
mod tipos;

use crate::resolved::{BodyTypes, UnitBodyTypes};
use crate::table::{CoreTypes, TypeId, TypeTable};
use corpo::Corpo;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{ExtensionId, FunctionElementId, LibraryId, Program, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast;
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Estado da inferência de topo de uma variável.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EstadoVar {
    Pendente,
    EmCurso,
    Pronta,
}

/// Símbolos usados com frequência, resolvidos uma vez.
pub(crate) struct Simbolos {
    pub vazio: Option<SymbolId>,
    pub call: Option<SymbolId>,
    pub new_: Option<SymbolId>,
    pub indice: Option<SymbolId>,
    pub indice_set: Option<SymbolId>,
    pub igual: Option<SymbolId>,
    pub menos_unario: Option<SymbolId>,
    pub til: Option<SymbolId>,
    pub remainder: Option<SymbolId>,
    pub clamp: Option<SymbolId>,
    pub this_: Option<SymbolId>,
}

impl Simbolos {
    fn new(i: &Interner) -> Self {
        Self {
            vazio: i.lookup(""),
            call: i.lookup("call"),
            new_: i.lookup("new"),
            indice: i.lookup("[]"),
            indice_set: i.lookup("[]="),
            igual: i.lookup("=="),
            menos_unario: i.lookup("unary-"),
            til: i.lookup("~"),
            remainder: i.lookup("remainder"),
            clamp: i.lookup("clamp"),
            this_: i.lookup("this"),
        }
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
    estado_vars: Vec<EstadoVar>,
    /// Inicializadores já visitados para as tabelas laterais.
    inicializador_visitado: Vec<bool>,
    extensoes: HashMap<u32, Rc<[ExtensionId]>>,
    pub(crate) sym: Simbolos,
    /// Profundidade de inferências de topo aninhadas (proteção de pilha).
    profundidade_topo: u32,
    /// Parâmetros auxiliares (E, K, V) da inferência de literais de coleção.
    pub(crate) params_colecao_cache: Option<[crate::table::TypeParamId; 3]>,
    /// Parâmetros novos, por classe, da inferência de construtores.
    pub(crate) params_construtor: HashMap<u32, Vec<crate::table::TypeParamId>>,
}

impl<'a> BodyInferrer<'a> {
    pub fn new(
        program: &'a Program,
        interner: &'a Interner,
        table: &'a mut TypeTable,
        core: &'a CoreTypes,
        outline: &'a mut crate::resolve::OutlineTypes,
    ) -> Self {
        let mut units = Vec::with_capacity(program.units.len());
        for u in &program.units {
            // Unidades do SDK não recebem inferência de corpos: tabela vazia
            // (`get_type`/`get_resolved` devolvem `None`, `set_*` ignoram).
            if program.library(u.library).is_sdk {
                units.push(UnitBodyTypes::default());
            } else {
                units.push(UnitBodyTypes::new(u.ast.exprs.len(), core.dynamic_));
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
            body_types: BodyTypes { units },
            apenas_bibliotecas: None,
            estado_vars: vec![EstadoVar::Pendente; nvars],
            inicializador_visitado: vec![false; nvars],
            extensoes: HashMap::new(),
            sym: Simbolos::new(interner),
            profundidade_topo: 0,
            params_colecao_cache: None,
            params_construtor: HashMap::new(),
        }
    }

    /// Ponto de entrada: infere inicializadores de variáveis e corpos.
    pub fn infer_all(mut self) -> (BodyTypes, Vec<Diagnostic>) {
        for v in 0..self.program.variables.len() {
            let vid = VariableId(v as u32);
            self.tipo_variavel(vid);
            self.visitar_inicializador(vid);
        }
        for f in 0..self.program.functions.len() {
            let fe = &self.program.functions[f];
            if self.program.library(fe.library).is_sdk {
                continue;
            }
            if self.apenas_bibliotecas.as_ref().is_some_and(|s| !s.contains(&fe.library.0)) {
                continue;
            }
            funcoes::inferir_funcao_declarada(&mut self, FunctionElementId(f as u32));
        }
        for ui in 0..self.program.units.len() {
            let u = &self.program.units[ui];
            if self.program.library(u.library).is_sdk {
                continue;
            }
            if self.apenas_bibliotecas.as_ref().is_some_and(|s| !s.contains(&u.library.0)) {
                continue;
            }
            funcoes::inferir_metadados_da_unidade(&mut self, UnitId(ui as u32));
        }
        (self.body_types, self.diagnostics)
    }

    /// Tipo de uma variável de topo ou campo, inferindo o inicializador sob
    /// demanda quando o tipo foi omitido.
    pub(crate) fn tipo_variavel(&mut self, vid: VariableId) -> TypeId {
        let d = &self.outline.variables[vid.0 as usize];
        if let Some(t) = d.declared_type {
            return t;
        }
        match self.estado_vars[vid.0 as usize] {
            EstadoVar::Pronta => return d.inferred.unwrap_or(self.core.dynamic_),
            // Ciclo de inferência: erro no Dart; `dynamic` para seguir.
            EstadoVar::EmCurso => return self.core.dynamic_,
            EstadoVar::Pendente => {}
        }
        if self.profundidade_topo > 200 {
            return self.core.dynamic_;
        }
        self.estado_vars[vid.0 as usize] = EstadoVar::EmCurso;
        self.profundidade_topo += 1;
        let t = funcoes::inferir_tipo_de_variavel_sem_tipo(self, vid);
        self.profundidade_topo -= 1;
        self.outline.variables[vid.0 as usize].inferred = Some(t);
        self.estado_vars[vid.0 as usize] = EstadoVar::Pronta;
        self.inicializador_visitado[vid.0 as usize] = true;
        self.sincronizar_acessores(vid, t);
        t
    }

    /// Os acessores implícitos da variável passam a ter o tipo inferido.
    fn sincronizar_acessores(&mut self, vid: VariableId, t: TypeId) {
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
    fn visitar_inicializador(&mut self, vid: VariableId) {
        if self.inicializador_visitado[vid.0 as usize] {
            return;
        }
        self.inicializador_visitado[vid.0 as usize] = true;
        let v = self.program.variable(vid);
        if self.program.library(v.library).is_sdk {
            return;
        }
        let Some(declarado) = self.outline.variables[vid.0 as usize].declared_type else { return };
        if let Some((unit, init)) = self.inicializador(vid) {
            let mut cx = Corpo::para_variavel(self, vid, unit);
            let t = expr::inferir(self, &mut cx, init, declarado);
            let span = self.span_expr(unit, init);
            self.verificar_atribuivel(t, declarado, span, crate::codes::INVALID_ASSIGNMENT.template);
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
    /// sem nome) e as importadas, com ou sem prefixo.
    pub(crate) fn extensoes_acessiveis(&mut self, lib: LibraryId) -> Rc<[ExtensionId]> {
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
        let mut nomes: Vec<(&SymbolId, &dartforge_elements::model::Binding)> = l.scope.iter().collect();
        nomes.sort_by_key(|(s, _)| s.as_u32());
        for (_, b) in nomes {
            if let Some(dartforge_elements::model::Element::Extension(e)) = b.getter {
                empurrar(e, &mut v);
            }
        }
        let mut prefixos: Vec<_> = l.prefixes.iter().collect();
        prefixos.sort_by_key(|(s, _)| s.as_u32());
        for (_, ns) in prefixos {
            let mut nomes: Vec<_> = ns.iter().collect();
            nomes.sort_by_key(|(s, _)| s.as_u32());
            for (_, b) in nomes {
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

    pub(crate) fn aviso(&mut self, msg: String, span: Span) {
        self.diagnostics.push(Diagnostic::new(msg, span));
    }

    pub(crate) fn span_expr(&self, unit: UnitId, e: ast::ExprId) -> Span {
        self.program.unit(unit).ast.expr(e).span
    }

    /// Diagnóstico de atribuibilidade: `dynamic` é atribuível a tudo (cast
    /// implícito); um objeto com `call` a um tipo de função também.
    pub(crate) fn verificar_atribuivel(&mut self, de: TypeId, para: TypeId, span: Span, template: &str) {
        if self.atribuivel(de, para) {
            return;
        }
        let msg = format!(
            "{}: '{}' não é atribuível a '{}'",
            template,
            self.table.format(de, self.interner, self.program),
            self.table.format(para, self.interner, self.program)
        );
        self.aviso(msg, span);
    }
}
