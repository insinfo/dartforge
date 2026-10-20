//! Local uniform and typed entries with confined code-bearing handles.
//!
//! Allocation reachability alone cannot authorize an indirect invocation: a
//! write through an alias could replace the code pointer. Keep handles confined
//! to their defining SSA value and the uniform entry's implicit receiver. Only
//! invoking that handle or reading its captured context is admitted. Forwarded,
//! stored, returned and externally supplied closures remain unproved. Typed
//! entries additionally need a local body, exact ABI and a confined code read.
use super::*;

const CONTEXT_FIELD: usize = (dartforge_runtime::layout::offsets::CLOSURE_CONTEXT
    - dartforge_runtime::layout::offsets::BODY)
    / std::mem::size_of::<i64>();
const TYPED_FIELD: usize = (dartforge_runtime::layout::offsets::CLOSURE_TYPED
    - dartforge_runtime::layout::offsets::BODY)
    / std::mem::size_of::<i64>();
const ABI_FIELD: usize = (dartforge_runtime::layout::offsets::CLOSURE_ABI
    - dartforge_runtime::layout::offsets::BODY)
    / std::mem::size_of::<i64>();

pub(super) struct Proof {
    pub handles: HashSet<ValueId>,
    pub calls: HashSet<ValueId>,
    /// Exact immutable closure-field reads checked by this witness.
    pub reads: HashSet<ValueId>,
}

fn typed_abi(body: &Function) -> Option<i64> {
    let ((_, _, Type::Ref), args) = body.params.split_first()? else {
        return None;
    };
    if args.len() > 12 {
        return None;
    }
    args.iter()
        .map(|(_, _, ty)| *ty)
        .chain(std::iter::once(body.return_ty))
        .try_fold(1_i64, |abi, ty| {
            Some(
                abi * 5
                    + match ty {
                        Type::Ref => 1,
                        Type::I64 => 2,
                        Type::F64 => 3,
                        Type::I1 => 4,
                        _ => return None,
                    },
            )
        })
}

fn field_read(instruction: &Instruction, result: Type) -> Option<(&Operand, usize)> {
    match instruction {
        Instruction::GetField { object, index } => Some((object, *index)),
        Instruction::CallRuntime { name, args, ret_ty }
        | Instruction::DeclaredFieldCall {
            name, args, ret_ty, ..
        } if name == "dartforge_object_get" && *ret_ty == result => {
            let [
                (object, Type::Ref),
                (Operand::Constant(Constant::Int(index)), Type::I64),
            ] = args.as_slice()
            else {
                return None;
            };
            Some((object, usize::try_from(*index).ok()?))
        }
        _ => None,
    }
}

pub(super) fn targets(module: &Module) -> HashSet<&str> {
    module
        .functions
        .iter()
        .flat_map(|f| &f.blocks)
        .flat_map(|b| &b.instructions)
        .filter_map(|(_, instruction, _)| match instruction {
            Instruction::AllocClosure { code_symbol, .. }
            | Instruction::AllocClosureTyped { code_symbol, .. }
            | Instruction::TearOff { code_symbol } => Some(code_symbol.as_str()),
            _ => None,
        })
        .collect()
}

fn uniform(body: &Function) -> bool {
    body.return_ty == Type::Ref
        && body
            .params
            .iter()
            .map(|(_, _, ty)| *ty)
            .eq([Type::Ref, Type::Ptr, Type::Ptr])
}

