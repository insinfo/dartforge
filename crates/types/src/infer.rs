//! Motor principal de inferência de tipos e resolução de corpos.
//!
//! Percorre todo o [`Program`] — corpos de funções, métodos, getters, setters,
//! construtores e inicializadores de variáveis/campos — preenchendo as tabelas
//! laterais [`BodyTypes`] e emitindo diagnósticos compatíveis com o `analyzer`.

use crate::codes::*;
use crate::constraints::ConstraintSolver;
use crate::flow::FlowState;
use crate::ops::{non_nullable, substitute};
use crate::resolved::{BodyTypes, LocalId, MemberRef, Resolved, UnitBodyTypes};
use crate::scope::{MemberResolver, ScopeStack};
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeTable};
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::{
    Element, FunctionRef, LibraryId, Program, UnitId, VariableRef,
};
use dartforge_frontend::ast::{self, AssignOp, BinaryOp, ExprId, ExprKind, FunctionBody, StmtId, StmtKind, UnaryOp};
use dartforge_intern::{Interner, SymbolId};
use std::collections::HashMap;

/// Contexto de inferência de corpos para o programa inteiro.
pub struct BodyInferrer<'a> {
    pub program: &'a Program,
    pub interner: &'a Interner,
    pub table: &'a mut TypeTable,
    pub core: &'a CoreTypes,
    pub outline: &'a mut crate::resolve::OutlineTypes,
    pub diagnostics: Vec<Diagnostic>,
    pub body_types: BodyTypes,
    /// Rastreio de variáveis declaradas para o join de análise de fluxo: `LocalId -> TypeId`
    pub local_declared_types: HashMap<LocalId, TypeId>,
}

impl<'a> BodyInferrer<'a> {
    pub fn new(
        program: &'a Program,
        interner: &'a Interner,
        table: &'a mut TypeTable,
        core: &'a CoreTypes,
        outline: &'a mut crate::resolve::OutlineTypes,
    ) -> Self {
        let num_units = program.units.len();
        let mut units_body_types = Vec::with_capacity(num_units);
        for u in &program.units {
            // Unidades do SDK não recebem inferência de corpos: tabela vazia
            // (`get_type`/`get_resolved` devolvem `None`, `set_*` ignoram).
            if program.library(u.library).is_sdk {
                units_body_types.push(UnitBodyTypes::default());
            } else {
                units_body_types.push(UnitBodyTypes::new(u.ast.exprs.len(), core.dynamic_));
            }
        }

        Self {
            program,
            interner,
            table,
            core,
            outline,
            diagnostics: Vec::new(),
            body_types: BodyTypes {
                units: units_body_types,
            },
            local_declared_types: HashMap::new(),
        }
    }

    /// Ponto de entrada: infere corpos de todas as variáveis, funções e construtores.
    pub fn infer_all(mut self) -> (BodyTypes, Vec<Diagnostic>) {
        // 1. Inferir inicializadores de variáveis de topo e campos de classes
        self.infer_variable_initializers();

        // 2. Inferir corpos de todas as funções, métodos e construtores
        self.infer_functions();

        (self.body_types, self.diagnostics)
    }

    fn lub(&mut self, a: TypeId, b: TypeId) -> TypeId {
        let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
        crate::ops::lub(a, b, &mut env)
    }

    fn is_nullable(&mut self, ty: TypeId) -> bool {
        let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
        is_subtype(self.core.null, ty, &mut env)
    }

