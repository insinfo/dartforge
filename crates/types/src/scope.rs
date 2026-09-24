//! Escopos léxicos e resolução de identificadores e membros em corpos.
//!
//! Gerencia a pilha de escopos léxicos (locais, parâmetros, membros de instância implícitos,
//! biblioteca e prefixos) e a busca de membros e extensions aplicáveis sobre receptores estáticos.

use crate::codes::*;
use crate::hierarchy::ClassHierarchy;
use crate::ops::substitute;
use crate::resolved::{LocalId, MemberRef, Resolved};
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeTable};
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{
    ClassId, ClassKind, Element, ExtensionId, FunctionElementId, FunctionKind, LibraryId, Program,
};
use dartforge_frontend::ast;
use dartforge_intern::SymbolId;
use std::collections::HashMap;

/// Supertipos de `class` na ordem de precedência da busca de membros, e
/// determinística.
///
/// `ClassHierarchyData::supertypes` é um `HashMap`: percorrê-lo direto fazia
/// a busca de um membro herdado depender da semente do hash — com mixins
/// (dois supertipos definindo o mesmo membro) a resolução mudava de uma
/// execução para outra, e o teste de determinismo do backend nativo pegou.
/// Ordem: a cadeia de superclasses, com os mixins de cada aplicação logo
/// depois dela (o último mixin primeiro — a linearização do Dart); depois o
/// resto (interfaces), por `ClassId`.
pub fn supertipos_ordenados(
    program: &Program,
    hierarchy: &ClassHierarchy,
    class: ClassId,
) -> Vec<(ClassId, TypeId)> {
    let Some(data) = hierarchy.get(class) else {
        return Vec::new();
    };
    let mut saida: Vec<(ClassId, TypeId)> = Vec::new();
    let mut vistos: std::collections::HashSet<ClassId> = std::collections::HashSet::new();
    let mut empurrar = |c: ClassId, saida: &mut Vec<(ClassId, TypeId)>| {
        if let Some(&t) = data.supertypes.get(&c)
            && vistos.insert(c)
        {
            saida.push((c, t));
        }
    };
    let mut atual = program.classes[class.0 as usize].supertype_class;
    for &m in program.classes[class.0 as usize].mixin_classes.iter().rev() {
        empurrar(m, &mut saida);
    }
    while let Some(c) = atual {
        empurrar(c, &mut saida);
        for &m in program.classes[c.0 as usize].mixin_classes.iter().rev() {
            empurrar(m, &mut saida);
        }
        atual = program.classes[c.0 as usize].supertype_class;
    }
    let mut resto: Vec<(ClassId, TypeId)> = data
        .supertypes
        .iter()
        .filter(|(c, _)| !saida.iter().any(|(s, _)| s == *c))
        .map(|(c, t)| (*c, *t))
        .collect();
    resto.sort_by_key(|(c, _)| c.0);
    saida.extend(resto);
    saida
}

/// Metadados de uma variável local no escopo léxico.
#[derive(Debug, Clone)]
pub struct LocalVarInfo {
    pub id: LocalId,
    pub name: SymbolId,
    pub declared_type: TypeId,
    pub is_final: bool,
    pub is_const: bool,
    pub is_late: bool,
    pub declaration_offset: usize,
    /// `false` num curinga (Dart 3.7): o local existe (tem `LocalId`, o
    /// inicializador roda), mas o nome `_` não o encontra.
    pub ligada: bool,
}

/// Metadados de um parâmetro formal no escopo léxico.
#[derive(Debug, Clone)]
pub struct ParamInfo {
    pub index: u32,
    pub name: SymbolId,
    pub ty: TypeId,
    pub is_final: bool,
}

/// Camada de escopo léxico na pilha de um corpo de função.
#[derive(Debug, Clone, Default)]
pub struct ScopeBlock {
    pub locals: Vec<LocalVarInfo>,
    pub local_functions: HashMap<SymbolId, (ast::FunctionId, TypeId)>,
}

