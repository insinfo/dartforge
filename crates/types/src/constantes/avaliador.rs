//! O `ConstantVisitor` e o `_InstanceCreationEvaluator` do analyzer 6.11
//! (`src/dart/constant/evaluation.dart`): avaliação de expressões
//! constantes, com os mesmos erros (`InvalidConstant`), as mesmas posições
//! e as mesmas marcas (`avoidReporting`, `isUnresolved`,
//! `isRuntimeException`).
//!
//! **Pelo lado seguro.** O que depende de informação que a inferência desta
//! passada não produziu (corpos e inicializadores de bibliotecas que não
//! foram inferidas, como as do SDK) vira valor desconhecido válido
//! (`validWithUnknownValue`): nenhum erro é provado sobre ele.

use super::valor::{texto, Campo, Estado, Excecao, Funcao, Valor};
use crate::resolve::OutlineTypes;
use crate::resolved::{BodyTypes, MemberRef, Resolved};
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeTable};
use dartforge_diagnostics::{codigos::compile_time_error as c, Codigo, Span};
use dartforge_elements::model::{
    ClassId, ClassKind, Element, FunctionElementId, FunctionKind, FunctionRef, LibraryId, Program, UnitId, VariableId,
    VariableRef,
};
use dartforge_frontend::ast::{self, BinaryOp, CollectionElement, ExprId, ExprKind, UnaryOp};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// `InvalidConstant`.
#[derive(Clone, Debug)]
pub struct Invalid {
    pub codigo: Codigo,
    pub unidade: UnitId,
    pub span: Span,
    pub args: Vec<String>,
    pub avoid_report: bool,
    pub unresolved: bool,
    pub excecao: bool,
    /// `contextMessages`: unidade, intervalo e texto (a unidade decide o
    /// arquivo da mensagem quando o erro é relatado).
    pub contexto: Vec<(UnitId, Span, String)>,
}

/// `Constant`: um valor válido ou um erro.
#[derive(Clone, Debug)]
pub enum Constante {
    Valor(Valor),
    Invalid(Box<Invalid>),
}

impl Constante {
    pub fn valor(&self) -> Option<&Valor> {
        match self {
            Constante::Valor(v) => Some(v),
            Constante::Invalid(_) => None,
        }
    }
}

/// O estado do `ConstantVisitor`: a unidade da expressão, o ambiente léxico
/// dos parâmetros (`_lexicalEnvironment`) e o dos parâmetros de tipo
/// (`_lexicalTypeEnvironment`, que também faz a substituição da classe).
#[derive(Clone)]
pub struct Ctx {
    pub unidade: UnitId,
    pub lib: LibraryId,
    pub lexical: Option<Rc<HashMap<SymbolId, Valor>>>,
    pub tipos: Option<Rc<HashMap<TypeParamId, TypeId>>>,
    /// O relator do visitante é o do `ConstantVerifier` (e não um que
    /// descarta): os erros de `_valueOf` saem.
    pub relatar: bool,
}

impl Ctx {
    pub fn simples(unit_id: UnitId, lib: LibraryId) -> Ctx {
        Ctx { unidade: unit_id, lib, lexical: None, tipos: None, relatar: false }
    }
}

#[derive(Clone)]
enum VarState {
    InProgress,
    Ready(Constante),
}

/// O motor de avaliação (`ConstantEvaluationEngine` com os resultados
/// guardados nos elementos).
pub struct Motor<'a> {
    pub program: &'a Program,
    pub interner: &'a Interner,
    pub table: &'a mut TypeTable,
    pub core: &'a CoreTypes,
    pub outline: &'a OutlineTypes,
    pub body: &'a BodyTypes,
    /// Bibliotecas cujos corpos (e inicializadores) foram inferidos.
    pub the_inferred: &'a HashSet<LibraryId>,
    vars: HashMap<VariableId, VarState>,
    vars_stack: Vec<VariableId>,
    cyclic: HashSet<VariableId>,
    /// Valores padrão já avaliados, pela expressão.
    padroes: HashMap<(UnitId, ExprId), Constante>,
    /// Constantes locais: pelo offset do nome, a lista e o índice.
    unit_locals: HashMap<UnitId, HashMap<usize, (ast::StmtId, usize)>>,
    locais: HashMap<(UnitId, usize), Option<Constante>>,
    /// Construtores em avaliação (ciclo: `isCycleFree` falso).
    constructors_in_progress: Vec<FunctionElementId>,
    /// O grafo de dependências de constantes e quem está em ciclo
    /// ([`super::ciclos::computar`], antes do verificador de cada biblioteca).
    pub grafo: super::cycles::Estado,
    /// Argumentos posicionais diretos de uma criação constante: o
    /// `genericError` deles é `CONST_WITH_NON_CONSTANT_ARGUMENT`.
    creation_args: HashSet<(UnitId, ExprId)>,
    /// Erros relatados pelo próprio visitante (`_valueOf`) quando `relatar`.
    pub relatos: Vec<Invalid>,
    /// O analyzer 3.6.2 lançaria aqui (a exaustividade, ver
    /// [`crate::table::TypeTable::sem_tipo_padrao`]).
    pub analyzer_break: bool,
    /// Quando `Some`, o verificador guarda aqui, por (unidade, offset do
    /// `switch`), as testemunhas com partes de cada `non_exhaustive_switch_*`
    /// (o `diagnostic.data` do analyzer, usado pelo `AddMissingSwitchCases`).
    pub witnesses: Option<HashMap<(UnitId, usize), Vec<Vec<super::exhaustiveness::ParteDeTestemunha>>>>,
    /// Offsets dos nomes de parâmetros formais, por unidade inferida (a
    /// inferência resolve a leitura de um parâmetro como a de um local).
    parametros: HashMap<UnitId, HashSet<usize>>,
    depth: u32,
    sym_identical: Option<SymbolId>,
    sym_length: Option<SymbolId>,
    sym_from_environment: Option<SymbolId>,
    sym_has_environment: Option<SymbolId>,
    sym_default_value: Option<SymbolId>,
}

type R = Constante;

