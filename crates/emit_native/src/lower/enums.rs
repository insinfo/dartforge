//! Enums (P3): each value is a canonical object of the program, kept in a
//! lazy global (the same mechanism as top-level variables, N6).
//!
//! The object of an enum has, before the declared fields, the two implicit
//! fields of the specification (§13 "Enums"): `index` (position 0, `int`) and
//! `_name` (position 1, the `String` of the name). The value is created like the `const`
//! of an enhanced enum: it allocates, writes `index` and `_name`, and runs the
//! chosen constructor (`value(args)`/`value.name(args)`) with the declared
//! fields. `values` is the list of the values in declaration order;
//! `toString()` is `Enum.value` (the one of the VM's `_Enum`), unless the enum
//! declares its own. Equality is identity (the value is canonical), and the reload
//! key is `(class, name)` (PESQUISA-HOT-RELOAD §4.7): the symbol of the
//! global is the path `dfg.<library>.<Enum>.<value>`.

use super::fn_builder::FnBuilder;
use crate::context::Context;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{
    ClassId, ClassKind, Element, FunctionKind, VariableId, VariableRef,
};
use dartforge_frontend::ast::DeclKind;
use dartforge_types::resolved::{MemberRef, Resolved};

/// Certifies a direct enum constant, not an arbitrary expression with enum type.
fn pattern_enum_class(
    ctx: &Context,
    unit: dartforge_elements::model::UnitId,
    expr: dartforge_frontend::ast::ExprId,
) -> Option<ClassId> {
    certified_enum_constant(ctx, unit, expr).map(|(cid, _)| cid)
}

/// [`pattern_enum_class`] with the declaration index of the constant (the
/// `index` the value carries in its first field).
pub(super) fn certified_enum_constant(
    ctx: &Context,
    unit: dartforge_elements::model::UnitId,
    expr: dartforge_frontend::ast::ExprId,
) -> Option<(ClassId, usize)> {
    let variable = match ctx.get_resolved(unit, expr)? {
        Resolved::Element(Element::Variable(v))
        | Resolved::Member {
            member: MemberRef::Variable(v),
            ..
        } => *v,
        _ => return None,
    };
    let value = ctx.program.variable(variable);
    let VariableRef::EnumConstant { unit, decl, index } = value.node else {
        return None;
    };
    let cid = value.class?;
    let class = ctx.program.class(cid);
    let declaration = class.decl?;
    if !is_enum(ctx, cid)
        || declaration.unit != unit
        || declaration.decl != decl
        || class.enum_constants.get(index) != Some(&variable)
        || value.library != class.library
    {
        return None;
    }
    let DeclKind::Enum(declaration) = &ctx.program.unit(unit).ast.decl(decl).kind else {
        return None;
    };
    (declaration.constants.get(index)?.name.sym == value.name).then_some((cid, index))
}

/// Certifies only declaration shape; the optimizer still audits current bodies.
pub(super) fn simple_enum_constant(
    ctx: &Context,
    variable: VariableId,
) -> Option<crate::hir::SimpleEnumConstant> {
    let value = ctx.program.variable(variable);
    let VariableRef::EnumConstant { unit, decl, index } = value.node else {
        return None;
    };
    let class_id = value.class?;
    let class = ctx.program.class(class_id);
    let declaration = class.decl?;
    if declaration.unit != unit
        || declaration.decl != decl
        || value.library != class.library
        || class.enum_constants.get(index) != Some(&variable)
        || !ctx.enum_pattern_identity_enabled
        || !enum_identity_operator(ctx, class_id)
    {
        return None;
    }
    let DeclKind::Enum(ast) = &ctx.program.unit(unit).ast.decl(decl).kind else {
        return None;
    };
    if !ast.type_params.is_empty()
        || !ast.with.is_empty()
        || !ast.implements.is_empty()
        || !ast.members.is_empty()
        || ast.primary_constructor.is_some()
        || ast.constants.iter().any(|constant| {
            !constant.type_args.is_empty()
                || constant.constructor.is_some()
                || constant
                    .arguments
                    .as_ref()
                    .is_some_and(|arguments| !arguments.args.is_empty())
        })
        || ast.constants.get(index)?.name.sym != value.name
    {
        return None;
    }
    Some(crate::hir::SimpleEnumConstant {
        library_uri: ctx.program.library(class.library).uri.clone(),
        unit_uri: ctx.program.unit(unit).uri.clone(),
        owner: ctx.symbol_name(class.name).to_string(),
        declaration: decl.0,
        index,
        name: ctx.symbol_name(value.name).to_string(),
        class_id: ctx.class_id_of(class_id)?,
        getter: super::global_symbol(ctx, variable),
        global: super::global_value_symbol(ctx, variable),
        root: variable.0,
    })
}

