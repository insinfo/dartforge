//! Lexical scopes and resolution of identifiers and members in bodies.
//!
//! Manages the stack of lexical scopes (locals, parameters, implicit instance members,
//! library and prefixes) and the lookup of members and applicable extensions on static receivers.

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

/// Supertypes of `class` in member lookup precedence order, and
/// deterministic.
///
/// `ClassHierarchyData::supertypes` is a `HashMap`: walking it directly made
/// the lookup of an inherited member depend on the hash seed; with mixins
/// (two supertypes defining the same member) the resolution changed from one
/// run to another, and the native backend's determinism test caught it.
/// Order: the superclass chain, with the mixins of each application right
/// after it (the last mixin first: Dart's linearization); then the rest
/// (interfaces), by `ClassId`.
pub fn ordered_supertypes(
    program: &Program,
    hierarchy: &ClassHierarchy,
    class: ClassId,
) -> Vec<(ClassId, TypeId)> {
    let Some(data) = hierarchy.get(class) else {
        return Vec::new();
    };
    let mut output: Vec<(ClassId, TypeId)> = Vec::new();
    let mut seen: std::collections::HashSet<ClassId> = std::collections::HashSet::new();
    let mut push = |c: ClassId, output: &mut Vec<(ClassId, TypeId)>| {
        if let Some(&t) = data.supertypes.get(&c)
            && seen.insert(c)
        {
            output.push((c, t));
        }
    };
    let mut current = program.classes[class.0 as usize].supertype_class;
    for &m in program.classes[class.0 as usize].mixin_classes.iter().rev() {
        push(m, &mut output);
    }
    // The superclass chain may have a cycle (`class C extends C`, error
    // `recursive_interface_inheritance`): each class enters once.
    let mut chain: std::collections::HashSet<ClassId> = std::collections::HashSet::new();
    while let Some(c) = current {
        if !chain.insert(c) {
            break;
        }
        push(c, &mut output);
        for &m in program.classes[c.0 as usize].mixin_classes.iter().rev() {
            push(m, &mut output);
        }
        current = program.classes[c.0 as usize].supertype_class;
    }
    let mut rest: Vec<(ClassId, TypeId)> = data
        .supertypes
        .iter()
        .filter(|(c, _)| !output.iter().any(|(s, _)| s == *c))
        .map(|(c, t)| (*c, *t))
        .collect();
    rest.sort_by_key(|(c, _)| c.0);
    output.extend(rest);
    output
}

/// Metadata of a local variable in the lexical scope.
#[derive(Debug, Clone)]
pub struct LocalVarInfo {
    pub id: LocalId,
    pub name: SymbolId,
    pub declared_type: TypeId,
    pub is_final: bool,
    pub is_const: bool,
    pub is_late: bool,
    pub declaration_offset: usize,
    /// `false` for a wildcard (Dart 3.7): the local exists (it has a `LocalId`,
    /// the initializer runs), but the name `_` does not find it.
    pub enabled: bool,
}

/// Metadata of a formal parameter in the lexical scope.
#[derive(Debug, Clone)]
pub struct ParamInfo {
    pub index: u32,
    pub name: SymbolId,
    pub ty: TypeId,
    pub is_final: bool,
}

/// Lexical scope layer on the stack of a function body.
#[derive(Debug, Clone, Default)]
pub struct ScopeBlock {
    pub locals: Vec<LocalVarInfo>,
    pub local_functions: HashMap<SymbolId, (ast::FunctionId, TypeId)>,
}

/// Manager of lexical scopes while traversing a function or method body.
pub struct ScopeStack {
    /// Stack of nested lexical blocks (`{ ... }`).
    pub blocks: Vec<ScopeBlock>,
    /// Formal parameters of the enclosing function/method.
    pub parameters: Vec<ParamInfo>,
    /// Local generic type parameters (of the function).
    pub function_type_params: HashMap<SymbolId, TypeParamId>,
    /// Enclosing class (if inside an instance method, getter, setter or constructor).
    pub enclosing_class: Option<ClassId>,
    /// Enclosing extension (if inside an extension member).
    pub enclosing_extension: Option<ExtensionId>,
    /// Whether the current member is static (`static`).
    pub is_static_context: bool,
    /// Next ID to assign to a local variable.
    pub next_local_id: u32,
    /// The symbol `_` when the library has wildcards (Dart 3.7): a local or
    /// parameter with that name does not bind a name.
    pub wildcard: Option<SymbolId>,
}

