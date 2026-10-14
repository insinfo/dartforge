//! Experimental direct primitive multiplication bound to reviewed selected SDK implementations.
use super::fn_builder::FnBuilder;
use crate::context::Context;
use crate::hir::*;
use dartforge_elements::model::{Element, FunctionKind, FunctionRef};
use dartforge_frontend::ast::BinaryOp;

const INTEGER: &str = include_str!("../../../../native_sdk/core/integers.dart");
const DOUBLE: &str = include_str!("../../../../native_sdk/core/double.dart");

fn certify(ctx: &Context) -> bool {
    if !super::double_identity::reviewed_selected_sources(ctx) {
        return false;
    }
    let Some(core) = ctx.program.core else {
        return false;
    };
    for (owner, source, physical, methods) in [
        (
            "_IntegerImplementation",
            INTEGER,
            None,
            &["*", "_mulFromInteger", "toDouble"][..],
        ),
        (
            "_Smi",
            INTEGER,
            Some(dartforge_runtime::layout::cid::SMI),
            &[][..],
        ),
        (
            "_Mint",
            INTEGER,
            Some(dartforge_runtime::layout::cid::MINT),
            &[][..],
        ),
        (
            "_Double",
            DOUBLE,
            Some(dartforge_runtime::layout::cid::DOUBLE),
            &["*", "_mul", "_mulFromInteger", "toDouble"][..],
        ),
    ] {
        let Some(symbol) = ctx.interner.lookup(owner) else {
            return false;
        };
        let Some(Element::Class(cid)) = ctx.program.lookup(core, symbol).and_then(|e| e.getter)
        else {
            return false;
        };
        let class = ctx.program.class(cid);
        if class.library != core
            || !class.mixins.is_empty()
            || !class.mixin_classes.is_empty()
            || !class.shadowed_instance_members.is_empty()
            || class
                .decl
                .is_none_or(|decl| ctx.program.unit(decl.unit).source != source)
            || physical.is_some_and(|expected| ctx.class_id_of(cid) != Some(expected as u32))
        {
            return false;
        }
        for name in methods {
            let expected_kind = if *name == "*" {
                FunctionKind::Operator
            } else {
                FunctionKind::Function
            };
            let Some(name) = ctx.interner.lookup(name) else {
                return false;
            };
            let Some(&id) = class.instance_members.get(&name) else {
                return false;
            };
            let function = ctx.program.function(id);
            let FunctionRef::Function { unit, .. } = function.node else {
                return false;
            };
            if function.library != core
                || function.class != Some(cid)
                || function.name != name
                || function.kind != expected_kind
                || function.patched_by.is_some()
                || ctx.program.unit(unit).source != source
            {
                return false;
            }
        }
    }
    for (native, owner, source) in [
        ("Integer_mulFromInteger", "_IntegerImplementation", INTEGER),
        ("Double_mul", "_Double", DOUBLE),
        ("Double_doubleFromInteger", "_Double", DOUBLE),
    ] {
        let mut count = 0;
        for function in &ctx.program.functions {
            if !function.external
                || crate::natives::pragmas(ctx.program, ctx.interner, function)
                    .0
                    .as_deref()
                    != Some(native)
            {
                continue;
            }
            let Some(class) = function.class else {
                return false;
            };
            let unit = match function.node {
                FunctionRef::Function { unit, .. } | FunctionRef::Constructor { unit, .. } => unit,
                FunctionRef::None => return false,
            };
            if function.library != core
                || ctx.symbol_name(ctx.program.class(class).name) != owner
                || function.patched_by.is_some()
                || ctx.program.unit(unit).source != source
            {
                return false;
            }
            count += 1;
        }
        if count != 1 {
            return false;
        }
    }
    true
}

