//! Type inference of bodies: expressions, statements, patterns and flow.
//!
//! Follows the inference specification (`references/dart-language/resources/
//! type-system/inference.md`), flow analysis (`flow-analysis.md`) and, where
//! the specification is silent, the official implementation
//! (`pkg/analyzer/lib/src/dart/resolver`,
//! `_fe_analyzer_shared/lib/src/type_inference`).
//!
//! Organization:
//!
//! * `corpo`: state of a body (locals, scopes, function context);
//! * `flow`: flow model: promotion, definite assignment, reachability;
//! * `membros`: member lookup (interface, statics, extensions);
//! * `expr`: expressions, with context (schema) downwards and type upwards;
//! * `chamadas`: invocations and type argument inference;
//! * `the_collections`: list, set and map literals;
//! * `funcoes`: bodies, function expressions and local functions;
//! * `instrucoes`: statements;
//! * `padroes`: patterns (Dart 3);
//! * `types`: type operations used by all (subtype, UP, flatten…).
//!
//! Top-level inference (untyped variables and fields) is on demand
//! (`BodyInferrer::variable_type`): whoever reads the variable triggers the
//! inference of the initializer, with cycle detection (`inference.md`,
//! "Top-level inference procedure").

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

/// State of the top-level inference of a variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VarState {
    Pending,
    InProgress,
    Ready,
}

/// Frequently used symbols, resolved once.
pub(crate) struct Symbols {
    pub empty: Option<SymbolId>,
    pub call: Option<SymbolId>,
    pub new_: Option<SymbolId>,
    pub index: Option<SymbolId>,
    pub set_index: Option<SymbolId>,
    pub equal: Option<SymbolId>,
    pub unary_minus: Option<SymbolId>,
    pub til: Option<SymbolId>,
    pub remainder: Option<SymbolId>,
    pub clamp: Option<SymbolId>,
    pub this_: Option<SymbolId>,
}

impl Symbols {
    fn new(i: &Interner) -> Self {
        Self {
            empty: i.lookup(""),
            call: i.lookup("call"),
            new_: i.lookup("new"),
            index: i.lookup("[]"),
            set_index: i.lookup("[]="),
            equal: i.lookup("=="),
            unary_minus: i.lookup("unary-"),
            til: i.lookup("~"),
            remainder: i.lookup("remainder"),
            clamp: i.lookup("clamp"),
            this_: i.lookup("this"),
        }
    }
}

/// A body that can be inferred in isolation (see
/// [`BodyInferrer::only_bodies`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodyRef {
    /// Top-level function, method, constructor, getter/setter or local function
    /// declared as an element.
    Funcao(FunctionElementId),
    /// The initializer of a top-level variable or of a field.
    Variable(VariableId),
}

/// The body that contains offset `pos` of unit `u`: the executable or
/// variable initializer whose node encloses it (the smallest).
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
            best = Some((span.end - span.start, BodyRef::Variable(VariableId(i as u32))));
        }
    }
    best.map(|(_, c)| c)
}

/// The bodies of unit `u`, in the order of full inference (variables, then
/// functions). With `all = false`, only the initializers of variables
/// without a written type that the sketch already inferred: what the
/// inference of a library whose bodies were not requested writes into this
/// unit's tables (on-demand top-level inference).
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
            v.push(BodyRef::Variable(VariableId(i as u32)));
        }
    }
    if !all {
        return v;
    }
    for (i, f) in program.functions.iter().enumerate() {
        let its = match f.node {
            FunctionRef::Function { unit, .. } | FunctionRef::Constructor { unit, .. } => unit == u,
            // Synthetic ones (implicit constructor, field accessors): through the
            // class declaration or through the originating variable.
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

/// The result of [`inferir_corpos`].
pub struct InferredBodies {
    /// The unit's tables, filled by the requested bodies.
    pub tabelas: UnitBodyTypes,
    /// The diagnostics of those bodies.
    pub diagnostics: Vec<Diagnostic>,
    /// The scope captured by the probe, when requested and reached.
    pub escopo: Option<crate::resolved::EscopoSondado>,
}

/// Infers only `bodies` (all from unit `unit_id`) over a sketch and a type
/// table already populated by an earlier inference of the same program
/// (docs/LSP-ESPECIFICACAO.md §16.5, E1): the already inferred variable
/// types hold, field overrides were already completed (the sketch's list is
/// empty) and override inference already ran. With `metadata`, the unit's
/// annotations too (recreating the tables of a whole unit; completion does
/// not request them).
///
/// Invariant: for every body `c`, the tables and diagnostics of `c` are
/// those that full inference of the program gives `c`.
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
            BodyRef::Variable(v) => inf.reinfer_variable(v),
            BodyRef::Funcao(f) => functions::infer_declared_function(&mut inf, f),
        }
    }
    if metadata {
        inf.current_unit = Some(unit_id);
        functions::infer_unit_metadata(&mut inf, unit_id);
    }
    InferredBodies {
        tabelas: std::mem::take(&mut inf.body_types.units[unit_id.0 as usize]),
        diagnostics: std::mem::take(&mut inf.diagnostics),
        escopo: inf.probed_scope.take(),
    }
}