/// Missing synthetic enum ancestry is not evidence that the SDK kept identity.
/// Verify both implicit SDK classes and the actual patched Object operator.
pub(super) fn enum_identity_operator(ctx: &Context, cid: ClassId) -> bool {
    let Some(core) = ctx.program.core else {
        return false;
    };
    if ctx.program.library(core).uri != "dart:core"
        || !ctx.program.library(core).is_sdk
        || !ctx.compiled_library(core)
    {
        return false;
    }
    let class_named = |name: &str| {
        let symbol = ctx.interner.lookup(name)?;
        match ctx.program.lookup(core, symbol)?.getter? {
            Element::Class(c) if ctx.program.class(c).library == core => Some(c),
            _ => None,
        }
    };
    let (Some(object), Some(enum_interface), Some(enum_base), Some(eq)) = (
        ctx.core.object_class,
        class_named("Enum"),
        class_named("_Enum"),
        ctx.interner.lookup("=="),
    ) else {
        return false;
    };
    if class_named("Object") != Some(object) {
        return false;
    }
    let enum_class = ctx.program.class(cid);
    // Mixins are left on the general path, including otherwise valid mixins.
    // Their inherited equality must never be inferred from a missing edge.
    if !is_enum(ctx, cid)
        || !enum_class.mixins.is_empty()
        || !enum_class.mixin_classes.is_empty()
        || enum_class.supertype.is_some()
        || enum_class
            .supertype_class
            .is_some_and(|c| c != object && c != enum_interface && c != enum_base)
        || enum_class.instance_members.contains_key(&eq)
        || enum_class
            .shadowed_instance_members
            .iter()
            .any(|(name, _)| *name == eq)
    {
        return false;
    }
    for c in [enum_interface, enum_base] {
        let class = ctx.program.class(c);
        if class.kind != ClassKind::Class
            || !class.modifiers.abstract_
            || class.supertype_class != Some(object)
            || !class.mixins.is_empty()
            || !class.mixin_classes.is_empty()
            || class.instance_members.contains_key(&eq)
            || class
                .shadowed_instance_members
                .iter()
                .any(|(name, _)| *name == eq)
        {
            return false;
        }
    }
    let object_class = ctx.program.class(object);
    if object_class.supertype_class.is_some()
        || !object_class.mixins.is_empty()
        || !object_class.mixin_classes.is_empty()
    {
        return false;
    }
    let Some(mut function) = object_class.instance_members.get(&eq).copied() else {
        return false;
    };
    let mut seen = std::collections::HashSet::new();
    loop {
        if !seen.insert(function) {
            return false;
        }
        let member = ctx.program.function(function);
        if member.class != Some(object)
            || member.library != core
            || member.name != eq
            || member.kind != FunctionKind::Operator
            || member.static_
            || member.variable.is_some()
            || !member.external
            || member.abstract_
        {
            return false;
        }
        if let Some(patch) = member.patched_by {
            function = patch;
            continue;
        }
        let dartforge_elements::model::FunctionRef::Function {
            unit,
            function: ast_function,
        } = member.node
        else {
            return false;
        };
        if !matches!(
            ctx.program.unit(unit).ast.function(ast_function).body,
            dartforge_frontend::ast::FunctionBody::Empty
        ) {
            return false;
        }
        let Some(signature) = ctx.outline.functions.get(function.0 as usize) else {
            return false;
        };
        if signature.return_type != ctx.core.bool_
            || !signature.type_params.is_empty()
            || signature.parameters.len() != 1
            || signature.parameters[0].ty != ctx.core.object
            || signature.parameters[0].kind != dartforge_frontend::ast::ParameterKind::Required
        {
            return false;
        }
        let (native, _) = crate::natives::pragmas(ctx.program, ctx.interner, member);
        return native.as_deref() == Some("Object_equals")
            && crate::natives::native("Object_equals")
                .is_some_and(|n| n.state == crate::natives::State::Runtime);
    }
}