/// Gerenciador de escopos léxicos durante a travessia de um corpo de função ou método.
pub struct ScopeStack {
    /// Pilha de blocos léxicos aninhados (`{ ... }`).
    pub blocks: Vec<ScopeBlock>,
    /// Parâmetros formais da função/método envolvente.
    pub parameters: Vec<ParamInfo>,
    /// Parâmetros de tipo genéricos locais (da função).
    pub function_type_params: HashMap<SymbolId, TypeParamId>,
    /// Classe envolvente (se estiver dentro de método de instância, getter, setter ou construtor).
    pub enclosing_class: Option<ClassId>,
    /// Extensão envolvente (se estiver dentro de membro de extensão).
    pub enclosing_extension: Option<ExtensionId>,
    /// Se o membro atual é estático (`static`).
    pub is_static_context: bool,
    /// Próximo ID a ser atribuído para variável local.
    pub next_local_id: u32,
    /// O símbolo `_` quando a biblioteca tem curingas (Dart 3.7): local e
    /// parâmetro com esse nome não ligam nome.
    pub curinga: Option<SymbolId>,
}

impl ScopeStack {
    /// Cria uma nova pilha de escopos para uma função ou método.
    pub fn new(
        enclosing_class: Option<ClassId>,
        enclosing_extension: Option<ExtensionId>,
        is_static_context: bool,
    ) -> Self {
        Self {
            blocks: vec![ScopeBlock::default()],
            parameters: Vec::new(),
            function_type_params: HashMap::new(),
            enclosing_class,
            enclosing_extension,
            is_static_context,
            next_local_id: 0,
            curinga: None,
        }
    }

    /// Entra em um novo bloco léxico aninhado.
    pub fn push_block(&mut self) {
        self.blocks.push(ScopeBlock::default());
    }

    /// Sai do bloco léxico atual.
    pub fn pop_block(&mut self) {
        if self.blocks.len() > 1 {
            self.blocks.pop();
        }
    }

    /// Declara uma variável local no bloco mais interno.
    pub fn declare_local(
        &mut self,
        name: SymbolId,
        declared_type: TypeId,
        is_final: bool,
        is_const: bool,
        is_late: bool,
        declaration_offset: usize,
    ) -> LocalId {
        let id = LocalId(self.next_local_id);
        self.next_local_id += 1;
        let info = LocalVarInfo {
            id,
            name,
            declared_type,
            is_final,
            is_const,
            is_late,
            declaration_offset,
            ligada: self.curinga != Some(name),
        };
        if let Some(block) = self.blocks.last_mut() {
            block.locals.push(info);
        }
        id
    }

    /// Busca uma variável local pelo nome no bloco atual.
    pub fn find_local_in_current_block(&mut self, name: SymbolId) -> Option<&mut LocalVarInfo> {
        if let Some(block) = self.blocks.last_mut() {
            block.locals.iter_mut().find(|l| l.ligada && l.name == name)
        } else {
            None
        }
    }

    /// Adiciona um parâmetro formal ao escopo.
    pub fn add_parameter(&mut self, index: u32, name: SymbolId, ty: TypeId, is_final: bool) {
        if self.curinga == Some(name) {
            return;
        }
        self.parameters.push(ParamInfo {
            index,
            name,
            ty,
            is_final,
        });
    }

    /// Registra um parâmetro de tipo de função genérica no escopo.
    pub fn add_function_type_param(&mut self, name: SymbolId, param_id: TypeParamId) {
        self.function_type_params.insert(name, param_id);
    }

    /// Retorna os metadados de uma variável local a partir do seu [`LocalId`].
    pub fn get_local(&self, id: LocalId) -> Option<&LocalVarInfo> {
        for block in self.blocks.iter().rev() {
            for local in block.locals.iter().rev() {
                if local.id == id {
                    return Some(local);
                }
            }
        }
        None
    }

    /// Retorna os metadados de um parâmetro formal pelo nome.
    pub fn get_param(&self, name: SymbolId) -> Option<&ParamInfo> {
        self.parameters.iter().find(|p| p.name == name)
    }

