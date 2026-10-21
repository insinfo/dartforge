//! Class members in the native lowering: object layout, fields,
//! calls (direct and with dispatch by class), constructors and globals.
//!
//! Contract: `docs/NATIVO-PLANO.md` §6 — R7 (layout and indices by the resolved
//! element), N5 (constructor as a function) and N6 (lazy globals).

use super::fn_builder::FnBuilder;
use crate::context::Context;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::Element;
use dartforge_elements::model::{
    ClassId, FunctionElementId, FunctionKind, FunctionRef, UnitId, VariableId,
};
use dartforge_frontend::ast::{self, ExprId, FunctionBody, MemberKind, ParameterKind};
use dartforge_intern::SymbolId;
use dartforge_types::resolved::MemberRef;
use dartforge_types::table::{TypeId, TypeParamId};

/// An already evaluated argument: name (if named) and value.
pub type Evaluated = (Option<SymbolId>, Operand);

/// The class is a `mixin` (not instantiable; its members enter the classes
/// that apply it).
pub fn is_mixin(ctx: &Context, cid: ClassId) -> bool {
    ctx.program.classes[cid.0 as usize].kind == dartforge_elements::model::ClassKind::Mixin
}

/// The linearization of a program class (P4, specification §12.3 "Mixin
/// Application"): `class C extends B with M1, M2` is `C`, `M2`, `M1` and then
/// the linearization of `B` — the order in which a member is looked up. Stops at the
/// SDK. The element does not synthesize the class `B&M1`: the application is read here.
pub fn linearization(ctx: &Context, cid: ClassId) -> Vec<ClassId> {
    let mut exit = Vec::new();
    let mut current = Some(cid);
    while let Some(c) = current {
        let class_ref = &ctx.program.classes[c.0 as usize];
        if !ctx.compiled_library(class_ref.library) || exit.contains(&c) {
            break;
        }
        exit.push(c);
        for m in class_ref.mixin_classes.iter().rev() {
            if ctx.compiled_library(ctx.program.classes[m.0 as usize].library) && !exit.contains(m)
            {
                exit.push(*m);
            }
        }
        current = class_ref.supertype_class;
    }
    exit
}

/// **Instance** fields of the whole user linearization (superclasses and mixins),
/// from the root to the leaf (R7).
///
/// `ClassElement::fields` mixes instance and static fields; the layout
/// only has the instance ones. SDK superclasses do not contribute fields: the
/// runtime does not have the SDK objects in our layout. The fields of a mixin
/// sit between those of the superclass and those of the class that applies it.
pub fn layout(ctx: &Context, cid: ClassId) -> Vec<VariableId> {
    let chain = linearization(ctx, cid);
    let mut fields = Vec::new();
    for c in chain.into_iter().rev() {
        for &vid in &ctx.program.classes[c.0 as usize].fields {
            if !ctx.program.variables[vid.0 as usize].static_ {
                fields.push(vid);
            }
        }
    }
    fields
}

/// Declaration and layout premises of one resolved managed field, independent of its values.
pub(super) fn declared_field_metadata(
    ctx: &Context,
    vid: VariableId,
    layout_index: usize,
) -> Option<FieldDeclaration> {
    let variable = ctx.program.variable(vid);
    let owner = variable.class?;
    let class = ctx.program.class(owner);
    if variable.static_
        || variable.external
        || class.library != variable.library
        || ctx.program.class_with_homonym(owner)
        || ctx.program.library_has_augmentations(variable.library)
        || !class.fields.contains(&vid)
        || class
            .fields
            .iter()
            .filter(|&&field| ctx.program.variable(field).name == variable.name)
            .count()
            != 1
    {
        return None;
    }
    let storage_type = super::layouts_arc::representation(ctx, vid);
    if matches!(storage_type, Type::Void | Type::Ptr) || storage_type.is_vector() {
        return None;
    }
    Some(FieldDeclaration {
        library_uri: ctx.program.library(variable.library).uri.clone(),
        owner: ctx.symbol_name(class.name).to_owned(),
        name: ctx.symbol_name(variable.name).to_owned(),
        is_final: variable.final_,
        is_late: variable.late,
        layout_index,
        storage_type,
    })
}

/// Declared (or inferred) type of a variable/field.
pub fn variable_type(ctx: &Context, vid: VariableId) -> TypeId {
    let data = &ctx.outline.variables[vid.0 as usize];
    data.declared_type
        .or(data.inferred)
        .unwrap_or(ctx.core.dynamic_)
}

/// `sub` is `sup` or a nominal subtype of it (superclass, mixin or interface).
pub fn subclass_of(ctx: &Context, sub: ClassId, sup: ClassId) -> bool {
    if sub == sup {
        return true;
    }
    ctx.outline
        .hierarchy
        .get(sub)
        .is_some_and(|d| d.supertypes.contains_key(&sup))
}

/// How many distinct implementations the dispatch by class of a program
/// member (`call_member`) accepts as a `switch` of direct calls; above that,
/// the selector — the VM's polymorphic inline cache holds four.
const DEVIRTUALIZATION_FAN_OUT: usize = 4;

/// `DARTFORGE_NO_DEVIRTUALIZATION_CAP=1`: the `switch` whatever the fan-out
/// (ablation for measurement).
fn devirtualization_cap_off() -> bool {
    static OFF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OFF.get_or_init(|| {
        std::env::var_os("DARTFORGE_NO_DEVIRTUALIZATION_CAP")
            .is_some_and(|v| !v.is_empty() && v != "0")
    })
}

