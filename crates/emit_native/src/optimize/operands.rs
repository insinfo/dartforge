//! The operands of an instruction and of a terminator, for reading and for
//! replacing (including the `phi` entries).

use crate::hir::*;

macro_rules! visit {
    ($name:ident, $name_t:ident, $($m:tt)*) => {
        /// Visits each operand of the instruction, in evaluation order.
        pub fn $name(inst: & $($m)* Instruction, f: &mut dyn FnMut(& $($m)* Operand)) {
            match inst {
                Instruction::Const(_)
                | Instruction::Alloca(_)
                | Instruction::LoadGlobal { .. }
                | Instruction::TearOff { .. }
                | Instruction::ConstArray(_)
                | Instruction::FunctionTable(_) => {}
                Instruction::Add(a, b)
                | Instruction::Sub(a, b)
                | Instruction::Mul(a, b)
                | Instruction::SDiv(a, b)
                | Instruction::SRem(a, b)
                | Instruction::Shl(a, b)
                | Instruction::AShr(a, b)
                | Instruction::LShr(a, b)
                | Instruction::And(a, b)
                | Instruction::Or(a, b)
                | Instruction::Xor(a, b)
                | Instruction::FAdd(a, b)
                | Instruction::FSub(a, b)
                | Instruction::FMul(a, b)
                | Instruction::FDiv(a, b)
                | Instruction::ICmp(_, a, b)
                | Instruction::FCmp(_, a, b) => {
                    f(a);
                    f(b);
                }
                Instruction::Neg(a)
                | Instruction::Not(a)
                | Instruction::FNeg(a)
                | Instruction::LNot(a)
                | Instruction::IntToDouble(a)
                | Instruction::DoubleToInt(a)
                | Instruction::CheckNotNull(a) => f(a),
                Instruction::ZExt { op: a, .. }
                | Instruction::ArcCopy { value: a }
                | Instruction::ArcDrop { value: a }
                | Instruction::ArcMove { value: a }
                | Instruction::Trunc { op: a, .. }
                | Instruction::Bitcast { op: a, .. }
                | Instruction::Box { op: a, .. }
                | Instruction::Unbox { op: a, .. } => f(a),
                Instruction::AllocObject { fields, .. } => fields.$name_t().for_each(|a| f(a)),
                Instruction::AllocList { elements } | Instruction::AllocRecord { elements } => {
                    elements.$name_t().for_each(|(a, _)| f(a))
                }
                Instruction::AllocCell { value } => f(value),
                Instruction::AllocEnv { values } => values.$name_t().for_each(|a| f(a)),
                Instruction::ConcatStrings { parts } => parts.$name_t().for_each(|a| f(a)),
                Instruction::AllocClosure { env, .. } | Instruction::AllocClosureTyped { env, .. } => f(env),
                Instruction::TypedCall { target, args, .. } => {
                    f(target);
                    args.$name_t().for_each(|(a, _)| f(a));
                }
                Instruction::Load { ptr, .. } => f(ptr),
                Instruction::ArcLoadStrong { slot: StrongSlot::Frame { frame, .. } } => f(frame),
                Instruction::ArcLoadStrong { slot: StrongSlot::Global { .. } } => {},
                Instruction::ArcStoreStrong { slot: StrongSlot::Global { .. }, value, .. } => f(value),
                Instruction::ArcStoreStrong { slot: StrongSlot::Frame { frame, .. }, value, .. } => {
                    f(frame);
                    f(value);
                }
                Instruction::Store { ptr, val } => {
                    f(ptr);
                    f(val);
                }
                Instruction::GetField { object, .. } => f(object),
                Instruction::SetField { object, value, .. } => {
                    f(object);
                    f(value);
                }
                Instruction::GetListElement { list, index } => {
                    f(list);
                    f(index);
                }
                Instruction::SetListElement { list, index, value } => {
                    f(list);
                    f(index);
                    f(value);
                }
                Instruction::CellGet { cell } => f(cell),
                Instruction::CellSet { cell, value } => {
                    f(cell);
                    f(value);
                }
                Instruction::EnvGet { env, .. } => f(env),
                Instruction::CallStatic { args, .. } => args.$name_t().for_each(|a| f(a)),
                Instruction::CallInterface { receiver, args, .. } | Instruction::CallDynamic { receiver, args, .. } => {
                    f(receiver);
                    args.$name_t().for_each(|a| f(a));
                }
                Instruction::CallClosure { closure, args, tuple_types, .. } => {
                    f(closure);
                    args.$name_t().for_each(|a| f(a));
                    f(tuple_types);
                }
                Instruction::CallRuntime { args, .. } | Instruction::DeclaredFieldCall { args, .. } => args.$name_t().for_each(|(a, _)| f(a)),
                Instruction::IsClass { object, .. } => f(object),
                Instruction::StoreGlobal { val, .. } => f(val),
                Instruction::Phi { incoming, .. } => incoming.$name_t().for_each(|(_, a)| f(a)),
                Instruction::LoadIndexed { base, index } => {
                    f(base);
                    f(index);
                }
                Instruction::CallSelector { recv, args, tuple_types, .. } => {
                    f(recv);
                    args.$name_t().for_each(|a| f(a));
                    f(tuple_types);
                }
                Instruction::CallClosureForward { closure, args, desc, .. } => {
                    f(closure);
                    f(args);
                    f(desc);
                }
                Instruction::CallSelectorForward { recv, args, desc, .. } => {
                    f(recv);
                    f(args);
                    f(desc);
                }
                Instruction::NativeCall { target, args, .. } => {
                    f(target);
                    args.$name_t().for_each(|(a, _)| f(a));
                }
                Instruction::NativeLoad { address, index: idx_value, .. } => {
                    f(address);
                    f(idx_value);
                }
                Instruction::NativeWrite { address, index: idx_value, value: val_ref, .. } => {
                    f(address);
                    f(idx_value);
                    f(val_ref);
                }
                Instruction::CompositeNativeCall { target, args, dest, .. } => {
                    f(target);
                    args.$name_t().for_each(|(a, _)| f(a));
                    dest.$name_t().for_each(|a| f(a));
                }
                Instruction::Simd { args, .. } => args.$name_t().for_each(|a| f(a)),
            }
        }
    };
}