    /// Resolve um identificador não qualificado no escopo léxico atual.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_identifier(
        &self,
        name: SymbolId,
        span: Span,
        current_library: LibraryId,
        program: &Program,
        hierarchy: &ClassHierarchy,
        interner: &dartforge_intern::Interner,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Resolved> {
        // 1. Variáveis locais (do bloco mais interno para o mais externo)
        for block in self.blocks.iter().rev() {
            for local in block.locals.iter().rev() {
                if local.ligada && local.name == name {
                    // Valida regra: variável local só é visível após sua declaração
                    if span.start < local.declaration_offset {
                        diagnostics.push(Diagnostic::new(
                            format!(
                                "{}: '{}'",
                                REFERENCED_BEFORE_DECLARATION.template,
                                interner.resolve(name)
                            ),
                            span,
                        ));
                    }
                    return Some(Resolved::Local(local.id));
                }
            }
        }

        // 2. Parâmetros formais
        for param in self.parameters.iter().rev() {
            if param.name == name {
                return Some(Resolved::Parameter {
                    index: param.index,
                    name: param.name,
                });
            }
        }

        // 3. Funções locais
        for block in self.blocks.iter().rev() {
            if let Some(&(func_id, _)) = block.local_functions.get(&name) {
                // Representado como LocalId ou Element::Function
                return Some(Resolved::Local(LocalId(func_id.0)));
            }
        }

        // 4. Parâmetros de tipo da função
        if let Some(&pid) = self.function_type_params.get(&name) {
            return Some(Resolved::TypeParameter(pid));
        }

        // 5. Membros de instância da classe envolvente (se não for contexto estático)
        if !self.is_static_context && let Some(class_id) = self.enclosing_class {
            let class_elem = &program.classes[class_id.0 as usize];

            // Busca membros declarados na classe
            if let Some(&func_id) = class_elem.instance_members.get(&name) {
                return Some(Resolved::Member {
                    class: class_id,
                    member: MemberRef::Function(func_id),
                    via_super: false,
                });
            }
            if let Some(&field_id) = class_elem.fields.iter().find(|&&f| {
                program.variables[f.0 as usize].name == name
            }) {
                return Some(Resolved::Member {
                    class: class_id,
                    member: MemberRef::Variable(field_id),
                    via_super: false,
                });
            }

            // Busca membros herdados na hierarquia
            {
                for (super_cls, _) in supertipos_ordenados(program, hierarchy, class_id) {
                    let super_elem = &program.classes[super_cls.0 as usize];
                    if let Some(&func_id) = super_elem.instance_members.get(&name) {
                        return Some(Resolved::Member {
                            class: super_cls,
                            member: MemberRef::Function(func_id),
                            via_super: false,
                        });
                    }
                    if let Some(&field_id) = super_elem.fields.iter().find(|&&f| {
                        program.variables[f.0 as usize].name == name
                    }) {
                        return Some(Resolved::Member {
                            class: super_cls,
                            member: MemberRef::Variable(field_id),
                            via_super: false,
                        });
                    }
                }
            }
        }

        // 6. Símbolos de nível superior da biblioteca (declarados e importados)
        if let Some(binding) = program.lookup(current_library, name) {
            return binding.getter.map(Resolved::Element);
        }

        // 7. Prefixos de importação da biblioteca atual
        let lib = &program.libraries[current_library.0 as usize];
        if lib.prefixes.contains_key(&name) {
            return Some(Resolved::Prefix(current_library));
        }

        None
    }
}

/// Auxiliar para busca de membros em receptores com tipo estático conhecido.
pub struct MemberResolver<'a> {
    pub program: &'a Program,
    pub outline: &'a crate::resolve::OutlineTypes,
    pub interner: &'a dartforge_intern::Interner,
    pub hierarchy: &'a ClassHierarchy,
    pub table: &'a mut TypeTable,
    pub core: &'a CoreTypes,
}

impl<'a> MemberResolver<'a> {
    pub fn new(
        program: &'a Program,
        outline: &'a crate::resolve::OutlineTypes,
        interner: &'a dartforge_intern::Interner,
        hierarchy: &'a ClassHierarchy,
        table: &'a mut TypeTable,
        core: &'a CoreTypes,
    ) -> Self {
        Self {
            program,
            outline,
            interner,
            hierarchy,
            table,
            core,
        }
    }

