//! Resolução de anotações de tipo do outline para [`TypeId`].
//!
//! Percorre todo o [`Program`] — classes, mixins, enums, extensions, extension types,
//! typedefs, funções, métodos, getters, setters, construtores e variáveis/campos —
//! e resolve toda anotação de tipo escrita (`TypeAnnotation`) para seu respectivo [`TypeId`].
//!
//! Suporta escopos aninhados (parâmetros de tipo locais sombreiam a biblioteca),
//! expansão transparente de `typedef`, *override inference* de membros herdados
//! sem anotação explícita e acumulação de diagnósticos sem parada prematura.

use crate::hierarchy::{ClassHierarchy, build_class_hierarchy};
use crate::ops::{nullable, substitute};
use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeParamOwner, TypeTable, Variance};
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::{
    ClassId, ClassKind, Element, ExtensionId, FunctionElement, FunctionElementId, FunctionKind,
    FunctionRef, LibraryId, Program, TypedefId, UnitId, VariableRef,
};
use dartforge_frontend::ast::{self, DeclKind, MemberKind, ParameterKind};
use dartforge_intern::{Interner, SymbolId};
use std::collections::HashMap;

type ParameterComponents = (Vec<TypeId>, Vec<TypeId>, Vec<(SymbolId, TypeId, bool)>);

/// Dados de tipo de uma função/método/construtor no outline.
#[derive(Debug, Clone)]
pub struct FunctionTypeData {
    /// Assinatura completa em forma de tipo de função (`Type::Function`).
    pub signature: TypeId,
    /// Tipo de retorno.
    pub return_type: TypeId,
    /// Parâmetros de tipo genéricos declarados nesta função.
    pub type_params: Box<[TypeParamId]>,
    /// Metadados e tipos de cada parâmetro formal.
    pub parameters: Box<[ParameterTypeData]>,
}

/// Metadados de um parâmetro formal de função.
#[derive(Debug, Clone)]
pub struct ParameterTypeData {
    pub name: Option<SymbolId>,
    pub ty: TypeId,
    pub required: bool,
    pub kind: ParameterKind,
}

/// Dados de tipo de uma variável de topo ou campo.
#[derive(Debug, Clone)]
pub struct VariableTypeData {
    /// Tipo explicitamente anotado na declaração (se houver).
    pub declared_type: Option<TypeId>,
    /// Tipo resolvido / inferido (no outline, `None` se depender do inicializador).
    pub inferred: Option<TypeId>,
}

/// Dados de tipos anotados em uma classe, mixin, enum ou extension type.
#[derive(Debug, Clone)]
pub struct ClassTypeData {
    pub type_params: Box<[TypeParamId]>,
    pub supertype: Option<TypeId>,
    pub mixins: Box<[TypeId]>,
    pub interfaces: Box<[TypeId]>,
    pub on: Box<[TypeId]>,
}

/// Dados de tipos anotados em uma extensão.
#[derive(Debug, Clone)]
pub struct ExtensionTypeData {
    pub type_params: Box<[TypeParamId]>,
    pub on: TypeId,
}

/// Dados de tipos de um typedef.
#[derive(Debug, Clone)]
pub struct TypedefTypeData {
    pub type_params: Box<[TypeParamId]>,
    pub target_type: TypeId,
}

/// Conjunto de todas as tabelas laterais de tipos do outline, indexadas pelos IDs dos elementos.
#[derive(Debug)]
pub struct OutlineTypes {
    pub functions: Vec<FunctionTypeData>,
    pub variables: Vec<VariableTypeData>,
    pub classes: Vec<ClassTypeData>,
    pub extensions: Vec<ExtensionTypeData>,
    pub typedefs: Vec<TypedefTypeData>,
    pub hierarchy: ClassHierarchy,
}

/// Contexto de resolução com acesso ao programa e acumuladores de estado.
pub struct OutlineResolver<'a> {
    pub program: &'a Program,
    pub interner: &'a Interner,
    pub table: &'a mut TypeTable,
    pub core: &'a CoreTypes,
    pub diagnostics: Vec<Diagnostic>,
    /// Parâmetros de tipo alocados por classe: `[ClassId] -> Box<[TypeParamId]>`
    pub class_type_params: Vec<Box<[TypeParamId]>>,
    /// Parâmetros de tipo alocados por typedef: `[TypedefId] -> Box<[TypeParamId]>`
    pub typedef_type_params: Vec<Box<[TypeParamId]>>,
    /// Parâmetros de tipo alocados por extension: `[ExtensionId] -> Box<[TypeParamId]>`
    pub extension_type_params: Vec<Box<[TypeParamId]>>,
    /// Tipos alvo expandidos de typedefs: `[TypedefId] -> Option<TypeId>`
    pub typedef_targets: Vec<Option<TypeId>>,
}