/// Does the function have a body that the lowering compiles?
pub fn has_body(ctx: &Context, fid: usize) -> bool {
    let f = &ctx.program.functions[fid];
    match f.node {
        // An SDK `external` is implemented by the patch or by the native
        // (`sdk_source::call_external`).
        FunctionRef::Function { .. } if f.external => true,
        FunctionRef::Function { unit, function } => !matches!(
            ctx.program.unit(unit).ast.function(function).body,
            FunctionBody::Empty | FunctionBody::Native(_)
        ),
        FunctionRef::Constructor { .. } => true,
        FunctionRef::None => f.kind == FunctionKind::SyntheticConstructor,
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Representation of a Dart type (R1). `void` in a value position
    /// becomes `Ref` (the value is always null).
    pub fn repr(&self, ty: TypeId) -> Type {
        match self.ctx.to_hir_type(ty) {
            Type::Void => Type::Ref,
            t => t,
        }
    }

    /// Neutral value of a representation: an optional argument without a default
    /// (always of a nullable type, hence a null `Ref`) and an uninitialized field.
    pub fn zero_value(ty: Type) -> Operand {
        match ty {
            Type::I64 => Operand::Constant(Constant::Int(0)),
            Type::F64 => Operand::Constant(Constant::Double(0.0)),
            Type::I1 | Type::I8 => Operand::Constant(Constant::Bool(false)),
            _ => Operand::Constant(Constant::Null),
        }
    }

    /// Uses the resolved declaration, including inherited fields, rather than the receiver type.
    fn tracing_field_declaration(
        &self,
        vid: VariableId,
        object: &Operand,
    ) -> Option<FieldDeclaration> {
        if self.ctx.arc_memory || self.operand_type(object) != Type::Ref {
            return None;
        }
        let owner = self.ctx.program.variable(vid).class?;
        if is_mixin(self.ctx, owner) {
            return None;
        }
        declared_field_metadata(self.ctx, vid, self.field_index(vid)?)
    }

    /// Position of the field in the layout of the class that declares it (R7). The layout of
    /// a subclass extends that of the superclass, so the index holds for
    /// any receiver whose static class is a subtype of the declaring one.
    pub fn field_index(&self, vid: VariableId) -> Option<usize> {
        let owner = self.ctx.program.variables[vid.0 as usize].class?;
        if is_mixin(self.ctx, owner) {
            // The index of a mixin field depends on the class that applies it
            // (`mixin_field_indices`).
            return None;
        }
        let base = super::enums::layout_base(self.ctx, owner);
        layout(self.ctx, owner)
            .iter()
            .position(|&v| v == vid)
            .map(|i| i + base)
    }

    /// Representation of a field in the object: that of the type (R1), except the scalar
    /// `late` field with an initializer, which lives in a box (`Ref`) — null
    /// is "not initialized yet" (P4; the VM uses the `_sentinel` sentinel).
    pub fn field_repr(&self, vid: VariableId) -> Type {
        super::layouts_arc::representation(self.ctx, vid)
    }

    /// Converts the `i64` bits read from the heap to the representation `repr`.
    pub fn bits_to(&mut self, bits: Operand, repr: Type) -> Operand {
        match repr {
            Type::F64 => self.emit(
                Instruction::Bitcast {
                    op: bits,
                    to: Type::F64,
                },
                Type::F64,
            ),
            Type::I1 => self.emit(
                Instruction::ICmp(ICmpOp::Ne, bits, Operand::Constant(Constant::Int(0))),
                Type::I1,
            ),
            _ => bits,
        }
    }

    /// `i64` bits of a value, to store in the heap, and whether it is a reference
    /// (E1: `is_ref` comes from the representation, never from a literal).
    pub fn to_bits(&mut self, op: Operand) -> (Operand, bool) {
        match self.operand_type(&op) {
            Type::F64 => (
                self.emit(Instruction::Bitcast { op, to: Type::I64 }, Type::I64),
                false,
            ),
            Type::I1 => (
                self.emit(
                    Instruction::ZExt {
                        op,
                        from: Type::I1,
                        to: Type::I64,
                    },
                    Type::I64,
                ),
                false,
            ),
            Type::I8 => (
                self.emit(
                    Instruction::ZExt {
                        op,
                        from: Type::I8,
                        to: Type::I64,
                    },
                    Type::I64,
                ),
                false,
            ),
            Type::Ref => (op, true),
            _ => (op, false),
        }
    }

    /// The index of a mixin field in each concrete class that applies it.
    fn mixin_field_indices(&self, vid: VariableId) -> Vec<(i64, usize)> {
        let mut exit = Vec::new();
        for (k, class_ref) in self.ctx.program.classes.iter().enumerate() {
            let kid = ClassId(k as u32);
            if !self.ctx.compiled_library(class_ref.library) || is_mixin(self.ctx, kid) {
                continue;
            }
            let base = super::enums::layout_base(self.ctx, kid);
            if let (Some(id), Some(i)) = (
                self.ctx.class_id_of(kid),
                layout(self.ctx, kid).iter().position(|&v| v == vid),
            ) {
                exit.push((id.into(), i + base));
            }
        }
        exit
    }

    /// Index of a mixin field by the dynamic class of the object (the code
    /// of the mixin is compiled once; the position of the field is that of the application).
    fn dynamic_index(&mut self, obj: Operand, vid: VariableId, span: Span) -> Option<Operand> {
        let owner = self.ctx.program.variables[vid.0 as usize].class?;
        if !is_mixin(self.ctx, owner) {
            return None;
        }
        let cases = self.mixin_field_indices(vid);
        if cases.is_empty() {
            // No class of the program applies the mixin: there is no object
            // with this field, and the access does not run (the `LocatorMixin` of
            // `state_notifier`, which riverpod imports and nobody applies).
            // The program is closed — no new class appears later.
            let _ = span;
            self.terminate(Terminator::Unreachable);
            let dead = self.new_block();
            self.set_block(dead);
            return Some(Operand::Constant(Constant::Int(0)));
        }
        let cls = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(obj, Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let join = self.new_block();
        let blocks: Vec<(i64, usize, BlockId)> = cases
            .iter()
            .map(|&(c, i)| (c, i, self.new_block()))
            .collect();
        self.terminate(Terminator::Switch {
            val: cls,
            default: blocks[0].2,
            cases: blocks.iter().map(|&(c, _, b)| (c, b)).collect(),
        });
        let mut entries = Vec::new();
        for &(_, i, b) in &blocks {
            self.set_block(b);
            entries.push((b, Operand::Constant(Constant::Int(i as i64))));
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(join);
        Some(self.emit(
            Instruction::Phi {
                incoming: entries,
                ty: Type::I64,
            },
            Type::I64,
        ))
    }

    // Under ARC, chooses the physical access in the branch of each application of the mixin.
    // The join carries the typed result, never a raw index whose read
    // would lose the Ref map. The guards/owners are produced later by the HIR.
    fn access_arc_mixin_field(
        &mut self,
        obj: Operand,
        vid: VariableId,
        val_ref: Option<Operand>,
    ) -> Option<Operand> {
        let owner = self.ctx.program.variables[vid.0 as usize].class?;
        if !is_mixin(self.ctx, owner) {
            return None;
        }
        let cases = self.mixin_field_indices(vid);
        if cases.is_empty() {
            self.terminate(Terminator::Unreachable);
            let dead = self.new_block();
            self.set_block(dead);
            return Some(Operand::Constant(Constant::Null));
        }
        let repr = self.field_repr(vid);
        let cls = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".into(),
                args: vec![(obj.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let join = self.new_block();
        let blocks: Vec<_> = cases
            .iter()
            .map(|&(c, i)| (c, i, self.new_block()))
            .collect();
        self.terminate(Terminator::Switch {
            val: cls,
            default: blocks[0].2,
            cases: blocks.iter().map(|&(c, _, b)| (c, b)).collect(),
        });
        let mut entries = Vec::new();
        for &(_, index, b) in &blocks {
            self.set_block(b);
            if let Some(value) = &val_ref {
                self.emit(
                    Instruction::SetField {
                        object: obj.clone(),
                        index,
                        value: value.clone(),
                    },
                    Type::Void,
                );
            } else {
                let v = self.emit(
                    Instruction::GetField {
                        object: obj.clone(),
                        index,
                    },
                    repr,
                );
                entries.push((b, v));
            }
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(join);
        Some(if val_ref.is_some() {
            Operand::Constant(Constant::Null)
        } else {
            self.emit(
                Instruction::Phi {
                    incoming: entries,
                    ty: repr,
                },
                repr,
            )
        })
    }

    /// Reads an instance field in the representation of its declared type.
    pub fn read_field(&mut self, obj: Operand, vid: VariableId, span: Span) -> Operand {
        if let Some(r) = self.read_ffi_field(obj.clone(), vid) {
            return r;
        }
        if self.ctx.arc_memory
            && !self.ctx.program.variables[vid.0 as usize].late
            && self.field_index(vid).is_none()
            && let Some(v) = self.access_arc_mixin_field(obj.clone(), vid, None)
        {
            return v;
        }
        let idx = match self.field_index(vid) {
            Some(i) => Operand::Constant(Constant::Int(i as i64)),
            None => match self.dynamic_index(obj.clone(), vid, span) {
                Some(i) => i,
                None => return self.unsupported("field outside the object layout", span),
            },
        };
        let repr = self.field_repr(vid);
        if self.ctx.arc_memory
            && self.ctx.program.variables[vid.0 as usize].late
            && repr == Type::Ref
        {
            // The probe before the late guard may find zero without a Ref
            // mark. The ABI accepts only that empty state or a real edge.
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_arc_read_field_late_ref_v1".into(),
                    args: vec![(obj, Type::Ref), (idx, Type::I64)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        }
        if self.ctx.arc_memory
            && !self.ctx.program.variables[vid.0 as usize].late
            && let Operand::Constant(Constant::Int(i)) = &idx
            && let Ok(index) = usize::try_from(*i)
        {
            // Carries the representation without choosing an audited ABI:
            // the origin of the receiver and the guards are still checked later.
            return self.emit(Instruction::GetField { object: obj, index }, repr);
        }
        let ret = if repr == Type::Ref {
            Type::Ref
        } else {
            Type::I64
        };
        let declaration = self.tracing_field_declaration(vid, &obj);
        let bits = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_get".to_string(),
                args: vec![(obj, Type::Ref), (idx, Type::I64)],
                ret_ty: ret,
            }
            .with_field_declaration(declaration),
            ret,
        );
        self.bits_to(bits, repr)
    }

    /// Stores an instance field; `is_ref` by the representation (E1).
    pub fn store_field(&mut self, obj: Operand, vid: VariableId, val: Operand, span: Span) {
        if self.store_ffi_field(obj.clone(), vid, val.clone()) {
            return;
        }
        let var = &self.ctx.program.variables[vid.0 as usize];
        if self.ctx.arc_memory
            && !var.late
            && self.field_index(vid).is_none()
            && var.class.is_some_and(|owner| is_mixin(self.ctx, owner))
        {
            let repr = self.field_repr(vid);
            let value = self.coerce(val.clone(), repr);
            if self
                .access_arc_mixin_field(obj.clone(), vid, Some(value))
                .is_some()
            {
                return;
            }
        }
        let idx = match self.field_index(vid) {
            Some(i) => Operand::Constant(Constant::Int(i as i64)),
            None => match self.dynamic_index(obj.clone(), vid, span) {
                Some(i) => i,
                None => {
                    self.unsupported("field outside the object layout", span);
                    return;
                }
            },
        };
        let var = &self.ctx.program.variables[vid.0 as usize];
        let late_without_init = var.late && self.variable_initializer_in(vid).is_none();
        let late_final = late_without_init && var.final_;
        let name_str = self.ctx.symbol_name(var.name).to_string();
        if late_final {
            let already_initialized = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_late_field_initialized".to_string(),
                    args: vec![(obj.clone(), Type::Ref), (idx.clone(), Type::I64)],
                    ret_ty: Type::I8,
                },
                Type::I8,
            );
            let already_initialized = self.emit(
                Instruction::ICmp(
                    ICmpOp::Ne,
                    already_initialized,
                    Operand::Constant(Constant::Int(0)),
                ),
                Type::I1,
            );
            let b_error = self.new_block();
            let b_store = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: already_initialized,
                then_block: b_error,
                else_block: b_store,
            });
            self.set_block(b_error);
            self.throw_late_error(&name_str, 1);
            self.set_block(b_store);
        }
        let repr = self.field_repr(vid);
        let val = self.coerce(val, repr);
        if self.ctx.arc_memory
            && let Operand::Constant(Constant::Int(i)) = &idx
            && let Ok(index) = usize::try_from(*i)
        {
            // The typed write also holds for late; the side state only
            // changes after storing, including for null/false/zero.
            self.emit(
                Instruction::SetField {
                    object: obj.clone(),
                    index,
                    value: val,
                },
                Type::Void,
            );
        } else {
            let (bits, is_ref) = self.to_bits(val);
            let declaration = self.tracing_field_declaration(vid, &obj).filter(|field| {
                self.operand_type(&bits)
                    == if field.storage_type == Type::Ref {
                        Type::Ref
                    } else {
                        Type::I64
                    }
            });
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_object_set".to_string(),
                    args: vec![
                        (obj.clone(), Type::Ref),
                        (idx.clone(), Type::I64),
                        (bits, Type::I64),
                        (
                            Operand::Constant(Constant::Int(i64::from(is_ref))),
                            Type::I8,
                        ),
                    ],
                    ret_ty: Type::Void,
                }
                .with_field_declaration(declaration),
                Type::Void,
            );
        }
        if var.late {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_late_field_mark_initialized".to_string(),
                    args: vec![(obj, Type::Ref), (idx, Type::I64)],
                    ret_ty: Type::Void,
                },
                Type::Void,
            );
        }
    }

    /// Instance field by name, in the class itself (target of `this.x` and
    /// of the initializer list).
    pub fn own_field(&self, cid: ClassId, name_str: SymbolId) -> Option<VariableId> {
        self.ctx.program.classes[cid.0 as usize]
            .fields
            .iter()
            .copied()
            .find(|&v| {
                let var = &self.ctx.program.variables[v.0 as usize];
                !var.static_ && var.name == name_str
            })
    }

    /// User class of the static type of an expression, if it is one.
    pub fn user_class_of(&self, e: ExprId) -> Option<ClassId> {
        let ty = self.ctx.get_type(self.unit_id, e)?;
        match self.ctx.table.get(ty) {
            dartforge_types::table::Type::Interface { class, .. } => {
                let class_ref = &self.ctx.program.classes[class.0 as usize];
                (self.ctx.compiled_library(class_ref.library)).then_some(*class)
            }
            _ => None,
        }
    }

    /// User class member accessed in `recv.name` (R7).
    ///
    /// First the element that inference resolved for the access; if it
    /// did not resolve it (a nullable receiver of `?.` or `x!`, from which the member
    /// lookup of `types` does not strip the `?` yet), the STATIC class of the
    /// receiver — never the first member with that name in the program.
    pub fn user_member(
        &self,
        access: ExprId,
        recv: ExprId,
        name_str: SymbolId,
        setter: bool,
    ) -> Option<(ClassId, MemberRef)> {
        if let Some(dartforge_types::resolved::Resolved::Member { class, member, .. }) =
            self.ctx.get_resolved(self.unit_id, access)
        {
            let class_ref = &self.ctx.program.classes[class.0 as usize];
            // A member inherited from the SDK (`index`/`name` of an enum, `hashCode`,
            // `toString` of `Object`…) in a program class: it is not a
            // compiled member — it goes through the SDK paths.
            let member_lib = match member {
                MemberRef::Function(f) => self.ctx.program.functions[f.0 as usize].library,
                MemberRef::Variable(v) => self.ctx.program.variables[v.0 as usize].library,
            };
            if !self.ctx.compiled_library(member_lib) {
                return None;
            }
            // The resolution of `o.x = v` may point to the getter `x`.
            // In that case the write needs to look up the distinct entry
            // `x_=` (or the implicit setter of a field) in the hierarchy.
            let is_getter = matches!(member, MemberRef::Function(f)
                if self.ctx.program.functions[f.0 as usize].kind != FunctionKind::Setter
                    && self.ctx.program.functions[f.0 as usize].variable.is_none());
            if !(setter && is_getter) {
                return (self.ctx.compiled_library(class_ref.library)).then_some((*class, *member));
            }
        }
        let cid = self.user_class_of(recv)?;
        for c in crate::lower::members::linearization(self.ctx, cid) {
            let class_ref = &self.ctx.program.classes[c.0 as usize];
            if !self.ctx.compiled_library(class_ref.library) {
                return None;
            }
            if let Some(&f) = class_ref.instance_members.get(&name_str) {
                let func = &self.ctx.program.functions[f.0 as usize];
                if !setter && func.kind != FunctionKind::Setter {
                    return Some((c, MemberRef::Function(f)));
                }
            }
            if setter {
                let setter_name = format!("{}_=", self.ctx.symbol_name(name_str));
                if let Some(s) = self.ctx.interner.lookup(&setter_name)
                    && let Some(&f) = class_ref.instance_members.get(&s)
                {
                    return Some((c, MemberRef::Function(f)));
                }
            }
            if let Some(&v) = class_ref.fields.iter().find(|&&v| {
                let var = &self.ctx.program.variables[v.0 as usize];
                !var.static_ && var.name == name_str
            }) {
                return Some((c, MemberRef::Variable(v)));
            }
        }
        None
    }

    /// `E` of a `List<E>`/`Iterable<E>` (the first type argument).
    pub fn element_type(&self, ty: TypeId) -> Option<TypeId> {
        let dartforge_types::table::Type::Interface { class, args, .. } = self.ctx.table.get(ty)
        else {
            return None;
        };
        let iter = self.ctx.core.iterable_class?;
        if *class == iter || self.ctx.core.list_class == Some(*class) {
            return args.first().copied();
        }
        // A subtype of `Iterable<E>` (e.g. `Runes`): `E` comes from the instantiated
        // supertype; with no arguments to substitute, it is already the final type.
        let data = self.ctx.outline.hierarchy.get(*class)?;
        let sup = *data.supertypes.get(&iter)?;
        if !data.type_params.is_empty() {
            return None;
        }
        match self.ctx.table.get(sup) {
            dartforge_types::table::Type::Interface { args, .. } => args.first().copied(),
            _ => None,
        }
    }

    /// Instance member by name, going up the superclass chain.
    /// The operator `name` (`+`, `[]`, `unary-`…) declared by a class of the
    /// PROGRAM (not of the SDK) for the receiver of static type `e`: the call
    /// goes through `call_member` (direct, or the dispatch only among the
    /// program's implementations), not through the dynamic selector.
    pub fn program_operator(&self, e: ExprId, name_str: &str) -> Option<usize> {
        let cid = self.user_class_of(e)?;
        if self
            .ctx
            .program
            .library(self.ctx.program.classes[cid.0 as usize].library)
            .is_sdk
        {
            return None;
        }
        let fid = self.member_in_class(cid, name_str)?;
        let f = &self.ctx.program.functions[fid];
        (crate::lower::user_function(self.ctx, fid) && !self.ctx.program.library(f.library).is_sdk)
            .then_some(fid)
    }

    pub fn member_in_class(&self, cid: ClassId, name_str: &str) -> Option<usize> {
        let sym = self.ctx.interner.lookup(name_str)?;
        for c in crate::lower::members::linearization(self.ctx, cid) {
            let class_ref = &self.ctx.program.classes[c.0 as usize];
            if let Some(&f) = class_ref.instance_members.get(&sym) {
                return Some(f.0 as usize);
            }
        }
        None
    }

    // ------------------------------------------------------------------
    // Reading identifiers

    /// Class id used in the runtime type tests: the `class_id` of a
    /// user object, or the code that `dartforge_value_class` returns
    /// for the values that the runtime represents on its own.
    pub fn class_id_of(&self, cid: ClassId) -> Option<i64> {
        let class_ref = &self.ctx.program.classes[cid.0 as usize];
        if self.ctx.compiled_library(class_ref.library) {
            return self.ctx.class_id_of(cid).map(i64::from);
        }
        Some(match self.ctx.symbol_name(class_ref.name) {
            "String" => -2,
            "List" => -3,
            "Map" => -4,
            "Set" => -5,
            "Function" => -6,
            "Record" => -7,
            "StringBuffer" => -8,
            "int" => -9,
            "double" => -10,
            "bool" => -11,
            "Object" => 0,
            "Exception" => 1000,
            "FormatException" => 1001,
            "StateError" => 1002,
            "ArgumentError" => 1003,
            "RangeError" => 1004,
            "UnsupportedError" => 1005,
            "StackTrace" => 1006,
            "Error" => 1007,
            "UnimplementedError" => 1008,
            "AssertionError" => 1009,
            "ConcurrentModificationError" => 1010,
            "TypeError" => 1011,
            "NoSuchMethodError" => 1012,
            // The `Type` object of the RTI (`rti.rs`, the type class constant).
            "Type" => super::rti::TYPE_CLASS,
            _ => return None,
        })
    }

    /// Identifier that the resolution says is a member of the enclosing class
    /// (`x` = `this.x`, or a static member).
    pub fn read_implicit_member(&mut self, member: MemberRef, span: Span) -> Operand {
        // Implicit `values` of a program enum, read without the `E.`
        // prefix in the body of a member of the enum: the list of the values, like the
        // explicit path (`E.values`, `enum_values`). The implicit
        // getter has no body to call — emitting the call left an
        // undefined symbol at link time.
        if let MemberRef::Function(f) = member {
            let fid = f.0 as usize;
            let (is_static, name_str, cid) = {
                let func = &self.ctx.program.functions[fid];
                (
                    func.static_,
                    self.ctx.symbol_name(func.name).to_string(),
                    func.class,
                )
            };
            if is_static
                && name_str == "values"
                && !has_body(self.ctx, fid)
                && let Some(cid) = cid
                && super::enums::is_enum(self.ctx, cid)
            {
                return self.enum_values(cid, span);
            }
        }
        let vid = match member {
            MemberRef::Variable(v) => Some(v),
            MemberRef::Function(f) => self.ctx.program.functions[f.0 as usize].variable,
        };
        if let Some(vid) = vid {
            if super::is_global(self.ctx, vid) {
                return self.read_global(vid, span);
            }
            let Some(this) = self.this_param.clone() else {
                return self.unsupported("instance field outside an instance member", span);
            };
            return self.read_field_with_late(this, vid, span);
        }
        let MemberRef::Function(f) = member else {
            unreachable!()
        };
        let fid = f.0 as usize;
        let func = &self.ctx.program.functions[fid];
        if func.kind != FunctionKind::Getter {
            if !super::user_function(self.ctx, fid) {
                return self.unsupported("SDK method tear-off", span);
            }
            if func.static_ {
                return self.function_tearoff(fid);
            }
            let Some(this) = self.this_param.clone() else {
                return self.unsupported("method tear-off outside an instance member", span);
            };
            return self.method_tearoff(this, fid, span);
        }
        if func.static_ {
            if !super::user_function(self.ctx, fid) {
                return self.unsupported("SDK static getter", span);
            }
            return self.call_direct(fid, None, Vec::new());
        }
        let Some(this) = self.this_param.clone() else {
            return self.unsupported("instance getter outside an instance member", span);
        };
        self.call_member(this, fid, &[], span)
    }

    /// `C.x` with `C` a class literal: a static field or a static getter.
    pub fn read_static_member(&mut self, member: MemberRef, span: Span) -> Operand {
        let vid = match member {
            MemberRef::Variable(v) => Some(v),
            MemberRef::Function(f) => self.ctx.program.functions[f.0 as usize].variable,
        };
        if let Some(vid) = vid {
            if super::is_global(self.ctx, vid) {
                return self.read_global(vid, span);
            }
            return self.unsupported("enum constant", span);
        }
        let MemberRef::Function(f) = member else {
            unreachable!()
        };
        let fid = f.0 as usize;
        if self.ctx.program.functions[fid].kind == FunctionKind::Getter
            && super::user_function(self.ctx, fid)
        {
            return self.call_direct(fid, None, Vec::new());
        }
        if super::user_function(self.ctx, fid) && self.ctx.program.functions[fid].static_ {
            return self.function_tearoff(fid);
        }
        self.unsupported("static member tear-off", span)
    }

    /// Identifier that resolves to a top-level element.
    pub fn read_element(&mut self, el: Element, span: Span) -> Operand {
        match el {
            Element::Variable(vid) => self.read_global(vid, span),
            Element::Function(f) => {
                let fid = f.0 as usize;
                let func = &self.ctx.program.functions[fid];
                if let Some(vid) = func.variable {
                    return self.read_global(vid, span);
                }
                if func.kind == FunctionKind::Getter && super::user_function(self.ctx, fid) {
                    return self.call_direct(fid, None, Vec::new());
                }
                if super::user_function(self.ctx, fid) && func.kind != FunctionKind::Setter {
                    return self.function_tearoff(fid);
                }
                self.unsupported("function tear-off", span)
            }
            // Type literal (`Fish`, `List`): the canonical `Type` object of the
            // raw type of the class (RTI).
            Element::Class(cid) => {
                let n = self
                    .ctx
                    .outline
                    .classes
                    .get(cid.0 as usize)
                    .map_or(0, |d| d.type_params.len());
                let mut r = super::rti::Recipe {
                    text: format!("C{}", self.ctx.id_rti(cid)),
                    variables: false,
                };
                if n > 0 {
                    r.text.push('<');
                    r.text.push_str(&vec!["D"; n].join(","));
                    r.text.push('>');
                }
                let t = self.recipe_rti(&r);
                self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_rti_object_type".to_string(),
                        args: vec![(t, Type::I64)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )
            }
            _ => self.unsupported("top-level element", span),
        }
    }

    /// Field read; if `late` with an initializer, initializes on the first read.
    pub fn read_field_with_late(&mut self, obj: Operand, vid: VariableId, span: Span) -> Operand {
        if let Some(r) = self.read_source_field(obj.clone(), vid) {
            return r;
        }
        self.read_field_with_late_direct(obj, vid, span)
    }

    /// `super.field` has lexical resolution: it preserves `late`, but cannot
    /// go back to the overridden getter of the dynamic class through the SDK selector.
    pub fn read_field_with_late_direct(
        &mut self,
        obj: Operand,
        vid: VariableId,
        span: Span,
    ) -> Operand {
        let var = &self.ctx.program.variables[vid.0 as usize];
        if !var.late {
            return self.read_field(obj, vid, span);
        }
        let dartforge_elements::model::VariableRef::Field { .. } = var.node else {
            return self.read_field(obj, vid, span);
        };
        if self.variable_initializer_in(vid).is_some() {
            // Inline fast path (see `non_null_late_is_initialized`): the
            // non-null value comes out directly; the getter (which initializes) is only
            // called with the field still null.
            let fast = self.non_null_late_is_initialized(vid).then(|| {
                let current = self.read_field(obj.clone(), vid, span);
                let non_null = self.emit(
                    Instruction::ICmp(
                        ICmpOp::Ne,
                        current.clone(),
                        Operand::Constant(Constant::Null),
                    ),
                    Type::I1,
                );
                let b_join = self.new_block();
                let b_getter = self.new_block();
                let of = self.current_block;
                self.terminate(Terminator::CondBranch {
                    cond: non_null,
                    then_block: b_join,
                    else_block: b_getter,
                });
                self.set_block(b_getter);
                (current, of, b_join)
            });
            let v = self.emit_call_with_check(
                Instruction::CallStatic {
                    symbol: super::late_field_getter_symbol(self.ctx, vid),
                    args: vec![obj],
                    ret_ty: self.field_repr(vid),
                },
                self.field_repr(vid),
            );
            let Some((current, of, b_join)) = fast else {
                return v;
            };
            let incoming = if self.is_terminated() {
                vec![(of, current)]
            } else {
                let b = self.current_block;
                self.terminate(Terminator::Branch(b_join));
                vec![(of, current), (b, v)]
            };
            self.set_block(b_join);
            return self.emit(
                Instruction::Phi {
                    incoming,
                    ty: Type::Ref,
                },
                Type::Ref,
            );
        }
        let current = self.read_field(obj.clone(), vid, span);
        if self.non_null_late_is_initialized(vid) {
            // Fast path: a non-null value ⇒ initialized; only null consults
            // the runtime (which distinguishes "not initialized").
            let non_null = self.emit(
                Instruction::ICmp(
                    ICmpOp::Ne,
                    current.clone(),
                    Operand::Constant(Constant::Null),
                ),
                Type::I1,
            );
            let b_ready = self.new_block();
            let b_check = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: non_null,
                then_block: b_ready,
                else_block: b_check,
            });
            self.set_block(b_check);
            self.check_late_initialized(obj, vid, span);
            if !self.is_terminated() {
                self.terminate(Terminator::Branch(b_ready));
            }
            self.set_block(b_ready);
            return current;
        }
        self.check_late_initialized(obj, vid, span);
        current
    }

    /// A `late` field without an initializer, of a non-nullable interface type and
    /// kept as a reference: in it the stored value is never `null`, and the
    /// field of a new object is zero (`null`) — so "non-null" already proves
    /// "initialized", without the runtime side table (a call with a
    /// `HashSet` lookup per read, which sat in the body of loops).
    fn non_null_late_is_initialized(&self, vid: VariableId) -> bool {
        let ty = variable_type(self.ctx, vid);
        // The natural representation of the type (not the forced one of `late` with an
        // initializer): `int`/`double`/`bool` stay out.
        if self.field_repr(vid) != Type::Ref || self.repr(ty) != Type::Ref {
            return false;
        }
        matches!(
            self.ctx.table.get(ty),
            dartforge_types::table::Type::Interface {
                nullable: false,
                ..
            }
        ) && ty != self.ctx.core.dynamic_
    }

    /// Checks in the runtime that the `late` field `vid` of `obj` was initialized
    /// and throws the `LateInitializationError` if it was not; ends in the block of the
    /// read, without terminating it.
    fn check_late_initialized(&mut self, obj: Operand, vid: VariableId, span: Span) {
        let idx = match self.field_index(vid) {
            Some(i) => Operand::Constant(Constant::Int(i as i64)),
            None => match self.dynamic_index(obj.clone(), vid, span) {
                Some(i) => i,
                None => {
                    self.unsupported("late field outside the layout", span);
                    return;
                }
            },
        };
        let initialized = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_late_field_initialized".to_string(),
                args: vec![(obj, Type::Ref), (idx, Type::I64)],
                ret_ty: Type::I8,
            },
            Type::I8,
        );
        let empty = self.emit(
            Instruction::ICmp(ICmpOp::Eq, initialized, Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let b_error = self.new_block();
        let b_read = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: empty,
            then_block: b_error,
            else_block: b_read,
        });
        self.set_block(b_error);
        let name_str = self
            .ctx
            .symbol_name(self.ctx.program.variables[vid.0 as usize].name)
            .to_string();
        self.throw_late_error(&name_str, 0);
        self.set_block(b_read);
    }

    /// Single body of the getter of a `late` field with an initializer. The state
    /// lives per object, including when the assigned value is `null`.
    pub fn lower_late_field_getter(&mut self, obj: Operand, vid: VariableId, span: Span) {
        // Dart body called by the reader of the field: the Ref return is Owned.
        if self.ctx.arc_memory {
            self.dart_ref_returns.insert(self.func.symbol.clone());
        }
        let Some(init) = self.variable_initializer_in(vid) else {
            unreachable!("late getter without an initializer")
        };
        let dartforge_elements::model::VariableRef::Field { unit, .. } =
            self.ctx.program.variables[vid.0 as usize].node
        else {
            unreachable!("late getter without a field")
        };
        let idx = match self.field_index(vid) {
            Some(i) => Operand::Constant(Constant::Int(i as i64)),
            None => match self.dynamic_index(obj.clone(), vid, span) {
                Some(i) => i,
                None => {
                    self.unsupported("late field outside the layout", span);
                    self.terminate(Terminator::Return(Some(Operand::Constant(Constant::Null))));
                    return;
                }
            },
        };
        if self.non_null_late_is_initialized(vid) {
            // Fast path (see `non_null_late_is_initialized`): a non-null value
            // is already the initialized value.
            let current = self.read_field(obj.clone(), vid, span);
            let non_null = self.emit(
                Instruction::ICmp(
                    ICmpOp::Ne,
                    current.clone(),
                    Operand::Constant(Constant::Null),
                ),
                Type::I1,
            );
            let b_fast = self.new_block();
            let b_slow = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: non_null,
                then_block: b_fast,
                else_block: b_slow,
            });
            self.set_block(b_fast);
            self.terminate(Terminator::Return(Some(current)));
            self.set_block(b_slow);
        }
        let ready = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_late_field_initialized".to_string(),
                args: vec![(obj.clone(), Type::Ref), (idx.clone(), Type::I64)],
                ret_ty: Type::I8,
            },
            Type::I8,
        );
        let ready = self.emit(
            Instruction::ICmp(ICmpOp::Ne, ready, Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let b_read = self.new_block();
        let b_verify = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: ready,
            then_block: b_read,
            else_block: b_verify,
        });
        self.set_block(b_read);
        let current = self.read_field(obj.clone(), vid, span);
        self.terminate(Terminator::Return(Some(current)));

        self.set_block(b_verify);
        let starting = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_late_field_initializing".to_string(),
                args: vec![(obj.clone(), Type::Ref), (idx.clone(), Type::I64)],
                ret_ty: Type::I8,
            },
            Type::I8,
        );
        let starting = self.emit(
            Instruction::ICmp(ICmpOp::Ne, starting, Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let b_stack = self.new_block();
        let b_evaluate = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: starting,
            then_block: b_stack,
            else_block: b_evaluate,
        });
        self.set_block(b_stack);
        let error = self.new_runtime_error("dartforge_stack_overflow_error_new", Vec::new());
        self.emit_throw_op(error);

        self.set_block(b_evaluate);
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_late_field_set_initializing".to_string(),
                args: vec![
                    (obj.clone(), Type::Ref),
                    (idx.clone(), Type::I64),
                    (Operand::Constant(Constant::Int(1)), Type::I8),
                ],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        let b_fail = self.new_block();
        self.exception_targets.push(b_fail);
        let val_ref = self.lower_expr_of(unit, init);
        self.exception_targets.pop();
        let val_ref = self.coerce(val_ref, self.field_repr(vid));
        self.store_field(obj.clone(), vid, val_ref.clone(), span);
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_late_field_set_initializing".to_string(),
                args: vec![
                    (obj.clone(), Type::Ref),
                    (idx.clone(), Type::I64),
                    (Operand::Constant(Constant::Int(0)), Type::I8),
                ],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        self.terminate(Terminator::Return(Some(val_ref)));
        self.set_block(b_fail);
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_late_field_set_initializing".to_string(),
                args: vec![
                    (obj, Type::Ref),
                    (idx, Type::I64),
                    (Operand::Constant(Constant::Int(0)), Type::I8),
                ],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        self.terminate(Terminator::Return(Some(Operand::Constant(Constant::Null))));
    }

    // ------------------------------------------------------------------
    // Argumentos

    /// Evaluates the arguments in text order (Dart's evaluation order).
    ///
    /// Each `dynamic` argument gets, right after being evaluated, the implicit
    /// cast to the type of the parameter in the invocation built for this
    /// list (`arm_arg_types`, `typed_entry.rs`), like the CFE.
    pub fn evaluate_args(&mut self, ast: &ast::Ast, args: &[ast::Argument]) -> Vec<Evaluated> {
        let types = match &self.arg_types {
            Some((key, _)) if *key == args.as_ptr() as usize && !args.is_empty() => {
                self.arg_types.take().map(|(_, t)| t)
            }
            _ => None,
        };
        let mut i_pos = 0;
        args.iter()
            .map(|a| {
                let v = self.lower_expr(ast, a.value);
                let name_str = a.name.map(|n| n.sym);
                if !self.is_terminated() {
                    self.implicit_argument_cast(types.as_ref(), a.value, name_str, i_pos, &v);
                }
                if name_str.is_none() {
                    i_pos += 1;
                }
                (name_str, v)
            })
            .collect()
    }

    /// Parameters as written in the declaration (for default values).
    fn ast_parameters(&self, fid: usize) -> Option<(UnitId, &'a [ast::Parameter])> {
        let program = self.ctx.program;
        match program.functions[fid].node {
            FunctionRef::Function { unit, function } => program
                .unit(unit)
                .ast
                .function(function)
                .parameters
                .as_deref()
                .map(|p| (unit, p)),
            FunctionRef::Constructor { unit, member } => {
                match &program.unit(unit).ast.member(member).kind {
                    MemberKind::Constructor(c) => Some((unit, &c.parameters[..])),
                    _ => None,
                }
            }
            FunctionRef::None => None,
        }
    }

    /// The parameters of `fid` in the AST (the unit and the list).
    pub fn parameters_of(&self, fid: usize) -> Option<(UnitId, &'a [ast::Parameter])> {
        self.ast_parameters(fid)
    }

    /// Lowers an expression of another unit (a parameter default value).
    ///
    /// `lower_expr` uses `self.unit_id` for the types and resolutions; switching the
    /// unit while the expression is lowered is what makes this safe
    /// (docs/NATIVO-PLANO.md §5.1). Default values are constants: they do not read
    /// locals of the callee.
    pub fn lower_expr_of(&mut self, unit: UnitId, expr: ExprId) -> Operand {
        let saved = self.unit_id;
        self.unit_id = unit;
        let ast = &self.ctx.program.unit(unit).ast;
        let v = self.lower_expr(ast, expr);
        self.unit_id = saved;
        v
    }

    /// Default value of parameter `i` of `fid`, or the one inherited from the constructor of the
    /// superclass when the parameter is `super.x` without a default of its own.
    pub fn default_value(&mut self, fid: usize, i: usize) -> Option<Operand> {
        let (unit, params) = self.ast_parameters(fid)?;
        let p = params.get(i)?;
        if let Some(e) = p.default_value {
            // The default of a function of the source SDK called from outside its
            // module: the resolutions of the SDK unit do not exist here;
            // the SDK module exports the value (`lower_sdk_patterns`).
            if exported_pattern(self.ctx, fid, unit, e) {
                return Some(self.emit_call_with_check(
                    Instruction::CallStatic {
                        symbol: pattern_symbol(self.ctx, fid, i),
                        args: Vec::new(),
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ));
            }
            // The default value is constant (§9.2.2): const context.
            let saved = std::mem::replace(&mut self.in_const_context, true);
            let v = self.lower_expr_of(unit, e);
            self.in_const_context = saved;
            return Some(v);
        }
        // Redirecting factory (`factory C({x}) = D;`): its parameter
        // has no default of its own and takes that of the corresponding parameter of the
        // target (specification, "Redirecting Factory Constructors"; the CFE
        // copies the defaults of the effective target).
        if let Some(target) = self.redirected_target(fid) {
            let target_params = &self.ctx.outline.functions.get(target)?.parameters;
            let j = if p.kind == ParameterKind::Named {
                target_params.iter().position(|sp| {
                    sp.kind == ParameterKind::Named && sp.external_name == p.name.map(|n| n.sym)
                })?
            } else {
                // The positional ones come before the named ones, in the same order.
                i
            };
            return self.default_value(target, j);
        }
        if !p.super_ {
            return None;
        }
        // `super.x` without a default inherits that of the corresponding parameter of the
        // superclass constructor.
        let (sup_fid, explicit_positionals) = self.super_constructor_of(fid)?;
        let sup_params = &self.ctx.outline.functions.get(sup_fid)?.parameters;
        let target = if p.kind == ParameterKind::Named {
            sup_params
                .iter()
                .position(|sp| sp.external_name == p.name.map(|n| n.sym))?
        } else {
            let k = params[..i]
                .iter()
                .filter(|q| q.super_ && q.kind != ParameterKind::Named)
                .count();
            let positionals: Vec<usize> = sup_params
                .iter()
                .enumerate()
                .filter(|(_, sp)| sp.kind != ParameterKind::Named)
                .map(|(j, _)| j)
                .collect();
            *positionals.get(explicit_positionals + k)?
        };
        self.default_value(sup_fid, target)
    }

    /// The constructor to which the factory `fid` redirects
    /// (`factory C(…) = D.name;`), resolved in the unit that declares it.
    fn redirected_target(&mut self, fid: usize) -> Option<usize> {
        let FunctionRef::Constructor { unit, member } = self.ctx.program.functions.get(fid)?.node
        else {
            return None;
        };
        let ast = &self.ctx.program.unit(unit).ast;
        let MemberKind::Constructor(c) = &ast.member(member).kind else {
            return None;
        };
        let r = c.redirect.as_ref()?;
        let saved = self.unit_id;
        self.unit_id = unit;
        let target = self.type_constructor(ast.ty(r.ty), r.constructor.map(|n| n.sym));
        self.unit_id = saved;
        target.map(|f| f.0 as usize).filter(|&a| a != fid)
    }

    /// Superclass constructor called by a constructor (explicit in the
    /// initializer list or the implicit unnamed one), and how many
    /// explicit positional arguments it receives.
    fn super_constructor_of(&self, fid: usize) -> Option<(usize, usize)> {
        let f = &self.ctx.program.functions[fid];
        let cid = f.class?;
        let sup = self.ctx.program.classes[cid.0 as usize].supertype_class?;
        let sup_class = &self.ctx.program.classes[sup.0 as usize];
        let FunctionRef::Constructor { unit, member } = f.node else {
            return None;
        };
        let MemberKind::Constructor(c) = &self.ctx.program.unit(unit).ast.member(member).kind
        else {
            return None;
        };
        let explicit = c.initializers.iter().find_map(|i| match i {
            ast::Initializer::Super {
                constructor,
                arguments,
                ..
            } => Some((*constructor, arguments)),
            _ => None,
        });
        let empty = self.ctx.interner.lookup("")?;
        match explicit {
            Some((name_str, args)) => {
                let s = name_str.map(|n| n.sym).unwrap_or(empty);
                let sf = sup_class.constructors.get(&s)?;
                Some((
                    sf.0 as usize,
                    args.args.iter().filter(|a| a.name.is_none()).count(),
                ))
            }
            None => sup_class
                .constructors
                .get(&empty)
                .map(|sf| (sf.0 as usize, 0)),
        }
    }

    /// Matches evaluated arguments with the parameters of `fid`: positional ones in
    /// order, named ones by name, missing ones by the default value (N5).
    pub fn match_args(&mut self, fid: usize, evaluated: &[Evaluated]) -> Vec<Operand> {
        self.call_shape = Some((
            fid,
            super::js_members::CallShape {
                positional: evaluated.iter().filter(|(n, _)| n.is_none()).count(),
                named: evaluated.iter().filter_map(|(n, _)| *n).collect(),
            },
        ));
        let Some(data) = self.ctx.outline.functions.get(fid) else {
            return evaluated.iter().map(|(_, v)| v.clone()).collect();
        };
        let params: Vec<(Option<SymbolId>, TypeId, ParameterKind)> = data
            .parameters
            .iter()
            // The external name matches that of the argument (private named parameter of 3.12).
            .map(|p| (p.external_name, p.ty, p.kind))
            .collect();
        let mut positionals = evaluated
            .iter()
            .filter(|(n, _)| n.is_none())
            .map(|(_, v)| v.clone());
        let mut exit = Vec::with_capacity(params.len());
        for (i, (name_str, ty, kind)) in params.into_iter().enumerate() {
            let datum = if kind == ParameterKind::Named {
                evaluated
                    .iter()
                    .find(|(n, _)| n.is_some() && *n == name_str)
                    .map(|(_, v)| v.clone())
            } else {
                positionals.next()
            };
            let repr = self.repr(ty);
            let v = match datum {
                Some(v) => v,
                None => self
                    .default_value(fid, i)
                    .unwrap_or_else(|| Self::zero_value(repr)),
            };
            exit.push(self.coerce(v, repr));
        }
        exit
    }

    /// Representation of the return of a function (Void for a constructor).
    pub fn return_repr(&self, fid: usize) -> Type {
        if super::generative_constructor(self.ctx, fid) {
            return Type::Void;
        }
        // Extension type constructor: the representation.
        let f = &self.ctx.program.functions[fid];
        if matches!(f.node, FunctionRef::Constructor { .. })
            && let Some(c) = f.class.filter(|&c| self.ctx.is_extension_type(c))
        {
            return self
                .ctx
                .te
                .this
                .get(&c)
                .map_or(Type::Ref, |&t| self.repr(t));
        }
        self.ctx.hir_return(fid)
    }

    /// Direct call to a user function, with `this` when it is a member.
    pub fn call_direct(
        &mut self,
        fid: usize,
        this: Option<Operand>,
        args: Vec<Operand>,
    ) -> Operand {
        if let Some((target, shape)) = self.redirected_ffi_call(fid) {
            // `Pointer.fromFunction`, `NativeCallable.*`, `Struct.create`: the
            // helper of the overlay has the same parameters
            // (`lower/ffi.rs`).
            let mut args = args;
            let saved = self.armed_tuple.clone();
            match shape {
                super::ffi::Redirection::DirectOnly => {}
                super::ffi::Redirection::ClassTuple => self.armed_tuple = args.pop(),
                super::ffi::Redirection::WithSite => {
                    // The site, stable across compilations (symbol of the function
                    // and order in the body).
                    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
                    for b in self
                        .func
                        .symbol
                        .bytes()
                        .chain(self.callback_sites.to_le_bytes())
                    {
                        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
                    }
                    self.callback_sites += 1;
                    args.push(Operand::Constant(Constant::Int((h >> 1) as i64)));
                }
            }
            let r = self.call_direct(target, this, args);
            self.armed_tuple = saved;
            return r;
        }
        if let Some(r) = self.call_external(fid, this.clone(), &args) {
            return r;
        }
        let symbol = super::symbol_of(self.ctx, fid);
        let ret_ty = self.return_repr(fid);
        let mut all = Vec::with_capacity(args.len() + 2);
        all.extend(this);
        all.extend(args);
        // RTI: the generic function receives the tuple of the type arguments in the
        // last parameter (that of the current call, or 0 = `dynamic`).
        if self.generic_function(fid) {
            all.push(
                self.armed_tuple
                    .clone()
                    .unwrap_or(Operand::Constant(Constant::Int(0))),
            );
        }
        let r = self.emit_call_with_check(
            Instruction::CallStatic {
                symbol,
                args: all,
                ret_ty,
            },
            ret_ty,
        );
        if ret_ty == Type::Void {
            Operand::Constant(Constant::Null)
        } else {
            r
        }
    }

    /// Concrete implementations of an instance member, per user class
    /// that is a subtype of the declaring class.
    fn implementations(&self, decl_fid: usize) -> Vec<(u32, usize)> {
        let ctx = self.ctx;
        let decl = &ctx.program.functions[decl_fid];
        let Some(cdecl) = decl.class else {
            return Vec::new();
        };
        // Getters and setters share `FunctionElement.name`, but the
        // outline keeps the setter under the distinct key `x_=`. Dispatching by
        // `x` would call the getter and discard the assigned value.
        let name_str = if decl.kind == FunctionKind::Setter {
            let key = format!("{}_=", ctx.symbol_name(decl.name));
            let Some(sym) = ctx.interner.lookup(&key) else {
                return Vec::new();
            };
            sym
        } else {
            decl.name
        };
        let mut exit = Vec::new();
        for &kid in ctx.subtypes(cdecl).iter() {
            let class_ref = &ctx.program.classes[kid.0 as usize];
            if !ctx.compiled_library(class_ref.library) {
                continue;
            }
            if (class_ref.modifiers.abstract_ && kid != cdecl) || is_mixin(ctx, kid) {
                continue;
            }
            for c in crate::lower::members::linearization(self.ctx, kid) {
                let cl = &ctx.program.classes[c.0 as usize];
                if let Some(&f) = cl.instance_members.get(&name_str) {
                    let f = f.0 as usize;
                    if super::user_function(ctx, f) && has_body(ctx, f) {
                        if let Some(id) = ctx.class_id_of(kid) {
                            exit.push((id, f));
                        }
                        break;
                    }
                }
            }
        }
        exit
    }

    /// `recv.<member>(evaluated)` through the selector of the declared member
    /// `decl_fid` (the method table of the dynamic class, with the receiver's
    /// `noSuchMethod` on a miss, as in the VM), with the result in `ret`.
    fn call_member_by_selector(
        &mut self,
        recv: Operand,
        decl_fid: usize,
        evaluated: &[Evaluated],
        ret: Type,
    ) -> Operand {
        use super::sdk_source::AccessKind;
        let f = &self.ctx.program.functions[decl_fid];
        let ty = match f.kind {
            FunctionKind::Getter => AccessKind::Read,
            FunctionKind::Setter => AccessKind::Write,
            FunctionKind::ImplicitAccessor if evaluated.len() == 1 => AccessKind::Write,
            FunctionKind::ImplicitAccessor => AccessKind::Read,
            _ => AccessKind::Call,
        };
        let name_str = self.ctx.symbol_name(f.name).to_string();
        let s = super::sdk_source::selector_string(self.ctx, ty, &name_str, f.library);
        // Guaranteed arguments: the entry that only checks the covariant ones
        // (`typed_entry.rs`), as the SDK dispatch of `call_source_member` does.
        let s = if matches!(ty, AccessKind::Call | AccessKind::Write) && self.all_checked(evaluated)
        {
            super::typed_entry::typed_selector(&s)
        } else {
            s
        };
        let tuple = if self.generic_function(decl_fid) {
            self.armed_tuple
                .clone()
                .unwrap_or(Operand::Constant(Constant::Int(0)))
        } else {
            Operand::Constant(Constant::Int(0))
        };
        // The VM's forwarder passes all the parameters of the member,
        // with the defaults of the omitted optional ones.
        let complete_set: Vec<Evaluated> = if matches!(ty, AccessKind::Call) {
            let values = self.match_args(decl_fid, evaluated);
            self.ctx.outline.functions[decl_fid]
                .parameters
                .iter()
                .zip(values)
                .map(|(p, v)| {
                    (
                        if p.kind == ParameterKind::Named {
                            p.external_name
                        } else {
                            None
                        },
                        v,
                    )
                })
                .collect()
        } else {
            evaluated.to_vec()
        };
        let r = self.call_by_selector_with_tuple(recv, s, &complete_set, tuple);
        if ret == Type::Void {
            Operand::Constant(Constant::Null)
        } else {
            self.coerce(r, ret)
        }
    }

    /// Call of an instance method/getter/setter with dispatch by the dynamic class
    /// of the receiver when some subclass overrides the member.
    ///
    /// There is no vtable yet: a `switch` over `dartforge_value_class`
    /// chooses the implementation (up to `DEVIRTUALIZATION_FAN_OUT`
    /// implementations; above that, the selector). Each target matches the
    /// arguments with ITS parameters (defaults and representation may differ
    /// between overrides) and the result is coerced to the representation of
    /// the declared member.
    pub fn call_member(
        &mut self,
        recv: Operand,
        decl_fid: usize,
        evaluated: &[Evaluated],
        _span: Span,
    ) -> Operand {
        if let Some(r) = self.call_source_member(recv.clone(), decl_fid, evaluated) {
            return r;
        }
        let impls = self.implementations(decl_fid);
        let mut distinct: Vec<usize> = impls.iter().map(|(_, f)| *f).collect();
        distinct.sort_unstable();
        distinct.dedup();
        let ret = self.return_repr(decl_fid);
        if distinct.is_empty() {
            // Implicit getter `index`/`name` of a program enum
            // (specification §13 "Enums"): the resolved element has no body
            // — the value lives in the two implicit fields of the object (positions 0
            // and 1, stored by the constructor of the value). The explicit path
            // (`E.a.name`, `enum_member`) reads the same fields; the implicit one
            // (`$name`, `index`, `this.name` in the body of a member of the enum)
            // arrived here as "no compiled implementation".
            if evaluated.is_empty()
                && let Some(cid) = self.ctx.program.functions[decl_fid].class
                && super::enums::is_enum(self.ctx, cid)
            {
                let name_str = self
                    .ctx
                    .symbol_name(self.ctx.program.functions[decl_fid].name)
                    .to_string();
                if (name_str == "index" || name_str == "name")
                    && let Some(r) = self.enum_member(cid, &name_str, recv.clone())
                {
                    return self.coerce(r, ret);
                }
            }
            if has_body(self.ctx, decl_fid) && super::user_function(self.ctx, decl_fid) {
                distinct.push(decl_fid);
            } else {
                // No class of the program implements the member: whatever
                // reaches it can only be a `noSuchMethod` forwarder
                // (a class with `noSuchMethod` that does not declare it). Through the
                // selector, the miss in the table reaches the `noSuchMethod` of the
                // receiver with the `Invocation`, as in the VM.
                return self.call_member_by_selector(recv, decl_fid, evaluated, ret);
            }
        }
        // A megamorphic member (more implementations than the VM's polymorphic
        // inline cache holds) stays on the selector: the `switch` below with
        // one direct call per implementation, each one a candidate of the
        // inliner, made `RenderObjectElement.performRebuild` (137
        // `updateRenderObject`s) 22 KB for a one-line body
        // (docs/PLANO-NATIVO-TAMANHO-PERFORMANCE.md). The selector call costs
        // the inline cache, like the `switch` costs its jump table.
        // `DARTFORGE_NO_DEVIRTUALIZATION_CAP=1` keeps the switch (measurement).
        if distinct.len() > DEVIRTUALIZATION_FAN_OUT && !devirtualization_cap_off() {
            return self.call_member_by_selector(recv, decl_fid, evaluated, ret);
        }
        if distinct.len() == 1 {
            let target = distinct[0];
            self.check_target_covariants(target, &recv, evaluated);
            let args = self.match_args(target, evaluated);
            let r = self.call_direct(target, Some(recv), args);
            return if ret == Type::Void {
                r
            } else {
                self.coerce(r, ret)
            };
        }
        let cls = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(recv.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let join = self.new_block();
        let pattern_fid = if distinct.contains(&decl_fid) {
            decl_fid
        } else {
            distinct[0]
        };
        let mut blocks: Vec<(usize, BlockId)> = Vec::new();
        for &f in &distinct {
            let b = self.new_block();
            blocks.push((f, b));
        }
        let block_of = |f: usize| {
            blocks
                .iter()
                .find(|(g, _)| *g == f)
                .map(|(_, b)| *b)
                .expect("target block")
        };
        let cases: Vec<(i64, BlockId)> = impls
            .iter()
            .map(|(cid, f)| (i64::from(*cid), block_of(*f)))
            .collect();
        self.terminate(Terminator::Switch {
            val: cls,
            default: block_of(pattern_fid),
            cases,
        });
        let mut entries: Vec<(BlockId, Operand)> = Vec::new();
        for (f, b) in blocks.clone() {
            self.set_block(b);
            self.check_target_covariants(f, &recv, evaluated);
            let args = self.match_args(f, evaluated);
            let r = self.call_direct(f, Some(recv.clone()), args);
            let r = if ret == Type::Void {
                r
            } else {
                self.coerce(r, ret)
            };
            if !self.is_terminated() {
                entries.push((self.current_block, r));
                self.terminate(Terminator::Branch(join));
            }
        }
        self.set_block(join);
        if ret == Type::Void || entries.is_empty() {
            return Operand::Constant(Constant::Null);
        }
        self.emit(
            Instruction::Phi {
                incoming: entries,
                ty: ret,
            },
            ret,
        )
    }

    // ------------------------------------------------------------------
    // Construtores (N5)

    /// `C(args)`: allocates with the layout of the class and calls the constructor.
    pub fn instantiate(
        &mut self,
        ast: &ast::Ast,
        ctor_fid: FunctionElementId,
        args: &[ast::Argument],
        span: Span,
    ) -> Operand {
        let fid = ctor_fid.0 as usize;
        let f = &self.ctx.program.functions[fid];
        let Some(cid) = f.class else {
            return self.unsupported("constructor without a class", span);
        };
        // Environment factories are evaluated by the native compiler, while
        // their argument expressions retain normal evaluation order outside const.
        if let Some((kind, has)) = self.environment_factory(ctor_fid) {
            if self.in_const_context
                && args
                    .iter()
                    .find(|a| a.name.is_none())
                    .is_some_and(|a| self.environment_key(self.unit_id, a.value, 0).is_none())
            {
                return self.unsupported("constant compilation environment name", span);
            }
            let value = args
                .iter()
                .find(|a| a.name.is_none())
                .and_then(|a| self.environment_value(self.unit_id, a.value, kind, has));
            let evaluated = self.evaluate_args(ast, args);
            if let Some(value) = value {
                let ty = match value {
                    Constant::Bool(_) => Type::I1,
                    _ => Type::Ref,
                };
                return self.emit(Instruction::Const(value), ty);
            }
            if has {
                return self.emit(Instruction::Const(Constant::Bool(false)), Type::I1);
            }
            if let Some(default) = evaluated.iter().find(|(name, _)| {
                name.is_some_and(|name| self.ctx.symbol_name(name) == "defaultValue")
            }) {
                return default.1.clone();
            }
            return match kind {
                "bool" => self.emit(Instruction::Const(Constant::Bool(false)), Type::I1),
                "int" => self.emit(Instruction::Const(Constant::Int(0)), Type::I64),
                _ => self.emit(
                    Instruction::Const(Constant::String(String::new())),
                    Type::Ref,
                ),
            };
        }
        // RTI: the static type of the creation (`C<T…>`), stored by the caller.
        let ty = self.creation_type.take();
        if !super::user_function(self.ctx, fid) {
            let evaluated = self.evaluate_args(ast, args);
            self.creation_type = ty;
            let r = self.runtime_error_constructor(fid, &evaluated);
            self.creation_type = None;
            if let Some(op) = r {
                return op;
            }
            let name_str = self
                .ctx
                .symbol_name(self.ctx.program.classes[cid.0 as usize].name)
                .to_string();
            return self.unsupported(&format!("SDK class constructor ({name_str})"), span);
        }
        // The concrete class (a mixin application) holds for this creation, not
        // for those that the evaluation of the arguments makes.
        let concrete = self.concrete_class.take();
        let armed = self.arm_constructor_types(fid, ty, args);
        let evaluated = self.evaluate_args(ast, args);
        self.arg_types = armed;
        self.creation_type = ty;
        self.concrete_class = concrete;
        self.instantiate_evaluated(ctor_fid, &evaluated, span)
    }

    /// The constructor `name` of the class `cid`. A mixin application
    /// (`class C = S with M;`, specification §12.1) declares no constructors:
    /// it has a forwarding constructor for each generative constructor
    /// of the superclass. Then it returns that of the superclass (going up through the
    /// chained applications) and records `cid` as the concrete class of the next
    /// creation — the object is of `cid`, and the fields of the mixins are
    /// initialized before the superclass constructor.
    pub fn constructor_of(
        &mut self,
        cid: ClassId,
        name_str: SymbolId,
    ) -> Option<FunctionElementId> {
        let mut c = cid;
        loop {
            let class_ref = &self.ctx.program.classes[c.0 as usize];
            if let Some(&f) = class_ref.constructors.get(&name_str) {
                if c != cid {
                    if self.ctx.program.functions[f.0 as usize].factory {
                        return None;
                    }
                    self.concrete_class = Some(cid);
                }
                return Some(f);
            }
            if class_ref.kind != dartforge_elements::model::ClassKind::MixinApplication
                || !class_ref.constructors.is_empty()
            {
                return None;
            }
            c = class_ref.supertype_class?;
        }
    }

    /// `C(args)` with the arguments already evaluated.
    pub fn instantiate_evaluated(
        &mut self,
        ctor_fid: FunctionElementId,
        evaluated: &[Evaluated],
        span: Span,
    ) -> Operand {
        self.instantiate_evaluated_with_rti(ctor_fid, evaluated, span, None, None)
    }

    /// A redirecting factory may change the class and even permute its
    /// type arguments. In that case, the RTI and the tuple of the target come from the
    /// annotation `= Target<U, T>`, not from the return type of the source.
    pub fn instantiate_evaluated_with_rti(
        &mut self,
        ctor_fid: FunctionElementId,
        evaluated: &[Evaluated],
        span: Span,
        explicit_rti: Option<Operand>,
        explicit_tuple: Option<Operand>,
    ) -> Operand {
        let ty = self.creation_type.take();
        let concrete = self.concrete_class.take();
        let fid = ctor_fid.0 as usize;
        let f = &self.ctx.program.functions[fid];
        let Some(cid_ctor) = f.class else {
            return self.unsupported("constructor without a class", span);
        };
        // The object is of the concrete class (a mixin application) when the
        // constructor is that of the superclass it forwards.
        let cid = concrete.unwrap_or(cid_ctor);
        // The `core.Symbol` factory redirects to the concrete class of the
        // internal library. The target is unambiguous and prevents the resolution
        // of the factory from going back to the abstract declaration itself.
        if Some(cid) == self.ctx.sdk_class("core", "Symbol")
            && let Some(concrete) = self.ctx.sdk_class("_internal", "Symbol")
            && let Some(empty) = self.ctx.interner.lookup("")
            && let Some(&constructor) = self.ctx.program.classes[concrete.0 as usize]
                .constructors
                .get(&empty)
        {
            return self.instantiate_evaluated_with_rti(
                constructor,
                evaluated,
                span,
                explicit_rti,
                explicit_tuple,
            );
        }
        if !super::user_function(self.ctx, fid) {
            if let Some(op) = self.runtime_error_constructor(fid, evaluated) {
                return op;
            }
            let name_str = self
                .ctx
                .symbol_name(self.ctx.program.classes[cid.0 as usize].name)
                .to_string();
            return self.unsupported(&format!("SDK class constructor ({name_str})"), span);
        }
        // The extension type constructor (`extension_types.rs`) is a
        // function that returns the representation, called like a factory.
        let factory = f.factory || self.ctx.is_extension_type(cid_ctor);
        let mut args = self.match_args(fid, evaluated);
        let generic = self.generic_class(cid);
        if factory {
            // RTI: the factory of a generic class receives the type
            // arguments of the class in the tuple (the last parameter).
            if generic {
                let t = explicit_tuple.unwrap_or_else(|| self.creation_tuple(ty));
                args.push(t);
            }
            return self.call_direct(fid, None, args);
        }
        let fields = layout(self.ctx, cid).len() + super::enums::layout_base(self.ctx, cid);
        let obj = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_new".to_string(),
                args: vec![
                    (
                        Operand::Constant(Constant::Int(i64::from(
                            self.ctx.class_id_of(cid).unwrap_or(0),
                        ))),
                        Type::I64,
                    ),
                    (Operand::Constant(Constant::Int(fields as i64)), Type::I64),
                ],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        // RTI: the instance of a generic class keeps the type (`C<T…>`).
        if generic && let Some(r) = explicit_rti.or_else(|| ty.map(|t| self.type_rti(t))) {
            self.define_rti(obj.clone(), r);
        }
        if concrete.is_some() {
            self.initialize_application_mixins(obj.clone(), cid, cid_ctor);
        }
        self.call_direct(fid, Some(obj.clone()), args);
        obj
    }

    /// The forwarding constructors of the mixin applications from `of` to
    /// `until` (exclusive): each one initializes the fields of its mixins, from the
    /// last to the first, before the superclass constructor.
    fn initialize_application_mixins(&mut self, obj: Operand, of: ClassId, until: ClassId) {
        let saved = self.this_param.replace(obj);
        let saved_finalizable = self.finalizable_this;
        self.finalizable_this = self.ctx.classify_finalizable_class(of);
        let mut c = of;
        while c != until {
            let class_ref = &self.ctx.program.classes[c.0 as usize];
            let mixins = class_ref.mixin_classes.clone();
            let sup = class_ref.supertype_class;
            for m in mixins.into_iter().rev() {
                if self
                    .ctx
                    .compiled_library(self.ctx.program.classes[m.0 as usize].library)
                {
                    self.initialize_fields(m);
                }
            }
            let Some(s) = sup else { break };
            c = s;
        }
        self.this_param = saved;
        self.finalizable_this = saved_finalizable;
    }

    /// The function (not a constructor) declares type parameters: it receives the tuple.
    /// An instance member of a generic extension also receives it: the
    /// arguments of the extension come first (`type_params_of`).
    pub fn generic_function(&self, fid: usize) -> bool {
        !matches!(
            self.ctx.program.functions[fid].node,
            FunctionRef::Constructor { .. }
        ) && !self.type_params_of(fid).is_empty()
            && super::user_function(self.ctx, fid)
    }

    /// The type parameters that the tuple of `fid` carries, in order: those of the
    /// extension (instance member; a static one does not see them) and its own.
    pub fn type_params_of(&self, fid: usize) -> Vec<TypeParamId> {
        let f = &self.ctx.program.functions[fid];
        let mut v = Vec::new();
        if let Some(e) = f.extension.filter(|_| !f.static_)
            && let Some(x) = self.ctx.outline.extensions.get(e.0 as usize)
        {
            v.extend(x.type_params.iter().copied());
        }
        // Instance member of an extension type: those of the type, like those of
        // an extension (there is no object that keeps them).
        if let Some(c) = super::extension_types::instance_owner(self.ctx, fid)
            && let Some(d) = self.ctx.outline.classes.get(c.0 as usize)
        {
            v.extend(d.type_params.iter().copied());
        }
        if let Some(d) = self.ctx.outline.functions.get(fid) {
            v.extend(d.type_params.iter().copied());
        }
        v
    }

    /// The class declares type parameters.
    pub fn generic_class(&self, cid: ClassId) -> bool {
        self.ctx
            .outline
            .classes
            .get(cid.0 as usize)
            .is_some_and(|d| !d.type_params.is_empty())
    }

    /// The type argument tuple (`L<…>`) of the type of a creation; without a
    /// type, 0 (the arguments stay `dynamic`).
    pub fn creation_tuple(&mut self, ty: Option<TypeId>) -> Operand {
        let Some(t) = ty else {
            return Operand::Constant(Constant::Int(0));
        };
        // (That of an extension type too: its constructors receive the
        // tuple like the factories.)
        let (dartforge_types::table::Type::Interface { args, .. }
        | dartforge_types::table::Type::ExtensionType { args, .. }) = self.ctx.table.get(t)
        else {
            return Operand::Constant(Constant::Int(0));
        };
        let args = args.clone();
        self.rti_type_tuple(&args)
    }

    /// The tuple `L<a…>` of the types `args`, as `I64`, in the current environment.
    pub fn rti_type_tuple(&mut self, args: &[TypeId]) -> Operand {
        let mut r = super::rti::Recipe {
            text: "L<".to_string(),
            variables: false,
        };
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                r.text.push(',');
            }
            let x = self.type_recipe(*a);
            r.text.push_str(&x.text);
            r.variables |= x.variables;
        }
        r.text.push('>');
        self.recipe_rti(&r)
    }

    /// Field initializers of the class itself, in declaration order.
    ///
    /// Those of a mixin (or of a superclass without a constructor) run in the
    /// constructor of whoever applies it, but are code of `cid`: their type
    /// variables are those of `cid`, and `this` is seen as `cid` (the analyzer's `reader =
    /// FlowLinkReader<PromotionInfo<Type>>()` of `FlowModelHelper<Type>`,
    /// applied to a class whose first parameter is another one).
    fn initialize_fields(&mut self, cid: ClassId) {
        let saved = self.enclosing_class.replace(cid);
        self.initialize_fields_of(cid);
        self.enclosing_class = saved;
    }

    fn initialize_fields_of(&mut self, cid: ClassId) {
        let this = self.this_param.clone().expect("constructor has this");
        let fields: Vec<VariableId> = self.ctx.program.classes[cid.0 as usize].fields.clone();
        for vid in fields {
            let var = &self.ctx.program.variables[vid.0 as usize];
            if var.static_ || var.late {
                continue;
            }
            let dartforge_elements::model::VariableRef::Field { unit, .. } = var.node else {
                continue;
            };
            let Some(init) = self.variable_initializer_in(vid) else {
                continue;
            };
            let v = self.lower_expr_of(unit, init);
            let span = self.ctx.program.unit(unit).ast.expr(init).span;
            self.store_field(this.clone(), vid, v, span);
        }
    }

    /// Calls the constructor of the user superclass, if there is one.
    fn call_super(
        &mut self,
        cid: ClassId,
        name_str: Option<SymbolId>,
        evaluated: Vec<Evaluated>,
        span: Span,
    ) {
        // The applied mixins run the initializers of their fields before
        // the superclass constructor, from the last to the first (each
        // application `S with M` is a class whose constructor initializes `M`
        // and calls that of `S`).
        let mixins: Vec<ClassId> = self.ctx.program.classes[cid.0 as usize]
            .mixin_classes
            .clone();
        for m in mixins.into_iter().rev() {
            if self
                .ctx
                .compiled_library(self.ctx.program.classes[m.0 as usize].library)
            {
                self.initialize_fields(m);
            }
        }
        let Some(sup) = self.ctx.program.classes[cid.0 as usize].supertype_class else {
            return;
        };
        let sup_class = &self.ctx.program.classes[sup.0 as usize];
        if !self.ctx.compiled_library(sup_class.library) {
            // `Object()` and SDK superclasses: nothing to run in our heap.
            return;
        }
        // `Object()` (`const Object();`): no body, no initializers and no
        // superclass — the call does nothing (and cost a call, an
        // exception check and a frame per created object).
        if sup_class.supertype_class.is_none()
            && self.ctx.program.library(sup_class.library).uri == "dart:core"
            && self.ctx.symbol_name(sup_class.name) == "Object"
        {
            return;
        }
        let Some(empty) = self.ctx.interner.lookup("") else {
            return;
        };
        let s = name_str.unwrap_or(empty);
        if name_str.is_none() && sup_class.constructors.is_empty() {
            // A superclass without a declared constructor and without the synthetic one (the
            // abstract class does not get it in the element): the implicit default
            // constructor — field initializers and its `super()`.
            self.initialize_fields(sup);
            self.call_super(sup, None, Vec::new(), span);
            return;
        }
        let Some(&sf) = sup_class.constructors.get(&s) else {
            self.unsupported("superclass constructor not found", span);
            return;
        };
        let sf = sf.0 as usize;
        let args = self.match_args(sf, &evaluated);
        let this = self.this_param.clone().expect("constructor has this");
        self.call_direct(sf, Some(this), args);
    }

    /// Body of a generative constructor, in the order of the specification (N5).
    pub fn lower_constructor(
        &mut self,
        ast: &ast::Ast,
        cid: ClassId,
        ctor: &ast::Constructor,
        span_ctor: Span,
    ) {
        let this = self.this_param.clone().expect("constructor has this");
        let class_ref = &self.ctx.program.classes[cid.0 as usize];

        // `: this(…)` delegates everything to the other constructor.
        if let Some((target, args, span)) = ctor.initializers.iter().find_map(|i| match i {
            ast::Initializer::Redirect {
                constructor,
                arguments,
                span,
            } => Some((*constructor, arguments, *span)),
            _ => None,
        }) {
            let Some(empty) = self.ctx.interner.lookup("") else {
                return;
            };
            let s = target.map(|n| n.sym).unwrap_or(empty);
            let Some(&tf) = class_ref.constructors.get(&s) else {
                self.unsupported("redirected constructor not found", span);
                return;
            };
            let evaluated = self.evaluate_args(ast, &args.args);
            let args = self.match_args(tf.0 as usize, &evaluated);
            self.call_direct(tf.0 as usize, Some(this), args);
            self.terminate(Terminator::Return(None));
            return;
        }

        // 1. Field initializers of the class itself.
        self.initialize_fields(cid);

        // 2. `this.x` parameters.
        for p in ctor.parameters.iter() {
            if !p.this_ {
                continue;
            }
            let Some(n) = p.name else { continue };
            let Some(vid) = self.own_field(cid, n.sym) else {
                self.unsupported("this.x parameter without a field", p.span);
                continue;
            };
            let v = self.read_local_by_name(n.sym).expect("declared parameter");
            self.store_field(this.clone(), vid, v, p.span);
        }

        // 3. Initializer list, in written order; the arguments of
        //    `super(…)` are evaluated at its position.
        let mut explicit_super: Option<(Option<SymbolId>, Vec<Evaluated>, Span)> = None;
        for init in ctor.initializers.iter() {
            match init {
                ast::Initializer::Field {
                    name, value, span, ..
                } => {
                    let Some(vid) = self.own_field(cid, name.sym) else {
                        self.unsupported("initializer of a nonexistent field", *span);
                        continue;
                    };
                    let v = self.lower_expr(ast, *value);
                    self.store_field(this.clone(), vid, v, *span);
                }
                ast::Initializer::Assert {
                    condition, message, ..
                } => {
                    self.lower_assert(ast, *condition, *message);
                }
                ast::Initializer::Super {
                    constructor,
                    arguments,
                    span,
                } => {
                    let evaluated = self.evaluate_args(ast, &arguments.args);
                    explicit_super = Some((constructor.map(|n| n.sym), evaluated, *span));
                }
                ast::Initializer::Redirect { .. } => {}
            }
        }

        // 4. `super(…)`: explicit or implicit, plus the `super.x` ones.
        let (super_name, mut evaluated, span_super) =
            explicit_super.unwrap_or((None, Vec::new(), span_ctor));
        for p in ctor.parameters.iter().filter(|p| p.super_) {
            let Some(n) = p.name else { continue };
            let v = self.read_local_by_name(n.sym).expect("declared parameter");
            let name_str = if p.kind == ParameterKind::Named {
                Some(n.sym)
            } else {
                None
            };
            evaluated.push((name_str, v));
        }
        // Positional `super.x` ones go after the explicit ones, in order.
        evaluated.sort_by_key(|(n, _)| n.is_some());
        self.call_super(cid, super_name, evaluated, span_super);

        // 5. Body. The `this.x` are not in the scope of the body: `x` there is the
        //    field, and the resolution (`Resolved::Member`) already says so.
        for p in ctor.parameters.iter() {
            if p.this_
                && let Some(n) = p.name
            {
                self.remove_local(n.sym);
            }
        }
        match &ctor.body {
            FunctionBody::Block(s) => self.lower_stmt(ast, *s),
            FunctionBody::Expression(e) => {
                self.lower_expr(ast, *e);
            }
            _ => {}
        }
    }

    /// Synthetic constructor: field initializers and `super()`.
    pub fn lower_synthetic_constructor(&mut self, ast: &ast::Ast, cid: ClassId) {
        self.initialize_fields(cid);
        let span = self.ctx.program.classes[cid.0 as usize]
            .decl
            .map(|d| ast.decl(d.decl).span)
            .unwrap_or(Span { start: 0, end: 0 });
        self.call_super(cid, None, Vec::new(), span);
    }

    // ------------------------------------------------------------------
    // Globais (N6)

    /// Builds the SDK `LateError` and throws in the exception flow of the HIR.
    /// `code`: 0/1 field not initialized/already initialized; 2/3 local.
    pub fn throw_late_error(&mut self, name_str: &str, code: i64) {
        let n = self.emit(
            Instruction::Const(Constant::String(name_str.to_string())),
            Type::Ref,
        );
        let error = self.new_runtime_error(
            "dartforge_late_error_new",
            vec![
                (n, Type::Ref),
                (Operand::Constant(Constant::Int(code)), Type::I64),
            ],
        );
        self.emit_throw_op(error);
    }

    /// Body of the lazy getter of a global.
    pub fn lower_global_getter(&mut self, vid: VariableId, repr: Type) {
        // The getter returns the value like Dart functions: Ref is Owned.
        if self.ctx.arc_memory && repr == Type::Ref {
            self.dart_ref_returns.insert(self.func.symbol.clone());
        }
        let val_ref = super::global_value_symbol(self.ctx, vid);
        let flag = format!("{val_ref}$ok");
        let Some(init) = self.variable_initializer_in(vid) else {
            if self.ctx.program.variables[vid.0 as usize].late {
                let ok = self.emit(
                    Instruction::LoadGlobal {
                        symbol: flag,
                        ty: Type::I8,
                    },
                    Type::I8,
                );
                let ready = self.emit(
                    Instruction::ICmp(ICmpOp::Ne, ok, Operand::Constant(Constant::Int(0))),
                    Type::I1,
                );
                let b_read = self.new_block();
                let b_error = self.new_block();
                self.terminate(Terminator::CondBranch {
                    cond: ready,
                    then_block: b_read,
                    else_block: b_error,
                });
                self.set_block(b_error);
                let name_str = self
                    .ctx
                    .symbol_name(self.ctx.program.variables[vid.0 as usize].name)
                    .to_string();
                self.throw_late_error(&name_str, 0);
                self.set_block(b_read);
            }
            let v = self.emit(
                Instruction::LoadGlobal {
                    symbol: val_ref,
                    ty: repr,
                },
                repr,
            );
            self.terminate(Terminator::Return(Some(v)));
            return;
        };
        let ok = self.emit(
            Instruction::LoadGlobal {
                symbol: flag.clone(),
                ty: Type::I8,
            },
            Type::I8,
        );
        let ready = self.emit(
            Instruction::ICmp(ICmpOp::Eq, ok.clone(), Operand::Constant(Constant::Int(1))),
            Type::I1,
        );
        let b_read = self.new_block();
        let b_init = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: ready,
            then_block: b_read,
            else_block: b_init,
        });
        self.set_block(b_read);
        let v = self.emit(
            Instruction::LoadGlobal {
                symbol: val_ref.clone(),
                ty: repr,
            },
            repr,
        );
        self.terminate(Terminator::Return(Some(v)));
        self.set_block(b_init);
        let reentrant = self.emit(
            Instruction::ICmp(ICmpOp::Eq, ok, Operand::Constant(Constant::Int(2))),
            Type::I1,
        );
        let b_stack = self.new_block();
        let b_evaluate = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: reentrant,
            then_block: b_stack,
            else_block: b_evaluate,
        });
        self.set_block(b_stack);
        let error = self.new_runtime_error("dartforge_stack_overflow_error_new", Vec::new());
        self.emit_throw_op(error);
        self.set_block(b_evaluate);
        // 0 = not started; 2 = evaluating; 1 = ready. The reentrant read
        // produces the SDK's `StackOverflowError` without growing the native stack.
        self.emit(
            Instruction::StoreGlobal {
                symbol: flag,
                val: Operand::Constant(Constant::Int(2)),
                ty: Type::I8,
                root: None,
            },
            Type::Void,
        );
        let unit = self.unit_id;
        let is_const = self.ctx.program.variables[vid.0 as usize].const_;
        let saved = std::mem::replace(&mut self.in_const_context, is_const);
        // If the initializer throws, the variable stays uninitialized
        // (§10 "Variables": the next read runs the initializer again):
        // the flag goes back to 0 and the exception stays pending.
        let b_fail = self.new_block();
        self.exception_targets.push(b_fail);
        let v = self.lower_expr_of(unit, init);
        self.exception_targets.pop();
        self.in_const_context = saved;
        let continues = self.current_block;
        self.set_block(b_fail);
        self.emit(
            Instruction::StoreGlobal {
                symbol: format!("{}$ok", super::global_value_symbol(self.ctx, vid)),
                val: Operand::Constant(Constant::Int(0)),
                ty: Type::I8,
                root: None,
            },
            Type::Void,
        );
        self.terminate(Terminator::Return(Some(Self::zero_value(repr))));
        self.set_block(continues);
        let v = self.coerce(v, repr);
        // The assembled string `const` (`'a${'b'}'`) is the canonical literal with the
        // same content (`constants.rs`).
        let v = if is_const && repr == Type::Ref {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_canonical_constant".to_string(),
                    args: vec![(v, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )
        } else {
            v
        };
        let root = (repr == Type::Ref).then_some(vid.0);
        self.emit(
            Instruction::StoreGlobal {
                symbol: val_ref,
                val: v.clone(),
                ty: repr,
                root,
            },
            Type::Void,
        );
        if is_const && repr == Type::Ref {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_mark_constant".to_string(),
                    args: vec![
                        (v.clone(), Type::Ref),
                        (
                            Operand::Constant(Constant::Function(self.func.symbol.clone())),
                            Type::I64,
                        ),
                    ],
                    ret_ty: Type::Void,
                },
                Type::Void,
            );
        }
        self.emit(
            Instruction::StoreGlobal {
                symbol: format!("{}$ok", super::global_value_symbol(self.ctx, vid)),
                val: Operand::Constant(Constant::Int(1)),
                ty: Type::I8,
                root: None,
            },
            Type::Void,
        );
        self.terminate(Terminator::Return(Some(v)));
    }

    /// Reads a global through the lazy getter.
    pub fn read_global(&mut self, vid: VariableId, span: Span) -> Operand {
        if let Some(value) = self.read_platform_flag(vid) {
            return value;
        }
        // `@JS() external T name;`: the interop getter.
        if let Some(getter) = super::js_members::interop_variable_accessor(self.ctx, vid, false) {
            return self.call_direct(getter, None, Vec::new());
        }
        if let Some(v) = self.read_native_variable(vid, span) {
            return v;
        }
        // Primitive `const`: the constant, without the getter (`primitive_const.rs`).
        if let Some(v) = self.read_primitive_const(vid) {
            return v;
        }
        if !self
            .ctx
            .compiled_library(self.ctx.program.variables[vid.0 as usize].library)
        {
            let name_str = self
                .ctx
                .symbol_name(self.ctx.program.variables[vid.0 as usize].name)
                .to_string();
            return self.unsupported(&format!("SDK global ({name_str})"), span);
        }
        let repr = self.repr(variable_type(self.ctx, vid));
        let getter = Instruction::CallStatic {
            symbol: super::global_symbol(self.ctx, vid),
            args: Vec::new(),
            ret_ty: repr,
        };
        let v = if self.inline_global_read(vid) {
            // The fast path of the getter at the use site: the flag says the
            // value is ready (1; 0 is not started, 2 is being evaluated) and
            // the global holds it. The getter — which creates the value, marks
            // the constant (`dartforge_mark_constant`, the key of the copy
            // between isolates) or throws the `StackOverflowError` of a
            // reentrant read and the `LateInitializationError` — only runs in
            // the cold block. The flag and the slot are those the getter reads
            // itself (per isolate, in the area of the module): the read is the
            // same. Before, every use of an enum value was a call with a full
            // prologue; `convertFormatValue` of the dart_ui color kernel
            // made 15 per iteration.
            let value = super::global_value_symbol(self.ctx, vid);
            let ok = self.emit(
                Instruction::LoadGlobal {
                    symbol: format!("{value}$ok"),
                    ty: Type::I8,
                },
                Type::I8,
            );
            let ready = self.emit(
                Instruction::ICmp(ICmpOp::Eq, ok, Operand::Constant(Constant::Int(1))),
                Type::I1,
            );
            let b_ready = self.new_block();
            let b_cold = self.new_block();
            let b_end = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: ready,
                then_block: b_ready,
                else_block: b_cold,
            });
            self.set_block(b_ready);
            let cached = self.emit(
                Instruction::LoadGlobal {
                    symbol: value,
                    ty: repr,
                },
                repr,
            );
            self.terminate(Terminator::Branch(b_end));
            self.set_block(b_cold);
            let first = self.emit_call_with_check(getter, repr);
            let mut incoming = vec![(b_ready, cached)];
            if !self.is_terminated() {
                incoming.push((self.current_block, first));
                self.terminate(Terminator::Branch(b_end));
            }
            self.set_block(b_end);
            self.emit(Instruction::Phi { incoming, ty: repr }, repr)
        } else {
            self.emit_call_with_check(getter, repr)
        };
        if let Operand::Val(id) = &v {
            self.read_from_global.insert(*id);
        }
        v
    }

    /// The read of the global `vid` can test the `$ok` flag and load the slot
    /// at the use site ([`Self::read_global`]): the global lives in this
    /// module (an SDK global of another module only has its getter), its
    /// getter keeps the flag protocol — an enum value, a variable with an
    /// initializer, or a `late` one, whose getter throws when the flag is
    /// down — and the memory is tracing (under ARC a `Ref` leaving a slot
    /// needs the borrow rules of the getter). `DARTFORGE_NO_INLINE_GLOBALS=1`
    /// keeps the call (measurement).
    fn inline_global_read(&self, vid: VariableId) -> bool {
        static OFF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if *OFF.get_or_init(|| {
            std::env::var_os("DARTFORGE_NO_INLINE_GLOBALS")
                .is_some_and(|v| !v.is_empty() && v != "0")
        }) {
            return false;
        }
        let var = &self.ctx.program.variables[vid.0 as usize];
        if self.ctx.arc_memory || !self.ctx.library_in_module(var.library) {
            return false;
        }
        matches!(
            var.node,
            dartforge_elements::model::VariableRef::EnumConstant { .. }
        ) || var.late
            || self.variable_initializer_in(vid).is_some()
    }

    /// Stores a global (and marks it initialized).
    pub fn store_global(&mut self, vid: VariableId, val: Operand, span: Span) -> Operand {
        if self.store_native_variable(vid, val.clone(), span) {
            return val;
        }
        // `@JS() external T name;`: the interop setter.
        if let Some(setter) = super::js_members::interop_variable_accessor(self.ctx, vid, true) {
            self.call_direct(setter, None, vec![val.clone()]);
            return val;
        }
        if !self
            .ctx
            .compiled_library(self.ctx.program.variables[vid.0 as usize].library)
        {
            return self.unsupported("assignment to an SDK global", span);
        }
        let repr = self.repr(variable_type(self.ctx, vid));
        let val = self.coerce(val, repr);
        if !self
            .ctx
            .library_in_module(self.ctx.program.variables[vid.0 as usize].library)
        {
            // The global lives in another module (source SDK): through its setter.
            self.emit_call_with_check(
                Instruction::CallStatic {
                    symbol: format!("{}$set", super::global_symbol(self.ctx, vid)),
                    args: vec![val.clone()],
                    ret_ty: Type::Void,
                },
                Type::Void,
            );
            return val;
        }
        let var = &self.ctx.program.variables[vid.0 as usize];
        if var.late && var.final_ {
            let flag = format!("{}$ok", super::global_value_symbol(self.ctx, vid));
            let ok = self.emit(
                Instruction::LoadGlobal {
                    symbol: flag,
                    ty: Type::I8,
                },
                Type::I8,
            );
            let already_initialized = self.emit(
                Instruction::ICmp(ICmpOp::Ne, ok, Operand::Constant(Constant::Int(0))),
                Type::I1,
            );
            let b_error = self.new_block();
            let b_store = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: already_initialized,
                then_block: b_error,
                else_block: b_store,
            });
            self.set_block(b_error);
            let name_str = self.ctx.symbol_name(var.name).to_string();
            self.throw_late_error(&name_str, 1);
            self.set_block(b_store);
        }
        self.emit(
            Instruction::StoreGlobal {
                symbol: format!("{}$ok", super::global_value_symbol(self.ctx, vid)),
                val: Operand::Constant(Constant::Int(1)),
                ty: Type::I8,
                root: None,
            },
            Type::Void,
        );
        let root = (repr == Type::Ref).then_some(vid.0);
        self.emit(
            Instruction::StoreGlobal {
                symbol: super::global_value_symbol(self.ctx, vid),
                val: val.clone(),
                ty: repr,
                root,
            },
            Type::Void,
        );
        val
    }

    /// The constructor `name` (or the unnamed one) of the program class named by
    /// an annotated type.
    pub fn type_constructor(
        &self,
        ty: &dartforge_frontend::ast::TypeAnnotation,
        name_str: Option<dartforge_intern::SymbolId>,
    ) -> Option<FunctionElementId> {
        let dartforge_frontend::ast::TypeKind::Named { name, .. } = &ty.kind else {
            return None;
        };
        let lib = self.ctx.program.unit(self.unit_id).library;
        let local_class = |s: dartforge_intern::SymbolId| match self
            .ctx
            .program
            .lookup(lib, s)
            .and_then(|b| b.getter)
        {
            Some(Element::Class(c)) => Some(c),
            _ => None,
        };
        let prefixed_class =
            |p: dartforge_intern::SymbolId, s: dartforge_intern::SymbolId| match self
                .ctx
                .program
                .lookup_prefixed(lib, p, s)
                .and_then(|b| b.getter)
            {
                Some(Element::Class(c)) => Some(c),
                _ => None,
            };
        // `= Target`, `= Target.name`, `= p.Target` and `= p.Target.name` (the parser reads
        // `Target.name` as a two-part type name, and `p.Target.name`
        // as three; the `= t.Template.fromSource` of `mustache_template`).
        let (c, name_str) = match (&name[..], name_str) {
            ([c, n], None) if local_class(c.sym).is_some() => (local_class(c.sym)?, Some(n.sym)),
            ([p, c], n) if prefixed_class(p.sym, c.sym).is_some() => {
                (prefixed_class(p.sym, c.sym)?, n)
            }
            ([p, c, n], None) => (prefixed_class(p.sym, c.sym)?, Some(n.sym)),
            (parts, n) => (local_class(parts.last()?.sym)?, n),
        };
        let key = name_str.or_else(|| self.ctx.interner.lookup(""))?;
        self.ctx.program.classes[c.0 as usize]
            .constructors
            .get(&key)
            .copied()
    }
}

