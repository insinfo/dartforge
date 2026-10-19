//! Uniform closures with local bodies and no escaping code-bearing handles.
//!
//! Allocation reachability alone cannot authorize an indirect invocation: a
//! write through an alias could replace the code pointer. Keep handles confined
//! to their defining SSA value and the uniform entry's implicit receiver. Only
//! invoking that handle or reading its captured context is admitted. Forwarded,
//! typed, stored, returned and externally supplied closures remain unproved.
use super::*;

const CONTEXT_FIELD: usize = (dartforge_runtime::layout::offsets::CLOSURE_CONTEXT
    - dartforge_runtime::layout::offsets::BODY)
    / std::mem::size_of::<i64>();

pub(super) fn targets(module: &Module) -> HashSet<&str> {
    module
        .functions
        .iter()
        .flat_map(|f| &f.blocks)
        .flat_map(|b| &b.instructions)
        .filter_map(|(_, instruction, _)| match instruction {
            Instruction::AllocClosure { code_symbol, .. }
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

/// A witness for each stable local closure receiver.
pub(super) fn receivers(
    function: &Function,
    functions: &HashMap<&str, &Function>,
    targets: &HashSet<&str>,
    shape: &super::super::receiver_provenance::Shape,
) -> Option<HashSet<ValueId>> {
    let implicit_receiver = if targets.contains(function.symbol.as_str()) {
        if !uniform(function) {
            return None;
        }
        Some(function.params[0].0)
    } else {
        None
    };
    let mut handles = HashSet::new();
    for (value, instruction, result) in function.blocks.iter().flat_map(|b| &b.instructions) {
        let (symbol, env) = match instruction {
            Instruction::AllocClosure { code_symbol, env } => (code_symbol, Some(env)),
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
    let protected = |operand: &Operand| {
        matches!(operand, Operand::Val(id)
        if handles.contains(id) || Some(*id) == implicit_receiver)
    };
    for block in &function.blocks {
        for (_, instruction, result) in &block.instructions {
            let mut escaped = false;
            let mut inspect = |operand: &Operand| escaped |= protected(operand);
            match instruction {
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
                // The captured context is distinct from the code/typed entry.
                Instruction::GetField { object, index }
                    if *index == CONTEXT_FIELD && protected(object) && *result == Type::Ref => {}
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
    Some(handles)
}