impl<'a> OutlineResolver<'a> {
    pub fn new(
        program: &'a Program,
        interner: &'a Interner,
        table: &'a mut TypeTable,
        core: &'a CoreTypes,
    ) -> Self {
        let num_classes = program.classes.len();
        let num_typedefs = program.typedefs.len();
        let num_extensions = program.extensions.len();

        Self {
            program,
            interner,
            table,
            core,
            diagnostics: Vec::new(),
            class_type_params: vec![Box::new([]); num_classes],
            typedef_type_params: vec![Box::new([]); num_typedefs],
            extension_type_params: vec![Box::new([]); num_extensions],
            typedef_targets: vec![None; num_typedefs],
        }
    }

    /// Resolve todo o outline do programa, retornando as tabelas laterais e os diagnósticos acumulados.
    pub fn resolve_all(mut self) -> (OutlineTypes, Vec<Diagnostic>) {
        // 1. Alocar TypeParamIds para classes, typedefs e extensions
        self.allocate_outline_type_params();

        // 2. Resolver typedefs (com suporte a referências mútuas)
        self.resolve_typedefs();

        // 3. Resolver anotações de supertipos das classes e construir a hierarquia instanciada
        let (class_type_data, hierarchy) = self.resolve_classes_and_hierarchy();

        // 4. Resolver extensions
        let extension_type_data = self.resolve_extensions();

        // 5. Resolver variáveis de topo e campos
        let mut variable_type_data = self.resolve_variables();

        // 6. Resolver funções, métodos e construtores (com override inference)
        let function_type_data = self.resolve_functions(&hierarchy, &mut variable_type_data);

        let typedef_type_data: Vec<TypedefTypeData> = self
            .typedef_type_params
            .iter()
            .zip(self.typedef_targets.iter())
            .map(|(params, target)| TypedefTypeData {
                type_params: params.clone(),
                target_type: target.unwrap_or(self.core.dynamic_),
            })
            .collect();

        let outline = OutlineTypes {
            functions: function_type_data,
            variables: variable_type_data,
            classes: class_type_data,
            extensions: extension_type_data,
            typedefs: typedef_type_data,
            hierarchy,
        };

        (outline, self.diagnostics)
    }

    fn allocate_outline_type_params(&mut self) {
        for (i, class) in self.program.classes.iter().enumerate() {
            let class_id = ClassId(i as u32);
            let mut params = Vec::with_capacity(class.type_params.len());
            for p in class.type_params.iter() {
                let pid = self.table.alloc_type_param(
                    p.name,
                    TypeParamOwner::Class(class_id),
                    self.core.object_nullable,
                    Variance::Unspecified,
                );
                params.push(pid);
            }
            self.class_type_params[i] = params.into_boxed_slice();
        }

        for (i, typedef) in self.program.typedefs.iter().enumerate() {
            let typedef_id = TypedefId(i as u32);
            let mut params = Vec::with_capacity(typedef.type_params.len());
            for p in typedef.type_params.iter() {
                let pid = self.table.alloc_type_param(
                    p.name,
                    TypeParamOwner::Typedef(typedef_id),
                    self.core.object_nullable,
                    Variance::Unspecified,
                );
                params.push(pid);
            }
            self.typedef_type_params[i] = params.into_boxed_slice();
        }

        for (i, ext) in self.program.extensions.iter().enumerate() {
            let ext_id = ExtensionId(i as u32);
            let mut params = Vec::with_capacity(ext.type_params.len());
            for p in ext.type_params.iter() {
                let pid = self.table.alloc_type_param(
                    p.name,
                    TypeParamOwner::Extension(ext_id),
                    self.core.object_nullable,
                    Variance::Unspecified,
                );
                params.push(pid);
            }
            self.extension_type_params[i] = params.into_boxed_slice();
        }

        // Atualizar bounds escritos para classes
        for (i, class) in self.program.classes.iter().enumerate() {
            let params = self.class_type_params[i].clone();
            let mut scope = HashMap::with_capacity(params.len());
            for (p_elem, &pid) in class.type_params.iter().zip(params.iter()) {
                scope.insert(p_elem.name, pid);
            }
            for (p_elem, &pid) in class.type_params.iter().zip(params.iter()) {
                if let Some((unit_id, ast_ty_id)) = p_elem.bound {
                    let bound_ty =
                        self.resolve_annotation(unit_id, ast_ty_id, class.library, &scope);
                    self.table.set_type_param_bound(pid, bound_ty);
                }
            }
        }
    }

    fn resolve_typedefs(&mut self) {
        for i in 0..self.program.typedefs.len() {
            let typedef_id = TypedefId(i as u32);
            self.ensure_typedef_resolved(typedef_id);
        }
    }