    /// Resolve um membro `name` em um receptor com tipo estático `receiver_ty`.
    ///
    /// Retorna `(Resolved, TipoDoMembroSubstituído)` ou `None` se não encontrado.
    pub fn lookup_member(
        &mut self,
        receiver_ty: TypeId,
        name: SymbolId,
        is_setter: bool,
        current_library: LibraryId,
    ) -> Option<(Resolved, TypeId)> {
        let t = self.table.get(receiver_ty).clone();

        match t {
            // Receptor dynamic: tudo é resolvido dinamicamente
            Type::Dynamic => {
                Some((Resolved::Dynamic, self.core.dynamic_))
            }
            // Tipo de interface de classe / mixin / enum
            Type::Interface { class, args, .. } => {
                self.lookup_in_class(class, &args, name, is_setter, false)
                    .or_else(|| self.lookup_extension_member(receiver_ty, name, is_setter, current_library))
            }
            // Tipo de extensão (extension type)
            Type::ExtensionType { decl, args, .. } => {
                self.lookup_in_extension_type(decl, &args, name, is_setter)
                    .or_else(|| self.lookup_extension_member(receiver_ty, name, is_setter, current_library))
            }
            // Tipo de função: membro `.call` resolve para o próprio tipo de função
            Type::Function { .. } => {
                let call_sym = self.interner.lookup("call");
                if Some(name) == call_sym && !is_setter {
                    Some((Resolved::Dynamic, receiver_ty))
                } else {
                    // Herda membros de Object (hashCode, toString, noSuchMethod, runtimeType)
                    if let Some(obj_cid) = self.core.object_class {
                        self.lookup_in_class(obj_cid, &[], name, is_setter, false)
                    } else {
                        None
                    }
                }
            }
            // Tipo de registro (Record): campos posicionais `$1`, `$2` ou nomeados
            Type::Record { positional, named, .. } => {
                self.lookup_in_record(&positional, &named, name)
                    .or_else(|| {
                        if let Some(rec_cid) = self.core.record_class {
                            self.lookup_in_class(rec_cid, &[], name, is_setter, false)
                        } else {
                            None
                        }
                    })
            }
            // Tipo Never: qualquer acesso tem tipo Never
            Type::Never => {
                Some((Resolved::Dynamic, self.core.never))
            }
            _ => {
                // Fallback para Object
                if let Some(obj_cid) = self.core.object_class {
                    self.lookup_in_class(obj_cid, &[], name, is_setter, false)
                        .or_else(|| self.lookup_extension_member(receiver_ty, name, is_setter, current_library))
                } else {
                    None
                }
            }
        }
    }