fn certified(ctx: &Context) -> bool {
    ctx.primitive_multiply_enabled
        && !ctx.arc_memory
        && std::env::var("DARTFORGE_NO_PRIMITIVE_MULTIPLY").as_deref() != Ok("1")
        && *ctx
            .primitive_multiply_certificate
            .get_or_init(|| certify(ctx))
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Guards physical primitive classes before bypassing canonical multiplication dispatch.
    /// Changed SDK bodies keep the generic selector path, including conversion methods.
    /// The existing Smi specialization calls this only on its slow edge.
    pub(super) fn certified_primitive_multiply(
        &mut self,
        op: BinaryOp,
        left: Operand,
        right: Operand,
        span: dartforge_diagnostics::Span,
    ) -> Option<Operand> {
        if !matches!(op, BinaryOp::Mul) || !certified(self.ctx) {
            return None;
        }
        let left_type = self.operand_type(&left);
        let right_type = self.operand_type(&right);
        if ![left_type, right_type]
            .iter()
            .all(|ty| matches!(ty, Type::Ref | Type::I64 | Type::F64))
        {
            return None;
        }
        let numeric = |builder: &mut Self, value: &Operand, ty| {
            if ty != Type::Ref {
                return (
                    Operand::Constant(Constant::Bool(true)),
                    Operand::Constant(Constant::Bool(ty == Type::F64)),
                );
            }
            let cid = builder.emit(
                Instruction::CallRuntime {
                    name: "dartforge_value_class".into(),
                    args: vec![(value.clone(), Type::Ref)],
                    ret_ty: Type::I64,
                },
                Type::I64,
            );
            let compare = |builder: &mut Self, class| {
                builder.emit(
                    Instruction::ICmp(
                        ICmpOp::Eq,
                        cid.clone(),
                        Operand::Constant(Constant::Int(i64::from(class))),
                    ),
                    Type::I1,
                )
            };
            let small = compare(builder, dartforge_runtime::layout::cid::SMI);
            let mint = compare(builder, dartforge_runtime::layout::cid::MINT);
            let double = compare(builder, dartforge_runtime::layout::cid::DOUBLE);
            let integer = builder.emit(Instruction::Or(small, mint), Type::I1);
            let number = builder.emit(Instruction::Or(integer, double.clone()), Type::I1);
            (number, double)
        };
        let (left_numeric, left_double) = numeric(self, &left, left_type);
        let (right_numeric, right_double) = numeric(self, &right, right_type);
        let condition = self.emit(Instruction::And(left_numeric, right_numeric), Type::I1);
        let fast = self.new_block();
        let slow = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: condition,
            then_block: fast,
            else_block: slow,
        });
        self.set_block(fast);
        let floating = match (left_type, right_type) {
            (Type::F64, _) | (_, Type::F64) => Operand::Constant(Constant::Bool(true)),
            (Type::I64, _) => right_double,
            (_, Type::I64) => left_double,
            _ => self.emit(Instruction::Or(left_double, right_double), Type::I1),
        };
        let int_block = self.new_block();
        let double_block = self.new_block();
        let arithmetic_join = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: floating,
            then_block: double_block,
            else_block: int_block,
        });
        self.set_block(int_block);
        let int_left = self.coerce(left.clone(), Type::I64);
        let int_right = self.coerce(right.clone(), Type::I64);
        let product = self.emit(Instruction::Mul(int_left, int_right), Type::I64);
        let int_result = self.coerce(product, Type::Ref);
        let int_end = self.current_block;
        self.terminate(Terminator::Branch(arithmetic_join));
        self.set_block(double_block);
        let as_double = |builder: &mut Self, value: Operand, ty| {
            if ty == Type::Ref {
                builder.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: "df.num_to_double".into(),
                        args: vec![(value, Type::Ref)],
                        ret_ty: Type::F64,
                    },
                    Type::F64,
                )
            } else {
                builder.coerce(value, Type::F64)
            }
        };
        let double_left = as_double(self, left.clone(), left_type);
        let double_right = as_double(self, right.clone(), right_type);
        let product = self.emit(Instruction::FMul(double_left, double_right), Type::F64);
        let double_result = self.coerce(product, Type::Ref);
        let double_end = self.current_block;
        self.terminate(Terminator::Branch(arithmetic_join));
        self.set_block(arithmetic_join);
        let value = self.emit(
            Instruction::Phi {
                incoming: vec![(int_end, int_result), (double_end, double_result)],
                ty: Type::Ref,
            },
            Type::Ref,
        );
        let fast_end = self.current_block;
        self.terminate(Terminator::Branch(join));
        self.set_block(slow);
        let fallback = self.operate_dynamic(op, left, right, span);
        let slow_end = self.current_block;
        if self.is_terminated() {
            self.set_block(join);
            return Some(value);
        }
        self.terminate(Terminator::Branch(join));
        self.set_block(join);
        Some(self.emit(
            Instruction::Phi {
                incoming: vec![(fast_end, value), (slow_end, fallback)],
                ty: Type::Ref,
            },
            Type::Ref,
        ))
    }
}

#[cfg(test)]
mod tests;