/// Prove that local closure handles and typed entry addresses stay confined.
pub(super) fn prove(
    function: &Function,
    functions: &HashMap<&str, &Function>,
    targets: &HashSet<&str>,
    shape: &super::super::receiver_provenance::Shape,
) -> Option<Proof> {
    let implicit_receiver = if targets.contains(function.symbol.as_str()) {
        if !uniform(function) {
            return None;
        }
        Some(function.params[0].0)
    } else {
        None
    };
    let mut handles = HashSet::new();
    let mut typed_bodies = HashMap::new();
    for (value, instruction, result) in function.blocks.iter().flat_map(|b| &b.instructions) {
        let (symbol, env) = match instruction {
            Instruction::AllocClosure { code_symbol, env } => (code_symbol, Some(env)),
            Instruction::AllocClosureTyped {
                code_symbol,
                env,
                typed,
                abi,
                ..
            } => {
                let body = *functions.get(typed.as_str())?;
                if typed_abi(body) != Some(*abi) {
                    return None;
                }
                typed_bodies.insert(*value, body);
                (code_symbol, Some(env))
            }
            Instruction::TearOff { code_symbol } => (code_symbol, None),
            _ => continue,
        };
        if *result != Type::Ref
            || !functions
                .get(symbol.as_str())
                .is_some_and(|body| uniform(body))
            || env.is_some_and(|env| shape.operand_type(env) != Some(Type::Ref))
        {
            return None;
        }
        handles.insert(*value);
    }
    // Record only direct reads of a known immutable typed entry. A copied,
    // stored or arithmetically reconstructed address is not a call witness.
    let mut code = HashMap::new();
    for (value, instruction, result) in function.blocks.iter().flat_map(|b| &b.instructions) {
        if let Some((Operand::Val(handle), TYPED_FIELD)) = field_read(instruction, *result)
            && let Some(body) = typed_bodies.get(handle)
        {
            if *result != Type::I64 {
                return None;
            }
            code.insert(*value, *body);
        }
    }
    let protected = |operand: &Operand| {
        matches!(operand, Operand::Val(id)
        if handles.contains(id) || code.contains_key(id) || Some(*id) == implicit_receiver)
    };
    let mut calls = HashSet::new();
    let mut reads = HashSet::new();
    for block in &function.blocks {
        for (value, instruction, result) in &block.instructions {
            let mut escaped = false;
            let mut inspect = |operand: &Operand| escaped |= protected(operand);
            if let Some((object, index)) = field_read(instruction, *result)
                && matches!(object, Operand::Val(id) if handles.contains(id) || Some(*id) == implicit_receiver)
            {
                match index {
                    CONTEXT_FIELD if *result == Type::Ref => {
                        reads.insert(*value);
                        continue;
                    }
                    ABI_FIELD if *result == Type::I64 => {
                        reads.insert(*value);
                        continue;
                    }
                    TYPED_FIELD if code.contains_key(value) => {
                        reads.insert(*value);
                        continue;
                    }
                    _ => return None,
                }
            }
            match instruction {
                // The lowering's typed-entry guard only reads the header.
                Instruction::CallRuntime { name, args, ret_ty }
                    if name == "dartforge_value_class"
                        && *result == Type::I64
                        && *ret_ty == Type::I64
                        && matches!(args.as_slice(), [(Operand::Val(id), Type::Ref)]
                            if handles.contains(id) || Some(*id) == implicit_receiver) => {}
                Instruction::TypedCall { target, args, ret } => {
                    let Operand::Val(target) = target else {
                        return None;
                    };
                    let body = code.get(target)?;
                    if *result != *ret
                        || body.return_ty != *ret
                        || args.len() != body.params.len()
                        || args
                            .iter()
                            .zip(&body.params)
                            .any(|((operand, ty), (_, _, expected))| {
                                ty != expected || shape.operand_type(operand) != Some(*ty)
                            })
                    {
                        return None;
                    }
                    for (argument, _) in args {
                        inspect(argument);
                    }
                    calls.insert(*value);
                }
                Instruction::CallClosure {
                    closure,
                    args,
                    names,
                    ret_ty,
                    tuple_types,
                } => {
                    // An entry may also be reached as a selector/static body.
                    // Its implicit receiver is protected from writes, but that
                    // ABI alone does not prove that it is this closure object.
                    if !matches!(closure, Operand::Val(id) if handles.contains(id))
                        || *result != Type::Ref
                        || *ret_ty != Type::Ref
                        || args
                            .iter()
                            .any(|arg| shape.operand_type(arg) != Some(Type::Ref))
                        || !matches!(shape.operand_type(tuple_types), Some(Type::Ref | Type::I64))
                        || names.len() > args.len()
                        || names.iter().collect::<HashSet<_>>().len() != names.len()
                    {
                        return None;
                    }
                    for argument in args {
                        inspect(argument);
                    }
                    inspect(tuple_types);
                }
                _ => crate::optimize::operands::operands(instruction, &mut inspect),
            }
            if escaped {
                return None;
            }
        }
        let mut escaped = false;
        crate::optimize::operands::terminator_operands(&block.terminator, &mut |operand| {
            escaped |= protected(operand);
        });
        if escaped {
            return None;
        }
    }
    Some(Proof {
        handles,
        calls,
        reads,
    })
}