    fn ensure_typedef_resolved(&mut self, id: TypedefId) -> TypeId {
        let idx = id.0 as usize;
        if let Some(target) = self.typedef_targets[idx] {
            return target;
        }

        let typedef = &self.program.typedefs[idx];
        let params = self.typedef_type_params[idx].clone();
        let mut scope = HashMap::with_capacity(params.len());
        for (p_elem, &pid) in typedef.type_params.iter().zip(params.iter()) {
            scope.insert(p_elem.name, pid);
        }

        // Define fallback temporário para evitar ciclos
        self.typedef_targets[idx] = Some(self.core.dynamic_);

        let unit_id = typedef.decl.unit;
        let decl = self.program.unit(unit_id).ast.decl(typedef.decl.decl);
        let resolved_target = match &decl.kind {
            DeclKind::Typedef(d) => match &d.kind {
                ast::TypedefKind::Alias(ast_ty) => {
                    self.resolve_annotation(unit_id, *ast_ty, typedef.library, &scope)
                }
                ast::TypedefKind::Legacy {
                    return_type,
                    parameters,
                } => {
                    let ret = if let Some(r) = return_type {
                        self.resolve_annotation(unit_id, *r, typedef.library, &scope)
                    } else {
                        self.core.dynamic_
                    };
                    let (pos, opt, named) = self.resolve_ast_parameter_types(
                        unit_id,
                        parameters,
                        typedef.library,
                        &scope,
                    );
                    self.table.intern(Type::Function {
                        type_params: Box::new([]),
                        ret,
                        positional: pos.into_boxed_slice(),
                        optional: opt.into_boxed_slice(),
                        named: named.into_boxed_slice(),
                        nullable: false,
                    })
                }
            },
            _ => self.core.dynamic_,
        };

        self.typedef_targets[idx] = Some(resolved_target);
        resolved_target
    }

    fn resolve_classes_and_hierarchy(&mut self) -> (Vec<ClassTypeData>, ClassHierarchy) {
        let mut class_type_data = Vec::with_capacity(self.program.classes.len());
        let mut hierarchy_inputs: Vec<Option<crate::hierarchy::ImmediateSupertypeInput>> =
            Vec::with_capacity(self.program.classes.len());

        for (i, class) in self.program.classes.iter().enumerate() {
            let params = self.class_type_params[i].clone();
            let mut scope = HashMap::with_capacity(params.len());
            for (p_elem, &pid) in class.type_params.iter().zip(params.iter()) {
                scope.insert(p_elem.name, pid);
            }

            let supertype = class
                .supertype
                .map(|(unit, ast_id)| self.resolve_annotation(unit, ast_id, class.library, &scope));

            let mixins: Vec<TypeId> = class
                .mixins
                .iter()
                .map(|&(unit, ast_id)| self.resolve_annotation(unit, ast_id, class.library, &scope))
                .collect();

            let interfaces: Vec<TypeId> = class
                .interfaces
                .iter()
                .map(|&(unit, ast_id)| self.resolve_annotation(unit, ast_id, class.library, &scope))
                .collect();

            let on: Vec<TypeId> = class
                .on
                .iter()
                .map(|&(unit, ast_id)| self.resolve_annotation(unit, ast_id, class.library, &scope))
                .collect();

            let mut all_direct = Vec::new();
            if let Some(s) = supertype {
                all_direct.push(s);
            }
            all_direct.extend_from_slice(&mixins);
            all_direct.extend_from_slice(&interfaces);
            all_direct.extend_from_slice(&on);

            hierarchy_inputs.push(Some((params.clone(), all_direct)));

            class_type_data.push(ClassTypeData {
                type_params: params,
                supertype,
                mixins: mixins.into_boxed_slice(),
                interfaces: interfaces.into_boxed_slice(),
                on: on.into_boxed_slice(),
            });
        }

        let hierarchy = build_class_hierarchy(
            self.program.classes.len(),
            &hierarchy_inputs,
            self.table,
            self.core,
        );

        (class_type_data, hierarchy)
    }

    fn resolve_extensions(&mut self) -> Vec<ExtensionTypeData> {
        let mut data = Vec::with_capacity(self.program.extensions.len());
        for (i, ext) in self.program.extensions.iter().enumerate() {
            let params = self.extension_type_params[i].clone();
            let mut scope = HashMap::with_capacity(params.len());
            for (p_elem, &pid) in ext.type_params.iter().zip(params.iter()) {
                scope.insert(p_elem.name, pid);
            }
            let on_ty = self.resolve_annotation(ext.on.0, ext.on.1, ext.library, &scope);
            data.push(ExtensionTypeData {
                type_params: params,
                on: on_ty,
            });
        }
        data
    }