    /// Busca membro em uma classe e sua cadeia de herança, aplicando substituição de generics.
    pub fn lookup_in_class(
        &mut self,
        class_id: ClassId,
        args: &[TypeId],
        name: SymbolId,
        is_setter: bool,
        via_super: bool,
    ) -> Option<(Resolved, TypeId)> {
        let class_elem = &self.program.classes[class_id.0 as usize];

        // 1. Procura membro declarado diretamente na classe
        if let Some(&func_id) = class_elem.instance_members.get(&name) {
            let func_elem = &self.program.functions[func_id.0 as usize];
            if is_setter {
                if func_elem.kind == FunctionKind::Setter {
                    let member_ty = self.get_instantiated_function_type(func_id, class_id, args);
                    let val_ty = if let Type::Function { positional, .. } = self.table.get(member_ty) {
                        positional.first().copied().unwrap_or(self.core.dynamic_)
                    } else {
                        member_ty
                    };
                    return Some((
                        Resolved::Member {
                            class: class_id,
                            member: MemberRef::Function(func_id),
                            via_super,
                        },
                        val_ty,
                    ));
                }
            } else if func_elem.kind != FunctionKind::Setter {
                let member_ty = self.get_instantiated_function_type(func_id, class_id, args);
                let ty = if matches!(func_elem.kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor) {
                    if let Type::Function { ret, .. } = self.table.get(member_ty) {
                        *ret
                    } else {
                        member_ty
                    }
                } else {
                    member_ty
                };
                return Some((
                    Resolved::Member {
                        class: class_id,
                        member: MemberRef::Function(func_id),
                        via_super,
                    },
                    ty,
                ));
            }
        }

        // Procura campos / variáveis
        if let Some(&var_id) = class_elem.fields.iter().find(|&&f| {
            self.program.variables[f.0 as usize].name == name
        }) {
            let var_elem = &self.program.variables[var_id.0 as usize];
            if !is_setter || (!var_elem.final_ && !var_elem.const_) {
                let var_data = &self.outline.variables[var_id.0 as usize];
                let base_ty = var_data.declared_type.or(var_data.inferred).unwrap_or(self.core.dynamic_);
                let subst_ty = self.substitute_class_type_params(class_id, args, base_ty);
                return Some((
                    Resolved::Member {
                        class: class_id,
                        member: MemberRef::Variable(var_id),
                        via_super,
                    },
                    subst_ty,
                ));
            }
        }

        // 2. Procura na hierarquia transitiva instanciada
        {
            for (super_cls, super_ty) in supertipos_ordenados(self.program, self.hierarchy, class_id) {
                let super_elem = &self.program.classes[super_cls.0 as usize];

                if let Some(&func_id) = super_elem.instance_members.get(&name) {
                    let func_elem = &self.program.functions[func_id.0 as usize];
                    if is_setter {
                        if func_elem.kind == FunctionKind::Setter {
                            let super_args = match self.table.get(super_ty) {
                                Type::Interface { args, .. } => args.clone(),
                                _ => Box::new([]),
                            };
                            let mapping = self.build_class_substitution_map(class_id, args);
                            let final_super_args: Vec<TypeId> = super_args
                                .iter()
                                .map(|&a| substitute(a, &mapping, self.table))
                                .collect();

                            let member_ty = self.get_instantiated_function_type(
                                func_id,
                                super_cls,
                                &final_super_args,
                            );
                            let val_ty = if let Type::Function { positional, .. } = self.table.get(member_ty) {
                                positional.first().copied().unwrap_or(self.core.dynamic_)
                            } else {
                                member_ty
                            };
                            return Some((
                                Resolved::Member {
                                    class: super_cls,
                                    member: MemberRef::Function(func_id),
                                    via_super,
                                },
                                val_ty,
                            ));
                        }
                    } else if func_elem.kind != FunctionKind::Setter {
                        let super_args = match self.table.get(super_ty) {
                            Type::Interface { args, .. } => args.clone(),
                            _ => Box::new([]),
                        };
                        let mapping = self.build_class_substitution_map(class_id, args);
                        let final_super_args: Vec<TypeId> = super_args
                            .iter()
                            .map(|&a| substitute(a, &mapping, self.table))
                            .collect();

                        let member_ty = self.get_instantiated_function_type(
                            func_id,
                            super_cls,
                            &final_super_args,
                        );
                        let ty = if matches!(func_elem.kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor) {
                            if let Type::Function { ret, .. } = self.table.get(member_ty) {
                                *ret
                            } else {
                                member_ty
                            }
                        } else {
                            member_ty
                        };
                        return Some((
                            Resolved::Member {
                                class: super_cls,
                                member: MemberRef::Function(func_id),
                                via_super,
                            },
                            ty,
                        ));
                    }
                }

                if let Some(&var_id) = super_elem.fields.iter().find(|&&f| {
                    self.program.variables[f.0 as usize].name == name
                }) {
                    let var_elem = &self.program.variables[var_id.0 as usize];
                    if !is_setter || (!var_elem.final_ && !var_elem.const_) {
                        let var_data = &self.outline.variables[var_id.0 as usize];
                        let base_ty = var_data.declared_type.or(var_data.inferred).unwrap_or(self.core.dynamic_);
                        let mapping = self.build_class_substitution_map(class_id, args);
                        let subst_ty = substitute(base_ty, &mapping, self.table);
                        return Some((
                            Resolved::Member {
                                class: super_cls,
                                member: MemberRef::Variable(var_id),
                                via_super,
                            },
                            subst_ty,
                        ));
                    }
                }
            }
        }

        // 3. Receptor que é a própria classe (`C.x`): constantes de enum e
        // membros estáticos. O tipo do receptor não distingue instância de
        // literal de classe, por isso é o último recurso.
        if !via_super {
            let class_elem = &self.program.classes[class_id.0 as usize];
            if let Some(&var_id) = class_elem.enum_constants.iter().find(|&&v| self.program.variables[v.0 as usize].name == name) {
                if !is_setter {
                    let ty = self.table.intern(Type::Interface { class: class_id, args: Box::new([]), nullable: false });
                    return Some((
                        Resolved::Member { class: class_id, member: MemberRef::Variable(var_id), via_super },
                        ty,
                    ));
                }
            }
            if let Some(&func_id) = class_elem.static_members.get(&name) {
                let func_elem = &self.program.functions[func_id.0 as usize];
                if is_setter && func_elem.kind != FunctionKind::Setter && func_elem.kind != FunctionKind::ImplicitAccessor {
                    return None;
                }
                let ty = match func_elem.kind {
                    FunctionKind::ImplicitAccessor => match func_elem.variable {
                        Some(vid) => {
                            let var_data = &self.outline.variables[vid.0 as usize];
                            var_data.declared_type.or(var_data.inferred).unwrap_or(self.core.dynamic_)
                        }
                        None => self.core.dynamic_,
                    },
                    FunctionKind::Getter => self.outline.functions[func_id.0 as usize].return_type,
                    FunctionKind::Setter => self.outline.functions[func_id.0 as usize]
                        .parameters
                        .first()
                        .map(|p| p.ty)
                        .unwrap_or(self.core.dynamic_),
                    _ => self.outline.functions[func_id.0 as usize].signature,
                };
                let member = match func_elem.variable {
                    Some(vid) if func_elem.kind == FunctionKind::ImplicitAccessor => MemberRef::Variable(vid),
                    _ => MemberRef::Function(func_id),
                };
                return Some((Resolved::Member { class: class_id, member, via_super }, ty));
            }
        }

        None
    }