/// Body inference context for the whole program.
pub struct BodyInferrer<'a> {
    pub program: &'a Program,
    pub interner: &'a Interner,
    pub table: &'a mut TypeTable,
    pub core: &'a CoreTypes,
    pub outline: &'a mut crate::resolve::OutlineTypes,
    pub diagnostics: Vec<Diagnostic>,
    pub body_types: BodyTypes,
    /// Resident session: when `Some`, only the function bodies of these
    /// libraries are inferred; the others will not be re-emitted, and the
    /// emitter only consults the bodies of what it emits. Variable initializers
    /// are still inferred in all libraries, because the inferred type of a
    /// variable (`var x = 1;`) is read by whoever uses it.
    pub apenas_bibliotecas: Option<HashSet<u32>>,
    /// Incremental inference (LSP completion): when `Some`, only these bodies
    /// (functions/methods/constructors and variable initializers) are inferred;
    /// the types of top-level variables they read are still on demand. Unit
    /// metadata is not visited.
    pub only_bodies: Option<HashSet<BodyRef>>,
    vars_state: Vec<VarState>,
    /// Initializers already visited for the side tables.
    visited_initializer: Vec<bool>,
    extensions: HashMap<u32, Rc<[ExtensionId]>>,
    pub(crate) sym: Symbols,
    /// Depth of nested top-level inferences (stack protection).
    top_depth: u32,
    /// Auxiliary parameters (E, K, V) for collection literal inference.
    pub(crate) collection_params_cache: Option<[crate::table::TypeParamId; 3]>,
    /// New parameters, per class, for constructor inference.
    pub(crate) constructor_params: HashMap<u32, Vec<crate::table::TypeParamId>>,
    /// The `FieldPromotability` of each library already consulted.
    pub(crate) promotion_by_library: HashMap<LibraryId, std::rc::Rc<crate::field_promotion::LibraryPromotion>>,
    /// The `whyNotPromoted` of each reference read (local or property) that has
    /// some reason: `ty -> motivo`, in the order of the flow's map.
    pub(crate) non_promotions: HashMap<(UnitId, ast::ExprId), Vec<(TypeId, flow::NonPromotionReason)>>,
    /// Refined context of the arguments of `clamp`/`remainder` for the next
    /// invocation.
    pub(crate) pending_numeric_context: Option<TypeId>,
    /// The `errorEntity` of the next generic invocation (where the
    /// `COULD_NOT_INFER` goes) and the names of the target's positional
    /// parameters (for the message); only whoever knows the target sets them.
    pub(crate) inference_entity: Option<Span>,
    /// Parameters whose written type does not resolve (`InvalidType` in the
    /// analyzer), by unit and name offset: reading them has the recovery type.
    pub(crate) invalid_locals: HashSet<(UnitId, usize)>,
    pub(crate) positional_names: Option<Vec<String>>,
    /// Target of the next arity check (`calls::verify_arity`): the name
    /// cited in `NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_*` and the range of
    /// `MISSING_REQUIRED_ARGUMENT`; only whoever knows the call sets it.
    pub(crate) arity_target: Option<calls::ArityTarget>,
    /// The next [`calls::invoke`] is that of an implicit `call`: the invoked
    /// function has an interface type, and `_checkInvocationTypeArguments`
    /// (`type_arguments_verifier.dart`) does not check the bounds of the written
    /// type arguments (the generic type is not a `FunctionType`).
    pub(crate) implicit_call: bool,
    /// Unit of each diagnostic (parallel to `diagnostics`), for whoever needs
    /// the file (tools; the LSP).
    pub warning_units: Vec<Option<UnitId>>,
    /// Unit of the body being inferred.
    pub(crate) current_unit: Option<UnitId>,
    /// Types of the spreads of a spreads-only `{…}`, already inferred without a
    /// context to decide between set and map (the visit of the elements reuses
    /// them instead of inferring again).
    pub(crate) inferred_spreads: HashMap<dartforge_frontend::ast::ExprId, TypeId>,
    /// The type of each spread after the literal's visit, for the shape from the
    /// elements (`collections::literal`).
    pub(crate) visited_spreads: HashMap<dartforge_frontend::ast::ExprId, TypeId>,
    /// LSP: records in [`UnitBodyTypes::declaracoes_de_locais`] the declaration
    /// of each referenced local (rename, references). Off in the compiler, which
    /// does not pay for the table.
    pub registrar_locais: bool,
    /// LSP (completion): the simple identifier at `(unit_id, offset do name)`
    /// has its lexical scope captured in [`BodyInferrer::probed_scope`] when
    /// inference goes through it.
    pub sonda_escopo: Option<(UnitId, usize)>,
    /// The scope captured by [`BodyInferrer::sonda_escopo`].
    pub probed_scope: Option<crate::resolved::EscopoSondado>,
    /// The context of the next resolved annotation (read and cleared by
    /// `resolve_annotation`).
    pub(crate) type_context: crate::resolve::TypeContext,
    /// The body's locals visible in the annotation being resolved (see
    /// [`Body::visible_locals`](body::Body::visible_locals)).
    pub(crate) locals_as_type: HashMap<SymbolId, (Span, bool)>,
    /// The class and the extension whose members are in scope in the
    /// annotation.
    pub(crate) type_container: (Option<dartforge_elements::model::ClassId>, Option<ExtensionId>),
    /// Extension ambiguity of the last member lookup: `(name, list)`.
    pub(crate) extension_ambiguity: Option<(String, String, Option<(String, String)>)>,
    /// The annotation being resolved is in a static method or field.
    pub(crate) in_static_member: bool,
    /// The `InheritanceManager3` (the classes' interfaces), with the types of
    /// the fields inferred on demand.
    pub(crate) inheritance: crate::heranca::Heranca,
    /// The parents of the expressions of each unit, on demand (the rules that
    /// depend on a node's syntactic place).
    pais: HashMap<UnitId, Rc<dartforge_frontend::parents::Parents>>,
    /// `_inferring` of top-level inference: the variables whose initializer is
    /// being inferred, in order.
    variable_stack: Vec<VariableId>,
    /// The variables in an inference cycle (`dependencyCycle`): the type is
    /// `dynamic`, and the report was already made.
    in_cycle: HashSet<VariableId>,
    /// When looking up what an untyped field overrides, its class: the fields
    /// declared in it are not typed when building the interface
    /// ([`crate::heranca::Provedor::tipo_do_membro`]).
    pub(crate) declared_untyped: Option<dartforge_elements::model::ClassId>,
    /// Some interface in the cache was built with an untyped field.
    pub(crate) provisional_inheritance: bool,
    /// The name (without the `=`) looked up in the interface during a top-level
    /// inference: fields not yet inferred with another name stay provisional
    /// ([`crate::heranca::Provedor::tipo_do_membro`]).
    pub(crate) lookup_name: Option<SymbolId>,
}