visit!(operands, iter,);
visit!(operands_mut, iter_mut, mut);

/// Visits the operands of the terminator.
pub fn terminator_operands_mut(t: &mut Terminator, f: &mut dyn FnMut(&mut Operand)) {
    match t {
        Terminator::Return(Some(o)) | Terminator::Throw(o) => f(o),
        Terminator::CondBranch { cond, .. } => f(cond),
        Terminator::Switch { val, .. } => f(val),
        Terminator::Return(None) | Terminator::Branch(_) | Terminator::Unreachable => {}
    }
}

/// Visits the operands of the terminator (read-only).
pub fn terminator_operands(t: &Terminator, f: &mut dyn FnMut(&Operand)) {
    match t {
        Terminator::Return(Some(o)) | Terminator::Throw(o) => f(o),
        Terminator::CondBranch { cond, .. } => f(cond),
        Terminator::Switch { val, .. } => f(val),
        Terminator::Return(None) | Terminator::Branch(_) | Terminator::Unreachable => {}
    }
}

/// The successors of the terminator, replaceable.
pub fn successors_mut(t: &mut Terminator, f: &mut dyn FnMut(&mut BlockId)) {
    match t {
        Terminator::Branch(b) => f(b),
        Terminator::CondBranch {
            then_block,
            else_block,
            ..
        } => {
            f(then_block);
            f(else_block);
        }
        Terminator::Switch { default, cases, .. } => {
            f(default);
            cases.iter_mut().for_each(|(_, b)| f(b));
        }
        Terminator::Return(_) | Terminator::Throw(_) | Terminator::Unreachable => {}
    }
}

/// Replaces, in the whole function, each use of a value with the operand that
/// `swap` returns.
pub fn replace(func: &mut Function, swap: &dyn Fn(ValueId) -> Option<Operand>) {
    let mut t = |o: &mut Operand| {
        if let Operand::Val(v) = o
            && let Some(n) = swap(*v)
        {
            *o = n;
        }
    };
    for b in &mut func.blocks {
        for (_, inst, _) in &mut b.instructions {
            operands_mut(inst, &mut t);
        }
        terminator_operands_mut(&mut b.terminator, &mut t);
    }
}

/// The largest value id and block id of the function (to number new ones).
pub fn largest_ids(func: &Function) -> (u32, u32) {
    let mut v = func.params.iter().map(|(p, _, _)| p.0).max().unwrap_or(0);
    let mut b = 0;
    for bl in &func.blocks {
        b = b.max(bl.id.0);
        for (id, _, _) in &bl.instructions {
            v = v.max(id.0);
        }
    }
    (v, b)
}

/// The type of an operand: the recorded one of the value, or the natural one of the constant.
pub fn operand_type(o: &Operand, types: &std::collections::HashMap<ValueId, Type>) -> Option<Type> {
    match o {
        Operand::Val(v) => types.get(v).copied(),
        Operand::Constant(c) => Some(match c {
            Constant::Int(_) => Type::I64,
            Constant::Double(_) => Type::F64,
            Constant::Bool(_) => Type::I1,
            Constant::String(_) | Constant::StringWtf8(_) | Constant::Null => Type::Ref,
            Constant::Function(_) => Type::I64,
        }),
    }
}

/// The types of all the values of the function (parameters and instructions).
pub fn function_types(func: &Function) -> std::collections::HashMap<ValueId, Type> {
    let mut t: std::collections::HashMap<ValueId, Type> =
        func.params.iter().map(|(v, _, ty)| (*v, *ty)).collect();
    for b in &func.blocks {
        for (v, _, ty) in &b.instructions {
            t.insert(*v, *ty);
        }
    }
    t
}

/// The neutral constant of a type (the value of a local read before it is
/// written, which Dart only allows for `null`).
pub fn default_constant(ty: Type) -> Operand {
    Operand::Constant(match ty {
        Type::I64 | Type::I8 => Constant::Int(0),
        Type::F64 => Constant::Double(0.0),
        Type::I1 => Constant::Bool(false),
        _ => Constant::Null,
    })
}