    /// Busca membro em um extension type.
    pub fn lookup_in_extension_type(
        &mut self,
        decl: ClassId,
        args: &[TypeId],
        name: SymbolId,
        is_setter: bool,
    ) -> Option<(Resolved, TypeId)> {
        let class_elem = &self.program.classes[decl.0 as usize];
        if class_elem.kind != ClassKind::ExtensionType {
            return None;
        }

        // Membros declarados no próprio extension type
        if let Some(&func_id) = class_elem.instance_members.get(&name) {
            let func_elem = &self.program.functions[func_id.0 as usize];
            if is_setter {
                if func_elem.kind == FunctionKind::Setter {
                    let member_ty = self.get_instantiated_function_type(func_id, decl, args);
                    let val_ty = if let Type::Function { positional, .. } = self.table.get(member_ty) {
                        positional.first().copied().unwrap_or(self.core.dynamic_)
                    } else {
                        member_ty
                    };
                    return Some((
                        Resolved::Member {
                            class: decl,
                            member: MemberRef::Function(func_id),
                            via_super: false,
                        },
                        val_ty,
                    ));
                }
            } else if func_elem.kind != FunctionKind::Setter {
                let member_ty = self.get_instantiated_function_type(func_id, decl, args);
                let ty = if matches!(func_elem.kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor) {
                    if let Type::Function { ret, .. } = self.table.get(member_ty) {
                        *ret
                    } else {
                        member_ty
                    }
                } else {
                    member_ty
                };
                return Some((
                    Resolved::Member {
                        class: decl,
                        member: MemberRef::Function(func_id),
                        via_super: false,
                    },
                    ty,
                ));
            }
        }

        // Se implements outras interfaces, busca nelas
        {
            for (super_cls, super_ty) in supertipos_ordenados(self.program, self.hierarchy, decl) {
                let super_args = match self.table.get(super_ty) {
                    Type::Interface { args, .. } => args.clone(),
                    _ => Box::new([]),
                };
                if let Some(found) = self.lookup_in_class(super_cls, &super_args, name, is_setter, false) {
                    return Some(found);
                }
            }
        }

        None
    }

    /// Busca campo em um tipo de Record por `$1` ou por nome.
    pub fn lookup_in_record(
        &self,
        positional: &[TypeId],
        named: &[(SymbolId, TypeId)],
        name: SymbolId,
    ) -> Option<(Resolved, TypeId)> {
        // Campos nomeados
        for &(sym, ty) in named {
            if sym == name {
                return Some((Resolved::Dynamic, ty));
            }
        }

        // Campos posicionais `$1`, `$2`, etc.
        let name_str = self.interner.resolve(name);
        if let Some(digits) = name_str.strip_prefix('$')
            && let Ok(idx) = digits.parse::<usize>()
            && (1..=positional.len()).contains(&idx)
        {
            return Some((Resolved::Dynamic, positional[idx - 1]));
        }

        None
    }