/// The extensions accessible in `lib` (the library's own and those the
/// imports bring in, `BodyInferrer::accessible_extensions`) that apply to
/// `recv`, with the inferred type arguments (the analyzer's `applicableTo`,
/// used by completion: `InstanceExtensionMembersOperation` and the extension
/// members of `DeclarationHelper`). The receiver is taken as is: the caller
/// already promoted it to non-nullable (`promoteToNonNull`).
pub fn extensoes_aplicaveis(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &mut crate::resolve::OutlineTypes,
    lib: LibraryId,
    recv: TypeId,
) -> Vec<(ExtensionId, Vec<TypeId>)> {
    // Without body tables: only applicability is consulted.
    let mut inf = BodyInferrer::with_tables(program, interner, table, core, outline, Some(UnitId(u32::MAX)));
    let exts = inf.accessible_extensions(lib);
    exts.iter().filter_map(|&e| inf.applicable_extension(e, recv).map(|a| (e, a))).collect()
}

/// Like [`extensoes_aplicaveis`], for a given list of extensions (those of
/// the `exportNamespace` of a library not yet imported,
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
    candidates.iter().filter_map(|&e| inf.applicable_extension(e, recv).map(|a| (e, a))).collect()
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

    /// Like [`BodyInferrer::new`]; with `only = Some(u)`, only unit `u` has tables
    /// (the inference of isolated bodies, [`inferir_corpos`]).
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
            // SDK units do not get body inference: an empty table
            // (`get_type`/`get_resolved` return `None`, `set_*` ignore).
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
            vars_state: vec![VarState::Pending; nvars],
            visited_initializer: vec![false; nvars],
            extensions: HashMap::new(),
            sym: Symbols::new(interner),
            top_depth: 0,
            collection_params_cache: None,
            constructor_params: HashMap::new(),
            promotion_by_library: HashMap::new(),
            non_promotions: HashMap::new(),
            pending_numeric_context: None,
            inference_entity: None,
            invalid_locals: HashSet::new(),
            positional_names: None,
            arity_target: None,
            implicit_call: false,
            warning_units: Vec::new(),
            current_unit: None,
            inheritance: crate::heranca::Heranca::default(),
            declared_untyped: None,
            provisional_inheritance: false,
            lookup_name: None,
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

    /// Gives body tables to the SDK units of the libraries `libs` (which `new`
    /// leaves empty): the LSP's reference search infers the SDK bodies that
    /// cite the searched name (docs/LSP-ESPECIFICACAO.md §11.9 B).
    pub fn alocar_corpos_de(&mut self, libs: &[LibraryId]) {
        for (i, u) in self.program.units.iter().enumerate() {
            if libs.contains(&u.library) && self.program.library(u.library).is_sdk && self.body_types.units[i].static_types.is_empty() {
                self.body_types.units[i] = UnitBodyTypes::new(u.ast.exprs.len(), self.core.dynamic_);
            }
        }
    }

    /// The variable types already inferred in the sketch hold (a sketch retained
    /// from an earlier inference of the same program, docs/LSP-ESPECIFICACAO.md
    /// §16.5): their state starts ready, and only the requested ones are
    /// reinferred.
    fn use_retained_sketch(&mut self) {
        for (i, d) in self.outline.variables.iter().enumerate() {
            if d.declared_type.is_none() && d.inferred.is_some() {
                self.vars_state[i] = VarState::Ready;
            }
        }
    }

    /// The initializer of `vid` again, for the unit's tables: with a written
    /// type, the visit; without, the top-level inference, keeping the sketch's
    /// type (an untyped variable's initializer is a signature: in a body-class
    /// swap it did not change; in completion's speculative swap, the new type is
    /// not recorded).
    fn reinfer_variable(&mut self, vid: VariableId) {
        let d = &self.outline.variables[vid.0 as usize];
        if d.declared_type.is_some() {
            self.visit_initializer(vid);
            return;
        }
        let before = d.inferred;
        self.vars_state[vid.0 as usize] = VarState::Pending;
        let t = self.variable_type(vid);
        if let Some(a) = before
            && a != t
        {
            self.outline.variables[vid.0 as usize].inferred = Some(a);
            self.sync_accessors(vid, a);
        }
    }

    /// Entry point: infers variable initializers and bodies.
    pub fn infer_all(self) -> (BodyTypes, Vec<Diagnostic>) {
        let (b, d, _) = self.infer_all_with_units();
        (b, d)
    }

    /// Like [`BodyInferrer::infer_all`], with the unit of each diagnostic.
    pub fn infer_all_with_units(mut self) -> (BodyTypes, Vec<Diagnostic>, Vec<Option<UnitId>>) {
        self.infer_everything();
        (self.body_types, self.diagnostics, self.warning_units)
    }

    /// Like [`BodyInferrer::infer_all`], also returning the scope captured by
    /// [`BodyInferrer::sonda_escopo`] (LSP completion).
    pub fn infer_all_com_sonda(mut self) -> (BodyTypes, Vec<Diagnostic>, Option<crate::resolved::EscopoSondado>) {
        self.infer_everything();
        (self.body_types, self.diagnostics, self.probed_scope)
    }

    /// Infers initializers, bodies and metadata, filling the tables.
    fn infer_everything(&mut self) {
        // SDK libraries requested explicitly (the native backend compiles the
        // SDK from source) get side tables like the user's.
        if let Some(requested) = self.apenas_bibliotecas.clone() {
            for (ui, u) in self.program.units.iter().enumerate() {
                if self.program.library(u.library).is_sdk && requested.contains(&u.library.0) {
                    self.body_types.units[ui] = UnitBodyTypes::new(u.ast.exprs.len(), self.core.dynamic_);
                }
            }
        }
        self.complete_field_overrides();
        // `_performOverrideInference`: the combined signature of the overridden
        // members in the omitted types of instance members.
        self.infer_overrides();
        if let Some(bodies) = self.only_bodies.clone() {
            for c in bodies {
                match c {
                    BodyRef::Variable(vid) => {
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
            // Resident session: variables of libraries that will not be re-emitted
            // are only inferred on demand (whoever reads them asks for the type).
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

    /// The bodies of `lib` are inferred: the requested ones, when there is a
    /// request (including the SDK's); otherwise all except the SDK's.
    pub(crate) fn infer_bodies_of(&self, lib: LibraryId) -> bool {
        match &self.apenas_bibliotecas {
            Some(s) => s.contains(&lib.0),
            None => !self.program.library(lib).is_sdk,
        }
    }

    /// Type of a top-level variable or field, inferring the initializer on
    /// demand when the type was omitted.
    pub(crate) fn variable_type(&mut self, vid: VariableId) -> TypeId {
        let d = &self.outline.variables[vid.0 as usize];
        if let Some(t) = d.declared_type {
            return t;
        }
        match self.vars_state[vid.0 as usize] {
            VarState::Ready => return d.inferred.unwrap_or(self.core.dynamic_),
            // Inference cycle: `TopLevelInference` (`top_level_inference.dart:228-240`).
            VarState::InProgress => {
                self.inference_cycle(vid);
                return self.core.dynamic_;
            }
            VarState::Pending => {}
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
        // In a cycle, the type already became `dynamic` (the initializer's result
        // does not count).
        let t = if self.in_cycle.contains(&vid) { self.core.dynamic_ } else { t };
        self.current_unit = saved_unit;
        // On-demand inference of a variable whose bodies were not requested
        // (SDK, a library not re-emitted): its warnings do not belong to this
        // request.
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

    /// The cycle found when the type of `vid` is requested again: the variables
    /// on the stack from it onward, still being inferred, become `dynamic` and
    /// report `TOP_LEVEL_CYCLE` (`ResolverVisitor._checkTopLevelCycle`,
    /// `resolver.dart:4009-4030`, only the non-`const` ones) at the name, with
    /// the names in the cycle in order.
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
            self.warning_with_code(dartforge_diagnostics::codes::compile_time_error::TOP_LEVEL_CYCLE, span, &[&name, &list]);
        }
        self.current_unit = saved_unit;
    }

    /// Override inference over a field without a written type: the overriding
    /// getter (return) or setter (parameter) that omitted the type inherits the
    /// inferred type of the overridden field, instantiated in the overrider's
    /// class.
    fn complete_field_overrides(&mut self) {
        let pendentes = std::mem::take(&mut self.outline.field_overrides);
        for p in &pendentes {
            let t = self.variable_type(p.field);
            let subst: HashMap<_, _> = p.subst.iter().copied().collect();
            let t = crate::ops::substitute(t, &subst, self.table);
            let fd = &mut self.outline.functions[p.function.0 as usize];
            match p.parameter {
                None => fd.return_type = t,
                Some(i) => match fd.parameters.get_mut(i) {
                    Some(par) => par.ty = t,
                    None => continue,
                },
            }
            let fd = &self.outline.functions[p.function.0 as usize];
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
            self.outline.functions[p.function.0 as usize].signature = sig;
        }
        self.outline.field_overrides = pendentes;
    }

    /// The variable's implicit accessors get the inferred type.
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

    /// Visits (once) the initializer of a variable with a written type, to fill
    /// the side tables of its expressions.
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
        if let Some((unit, init)) = self.initializer(vid) {
            let mut cx = Body::to_variable(self, vid, unit);
            let t = expr::infer(self, &mut cx, init, declared);
            expr::verify_assignable_expr(self, &cx, init, t, declared, crate::codes::INVALID_ASSIGNMENT.template);
        }
    }

    /// The non-`late` field initializers of a class with a primary constructor
    /// (Dart 3.13) are evaluated in the scope of its parameters (spec, "primary
    /// initializer scope"): a parameter shadows whatever is outside the class.
    /// The default values were already inferred in the constructor itself.
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
            let id = expr::declare_local(
                self,
                cx,
                body::Local { name: n.sym, ty, final_: p.final_, late: false, const_: false, offset: n.span.start, local_function: false },
                true,
            );
            cx.primary_parameters.insert(id);
        }
    }

    /// `(unit_id, expressão)` of a variable's initializer, if any.
    pub(crate) fn initializer(&self, vid: VariableId) -> Option<(UnitId, ast::ExprId)> {
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

    /// Extensions accessible in a library: those declared in it (including
    /// unnamed ones) and those that a non-deferred import brings into its
    /// namespace (with or without a prefix, after `show`/`hide`). As in the
    /// extensions specification, the name does not need to be visible in scope:
    /// two imported `IterableExtension`s (`package:collection`'s and the
    /// analyzer's own) conflict as names, but both stay implicitly applicable.
    pub(crate) fn accessible_extensions(&mut self, lib: LibraryId) -> Rc<[ExtensionId]> {
        if let Some(e) = self.extensions.get(&lib.0) {
            return e.clone();
        }
        let l = self.program.library(lib);
        let mut v: Vec<ExtensionId> = Vec::new();
        fn the_push(e: ExtensionId, v: &mut Vec<ExtensionId>) {
            if !v.contains(&e) {
                v.push(e);
            }
        }
        for (i, e) in self.program.extensions.iter().enumerate() {
            if e.library == lib {
                the_push(ExtensionId(i as u32), &mut v);
            }
        }
        // The scope also brings the implicit `dart:core` (which is not a
        // directive).
        let mut names: Vec<(&SymbolId, &dartforge_elements::model::Binding)> = l.scope.iter().collect();
        names.sort_by_key(|(s, _)| s.as_u32());
        for (_, b) in names {
            if let Some(dartforge_elements::model::Element::Extension(e)) = b.getter {
                the_push(e, &mut v);
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
                    the_push(e, &mut v);
                }
            }
        }
        let rc: Rc<[ExtensionId]> = v.into();
        self.extensions.insert(lib.0, rc.clone());
        rc
    }

    // ---------------------------------------------------------------
    // Diagnostics
    // ---------------------------------------------------------------

    /// The syntactic parent of expression `e` of unit `u`.
    pub(crate) fn parent_of(&mut self, u: UnitId, e: ast::ExprId) -> dartforge_frontend::parents::Parent {
        let program = self.program;
        let pais = self.pais.entry(u).or_insert_with(|| Rc::new(crate::lints_tipados::unit_parents(program, u))).clone();
        pais.parent(e)
    }

    pub(crate) fn warning(&mut self, msg: String, span: Span) {
        self.diagnostics.push(Diagnostic::new(msg, span));
        self.warning_units.push(self.current_unit);
    }

    /// Use without a null check of a potentially nullable receiver (`code`,
    /// one of the `UNCHECKED_…_OF_NULLABLE_VALUE`); with a receiver of type
    /// `Null`, it is `INVALID_USE_OF_NULL_VALUE` (without arguments), at the
    /// same place.
    pub(crate) fn null_warning(&mut self, recv: TypeId, code: dartforge_diagnostics::Code, span: Span, args: &[&str]) {
        // `Never?` is not `Null` for the analyzer (it is `NeverType` with `?`).
        let nullable_never = matches!(self.table.exibicao(recv), Some(crate::table::Exibicao::NeverAnulavel));
        if matches!(self.table.get(recv), crate::table::Type::Null) && !nullable_never {
            self.warning_with_code(dartforge_diagnostics::codes::compile_time_error::INVALID_USE_OF_NULL_VALUE, span, &[]);
        } else {
            self.warning_with_code(code, span, args);
        }
    }

    /// A warning already with the analyzer's code, the official message and the
    /// arguments (the T1 design): the parity bridge leaves it as is.
    pub(crate) fn warning_with_code(&mut self, code: dartforge_diagnostics::Code, span: Span, args: &[&str]) {
        self.diagnostics.push(Diagnostic::with_code(code, span, args.iter().copied()));
        self.warning_units.push(self.current_unit);
    }

    /// A warning with a code and context messages (`DiagnosticFactory`):
    /// `(file, intervalo, text)`, the file when it is not the warning's.
    pub(crate) fn warning_with_context(&mut self, code: dartforge_diagnostics::Code, span: Span, args: &[&str], context: Vec<(Option<String>, Span, String)>) {
        let mut d = Diagnostic::with_code(code, span, args.iter().copied());
        d.context.extend(context.into_iter().map(|(file, span, message)| dartforge_diagnostics::ContextMessage { file: file.map(Into::into), span, message: message.into() }));
        self.diagnostics.push(d);
        self.warning_units.push(self.current_unit);
    }

    /// A warning with a code whose arguments go together through the
    /// `ErrorReporter`'s conversion ([`crate::exibicao::Exibidor::argumentos`],
    /// T7): the types come out with aliases, and two types of the same report
    /// with the same text get the `(where X is defined in …)`.
    pub(crate) fn warning_with_args(&mut self, code: dartforge_diagnostics::Code, span: Span, args: &[crate::exibicao::Arg<'_>]) {
        let (texts, context) = {
            let displayer = crate::exibicao::Exibidor { table: &*self.table, interner: self.interner, program: self.program };
            displayer.arguments_and_context(args)
        };
        let mut d = Diagnostic::with_code(code, span, texts);
        d.context.extend(context);
        self.diagnostics.push(d);
        self.warning_units.push(self.current_unit);
    }

    /// Records a language error (`codes::ERRO_DE_LINGUAGEM`,
    /// docs/VERSOES-LINGUAGEM.md §3): aborts compilation, with the file and the
    /// offset in the message, like load errors.
    pub(crate) fn language_error(&mut self, unit: UnitId, span: Span, msg: String) {
        let file = self.program.unit(unit).path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        let text = format!("{}{file}:{}: {msg}", crate::codes::ERRO_DE_LINGUAGEM, span.start);
        self.diagnostics.push(Diagnostic::new(text, span));
        self.warning_units.push(Some(unit));
    }

    pub(crate) fn span_expr(&self, unit: UnitId, e: ast::ExprId) -> Span {
        self.program.unit(unit).ast.expr(e).span
    }

    /// Assignability diagnostic: `dynamic` is assignable to everything (implicit
    /// cast); an object with `call` to a function type too.
    pub(crate) fn verify_assignable(&mut self, of: TypeId, for_: TypeId, span: Span, template: &str) {
        if self.assignable(of, for_) {
            return;
        }
        // Codes with types in their arguments go through the `ErrorReporter`'s
        // conversion (aliases and the `(where C is defined in …)`
        // disambiguation of two types with the same name).
        use crate::exibicao::Arg;
        use dartforge_diagnostics::codes::compile_time_error as c;
        if template == crate::codes::INVALID_ASSIGNMENT.template {
            self.warning_with_args(c::INVALID_ASSIGNMENT, span, &[Arg::Type_(of), Arg::Type_(for_)]);
            return;
        }
        if template == crate::codes::ARGUMENT_TYPE_NOT_ASSIGNABLE.template {
            self.warning_with_args(c::ARGUMENT_TYPE_NOT_ASSIGNABLE, span, &[Arg::Type_(of), Arg::Type_(for_), Arg::Text("".into())]);
            return;
        }
        let msg = format!(
            "{}: '{}' is not assignable to '{}'",
            template,
            self.table.format(of, self.interner, self.program),
            self.table.format(for_, self.interner, self.program)
        );
        self.warning(msg, span);
    }
}