impl<'a> Motor<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn novo(
        program: &'a Program,
        interner: &'a Interner,
        table: &'a mut TypeTable,
        core: &'a CoreTypes,
        outline: &'a OutlineTypes,
        body: &'a BodyTypes,
        the_inferred: &'a HashSet<LibraryId>,
    ) -> Self {
        let mut params_list: HashMap<UnitId, HashSet<usize>> = HashMap::new();
        for (i, u) in program.units.iter().enumerate() {
            if the_inferred.contains(&u.library) {
                params_list.insert(UnitId(i as u32), parameter_offsets(&u.ast));
            }
        }
        Motor {
            program,
            interner,
            table,
            core,
            outline,
            body,
            the_inferred,
            vars: HashMap::new(),
            vars_stack: Vec::new(),
            cyclic: HashSet::new(),
            padroes: HashMap::new(),
            unit_locals: HashMap::new(),
            locais: HashMap::new(),
            constructors_in_progress: Vec::new(),
            grafo: super::cycles::Estado::default(),
            creation_args: HashSet::new(),
            relatos: Vec::new(),
            analyzer_break: false,
            witnesses: None,
            parametros: params_list,
            depth: 0,
            sym_identical: interner.lookup("identical"),
            sym_length: interner.lookup("length"),
            sym_from_environment: interner.lookup("fromEnvironment"),
            sym_has_environment: interner.lookup("hasEnvironment"),
            sym_default_value: interner.lookup("defaultValue"),
        }
    }

    // -- Acesso --------------------------------------------------------------

    pub fn ast(&self, u: UnitId) -> &'a ast::Ast {
        &self.program.unit(u).ast
    }

    pub fn span(&self, u: UnitId, e: ExprId) -> Span {
        self.ast(u).expr(e).span
    }

    fn inferred_(&self, u: UnitId) -> bool {
        self.the_inferred.contains(&self.program.unit(u).library)
    }

    /// Tipo estático de `e` (`dynamic` numa unidade não inferida).
    pub fn estatico(&self, u: UnitId, e: ExprId) -> TypeId {
        self.body.units.get(u.0 as usize).and_then(|b| b.get_type(e)).unwrap_or(self.core.dynamic_)
    }

    pub fn resolvido(&self, u: UnitId, e: ExprId) -> Option<&'a Resolved> {
        self.body.units.get(u.0 as usize).and_then(|b| b.get_resolved(e))
    }

    /// O tipo estático de `e` é o de recuperação (`InvalidType`): o tipo
    /// registrado inválido, ou o local de inicializador inválido.
    pub(crate) fn invalid_type(&self, u: UnitId, e: ExprId) -> bool {
        self.body.units.get(u.0 as usize).is_some_and(|b| {
            b.invalid_types.contains(&e) || b.get_type(e).is_some_and(|t| self.table.e_invalido(t))
        })
    }

    pub fn sub(&mut self, a: TypeId, b: TypeId) -> bool {
        let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
        is_subtype(a, b, &mut env)
    }

    pub fn formatar(&self, t: TypeId) -> String {
        self.table.format(t, self.interner, self.program)
    }

    /// Sem o alias (`getDisplayString()`): onde o analyzer já passa texto.
    pub fn format_without_alias(&self, t: TypeId) -> String {
        self.table.format_sem_alias(t, self.interner, self.program)
    }

    /// `extensionTypeErasure`.
    pub fn apagar(&mut self, t: TypeId) -> TypeId {
        let outline = self.outline;
        let program = self.program;
        let rep = |decl: ClassId, args: &[TypeId], table: &mut TypeTable| -> Option<TypeId> {
            let v = program.class(decl).representation?;
            let t = outline.variables[v.0 as usize].declared_type?;
            let params = &outline.classes[decl.0 as usize].type_params;
            if params.is_empty() || params.len() != args.len() {
                return Some(t);
            }
            let map: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
            Some(crate::ops::substitute(t, &map, table))
        };
        crate::ops::erase_extension_type(t, self.table, &rep)
    }

    /// `runtimeTypeMatch`: o tipo do objeto é subtipo do tipo apagado. Um
    /// objeto de tipo `dynamic` (expressão não resolvida) casa com tudo.
    pub fn casa(&mut self, v: &Valor, t: TypeId) -> bool {
        if v.tipo == self.core.dynamic_ || matches!(v.estado, Estado::Null { invalid: true }) {
            return true;
        }
        let t = self.apagar(t);
        self.sub(v.tipo, t)
    }

    /// `isAssignableTo` do tipo estático.
    pub fn atribuivel(&mut self, a: TypeId, b: TypeId) -> bool {
        a == self.core.dynamic_ || self.sub(a, b)
    }

    fn valor(&mut self, typ: TypeId, state: Estado) -> Valor {
        let typ = self.apagar(typ);
        Valor::novo(typ, state)
    }

    pub fn desconhecido(&mut self, typ: TypeId) -> Valor {
        let typ = self.apagar(typ);
        let element = match self.table.get(typ) {
            Type::Interface { class, args, .. } if Some(*class) == self.core.list_class && args.len() == 1 => Some(args[0]),
            _ => None,
        };
        Valor::desconhecido(self.table, self.core, typ, element)
    }

    fn interface(&mut self, class: Option<ClassId>, args: Vec<TypeId>) -> TypeId {
        match class {
            Some(class) => self.table.intern(Type::Interface { class, args: args.into_boxed_slice(), nullable: false }),
            None => self.core.dynamic_,
        }
    }

    // -- Erros ---------------------------------------------------------------

    pub fn erro(&self, u: UnitId, span: Span, code: Codigo) -> Invalid {
        Invalid { codigo: code, unidade: u, span, args: Vec::new(), avoid_report: false, unresolved: false, excecao: false, contexto: Vec::new() }
    }

    /// `ConstructorElement.displayName`: `C` ou `C.nome`.
    fn constructor_display(&self, f: FunctionElementId) -> String {
        let fe = self.program.function(f);
        let cls = fe.class.map(|k| self.interner.resolve(self.program.class(k).name).to_string()).unwrap_or_default();
        let the_name = self.interner.resolve(fe.name);
        if the_name.is_empty() || the_name == "new" {
            cls
        } else {
            format!("{cls}.{the_name}")
        }
    }

    /// `_constructor.source`: a unidade da declaração do construtor (a da
    /// classe, no sintético).
    fn constructor_unit(&self, f: FunctionElementId) -> Option<UnitId> {
        let fe = self.program.function(f);
        match fe.node {
            FunctionRef::Function { unit, .. } | FunctionRef::Constructor { unit, .. } => Some(unit),
            FunctionRef::None => fe.class.and_then(|k| self.program.class(k).decl).map(|d| d.unit),
        }
    }

    /// `_checkInitializers`: o erro (que não é de execução) no inicializador
    /// passa ao nó da criação; sem contexto ainda, ganha o de onde ocorre.
    fn error_in_initializer(&self, i: &Invalid, f: Option<FunctionElementId>, where_: &str, error: ErrorAt) -> Invalid {
        let mut n = i.clone();
        if n.contexto.is_empty() {
            if let Some(f) = f {
                if let Some(u) = self.constructor_unit(f) {
                    let text = format!("The error is in the {where_} of '{}', and occurs here.", self.constructor_display(f));
                    n.contexto.push((u, i.span, text));
                }
            }
        }
        n.unidade = error.unidade;
        n.span = error.span;
        n
    }

    /// `_stackTraceContextMessage`: no nome do construtor que chama (nenhum
    /// no sintético, cujo `nameOffset` é -1).
    fn constructor_stack(&self, super_: FunctionElementId, f: FunctionElementId) -> Option<(UnitId, Span, String)> {
        if !matches!(self.program.function(f).node, FunctionRef::Constructor { .. }) {
            return None;
        }
        let (u, span) = self.program.nome_nao_sintetico_da_funcao(f)?;
        let who = self.constructor_display(f);
        let text = format!("The evaluated constructor '{}' is called by '{who}' and '{who}' is defined here.", self.constructor_display(super_));
        Some((u, span, text))
    }

    fn inv(&self, u: UnitId, e: ExprId, code: Codigo) -> R {
        Constante::Invalid(Box::new(self.erro(u, self.span(u, e), code)))
    }

    /// `InvalidConstant.genericError`.
    fn generic(&self, u: UnitId, e: ExprId, unresolved: bool) -> R {
        let code = if self.creation_args.contains(&(u, e)) { c::CONST_WITH_NON_CONSTANT_ARGUMENT } else { c::INVALID_CONSTANT };
        let mut i = self.erro(u, self.span(u, e), code);
        i.unresolved = unresolved;
        Constante::Invalid(Box::new(i))
    }

    fn excecao(&self, u: UnitId, e: ExprId, x: Excecao) -> R {
        let mut i = self.erro(u, self.span(u, e), x.codigo);
        i.excecao = x.runtime;
        Constante::Invalid(Box::new(i))
    }

    // -- Expressões ----------------------------------------------------------

    /// `evaluateConstant`: `em_const` diz se o nó está num contexto
    /// constante (`inConstantContext`).
    pub fn avaliar(&mut self, cx: &Ctx, e: ExprId, in_const: bool) -> R {
        if self.depth > 400 {
            return self.generic(cx.unidade, e, true);
        }
        self.depth += 1;
        let r = self.evaluate_at(cx, e, in_const);
        self.depth -= 1;
        self.instantiate_reference(cx, e, r)
    }

    /// `visitFunctionReference` sem argumentos escritos (3.6.2
    /// `evaluation.dart:852-882`): a instanciação implícita de qualquer
    /// expressão (`instanciacao_de_tearoff`). Argumento inferido que menciona
    /// parâmetro de tipo é `CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF`; senão
    /// `_instantiateFunctionType` (`:1939-1959`): o valor cuja função é
    /// genérica (o tipo **do elemento**) fica com esse tipo instanciado pelos
    /// argumentos e depois substituído pelo ambiente do construtor.
    fn instantiate_reference(&mut self, cx: &Ctx, e: ExprId, r: R) -> R {
        let Constante::Valor(mut val) = r else { return r };
        let u = cx.unidade;
        // O tear-off de construtor é `ConstructorReference`
        // (`visitConstructorReference`), não `FunctionReference`.
        if matches!(self.resolvido(u, e), Some(Resolved::Constructor(_))) {
            return Constante::Valor(val);
        }
        let Some(args) = self.body.units.get(u.0 as usize).and_then(|b| b.instanciacao_de_tearoff(e)).map(|a| a.to_vec()) else {
            return Constante::Valor(val);
        };
        if self.instantiation_with_parameter(cx, e) {
            return self.inv(u, e, c::CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF);
        }
        let Estado::Funcao { elemento: Funcao::Elemento(f), .. } = val.estado else { return Constante::Valor(val) };
        let Some(sig) = self.outline.functions.get(f.0 as usize).map(|d| d.signature) else { return Constante::Valor(val) };
        let Type::Function { type_params, ret, positional, optional, named, nullable } = self.table.get(sig).clone() else {
            return Constante::Valor(val);
        };
        if type_params.is_empty() || args.is_empty() || type_params.len() != args.len() {
            return Constante::Valor(val);
        }
        let without = self.table.intern(Type::Function { type_params: Box::new([]), ret, positional, optional, named, nullable });
        let m: HashMap<TypeParamId, TypeId> = type_params.iter().copied().zip(args.iter().copied()).collect();
        let t = crate::ops::substitute(without, &m, self.table);
        val.tipo = match &cx.tipos {
            Some(m) => crate::ops::substitute(t, m, self.table),
            None => t,
        };
        Constante::Valor(val)
    }

    fn evaluate_at(&mut self, cx: &Ctx, e: ExprId, in_const: bool) -> R {
        let u = cx.unidade;
        let a = self.ast(u);
        match &a.expr(e).kind {
            ExprKind::Int(span) => {
                let t = &self.program.unit(u).source[span.start..span.end];
                let v = crate::constant::integer_literal(t);
                if self.estatico(u, e) == self.core.double {
                    let d = v.map(|i| i as f64).or_else(|| t.replace('_', "").parse::<f64>().ok());
                    Constante::Valor(Valor::novo(self.core.double, Estado::Double(d)))
                } else {
                    Constante::Valor(Valor::novo(self.core.int, Estado::Int(v)))
                }
            }
            ExprKind::Double(span) => {
                let t = &self.program.unit(u).source[span.start..span.end];
                Constante::Valor(Valor::novo(self.core.double, Estado::Double(crate::constant::real_literal(t))))
            }
            ExprKind::Bool(b) => Constante::Valor(Valor::bool_(self.core, Some(*b))),
            ExprKind::Null => Constante::Valor(Valor::nulo(self.core)),
            ExprKind::String(lit) => self.string(cx, e, lit, in_const),
            ExprKind::Symbol(parts) => {
                let s: Vec<&str> = parts.iter().map(|n| self.interner.resolve(n.sym)).collect();
                Constante::Valor(Valor::novo(self.core.symbol, Estado::Simbolo(Some(s.join(".").into()))))
            }
            ExprKind::Identifier(n) => {
                if let Some(v) = cx.lexical.as_ref().and_then(|l| l.get(&n.sym)) {
                    return Constante::Valor(v.clone());
                }
                self.constant_value(cx, e, e, true)
            }
            ExprKind::Parenthesized(x) => self.avaliar(cx, *x, in_const),
            ExprKind::List { const_, elements, .. } => {
                if !(*const_ || in_const) {
                    return self.inv(u, e, c::MISSING_CONST_IN_LIST_LITERAL);
                }
                let typ = self.estatico(u, e);
                let element = match self.table.get(typ) {
                    Type::Interface { args, .. } if args.len() == 1 => args[0],
                    _ => self.core.dynamic_,
                };
                let element = self.apagar(element);
                let list_type = self.interface(self.core.list_class, vec![element]);
                let mut list = Vec::new();
                if let Some(i) = self.build_list(cx, &mut list, elements, true) {
                    return match i {
                        Ok(()) => Constante::Valor(self.desconhecido(list_type)),
                        Err(i) => Constante::Invalid(i),
                    };
                }
                Constante::Valor(self.valor(list_type, Estado::Lista { elemento: element, elementos: Rc::new(list), unknown: false }))
            }
            ExprKind::SetOrMap { const_, elements, .. } => {
                let typ = self.estatico(u, e);
                let (cls, args) = match self.table.get(typ) {
                    Type::Interface { class, args, .. } => (Some(*class), args.to_vec()),
                    _ => (None, Vec::new()),
                };
                let is_set = cls.is_some() && cls == self.core.set_class;
                let is_map = cls.is_some() && cls == self.core.map_class;
                if !is_set {
                    if !(*const_ || in_const) {
                        return self.inv(u, e, c::MISSING_CONST_IN_MAP_LITERAL);
                    }
                    let (k, v) = if args.len() >= 2 { (args[0], args[1]) } else { (self.core.dynamic_, self.core.dynamic_) };
                    let map_type = self.interface(self.core.map_class, vec![k, v]);
                    let mut map = Vec::new();
                    return match self.build_map(cx, &mut map, elements) {
                        Some(Ok(())) => Constante::Valor(self.desconhecido(map_type)),
                        Some(Err(mut i)) => {
                            if !is_map {
                                i.avoid_report = true;
                            }
                            Constante::Invalid(i)
                        }
                        None => Constante::Valor(self.valor(map_type, Estado::Mapa { entradas: Rc::new(map), desconhecido: false })),
                    };
                }
                if !(*const_ || in_const) {
                    return self.inv(u, e, c::MISSING_CONST_IN_SET_LITERAL);
                }
                let el = args.first().copied().unwrap_or(self.core.dynamic_);
                let set_type = self.interface(self.core.set_class, vec![el]);
                let mut set = Vec::new();
                match self.build_set(cx, &mut set, elements) {
                    Some(Ok(())) => Constante::Valor(self.desconhecido(set_type)),
                    Some(Err(i)) => Constante::Invalid(i),
                    None => Constante::Valor(self.valor(set_type, Estado::Conjunto { elementos: Rc::new(set), desconhecido: false })),
                }
            }
            ExprKind::Record { const_, positional, named } => {
                let const_child = *const_ || in_const;
                let mut pos = Vec::new();
                for &x in positional.iter() {
                    match self.avaliar(cx, x, const_child) {
                        Constante::Valor(v) => pos.push(v),
                        i => return i,
                    }
                }
                let mut the_named = Vec::new();
                for (n, x) in named.iter() {
                    match self.avaliar(cx, *x, const_child) {
                        Constante::Valor(v) => the_named.push((n.sym, v)),
                        i => return i,
                    }
                }
                let mut named_t: Vec<(SymbolId, TypeId)> = the_named.iter().map(|(n, v)| (*n, v.tipo)).collect();
                named_t.sort_by_key(|(n, _)| *n);
                let typ = self.table.intern(Type::Record {
                    positional: pos.iter().map(|v| v.tipo).collect(),
                    named: named_t.into_boxed_slice(),
                    nullable: false,
                });
                Constante::Valor(Valor::novo(typ, Estado::Registro { posicionais: Rc::new(pos), nomeados: Rc::new(the_named) }))
            }
            ExprKind::InstanceCreation { keyword, arguments, constructor, .. } => {
                if !matches!(keyword, Some(ast::CreationKeyword::Const)) && !(keyword.is_none() && in_const) {
                    return self.generic(u, e, false);
                }
                let Some(Resolved::Constructor(f)) = self.resolvido(u, e) else {
                    // O construtor encaminhado de uma aplicação de mixin.
                    if let Some((f, typ)) = self.forwarded_constructor(u, e, constructor.map(|n| n.sym)) {
                        let kw = self.creation_keyword(u, e, keyword.is_some());
                        let r = self.call_constructor(cx, e, f, arguments, kw, Some(typ));
                        return self.format_constructor_error(u, e, r);
                    }
                    // `const E(0)` pelo construtor primário de um tipo de
                    // extensão (sem função no modelo): a representação.
                    let static_ = self.body.units.get(u.0 as usize).and_then(|b| b.get_type(e));
                    if let Some(t) = static_
                        && let Type::ExtensionType { decl: k, .. } = self.table.get(t).clone()
                        && let Some(r) = self.extension_primary(cx, e, k, constructor.map(|n| n.sym), arguments, true)
                    {
                        return r;
                    }
                    return self.inv(u, e, c::INVALID_CONSTANT);
                };
                let kw = self.creation_keyword(u, e, keyword.is_some());
                let r = self.call_constructor(cx, e, *f, arguments, kw, None);
                self.format_constructor_error(u, e, r)
            }
            ExprKind::Call { target, arguments } => {
                if let Some(Resolved::Constructor(f)) = self.resolvido(u, e) {
                    // Criação implícita: constante só num contexto constante.
                    if !in_const {
                        return self.generic(u, e, false);
                    }
                    let f = *f;
                    let r = self.call_constructor(cx, e, f, arguments, None, None);
                    return self.format_constructor_error(u, e, r);
                }
                let tgt = *target;
                // O construtor primário de um tipo de extensão (`A(true)`,
                // `A.n(1)`), que o modelo não guarda como função: a criação
                // vale a representação (`_fieldMap[representation]`, 3.6.2
                // `evaluation.dart:2573-2578`), o argumento avaliado.
                if let Some(r) = self.creation_via_extension_primary(cx, e, tgt, arguments, in_const) {
                    return r;
                }
                // `assert(…)` como expressão é `FunctionExpressionInvocation`
                // (o `visitNode` genérico), não `MethodInvocation`.
                let is_method = matches!(a.expr(tgt).kind, ExprKind::Identifier(_) | ExprKind::Property { .. })
                    && !a.invocacoes_de_assert.contains(&e);
                if !is_method {
                    return self.generic(u, e, false);
                }
                // `identical(a, b)` de `dart:core`.
                if let ExprKind::Identifier(n) = &a.expr(tgt).kind {
                    if Some(n.sym) == self.sym_identical && arguments.args.len() == 2 && arguments.args.iter().all(|x| x.name.is_none()) {
                        if let Some(Resolved::Element(Element::Function(f))) = self.resolvido(u, tgt) {
                            if Some(self.program.function(*f).library) == self.core.core_library {
                                let (x, y) = (arguments.args[0].value, arguments.args[1].value);
                                let l = match self.avaliar(cx, x, in_const) {
                                    Constante::Valor(v) => v,
                                    i => return i,
                                };
                                let r = match self.avaliar(cx, y, in_const) {
                                    Constante::Valor(v) => v,
                                    i => return i,
                                };
                                let b = self.identical(&l, &r);
                                return Constante::Valor(Valor::bool_(self.core, b));
                            }
                        }
                    }
                }
                if self.invalid_type(u, e) {
                    let mut i = self.erro(u, self.span(u, e), c::INVALID_CONSTANT);
                    i.unresolved = true;
                    return Constante::Invalid(Box::new(i));
                }
                self.inv(u, e, c::CONST_EVAL_METHOD_INVOCATION)
            }
            ExprKind::Property { target, name, .. } => self.propriedade(cx, e, *target, *name, in_const),
            ExprKind::TypeArguments { target, .. } => {
                // Literal de tipo genérico ou instanciação de função: o
                // valor exato do tipo não é reconstruído aqui.
                match self.avaliar(cx, *target, in_const) {
                    Constante::Valor(v) => match v.estado {
                        Estado::Tipo(_) => Constante::Valor(Valor::novo(self.core.type_, Estado::Tipo(None))),
                        Estado::Funcao { elemento: element, .. } => {
                            // `_instantiateFunctionType`: o tipo instanciado
                            // passa pela substituição do construtor.
                            let t = self.estatico(u, e);
                            let t = match &cx.tipos {
                                Some(m) => crate::ops::substitute(t, m, self.table),
                                None => t,
                            };
                            Constante::Valor(self.valor(t, Estado::Funcao { elemento: element, args: None }))
                        }
                        _ => self.inv(u, e, c::INVALID_CONSTANT),
                    },
                    i => i,
                }
            }
            ExprKind::Unary { op, operand } => self.unary(cx, e, *op, *operand, in_const),
            ExprKind::Binary { op, left, right } => self.binary(cx, e, *op, *left, *right, in_const),
            ExprKind::Conditional { condition, then, else_ } => {
                let cond = match self.avaliar(cx, *condition, in_const) {
                    Constante::Valor(v) => v,
                    i => return i,
                };
                if !cond.estado.e_bool() {
                    return self.inv(u, *condition, c::CONST_EVAL_TYPE_BOOL);
                }
                match cond.as_bool() {
                    Some(true) => {
                        if let Some(i) = self.not_potentially_constant(cx, *else_) {
                            return i;
                        }
                        self.avaliar(cx, *then, in_const)
                    }
                    Some(false) => {
                        if let Some(i) = self.not_potentially_constant(cx, *then) {
                            return i;
                        }
                        self.avaliar(cx, *else_, in_const)
                    }
                    None => {
                        if let i @ Constante::Invalid(_) = self.avaliar(cx, *then, in_const) {
                            return i;
                        }
                        if let i @ Constante::Invalid(_) = self.avaliar(cx, *else_, in_const) {
                            return i;
                        }
                        let t = self.estatico(u, e);
                        Constante::Valor(self.desconhecido(t))
                    }
                }
            }
            ExprKind::Is { value, negated, .. } => {
                let v = match self.avaliar(cx, *value, in_const) {
                    Constante::Valor(v) => v,
                    i => return i,
                };
                // O tipo testado não é reconstruído aqui: o resultado é
                // desconhecido (nunca prova erro).
                let _ = (v, negated);
                Constante::Valor(Valor::bool_(self.core, None))
            }
            ExprKind::As { value, ty } => {
                let ty = *ty;
                let v = match self.avaliar(cx, *value, in_const) {
                    Constante::Valor(v) => v,
                    i => return i,
                };
                // `node.type.type` (`visitAsExpression`): o tipo escrito, não
                // o estático do `as` (que a instanciação implícita muda).
                let tgt = self
                    .body
                    .units
                    .get(u.0 as usize)
                    .and_then(|b| b.tipos_de_anotacoes.get(&ty).copied())
                    .or_else(|| self.outline.tipos_escritos.get(&(u, ty)).copied())
                    .unwrap_or_else(|| self.estatico(u, e));
                // O tipo do `as` com o ambiente léxico do construtor aplicado
                // (`_substitution`, `x as List<T>` em `C<int>`): só o que
                // ainda menciona parâmetro de tipo fica sem conferir.
                let tgt = match &cx.tipos {
                    Some(m) => crate::ops::substitute(tgt, m, self.table),
                    None => tgt,
                };
                if v.tipo == self.core.dynamic_ || tgt == self.core.dynamic_ || self.mentions_parameter(tgt) {
                    return Constante::Valor(v);
                }
                let tgt = self.apagar(tgt);
                if !self.sub(v.tipo, tgt) {
                    return self.inv(u, e, c::CONST_EVAL_THROWS_EXCEPTION);
                }
                Constante::Valor(v)
            }
            ExprKind::DotShorthand { .. } => {
                // Atalho resolvido para um membro estático ou constante de enum.
                match self.resolvido(u, e) {
                    Some(Resolved::Element(Element::Class(d))) => {
                        let d = *d;
                        let ExprKind::DotShorthand { name, .. } = &a.expr(e).kind else { unreachable!() };
                        match self.static_variable_member(d, name.sym) {
                            Some(v) => self.variable_reference_value(cx, e, v),
                            None => self.generic(u, e, false),
                        }
                    }
                    _ => self.generic(u, e, true),
                }
            }
            _ => self.generic(u, e, false),
        }
    }

    pub(crate) fn mentions_parameter(&self, t: TypeId) -> bool {
        match self.table.get(t) {
            Type::TypeParameter { .. } | Type::Intersection { .. } => true,
            Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.iter().any(|a| self.mentions_parameter(*a)),
            Type::FutureOr { arg, .. } => self.mentions_parameter(*arg),
            Type::Function { ret, positional, optional, named, .. } => {
                self.mentions_parameter(*ret)
                    || positional.iter().chain(optional.iter()).any(|a| self.mentions_parameter(*a))
                    || named.iter().any(|(_, a, _)| self.mentions_parameter(*a))
            }
            Type::Record { positional, named, .. } => {
                positional.iter().any(|a| self.mentions_parameter(*a)) || named.iter().any(|(_, a)| self.mentions_parameter(*a))
            }
            _ => false,
        }
    }

    /// O span da palavra-chave `new`/`const` de uma criação explícita.
    fn creation_keyword(&self, u: UnitId, e: ExprId, has: bool) -> Option<Span> {
        if !has {
            return None;
        }
        let s = self.span(u, e);
        let source = &self.program.unit(u).source;
        let text = source.get(s.start..s.end)?;
        let n = text.find(|c: char| !c.is_ascii_alphabetic()).unwrap_or(text.len());
        Some(Span { start: s.start, end: s.start + n })
    }

    /// `evaluateAndFormatErrorsInConstructorCall`: uma exceção de execução
    /// vira `CONST_EVAL_THROWS_EXCEPTION` na criação.
    pub fn format_constructor_error(&self, u: UnitId, e: ExprId, r: R) -> R {
        match r {
            Constante::Invalid(i) if i.excecao => {
                let mut n = self.erro(u, self.span(u, e), c::CONST_EVAL_THROWS_EXCEPTION);
                n.avoid_report = i.avoid_report;
                // O contexto da exceção, no arquivo da biblioteca
                // (`library.source`), depois dos que o erro já tinha.
                let text = dartforge_diagnostics::Diagnostic::com_codigo(i.codigo, i.span, i.args.iter().map(|s| s.as_str())).message;
                let defining = self.program.library(self.program.unit(u).library).units.first().copied().unwrap_or(u);
                n.contexto = i.contexto.clone();
                n.contexto.push((defining, i.span, format!("The exception is '{text}' and occurs here.")));
                Constante::Invalid(Box::new(n))
            }
            r => r,
        }
    }

    // -- Strings ---------------------------------------------------------------

    fn string(&mut self, cx: &Ctx, e: ExprId, lit: &ast::StringLit, in_const: bool) -> R {
        let u = cx.unidade;
        let mut acc: Vec<u16> = Vec::new();
        let mut unknown = false;
        for p in lit.parts.iter() {
            match p {
                ast::StringPart::Text(t) => acc.extend(t.code_units()),
                ast::StringPart::Interpolation(x) => {
                    let v = match self.avaliar(cx, *x, in_const) {
                        Constante::Valor(v) => v,
                        i => return i,
                    };
                    if !v.estado.is_bool_num_string_or_null() {
                        let s = self.interpolation_span(u, *x);
                        return Constante::Invalid(Box::new(self.erro(u, s, c::CONST_EVAL_TYPE_BOOL_NUM_STRING)));
                    }
                    match v.estado.para_texto(&|_| String::new(), &|_| String::new()) {
                        Ok(Estado::Str(Some(s))) => acc.extend(s.iter()),
                        Ok(_) => unknown = true,
                        Err(x) => return self.excecao(u, e, x),
                    }
                }
            }
        }
        let st = if unknown { None } else { Some(acc.into()) };
        Constante::Valor(Valor::novo(self.core.string, Estado::Str(st)))
    }

    /// O intervalo do `InterpolationExpression`: `$x` ou `${e}`.
    fn interpolation_span(&self, u: UnitId, x: ExprId) -> Span {
        let s = self.span(u, x);
        let b = self.program.unit(u).source.as_bytes();
        if s.start >= 1 && b[s.start - 1] == b'$' {
            return Span { start: s.start - 1, end: s.end };
        }
        let mut i = s.start;
        while i > 0 && b[i - 1].is_ascii_whitespace() {
            i -= 1;
        }
        if i >= 2 && &b[i - 2..i] == b"${" {
            let mut j = s.end;
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            let end = if j < b.len() && b[j] == b'}' { j + 1 } else { s.end };
            return Span { start: i - 2, end: end };
        }
        s
    }

    // -- Coleções --------------------------------------------------------------

    /// `_buildListConstant`: `None` completa; `Some(Ok(()))` desconhecida
    /// (condição de `if` desconhecida); `Some(Err)` erro.
    fn build_list(
        &mut self,
        cx: &Ctx,
        list: &mut Vec<Valor>,
        elems: &[CollectionElement],
        const_child: bool,
    ) -> Option<Result<(), Box<Invalid>>> {
        let u = cx.unidade;
        for el in elems {
            match el {
                CollectionElement::Expression(x) => match self.avaliar(cx, *x, const_child) {
                    Constante::Valor(v) => list.push(v),
                    Constante::Invalid(i) => return Some(Err(i)),
                },
                CollectionElement::For { .. } | CollectionElement::ForIn { .. } => {
                    return Some(Err(Box::new(self.erro(u, self.element_span(u, el), c::CONST_EVAL_FOR_ELEMENT))));
                }
                CollectionElement::If { condition, case_pattern, then, else_, .. } => {
                    if case_pattern.is_some() {
                        return Some(Err(Box::new(self.erro(u, self.element_span(u, el), c::INVALID_CONSTANT))));
                    }
                    let cond = match self.avaliar(cx, *condition, const_child) {
                        Constante::Valor(v) => v,
                        Constante::Invalid(i) => return Some(Err(i)),
                    };
                    if cond.actually_unknown() {
                        return Some(Ok(()));
                    }
                    match cond.as_bool() {
                        None => {
                            return Some(Err(Box::new(self.erro(u, self.span(u, *condition), c::NON_BOOL_CONDITION))));
                        }
                        Some(true) => {
                            if let Some(Err(i)) = self.build_list(cx, list, std::slice::from_ref(then), const_child) {
                                return Some(Err(i));
                            }
                        }
                        Some(false) => {
                            if let Some(el) = else_ {
                                if let Some(Err(i)) = self.build_list(cx, list, std::slice::from_ref(el), const_child) {
                                    return Some(Err(i));
                                }
                            }
                        }
                    }
                }
                CollectionElement::MapEntry { .. } => {
                    return Some(Err(Box::new(self.erro(u, self.element_span(u, el), c::MAP_ENTRY_NOT_IN_MAP))));
                }
                CollectionElement::Spread { value, null_aware } => {
                    let v = match self.avaliar(cx, *value, const_child) {
                        Constante::Valor(v) => v,
                        Constante::Invalid(i) => return Some(Err(i)),
                    };
                    if v.estado.is_null() && *null_aware {
                        continue;
                    }
                    match v.as_list().or_else(|| v.as_set()) {
                        Some(xs) => list.extend(xs.iter().cloned()),
                        None => {
                            return Some(Err(Box::new(self.erro(u, self.span(u, *value), c::CONST_SPREAD_EXPECTED_LIST_OR_SET))));
                        }
                    }
                }
                CollectionElement::NullAwareExpression(x) => {
                    let v = match self.avaliar(cx, *x, const_child) {
                        Constante::Valor(v) => v,
                        Constante::Invalid(i) => return Some(Err(i)),
                    };
                    if v.estado.is_null() {
                        continue;
                    }
                    list.push(v);
                }
            }
        }
        None
    }

    fn build_set(&mut self, cx: &Ctx, set: &mut Vec<Valor>, elems: &[CollectionElement]) -> Option<Result<(), Box<Invalid>>> {
        let u = cx.unidade;
        for el in elems {
            match el {
                CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => match self.avaliar(cx, *x, true) {
                    Constante::Valor(v) => {
                        if matches!(el, CollectionElement::NullAwareExpression(_)) && v.estado.is_null() {
                            continue;
                        }
                        self.insert_into_set(set, v);
                    }
                    Constante::Invalid(i) => return Some(Err(i)),
                },
                CollectionElement::For { .. } | CollectionElement::ForIn { .. } => {
                    return Some(Err(Box::new(self.erro(u, self.element_span(u, el), c::CONST_EVAL_FOR_ELEMENT))));
                }
                CollectionElement::If { condition, case_pattern, then, else_, .. } => {
                    if case_pattern.is_some() {
                        return Some(Err(Box::new(self.erro(u, self.element_span(u, el), c::INVALID_CONSTANT))));
                    }
                    let cond = match self.avaliar(cx, *condition, true) {
                        Constante::Valor(v) => v,
                        Constante::Invalid(i) => return Some(Err(i)),
                    };
                    if cond.actually_unknown() {
                        return Some(Ok(()));
                    }
                    let branch = match cond.as_bool() {
                        None => return Some(Err(Box::new(self.erro(u, self.span(u, *condition), c::NON_BOOL_CONDITION)))),
                        Some(true) => Some(&**then),
                        Some(false) => else_.as_deref(),
                    };
                    if let Some(r) = branch {
                        if let Some(Err(i)) = self.build_set(cx, set, std::slice::from_ref(r)) {
                            return Some(Err(i));
                        }
                    }
                }
                CollectionElement::MapEntry { .. } => {
                    return Some(Err(Box::new(self.erro(u, self.element_span(u, el), c::MAP_ENTRY_NOT_IN_MAP))));
                }
                CollectionElement::Spread { value, null_aware } => {
                    let v = match self.avaliar(cx, *value, true) {
                        Constante::Valor(v) => v,
                        Constante::Invalid(i) => return Some(Err(i)),
                    };
                    if v.estado.is_null() && *null_aware {
                        continue;
                    }
                    match v.as_set().or_else(|| v.as_list()) {
                        Some(xs) => {
                            for x in xs.iter() {
                                self.insert_into_set(set, x.clone());
                            }
                        }
                        None => {
                            return Some(Err(Box::new(self.erro(u, self.span(u, *value), c::CONST_SPREAD_EXPECTED_LIST_OR_SET))));
                        }
                    }
                }
            }
        }
        None
    }

    fn insert_into_set(&mut self, set: &mut Vec<Valor>, v: Valor) {
        if !set.iter().any(|x| self.iguais(x, &v)) {
            set.push(v);
        }
    }

    fn build_map(&mut self, cx: &Ctx, map: &mut Vec<(Valor, Valor)>, elems: &[CollectionElement]) -> Option<Result<(), Box<Invalid>>> {
        let u = cx.unidade;
        for el in elems {
            match el {
                CollectionElement::Expression(_) | CollectionElement::NullAwareExpression(_) => {
                    return Some(Err(Box::new(self.erro(u, self.element_span(u, el), c::EXPRESSION_IN_MAP))));
                }
                CollectionElement::For { .. } | CollectionElement::ForIn { .. } => {
                    return Some(Err(Box::new(self.erro(u, self.element_span(u, el), c::CONST_EVAL_FOR_ELEMENT))));
                }
                CollectionElement::If { condition, case_pattern, then, else_, .. } => {
                    if case_pattern.is_some() {
                        return Some(Err(Box::new(self.erro(u, self.element_span(u, el), c::INVALID_CONSTANT))));
                    }
                    let cond = match self.avaliar(cx, *condition, true) {
                        Constante::Valor(v) => v,
                        Constante::Invalid(i) => return Some(Err(i)),
                    };
                    if cond.actually_unknown() {
                        return Some(Ok(()));
                    }
                    let branch = match cond.as_bool() {
                        None => return Some(Err(Box::new(self.erro(u, self.span(u, *condition), c::NON_BOOL_CONDITION)))),
                        Some(true) => Some(&**then),
                        Some(false) => else_.as_deref(),
                    };
                    if let Some(r) = branch {
                        if let Some(Err(i)) = self.build_map(cx, map, std::slice::from_ref(r)) {
                            return Some(Err(i));
                        }
                    }
                }
                CollectionElement::MapEntry { key, value, null_aware_key, null_aware_value } => {
                    let k = self.avaliar(cx, *key, true);
                    let v = self.avaliar(cx, *value, true);
                    let k = match k {
                        Constante::Valor(k) => k,
                        Constante::Invalid(i) => return Some(Err(i)),
                    };
                    let v = match v {
                        Constante::Valor(v) => v,
                        Constante::Invalid(i) => return Some(Err(i)),
                    };
                    if (*null_aware_key && k.estado.is_null()) || (*null_aware_value && v.estado.is_null()) {
                        continue;
                    }
                    if let Some(p) = map.iter().position(|(x, _)| self.iguais(x, &k)) {
                        map[p].1 = v;
                    } else {
                        map.push((k, v));
                    }
                }
                CollectionElement::Spread { value, null_aware } => {
                    let v = match self.avaliar(cx, *value, true) {
                        Constante::Valor(v) => v,
                        Constante::Invalid(i) => return Some(Err(i)),
                    };
                    if v.estado.is_null() && *null_aware {
                        continue;
                    }
                    match v.as_map() {
                        Some(xs) => {
                            for (k, x) in xs.iter() {
                                if let Some(p) = map.iter().position(|(y, _)| self.iguais(y, k)) {
                                    map[p].1 = x.clone();
                                } else {
                                    map.push((k.clone(), x.clone()));
                                }
                            }
                        }
                        None => return Some(Err(Box::new(self.erro(u, self.span(u, *value), c::CONST_SPREAD_EXPECTED_MAP)))),
                    }
                }
            }
        }
        None
    }

    /// O intervalo de um elemento de coleção.
    pub fn element_span(&self, u: UnitId, el: &CollectionElement) -> Span {
        let a = self.ast(u);
        let source = self.program.unit(u).source.as_bytes();
        match el {
            CollectionElement::Expression(x) => a.expr(*x).span,
            CollectionElement::NullAwareExpression(x) => {
                let s = a.expr(*x).span;
                Span { start: back_up_to(source, s.start, b'?'), end: s.end }
            }
            CollectionElement::MapEntry { key, value, .. } => {
                let (k, v) = (a.expr(*key).span, a.expr(*value).span);
                Span { start: back_up_to(source, k.start, b'?'), end: v.end }
            }
            CollectionElement::Spread { value, .. } => {
                let s = a.expr(*value).span;
                Span { start: back_up_word(source, s.start, &["...?", "..."]), end: s.end }
            }
            CollectionElement::If { condition, then, else_, .. } => {
                let s = a.expr(*condition).span;
                let end = else_.as_ref().map(|x| self.element_span(u, x)).unwrap_or_else(|| self.element_span(u, then)).end;
                Span { start: back_up_word(source, s.start, &["if"]), end: end }
            }
            CollectionElement::For { body, init, condition, updates, .. } => {
                let end = self.element_span(u, body).end;
                let first = condition
                    .map(|x| a.expr(x).span.start)
                    .or_else(|| updates.first().map(|x| a.expr(*x).span.start))
                    .unwrap_or(end);
                let _ = init;
                Span { start: inicio_de_for(source, first), end: end }
            }
            CollectionElement::ForIn { body, iterable, .. } => {
                let end = self.element_span(u, body).end;
                Span { start: inicio_de_for(source, a.expr(*iterable).span.start), end: end }
            }
        }
    }

    // -- Operadores ------------------------------------------------------------

    fn is_of_extension(&self, u: UnitId, e: ExprId) -> Option<Codigo> {
        match self.resolvido(u, e)? {
            Resolved::ExtensionMember { .. } => Some(c::CONST_EVAL_EXTENSION_METHOD),
            Resolved::Member { class, .. } if self.program.class(*class).kind == ClassKind::ExtensionType => {
                Some(c::CONST_EVAL_EXTENSION_TYPE_METHOD)
            }
            _ => None,
        }
    }

    fn unary(&mut self, cx: &Ctx, e: ExprId, op: UnaryOp, operand: ExprId, in_const: bool) -> R {
        let u = cx.unidade;
        if let Some(code) = self.is_of_extension(u, e) {
            return self.inv(u, e, code);
        }
        // Pós-fixos (`a++`, `a!`) não têm visita própria (`visitNode`): o
        // erro genérico no nó. `visitPrefixExpression`: o operando primeiro
        // (o erro dele vence); só então `++`/`--` são o erro genérico.
        if matches!(op, UnaryOp::PostfixInc | UnaryOp::PostfixDec | UnaryOp::NullAssert) {
            return self.generic(u, e, false);
        }
        let v = match self.avaliar(cx, operand, in_const) {
            Constante::Valor(v) => v,
            i => return i,
        };
        if !matches!(op, UnaryOp::Neg | UnaryOp::Not | UnaryOp::BitNot) {
            return self.generic(u, e, false);
        }
        let r = match op {
            UnaryOp::Not => v.estado.logical_not().map(|s| (self.core.bool_, s)),
            UnaryOp::BitNot => v.estado.negate_bits().map(|s| (self.core.int, s)),
            _ => v.estado.negate().map(|s| {
                let t = if matches!(s, Estado::Double(_)) { self.core.double } else { self.core.int };
                (t, s)
            }),
        };
        match r {
            Ok((t, s)) => Constante::Valor(Valor::novo(t, s)),
            Err(x) => self.excecao(u, e, x),
        }
    }

    fn binary(&mut self, cx: &Ctx, e: ExprId, op: BinaryOp, left: ExprId, right: ExprId, in_const: bool) -> R {
        let u = cx.unidade;
        if let Some(code) = self.is_of_extension(u, e) {
            return self.inv(u, e, code);
        }
        let l = match self.avaliar(cx, left, in_const) {
            Constante::Valor(v) => v,
            i => return i,
        };
        match op {
            BinaryOp::And | BinaryOp::Or => {
                let short_circuit = if op == BinaryOp::And { Some(false) } else { Some(true) };
                if l.as_bool() == short_circuit {
                    if let Some(i) = self.not_potentially_constant(cx, right) {
                        return i;
                    }
                }
                // `lazyAnd`/`lazyOr`.
                let r = (|| -> Result<Estado, Excecao> {
                    match &l.estado {
                        Estado::Bool(v) => {
                            if *v == short_circuit {
                                return Ok(Estado::Bool(*v));
                            }
                            let d = match self.avaliar(cx, right, in_const) {
                                Constante::Valor(d) => d,
                                Constante::Invalid(i) => return Err(Excecao::nova(i.codigo)),
                            };
                            if !d.estado.e_bool() {
                                return Err(Excecao::nova(c::CONST_EVAL_TYPE_BOOL));
                            }
                            if v.is_none() {
                                return Ok(Estado::Bool(None));
                            }
                            d.estado.para_bool()
                        }
                        _ => Err(Excecao::nova(c::CONST_EVAL_TYPE_BOOL)),
                    }
                })();
                return match r {
                    Ok(s) => Constante::Valor(Valor::novo(self.core.bool_, s)),
                    Err(x) => self.excecao(u, e, x),
                };
            }
            BinaryOp::IfNull => {
                if !l.estado.is_null() {
                    if let Some(i) = self.not_potentially_constant(cx, right) {
                        return i;
                    }
                    return Constante::Valor(l);
                }
                return self.avaliar(cx, right, in_const);
            }
            _ => {}
        }
        let r = match self.avaliar(cx, right, in_const) {
            Constante::Valor(v) => v,
            i => return i,
        };
        let res: Result<(TypeId, Estado), Excecao> = match op {
            BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
                if l.estado.e_bool() && r.estado.e_bool() {
                    let s = match op {
                        BinaryOp::BitAnd => l.estado.logical_and(&r.estado),
                        BinaryOp::BitOr => l.estado.logical_or(&r.estado),
                        _ => l.estado.logical_xor(&r.estado),
                    };
                    s.map(|s| (self.core.bool_, s))
                } else if l.estado.e_int() && r.estado.e_int() {
                    let s = match op {
                        BinaryOp::BitAnd => l.estado.e_bit(&r.estado),
                        BinaryOp::BitOr => l.estado.bit_or(&r.estado),
                        _ => l.estado.bit_xor(&r.estado),
                    };
                    s.map(|s| (self.core.int, s))
                } else {
                    Err(Excecao::nova(c::CONST_EVAL_TYPE_BOOL_INT))
                }
            }
            BinaryOp::Eq | BinaryOp::NotEq => {
                let lib = cx.lib;
                self.equal_equal(lib, &l, &r).map(|b| {
                    let b = if op == BinaryOp::NotEq { b.map(|x| !x) } else { b };
                    (self.core.bool_, Estado::Bool(b))
                })
            }
            BinaryOp::Gt => l.estado.greater(&r.estado).map(|s| (self.core.bool_, s)),
            BinaryOp::GtEq => l.estado.greater_or_equal(&r.estado).map(|s| (self.core.bool_, s)),
            BinaryOp::Lt => l.estado.less(&r.estado).map(|s| (self.core.bool_, s)),
            BinaryOp::LtEq => l.estado.less_or_equal(&r.estado).map(|s| (self.core.bool_, s)),
            BinaryOp::Shl => l.estado.shift_left(&r.estado).map(|s| (self.core.int, s)),
            BinaryOp::Shr => l.estado.shift_right(&r.estado).map(|s| (self.core.int, s)),
            BinaryOp::UShr => l.estado.logical_shift_right(&r.estado).map(|s| (self.core.int, s)),
            BinaryOp::TruncDiv => l.estado.integer_divide(&r.estado).map(|s| (self.core.int, s)),
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                let s = match op {
                    BinaryOp::Add => l.estado.somar(&r.estado),
                    BinaryOp::Sub => l.estado.subtract(&r.estado),
                    BinaryOp::Mul => l.estado.multiply(&r.estado),
                    BinaryOp::Div => l.estado.divide(&r.estado),
                    _ => l.estado.resto(&r.estado),
                };
                s.map(|s| {
                    let t = match s {
                        Estado::Int(_) => self.core.int,
                        Estado::Double(_) => self.core.double,
                        _ => self.core.string,
                    };
                    (t, s)
                })
            }
            _ => return self.generic(u, e, false),
        };
        match res {
            Ok((t, s)) => Constante::Valor(Valor::novo(t, s)),
            Err(x) => self.excecao(u, e, x),
        }
    }

    /// `equalEqual`: com `patterns` ligado (3.0+), primitiva ou `double`.
    fn equal_equal(&mut self, lib: LibraryId, l: &Valor, r: &Valor) -> Result<Option<bool>, Excecao> {
        if l.estado.is_null() || r.estado.is_null() {
            return Ok(Some(l.estado.is_null() && r.estado.is_null()));
        }
        let patterns = self.program.library(lib).features.versao() >= dartforge_frontend::features::LanguageVersion::new(3, 0);
        let can = if patterns {
            matches!(l.estado, Estado::Double(_)) || self.primitive_equality(l, lib)
        } else {
            l.estado.is_bool_num_string_or_null()
        };
        if !can {
            return Err(Excecao::nova(c::CONST_EVAL_TYPE_BOOL_NUM_STRING));
        }
        Ok(self.identical_state(&l.estado, &r.estado))
    }

    /// `identical` (`isIdentical2`).
    pub fn identical(&mut self, l: &Valor, r: &Valor) -> Option<bool> {
        let (int, double) = (self.core.int, self.core.double);
        if (l.tipo == int && r.tipo == double) || (l.tipo == double && r.tipo == int) {
            return None;
        }
        if l.tipo != r.tipo {
            return Some(false);
        }
        self.identical_state(&l.estado, &r.estado)
    }

    /// `InstanceState.isIdentical` (e `equalEqual`, que é o mesmo aqui).
    fn identical_state(&mut self, l: &Estado, r: &Estado) -> Option<bool> {
        match (l, r) {
            (Estado::Bool(a), b) => {
                let a = (*a)?;
                match b {
                    Estado::Bool(b) => Some(a == (*b)?),
                    _ => Some(false),
                }
            }
            (Estado::Int(a), b) => {
                let a = (*a)?;
                match b {
                    Estado::Int(b) => Some(a == (*b)?),
                    Estado::Double(b) => Some((*b)? == a as f64),
                    _ => Some(false),
                }
            }
            (Estado::Double(a), b) => {
                let a = (*a)?;
                if a.is_nan() {
                    return Some(false);
                }
                match b {
                    Estado::Double(b) => {
                        let b = (*b)?;
                        if b.is_nan() {
                            return Some(false);
                        }
                        Some(a.to_bits() == b.to_bits())
                    }
                    Estado::Int(b) => Some(a == (*b)? as f64),
                    _ => Some(false),
                }
            }
            (Estado::Str(a), b) => {
                let a = a.as_ref()?;
                match b {
                    Estado::Str(b) => Some(a == b.as_ref()?),
                    _ => Some(false),
                }
            }
            (Estado::Null { .. }, b) => Some(b.is_null()),
            (Estado::Simbolo(a), b) => {
                let a = a.as_ref()?;
                match b {
                    Estado::Simbolo(b) => Some(a == b.as_ref()?),
                    _ => Some(false),
                }
            }
            (Estado::Tipo(a), b) => {
                let a = (*a)?;
                match b {
                    Estado::Tipo(b) => Some(a == (*b)?),
                    _ => Some(false),
                }
            }
            (Estado::Funcao { elemento: a, args: x }, b) => match b {
                Estado::Funcao { elemento: b, args: y } => Some(a == b && x == y),
                _ => Some(false),
            },
            (Estado::Lista { unknown, .. }, b) | (Estado::Conjunto { desconhecido: unknown, .. }, b) | (Estado::Mapa { desconhecido: unknown, .. }, b)
                if *unknown || b.desconhecido() =>
            {
                None
            }
            (Estado::Registro { .. }, _) => {
                if !self.equal_states(l, r) {
                    Some(false)
                } else {
                    None
                }
            }
            _ => Some(self.equal_states(l, r)),
        }
    }

    /// `==` de `DartObjectImpl` (a igualdade estrutural das chaves de mapa e
    /// dos elementos de conjunto).
    pub fn iguais(&mut self, a: &Valor, b: &Valor) -> bool {
        a.tipo == b.tipo && self.equal_states(&a.estado, &b.estado)
    }

    fn equal_states(&mut self, a: &Estado, b: &Estado) -> bool {
        // Valor não conhecido (constante de outra biblioteca, opaca) não é
        // provadamente igual a nada: dois `CompileTimeErrorCode.X` vindos
        // de fora não são o mesmo elemento de conjunto.
        if a.desconhecido() || b.desconhecido() {
            return false;
        }
        match (a, b) {
            (Estado::Bool(x), Estado::Bool(y)) => x == y,
            (Estado::Int(x), Estado::Int(y)) => x == y,
            (Estado::Double(x), Estado::Double(y)) => x.map(f64::to_bits) == y.map(f64::to_bits),
            (Estado::Str(x), Estado::Str(y)) => x == y,
            (Estado::Null { .. }, Estado::Null { .. }) => true,
            (Estado::Simbolo(x), Estado::Simbolo(y)) => x == y,
            (Estado::Tipo(x), Estado::Tipo(y)) => x == y,
            (Estado::Funcao { elemento: x, args: p }, Estado::Funcao { elemento: y, args: q }) => x == y && p == q,
            (Estado::Lista { elementos: x, .. }, Estado::Lista { elementos: y, .. })
            | (Estado::Conjunto { elementos: x, .. }, Estado::Conjunto { elementos: y, .. }) => {
                x.len() == y.len() && x.iter().zip(y.iter()).all(|(p, q)| self.iguais(p, q))
            }
            (Estado::Mapa { entradas: x, .. }, Estado::Mapa { entradas: y, .. }) => {
                x.len() == y.len()
                    && x.iter().all(|(k, v)| y.iter().any(|(k2, v2)| self.iguais(k, k2) && self.iguais(v, v2)))
            }
            (Estado::Registro { posicionais: p1, nomeados: n1 }, Estado::Registro { posicionais: p2, nomeados: n2 }) => {
                p1.len() == p2.len()
                    && n1.len() == n2.len()
                    && p1.iter().zip(p2.iter()).all(|(a, b)| self.iguais(a, b))
                    && n1.iter().all(|(n, v)| n2.iter().any(|(m, w)| m == n && self.iguais(v, w)))
            }
            (Estado::Generico { campos: x, .. }, Estado::Generico { campos: y, .. }) => {
                x.len() == y.len() && x.iter().all(|(k, v)| y.iter().any(|(k2, v2)| k == k2 && self.iguais(v, v2)))
            }
            _ => false,
        }
    }

    /// `hasPrimitiveEquality`.
    pub fn primitive_equality(&mut self, v: &Valor, lib: LibraryId) -> bool {
        match &v.estado {
            Estado::Registro { posicionais: positional_args, nomeados: named_args } => {
                let (p, n) = (positional_args.clone(), named_args.clone());
                p.iter().all(|x| self.primitive_equality(x, lib)) && n.iter().all(|(_, x)| self.primitive_equality(x, lib))
            }
            Estado::Generico { .. } => {
                let Type::Interface { class, .. } = self.table.get(v.tipo).clone() else { return false };
                let patterns = self.program.library(lib).features.versao() >= dartforge_frontend::features::LanguageVersion::new(3, 0);
                let eq = self.interner.lookup("==");
                let hash = self.interner.lookup("hashCode");
                let of_object = |m: &Motor<'_>, s: Option<SymbolId>| match s.and_then(|s| m.concrete_member(class, s)) {
                    Some(c) => Some(c) == m.core.object_class,
                    None => true,
                };
                of_object(self, eq) && (!patterns || of_object(self, hash))
            }
            Estado::Double(_) => false,
            _ => true,
        }
    }

    /// A classe que declara o membro de instância concreto `nome` visto de
    /// `c` (superclasses e mixins, na ordem de busca).
    fn concrete_member(&self, c: ClassId, the_name: SymbolId) -> Option<ClassId> {
        let mut current = Some(c);
        let mut seen = 0;
        while let Some(k) = current {
            seen += 1;
            if seen > 100 {
                return None;
            }
            let cls = self.program.class(k);
            if let Some(f) = cls.instance_members.get(&the_name) {
                if !self.program.function(*f).abstract_ {
                    return Some(k);
                }
            }
            for m in cls.mixin_classes.iter().rev() {
                if let Some(f) = self.program.class(*m).instance_members.get(&the_name) {
                    if !self.program.function(*f).abstract_ {
                        return Some(*m);
                    }
                }
            }
            current = cls.supertype_class;
        }
        None
    }

    // -- Nomes -----------------------------------------------------------------

    fn propriedade(&mut self, cx: &Ctx, e: ExprId, tgt: ExprId, the_name: ast::Name, in_const: bool) -> R {
        let u = cx.unidade;
        let a = self.ast(u);
        let simple_target = matches!(a.expr(tgt).kind, ExprKind::Identifier(_));
        let res_target = self.resolvido(u, tgt).cloned();
        match res_target {
            // Constante de topo importada com prefixo; do prefixo adiado,
            // o erro de biblioteca adiada no nome (`visitPrefixedIdentifier`,
            // 3.6.2 `evaluation.dart:1108-1115`).
            Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..))) => {
                if self.deferred_prefix(u, tgt) {
                    return self.deferred_library_error(u, e, the_name.span);
                }
                return self.constant_value(cx, e, e, true);
            }
            // `p.Ext.m` (`visitPropertyAccess`, `:1176-1185`): com `p`
            // adiado, o erro no nome da extensão.
            Some(Resolved::Element(Element::Extension(_))) => {
                if let ExprKind::Property { target: p, name: ext_name, .. } = &a.expr(tgt).kind
                    && matches!(self.resolvido(u, *p), Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..))))
                    && self.deferred_prefix(u, *p)
                {
                    let sp = ext_name.span;
                    return self.deferred_library_error(u, e, sp);
                }
                return self.constant_value(cx, e, e, true);
            }
            _ => {}
        }
        if let Some(Resolved::Element(Element::Class(k))) = &res_target {
            if self.program.class(*k).kind == ClassKind::ExtensionType && !simple_target {
                return self.constant_value(cx, e, e, true);
            }
        }
        let prefix = match self.avaliar(cx, tgt, in_const) {
            Constante::Valor(v) => v,
            i => return i,
        };
        let e_interface = simple_target && matches!(res_target, Some(Resolved::Element(Element::Class(_) | Element::Typedef(_))));
        // `C<T>.new`, `C.nome`: o `ConstructorReference` (tear-off de
        // construtor), não um acesso a propriedade do `Type`.
        if matches!(self.resolvido(u, e), Some(Resolved::Constructor(_))) {
            return self.constant_value(cx, e, e, true);
        }
        if !e_interface {
            if let Some(r) = self.property_access(u, e, &prefix, the_name) {
                return r;
            }
        }
        self.constant_value(cx, e, e, true)
    }

    /// `PrefixedIdentifier.isDeferred` (3.6.2 `ast.dart:14386-14396`): o
    /// prefixo `p` (um identificador) é de exatamente um import da unidade, e
    /// esse import é `deferred`.
    fn deferred_prefix(&self, u: UnitId, p: ExprId) -> bool {
        let ExprKind::Identifier(n) = &self.ast(u).expr(p).kind else {
            return false;
        };
        let lib = self.program.unit(u).library;
        let mut imports = self.program.library(lib).imports.iter().filter(|i| i.unit == u && i.prefix == Some(n.sym));
        match (imports.next(), imports.next()) {
            (Some(i), None) => i.deferred,
            _ => false,
        }
    }

    /// `_getDeferredLibraryError` (3.6.2 `evaluation.dart:1873-1933`): o código
    /// pelo primeiro ancestral de `no` que o decide, no `alvo`; sem nenhum,
    /// `INVALID_CONSTANT` no próprio `no`.
    fn deferred_library_error(&self, u: UnitId, no: ExprId, tgt: Span) -> R {
        match self.deferred_library_code(u, no) {
            Some(code) => Constante::Invalid(Box::new(self.erro(u, tgt, code))),
            None => self.inv(u, no, c::INVALID_CONSTANT),
        }
    }

    /// A subida do `_getDeferredLibraryError`: `Annotation`, `DefaultFormalParameter`,
    /// `IfElement` cuja condição é o próprio `no`, `InstanceCreationExpression`,
    /// `ListLiteral`, `MapLiteralEntry` (chave ou valor, pelo filho), `RecordLiteral`,
    /// `SetOrMapLiteral`, `SpreadElement`, `SwitchCase` (o antigo), `SwitchPatternCase`
    /// e `VariableDeclaration`. Os pais vêm de `dartforge_frontend::pais`; onde o
    /// pai não é expressão (instrução, padrão, valor padrão…), a subida continua
    /// pela menor expressão que contém o trecho (uma expressão de função, um
    /// `switch` de expressão, a coleção do `if (… case …)`).
    fn deferred_library_code(&self, u: UnitId, no: ExprId) -> Option<Codigo> {
        let a = self.ast(u);
        let pais = crate::lints_tipados::pais_da_unidade(self.program, u);
        let lib = self.program.unit(u).library;
        let before_3 = self.program.library(lib).features.versao() < dartforge_frontend::features::LanguageVersion::new(3, 0);
        let contains = |of: Span, sp: Span| of.start <= sp.start && sp.end <= of.end;
        let mut current = no;
        for _ in 0..a.exprs.len() + 1 {
            let sp = a.expr(current).span;
            let next_ = match pais.pai(current) {
                dartforge_frontend::pais::Pai::Expr(p) => Some(p),
                dartforge_frontend::pais::Pai::Anotacao => return Some(c::INVALID_ANNOTATION_CONSTANT_VALUE_FROM_DEFERRED_LIBRARY),
                dartforge_frontend::pais::Pai::Variavel { .. } => return Some(c::CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE_FROM_DEFERRED_LIBRARY),
                // `EnumConstantArguments` → `EnumConstantDeclaration` →
                // `EnumDeclaration`: nenhum decide.
                dartforge_frontend::pais::Pai::ConstanteDeEnum => return None,
                dartforge_frontend::pais::Pai::PadraoConstante { .. } => {
                    // No `case` de um `switch` de instrução, o código do caso.
                    let in_case = a.stmts.iter().any(|s| match &s.kind {
                        ast::StmtKind::Switch { cases, .. } => {
                            cases.iter().any(|k| k.pattern.is_some_and(|p| contains(a.pattern(p).span, sp)))
                        }
                        _ => false,
                    });
                    if in_case {
                        return Some(if before_3 {
                            c::NON_CONSTANT_CASE_EXPRESSION_FROM_DEFERRED_LIBRARY
                        } else {
                            c::PATTERN_CONSTANT_FROM_DEFERRED_LIBRARY
                        });
                    }
                    None
                }
                dartforge_frontend::pais::Pai::Fronteira => {
                    let pattern = |ps: &[ast::Parameter]| ps.iter().any(|p| p.default_value == Some(current));
                    fn nested(ps: &[ast::Parameter], current: ExprId) -> bool {
                        ps.iter().any(|p| {
                            p.default_value == Some(current) || p.function_parameters.as_deref().is_some_and(|fs| nested(fs, current))
                        })
                    }
                    let is_pattern = a.members.iter().any(|m| matches!(&m.kind, ast::MemberKind::Constructor(k) if pattern(&k.parameters) || nested(&k.parameters, current)))
                        || a.functions.iter().any(|f| f.parameters.as_deref().is_some_and(|ps| nested(ps, current)))
                        || a.types.iter().any(|t| matches!(&t.kind, ast::TypeKind::Function { parameters, .. } if nested(parameters, current)));
                    if is_pattern {
                        return Some(c::NON_CONSTANT_DEFAULT_VALUE_FROM_DEFERRED_LIBRARY);
                    }
                    None
                }
            };
            // O pai que não é expressão: a menor expressão que contém o trecho.
            let p = match next_ {
                Some(p) => p,
                None => {
                    let mut best: Option<(usize, ExprId)> = None;
                    for (i, x) in a.exprs.iter().enumerate() {
                        let id = ExprId(i as u32);
                        if id == current || !contains(x.span, sp) || x.span == sp {
                            continue;
                        }
                        let tam = x.span.end - x.span.start;
                        if best.is_none_or(|(t, _)| tam < t) {
                            best = Some((tam, id));
                        }
                    }
                    match best {
                        Some((_, id)) => id,
                        None => return None,
                    }
                }
            };
            if let Some(code) = self.role_on_ascent(u, p, sp, no) {
                return Some(code);
            }
            current = p;
        }
        None
    }

    /// O código que a expressão `p`, pai do trecho `filho`, decide na subida.
    fn role_on_ascent(&self, u: UnitId, p: ExprId, child: Span, no: ExprId) -> Option<Codigo> {
        let a = self.ast(u);
        let contains = |of: Span, sp: Span| of.start <= sp.start && sp.end <= of.end;
        // O elemento de coleção que contém o filho, do mais fundo ao mais raso.
        fn elemento(a: &ast::Ast, el: &ast::CollectionElement, child: Span, no: ExprId, contains: &dyn Fn(Span, Span) -> bool) -> Option<Option<Codigo>> {
            let span = |e: ExprId| a.expr(e).span;
            match el {
                ast::CollectionElement::Expression(e) | ast::CollectionElement::NullAwareExpression(e) => {
                    contains(span(*e), child).then_some(None)
                }
                ast::CollectionElement::MapEntry { key, value, .. } => {
                    if contains(span(*key), child) {
                        Some(Some(c::NON_CONSTANT_MAP_KEY_FROM_DEFERRED_LIBRARY))
                    } else if contains(span(*value), child) {
                        Some(Some(c::NON_CONSTANT_MAP_VALUE_FROM_DEFERRED_LIBRARY))
                    } else {
                        None
                    }
                }
                ast::CollectionElement::Spread { value, .. } => {
                    contains(span(*value), child).then_some(Some(c::SPREAD_EXPRESSION_FROM_DEFERRED_LIBRARY))
                }
                ast::CollectionElement::If { condition, case_pattern, guard, then, else_ } => {
                    for sub in std::iter::once(&**then).chain(else_.as_deref()) {
                        if let Some(r) = elemento(a, sub, child, no, contains) {
                            return Some(r);
                        }
                    }
                    let inside = contains(span(*condition), child)
                        || case_pattern.is_some_and(|p| contains(a.pattern(p).span, child))
                        || guard.is_some_and(|g| contains(span(g), child));
                    if !inside {
                        return None;
                    }
                    Some((*condition == no).then_some(c::IF_ELEMENT_CONDITION_FROM_DEFERRED_LIBRARY))
                }
                ast::CollectionElement::For { condition, updates, body, .. } => {
                    if let Some(r) = elemento(a, body, child, no, contains) {
                        return Some(r);
                    }
                    let inside = condition.is_some_and(|x| contains(span(x), child)) || updates.iter().any(|x| contains(span(*x), child));
                    inside.then_some(None)
                }
                ast::CollectionElement::ForIn { iterable, body, .. } => {
                    if let Some(r) = elemento(a, body, child, no, contains) {
                        return Some(r);
                    }
                    contains(span(*iterable), child).then_some(None)
                }
            }
        }
        match &a.expr(p).kind {
            ExprKind::InstanceCreation { .. } => Some(c::CONST_CONSTRUCTOR_CONSTANT_FROM_DEFERRED_LIBRARY),
            ExprKind::Call { .. } if matches!(self.resolvido(u, p), Some(Resolved::Constructor(_))) => {
                Some(c::CONST_CONSTRUCTOR_CONSTANT_FROM_DEFERRED_LIBRARY)
            }
            ExprKind::Record { .. } => Some(c::NON_CONSTANT_RECORD_FIELD_FROM_DEFERRED_LIBRARY),
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                let do_literal = if matches!(a.expr(p).kind, ExprKind::List { .. }) {
                    c::NON_CONSTANT_LIST_ELEMENT_FROM_DEFERRED_LIBRARY
                } else {
                    c::SET_ELEMENT_FROM_DEFERRED_LIBRARY
                };
                for el in elements.iter() {
                    if let Some(r) = elemento(a, el, child, no, &contains) {
                        return Some(r.unwrap_or(do_literal));
                    }
                }
                Some(do_literal)
            }
            _ => None,
        }
    }

    /// `_evaluatePropertyAccess`.
    fn property_access(&mut self, u: UnitId, e: ExprId, tgt: &Valor, the_name: ast::Name) -> Option<R> {
        let res = self.resolvido(u, e).cloned();
        let static_ = |m: &Motor<'_>, f: FunctionElementId| m.program.function(f).static_;
        match &res {
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) if static_(self, *f) => return None,
            Some(Resolved::Member { member: MemberRef::Variable(v), .. }) if self.program.variable(*v).static_ => return None,
            Some(Resolved::ExtensionMember { .. }) => return Some(self.inv(u, e, c::CONST_EVAL_EXTENSION_METHOD)),
            Some(Resolved::Member { class, .. }) if self.program.class(*class).kind == ClassKind::ExtensionType => {
                return Some(self.inv(u, e, c::CONST_EVAL_EXTENSION_TYPE_METHOD));
            }
            _ => {}
        }
        if Some(the_name.sym) == self.sym_length && tgt.tipo == self.core.string {
            return Some(match tgt.estado.comprimento() {
                Ok(s) => Constante::Valor(Valor::novo(self.core.int, s)),
                Err(x) => self.excecao(u, e, x),
            });
        }
        let mut i = self.erro(u, self.span(u, e), c::CONST_EVAL_PROPERTY_ACCESS);
        i.args = vec![self.interner.resolve(the_name.sym).to_string(), self.formatar(tgt.tipo)];
        Some(Constante::Invalid(Box::new(i)))
    }

    /// `alvo(args)` que cria pelo construtor primário de um tipo de extensão:
    /// o alvo é o tipo (`A`) ou `A.nome` com o nome do primário. Fora de
    /// contexto constante é o genérico; o primário sem `const`, também.
    fn creation_via_extension_primary(&mut self, cx: &Ctx, e: ExprId, tgt: ExprId, arguments: &ast::Arguments, in_const: bool) -> Option<R> {
        let u = cx.unidade;
        let a = self.ast(u);
        let (cls, the_name) = match &a.expr(tgt).kind {
            ExprKind::Identifier(_) => (self.resolvido(u, tgt).cloned(), None),
            ExprKind::Property { target, name, .. } => (self.resolvido(u, *target).cloned(), Some(name.sym)),
            _ => return None,
        };
        let Some(Resolved::Element(Element::Class(k))) = cls else { return None };
        self.extension_primary(cx, e, k, the_name, arguments, in_const)
    }

    /// A criação pelo construtor primário do tipo de extensão `k` (`nome`: o
    /// do construtor escrito): vale a representação, o argumento avaliado.
    fn extension_primary(&mut self, cx: &Ctx, e: ExprId, k: ClassId, the_name: Option<SymbolId>, arguments: &ast::Arguments, in_const: bool) -> Option<R> {
        let u = cx.unidade;
        let d = self.program.class(k).decl?;
        let ast::DeclKind::ExtensionType(et) = &self.ast(d.unit).decl(d.decl).kind else { return None };
        let of_primary = match (the_name, et.constructor) {
            (None, None) => true,
            (Some(n), Some(c)) => n == c.sym,
            (Some(n), None) => self.interner.resolve(n) == "new",
            (None, Some(_)) => false,
        };
        if !of_primary {
            return None;
        }
        if !in_const || !et.const_ {
            return Some(self.generic(u, e, false));
        }
        let x = arguments.args.iter().find(|x| x.name.is_none())?;
        Some(self.avaliar(cx, x.value, true))
    }

    /// A instanciação implícita do tear-off `e` (`typeArgumentTypes` de um
    /// `FunctionReference` sem argumentos escritos) com algum argumento que
    /// menciona parâmetro de tipo depois do ambiente léxico do construtor
    /// (3.6.2 `evaluation.dart:862-876`).
    fn instantiation_with_parameter(&mut self, cx: &Ctx, e: ExprId) -> bool {
        let inferred: Option<Vec<TypeId>> = self.body.units.get(cx.unidade.0 as usize).and_then(|b| b.instanciacao_de_tearoff(e)).map(|a| a.to_vec());
        let Some(args) = inferred else { return false };
        // Só o argumento que é ele mesmo um parâmetro de tipo passa pelo
        // `_lexicalTypeEnvironment` (`:866-871`); `List<U>` fica como está.
        args.iter().any(|&a| {
            let a = match (self.table.get(a), &cx.tipos) {
                (Type::TypeParameter { param, .. }, Some(m)) => m.get(param).copied().unwrap_or(a),
                _ => a,
            };
            self.mentions_parameter(a)
        })
    }

    /// `_getConstantValue` do elemento a que `e` resolve (`erro_em` é o nó
    /// do erro).
    fn constant_value(&mut self, cx: &Ctx, e: ExprId, error_at: ExprId, with_identifier: bool) -> R {
        let u = cx.unidade;
        let res = self.resolvido(u, e).cloned();
        let _ = with_identifier;
        match res {
            Some(Resolved::Element(Element::Variable(v))) | Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => {
                // A variável que guarda uma função genérica, instanciada
                // implicitamente: o valor primeiro, depois os argumentos.
                let r = self.variable_reference_value(cx, error_at, v);
                if matches!(r, Constante::Valor(_)) && self.instantiation_with_parameter(cx, e) {
                    return self.inv(u, e, c::CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF);
                }
                return r;
            }
            // O estático de extensão citado pelo nome simples (o `foo` de
            // `@A(foo)` num membro da extensão) é um `ExecutableElement`
            // estático como o de classe: o valor é a função
            // (`_getConstantValue`, 3.6.2 `evaluation.dart`).
            Some(Resolved::Element(Element::Function(f)))
            | Some(Resolved::Member { member: MemberRef::Function(f), .. })
            | Some(Resolved::ExtensionMember { member: f, .. }) => {
                let fe = self.program.function(f);
                // `values` de um enum: o campo `const` sintético cujo
                // inicializador é a lista das constantes
                // (`LibraryBuilder.buildEnumChildren`); no modelo, um getter
                // estático sem nó.
                if fe.node == FunctionRef::None
                    && fe.static_
                    && fe.kind == FunctionKind::Getter
                    && let Some(k) = fe.class
                    && self.program.class(k).kind == ClassKind::Enum
                    && self.interner.resolve(fe.name) == "values"
                {
                    return self.valores_do_enum(u, error_at, k);
                }
                if fe.kind == FunctionKind::ImplicitAccessor {
                    if let Some(v) = fe.variable {
                        return self.variable_reference_value(cx, error_at, v);
                    }
                }
                let the_static_ = fe.class.is_none() && fe.extension.is_none() || fe.static_;
                if the_static_ && matches!(fe.kind, FunctionKind::Function | FunctionKind::Operator) {
                    // Instanciação implícita do tear-off genérico
                    // (`visitFunctionReference` sem argumentos escritos, 3.6.2
                    // `evaluation.dart:862-876`): um argumento inferido que
                    // menciona parâmetro de tipo (depois do ambiente léxico do
                    // construtor) é `CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF`.
                    if self.instantiation_with_parameter(cx, e) {
                        return self.inv(u, e, c::CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF);
                    }
                    let t = self.outline.functions.get(f.0 as usize).map(|d| d.signature).unwrap_or(self.core.dynamic_);
                    let t = if self.inferred_(u) { self.estatico(u, e) } else { t };
                    // `_instantiateFunctionType` (`evaluation.dart:1939-1959`):
                    // o tipo instanciado passa pela substituição do construtor.
                    let t = match &cx.tipos {
                        Some(m) => crate::ops::substitute(t, m, self.table),
                        None => t,
                    };
                    return Constante::Valor(self.valor(t, Estado::Funcao { elemento: Funcao::Elemento(f), args: None }));
                }
            }
            Some(Resolved::Constructor(f)) => {
                let t = self.estatico(u, e);
                return Constante::Valor(self.valor(t, Estado::Funcao { elemento: Funcao::Elemento(f), args: None }));
            }
            Some(Resolved::Element(Element::Class(k))) => {
                let n = self.program.class(k).type_params.len();
                let args = vec![self.core.dynamic_; n];
                let t = if self.program.class(k).kind == ClassKind::ExtensionType {
                    self.table.intern(Type::ExtensionType { decl: k, args: args.into_boxed_slice(), nullable: false })
                } else {
                    self.table.intern(Type::Interface { class: k, args: args.into_boxed_slice(), nullable: false })
                };
                let t = self.apagar(t);
                return Constante::Valor(Valor::novo(self.core.type_, Estado::Tipo(Some(t))));
            }
            Some(Resolved::Element(Element::Typedef(_))) => {
                return Constante::Valor(Valor::novo(self.core.type_, Estado::Tipo(None)));
            }
            Some(Resolved::TypeParameter(tp)) => {
                if let Some(t) = cx.tipos.as_ref().and_then(|m| m.get(&tp)) {
                    let t = *t;
                    return Constante::Valor(Valor::novo(self.core.type_, Estado::Tipo(Some(t))));
                }
                return self.inv(u, error_at, c::CONST_TYPE_PARAMETER);
            }
            Some(Resolved::Local(_)) => {
                if let Some(r) = self.local_value(cx, e, error_at) {
                    return r;
                }
            }
            // A leitura antes da declaração (`const x = [x]`): o analyzer
            // liga o nome ao local adiante (`REFERENCED_BEFORE_DECLARATION`),
            // e a constante em ciclo vale o inválido calado.
            None if self.body.units.get(u.0 as usize).and_then(|b| b.declaracao_local(e)).is_some() => {
                if let Some(r) = self.local_value(cx, e, error_at) {
                    return r;
                }
            }
            _ => {}
        }
        // `dynamic`/`Never`/`void` como literais de tipo.
        if let ExprKind::Identifier(n) = &self.ast(u).expr(e).kind {
            match self.interner.resolve(n.sym) {
                "dynamic" if res.is_none() => {
                    return Constante::Valor(Valor::novo(self.core.type_, Estado::Tipo(Some(self.core.dynamic_))));
                }
                "Never" if res.is_none() => {
                    return Constante::Valor(Valor::novo(self.core.type_, Estado::Tipo(Some(self.core.never))));
                }
                _ => {}
            }
        }
        if !self.inferred_(u) {
            let t = self.estatico(u, e);
            return Constante::Valor(self.desconhecido(t));
        }
        if self.invalid_type(u, e) {
            return self.generic(u, error_at, true);
        }
        self.generic(u, error_at, false)
    }

    /// O valor de `E.values`: `List<E>` com o valor de cada constante, na
    /// ordem; uma constante inválida invalida a lista (sem relato próprio).
    fn valores_do_enum(&mut self, u: UnitId, error_at: ExprId, k: ClassId) -> R {
        let n = self.program.class(k).type_params.len();
        let element = self.table.intern(Type::Interface { class: k, args: vec![self.core.dynamic_; n].into_boxed_slice(), nullable: false });
        let element = self.apagar(element);
        let typ = self.interface(self.core.list_class, vec![element]);
        let mut list = Vec::new();
        for v in self.program.class(k).enum_constants.clone() {
            match self.valor_de_variavel(v) {
                Some(Constante::Valor(x)) => list.push(x),
                // Uma constante inválida (num ciclo com o próprio `values`,
                // `e1(values)`): o `values` fica inválido como variável `const`
                // de resultado inválido, calado (`avoidReporting`,
                // `evaluation.dart:1867-1878`).
                Some(Constante::Invalid(_)) => {
                    let mut i = self.erro(u, self.span(u, error_at), c::INVALID_CONSTANT);
                    i.unresolved = true;
                    i.avoid_report = true;
                    return Constante::Invalid(Box::new(i));
                }
                None => return self.generic(u, error_at, true),
            }
        }
        Constante::Valor(self.valor(typ, Estado::Lista { elemento: element, elementos: Rc::new(list), unknown: false }))
    }

    /// O membro estático variável `nome` de `d` (constante de enum ou campo
    /// estático).
    fn static_variable_member(&self, d: ClassId, the_name: SymbolId) -> Option<VariableId> {
        let k = self.program.class(d);
        k.enum_constants
            .iter()
            .chain(k.fields.iter())
            .copied()
            .find(|v| self.program.variable(*v).name == the_name && self.program.variable(*v).static_)
    }

    /// Referência a uma variável de topo ou estática: o valor dela, se é
    /// `const`; senão o erro genérico no nó.
    fn variable_reference_value(&mut self, cx: &Ctx, error_at: ExprId, v: VariableId) -> R {
        let u = cx.unidade;
        let var = self.program.variable(v);
        if var.const_ {
            return match self.valor_de_variavel(v) {
                Some(Constante::Valor(x)) => Constante::Valor(x),
                Some(Constante::Invalid(_)) => {
                    let mut i = self.erro(u, self.span(u, error_at), c::INVALID_CONSTANT);
                    i.unresolved = true;
                    i.avoid_report = true;
                    Constante::Invalid(Box::new(i))
                }
                None => self.generic(u, error_at, true),
            };
        }
        if self.invalid_type(u, error_at) {
            return self.generic(u, error_at, true);
        }
        self.generic(u, error_at, false)
    }

    /// `ElementAnnotation.computeConstantValue` da anotação `m`, escrita na
    /// unidade `u`: a invocação de construtor (`@C(…)`, `@C.n(…)`,
    /// `@p.C(…)`, `@p.C.n(…)`, com os argumentos de tipo que a inferência
    /// registrou para a lista de argumentos) ou a leitura de variável
    /// (`@x`, `@p.x`, `@C.x`). `None` quando não resolve ou não é constante.
    pub fn evaluate_annotation(&mut self, u: UnitId, m: &ast::Annotation) -> Option<Valor> {
        let lib = self.program.unit(u).library;
        let cx = Ctx::simples(u, lib);
        let names: Vec<SymbolId> = m.name.iter().map(|n| n.sym).collect();
        let (tgt, mem) = match names.as_slice() {
            [c] => (self.program.lookup_na_unidade(u, *c).and_then(|b| b.getter), None),
            [a, b] => match self.program.lookup_na_unidade(u, *a).and_then(|x| x.getter) {
                Some(el @ Element::Class(_)) => (Some(el), Some(*b)),
                _ => (self.program.lookup_prefixed_na_unidade(u, *a, *b).and_then(|x| x.getter), None),
            },
            [p, c, n] => (self.program.lookup_prefixed_na_unidade(u, *p, *c).and_then(|x| x.getter), Some(*n)),
            _ => (None, None),
        };
        match (tgt?, &m.arguments) {
            (Element::Class(c), Some(args)) => {
                let the_key = mem.or_else(|| self.interner.lookup(""))?;
                let f = *self.program.class(c).constructors.get(&the_key)?;
                let n = self.program.class(c).type_params.len();
                let args_list: Vec<TypeId> = match self.body.units.get(u.0 as usize).and_then(|b| b.instanciacao(args.span.start)) {
                    Some(v) if v.len() == n => v.to_vec(),
                    _ => vec![self.core.dynamic_; n],
                };
                let typ = self.table.intern(Type::Interface { class: c, args: args_list.into_boxed_slice(), nullable: false });
                let error = ErrorAt { unidade: u, span: m.span };
                match self.evaluate_call(&cx, error, f, typ, &Argumentos::Ast(args), None, true) {
                    Constante::Valor(v) => Some(v),
                    Constante::Invalid(_) => None,
                }
            }
            (Element::Class(c), None) => {
                let v = self.static_variable_member(c, mem?)?;
                match self.valor_de_variavel(v)? {
                    Constante::Valor(x) => Some(x),
                    Constante::Invalid(_) => None,
                }
            }
            (Element::Variable(v), None) => match self.valor_de_variavel(v)? {
                Constante::Valor(x) => Some(x),
                Constante::Invalid(_) => None,
            },
            (Element::Function(g), None) => {
                // A leitura de uma variável de topo pelo getter sintético.
                let v = self.program.function(g).variable?;
                match self.valor_de_variavel(v)? {
                    Constante::Valor(x) => Some(x),
                    Constante::Invalid(_) => None,
                }
            }
            _ => None,
        }
    }

    /// O valor de uma variável com inicializador constante (`const`, ou
    /// `final` de instância numa classe com construtor gerador `const`),
    /// calculado uma vez. `None` enquanto está em cálculo (ciclo).
    /// O `_IsSerializableNodeVisitor` sobre `init`: algum nó da subárvore é
    /// `ForElement`, `FunctionExpression`, `PatternAssignment` ou
    /// `SwitchExpression`. Os nós de uma expressão nascem antes dela, em ids
    /// contíguos e dentro do intervalo dela.
    fn not_serializable(&self, u: UnitId, init: ExprId) -> bool {
        fn has_for(el: &ast::CollectionElement) -> bool {
            match el {
                ast::CollectionElement::For { .. } | ast::CollectionElement::ForIn { .. } => true,
                ast::CollectionElement::If { then, else_, .. } => has_for(then) || else_.as_deref().is_some_and(has_for),
                _ => false,
            }
        }
        let a = self.ast(u);
        let sp = a.expr(init).span;
        let mut i = init.0 as usize + 1;
        while i > 0 {
            i -= 1;
            let x = &a.exprs[i];
            if x.span.start < sp.start || x.span.end > sp.end {
                break;
            }
            let bad = match &x.kind {
                ExprKind::FunctionExpression(_) | ExprKind::PatternAssign { .. } | ExprKind::Switch { .. } => true,
                ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => elements.iter().any(has_for),
                _ => false,
            };
            if bad {
                return true;
            }
        }
        false
    }

    pub fn valor_de_variavel(&mut self, v: VariableId) -> Option<Constante> {
        match self.vars.get(&v) {
            Some(VarState::Ready(r)) => return Some(r.clone()),
            Some(VarState::InProgress) => {
                if let Some(p) = self.vars_stack.iter().position(|x| *x == v) {
                    let cycle: Vec<VariableId> = self.vars_stack[p..].to_vec();
                    self.cyclic.extend(cycle);
                }
                return None;
            }
            None => {}
        }
        // O componente do grafo de dependências a que a variável pertence
        // tem ciclo (`generateCycleError`): o resultado guardado é o
        // inválido no nome dela, e quem a lê recebe um inválido calado.
        if self.grafo.variables_in_cycle.contains(&v) {
            // A constante de enum também: o erro no nome dela.
            let unit_id = match self.program.variable(v).node {
                VariableRef::TopLevel { unit, .. } | VariableRef::Field { unit, .. } | VariableRef::EnumConstant { unit, .. } => Some(unit),
                _ => None,
            };
            if let (Some(unit_id), Some(the_name)) = (unit_id, self.variable_name_span(v)) {
                let r = Constante::Invalid(Box::new(self.erro(unit_id, the_name, c::RECURSIVE_COMPILE_TIME_CONSTANT)));
                self.vars.insert(v, VarState::Ready(r.clone()));
                return Some(r);
            }
        }
        let var = self.program.variable(v);
        let lib = var.library;
        let var_type = self.outline.variables[v.0 as usize].declared_type.or(self.outline.variables[v.0 as usize].inferred);
        let var_type = var_type.unwrap_or(self.core.dynamic_);
        let (unit_id, initial) = match var.node {
            VariableRef::TopLevel { unit, decl, index } => match &self.ast(unit).decl(decl).kind {
                ast::DeclKind::Variables(l) => (unit, l.variables.get(index).and_then(|x| x.initializer)),
                _ => (unit, None),
            },
            VariableRef::Field { unit, member, index } => match &self.ast(unit).member(member).kind {
                ast::MemberKind::Field(l) => (unit, l.variables.get(index).and_then(|x| x.initializer)),
                _ => (unit, None),
            },
            VariableRef::EnumConstant { unit, decl, index } => {
                self.vars.insert(v, VarState::InProgress);
                self.vars_stack.push(v);
                let r = self.enum_value(v, unit, decl, index);
                self.vars_stack.pop();
                self.vars.insert(v, VarState::Ready(r.clone()));
                return Some(r);
            }
            _ => return Some(Constante::Valor(self.desconhecido(var_type))),
        };
        let Some(init) = initial else {
            let r = Constante::Valor(self.desconhecido(var_type));
            return Some(r);
        };
        if !self.the_inferred.contains(&lib) {
            let r = Constante::Valor(self.desconhecido(var_type));
            self.vars.insert(v, VarState::Ready(r.clone()));
            return Some(r);
        }
        // `DetachNodes._detachConstVariable` (3.6.2 `summary2/detach_nodes.dart:103-115`,
        // `replaceNotSerializableNode` em `:18-51`): o inicializador da
        // constante de topo ou de campo que tem `for` de coleção, expressão de
        // função, atribuição de padrão ou `switch` de expressão vira um
        // identificador sintético; o `evaluationResult` é inválido, no
        // inicializador.
        if self.not_serializable(unit_id, init) {
            let r = Constante::Invalid(Box::new(self.erro(unit_id, self.span(unit_id, init), c::INVALID_CONSTANT)));
            self.vars.insert(v, VarState::Ready(r.clone()));
            return Some(r);
        }
        self.vars.insert(v, VarState::InProgress);
        self.vars_stack.push(v);
        let cx = Ctx::simples(unit_id, lib);
        let const_ = self.program.variable(v).const_;
        let saved = std::mem::take(&mut self.creation_args);
        let mut r = self.avaliar(&cx, init, const_);
        self.creation_args = saved;
        self.vars_stack.pop();
        if let Constante::Valor(x) = &r {
            if const_ && !self.casa(x, var_type) {
                let static_ = self.estatico(unit_id, init);
                if self.atribuivel(static_, var_type) {
                    let mut i = self.erro(unit_id, self.span(unit_id, init), c::VARIABLE_TYPE_MISMATCH);
                    i.args = vec![self.formatar(x.tipo), self.formatar(var_type)];
                    r = Constante::Invalid(Box::new(i));
                }
            }
        }
        if let Constante::Valor(x) = &mut r {
            if const_ {
                x.variavel = Some(v);
            }
        }
        if self.cyclic.contains(&v) {
            let the_name = self.variable_name_span(v).unwrap_or(Span { start: 0, end: 0 });
            r = Constante::Invalid(Box::new(self.erro(unit_id, the_name, c::RECURSIVE_COMPILE_TIME_CONSTANT)));
        }
        self.vars.insert(v, VarState::Ready(r.clone()));
        Some(r)
    }

    pub fn variable_name_span(&self, v: VariableId) -> Option<Span> {
        match self.program.variable(v).node {
            VariableRef::TopLevel { unit, decl, index } => match &self.ast(unit).decl(decl).kind {
                ast::DeclKind::Variables(l) => l.variables.get(index).map(|x| x.name.span),
                _ => None,
            },
            VariableRef::Field { unit, member, index } => match &self.ast(unit).member(member).kind {
                ast::MemberKind::Field(l) => l.variables.get(index).map(|x| x.name.span),
                _ => None,
            },
            VariableRef::EnumConstant { unit, decl, index } => match &self.ast(unit).decl(decl).kind {
                ast::DeclKind::Enum(en) => en.constants.get(index).map(|x| x.name.span),
                _ => None,
            },
            _ => None,
        }
    }

    /// Uma constante de enum: o objeto com `index` e `_name`.
    fn enum_value(&mut self, v: VariableId, unit: UnitId, decl: ast::DeclId, index: usize) -> Constante {
        let _ = (unit, decl);
        let Some(k) = self.program.variable(v).class else {
            return Constante::Valor(Valor::novo(self.core.dynamic_, Estado::Generico { campos: Rc::new(Vec::new()), desconhecido: true }));
        };
        // O tipo da constante (a instanciação escrita, `a<int>()`, ou a dos
        // limites), como o `DartObject` do analyzer.
        let vd = &self.outline.variables[v.0 as usize];
        let typ = match vd.declared_type.or(vd.inferred) {
            Some(t) => t,
            None => {
                let n = self.program.class(k).type_params.len();
                self.table.intern(Type::Interface { class: k, args: vec![self.core.dynamic_; n].into_boxed_slice(), nullable: false })
            }
        };
        let the_name = self.interner.resolve(self.program.variable(v).name);
        let mut the_fields = vec![
            (Campo::Indice, Valor::novo(self.core.int, Estado::Int(Some(index as i64)))),
            (Campo::EnumName, Valor::novo(self.core.string, Estado::Str(Some(texto(the_name))))),
        ];
        // O inicializador da constante é a criação `E.nome(args)` com a
        // declaração da constante como nó de erro (`_errorNodes`,
        // `evaluation.dart:39-61`); a exceção de avaliação vira
        // `CONST_EVAL_THROWS_EXCEPTION` nela (`:350-376`).
        let lib = self.program.class(k).library;
        if self.the_inferred.contains(&lib)
            && let ast::DeclKind::Enum(en) = &self.ast(unit).decl(decl).kind
            && let Some(cst) = en.constants.get(index)
        {
            let the_key = match cst.constructor {
                Some(n) if self.interner.resolve(n.sym) != "new" => Some(n.sym),
                _ => self.interner.lookup(""),
            };
            // O primário (3.13) numa biblioteca sem o recurso: o oráculo 3.13.4
            // não o avalia.
            let primary_disabled = |f: FunctionElementId| {
                matches!(self.program.function(f).node, FunctionRef::Constructor { member, .. } if en.primary_constructor == Some(member))
                    && !self.program.library(lib).features.tem(dartforge_frontend::Feature::PrimaryConstructors)
            };
            if let Some(&f) = the_key.and_then(|c| self.program.class(k).constructors.get(&c))
                && !primary_disabled(f)
            {
                let span = Span { start: cst.name.span.start, end: cst.span.end };
                let error = ErrorAt { unidade: unit, span };
                let cx = Ctx::simples(unit, lib);
                let empty = Argumentos::Values { posicionais: Vec::new(), nomeados: Vec::new() };
                let r = match &cst.arguments {
                    Some(a) => self.evaluate_call(&cx, error, f, typ, &Argumentos::Ast(a), None, true),
                    None => self.evaluate_call(&cx, error, f, typ, &empty, None, true),
                };
                match r {
                    Constante::Invalid(i) if i.excecao => {
                        let mut n = self.erro(unit, span, c::CONST_EVAL_THROWS_EXCEPTION);
                        n.avoid_report = i.avoid_report;
                        let msg = dartforge_diagnostics::Diagnostic::com_codigo(i.codigo, i.span, i.args.iter().map(|s| s.as_str())).message;
                        let defining = self.program.library(self.program.unit(unit).library).units.first().copied().unwrap_or(unit);
                        n.contexto = i.contexto.clone();
                        n.contexto.push((defining, i.span, format!("The exception is '{msg}' and occurs here.")));
                        return Constante::Invalid(Box::new(n));
                    }
                    Constante::Invalid(i) => return Constante::Invalid(i),
                    Constante::Valor(x) => {
                        if let Estado::Generico { campos: c2, .. } = &x.estado {
                            for par in c2.iter() {
                                if !matches!(par.0, Campo::Indice | Campo::EnumName) {
                                    the_fields.push(par.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
        Constante::Valor(Valor { tipo: typ, estado: Estado::Generico { campos: Rc::new(the_fields), desconhecido: false }, variavel: Some(v) })
    }

    /// Constante local: a declaração pelo offset do nome.
    fn local_value(&mut self, cx: &Ctx, e: ExprId, error_at: ExprId) -> Option<R> {
        let u = cx.unidade;
        let offset = self.body.units.get(u.0 as usize)?.declaracao_local(e)?;
        if !self.unit_locals.contains_key(&u) {
            let map = constant_locals(self.ast(u));
            self.unit_locals.insert(u, map);
        }
        let Some(&(stmt, i)) = self.unit_locals[&u].get(&offset) else {
            // Local que não é `const`: o erro genérico.
            return None;
        };
        if let Some(r) = self.locais.get(&(u, offset)) {
            return Some(match r {
                Some(Constante::Valor(v)) => Constante::Valor(v.clone()),
                Some(Constante::Invalid(_)) => {
                    let mut x = self.erro(u, self.span(u, error_at), c::INVALID_CONSTANT);
                    x.unresolved = true;
                    x.avoid_report = true;
                    Constante::Invalid(Box::new(x))
                }
                None => self.generic(u, error_at, true),
            });
        }
        let r = self.declared_local_value(u, cx.lib, stmt, i);
        Some(match r {
            Constante::Valor(v) => Constante::Valor(v),
            Constante::Invalid(_) => {
                let mut x = self.erro(u, self.span(u, error_at), c::INVALID_CONSTANT);
                x.unresolved = true;
                x.avoid_report = true;
                Constante::Invalid(Box::new(x))
            }
        })
    }

    /// O valor da constante local `i` da lista em `stmt` (guardado).
    pub fn declared_local_value(&mut self, u: UnitId, lib: LibraryId, stmt: ast::StmtId, i: usize) -> Constante {
        let list = match &self.ast(u).stmt(stmt).kind {
            ast::StmtKind::Variables(l) => l,
            ast::StmtKind::For { init: Some(ast::ForInit::Variables(l)), .. } => l,
            _ => return Constante::Valor(Valor::novo(self.core.dynamic_, Estado::Null { invalid: true })),
        };
        let var = &list.variables[i];
        let offset = var.name.span.start;
        if let Some(Some(r)) = self.locais.get(&(u, offset)) {
            return r.clone();
        }
        if self.grafo.locals_in_cycle.contains(&(u, offset)) {
            let r = Constante::Invalid(Box::new(self.erro(u, var.name.span, c::RECURSIVE_COMPILE_TIME_CONSTANT)));
            self.locais.insert((u, offset), Some(r.clone()));
            return r;
        }
        let Some(init) = var.initializer else {
            return Constante::Valor(Valor::nulo(self.core));
        };
        self.locais.insert((u, offset), None);
        let cx = Ctx::simples(u, lib);
        let saved = std::mem::take(&mut self.creation_args);
        let mut r = self.avaliar(&cx, init, true);
        self.creation_args = saved;
        if let Constante::Valor(x) = &r {
            let typ = self.body.units[u.0 as usize].tipo_local(offset).unwrap_or(self.core.dynamic_);
            if !self.casa(x, typ) {
                let static_ = self.estatico(u, init);
                if self.atribuivel(static_, typ) {
                    let mut i = self.erro(u, self.span(u, init), c::VARIABLE_TYPE_MISMATCH);
                    i.args = vec![self.formatar(x.tipo), self.formatar(typ)];
                    r = Constante::Invalid(Box::new(i));
                }
            }
        }
        self.locais.insert((u, offset), Some(r.clone()));
        r
    }

    /// O local lido por `e` é um parâmetro formal.
    pub fn parameter_local(&self, u: UnitId, e: ExprId) -> bool {
        let Some(offset) = self.body.units.get(u.0 as usize).and_then(|b| b.declaracao_local(e)) else { return false };
        self.parametros.get(&u).is_some_and(|s| s.contains(&offset))
    }

    /// O local lido por `e` é uma constante (`const`).
    pub fn constant_local(&self, u: UnitId, e: ExprId) -> bool {
        let Some(offset) = self.body.units.get(u.0 as usize).and_then(|b| b.declaracao_local(e)) else { return false };
        match self.unit_locals.get(&u) {
            Some(m) => m.contains_key(&offset),
            None => constant_locals(self.ast(u)).contains_key(&offset),
        }
    }

    // -- Potencialmente constante ---------------------------------------------

    /// `_reportNotPotentialConstants`: o primeiro nó que não é
    /// potencialmente constante, como `INVALID_CONSTANT`.
    fn not_potentially_constant(&self, cx: &Ctx, e: ExprId) -> Option<R> {
        let lib_nnbd = true;
        let _ = lib_nnbd;
        let mut nodes = Vec::new();
        super::potential::coletar(self, cx, e, &mut nodes);
        let n = nodes.first()?;
        Some(Constante::Invalid(Box::new(self.erro(cx.unidade, *n, c::INVALID_CONSTANT))))
    }

    // -- Construtores ----------------------------------------------------------

    /// O tipo de uma criação: o estático do nó, senão a classe crua.
    fn creation_type(&mut self, u: UnitId, e: ExprId, f: FunctionElementId) -> TypeId {
        let t = self.estatico(u, e);
        if matches!(self.table.get(t), Type::Interface { .. } | Type::ExtensionType { .. }) {
            return t;
        }
        let Some(k) = self.program.function(f).class else { return self.core.dynamic_ };
        let n = self.program.class(k).type_params.len();
        self.table.intern(Type::Interface { class: k, args: vec![self.core.dynamic_; n].into_boxed_slice(), nullable: false })
    }

    /// `_InstanceCreationEvaluator.evaluate` para uma criação escrita
    /// (`e` é o nó do erro; `palavra`, o `new`/`const` escrito).
    /// O construtor encaminhado de `class B = A with M` (sem elemento no
    /// modelo): o de mesmo nome da primeira superclasse que não é aplicação de
    /// mixin, com o tipo da criação (`B`). A avaliação do encaminhador é a do
    /// construtor da superclasse com os mesmos argumentos.
    fn forwarded_constructor(&self, u: UnitId, e: ExprId, the_name: Option<SymbolId>) -> Option<(FunctionElementId, TypeId)> {
        let typ = self.estatico(u, e);
        let Type::Interface { class, .. } = self.table.get(typ) else { return None };
        if self.program.class(*class).kind != ClassKind::MixinApplication {
            return None;
        }
        // `C.new` e `C()` são o construtor sem nome (o símbolo vazio).
        let empty = self.interner.lookup("");
        let the_key = match the_name {
            Some(n) if self.interner.resolve(n) != "new" => Some(n),
            _ => empty,
        };
        let mut current = self.program.class(*class).supertype_class;
        for _ in 0..64 {
            let k = current?;
            let cls = self.program.class(k);
            if cls.kind != ClassKind::MixinApplication {
                return the_key.and_then(|k| cls.constructors.get(&k).copied()).map(|f| (f, typ));
            }
            current = cls.supertype_class;
        }
        None
    }

    fn call_constructor(
        &mut self,
        cx: &Ctx,
        e: ExprId,
        f: FunctionElementId,
        args: &ast::Arguments,
        the_keyword: Option<Span>,
        typ: Option<TypeId>,
    ) -> R {
        let u = cx.unidade;
        let typ = match typ {
            Some(t) => t,
            None => self.creation_type(u, e, f),
        };
        let error = ErrorAt { unidade: u, span: self.span(u, e) };
        for arg in args.args.iter() {
            if arg.name.is_none() {
                self.creation_args.insert((u, arg.value));
            }
        }
        self.evaluate_call(cx, error, f, typ, &Argumentos::Ast(args), the_keyword, true)
    }

    /// Avalia a chamada do construtor `f` com os argumentos (`ConstantVisitor`
    /// de `cx` para eles), relatando em `erro`.
    fn evaluate_call(
        &mut self,
        cx: &Ctx,
        error: ErrorAt,
        f: FunctionElementId,
        typ: TypeId,
        args: &Argumentos<'_>,
        the_keyword: Option<Span>,
        in_const_args: bool,
    ) -> R {
        // T3: o `const` é o da declaração pública (no SDK, o patch de
        // `bool.fromEnvironment` e dos irmãos é uma fábrica sem `const`).
        let fe = self.program.function(self.program.publico(f));
        if !fe.const_ {
            let span = the_keyword.unwrap_or(error.span);
            return Constante::Invalid(Box::new(self.erro(error.unidade, span, c::CONST_WITH_NON_CONST)));
        }
        if self.constructors_in_progress.contains(&f) || self.constructors_in_progress.len() > 64 {
            return Constante::Valor(self.desconhecido(typ));
        }
        // Argumentos (`_valueOf`: o não resolvido é relatado e vira um
        // objeto fictício do tipo do parâmetro).
        let params = self.outline.functions.get(f.0 as usize).map(|d| d.parameters.clone()).unwrap_or_default();
        let mut positional_args: Vec<Valor> = Vec::new();
        let mut named_args: Vec<(SymbolId, Valor, Option<(UnitId, Span, TypeId)>)> = Vec::new();
        let mut positional_targets: Vec<Option<(UnitId, Span, TypeId)>> = Vec::new();
        match args {
            Argumentos::Ast(a) => {
                let mut i_pos = 0usize;
                let u = cx.unidade;
                for arg in a.args.iter() {
                    let type_param = match arg.name {
                        Some(n) => params.iter().find(|p| p.externo.or(p.name) == Some(n.sym)).map(|p| p.ty),
                        None => params.get(i_pos).map(|p| p.ty),
                    }
                    .unwrap_or(self.core.dynamic_);
                    let v = self.value_of(cx, arg.value, type_param, in_const_args);
                    let v = match v {
                        Constante::Valor(v) => v,
                        i => return i,
                    };
                    let tgt = Some((u, self.span(u, arg.value), self.estatico(u, arg.value)));
                    match arg.name {
                        Some(n) => {
                            let s = Span { start: n.span.start, end: self.span(u, arg.value).end };
                            named_args.push((n.sym, v, Some((u, s, self.estatico(u, arg.value)))));
                        }
                        None => {
                            positional_args.push(v);
                            positional_targets.push(tgt);
                            i_pos += 1;
                        }
                    }
                }
            }
            Argumentos::Values { posicionais: p, nomeados: n } => {
                positional_args = p.clone();
                positional_targets = vec![None; p.len()];
                named_args = n.iter().map(|(s, v)| (*s, v.clone(), None)).collect();
            }
        }
        // Redirecionamentos de factory `const` até um construtor que não é.
        let (f, typ) = self.follow_redirections(f, typ);
        // O construtor não é livre de ciclo (`isCycleFree`,
        // `evaluation.dart:3090-3099`): um desconhecido do tipo, sem erro.
        if self.grafo.constructors_in_cycle.contains(&f) {
            return Constante::Valor(self.desconhecido(typ));
        }
        let fe = self.program.function(f);
        let Some(k) = fe.class else { return Constante::Valor(self.desconhecido(typ)) };
        if fe.factory || !self.the_inferred.contains(&fe.library) || fe.kind == FunctionKind::SyntheticConstructor && self.program.class(k).kind == ClassKind::MixinApplication {
            return self.call_factory(error, f, typ, &positional_args, &named_args);
        }
        self.constructors_in_progress.push(f);
        let r = self.call_generator(error, f, k, typ, positional_args, positional_targets, named_args);
        self.constructors_in_progress.pop();
        r
    }

    /// `_valueOf`.
    fn value_of(&mut self, cx: &Ctx, e: ExprId, default_type: TypeId, in_const: bool) -> R {
        match self.avaliar(cx, e, in_const) {
            Constante::Invalid(i) if i.unresolved => {
                if cx.relatar && !i.avoid_report {
                    self.relatos.push((*i).clone());
                }
                let t = self.apagar(default_type);
                Constante::Valor(Valor::novo(t, Estado::Null { invalid: true }))
            }
            r => r,
        }
    }

    /// `_followConstantRedirectionChain`.
    fn follow_redirections(&mut self, mut f: FunctionElementId, mut typ: TypeId) -> (FunctionElementId, TypeId) {
        let mut seen = vec![f];
        loop {
            let fe = self.program.function(f);
            if !fe.factory {
                break;
            }
            if fe.class.is_some_and(|k| self.interner.resolve(self.program.class(k).name) == "Symbol")
                && Some(fe.library) == self.core.core_library
            {
                break;
            }
            let Some((g, t)) = self.redirection_target(f, typ) else { break };
            if !self.program.function(self.program.publico(g)).const_ || seen.contains(&g) {
                break;
            }
            seen.push(g);
            f = g;
            typ = t;
        }
        (f, typ)
    }

    /// O construtor para o qual a factory `f` redireciona (`= C.nome`), e o
    /// tipo instanciado.
    fn redirection_target(&mut self, f: FunctionElementId, typ: TypeId) -> Option<(FunctionElementId, TypeId)> {
        let FunctionRef::Constructor { unit, member } = self.program.function(f).node else { return None };
        let ast::MemberKind::Constructor(k) = &self.ast(unit).member(member).kind else { return None };
        let r = k.redirect.as_ref()?;
        let ast::TypeKind::Named { name, .. } = &self.ast(unit).ty(r.ty).kind else { return None };
        // `C.nome` sem prefixo de import é a classe `C` e o construtor `nome`
        // (o parser não distingue de um tipo prefixado).
        let mut the_constructor = r.constructor.map(|n| n.sym);
        let b = match &name[..] {
            [n] => self.program.lookup_na_unidade(unit, n.sym),
            [p, n] => match self.program.lookup_prefixed_na_unidade(unit, p.sym, n.sym) {
                Some(b) => Some(b),
                None if the_constructor.is_none() => {
                    the_constructor = Some(n.sym);
                    self.program.lookup_na_unidade(unit, p.sym)
                }
                None => None,
            },
            _ => None,
        }?;
        let Some(Element::Class(tgt)) = b.getter else { return None };
        let the_key = match the_constructor {
            Some(n) => n,
            None => self.interner.lookup("")?,
        };
        let g = *self.program.class(tgt).constructors.get(&the_key)?;
        // Os argumentos de tipo passam pela declaração: o alvo é tratado com
        // os do tipo original quando a classe é a mesma; senão, cru.
        let t = match self.table.get(typ) {
            Type::Interface { class, .. } if *class == tgt => typ,
            _ => {
                let n = self.program.class(tgt).type_params.len();
                self.table.intern(Type::Interface { class: tgt, args: vec![self.core.dynamic_; n].into_boxed_slice(), nullable: false })
            }
        };
        Some((g, t))
    }

    /// `evaluateFactoryConstructorCall`.
    fn call_factory(
        &mut self,
        error: ErrorAt,
        f: FunctionElementId,
        typ: TypeId,
        positional_args: &[Valor],
        named_args: &[(SymbolId, Valor, Option<(UnitId, Span, TypeId)>)],
    ) -> R {
        let fe = self.program.function(f);
        let the_name = fe.name;
        let cls = fe.class;
        let of_core = Some(fe.library) == self.core.core_library;
        let first = positional_args.first();
        let n_args = positional_args.len() + named_args.len();
        let exception = |m: &Motor<'_>| Constante::Invalid(Box::new(m.erro(error.unidade, error.span, c::CONST_EVAL_THROWS_EXCEPTION)));
        if of_core && Some(the_name) == self.sym_from_environment {
            // `_checkFromEnvironmentArguments`.
            let ok = (1..=2).contains(&n_args)
                && positional_args.len() == 1
                && first.is_some_and(|p| p.tipo == self.core.string)
                && (named_args.is_empty()
                    || named_args.len() == 1
                        && Some(named_args[0].0) == self.sym_default_value
                        && (named_args[0].1.tipo == typ || named_args[0].1.tipo == self.core.null));
            if !ok {
                return exception(self);
            }
            return Constante::Valor(self.desconhecido(typ));
        }
        if of_core && Some(the_name) == self.sym_has_environment && cls.is_some() && cls == self.core.bool_class {
            return Constante::Valor(Valor::bool_(self.core, None));
        }
        if of_core
            && self.interner.resolve(the_name).is_empty()
            && cls.is_some_and(|k| self.interner.resolve(self.program.class(k).name) == "Symbol")
            && n_args == 1
        {
            let ok = positional_args.len() == 1 && first.is_some_and(|p| p.tipo == self.core.string && p.como_texto().is_some());
            if !ok {
                return exception(self);
            }
            let s = String::from_utf16_lossy(first.unwrap().como_texto().unwrap());
            return Constante::Valor(Valor::novo(typ, Estado::Simbolo(Some(s.into()))));
        }
        let t = self.apagar(typ);
        Constante::Valor(self.desconhecido(t))
    }

    /// `evaluateGenerativeConstructorCall`.
    #[allow(clippy::too_many_arguments)]
    fn call_generator(
        &mut self,
        error: ErrorAt,
        f: FunctionElementId,
        k: ClassId,
        typ: TypeId,
        positional_args: Vec<Valor>,
        positional_targets: Vec<Option<(UnitId, Span, TypeId)>>,
        named_args: Vec<(SymbolId, Valor, Option<(UnitId, Span, TypeId)>)>,
    ) -> R {
        let lib = self.program.function(f).library;
        // Substituição dos parâmetros de tipo da classe.
        let class_params = self.outline.classes.get(k.0 as usize).map(|d| d.type_params.clone()).unwrap_or_default();
        let type_args: Vec<TypeId> = match self.table.get(typ) {
            Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.to_vec(),
            _ => Vec::new(),
        };
        let mut types_map: HashMap<TypeParamId, TypeId> = HashMap::new();
        if class_params.len() == type_args.len() {
            for (p, a) in class_params.iter().zip(type_args.iter()) {
                types_map.insert(*p, *a);
            }
        }
        let subst = |m: &mut Motor<'_>, t: TypeId| -> TypeId {
            if types_map.is_empty() { t } else { crate::ops::substitute(t, &types_map, m.table) }
        };
        let mut the_fields: Vec<(Campo, Valor)> = Vec::new();
        // `_checkFields`: campos `final` com inicializador na declaração.
        let fields = self.program.class(k).fields.clone();
        for v in fields {
            let var = self.program.variable(v);
            if !(var.final_ || var.const_) || var.static_ || var.late {
                continue;
            }
            let has_init = match var.node {
                VariableRef::Field { unit, member, index } => match &self.ast(unit).member(member).kind {
                    ast::MemberKind::Field(l) => l.variables.get(index).and_then(|x| x.initializer).map(|x| (unit, x)),
                    _ => None,
                },
                _ => None,
            };
            let Some((fu, fx)) = has_init else { continue };
            let Some(Constante::Valor(x)) = self.valor_de_variavel(v) else { continue };
            let declared = self.outline.variables[v.0 as usize].declared_type.or(self.outline.variables[v.0 as usize].inferred).unwrap_or(self.core.dynamic_);
            let field_type = subst(self, declared);
            if !self.casa(&x, field_type) {
                // `hasTypeParameterReference(field.type)`: o tipo declarado do
                // campo, antes da substituição (`final T x` com `C<String>`),
                // faz do descasamento uma exceção da avaliação — relatada como
                // `CONST_EVAL_THROWS_EXCEPTION` na criação, com a mensagem do
                // campo de contexto.
                let exception = self.mentions_parameter(declared);
                let (eu, es) = if fu == error.unidade || true { (fu, self.span(fu, fx)) } else { (error.unidade, error.span) };
                let mut i = self.erro(eu, es, c::CONST_CONSTRUCTOR_FIELD_TYPE_MISMATCH);
                i.args = vec![self.formatar(x.tipo), self.interner.resolve(var.name).to_string(), self.formatar(field_type)];
                i.excecao = exception;
                return Constante::Invalid(Box::new(i));
            }
            the_fields.push((Campo::Nome(var.name), x));
        }
        // `_checkParameters`.
        let FunctionRef::Constructor { unit: cu, member } = self.program.function(f).node else {
            // Construtor sintético: sem parâmetros nem inicializadores.
            let g = self.complete_generator(error, f, k, typ, lib, the_fields, None, Vec::new(), HashMap::new(), types_map);
            return self.finish_generator(g);
        };
        let ast::MemberKind::Constructor(ctor) = &self.ast(cu).member(member).kind else {
            return Constante::Valor(self.desconhecido(typ));
        };
        let type_params_list = self.outline.functions.get(f.0 as usize).map(|d| d.parameters.clone()).unwrap_or_default();
        let mut lexical: HashMap<SymbolId, Valor> = HashMap::new();
        let mut i_pos = 0usize;
        for (i, p) in ctor.parameters.iter().enumerate() {
            let Some(the_name) = p.name else { continue };
            let external = p.nome_externo().map(|n| n.sym).unwrap_or(the_name.sym);
            let type_p = type_params_list.get(i).map(|d| d.ty).unwrap_or(self.core.dynamic_);
            let type_p = subst(self, type_p);
            let (mut the_value, tgt) = if p.kind == ast::ParameterKind::Named {
                match named_args.iter().find(|(n, _, _)| *n == external) {
                    Some((_, v, a)) => (Some(v.clone()), *a),
                    None => (None, None),
                }
            } else {
                let r = (positional_args.get(i_pos).cloned(), positional_targets.get(i_pos).copied().flatten());
                i_pos += 1;
                r
            };
            // Só o opcional (`isOptional`) recebe o padrão; o nomeado
            // `required` ausente fica sem valor.
            if the_value.is_none() && p.kind != ast::ParameterKind::Required && !p.required {
                the_value = match p.default_value {
                    // `super.x` sem padrão: o do parâmetro do construtor super.
                    None if p.super_ => {
                        let idx = ctor.parameters[..i].iter().filter(|q| q.super_ && q.kind != ast::ParameterKind::Named).count();
                        Some(self.inherited_default(f, the_name.sym, p.kind == ast::ParameterKind::Named, idx, 0).unwrap_or_else(|| Valor::nulo(self.core)))
                    }
                    None => Some(Valor::nulo(self.core)),
                    Some(d) => match self.valor_padrao(cu, lib, d) {
                        Constante::Valor(v) => Some(v),
                        Constante::Invalid(_) => None,
                    },
                };
            }
            let Some(the_value) = the_value else { continue };
            let (au, aspan, astatic) = match tgt {
                Some((u, s, t)) => (u, s, Some(t)),
                None => (error.unidade, error.span, None),
            };
            let invalid = matches!(the_value.estado, Estado::Null { invalid: true });
            if !invalid && !self.casa(&the_value, type_p) {
                let exception = astatic.is_some_and(|t| self.atribuivel(t, type_p));
                let mut e = self.erro(au, aspan, c::CONST_CONSTRUCTOR_PARAM_TYPE_MISMATCH);
                e.args = vec![self.format_without_alias(the_value.tipo), self.format_without_alias(type_p)];
                e.excecao = exception;
                return Constante::Invalid(Box::new(e));
            }
            if p.this_ {
                let field = self.program.class(k).fields.iter().copied().find(|v| self.program.variable(*v).name == the_name.sym && !self.program.variable(*v).static_);
                if let Some(field) = field {
                    let field_type = self.outline.variables[field.0 as usize].declared_type.or(self.outline.variables[field.0 as usize].inferred).unwrap_or(self.core.dynamic_);
                    let field_type = subst(self, field_type);
                    if field_type != type_p && !invalid && !self.casa(&the_value, field_type) {
                        let mut e = self.erro(au, aspan, c::CONST_CONSTRUCTOR_PARAM_TYPE_MISMATCH);
                        e.args = vec![self.format_without_alias(the_value.tipo), self.format_without_alias(field_type)];
                        return Constante::Invalid(Box::new(e));
                    }
                    if the_fields.iter().any(|(c, _)| *c == Campo::Nome(the_name.sym)) {
                        return Constante::Invalid(Box::new(self.erro(error.unidade, error.span, c::CONST_EVAL_THROWS_EXCEPTION)));
                    }
                    the_fields.push((Campo::Nome(the_name.sym), the_value.clone()));
                }
            }
            lexical.insert(the_name.sym, the_value);
        }
        // Parâmetros `super.x`: argumentos implícitos da chamada ao super.
        let mut implicit_supers: (Vec<Valor>, Vec<(SymbolId, Valor)>) = (Vec::new(), Vec::new());
        for p in ctor.parameters.iter() {
            if !p.super_ {
                continue;
            }
            let Some(n) = p.name else { continue };
            let Some(v) = lexical.get(&n.sym).cloned() else { continue };
            if p.kind == ast::ParameterKind::Named {
                implicit_supers.1.push((n.sym, v));
            } else {
                implicit_supers.0.push(v);
            }
        }
        let inits: Vec<&ast::Initializer> = ctor.initializers.iter().collect();
        let g = self
            .complete_generator(error, f, k, typ, lib, the_fields, Some((cu, inits)), implicit_supers.0, lexical, types_map)
            .con_super_named(implicit_supers.1);
        self.finish_generator(g)
    }

    /// `SuperFormalParameterElementImpl.evaluationResult` (3.6.2
    /// `element.dart:1750-1761`, usado pelo `_checkParameters`,
    /// `evaluation.dart:2889-2900`): o `super.x` sem padrão próprio vale o
    /// padrão do parâmetro correspondente do construtor super (o nomeado pelo
    /// nome, o posicional pela posição entre os `super.` posicionais), sem
    /// conferir o tipo; em cadeia, se aquele também é `super.` sem padrão.
    /// `None`: sem parâmetro correspondente (o valor fica `null`).
    fn inherited_default(&mut self, f: FunctionElementId, the_name: SymbolId, named_one: bool, idx: usize, prof: u32) -> Option<Valor> {
        if prof > 16 {
            return None;
        }
        let FunctionRef::Constructor { unit, member } = self.program.function(f).node else { return None };
        let k = self.program.function(f).class?;
        let sup = self.program.class(k).supertype_class?;
        let ast::MemberKind::Constructor(ctor) = &self.ast(unit).member(member).kind else { return None };
        let super_name = ctor.initializers.iter().find_map(|i| match i {
            ast::Initializer::Super { constructor, .. } => Some(constructor.map(|n| n.sym)),
            _ => None,
        });
        let the_key = super_name.flatten().or_else(|| self.interner.lookup(""))?;
        let g = *self.program.class(sup).constructors.get(&the_key)?;
        let g = self.program.publico(g);
        let FunctionRef::Constructor { unit: gu, member: gm } = self.program.function(g).node else { return None };
        let ast::MemberKind::Constructor(gc) = &self.ast(gu).member(gm).kind else { return None };
        let tgt = if named_one {
            gc.parameters.iter().position(|q| q.kind == ast::ParameterKind::Named && q.name.is_some_and(|n| n.sym == the_name))
        } else {
            gc.parameters.iter().enumerate().filter(|(_, q)| q.kind != ast::ParameterKind::Named).nth(idx).map(|(j, _)| j)
        }?;
        let q = &gc.parameters[tgt];
        match q.default_value {
            Some(d) => {
                let glib = self.program.unit(gu).library;
                match self.valor_padrao(gu, glib, d) {
                    Constante::Valor(v) => Some(v),
                    Constante::Invalid(_) => None,
                }
            }
            None if q.super_ => {
                let qname = q.name?.sym;
                let qnamed = q.kind == ast::ParameterKind::Named;
                let qindex = gc.parameters[..tgt].iter().filter(|r| r.super_ && r.kind != ast::ParameterKind::Named).count();
                self.inherited_default(g, qname, qnamed, qindex, prof + 1)
            }
            None => Some(Valor::nulo(self.core)),
        }
    }

    /// `_checkInitializers`, `_checkSuperConstructorCall` e o objeto.
    #[allow(clippy::too_many_arguments)]
    fn complete_generator(
        &mut self,
        error: ErrorAt,
        f: FunctionElementId,
        k: ClassId,
        typ: TypeId,
        lib: LibraryId,
        mut the_fields: Vec<(Campo, Valor)>,
        inits: Option<(UnitId, Vec<&ast::Initializer>)>,
        super_positional_args: Vec<Valor>,
        lexical: HashMap<SymbolId, Valor>,
        types_map: HashMap<TypeParamId, TypeId>,
    ) -> Gerador {
        let lexical = Rc::new(lexical);
        let types = Rc::new(types_map);
        let ctor_unit = inits.as_ref().map(|(u, _)| *u).unwrap_or(error.unidade);
        let cxi = Ctx { unidade: ctor_unit, lib, lexical: Some(lexical.clone()), tipos: Some(types.clone()), relatar: false };
        let mut super_name: Option<SymbolId> = None;
        let mut super_args: Option<(UnitId, Vec<ast::Argument>)> = None;
        if let Some((cu, inits)) = inits {
            for init in inits {
                match init {
                    ast::Initializer::Field { name, value, .. } => {
                        let r = self.avaliar(&cxi, *value, false);
                        match r {
                            Constante::Valor(v) => {
                                if the_fields.iter().any(|(c, _)| *c == Campo::Nome(name.sym)) {
                                    return Gerador::ready(Constante::Invalid(Box::new(self.erro(error.unidade, error.span, c::CONST_EVAL_THROWS_EXCEPTION))));
                                }
                                let field = self.program.class(k).fields.iter().copied().find(|x| self.program.variable(*x).name == name.sym && !self.program.variable(*x).static_);
                                if let Some(field) = field {
                                    let tc = self.outline.variables[field.0 as usize].declared_type.or(self.outline.variables[field.0 as usize].inferred).unwrap_or(self.core.dynamic_);
                                    let tc = if types.is_empty() { tc } else { crate::ops::substitute(tc, &types, self.table) };
                                    if !self.casa(&v, tc) {
                                        let static_ = self.estatico(cu, *value);
                                        let exception = self.atribuivel(static_, tc);
                                        let (eu, es) = if exception { (cu, self.span(cu, *value)) } else { (error.unidade, error.span) };
                                        let mut i = self.erro(eu, es, c::CONST_CONSTRUCTOR_FIELD_TYPE_MISMATCH);
                                        i.args = vec![self.formatar(v.tipo), self.interner.resolve(name.sym).to_string(), self.formatar(tc)];
                                        i.excecao = exception;
                                        return Gerador::ready(Constante::Invalid(Box::new(i)));
                                    }
                                }
                                the_fields.push((Campo::Nome(name.sym), v));
                            }
                            Constante::Invalid(i) if !i.excecao => {
                                let n = self.error_in_initializer(&i, Some(f), "field initializer", error);
                                return Gerador::ready(Constante::Invalid(Box::new(n)));
                            }
                            r => return Gerador::ready(r),
                        }
                    }
                    ast::Initializer::Super { constructor, arguments, .. } => {
                        super_name = constructor.map(|n| n.sym);
                        super_args = Some((cu, arguments.args.to_vec()));
                    }
                    ast::Initializer::Redirect { constructor, arguments, .. } => {
                        let the_key = constructor.map(|n| n.sym).or_else(|| self.interner.lookup(""));
                        let tgt = the_key.and_then(|c| self.program.class(k).constructors.get(&c).copied());
                        if let Some(g) = tgt {
                            if self.program.function(self.program.publico(g)).const_ {
                                let r = self.evaluate_call(&cxi, error, g, typ, &Argumentos::Ast(arguments), None, false);
                                return Gerador::ready(r);
                            }
                        }
                    }
                    ast::Initializer::Assert { condition, message, span } => {
                        let r = self.avaliar(&cxi, *condition, false);
                        match r {
                            Constante::Valor(v) => {
                                if !v.estado.e_bool() || v.as_bool() == Some(false) {
                                    let mut i = None;
                                    if let Some(m) = message {
                                        if let Constante::Valor(mv) = self.avaliar(&cxi, *m, false) {
                                            if let Some(t) = mv.como_texto() {
                                                let mut x = self.erro(cu, *span, c::CONST_EVAL_ASSERTION_FAILURE_WITH_MESSAGE);
                                                x.args = vec![String::from_utf16_lossy(t)];
                                                x.excecao = true;
                                                i = Some(x);
                                            }
                                        }
                                    }
                                    let mut i = i.unwrap_or_else(|| self.erro(cu, *span, c::CONST_EVAL_ASSERTION_FAILURE));
                                    i.excecao = true;
                                    return Gerador::ready(Constante::Invalid(Box::new(i)));
                                }
                            }
                            Constante::Invalid(i) if !i.excecao => {
                                let n = self.error_in_initializer(&i, Some(f), "assert initializer", error);
                                return Gerador::ready(Constante::Invalid(Box::new(n)));
                            }
                            r => return Gerador::ready(r),
                        }
                    }
                }
            }
        }
        Gerador {
            ready_engine: None,
            f: Some(f),
            erro: error,
            k,
            tipo: typ,
            campos: the_fields,
            super_name,
            super_args,
            super_positional_args,
            super_named_args: Vec::new(),
            cxi,
        }
    }

    /// `_checkSuperConstructorCall` e o objeto final.
    fn finish_generator(&mut self, g: Gerador) -> R {
        if let Some(r) = g.ready_engine {
            return r;
        }
        let Gerador { f, erro: error, k, tipo: typ, campos: mut the_fields, super_name, super_args, super_positional_args, super_named_args, cxi, .. } = g;
        let cls = self.program.class(k);
        if let Some(sup) = cls.supertype_class {
            if Some(sup) != self.core.object_class {
                let the_key = super_name.or_else(|| self.interner.lookup(""));
                let tgt = the_key.and_then(|c| self.program.class(sup).constructors.get(&c).copied());
                if let Some(g) = tgt {
                    if self.program.function(self.program.publico(g)).const_ {
                        let super_type = self.outline.classes.get(k.0 as usize).and_then(|d| d.supertype).unwrap_or(self.core.dynamic_);
                        let super_type = match cxi.tipos.as_ref() {
                            Some(m) if !m.is_empty() => crate::ops::substitute(super_type, m, self.table),
                            _ => super_type,
                        };
                        let r = match super_args {
                            Some((cu, args)) => {
                                let a = ast::Arguments { span: error.span, type_args: Box::new([]), args: args.into_boxed_slice() };
                                let cx = Ctx { unidade: cu, ..cxi.clone() };
                                if super_positional_args.is_empty() && super_named_args.is_empty() {
                                    self.evaluate_call(&cx, error, g, super_type, &Argumentos::Ast(&a), None, false)
                                } else {
                                    // Argumentos escritos mais os de `super.x`.
                                    let mut pos = super_positional_args.clone();
                                    let mut the_named: Vec<(SymbolId, Valor)> = super_named_args.clone();
                                    for arg in a.args.iter() {
                                        match self.avaliar(&cx, arg.value, false) {
                                            Constante::Valor(v) => match arg.name {
                                                Some(n) => the_named.push((n.sym, v)),
                                                None => pos.push(v),
                                            },
                                            i => return i,
                                        }
                                    }
                                    self.evaluate_call(&cx, error, g, super_type, &Argumentos::Values { posicionais: pos, nomeados: the_named }, None, false)
                                }
                            }
                            None => self.evaluate_call(
                                &cxi,
                                error,
                                g,
                                super_type,
                                &Argumentos::Values { posicionais: super_positional_args, nomeados: super_named_args },
                                None,
                                false,
                            ),
                        };
                        match r {
                            Constante::Valor(v) => the_fields.push((Campo::Super, v)),
                            Constante::Invalid(i) if !i.excecao => {
                                let mut n = if i.contexto.is_empty() {
                                    self.error_in_initializer(&i, f, "super constructor invocation", error)
                                } else {
                                    let mut n = (*i).clone();
                                    n.unidade = error.unidade;
                                    n.span = error.span;
                                    n
                                };
                                if !i.contexto.is_empty() {
                                    if let Some(x) = f.and_then(|f| self.constructor_stack(g, f)) {
                                        n.contexto.push(x);
                                    }
                                }
                                return Constante::Invalid(Box::new(n));
                            }
                            Constante::Invalid(mut i) => {
                                if let Some(x) = f.and_then(|f| self.constructor_stack(g, f)) {
                                    i.contexto.push(x);
                                }
                                return Constante::Invalid(i);
                            }
                        }
                    }
                }
            }
        }
        if cls.kind == ClassKind::ExtensionType {
            if let Some(rep) = cls.representation {
                let the_name = self.program.variable(rep).name;
                if let Some((_, v)) = the_fields.iter().find(|(c, _)| *c == Campo::Nome(the_name)) {
                    return Constante::Valor(v.clone());
                }
            }
        }
        let t = self.apagar(typ);
        Constante::Valor(Valor::novo(t, Estado::Generico { campos: Rc::new(the_fields), desconhecido: false }))
    }

    /// Valor padrão de um parâmetro (guardado pela expressão).
    fn valor_padrao(&mut self, u: UnitId, lib: LibraryId, d: ExprId) -> Constante {
        if let Some(r) = self.padroes.get(&(u, d)) {
            return r.clone();
        }
        let cx = Ctx::simples(u, lib);
        let saved = std::mem::take(&mut self.creation_args);
        let r = self.avaliar(&cx, d, false);
        self.creation_args = saved;
        self.padroes.insert((u, d), r.clone());
        r
    }

    /// O resultado da avaliação do padrão `d` (para o verificador).
    pub fn default_result(&mut self, u: UnitId, lib: LibraryId, d: ExprId) -> Constante {
        self.valor_padrao(u, lib, d)
    }
}

/// Onde um erro de construtor é relatado (`_errorNode`).
#[derive(Clone, Copy, Debug)]
pub struct ErrorAt {
    pub unidade: UnitId,
    pub span: Span,
}

/// Os argumentos de uma chamada de construtor: escritos, ou já avaliados
/// (super implícito, `super.x`).
enum Argumentos<'x> {
    Ast(&'x ast::Arguments),
    Values { posicionais: Vec<Valor>, nomeados: Vec<(SymbolId, Valor)> },
}

/// O estado entre os inicializadores e a chamada ao super.
struct Gerador {
    ready_engine: Option<Constante>,
    /// O construtor avaliado (`_constructor`), para as mensagens de contexto.
    f: Option<FunctionElementId>,
    erro: ErrorAt,
    k: ClassId,
    tipo: TypeId,
    campos: Vec<(Campo, Valor)>,
    super_name: Option<SymbolId>,
    super_args: Option<(UnitId, Vec<ast::Argument>)>,
    super_positional_args: Vec<Valor>,
    super_named_args: Vec<(SymbolId, Valor)>,
    cxi: Ctx,
}

impl Gerador {
    fn ready(r: Constante) -> Gerador {
        Gerador {
            ready_engine: Some(r),
            f: None,
            erro: ErrorAt { unidade: UnitId(0), span: Span { start: 0, end: 0 } },
            k: ClassId(0),
            tipo: TypeId(0),
            campos: Vec::new(),
            super_name: None,
            super_args: None,
            super_positional_args: Vec::new(),
            super_named_args: Vec::new(),
            cxi: Ctx::simples(UnitId(0), LibraryId(0)),
        }
    }

    fn con_super_named(mut self, n: Vec<(SymbolId, Valor)>) -> Gerador {
        self.super_named_args = n;
        self
    }
}

/// Recua de `i` sobre espaços até o byte `b` (inclusive), se ele vier logo antes.
fn back_up_to(source: &[u8], i: usize, b: u8) -> usize {
    let mut j = i;
    while j > 0 && source[j - 1].is_ascii_whitespace() {
        j -= 1;
    }
    if j > 0 && source[j - 1] == b { j - 1 } else { i }
}

/// Recua de `i` sobre espaços até uma das palavras (a primeira que casar).
fn back_up_word(source: &[u8], i: usize, words: &[&str]) -> usize {
    let mut j = i;
    while j > 0 && source[j - 1].is_ascii_whitespace() {
        j -= 1;
    }
    // `if (cond)`: pula o `(`.
    if j > 0 && source[j - 1] == b'(' {
        j -= 1;
        while j > 0 && source[j - 1].is_ascii_whitespace() {
            j -= 1;
        }
    }
    for p in words {
        let p = p.as_bytes();
        if j >= p.len() && &source[j - p.len()..j] == p {
            return j - p.len();
        }
    }
    i
}

/// O início do `for` que envolve a posição `i` (o `for` mais próximo antes).
fn inicio_de_for(source: &[u8], i: usize) -> usize {
    let mut j = i.min(source.len());
    let mut prof = 0i32;
    while j > 0 {
        j -= 1;
        match source[j] {
            b')' => prof += 1,
            b'(' => {
                if prof == 0 {
                    let mut k = j;
                    while k > 0 && source[k - 1].is_ascii_whitespace() {
                        k -= 1;
                    }
                    if k >= 3 && &source[k - 3..k] == b"for" {
                        let mut s = k - 3;
                        // `await for`
                        let mut w = s;
                        while w > 0 && source[w - 1].is_ascii_whitespace() {
                            w -= 1;
                        }
                        if w >= 5 && &source[w - 5..w] == b"await" {
                            s = w - 5;
                        }
                        return s;
                    }
                    return i;
                }
                prof -= 1;
            }
            _ => {}
        }
    }
    i
}

/// As constantes locais de uma unidade: pelo offset do nome, o comando
/// (`Variables` ou `for`) e o índice na lista.
pub(super) fn constant_locals(a: &ast::Ast) -> HashMap<usize, (ast::StmtId, usize)> {
    let mut m = HashMap::new();
    for (i, s) in a.stmts.iter().enumerate() {
        let list = match &s.kind {
            ast::StmtKind::Variables(l) => l,
            ast::StmtKind::For { init: Some(ast::ForInit::Variables(l)), .. } => l,
            _ => continue,
        };
        if !list.const_ {
            continue;
        }
        for (j, v) in list.variables.iter().enumerate() {
            m.insert(v.name.span.start, (ast::StmtId(i as u32), j));
        }
    }
    m
}

/// Os offsets dos nomes de todos os parâmetros formais de uma unidade
/// (funções, construtores, tipos de função e parâmetros-função).
fn parameter_offsets(a: &ast::Ast) -> HashSet<usize> {
    fn params(ps: &[ast::Parameter], s: &mut HashSet<usize>) {
        for p in ps {
            if let Some(n) = p.name {
                s.insert(n.span.start);
            }
            if let Some(f) = &p.function_parameters {
                params(f, s);
            }
        }
    }
    let mut s = HashSet::new();
    for f in a.functions.iter() {
        if let Some(ps) = &f.parameters {
            params(ps, &mut s);
        }
    }
    for m in a.members.iter() {
        if let ast::MemberKind::Constructor(k) = &m.kind {
            params(&k.parameters, &mut s);
        }
    }
    s
}