    /// Busca métodos de extensão aplicáveis no escopo da biblioteca para `receiver_ty`.
    pub fn lookup_extension_member(
        &mut self,
        receiver_ty: TypeId,
        name: SymbolId,
        is_setter: bool,
        current_library: LibraryId,
    ) -> Option<(Resolved, TypeId)> {
        let mut candidates = Vec::new();

        // Itera sobre as extensões visíveis na biblioteca atual
        let lib = &self.program.libraries[current_library.0 as usize];
        for entry in lib.scope.values() {
            if let Some(Element::Extension(ext_id)) = entry.getter {
                let ext_elem = &self.program.extensions[ext_id.0 as usize];
                if let Some(&func_id) = ext_elem.instance_members.get(&name) {
                    let func_elem = &self.program.functions[func_id.0 as usize];
                    if is_setter == (func_elem.kind == FunctionKind::Setter) {
                        let on_ty = self.outline.extensions[ext_id.0 as usize].on;
                        let mut env = SubtypeEnv::new(self.table, self.hierarchy, self.core);
                        if is_subtype(receiver_ty, on_ty, &mut env) {
                            candidates.push((ext_id, func_id, on_ty));
                        }
                    }
                }
            }
        }

        if candidates.is_empty() {
            return None;
        }

        // Se houver 1 candidato único, resolve
        if candidates.len() == 1 {
            let (ext_id, func_id, _) = candidates[0];
            let func_elem = &self.program.functions[func_id.0 as usize];
            let sig = self.outline.functions[func_id.0 as usize].signature;
            let ty = if is_setter {
                if let Type::Function { positional, .. } = self.table.get(sig) {
                    positional.first().copied().unwrap_or(self.core.dynamic_)
                } else {
                    sig
                }
            } else if matches!(func_elem.kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor) {
                if let Type::Function { ret, .. } = self.table.get(sig) {
                    *ret
                } else {
                    sig
                }
            } else {
                sig
            };
            return Some((
                Resolved::ExtensionMember {
                    extension: ext_id,
                    member: func_id,
                },
                ty,
            ));
        }

        // Desempate por especificidade: escolhe o tipo `on` mais específico
        let mut best_idx = 0;
        for i in 1..candidates.len() {
            let on_best = candidates[best_idx].2;
            let on_current = candidates[i].2;
            let mut env = SubtypeEnv::new(self.table, self.hierarchy, self.core);
            if is_subtype(on_current, on_best, &mut env) && !is_subtype(on_best, on_current, &mut env) {
                best_idx = i;
            }
        }

        let (ext_id, func_id, _) = candidates[best_idx];
        let func_elem = &self.program.functions[func_id.0 as usize];
        let sig = self.outline.functions[func_id.0 as usize].signature;
        let ty = if is_setter {
            if let Type::Function { positional, .. } = self.table.get(sig) {
                positional.first().copied().unwrap_or(self.core.dynamic_)
            } else {
                sig
            }
        } else if matches!(func_elem.kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor) {
            if let Type::Function { ret, .. } = self.table.get(sig) {
                *ret
            } else {
                sig
            }
        } else {
            sig
        };
        Some((
            Resolved::ExtensionMember {
                extension: ext_id,
                member: func_id,
            },
            ty,
        ))
    }

    fn build_class_substitution_map(
        &self,
        class_id: ClassId,
        args: &[TypeId],
    ) -> HashMap<TypeParamId, TypeId> {
        let formals = &self.outline.classes[class_id.0 as usize].type_params;
        let mut mapping = HashMap::with_capacity(args.len());
        for (&f, &a) in formals.iter().zip(args.iter()) {
            mapping.insert(f, a);
        }
        mapping
    }

    fn substitute_class_type_params(
        &mut self,
        class_id: ClassId,
        args: &[TypeId],
        ty: TypeId,
    ) -> TypeId {
        if args.is_empty() {
            return ty;
        }
        let mapping = self.build_class_substitution_map(class_id, args);
        substitute(ty, &mapping, self.table)
    }

    fn get_instantiated_function_type(
        &mut self,
        func_id: FunctionElementId,
        class_id: ClassId,
        args: &[TypeId],
    ) -> TypeId {
        let base_sig = self.outline.functions[func_id.0 as usize].signature;
        self.substitute_class_type_params(class_id, args, base_sig)
    }
}