    fn infer_variable_initializers(&mut self) {
        for i in 0..self.program.variables.len() {
            let var_elem = &self.program.variables[i];
            let declared_ty = self.outline.variables[i].declared_type;

            // SDK: só o outline importa para tipar código do usuário. Uma
            // variável sem tipo escrito (`const json = JsonCodec()`) ainda
            // precisa do inicializador para ter tipo; as tipadas não.
            let is_sdk = self.program.library(var_elem.library).is_sdk;
            if is_sdk && declared_ty.is_some() {
                continue;
            }
            let diags_antes = self.diagnostics.len();

            match var_elem.node {
                VariableRef::TopLevel { unit, decl, index } => {
                    let decl_node = &self.program.unit(unit).ast.decls[decl.0 as usize];
                    if let ast::DeclKind::Variables(var_list) = &decl_node.kind {
                        if let Some(var_decl) = var_list.variables.get(index) {
                            if let Some(init_expr) = var_decl.initializer {
                                let mut scope = ScopeStack::new(None, None, true);
                                let mut flow = FlowState::new_reachable();
                                let init_ty = self.infer_expr(
                                    unit,
                                    init_expr,
                                    declared_ty,
                                    &mut scope,
                                    &mut flow,
                                );

                                if let Some(target_ty) = declared_ty {
                                    let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
                                    if !is_subtype(init_ty, target_ty, &mut env) {
                                        let expr_node = &self.program.unit(unit).ast.exprs[init_expr.0 as usize];
                                        self.diagnostics.push(Diagnostic::new(
                                            format!(
                                                "{}: inicializador com tipo '{}' não compatível com '{}'",
                                                INVALID_ASSIGNMENT.template,
                                                self.table.format(init_ty, self.interner, self.program),
                                                self.table.format(target_ty, self.interner, self.program),
                                            ),
                                            expr_node.span,
                                        ));
                                    }
                                } else {
                                    // Variável sem tipo anotado: infere a partir do inicializador
                                    self.outline.variables[i].inferred = Some(init_ty);
                                }
                            }
                        }
                    }
                }
                VariableRef::Field { unit, member, index } => {
                    let mem_node = &self.program.unit(unit).ast.members[member.0 as usize];
                    if let ast::MemberKind::Field(var_list) = &mem_node.kind {
                        if let Some(var_decl) = var_list.variables.get(index) {
                            if let Some(init_expr) = var_decl.initializer {
                                let mut scope = ScopeStack::new(var_elem.class, None, var_elem.static_);
                                let mut flow = FlowState::new_reachable();
                                let init_ty = self.infer_expr(
                                    unit,
                                    init_expr,
                                    declared_ty,
                                    &mut scope,
                                    &mut flow,
                                );

                                if let Some(target_ty) = declared_ty {
                                    let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
                                    if !is_subtype(init_ty, target_ty, &mut env) {
                                        let expr_node = &self.program.unit(unit).ast.exprs[init_expr.0 as usize];
                                        self.diagnostics.push(Diagnostic::new(
                                            format!(
                                                "{}: campo com tipo inicializador '{}' incompatível com '{}'",
                                                INVALID_ASSIGNMENT.template,
                                                self.table.format(init_ty, self.interner, self.program),
                                                self.table.format(target_ty, self.interner, self.program),
                                            ),
                                            expr_node.span,
                                        ));
                                    }
                                } else {
                                    self.outline.variables[i].inferred = Some(init_ty);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
            // Avisos em inicializadores do SDK são lacunas nossas, não do usuário.
            if is_sdk {
                self.diagnostics.truncate(diags_antes);
            }
        }
    }

    fn infer_functions(&mut self) {
        for i in 0..self.program.functions.len() {
            let func_elem = &self.program.functions[i];
            // Corpos do SDK não são emitidos nem consultados: pular poupa a
            // maior parte da fase (docs/EMISSAO-DDC.md: `dart:*` vem do
            // `dart_sdk.js`). `UnitBodyTypes` dessas unidades fica no fallback.
            if self.program.library(func_elem.library).is_sdk {
                continue;
            }
            let func_data = self.outline.functions[i].clone();

            match func_elem.node {
                FunctionRef::Function { unit, function } => {
                    let ast_func = &self.program.unit(unit).ast.functions[function.0 as usize];
                    let mut scope = ScopeStack::new(
                        func_elem.class,
                        func_elem.extension,
                        func_elem.static_,
                    );

                    // Adicionar parâmetros de tipo da função genérica
                    for &pid in func_data.type_params.iter() {
                        let sym = self.table.param(pid).name;
                        scope.add_function_type_param(sym, pid);
                    }

                    let mut flow = FlowState::new_reachable();

                    // Adicionar parâmetros formais no escopo e no fluxo
                    for (idx, p_data) in func_data.parameters.iter().enumerate() {
                        if let Some(name) = p_data.name {
                            let is_param_final = ast_func
                                .parameters
                                .as_ref()
                                .and_then(|params| params.get(idx))
                                .map_or(false, |p| p.final_);
                            scope.add_parameter(idx as u32, name, p_data.ty, is_param_final);
                            let local_id = scope.declare_local(
                                name,
                                p_data.ty,
                                is_param_final,
                                false,
                                false,
                                0,
                            );
                            self.local_declared_types.insert(local_id, p_data.ty);
                            flow.declare(local_id, true);
                        }
                    }

                    // Inferir corpo da função
                    match &ast_func.body {
                        FunctionBody::Expression(expr_id) => {
                            let expr_ty = self.infer_expr(
                                unit,
                                *expr_id,
                                Some(func_data.return_type),
                                &mut scope,
                                &mut flow,
                            );
                            if func_data.return_type != self.core.void_ && func_data.return_type != self.core.dynamic_ {
                                let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
                                if !is_subtype(expr_ty, func_data.return_type, &mut env) {
                                    let expr_node = &self.program.unit(unit).ast.exprs[expr_id.0 as usize];
                                    self.diagnostics.push(Diagnostic::new(
                                        format!(
                                            "{}: retorno de expressão '{}' incompatível com '{}'",
                                            RETURN_OF_INVALID_TYPE.template,
                                            self.table.format(expr_ty, self.interner, self.program),
                                            self.table.format(func_data.return_type, self.interner, self.program),
                                        ),
                                        expr_node.span,
                                    ));
                                }
                            }
                        }
                        FunctionBody::Block(stmt_id) => {
                            self.infer_stmt(unit, *stmt_id, func_data.return_type, &mut scope, &mut flow);
                        }
                        _ => {}
                    }
                }
                FunctionRef::Constructor { unit, member } => {
                    let mem_node = &self.program.unit(unit).ast.members[member.0 as usize];
                    if let ast::MemberKind::Constructor(ctor) = &mem_node.kind {
                        let mut scope = ScopeStack::new(func_elem.class, None, false);
                        let mut flow = FlowState::new_reachable();

                        for (idx, p_data) in func_data.parameters.iter().enumerate() {
                            if let Some(name) = p_data.name {
                                let is_param_final = ctor.parameters.get(idx).map_or(false, |p| p.final_);
                                scope.add_parameter(idx as u32, name, p_data.ty, is_param_final);
                                let local_id = scope.declare_local(
                                    name,
                                    p_data.ty,
                                    is_param_final,
                                    false,
                                    false,
                                    0,
                                );
                                self.local_declared_types.insert(local_id, p_data.ty);
                                flow.declare(local_id, true);
                            }
                        }

                        // Inicializadores do construtor
                        for init in &ctor.initializers {
                            match init {
                                ast::Initializer::Field { value, .. } => {
                                    self.infer_expr(unit, *value, None, &mut scope, &mut flow);
                                }
                                ast::Initializer::Super { arguments, .. } => {
                                    for arg in &arguments.args {
                                        self.infer_expr(unit, arg.value, None, &mut scope, &mut flow);
                                    }
                                }
                                ast::Initializer::Assert { condition, message, .. } => {
                                    self.infer_expr(unit, *condition, Some(self.core.bool_), &mut scope, &mut flow);
                                    if let Some(msg) = message {
                                        self.infer_expr(unit, *msg, None, &mut scope, &mut flow);
                                    }
                                }
                                ast::Initializer::Redirect { arguments, .. } => {
                                    for arg in &arguments.args {
                                        self.infer_expr(unit, arg.value, None, &mut scope, &mut flow);
                                    }
                                }
                            }
                        }

                        // Corpo do construtor
                        match &ctor.body {
                            FunctionBody::Block(stmt_id) => {
                                self.infer_stmt(unit, *stmt_id, self.core.void_, &mut scope, &mut flow);
                            }
                            FunctionBody::Expression(expr_id) => {
                                self.infer_expr(unit, *expr_id, None, &mut scope, &mut flow);
                            }
                            _ => {}
                        }
                    }
                }
                FunctionRef::None => {}
            }
        }
    }

    /// Infere uma expressão sintática dentro de uma unidade de compilação.
    pub fn infer_expr(
        &mut self,
        unit: UnitId,
        expr_id: ExprId,
        context_type: Option<TypeId>,
        scope: &mut ScopeStack,
        flow: &mut FlowState,
    ) -> TypeId {
        let expr = &self.program.unit(unit).ast.exprs[expr_id.0 as usize];
        let current_library = self.program.unit(unit).library;

        let ty = match &expr.kind {
            // 1. Literais numéricos: adaptação por contexto (int -> double)
            ExprKind::Int(_) => {
                if let Some(ctx) = context_type {
                    if ctx == self.core.num || (self.core.num_class.is_some() && ctx == self.table.intern(Type::Interface {
                        class: self.core.num_class.unwrap(),
                        args: Box::new([]),
                        nullable: false,
                    })) {
                        self.core.int
                    } else if ctx == self.core.dynamic_ || ctx == self.core.object || ctx == self.core.object_nullable {
                        self.core.int
                    } else {
                        // Verifica se o contexto espera double
                        let is_double_ctx = {
                            let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
                            is_subtype(self.core.int, ctx, &mut env)
                        };
                        if is_double_ctx {
                            self.core.int
                        } else {
                            // double x = 1; vira double
                            self.core.int
                        }
                    }
                } else {
                    self.core.int
                }
            }
            ExprKind::Double(_) => self.core.num, // double subtipa num
            ExprKind::Bool(b) => {
                let _ = b;
                self.core.bool_
            }
            ExprKind::Null => self.core.null,
            ExprKind::String(lit) => {
                for part in &lit.parts {
                    if let ast::StringPart::Interpolation(interp_expr) = part {
                        self.infer_expr(unit, *interp_expr, None, scope, flow);
                    }
                }
                self.core.string
            }
            ExprKind::Symbol(_) => {
                self.core.object
            }
            // 2. Identificadores não qualificados
            ExprKind::Identifier(name) => {
                if let Some(resolved) = scope.resolve_identifier(
                    name.sym,
                    expr.span,
                    current_library,
                    self.program,
                    &self.outline.hierarchy,
                    self.interner,
                    &mut self.diagnostics,
                ) {
                    self.body_types.units[unit.0 as usize].set_resolved(expr_id, resolved.clone());

                    match resolved {
                        Resolved::Local(local_id) => {
                            if !flow.is_assigned(local_id) {
                                if let Some(local_info) = scope.get_local(local_id) {
                                    if !local_info.is_late {
                                        self.diagnostics.push(Diagnostic::new(
                                            format!("{}: '{}'", DEFINITELY_UNASSIGNED_VARIABLE.template, self.interner.resolve(local_info.name)),
                                            expr.span,
                                        ));
                                    }
                                }
                            }
                            let decl_ty = self.local_declared_types.get(&local_id).copied().unwrap_or(self.core.dynamic_);
                            flow.get_effective_type(local_id, decl_ty)
                        }
                        Resolved::Parameter { index, .. } => {
                            if (index as usize) < scope.parameters.len() {
                                scope.parameters[index as usize].ty
                            } else {
                                self.core.dynamic_
                            }
                        }
                        Resolved::TypeParameter(pid) => {
                            self.table.intern(Type::TypeParameter {
                                param: pid,
                                nullable: false,
                            })
                        }
                        Resolved::Element(elem) => match elem {
                            Element::Class(cid) => {
                                self.table.intern(Type::Interface {
                                    class: cid,
                                    args: Box::new([]),
                                    nullable: false,
                                })
                            }
                            Element::Function(fid) => {
                                self.outline.functions[fid.0 as usize].signature
                            }
                            Element::Variable(vid) => {
                                let vdata = &self.outline.variables[vid.0 as usize];
                                vdata.declared_type.or(vdata.inferred).unwrap_or(self.core.dynamic_)
                            }
                            Element::Typedef(tid) => {
                                self.outline.typedefs[tid.0 as usize].target_type
                            }
                            _ => self.core.dynamic_,
                        },
                        Resolved::Member { class: _, member, via_super: _ } => {
                            match member {
                                MemberRef::Function(fid) => self.outline.functions[fid.0 as usize].signature,
                                MemberRef::Variable(vid) => {
                                    let vdata = &self.outline.variables[vid.0 as usize];
                                    vdata.declared_type.or(vdata.inferred).unwrap_or(self.core.dynamic_)
                                }
                            }
                        }
                        Resolved::Dynamic => self.core.dynamic_,
                        _ => self.core.dynamic_,
                    }
                } else {
                    self.diagnostics.push(Diagnostic::new(
                        format!("{}: '{}'", UNDEFINED_IDENTIFIER.template, self.interner.resolve(name.sym)),
                        expr.span,
                    ));
                    self.core.dynamic_
                }
            }
            ExprKind::This => {
                if let Some(cid) = scope.enclosing_class {
                    let formals = self.outline.classes[cid.0 as usize].type_params.clone();
                    let args: Vec<TypeId> = formals
                        .iter()
                        .map(|&p| self.table.intern(Type::TypeParameter { param: p, nullable: false }))
                        .collect();
                    self.table.intern(Type::Interface {
                        class: cid,
                        args: args.into_boxed_slice(),
                        nullable: false,
                    })
                } else {
                    self.core.dynamic_
                }
            }
            ExprKind::Super => {
                if let Some(cid) = scope.enclosing_class {
                    if let Some(super_ty) = self.outline.classes[cid.0 as usize].supertype {
                        super_ty
                    } else {
                        self.core.object
                    }
                } else {
                    self.core.dynamic_
                }
            }
            ExprKind::Parenthesized(inner) => {
                self.infer_expr(unit, *inner, context_type, scope, flow)
            }
            // 3. Literais de Lista: <T>[...] ou [...]
            ExprKind::List { type_args, elements, .. } => {
                let elem_ty = if let Some(first_arg) = type_args.first() {
                    let scope_map = HashMap::new();
                    self.resolve_ast_type_annotation(unit, *first_arg, current_library, &scope_map)
                } else {
                    // Inferência de tipo do elemento a partir do contexto ou LUB dos elementos
                    let elem_ctx = context_type.and_then(|ctx| {
                        if let Type::Interface { class, args, .. } = self.table.get(ctx) {
                            if self.core.list_class == Some(*class) || self.core.iterable_class == Some(*class) {
                                return args.first().copied();
                            }
                        }
                        None
                    });

                    let mut inferred_elems = Vec::new();
                    for el in elements {
                        match el {
                            ast::CollectionElement::Expression(e) => {
                                inferred_elems.push(self.infer_expr(unit, *e, elem_ctx, scope, flow));
                            }
                            ast::CollectionElement::NullAwareExpression(e) => {
                                let t = self.infer_expr(unit, *e, elem_ctx, scope, flow);
                                inferred_elems.push(non_nullable(t, self.table));
                            }
                            _ => {}
                        }
                    }

                    if let Some(ctx_elem) = elem_ctx {
                        ctx_elem
                    } else if !inferred_elems.is_empty() {
                        let mut cur_lub = inferred_elems[0];
                        for &t in &inferred_elems[1..] {
                            cur_lub = self.lub(cur_lub, t);
                        }
                        cur_lub
                    } else {
                        self.core.dynamic_
                    }
                };

                if let Some(list_cls) = self.core.list_class {
                    self.table.intern(Type::Interface {
                        class: list_cls,
                        args: Box::new([elem_ty]),
                        nullable: false,
                    })
                } else {
                    self.core.object
                }
            }
            // 4. Literais de Set ou Map
            ExprKind::SetOrMap { const_, type_args, elements, .. } => {
                let is_map = if type_args.len() == 2 {
                    true
                } else if type_args.len() == 1 {
                    false
                } else {
                    elements.iter().any(|el| matches!(el, ast::CollectionElement::MapEntry { .. }))
                };

                let res_ty = if is_map {
                    let (k_ctx, v_ctx) = context_type
                        .and_then(|ctx| {
                            if let Type::Interface { class, args, .. } = self.table.get(ctx) {
                                if self.core.map_class == Some(*class) && args.len() == 2 {
                                    return Some((Some(args[0]), Some(args[1])));
                                }
                            }
                            None
                        })
                        .unwrap_or((None, None));

                    let mut k_types = Vec::new();
                    let mut v_types = Vec::new();

                    for el in elements {
                        if let ast::CollectionElement::MapEntry { key, value, .. } = el {
                            k_types.push(self.infer_expr(unit, *key, k_ctx, scope, flow));
                            v_types.push(self.infer_expr(unit, *value, v_ctx, scope, flow));
                        }
                    }

                    let k_ty = if let Some(k) = k_ctx {
                        k
                    } else if !k_types.is_empty() {
                        let mut cur = k_types[0];
                        for &t in &k_types[1..] {
                            cur = self.lub(cur, t);
                        }
                        cur
                    } else {
                        self.core.dynamic_
                    };

                    let v_ty = if let Some(v) = v_ctx {
                        v
                    } else if !v_types.is_empty() {
                        let mut cur = v_types[0];
                        for &t in &v_types[1..] {
                            cur = self.lub(cur, t);
                        }
                        cur
                    } else {
                        self.core.dynamic_
                    };

                    if let Some(map_cls) = self.core.map_class {
                        self.table.intern(Type::Interface {
                            class: map_cls,
                            args: Box::new([k_ty, v_ty]),
                            nullable: false,
                        })
                    } else {
                        self.core.object
                    }
                } else {
                    // Set
                    let mut elem_types = Vec::new();
                    for el in elements {
                        if let ast::CollectionElement::Expression(e) = el {
                            elem_types.push(self.infer_expr(unit, *e, None, scope, flow));
                        }
                    }
                    let elem_ty = if !elem_types.is_empty() {
                        let mut cur = elem_types[0];
                        for &t in &elem_types[1..] {
                            cur = self.lub(cur, t);
                        }
                        cur
                    } else {
                        self.core.dynamic_
                    };

                    if let Some(iter_cls) = self.core.iterable_class {
                        self.table.intern(Type::Interface {
                            class: iter_cls,
                            args: Box::new([elem_ty]),
                            nullable: false,
                        })
                    } else {
                        self.core.object
                    }
                };

                if *const_ {
                    self.validate_const_collection(unit, expr_id);
                }

                res_ty
            }
            // 5. Registros (Records)
            ExprKind::Record { positional, named, .. } => {
                let mut pos_types = Vec::with_capacity(positional.len());
                for &e in positional {
                    pos_types.push(self.infer_expr(unit, e, None, scope, flow));
                }
                let mut named_types = Vec::with_capacity(named.len());
                for (name, e) in named {
                    named_types.push((name.sym, self.infer_expr(unit, *e, None, scope, flow)));
                }
                self.table.intern(Type::Record {
                    positional: pos_types.into_boxed_slice(),
                    named: named_types.into_boxed_slice(),
                    nullable: false,
                })
            }
            // 6. Acesso a propriedades e membros: target.name
            ExprKind::Property { target, name, null_aware } => {
                let target_ty = self.infer_expr(unit, *target, None, scope, flow);
                let is_setter = false;

                if *null_aware && !self.is_nullable(target_ty) {
                    self.diagnostics.push(Diagnostic::new(
                        INVALID_NULL_AWARE_OPERATOR.template.to_string(),
                        expr.span,
                    ));
                }

                let mut member_res = MemberResolver::new(
                    self.program,
                    self.outline,
                    self.interner,
                    &self.outline.hierarchy,
                    self.table,
                    self.core,
                );

                if let Some((resolved, member_ty)) = member_res.lookup_member(target_ty, name.sym, is_setter, current_library) {
                    self.body_types.units[unit.0 as usize].set_resolved(expr_id, resolved);
                    if *null_aware {
                        crate::ops::nullable(member_ty, self.table)
                    } else {
                        member_ty
                    }
                } else {
                    if target_ty != self.core.dynamic_ {
                        self.diagnostics.push(Diagnostic::new(
                            format!(
                                "{}: getter '{}' não definido para o tipo '{}'",
                                UNDEFINED_GETTER.template,
                                self.interner.resolve(name.sym),
                                self.table.format(target_ty, self.interner, self.program),
                            ),
                            expr.span,
                        ));
                    }
                    self.core.dynamic_
                }
            }
            // 7. Chamadas de função / método
            ExprKind::Call { target, arguments } => {
                let target_expr = &self.program.unit(unit).ast.exprs[target.0 as usize];
                let target_ty = if let ExprKind::Property { target: recv, name, null_aware } = &target_expr.kind {
                    let recv_ty = self.infer_expr(unit, *recv, None, scope, flow);
                    let mut member_res = MemberResolver::new(self.program, self.outline, self.interner, &self.outline.hierarchy, self.table, self.core);
                    if let Some((resolved, member_ty)) = member_res.lookup_member(recv_ty, name.sym, false, current_library) {
                        self.body_types.units[unit.0 as usize].set_resolved(*target, resolved);
                        if *null_aware {
                            if !self.is_nullable(recv_ty) {
                                self.diagnostics.push(Diagnostic::new(
                                    INVALID_NULL_AWARE_OPERATOR.template.to_string(),
                                    target_expr.span,
                                ));
                            }
                            crate::ops::nullable(member_ty, self.table)
                        } else {
                            member_ty
                        }
                    } else {
                        if recv_ty != self.core.dynamic_ {
                            self.diagnostics.push(Diagnostic::new(
                                format!("{}: '{}' para o tipo '{}'", UNDEFINED_METHOD.template, self.interner.resolve(name.sym), self.table.format(recv_ty, self.interner, self.program)),
                                target_expr.span,
                            ));
                        }
                        self.core.dynamic_
                    }
                } else {
                    self.infer_expr(unit, *target, None, scope, flow)
                };

                if target_ty == self.core.dynamic_ {
                    for arg in &arguments.args {
                        self.infer_expr(unit, arg.value, None, scope, flow);
                    }
                    self.core.dynamic_
                } else if let Type::Function {
                    type_params,
                    ret,
                    positional,
                    optional,
                    named,
                    ..
                } = self.table.get(target_ty).clone()
                {
                    // 1. Verificar contagem de argumentos posicionais
                    let num_pos_args = arguments.args.iter().filter(|a| a.name.is_none()).count();
                    let min_pos = positional.len();
                    let max_pos = positional.len() + optional.len();
                    if num_pos_args < min_pos {
                        self.diagnostics.push(Diagnostic::new(
                            format!("{}: esperava pelo menos {}, recebeu {}",
                                NOT_ENOUGH_POSITIONAL_ARGUMENTS.template,
                                min_pos,
                                num_pos_args
                            ),
                            arguments.span,
                        ));
                    } else if num_pos_args > max_pos {
                        self.diagnostics.push(Diagnostic::new(
                            format!("{}: esperava no máximo {}, recebeu {}",
                                EXTRA_POSITIONAL_ARGUMENTS.template,
                                max_pos,
                                num_pos_args
                            ),
                            arguments.span,
                        ));
                    }

                    // 2. Inferir tipos dos argumentos e validar subtipagem
                    let mut arg_types = Vec::with_capacity(arguments.args.len());
                    let mut pos_idx = 0;
                    for arg in &arguments.args {
                        if arg.name.is_none() {
                            let expected = if pos_idx < positional.len() {
                                Some(positional[pos_idx])
                            } else if pos_idx < max_pos {
                                Some(optional[pos_idx - positional.len()])
                            } else {
                                None
                            };
                            let actual_ty = self.infer_expr(unit, arg.value, expected, scope, flow);
                            arg_types.push(actual_ty);
                            if let Some(exp) = expected {
                                let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
                                if !is_subtype(actual_ty, exp, &mut env) {
                                    self.diagnostics.push(Diagnostic::new(
                                        format!(
                                            "{}: argumento '{}' incompatível com parâmetro '{}'",
                                            ARGUMENT_TYPE_NOT_ASSIGNABLE.template,
                                            self.table.format(actual_ty, self.interner, self.program),
                                            self.table.format(exp, self.interner, self.program),
                                        ),
                                        self.program.unit(unit).ast.exprs[arg.value.0 as usize].span,
                                    ));
                                }
                            }
                            pos_idx += 1;
                        } else if let Some(n) = &arg.name {
                            if let Some((_, exp, _)) = named.iter().find(|(name_sym, _, _)| *name_sym == n.sym) {
                                let actual_ty = self.infer_expr(unit, arg.value, Some(*exp), scope, flow);
                                arg_types.push(actual_ty);
                                let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
                                if !is_subtype(actual_ty, *exp, &mut env) {
                                    self.diagnostics.push(Diagnostic::new(
                                        format!(
                                            "{}: argumento '{}' incompatível com parâmetro '{}'",
                                            ARGUMENT_TYPE_NOT_ASSIGNABLE.template,
                                            self.table.format(actual_ty, self.interner, self.program),
                                            self.table.format(*exp, self.interner, self.program),
                                        ),
                                        self.program.unit(unit).ast.exprs[arg.value.0 as usize].span,
                                    ));
                                }
                            } else {
                                let actual_ty = self.infer_expr(unit, arg.value, None, scope, flow);
                                arg_types.push(actual_ty);
                                self.diagnostics.push(Diagnostic::new(
                                    format!("{}: '{}'", UNDEFINED_NAMED_PARAMETER.template, self.interner.resolve(n.sym)),
                                    n.span,
                                ));
                            }
                        }
                    }

                    // 3. Validar required named parameters
                    for (name_sym, _, req) in named.iter() {
                        if *req && !arguments.args.iter().any(|a| a.name.as_ref().map(|n| n.sym) == Some(*name_sym)) {
                            self.diagnostics.push(Diagnostic::new(
                                format!("{}: '{}'", MISSING_REQUIRED_ARGUMENT.template, self.interner.resolve(*name_sym)),
                                arguments.span,
                            ));
                        }
                    }

                    // 4. Parâmetros de tipo genéricos
                    if !type_params.is_empty() && arguments.type_args.is_empty() {
                        let mut solver = ConstraintSolver::new(self.table, &self.outline.hierarchy, self.core);
                        let inferred_targs = solver.infer_type_arguments(
                            &type_params,
                            &positional,
                            &arg_types,
                            ret,
                            context_type,
                        );

                        let mut mapping = HashMap::new();
                        for (&tp, &ta) in type_params.iter().zip(inferred_targs.iter()) {
                            mapping.insert(tp, ta);
                        }
                        substitute(ret, &mapping, self.table)
                    } else {
                        ret
                    }
                } else {
                    for arg in &arguments.args {
                        self.infer_expr(unit, arg.value, None, scope, flow);
                    }
                    self.core.dynamic_
                }
            }
            // 8. Operadores unários
            ExprKind::Unary { op, operand } => {
                let operand_ty = self.infer_expr(unit, *operand, None, scope, flow);
                match op {
                    UnaryOp::NullAssert => {
                        if !self.is_nullable(operand_ty) {
                            self.diagnostics.push(Diagnostic::new(
                                INVALID_NULL_AWARE_OPERATOR.template.to_string(),
                                expr.span,
                            ));
                        }
                        non_nullable(operand_ty, self.table)
                    }
                    UnaryOp::Not => {
                        let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
                        if !is_subtype(operand_ty, self.core.bool_, &mut env) {
                            self.diagnostics.push(Diagnostic::new(
                                NON_BOOL_NEGATION_EXPRESSION.template.to_string(),
                                expr.span,
                            ));
                        }
                        self.core.bool_
                    }
                    UnaryOp::Neg | UnaryOp::BitNot => {
                        operand_ty
                    }
                    UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec => {
                        operand_ty
                    }
                }
            }
            // 9. Operadores binários
            ExprKind::Binary { op, left, right } => {
                let left_ty = self.infer_expr(unit, *left, None, scope, flow);
                let right_ty = self.infer_expr(unit, *right, None, scope, flow);

                match op {
                    BinaryOp::Eq | BinaryOp::NotEq => self.core.bool_,
                    BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq => self.core.bool_,
                    BinaryOp::And | BinaryOp::Or => self.core.bool_,
                    BinaryOp::IfNull => {
                        // ?? produz LUB(non_nullable(left), right)
                        let left_non_null = non_nullable(left_ty, self.table);
                        self.lub(left_non_null, right_ty)
                    }
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul => {
                        if left_ty == self.core.int {
                            let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
                            if !is_subtype(right_ty, self.core.num, &mut env) {
                                self.diagnostics.push(Diagnostic::new(
                                    format!("{}: operador de int espera num, recebeu {}",
                                        ARGUMENT_TYPE_NOT_ASSIGNABLE.template,
                                        self.table.format(right_ty, self.interner, self.program)
                                    ),
                                    self.program.unit(unit).ast.exprs[right.0 as usize].span,
                                ));
                            }
                            if right_ty == self.core.int {
                                self.core.int
                            } else {
                                self.core.num
                            }
                        } else if left_ty == self.core.string {
                            self.core.string
                        } else {
                            self.core.num
                        }
                    }
                    BinaryOp::Div => self.core.num,
                    BinaryOp::TruncDiv => self.core.int,
                    BinaryOp::Rem => left_ty,
                    BinaryOp::Shl | BinaryOp::Shr | BinaryOp::UShr | BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
                        self.core.int
                    }
                }
            }
            // 10. Expressões condicionais: cond ? then : else
            ExprKind::Conditional { condition, then, else_ } => {
                let cond_ty = self.infer_expr(unit, *condition, Some(self.core.bool_), scope, flow);
                let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
                if !is_subtype(cond_ty, self.core.bool_, &mut env) {
                    self.diagnostics.push(Diagnostic::new(
                        NON_BOOL_CONDITION.template.to_string(),
                        self.program.unit(unit).ast.exprs[condition.0 as usize].span,
                    ));
                }
                let then_ty = self.infer_expr(unit, *then, context_type, scope, flow);
                let else_ty = self.infer_expr(unit, *else_, context_type, scope, flow);
                self.lub(then_ty, else_ty)
            }
            // 11. Teste de tipo: e is T / e is! T
            ExprKind::Is { value, ty, .. } => {
                let val_ty = self.infer_expr(unit, *value, None, scope, flow);
                let target_ty = self.resolve_ast_type_annotation(unit, *ty, current_library, &HashMap::new());

                let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
                if is_subtype(val_ty, target_ty, &mut env) && target_ty == self.core.object {
                    self.diagnostics.push(Diagnostic::new(
                        UNNECESSARY_TYPE_CHECK_TRUE.template.to_string(),
                        expr.span,
                    ));
                }

                // Se o valor for uma variável local, promove
                if let ast::ExprKind::Identifier(name) = &self.program.unit(unit).ast.exprs[value.0 as usize].kind {
                    let mut dummy_diags = Vec::new();
                    if let Some(Resolved::Local(local_id)) = scope.resolve_identifier(name.sym, expr.span, current_library, self.program, &self.outline.hierarchy, self.interner, &mut dummy_diags) {
                        let decl_ty = self.local_declared_types.get(&local_id).copied().unwrap_or(val_ty);
                        flow.promote(local_id, target_ty, decl_ty, self.table, &self.outline.hierarchy, self.core);
                    }
                }

                self.core.bool_
            }
            // 12. Cast explícito: e as T
            ExprKind::As { value, ty } => {
                let val_ty = self.infer_expr(unit, *value, None, scope, flow);
                let target_ty = self.resolve_ast_type_annotation(unit, *ty, current_library, &HashMap::new());

                let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
                if is_subtype(val_ty, target_ty, &mut env) {
                    self.diagnostics.push(Diagnostic::new(
                        UNNECESSARY_CAST.template.to_string(),
                        expr.span,
                    ));
                }

                target_ty
            }
            // 13. Atribuição simples e composta: target = value
            ExprKind::Assign { op, target, value } => {
                let target_node = &self.program.unit(unit).ast.exprs[target.0 as usize];
                let target_span = target_node.span;

                let (target_ty, is_valid_lhs) = match &target_node.kind {
                    ast::ExprKind::Identifier(name) => {
                        if let Some(res) = scope.resolve_identifier(
                            name.sym,
                            target_span,
                            current_library,
                            self.program,
                            &self.outline.hierarchy,
                            self.interner,
                            &mut self.diagnostics,
                        ) {
                            self.body_types.units[unit.0 as usize].set_resolved(*target, res.clone());
                            let t = match &res {
                                Resolved::Local(local_id) => {
                                    if let Some(info) = scope.get_local(*local_id) {
                                        if *op != AssignOp::Assign && !flow.is_assigned(*local_id) && !info.is_late {
                                            self.diagnostics.push(Diagnostic::new(
                                                format!("{}: '{}'", DEFINITELY_UNASSIGNED_VARIABLE.template, self.interner.resolve(info.name)),
                                                target_span,
                                            ));
                                        }
                                        if (info.is_final || info.is_const) && flow.is_assigned(*local_id) {
                                            self.diagnostics.push(Diagnostic::new(
                                                format!("{}: '{}'", ASSIGNMENT_TO_FINAL_LOCAL.template, self.interner.resolve(info.name)),
                                                expr.span,
                                            ));
                                        }
                                    }
                                    self.local_declared_types.get(local_id).copied().unwrap_or(self.core.dynamic_)
                                }
                                Resolved::Parameter { index, name: p_name } => {
                                    if let Some(param) = scope.get_param(*p_name) {
                                        if param.is_final {
                                            self.diagnostics.push(Diagnostic::new(
                                                format!("{}: '{}'", ASSIGNMENT_TO_FINAL_LOCAL.template, self.interner.resolve(param.name)),
                                                expr.span,
                                            ));
                                        }
                                    }
                                    scope.parameters.get(*index as usize).map(|p| p.ty).unwrap_or(self.core.dynamic_)
                                }
                                Resolved::Member { class: _, member, via_super: _ } => {
                                    match member {
                                        MemberRef::Variable(vid) => {
                                            let v_elem = &self.program.variables[vid.0 as usize];
                                            if v_elem.final_ || v_elem.const_ {
                                                self.diagnostics.push(Diagnostic::new(
                                                    format!("{}: '{}'", ASSIGNMENT_TO_FINAL_LOCAL.template, self.interner.resolve(v_elem.name)),
                                                    expr.span,
                                                ));
                                            }
                                            let vdata = &self.outline.variables[vid.0 as usize];
                                            vdata.declared_type.or(vdata.inferred).unwrap_or(self.core.dynamic_)
                                        }
                                        _ => self.core.dynamic_,
                                    }
                                }
                                Resolved::Element(elem) => {
                                    match elem {
                                        Element::Variable(vid) => {
                                            let v_elem = &self.program.variables[vid.0 as usize];
                                            if v_elem.final_ || v_elem.const_ {
                                                self.diagnostics.push(Diagnostic::new(
                                                    format!("{}: '{}'", ASSIGNMENT_TO_FINAL_LOCAL.template, self.interner.resolve(v_elem.name)),
                                                    expr.span,
                                                ));
                                            }
                                            let vdata = &self.outline.variables[vid.0 as usize];
                                            vdata.declared_type.or(vdata.inferred).unwrap_or(self.core.dynamic_)
                                        }
                                        _ => self.core.dynamic_,
                                    }
                                }
                                _ => self.core.dynamic_,
                            };
                            (t, true)
                        } else {
                            self.diagnostics.push(Diagnostic::new(
                                format!("{}: '{}'", UNDEFINED_IDENTIFIER.template, self.interner.resolve(name.sym)),
                                target_span,
                            ));
                            (self.core.dynamic_, false)
                        }
                    }
                    ast::ExprKind::Property { target: recv, name, null_aware } => {
                        let recv_ty = self.infer_expr(unit, *recv, None, scope, flow);
                        if *null_aware && !self.is_nullable(recv_ty) {
                            self.diagnostics.push(Diagnostic::new(
                                INVALID_NULL_AWARE_OPERATOR.template.to_string(),
                                target_span,
                            ));
                        }
                        let mut member_res = MemberResolver::new(
                            self.program,
                            self.outline,
                            self.interner,
                            &self.outline.hierarchy,
                            self.table,
                            self.core,
                        );
                        if let Some((resolved, setter_val_ty)) = member_res.lookup_member(recv_ty, name.sym, true, current_library) {
                            self.body_types.units[unit.0 as usize].set_resolved(*target, resolved);
                            (setter_val_ty, true)
                        } else {
                            if recv_ty != self.core.dynamic_ {
                                self.diagnostics.push(Diagnostic::new(
                                    format!(
                                        "{}: setter '{}' não definido para o tipo '{}'",
                                        UNDEFINED_SETTER.template,
                                        self.interner.resolve(name.sym),
                                        self.table.format(recv_ty, self.interner, self.program),
                                    ),
                                    target_span,
                                ));
                            }
                            (self.core.dynamic_, false)
                        }
                    }
                    ast::ExprKind::Index { target: recv, index, .. } => {
                        let recv_ty = self.infer_expr(unit, *recv, None, scope, flow);
                        let _ = self.infer_expr(unit, *index, None, scope, flow);
                        let elem_ty = match self.table.get(recv_ty) {
                            Type::Interface { class, args, .. } if Some(*class) == self.core.list_class && !args.is_empty() => {
                                args[0]
                            }
                            Type::Interface { class, args, .. } if Some(*class) == self.core.map_class && args.len() == 2 => {
                                args[1]
                            }
                            _ => self.core.dynamic_,
                        };
                        (elem_ty, true)
                    }
                    _ => {
                        let t = self.infer_expr(unit, *target, None, scope, flow);
                        (t, true)
                    }
                };

                self.body_types.units[unit.0 as usize].set_type(*target, target_ty);
                let val_ty = self.infer_expr(unit, *value, Some(target_ty), scope, flow);

                if let ast::ExprKind::Identifier(name) = &target_node.kind {
                    let mut dummy_diags = Vec::new();
                    if let Some(Resolved::Local(local_id)) = scope.resolve_identifier(
                        name.sym,
                        target_span,
                        current_library,
                        self.program,
                        &self.outline.hierarchy,
                        self.interner,
                        &mut dummy_diags,
                    ) {
                        let decl_ty = self.local_declared_types.get(&local_id).copied().unwrap_or(target_ty);
                        flow.assign(local_id, val_ty, decl_ty, self.table, &self.outline.hierarchy, self.core);
                    }
                }

                if *op == AssignOp::Assign && is_valid_lhs && target_ty != self.core.dynamic_ {
                    let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
                    if !is_subtype(val_ty, target_ty, &mut env) {
                        self.diagnostics.push(Diagnostic::new(
                            format!(
                                "{}: não é possível atribuir '{}' a '{}'",
                                INVALID_ASSIGNMENT.template,
                                self.table.format(val_ty, self.interner, self.program),
                                self.table.format(target_ty, self.interner, self.program),
                            ),
                            expr.span,
                        ));
                    }
                }

                val_ty
            }
            // 14. Await: await e
            ExprKind::Await(inner) => {
                let inner_ty = self.infer_expr(unit, *inner, None, scope, flow);
                match self.table.get(inner_ty).clone() {
                    Type::FutureOr { arg, .. } => arg,
                    Type::Interface { class, args, .. } if Some(class) == self.core.future_class && !args.is_empty() => {
                        args[0]
                    }
                    _ => inner_ty,
                }
            }
            // 15. Throw e Rethrow: Never
            ExprKind::Throw(inner) => {
                self.infer_expr(unit, *inner, None, scope, flow);
                flow.terminate();
                self.core.never
            }
            ExprKind::Rethrow => {
                flow.terminate();
                self.core.never
            }
            // 16. Instanciação: new C(args) ou const C(args)
            ExprKind::InstanceCreation { ty, constructor, arguments, .. } => {
                let class_ty = self.resolve_ast_type_annotation(unit, *ty, current_library, &HashMap::new());

                if let Type::Interface { class, .. } = self.table.get(class_ty).clone() {
                    let ctor_sym = constructor.as_ref().map(|n| n.sym).or_else(|| self.interner.lookup(""));
                    if let Some(ctor_sym) = ctor_sym {
                        if let Some(&func_elem_id) = self.program.classes[class.0 as usize].constructors.get(&ctor_sym) {
                            self.body_types.units[unit.0 as usize].set_resolved(expr_id, Resolved::Constructor(func_elem_id));
                            let expected_types: Vec<_> = self.outline.functions[func_elem_id.0 as usize]
                                .parameters
                                .iter()
                                .map(|p| p.ty)
                                .collect();
                            for (i, arg) in arguments.args.iter().enumerate() {
                                let expected = expected_types.get(i).copied();
                                self.infer_expr(unit, arg.value, expected, scope, flow);
                            }
                            return class_ty;
                        }
                    }
                }

                for arg in &arguments.args {
                    self.infer_expr(unit, arg.value, None, scope, flow);
                }

                class_ty
            }
            // 17. Closures / Funções Anônimas
            ExprKind::FunctionExpression(func_id) => {
                let ast_func = &self.program.unit(unit).ast.functions[func_id.0 as usize];
                let mut closure_scope = ScopeStack::new(scope.enclosing_class, scope.enclosing_extension, false);

                let mut param_types = Vec::new();
                if let Some(params) = &ast_func.parameters {
                    for (idx, p) in params.iter().enumerate() {
                        let p_ty = if let Some(t) = p.ty {
                            self.resolve_ast_type_annotation(unit, t, current_library, &HashMap::new())
                        } else {
                            self.core.dynamic_
                        };
                        param_types.push(p_ty);
                        if let Some(name) = &p.name {
                            closure_scope.add_parameter(idx as u32, name.sym, p_ty, p.final_);
                            let local_id = closure_scope.declare_local(name.sym, p_ty, p.final_, false, false, 0);
                            self.local_declared_types.insert(local_id, p_ty);
                            flow.declare(local_id, true);
                        }
                    }
                }

                let ret_ty = if let Some(ret) = ast_func.return_type {
                    self.resolve_ast_type_annotation(unit, ret, current_library, &HashMap::new())
                } else {
                    self.core.dynamic_
                };

                self.table.intern(Type::Function {
                    type_params: Box::new([]),
                    ret: ret_ty,
                    positional: param_types.into_boxed_slice(),
                    optional: Box::new([]),
                    named: Box::new([]),
                    nullable: false,
                })
            }
            // 18. Indexação: e[i]
            ExprKind::Index { target, index, .. } => {
                let target_ty = self.infer_expr(unit, *target, None, scope, flow);
                let _ = self.infer_expr(unit, *index, None, scope, flow);

                match self.table.get(target_ty) {
                    Type::Interface { class, args, .. } if Some(*class) == self.core.list_class && !args.is_empty() => {
                        args[0]
                    }
                    Type::Interface { class, args, .. } if Some(*class) == self.core.map_class && args.len() == 2 => {
                        crate::ops::nullable(args[1], self.table)
                    }
                    _ => self.core.dynamic_,
                }
            }
            // 19. Cascatas: e..m()
            ExprKind::Cascade { target, sections, .. } => {
                let target_ty = self.infer_expr(unit, *target, None, scope, flow);
                for &sec in sections {
                    self.infer_expr(unit, sec, None, scope, flow);
                }
                target_ty
            }
            ExprKind::CascadeTarget => {
                context_type.unwrap_or(self.core.dynamic_)
            }
            _ => self.core.dynamic_,
        };

        self.body_types.units[unit.0 as usize].set_type(expr_id, ty);
        ty
    }

    /// Infere um statement sintático dentro de um corpo.
    pub fn infer_stmt(
        &mut self,
        unit: UnitId,
        stmt_id: StmtId,
        return_context: TypeId,
        scope: &mut ScopeStack,
        flow: &mut FlowState,
    ) {
        let stmt = &self.program.unit(unit).ast.stmts[stmt_id.0 as usize];
        let current_library = self.program.unit(unit).library;

        if flow.is_terminated() {
            self.diagnostics.push(Diagnostic::new(
                DEAD_CODE.template.to_string(),
                stmt.span,
            ));
            return;
        }

        match &stmt.kind {
            StmtKind::Block(stmts) => {
                scope.push_block();
                for &s in stmts {
                    let s_node = &self.program.unit(unit).ast.stmts[s.0 as usize];
                    if let StmtKind::Variables(var_list) = &s_node.kind {
                        let decl_ty = var_list.ty.map(|t| {
                            self.resolve_ast_type_annotation(unit, t, current_library, &HashMap::new())
                        }).unwrap_or(self.core.dynamic_);
                        for var_decl in &var_list.variables {
                            let local_id = scope.declare_local(
                                var_decl.name.sym,
                                decl_ty,
                                var_list.final_,
                                var_list.const_,
                                var_list.late,
                                var_decl.name.span.start,
                            );
                            self.local_declared_types.insert(local_id, decl_ty);
                            flow.declare(local_id, false);
                        }
                    }
                }

                for &s in stmts {
                    self.infer_stmt(unit, s, return_context, scope, flow);
                }
                scope.pop_block();
            }
            StmtKind::Variables(var_list) => {
                let declared_ty = var_list.ty.map(|t| {
                    self.resolve_ast_type_annotation(unit, t, current_library, &HashMap::new())
                });

                for var_decl in &var_list.variables {
                    let mut init_ty = declared_ty;
                    if let Some(init_expr) = var_decl.initializer {
                        let expr_ty = self.infer_expr(unit, init_expr, declared_ty, scope, flow);
                        if declared_ty.is_none() {
                            init_ty = Some(expr_ty);
                        }

                        if var_list.const_ {
                            self.validate_const_collection(unit, init_expr);
                            let mut evaluator = crate::constant::ConstantEvaluator::new(self.program, self.interner, self.table, self.core);
                            if evaluator.evaluate_expr(unit, init_expr).is_none() {
                                if let Some(err) = &evaluator.error_thrown {
                                    self.diagnostics.push(Diagnostic::new(
                                        format!("{}: {}", CONST_EVAL_THROWS_EXCEPTION.template, err),
                                        self.program.unit(unit).ast.exprs[init_expr.0 as usize].span,
                                    ));
                                } else {
                                    self.diagnostics.push(Diagnostic::new(
                                        CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE.template.to_string(),
                                        self.program.unit(unit).ast.exprs[init_expr.0 as usize].span,
                                    ));
                                }
                            }
                        }
                    }

                    let effective_ty = init_ty.unwrap_or(self.core.dynamic_);
                    let local_id = if let Some(local_info) = scope.find_local_in_current_block(var_decl.name.sym) {
                        local_info.declared_type = effective_ty;
                        local_info.id
                    } else {
                        scope.declare_local(
                            var_decl.name.sym,
                            effective_ty,
                            var_list.final_,
                            var_list.const_,
                            var_list.late,
                            var_decl.name.span.start,
                        )
                    };
                    self.local_declared_types.insert(local_id, effective_ty);
                    flow.declare(local_id, var_decl.initializer.is_some() || var_list.late);
                }
            }
            StmtKind::Expression(expr_id) => {
                self.infer_expr(unit, *expr_id, None, scope, flow);
            }
            StmtKind::If { condition, then, else_, .. } => {
                let cond_ty = self.infer_expr(unit, *condition, Some(self.core.bool_), scope, flow);
                let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
                if !is_subtype(cond_ty, self.core.bool_, &mut env) {
                    self.diagnostics.push(Diagnostic::new(
                        NON_BOOL_CONDITION.template.to_string(),
                        self.program.unit(unit).ast.exprs[condition.0 as usize].span,
                    ));
                }

                let mut then_flow = flow.clone();
                self.infer_stmt(unit, *then, return_context, scope, &mut then_flow);

                let mut else_flow = flow.clone();
                if let Some(else_stmt) = else_ {
                    self.infer_stmt(unit, *else_stmt, return_context, scope, &mut else_flow);
                }

                *flow = FlowState::join(
                    &then_flow,
                    &else_flow,
                    &self.local_declared_types,
                    self.table,
                    &self.outline.hierarchy,
                    self.core,
                );
            }
            StmtKind::While { condition, body } => {
                let cond_ty = self.infer_expr(unit, *condition, Some(self.core.bool_), scope, flow);
                let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
                if !is_subtype(cond_ty, self.core.bool_, &mut env) {
                    self.diagnostics.push(Diagnostic::new(
                        NON_BOOL_CONDITION.template.to_string(),
                        self.program.unit(unit).ast.exprs[condition.0 as usize].span,
                    ));
                }
                self.infer_stmt(unit, *body, return_context, scope, flow);
            }
            StmtKind::DoWhile { body, condition } => {
                self.infer_stmt(unit, *body, return_context, scope, flow);
                let cond_ty = self.infer_expr(unit, *condition, Some(self.core.bool_), scope, flow);
                let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
                if !is_subtype(cond_ty, self.core.bool_, &mut env) {
                    self.diagnostics.push(Diagnostic::new(
                        NON_BOOL_CONDITION.template.to_string(),
                        self.program.unit(unit).ast.exprs[condition.0 as usize].span,
                    ));
                }
            }
            StmtKind::For { init, condition, updates, body, .. } => {
                scope.push_block();
                if let Some(for_init) = init {
                    match for_init {
                        ast::ForInit::Variables(vlist) => {
                            let declared_ty = vlist.ty.map(|t| {
                                self.resolve_ast_type_annotation(unit, t, current_library, &HashMap::new())
                            });

                            for var_decl in &vlist.variables {
                                let mut init_ty = declared_ty;
                                if let Some(init_expr) = var_decl.initializer {
                                    let expr_ty = self.infer_expr(unit, init_expr, declared_ty, scope, flow);
                                    if declared_ty.is_none() {
                                        init_ty = Some(expr_ty);
                                    }
                                }

                                let effective_ty = init_ty.unwrap_or(self.core.dynamic_);
                                let local_id = scope.declare_local(
                                    var_decl.name.sym,
                                    effective_ty,
                                    vlist.final_,
                                    vlist.const_,
                                    vlist.late,
                                    var_decl.name.span.start,
                                );
                                self.local_declared_types.insert(local_id, effective_ty);
                                flow.declare(local_id, var_decl.initializer.is_some() || vlist.late);
                            }
                        }
                        ast::ForInit::Expression(e) => {
                            self.infer_expr(unit, *e, None, scope, flow);
                        }
                    }
                }
                if let Some(cond) = condition {
                    self.infer_expr(unit, *cond, Some(self.core.bool_), scope, flow);
                }
                self.infer_stmt(unit, *body, return_context, scope, flow);
                for &up in updates {
                    self.infer_expr(unit, up, None, scope, flow);
                }
                scope.pop_block();
            }
            StmtKind::Return(expr_opt) => {
                if let Some(ret_expr) = expr_opt {
                    let expr_ty = self.infer_expr(unit, *ret_expr, Some(return_context), scope, flow);
                    if return_context != self.core.void_ && return_context != self.core.dynamic_ {
                        let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
                        if !is_subtype(expr_ty, return_context, &mut env) {
                            self.diagnostics.push(Diagnostic::new(
                                format!(
                                    "{}: retorno '{}' incompatível com retorno declarado '{}'",
                                    RETURN_OF_INVALID_TYPE.template,
                                    self.table.format(expr_ty, self.interner, self.program),
                                    self.table.format(return_context, self.interner, self.program),
                                ),
                                stmt.span,
                            ));
                        }
                    }
                }
                flow.terminate();
            }
            StmtKind::Try { body, catches, finally_ } => {
                self.infer_stmt(unit, *body, return_context, scope, flow);
                for c in catches {
                    scope.push_block();
                    if let Some(ex_name) = &c.exception {
                        let local_id = scope.declare_local(ex_name.sym, self.core.object, false, false, false, c.span.start);
                        self.local_declared_types.insert(local_id, self.core.object);
                        flow.declare(local_id, true);
                    }
                    self.infer_stmt(unit, c.body, return_context, scope, flow);
                    scope.pop_block();
                }
                if let Some(fin) = finally_ {
                    self.infer_stmt(unit, *fin, return_context, scope, flow);
                }
            }
            StmtKind::Assert { condition, message } => {
                self.infer_expr(unit, *condition, Some(self.core.bool_), scope, flow);
                if let Some(msg) = message {
                    self.infer_expr(unit, *msg, None, scope, flow);
                }
            }
            _ => {}
        }
    }

    fn resolve_ast_type_annotation(
        &mut self,
        unit: UnitId,
        ast_id: ast::TypeId,
        library: LibraryId,
        scope: &HashMap<SymbolId, TypeParamId>,
    ) -> TypeId {
        let node = &self.program.unit(unit).ast.types[ast_id.0 as usize];
        let is_nullable = node.nullable;

        match &node.kind {
            ast::TypeKind::Void => self.core.void_,
            ast::TypeKind::Named { name, args } => {
                if name.len() == 1 {
                    let sym = name[0].sym;
                    if let Some(&pid) = scope.get(&sym) {
                        return self.table.intern(Type::TypeParameter {
                            param: pid,
                            nullable: is_nullable,
                        });
                    }

                    let str_val = self.interner.resolve(sym);
                    match str_val {
                        "dynamic" => return self.core.dynamic_,
                        "void" => return self.core.void_,
                        "Never" => return if is_nullable { self.core.null } else { self.core.never },
                        "Null" => return self.core.null,
                        _ => {}
                    }

                    if let Some(binding) = self.program.lookup(library, sym) {
                        if let Some(Element::Class(cid)) = binding.getter {
                            let resolved_args: Vec<TypeId> = args
                                .iter()
                                .map(|&a| self.resolve_ast_type_annotation(unit, a, library, scope))
                                .collect();

                            return self.table.intern(Type::Interface {
                                class: cid,
                                args: resolved_args.into_boxed_slice(),
                                nullable: is_nullable,
                            });
                        }
                    }
                }
                self.core.dynamic_
            }
            _ => self.core.dynamic_,
        }
    }

    fn validate_const_collection(&mut self, unit: UnitId, expr_id: ExprId) {
        let expr = &self.program.unit(unit).ast.exprs[expr_id.0 as usize];
        let mut sub_exprs = Vec::new();

        {
            let mut evaluator = crate::constant::ConstantEvaluator::new(self.program, self.interner, self.table, self.core);
            match &expr.kind {
                ExprKind::SetOrMap { elements, .. } => {
                    let is_map = elements.iter().any(|el| matches!(el, ast::CollectionElement::MapEntry { .. }));
                    if is_map {
                        let mut seen_keys = Vec::new();
                        for el in elements {
                            if let ast::CollectionElement::MapEntry { key, value, .. } = el {
                                if let Some(k_val) = evaluator.evaluate_expr(unit, *key) {
                                    if seen_keys.contains(&k_val) {
                                        self.diagnostics.push(Diagnostic::new(
                                            EQUAL_KEYS_IN_CONST_MAP.template.to_string(),
                                            self.program.unit(unit).ast.exprs[key.0 as usize].span,
                                        ));
                                    } else {
                                        seen_keys.push(k_val);
                                    }
                                } else {
                                    self.diagnostics.push(Diagnostic::new(
                                        CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE.template.to_string(),
                                        self.program.unit(unit).ast.exprs[key.0 as usize].span,
                                    ));
                                }
                                if evaluator.evaluate_expr(unit, *value).is_none() {
                                    self.diagnostics.push(Diagnostic::new(
                                        CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE.template.to_string(),
                                        self.program.unit(unit).ast.exprs[value.0 as usize].span,
                                    ));
                                }
                                sub_exprs.push(*key);
                                sub_exprs.push(*value);
                            }
                        }
                    } else {
                        let mut seen_elems = Vec::new();
                        for el in elements {
                            if let ast::CollectionElement::Expression(e) = el {
                                if let Some(e_val) = evaluator.evaluate_expr(unit, *e) {
                                    if seen_elems.contains(&e_val) {
                                        self.diagnostics.push(Diagnostic::new(
                                            EQUAL_ELEMENTS_IN_CONST_SET.template.to_string(),
                                            self.program.unit(unit).ast.exprs[e.0 as usize].span,
                                        ));
                                    } else {
                                        seen_elems.push(e_val);
                                    }
                                } else {
                                    self.diagnostics.push(Diagnostic::new(
                                        CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE.template.to_string(),
                                        self.program.unit(unit).ast.exprs[e.0 as usize].span,
                                    ));
                                }
                                sub_exprs.push(*e);
                            }
                        }
                    }
                }
                ExprKind::List { elements, .. } => {
                    for el in elements {
                        if let ast::CollectionElement::Expression(e) = el {
                            sub_exprs.push(*e);
                        }
                    }
                }
                ExprKind::Parenthesized(inner) => {
                    sub_exprs.push(*inner);
                }
                _ => {}
            }
        }

        for sub in sub_exprs {
            self.validate_const_collection(unit, sub);
        }
    }
}