    fn resolve_variables(&mut self) -> Vec<VariableTypeData> {
        let mut data = Vec::with_capacity(self.program.variables.len());
        for var in self.program.variables.iter() {
            let (declared, inferred) = match var.node {
                VariableRef::TopLevel { unit, decl, .. } => {
                    let decl_node = self.program.unit(unit).ast.decl(decl);
                    if let DeclKind::Variables(var_list) = &decl_node.kind {
                        if let Some(ast_ty) = var_list.ty {
                            let ty =
                                self.resolve_annotation(unit, ast_ty, var.library, &HashMap::new());
                            (Some(ty), Some(ty))
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    }
                }
                VariableRef::Field { unit, member, .. } => {
                    let mem_node = &self.program.unit(unit).ast.members[member.0 as usize];
                    if let MemberKind::Field(var_list) = &mem_node.kind {
                        let scope = self.get_enclosing_type_param_scope(var.class, var.extension);
                        if let Some(ast_ty) = var_list.ty {
                            let ty = self.resolve_annotation(unit, ast_ty, var.library, &scope);
                            (Some(ty), Some(ty))
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    }
                }
                VariableRef::EnumConstant { .. } => {
                    if let Some(cls) = var.class {
                        let ty = self.table.intern(Type::Interface {
                            class: cls,
                            args: Box::new([]),
                            nullable: false,
                        });
                        (Some(ty), Some(ty))
                    } else {
                        (None, None)
                    }
                }
                VariableRef::Representation { unit, decl } => {
                    let decl_node = self.program.unit(unit).ast.decl(decl);
                    if let DeclKind::ExtensionType(ext) = &decl_node.kind {
                        let scope = self.get_enclosing_type_param_scope(var.class, None);
                        let ty = self.resolve_annotation(
                            unit,
                            ext.representation_type,
                            var.library,
                            &scope,
                        );
                        (Some(ty), Some(ty))
                    } else {
                        (None, None)
                    }
                }
                VariableRef::None => (None, None),
            };

            data.push(VariableTypeData {
                declared_type: declared,
                inferred,
            });
        }
        data
    }

    fn resolve_functions(
        &mut self,
        hierarchy: &ClassHierarchy,
        variables: &mut [VariableTypeData],
    ) -> Vec<FunctionTypeData> {
        let mut data = Vec::with_capacity(self.program.functions.len());

        for (i, func) in self.program.functions.iter().enumerate() {
            let func_id = FunctionElementId(i as u32);
            let (sig, ret, params, tparams) =
                self.resolve_function_signature(func_id, func, hierarchy, variables);

            // Se for acessor implícito de variável, sincronizar se necessário
            if let Some(var_id) = func.variable {
                let var_data = &mut variables[var_id.0 as usize];
                if func.kind == FunctionKind::Getter && var_data.declared_type.is_none() {
                    var_data.inferred = Some(ret);
                }
            }

            data.push(FunctionTypeData {
                signature: sig,
                return_type: ret,
                type_params: tparams,
                parameters: params,
            });
        }

        data
    }

    fn resolve_function_signature(
        &mut self,
        func_id: FunctionElementId,
        func: &FunctionElement,
        hierarchy: &ClassHierarchy,
        variables: &[VariableTypeData],
    ) -> (TypeId, TypeId, Box<[ParameterTypeData]>, Box<[TypeParamId]>) {
        let mut scope = self.get_enclosing_type_param_scope(func.class, func.extension);

        match func.node {
            FunctionRef::Function { unit, function } => {
                let ast_func = &self.program.unit(unit).ast.functions[function.0 as usize];

                // Parâmetros de tipo da função genérica
                let mut func_type_params = Vec::with_capacity(ast_func.type_params.len());
                for tp in ast_func.type_params.iter() {
                    let pid = self.table.alloc_type_param(
                        tp.name.sym,
                        TypeParamOwner::Function(func_id),
                        self.core.object_nullable,
                        Variance::Unspecified,
                    );
                    func_type_params.push(pid);
                    scope.insert(tp.name.sym, pid);
                }

                // Bounds dos parâmetros de tipo
                for (tp, &pid) in ast_func.type_params.iter().zip(func_type_params.iter()) {
                    if let Some(ast_bound) = tp.bound {
                        let b = self.resolve_annotation(unit, ast_bound, func.library, &scope);
                        self.table.set_type_param_bound(pid, b);
                    }
                }

                // Resolução de parâmetros formais
                let mut param_types = Vec::new();
                let mut positional = Vec::new();
                let mut optional = Vec::new();
                let mut named = Vec::new();

                if let Some(parameters) = &ast_func.parameters {
                    for p in parameters.iter() {
                        let p_name = p.name.as_ref().map(|n| n.sym);
                        let p_ty = if let Some(ast_ty) = p.ty {
                            self.resolve_annotation(unit, ast_ty, func.library, &scope)
                        } else {
                            // Tenta override inference se for método de instância
                            self.infer_override_parameter_type(func, p_name, hierarchy)
                                .unwrap_or(self.core.dynamic_)
                        };

                        param_types.push(ParameterTypeData {
                            name: p_name,
                            ty: p_ty,
                            required: p.required,
                            kind: p.kind,
                        });

                        match p.kind {
                            ParameterKind::Required => positional.push(p_ty),
                            ParameterKind::Optional => optional.push(p_ty),
                            ParameterKind::Named => {
                                if let Some(sym) = p_name {
                                    named.push((sym, p_ty, p.required));
                                }
                            }
                        }
                    }
                }

                // Retorno da função
                let ret_ty = if let Some(ast_ret) = ast_func.return_type {
                    self.resolve_annotation(unit, ast_ret, func.library, &scope)
                } else if func.kind == FunctionKind::Setter {
                    self.core.void_
                } else {
                    // Tenta override inference para retorno
                    self.infer_override_return_type(func, hierarchy)
                        .unwrap_or(self.core.dynamic_)
                };

                let sig = self.table.intern(Type::Function {
                    type_params: func_type_params.clone().into_boxed_slice(),
                    ret: ret_ty,
                    positional: positional.into_boxed_slice(),
                    optional: optional.into_boxed_slice(),
                    named: named.into_boxed_slice(),
                    nullable: false,
                });

                (
                    sig,
                    ret_ty,
                    param_types.into_boxed_slice(),
                    func_type_params.into_boxed_slice(),
                )
            }
            FunctionRef::Constructor { unit, member } => {
                let mem_node = &self.program.unit(unit).ast.members[member.0 as usize];
                let ctor = match &mem_node.kind {
                    MemberKind::Constructor(c) => c,
                    _ => unreachable!(),
                };

                let mut param_types = Vec::new();
                let mut positional = Vec::new();
                let mut optional = Vec::new();
                let mut named = Vec::new();

                for p in ctor.parameters.iter() {
                    let p_name = p.name.as_ref().map(|n| n.sym);
                    let p_ty = if let Some(ast_ty) = p.ty {
                        self.resolve_annotation(unit, ast_ty, func.library, &scope)
                    } else {
                        self.core.dynamic_
                    };

                    param_types.push(ParameterTypeData {
                        name: p_name,
                        ty: p_ty,
                        required: p.required,
                        kind: p.kind,
                    });

                    match p.kind {
                        ParameterKind::Required => positional.push(p_ty),
                        ParameterKind::Optional => optional.push(p_ty),
                        ParameterKind::Named => {
                            if let Some(sym) = p_name {
                                named.push((sym, p_ty, p.required));
                            }
                        }
                    }
                }

                // Construtor instancia a classe dona com seus próprios parâmetros de tipo
                let ret_ty = self.instantiate_self_class(func.class);

                let sig = self.table.intern(Type::Function {
                    type_params: Box::new([]),
                    ret: ret_ty,
                    positional: positional.into_boxed_slice(),
                    optional: optional.into_boxed_slice(),
                    named: named.into_boxed_slice(),
                    nullable: false,
                });

                (sig, ret_ty, param_types.into_boxed_slice(), Box::new([]))
            }
            FunctionRef::None => {
                // Sintéticos ou acessores implícitos
                if let Some(var_id) = func.variable {
                    let var_data = &variables[var_id.0 as usize];
                    let var_ty = var_data
                        .declared_type
                        .or(var_data.inferred)
                        .unwrap_or(self.core.dynamic_);

                    if func.kind == FunctionKind::Getter {
                        let sig = self.table.intern(Type::Function {
                            type_params: Box::new([]),
                            ret: var_ty,
                            positional: Box::new([]),
                            optional: Box::new([]),
                            named: Box::new([]),
                            nullable: false,
                        });
                        (sig, var_ty, Box::new([]), Box::new([]))
                    } else {
                        // Setter
                        let param = ParameterTypeData {
                            name: None,
                            ty: var_ty,
                            required: true,
                            kind: ParameterKind::Required,
                        };
                        let sig = self.table.intern(Type::Function {
                            type_params: Box::new([]),
                            ret: self.core.void_,
                            positional: Box::new([var_ty]),
                            optional: Box::new([]),
                            named: Box::new([]),
                            nullable: false,
                        });
                        (sig, self.core.void_, Box::new([param]), Box::new([]))
                    }
                } else {
                    // Construtor sintético padrão
                    let ret_ty = self.instantiate_self_class(func.class);
                    let sig = self.table.intern(Type::Function {
                        type_params: Box::new([]),
                        ret: ret_ty,
                        positional: Box::new([]),
                        optional: Box::new([]),
                        named: Box::new([]),
                        nullable: false,
                    });
                    (sig, ret_ty, Box::new([]), Box::new([]))
                }
            }
        }
    }

    fn instantiate_self_class(&mut self, class_opt: Option<ClassId>) -> TypeId {
        if let Some(cls) = class_opt {
            let params = self.class_type_params[cls.0 as usize].clone();
            let args: Vec<TypeId> = params
                .iter()
                .map(|&p| {
                    self.table.intern(Type::TypeParameter {
                        param: p,
                        nullable: false,
                    })
                })
                .collect();
            let is_ext = self.program.class(cls).kind == ClassKind::ExtensionType;
            if is_ext {
                self.table.intern(Type::ExtensionType {
                    decl: cls,
                    args: args.into_boxed_slice(),
                    nullable: false,
                })
            } else {
                self.table.intern(Type::Interface {
                    class: cls,
                    args: args.into_boxed_slice(),
                    nullable: false,
                })
            }
        } else {
            self.core.dynamic_
        }
    }

    fn infer_override_return_type(
        &mut self,
        func: &FunctionElement,
        hierarchy: &ClassHierarchy,
    ) -> Option<TypeId> {
        let class_id = func.class?;
        if func.static_ {
            return None;
        }

        let class_data = hierarchy.get(class_id)?;
        for &super_class in class_data.supertypes.keys() {
            let super_elem = self.program.class(super_class);
            if let Some(&super_func_id) = super_elem.instance_members.get(&func.name) {
                let super_func = self.program.function(super_func_id);
                if super_func.kind == func.kind {
                    // Obter retorno da função super ancestral instanciada
                    if let FunctionRef::Function { unit, function } = super_func.node {
                        let ast_func = &self.program.unit(unit).ast.functions[function.0 as usize];
                        if let Some(ast_ret) = ast_func.return_type {
                            let super_scope =
                                self.get_enclosing_type_param_scope(Some(super_class), None);
                            let uninstantiated_ret = self.resolve_annotation(
                                unit,
                                ast_ret,
                                super_func.library,
                                &super_scope,
                            );
                            // Mapeia parâmetros formais da superclasse para os tipos desta classe
                            let super_ty = hierarchy.supertype_of(
                                self.instantiate_self_class(Some(class_id)),
                                super_class,
                                self.table,
                                self.core,
                            )?;
                            if let Type::Interface { args, .. } = self.table.get(super_ty).clone() {
                                let super_params = &self.class_type_params[super_class.0 as usize];
                                let mut subst = HashMap::with_capacity(super_params.len());
                                for (&p, &a) in super_params.iter().zip(args.iter()) {
                                    subst.insert(p, a);
                                }
                                return Some(substitute(uninstantiated_ret, &subst, self.table));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn infer_override_parameter_type(
        &mut self,
        func: &FunctionElement,
        param_name: Option<SymbolId>,
        hierarchy: &ClassHierarchy,
    ) -> Option<TypeId> {
        let class_id = func.class?;
        let p_name = param_name?;
        if func.static_ {
            return None;
        }

        let class_data = hierarchy.get(class_id)?;
        for &super_class in class_data.supertypes.keys() {
            let super_elem = self.program.class(super_class);
            if let Some(&super_func_id) = super_elem.instance_members.get(&func.name) {
                let super_func = self.program.function(super_func_id);
                if let FunctionRef::Function { unit, function } = super_func.node {
                    let ast_func = &self.program.unit(unit).ast.functions[function.0 as usize];
                    if let Some(params) = &ast_func.parameters {
                        for p in params.iter() {
                            if p.name.as_ref().map(|n| n.sym) == Some(p_name)
                                && let Some(ast_ty) = p.ty
                            {
                                let super_scope =
                                    self.get_enclosing_type_param_scope(Some(super_class), None);
                                let uninstantiated_ty = self.resolve_annotation(
                                    unit,
                                    ast_ty,
                                    super_func.library,
                                    &super_scope,
                                );
                                let super_ty = hierarchy.supertype_of(
                                    self.instantiate_self_class(Some(class_id)),
                                    super_class,
                                    self.table,
                                    self.core,
                                )?;
                                if let Type::Interface { args, .. } =
                                    self.table.get(super_ty).clone()
                                {
                                    let super_params =
                                        &self.class_type_params[super_class.0 as usize];
                                    let mut subst = HashMap::with_capacity(super_params.len());
                                    for (&sp, &sa) in super_params.iter().zip(args.iter()) {
                                        subst.insert(sp, sa);
                                    }
                                    return Some(substitute(uninstantiated_ty, &subst, self.table));
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn get_enclosing_type_param_scope(
        &self,
        class: Option<ClassId>,
        ext: Option<ExtensionId>,
    ) -> HashMap<SymbolId, TypeParamId> {
        let mut scope = HashMap::new();
        if let Some(cls) = class {
            let elem = self.program.class(cls);
            let params = &self.class_type_params[cls.0 as usize];
            for (p_elem, &pid) in elem.type_params.iter().zip(params.iter()) {
                scope.insert(p_elem.name, pid);
            }
        } else if let Some(e) = ext {
            let elem = self.program.extension(e);
            let params = &self.extension_type_params[e.0 as usize];
            for (p_elem, &pid) in elem.type_params.iter().zip(params.iter()) {
                scope.insert(p_elem.name, pid);
            }
        }
        scope
    }

    fn resolve_annotation(
        &mut self,
        unit_id: UnitId,
        ast_ty_id: ast::TypeId,
        library: LibraryId,
        type_param_scope: &HashMap<SymbolId, TypeParamId>,
    ) -> TypeId {
        let annot = self.program.unit(unit_id).ast.ty(ast_ty_id);
        let is_nullable = annot.nullable;
        let span = annot.span;

        match &annot.kind {
            ast::TypeKind::Void => self.core.void_,
            ast::TypeKind::Named { name, args } => {
                if name.len() == 1 {
                    let sym = name[0].sym;

                    // 1. Verificar escopo de parâmetros de tipo vigentes
                    if let Some(&param_id) = type_param_scope.get(&sym) {
                        if !args.is_empty() {
                            self.diagnostics.push(Diagnostic::new(
                                "Parâmetro de tipo não aceita argumentos de tipo",
                                span,
                            ));
                        }
                        return self.table.intern(Type::TypeParameter {
                            param: param_id,
                            nullable: is_nullable,
                        });
                    }

                    // 2. Verificar tipos especiais do sistema
                    if self.interner.lookup("dynamic") == Some(sym) {
                        return self.core.dynamic_;
                    }
                    if self.interner.lookup("void") == Some(sym) {
                        return self.core.void_;
                    }
                    if self.interner.lookup("Never") == Some(sym) {
                        return if is_nullable {
                            self.core.null
                        } else {
                            self.core.never
                        };
                    }
                    if self.interner.lookup("Null") == Some(sym) {
                        return self.core.null;
                    }
                    if self.interner.lookup("FutureOr") == Some(sym) {
                        if args.len() == 1 {
                            let arg_ty = self.resolve_annotation(
                                unit_id,
                                args[0],
                                library,
                                type_param_scope,
                            );
                            return self.table.intern(Type::FutureOr {
                                arg: arg_ty,
                                nullable: is_nullable,
                            });
                        } else {
                            self.diagnostics.push(Diagnostic::new(
                                "FutureOr exige exatamente um argumento de tipo",
                                span,
                            ));
                            return self.core.dynamic_;
                        }
                    }

                    // 3. Resolução no escopo da biblioteca
                    let binding = self.program.lookup(library, sym);
                    match binding {
                        Some(b) => {
                            if b.ambiguous {
                                self.diagnostics
                                    .push(Diagnostic::new("Referência ambígua de tipo", span));
                                return self.core.dynamic_;
                            }
                            match b.getter {
                                Some(Element::Class(cid)) => {
                                    let resolved_args: Vec<TypeId> = args
                                        .iter()
                                        .map(|&a| {
                                            self.resolve_annotation(
                                                unit_id,
                                                a,
                                                library,
                                                type_param_scope,
                                            )
                                        })
                                        .collect();

                                    let is_ext =
                                        self.program.class(cid).kind == ClassKind::ExtensionType;
                                    if is_ext {
                                        self.table.intern(Type::ExtensionType {
                                            decl: cid,
                                            args: resolved_args.into_boxed_slice(),
                                            nullable: is_nullable,
                                        })
                                    } else {
                                        self.table.intern(Type::Interface {
                                            class: cid,
                                            args: resolved_args.into_boxed_slice(),
                                            nullable: is_nullable,
                                        })
                                    }
                                }
                                Some(Element::Typedef(tid)) => {
                                    let resolved_args: Vec<TypeId> = args
                                        .iter()
                                        .map(|&a| {
                                            self.resolve_annotation(
                                                unit_id,
                                                a,
                                                library,
                                                type_param_scope,
                                            )
                                        })
                                        .collect();

                                    let target_ty = self.ensure_typedef_resolved(tid);
                                    let formals = self.typedef_type_params[tid.0 as usize].clone();
                                    let mut subst = HashMap::with_capacity(formals.len());
                                    for (&f, &a) in formals.iter().zip(resolved_args.iter()) {
                                        subst.insert(f, a);
                                    }
                                    let expanded = substitute(target_ty, &subst, self.table);
                                    if is_nullable {
                                        nullable(expanded, self.table)
                                    } else {
                                        expanded
                                    }
                                }
                                _ => {
                                    self.diagnostics.push(Diagnostic::new(
                                        "O símbolo encontrado não é um tipo",
                                        span,
                                    ));
                                    self.core.dynamic_
                                }
                            }
                        }
                        None => {
                            self.diagnostics.push(Diagnostic::new(
                                "Tipo não encontrado no escopo da biblioteca",
                                span,
                            ));
                            self.core.dynamic_
                        }
                    }
                } else if name.len() == 2 {
                    let prefix = name[0].sym;
                    let member = name[1].sym;
                    let binding = self.program.lookup_prefixed(library, prefix, member);
                    match binding {
                        Some(b) => match b.getter {
                            Some(Element::Class(cid)) => {
                                let resolved_args: Vec<TypeId> = args
                                    .iter()
                                    .map(|&a| {
                                        self.resolve_annotation(
                                            unit_id,
                                            a,
                                            library,
                                            type_param_scope,
                                        )
                                    })
                                    .collect();

                                let is_ext =
                                    self.program.class(cid).kind == ClassKind::ExtensionType;
                                if is_ext {
                                    self.table.intern(Type::ExtensionType {
                                        decl: cid,
                                        args: resolved_args.into_boxed_slice(),
                                        nullable: is_nullable,
                                    })
                                } else {
                                    self.table.intern(Type::Interface {
                                        class: cid,
                                        args: resolved_args.into_boxed_slice(),
                                        nullable: is_nullable,
                                    })
                                }
                            }
                            Some(Element::Typedef(tid)) => {
                                let resolved_args: Vec<TypeId> = args
                                    .iter()
                                    .map(|&a| {
                                        self.resolve_annotation(
                                            unit_id,
                                            a,
                                            library,
                                            type_param_scope,
                                        )
                                    })
                                    .collect();

                                let target_ty = self.ensure_typedef_resolved(tid);
                                let formals = self.typedef_type_params[tid.0 as usize].clone();
                                let mut subst = HashMap::with_capacity(formals.len());
                                for (&f, &a) in formals.iter().zip(resolved_args.iter()) {
                                    subst.insert(f, a);
                                }
                                let expanded = substitute(target_ty, &subst, self.table);
                                if is_nullable {
                                    nullable(expanded, self.table)
                                } else {
                                    expanded
                                }
                            }
                            _ => {
                                self.diagnostics.push(Diagnostic::new(
                                    "O elemento prefixado não é um tipo",
                                    span,
                                ));
                                self.core.dynamic_
                            }
                        },
                        None => {
                            self.diagnostics
                                .push(Diagnostic::new("Tipo prefixado não encontrado", span));
                            self.core.dynamic_
                        }
                    }
                } else {
                    self.core.dynamic_
                }
            }
            ast::TypeKind::Function {
                return_type,
                type_params,
                parameters,
            } => {
                let mut local_scope = type_param_scope.clone();
                let mut local_params = Vec::with_capacity(type_params.len());
                for tp in type_params.iter() {
                    let pid = self.table.alloc_type_param(
                        tp.name.sym,
                        TypeParamOwner::GenericFunctionType,
                        self.core.object_nullable,
                        Variance::Unspecified,
                    );
                    local_params.push(pid);
                    local_scope.insert(tp.name.sym, pid);
                }

                for (tp, &pid) in type_params.iter().zip(local_params.iter()) {
                    if let Some(ast_bound) = tp.bound {
                        let b = self.resolve_annotation(unit_id, ast_bound, library, &local_scope);
                        self.table.set_type_param_bound(pid, b);
                    }
                }

                let ret = if let Some(r) = return_type {
                    self.resolve_annotation(unit_id, *r, library, &local_scope)
                } else {
                    self.core.dynamic_
                };

                let (pos, opt, named) =
                    self.resolve_ast_parameter_types(unit_id, parameters, library, &local_scope);

                self.table.intern(Type::Function {
                    type_params: local_params.into_boxed_slice(),
                    ret,
                    positional: pos.into_boxed_slice(),
                    optional: opt.into_boxed_slice(),
                    named: named.into_boxed_slice(),
                    nullable: is_nullable,
                })
            }
            ast::TypeKind::Record { positional, named } => {
                let mut pos = Vec::with_capacity(positional.len());
                for &p in positional.iter() {
                    pos.push(self.resolve_annotation(unit_id, p, library, type_param_scope));
                }

                let mut n = Vec::with_capacity(named.len());
                for (name, ty) in named.iter() {
                    let resolved = self.resolve_annotation(unit_id, *ty, library, type_param_scope);
                    n.push((name.sym, resolved));
                }

                self.table.intern(Type::Record {
                    positional: pos.into_boxed_slice(),
                    named: n.into_boxed_slice(),
                    nullable: is_nullable,
                })
            }
        }
    }

    fn resolve_ast_parameter_types(
        &mut self,
        unit_id: UnitId,
        parameters: &[ast::Parameter],
        library: LibraryId,
        scope: &HashMap<SymbolId, TypeParamId>,
    ) -> ParameterComponents {
        let mut positional = Vec::new();
        let mut optional = Vec::new();
        let mut named = Vec::new();

        for p in parameters.iter() {
            let p_ty = if let Some(ast_ty) = p.ty {
                self.resolve_annotation(unit_id, ast_ty, library, scope)
            } else {
                self.core.dynamic_
            };

            match p.kind {
                ParameterKind::Required => positional.push(p_ty),
                ParameterKind::Optional => optional.push(p_ty),
                ParameterKind::Named => {
                    if let Some(n) = &p.name {
                        named.push((n.sym, p_ty, p.required));
                    }
                }
            }
        }

        (positional, optional, named)
    }
}