/// The class is an `enum` of the program.
pub fn is_enum(ctx: &Context, cid: ClassId) -> bool {
    let c = &ctx.program.classes[cid.0 as usize];
    c.kind == ClassKind::Enum && ctx.compiled_library(c.library)
}

/// How many implicit fields come before the declared layout (2 in an enum).
pub fn layout_base(ctx: &Context, cid: ClassId) -> usize {
    if is_enum(ctx, cid) { 2 } else { 0 }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Matches a certified direct enum constant without invoking the candidate's operator.
    pub(super) fn enum_constant_pattern_identity(
        &mut self,
        expr: dartforge_frontend::ast::ExprId,
        constant: Operand,
        candidate: Operand,
    ) -> Option<Operand> {
        if !self.ctx.enum_pattern_identity_enabled
            || self.operand_type(&constant) != Type::Ref
            || !enum_identity_operator(self.ctx, pattern_enum_class(self.ctx, self.unit_id, expr)?)
        {
            return None;
        }
        // Preserve the general equality path's right-side boxing/coercion.
        let candidate = self.coerce(candidate, Type::Ref);
        Some(self.emit(Instruction::ICmp(ICmpOp::Eq, constant, candidate), Type::I1))
    }

    /// `index`/`name` of an enum value of class `cid`: the two implicit
    /// fields (positions 0 and 1), read in line (`GetField`: a load, not a
    /// call; the emitter already read a constant-index `dartforge_object_get`
    /// in line, but the HIR saw a runtime call with effects). Under ARC the
    /// field goes through the runtime, which owns the borrow rules.
    pub fn enum_member(&mut self, cid: ClassId, name: &str, value: Operand) -> Option<Operand> {
        if !is_enum(self.ctx, cid) {
            return None;
        }
        let (idx, ty) = match name {
            "index" => (0, Type::I64),
            "name" => (1, Type::Ref),
            _ => return None,
        };
        let value = self.coerce(value, Type::Ref);
        if !self.ctx.arc_memory {
            return Some(self.emit(
                Instruction::GetField {
                    object: value,
                    index: idx as usize,
                },
                ty,
            ));
        }
        Some(self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_get".to_string(),
                args: vec![
                    (value, Type::Ref),
                    (Operand::Constant(Constant::Int(idx)), Type::I64),
                ],
                ret_ty: ty,
            },
            ty,
        ))
    }

    /// `a == b` (or `a != b`, with `negate`) when the static type of `a` is a
    /// certified enum (`E` or `E?`, [`enum_identity_operator`]): the identity
    /// of the handles. §17.26: with a null side the result is `identical`;
    /// otherwise `a.==(b)`, which for an enum is `Object.==` — `identical`
    /// again. A handle comparison is `identical` here because an enum value
    /// (or null) is never a number box, the only case where `identical`
    /// looks past the handle. `None`: the general path (a left side of
    /// another type, whose `==` the right side cannot predict).
    pub(super) fn enum_identity_equality(
        &mut self,
        type_of_a: Option<dartforge_types::table::TypeId>,
        a: &Operand,
        b: &Operand,
        negate: bool,
    ) -> Option<Operand> {
        if self.in_adapter
            || !self.ctx.enum_pattern_identity_enabled
            || self.operand_type(a) != Type::Ref
        {
            return None;
        }
        let dartforge_types::table::Type::Interface { class, .. } = self.ctx.table.get(type_of_a?)
        else {
            return None;
        };
        if !enum_identity_operator(self.ctx, *class) {
            return None;
        }
        let b = self.coerce(b.clone(), Type::Ref);
        let e = self.emit(Instruction::ICmp(ICmpOp::Eq, a.clone(), b), Type::I1);
        Some(if negate {
            self.emit(Instruction::LNot(e), Type::I1)
        } else {
            e
        })
    }

    /// `E.values`: the list of the values, in declaration order.
    pub fn enum_values(&mut self, cid: ClassId, span: Span) -> Operand {
        // `E.values` is a constant: the same list (`List<E>`, immutable)
        // on every read.
        let class = &self.ctx.program.classes[cid.0 as usize];
        let symbol = format!(
            "dfc.{}.{}.values",
            crate::context::escape_str(&self.ctx.library_name(class.library)),
            crate::context::escape_str(self.ctx.symbol_name(class.name))
        );
        let consts: Vec<VariableId> = class.enum_constants.clone();
        self.generated_constant(&symbol, true, |b| {
            let mut elements = Vec::with_capacity(consts.len());
            for v in consts {
                let x = b.read_global(v, span);
                elements.push((x, 3));
            }
            let l = b.emit(Instruction::AllocList { elements }, Type::Ref);
            if let Some(list) = b.ctx.core.list_class {
                let ty = b.recipe_rti(&super::rti::Recipe {
                    text: format!("C{}<C{}>", b.ctx.id_rti(list), b.ctx.id_rti(cid)),
                    variables: false,
                });
                b.define_rti(l.clone(), ty);
            }
            l
        })
    }

    /// Body of the lazy getter of an enum value: creates the canonical
    /// object on the first read.
    pub fn lower_enum_value(&mut self, vid: VariableId) {
        // The getter returns the value like Dart functions: a Ref is Owned.
        if self.ctx.arc_memory {
            self.dart_ref_returns.insert(self.func.symbol.clone());
        }
        let var = &self.ctx.program.variables[vid.0 as usize];
        let VariableRef::EnumConstant { unit, decl, index } = var.node else {
            return;
        };
        let Some(cid) = var.class else { return };
        let value = super::global_value_symbol(self.ctx, vid);
        let flag = format!("{value}$ok");
        let ok = self.emit(
            Instruction::LoadGlobal {
                symbol: flag.clone(),
                ty: Type::I8,
            },
            Type::I8,
        );
        let ready = self.emit(
            Instruction::ICmp(ICmpOp::Ne, ok, Operand::Constant(Constant::Int(0))),
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
                symbol: value.clone(),
                ty: Type::Ref,
            },
            Type::Ref,
        );
        self.terminate(Terminator::Return(Some(v)));
        self.set_block(b_init);
        self.emit(
            Instruction::StoreGlobal {
                symbol: flag,
                val: Operand::Constant(Constant::Int(1)),
                ty: Type::I8,
                root: None,
            },
            Type::Void,
        );
        let n_fields = super::members::layout(self.ctx, cid).len() + layout_base(self.ctx, cid);
        let id = self.ctx.class_id_of(cid).unwrap_or(0);
        let obj = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_new".to_string(),
                args: vec![
                    (Operand::Constant(Constant::Int(i64::from(id))), Type::I64),
                    (Operand::Constant(Constant::Int(n_fields as i64)), Type::I64),
                ],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        // Generic enum: the value keeps its type, like any generic
        // instance — the written arguments (`a<bool>()`) or, without them, the
        // bounds of the parameters (`E<Object>` for `T extends Object`).
        if self.generic_class(cid)
            && let DeclKind::Enum(e) = &self.ctx.program.unit(unit).ast.decl(decl).kind
        {
            let written_set = e.constants[index].type_args.to_vec();
            let recipe = if written_set.is_empty() {
                let params = self.ctx.outline.classes[cid.0 as usize].type_params.clone();
                let mut r = super::rti::Recipe {
                    text: String::new(),
                    variables: false,
                };
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        r.text.push(',');
                    }
                    // Without a written bound, `dynamic` (instantiate to bounds).
                    let d = self.ctx.table.param(*p);
                    if !d.explicit {
                        r.text.push('D');
                        continue;
                    }
                    let x = self.type_recipe(d.bound);
                    r.text.push_str(&x.text);
                    r.variables |= x.variables;
                }
                r
            } else {
                let saved = std::mem::replace(&mut self.unit_id, unit);
                let r = self.type_argument_recipes(&written_set);
                self.unit_id = saved;
                r
            };
            if !recipe.variables {
                let r = self.recipe_rti(&super::rti::Recipe {
                    text: format!("C{}<{}>", self.ctx.id_rti(cid), recipe.text),
                    variables: false,
                });
                self.define_rti(obj.clone(), r);
            }
        }
        // The object is already the value of the global (and a root) before the constructor: a
        // value that refers to itself in the constructor reads the object.
        self.emit(
            Instruction::StoreGlobal {
                symbol: value.clone(),
                val: obj.clone(),
                ty: Type::Ref,
                root: Some(vid.0),
            },
            Type::Void,
        );
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_mark_constant".to_string(),
                args: vec![
                    (obj.clone(), Type::Ref),
                    (
                        Operand::Constant(Constant::Function(self.func.symbol.clone())),
                        Type::I64,
                    ),
                ],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_set".to_string(),
                args: vec![
                    (obj.clone(), Type::Ref),
                    (Operand::Constant(Constant::Int(0)), Type::I64),
                    (Operand::Constant(Constant::Int(index as i64)), Type::I64),
                    (Operand::Constant(Constant::Int(0)), Type::I8),
                ],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        let name = self.ctx.symbol_name(var.name).to_string();
        let text = self.emit(Instruction::Const(Constant::String(name)), Type::Ref);
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_set".to_string(),
                args: vec![
                    (obj.clone(), Type::Ref),
                    (Operand::Constant(Constant::Int(1)), Type::I64),
                    (text, Type::I64),
                    (Operand::Constant(Constant::Int(1)), Type::I8),
                ],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        // The constructor of the value (the unnamed one when the value does not choose).
        let ast = &self.ctx.program.unit(unit).ast;
        let DeclKind::Enum(e) = &ast.decl(decl).kind else {
            self.terminate(Terminator::Return(Some(obj)));
            return;
        };
        let constant = &e.constants[index];
        let ctor_name = constant
            .constructor
            .map(|n| n.sym)
            .or_else(|| self.ctx.interner.lookup(""));
        let ctor = ctor_name.and_then(|s| {
            self.ctx.program.classes[cid.0 as usize]
                .constructors
                .get(&s)
                .copied()
        });
        if let Some(f) = ctor {
            let fid = f.0 as usize;
            if super::members::has_body(self.ctx, fid) {
                let saved = self.unit_id;
                self.unit_id = unit;
                let evaluated = match &constant.arguments {
                    Some(a) => self.evaluate_args(ast, &a.args),
                    None => Vec::new(),
                };
                let args = self.match_args(fid, &evaluated);
                self.unit_id = saved;
                self.call_direct(fid, Some(obj.clone()), args);
            }
        }
        self.terminate(Terminator::Return(Some(obj)));
    }

    /// Default `toString()` of an enum: `Enum.value`.
    pub fn lower_enum_to_string(&mut self, cid: ClassId) {
        // Body called as a Dart function (by `toString$c` and by
        // dispatch): the Ref return is Owned.
        if self.ctx.arc_memory {
            self.dart_ref_returns.insert(self.func.symbol.clone());
        }
        let this = Operand::Val(self.add_param("this".to_string(), Type::Ref));
        let name = self
            .ctx
            .symbol_name(self.ctx.program.classes[cid.0 as usize].name)
            .to_string();
        let prefix = self.emit(
            Instruction::Const(Constant::String(format!("{name}."))),
            Type::Ref,
        );
        let n = self.enum_member(cid, "name", this).expect("enum");
        let r = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_string_concat".to_string(),
                args: vec![(prefix, Type::Ref), (n, Type::Ref)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        self.terminate(Terminator::Return(Some(r)));
    }
}

#[cfg(test)]
mod pattern_identity_tests;