impl ScopeStack {
    /// Creates a new scope stack for a function or method.
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
            wildcard: None,
        }
    }

    /// Enters a new nested lexical block.
    pub fn push_block(&mut self) {
        self.blocks.push(ScopeBlock::default());
    }

    /// Leaves the current lexical block.
    pub fn pop_block(&mut self) {
        if self.blocks.len() > 1 {
            self.blocks.pop();
        }
    }

    /// Declares a local variable in the innermost block.
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
            enabled: self.wildcard != Some(name),
        };
        if let Some(block) = self.blocks.last_mut() {
            block.locals.push(info);
        }
        id
    }

    /// Looks up a local variable by name in the current block.
    pub fn find_local_in_current_block(&mut self, name: SymbolId) -> Option<&mut LocalVarInfo> {
        if let Some(block) = self.blocks.last_mut() {
            block.locals.iter_mut().find(|l| l.enabled && l.name == name)
        } else {
            None
        }
    }

    /// Adds a formal parameter to the scope.
    pub fn add_parameter(&mut self, index: u32, name: SymbolId, ty: TypeId, is_final: bool) {
        if self.wildcard == Some(name) {
            return;
        }
        self.parameters.push(ParamInfo {
            index,
            name,
            ty,
            is_final,
        });
    }

    /// Registers a type parameter of a generic function in the scope.
    pub fn add_function_type_param(&mut self, name: SymbolId, param_id: TypeParamId) {
        self.function_type_params.insert(name, param_id);
    }

    /// Returns the metadata of a local variable from its [`LocalId`].
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

    /// Returns the metadata of a formal parameter by name.
    pub fn get_param(&self, name: SymbolId) -> Option<&ParamInfo> {
        self.parameters.iter().find(|p| p.name == name)
    }

    /// Resolves an unqualified identifier in the current lexical scope.
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
        // 1. Local variables (from the innermost block to the outermost)
        for block in self.blocks.iter().rev() {
            for local in block.locals.iter().rev() {
                if local.enabled && local.name == name {
                    // Enforces the rule: a local variable is only visible after its declaration
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

        // 2. Formal parameters
        for param in self.parameters.iter().rev() {
            if param.name == name {
                return Some(Resolved::Parameter {
                    index: param.index,
                    name: param.name,
                });
            }
        }

        // 3. Local functions
        for block in self.blocks.iter().rev() {
            if let Some(&(func_id, _)) = block.local_functions.get(&name) {
                // Represented as LocalId or Element::Function
                return Some(Resolved::Local(LocalId(func_id.0)));
            }
        }

        // 4. Type parameters of the function
        if let Some(&pid) = self.function_type_params.get(&name) {
            return Some(Resolved::TypeParameter(pid));
        }

        // 5. Instance members of the enclosing class (if not a static context)
        if !self.is_static_context && let Some(class_id) = self.enclosing_class {
            let class_elem = &program.classes[class_id.0 as usize];

            // Looks up members declared in the class
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

            // Looks up inherited members in the hierarchy
            {
                for (super_cls, _) in ordered_supertypes(program, hierarchy, class_id) {
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

        // 6. Top-level symbols of the library (declared and imported)
        if let Some(binding) = program.lookup(current_library, name) {
            return binding.getter.map(Resolved::Element);
        }

        // 7. Import prefixes of the current library
        let lib = &program.libraries[current_library.0 as usize];
        if lib.prefixes.contains_key(&name) {
            return Some(Resolved::Prefix(current_library));
        }

        None
    }
}

/// Helper for looking up members on receivers with a known static type.
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

    /// Resolves a member `name` on a receiver with static type `receiver_ty`.
    ///
    /// Returns `(Resolved, TipoDoMembroSubstituído)` or `None` if not found.
    pub fn lookup_member(
        &mut self,
        receiver_ty: TypeId,
        name: SymbolId,
        is_setter: bool,
        current_library: LibraryId,
    ) -> Option<(Resolved, TypeId)> {
        let t = self.table.get(receiver_ty).clone();

        match t {
            // dynamic receiver: everything is resolved dynamically
            Type::Dynamic => {
                Some((Resolved::Dynamic, self.core.dynamic_))
            }
            // Class / mixin / enum interface type
            Type::Interface { class, args, .. } => {
                self.lookup_in_class(class, &args, name, is_setter, false)
                    .or_else(|| self.lookup_extension_member(receiver_ty, name, is_setter, current_library))
            }
            // Extension type
            Type::ExtensionType { decl, args, .. } => {
                self.lookup_in_extension_type(decl, &args, name, is_setter)
                    .or_else(|| self.lookup_extension_member(receiver_ty, name, is_setter, current_library))
            }
            // Function type: member `.call` resolves to the function type itself
            Type::Function { .. } => {
                let call_sym = self.interner.lookup("call");
                if Some(name) == call_sym && !is_setter {
                    Some((Resolved::Dynamic, receiver_ty))
                } else {
                    // Inherits members from Object (hashCode, toString, noSuchMethod, runtimeType)
                    if let Some(obj_cid) = self.core.object_class {
                        self.lookup_in_class(obj_cid, &[], name, is_setter, false)
                    } else {
                        None
                    }
                }
            }
            // Record type: positional fields `$1`, `$2` or named ones
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
            // Never type: any access has type Never
            Type::Never => {
                Some((Resolved::Dynamic, self.core.never))
            }
            _ => {
                // Fallback to Object
                if let Some(obj_cid) = self.core.object_class {
                    self.lookup_in_class(obj_cid, &[], name, is_setter, false)
                        .or_else(|| self.lookup_extension_member(receiver_ty, name, is_setter, current_library))
                } else {
                    None
                }
            }
        }
    }

    /// Looks up a member in a class and its inheritance chain, applying generic substitution.
    pub fn lookup_in_class(
        &mut self,
        class_id: ClassId,
        args: &[TypeId],
        name: SymbolId,
        is_setter: bool,
        via_super: bool,
    ) -> Option<(Resolved, TypeId)> {
        // T1.1 d: the interface of a same-named class is that of the group's
        // owner (the analyzer's `InheritanceManager3._interfaces` is per element).
        let class_id = self.program.dono_da_classe(class_id);
        let class_elem = &self.program.classes[class_id.0 as usize];

        // 1. Looks for a member declared directly in the class
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

        // Looks for fields / variables
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

        // 2. Looks in the instantiated transitive hierarchy
        {
            for (super_cls, super_ty) in ordered_supertypes(self.program, self.hierarchy, class_id) {
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

        // 3. A receiver that is the class itself (`C.x`): enum constants and
        // static members. The receiver's type does not distinguish an instance
        // from a class literal, so this is the last resort.
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

    /// Looks up a member in an extension type.
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

        // Members declared in the extension type itself
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

        // If it implements other interfaces, looks in them
        {
            for (super_cls, super_ty) in ordered_supertypes(self.program, self.hierarchy, decl) {
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

    /// Looks up a field in a Record type by `$1` or by name.
    pub fn lookup_in_record(
        &self,
        positional: &[TypeId],
        named: &[(SymbolId, TypeId)],
        name: SymbolId,
    ) -> Option<(Resolved, TypeId)> {
        // Named fields
        for &(sym, ty) in named {
            if sym == name {
                return Some((Resolved::Dynamic, ty));
            }
        }

        // Positional fields `$1`, `$2`, etc.
        let name_str = self.interner.resolve(name);
        if let Some(digits) = name_str.strip_prefix('$')
            && let Ok(idx) = digits.parse::<usize>()
            && (1..=positional.len()).contains(&idx)
        {
            return Some((Resolved::Dynamic, positional[idx - 1]));
        }

        None
    }

    /// Looks up applicable extension methods in the library scope for `receiver_ty`.
    pub fn lookup_extension_member(
        &mut self,
        receiver_ty: TypeId,
        name: SymbolId,
        is_setter: bool,
        current_library: LibraryId,
    ) -> Option<(Resolved, TypeId)> {
        let mut candidates = Vec::new();

        // Iterates over the extensions visible in the current library
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

        // If there is a single candidate, resolves it
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

        // Tie-break by specificity: picks the most specific `on` type
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

/// `LibraryFragment.shouldIgnoreUndefined`
/// (`an611:src/dart/element/element.dart:1186-1221`): a name that does not
/// resolve is not reported when it may come from an import that does not
/// exist (with the same prefix and without `show`, or in a `show` that cites
/// it) or, without a prefix and starting with `_$`, from a generated part
/// that does not exist yet. The library already has another error (that of
/// the import or the part), so the program does not pass as valid.
pub fn should_ignore_undefined(
    program: &Program,
    interner: &dartforge_intern::Interner,
    unit: dartforge_elements::model::UnitId,
    the_prefix: Option<SymbolId>,
    the_name: SymbolId,
) -> bool {
    let lib_id = program.unit(unit).library;
    let lib = program.library(lib_id);
    for &uid in &lib.units {
        let u = program.unit(uid);
        for (i, d) in u.unit.directives.iter().enumerate() {
            match &d.kind {
                ast::DirectiveKind::Import { prefix, combinators, .. } => {
                    if prefix.map(|p| p.sym) != the_prefix {
                        continue;
                    }
                    // `importedLibrary?.isSynthetic != false`: no library loaded
                    // for the directive.
                    let synthetic = !lib
                        .imports
                        .iter()
                        .any(|im| im.unit == uid && im.directive == i && !program.library(im.library).units.is_empty());
                    if !synthetic {
                        continue;
                    }
                    let mut shows = combinators.iter().filter_map(|c| match c {
                        ast::Combinator::Show(v) => Some(v),
                        ast::Combinator::Hide(_) => None,
                    });
                    let mut some_show = false;
                    for v in shows.by_ref() {
                        some_show = true;
                        if v.iter().any(|n| n.sym == the_name) {
                            return true;
                        }
                    }
                    if the_prefix.is_some() && !some_show {
                        return true;
                    }
                }
                ast::DirectiveKind::Part { uri } if the_prefix.is_none() && interner.resolve(the_name).starts_with("_$") => {
                    let Some(text) = dartforge_elements::load::string_lit_value(uri) else { continue };
                    // `file_paths.isGenerated`, for a part whose file does not exist.
                    let generated = [".g.dart", ".pb.dart", ".pbenum.dart", ".pbserver.dart", ".pbjson.dart", ".template.dart"]
                        .iter()
                        .any(|s| text.ends_with(s));
                    let exists =
                        text.contains(':') || u.path.as_ref().and_then(|p| p.parent()).is_none_or(|d| d.join(&text).is_file());
                    if generated && !exists {
                        return true;
                    }
                }
                _ => {}
            }
        }
    }
    false
}

/// The `{1}` of `AMBIGUOUS_IMPORT` (`ErrorVerifier._checkForAmbiguousImport`
/// and `_getLibraryName`, 3.6.2 `error_verifier.dart:2116-2130`,
/// `:6278-6322`): the elements that the imports (with `the_prefix`) bring in for
/// `the_name`, each by the URI of the library that declares it, with ` (via …)`
/// for the imports that re-export it when it is not imported directly;
/// sorted, quoted, joined with `and`.
pub fn ambiguous_libraries(
    program: &Program,
    unit: dartforge_elements::model::UnitId,
    the_prefix: Option<SymbolId>,
    the_name: SymbolId,
) -> String {
    use dartforge_elements::model::Element;
    let lib = program.unit(unit).library;
    let imports = &program.library(lib).imports;
    let passes = |cs: &[ast::Combinator]| {
        cs.iter().all(|c| match c {
            ast::Combinator::Show(ns) => ns.iter().any(|n| n.sym == the_name),
            ast::Combinator::Hide(ns) => !ns.iter().any(|n| n.sym == the_name),
        })
    };
    let mut elements: Vec<Element> = Vec::new();
    for i in imports.iter().filter(|i| i.prefix == the_prefix && passes(&i.combinators)) {
        if let Some(b) = program.library(i.library).exported.get(&the_name) {
            if let Some(el) = b.getter.or(b.setter)
                && !elements.contains(&el)
            {
                elements.push(el);
            }
        }
    }
    let library = |el: Element| match el {
        Element::Class(c) => Some(program.class(c).library),
        Element::Extension(x) => Some(program.extension(x).library),
        Element::Typedef(t) => Some(program.typedefs[t.0 as usize].library),
        Element::Function(f) => Some(program.function(f).library),
        Element::Variable(v) => Some(program.variable(v).library),
        Element::Prefix(..) => None,
    };
    let mut names: Vec<String> = Vec::new();
    for el in elements {
        let Some(l) = library(el) else { continue };
        let mut text = program.library(l).uri.clone();
        if !imports.iter().any(|i| i.library == l) {
            let mut indirect: Vec<String> = imports
                .iter()
                .filter(|i| passes(&i.combinators) && program.library(i.library).exported.get(&the_name).and_then(|b| b.getter.or(b.setter)) == Some(el))
                .map(|i| program.library(i.library).uri.clone())
                .collect();
            if !indirect.is_empty() {
                text.push_str(" (via ");
                if indirect.len() > 1 {
                    indirect.sort();
                    text.push_str(&quotes_with_and(&indirect));
                } else {
                    text.push_str(&indirect[0]);
                }
                text.push(')');
            }
        }
        names.push(text);
    }
    names.sort();
    quotes_with_and(&names)
}

/// `quotedAndCommaSeparatedWithAnd`: `'a'`, `'a' and 'b'`, `'a', 'b' and 'c'`.
fn quotes_with_and(names: &[String]) -> String {
    let q: Vec<String> = names.iter().map(|n| format!("'{n}'")).collect();
    match q.len() {
        0 => String::new(),
        1 => q[0].clone(),
        n => format!("{} and {}", q[..n - 1].join(", "), q[n - 1]),
    }
}