/// A default value that needs no resolution (a literal of null, bool,
/// number or string without interpolation): any module lowers it.
pub fn literal_pattern(ast: &ast::Ast, e: ExprId) -> bool {
    use dartforge_frontend::ast::ExprKind;
    match &ast.expr(e).kind {
        ExprKind::Null | ExprKind::Bool(_) | ExprKind::Int(_) | ExprKind::Double(_) => true,
        ExprKind::String(s) => s
            .parts
            .iter()
            .all(|p| matches!(p, ast::StringPart::Text(_))),
        ExprKind::Unary {
            op: ast::UnaryOp::Neg,
            operand,
        } => {
            matches!(
                ast.expr(*operand).kind,
                ExprKind::Int(_) | ExprKind::Double(_)
            )
        }
        _ => false,
    }
}

/// The default of the parameter of a source SDK function that its module
/// exports: not a literal, from an SDK library outside the current module.
pub fn exported_pattern(
    ctx: &crate::context::Context,
    fid: usize,
    unit: UnitId,
    e: ExprId,
) -> bool {
    let lib = ctx.program.functions[fid].library;
    ctx.program.library(lib).is_sdk
        && !ctx.library_in_module(lib)
        && !literal_pattern(&ctx.program.unit(unit).ast, e)
}

/// The symbol of the function that returns the default of parameter `i` of `fid`.
pub fn pattern_symbol(ctx: &crate::context::Context, fid: usize, i: usize) -> String {
    format!("{}.padrao.{i}", super::symbol_of(ctx, fid))
}

#[cfg(test)]
mod arc_mixin_field_tests {
    use super::*;
    use dartforge_intern::Interner;
    use dartforge_types::table::{CoreTypes, TypeTable};

    #[test]
    fn arc_late_write_preserves_mark_and_final_guard_with_typed_representation() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let dir = tempfile::tempdir().unwrap();
            let entry = dir.path().join("main.dart");
            std::fs::write(&entry,
                "class C { late Object? ref; late final int inteiro; late bool booleano; late double real; late int preparado = 1; Object? ler() => ref; int inicializado() => preparado; void trocar() { ref = null; inteiro = 0; booleano = false; real = 0.0; } } void main() { C().trocar(); }\n").unwrap();
            let sdk = crate::sdk_module::load_native_sdk(crate::sdk_tests()).unwrap();
            let mut interner = Interner::new();
            let (program, diags) = dartforge_elements::load::load_lenient(&entry, &sdk, None, &mut interner);
            assert!(diags.is_empty(), "{diags:?}");
            let mut table = TypeTable::new();
            let core = CoreTypes::init(&mut table, &program, &interner);
            let (mut outline, diags) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
            let (bodies, body_diags) = dartforge_types::infer_program_bodies(&program, &interner, &mut table, &core, &mut outline);
            for d in diags.iter().chain(&body_diags) {
                assert!(!dartforge_types::codes::is_compile_error(&d.message), "{d:?}");
            }
            let te = crate::erasure::compute(&program, &outline, &mut table);
            let mut ctx = Context::new(&program, &interner, &table, &core, &outline, &bodies);
            ctx.te = te;
            for arc in [false, true] {
                ctx.arc_memory = arc;
                let m = crate::lower::lower_program(&ctx);
                assert!(m.errors.is_empty(), "{:?}", m.errors);
                let f = m.functions.iter().find(|f| f.name == "trocar").unwrap();
                let reads: Vec<_> = m.functions.iter().filter(|f| f.name == "ler" || f.symbol.contains("preparado"))
                    .flat_map(|f| &f.blocks).flat_map(|b| &b.instructions).collect();
                let probes = reads.iter().filter(|(_, i, ty)| matches!(i,
                    Instruction::CallRuntime { name, ret_ty: Type::Ref, .. } | Instruction::DeclaredFieldCall { name, ret_ty: Type::Ref, .. }
                        if name == "dartforge_arc_read_field_late_ref_v1") && *ty == Type::Ref).count();
                if arc {
                    assert_eq!(probes, 2, "late Ref and scalar late with an initializer use the Ref probe");
                    assert!(!reads.iter().any(|(_, i, _)| matches!(i,
                        Instruction::CallRuntime { name, .. } | Instruction::DeclaredFieldCall { name, .. } if name == "dartforge_object_get")));
                    assert!(reads.iter().any(|(_, i, _)| matches!(i,
                        Instruction::CallRuntime { name, .. } | Instruction::DeclaredFieldCall { name, .. } if name == "dartforge_late_field_initialized")));
                } else {
                    assert_eq!(probes, 0);
                    assert!(reads.iter().any(|(_, i, _)| matches!(i,
                        Instruction::CallRuntime { name, .. } | Instruction::DeclaredFieldCall { name, .. } if name == "dartforge_object_get")));
                }
                let mut types = std::collections::HashSet::new();
                let mut marks = 0;
                let mut guards = 0;
                for b in &f.blocks {
                    for (pos, (_, i, _)) in b.instructions.iter().enumerate() {
                        if let Instruction::CallRuntime { name, args, .. } | Instruction::DeclaredFieldCall { name, args, .. } = i {
                            if name == "dartforge_late_field_initialized" { guards += 1; }
                            if name == "dartforge_late_field_mark_initialized" {
                                marks += 1;
                                let (obj, idx) = (&args[0].0, &args[1].0);
                                let previous_ones = &b.instructions[..pos];
                                if arc {
                                    let (_, Instruction::SetField { object, index, value }, _) = previous_ones.last().unwrap() else {
                                        panic!("late mark without an immediately preceding typed write");
                                    };
                                    assert_eq!(object, obj);
                                    assert_eq!(*idx, Operand::Constant(Constant::Int(*index as i64)));
                                    let ty = match value {
                                        Operand::Constant(Constant::Null) => Type::Ref,
                                        Operand::Constant(Constant::Int(_)) => Type::I64,
                                        Operand::Constant(Constant::Bool(_)) => Type::I1,
                                        Operand::Constant(Constant::Double(_)) => Type::F64,
                                        Operand::Val(v) => f.blocks.iter().flat_map(|b| &b.instructions)
                                            .find(|(id, _, _)| id == v).unwrap().2,
                                        _ => panic!("late value without a representation"),
                                    };
                                    types.insert(ty);
                                } else {
                                    assert!(matches!(&previous_ones.last().unwrap().1, Instruction::CallRuntime { name, .. } | Instruction::DeclaredFieldCall { name, .. }
                                        if name == "dartforge_object_set"));
                                }
                            }
                        }
                    }
                }
                assert_eq!(marks, 4);
                assert_eq!(guards, 1, "late final keeps the check before the second write");
                if arc {
                    assert_eq!(types, std::collections::HashSet::from([Type::Ref, Type::I64, Type::I1, Type::F64]));
                    assert!(!f.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)|
                        matches!(i, Instruction::CallRuntime { name, .. } | Instruction::DeclaredFieldCall { name, .. } if name == "dartforge_object_set")));
                } else {
                    assert!(!f.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i, Instruction::SetField { .. })));
                }
            }
        }).unwrap().join().unwrap();
    }

    #[test]
    fn applications_with_distinct_indices_carry_typed_accesses_in_arc() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let dir = tempfile::tempdir().unwrap();
            let entry = dir.path().join("main.dart");
            std::fs::write(&entry, "mixin M { Object? campo; int contador = 0; Object? ler() => campo; void trocar(Object? valor) { campo = valor; contador += 1; } } class Base { int a = 0; int b = 0; } class A with M {} class B extends Base with M {} void main() { A().trocar(1); B().trocar(2); A().ler(); B().ler(); }\n").unwrap();
            let sdk = crate::sdk_module::load_native_sdk(crate::sdk_tests()).unwrap();
            let mut interner = Interner::new();
            let (program, diags) = dartforge_elements::load::load_lenient(&entry, &sdk, None, &mut interner);
            assert!(diags.is_empty(), "{diags:?}");
            let mut table = TypeTable::new();
            let core = CoreTypes::init(&mut table, &program, &interner);
            let (mut outline, diags) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
            let (bodies, body_diags) = dartforge_types::infer_program_bodies(&program, &interner, &mut table, &core, &mut outline);
            for d in diags.iter().chain(&body_diags) {
                assert!(!dartforge_types::codes::is_compile_error(&d.message), "{d:?}");
            }
            let te = crate::erasure::compute(&program, &outline, &mut table);
            let mut ctx = Context::new(&program, &interner, &table, &core, &outline, &bodies);
            ctx.te = te;
            for arc in [false, true] {
                ctx.arc_memory = arc;
                let m = crate::lower::lower_program(&ctx);
                assert!(m.errors.is_empty(), "{:?}", m.errors);
                let methods: Vec<_> = m.functions.iter().filter(|f| matches!(f.name.as_str(), "ler" | "trocar")).collect();
                assert!(!methods.is_empty());
                let mut reads = std::collections::HashSet::new();
                let mut writes = std::collections::HashSet::new();
                let mut raw_items = 0;
                for (_, inst, ty) in methods.iter().flat_map(|f| &f.blocks).flat_map(|b| &b.instructions) {
                    match inst {
                        Instruction::GetField { index, .. } if *ty == Type::Ref => { reads.insert(*index); }
                        Instruction::SetField { index, .. } => { writes.insert(*index); }
                        Instruction::CallRuntime { name, .. } | Instruction::DeclaredFieldCall { name, .. } if matches!(name.as_str(), "dartforge_object_get" | "dartforge_object_set") => raw_items += 1,
                        _ => {}
                    }
                }
                if arc { assert!(reads.len() >= 2, "{reads:?}"); assert!(writes.len() >= 2, "{writes:?}"); assert_eq!(raw_items, 0); }
                else { assert!(reads.is_empty()); assert!(writes.is_empty()); assert!(raw_items > 0); }
            }
        }).unwrap().join().unwrap();
    }
}
