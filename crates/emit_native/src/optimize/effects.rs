//! Per-function summary of exceptions: who cannot leave a pending exception.
//!
//! In the generated code, an exception is a pending state of the runtime that the
//! caller checks right after the call (`dartforge_exception_pending`,
//! `FnBuilder::emit_call_with_check`). At every call the state is clear
//! (each operation that may throw is checked right after), so, after
//! a call to a function that cannot throw, the check is always
//! false. The summary is the greatest fixpoint: it starts with every function of the
//! module not throwing and removes those that have an operation that throws. A function
//! in a call cycle (recursion, direct or mutual) throws: the prologue
//! checks the stack and throws `StackOverflowError` (`llvm/mod.rs`,
//! `emit_stack_check`; docs/NATIVO-PROJETOS-REAIS.md C22) — without
//! that, `void f() => f();` had no check at all, nor the context in
//! which the stack check lives, and overflowed the system stack.

use super::cfg::successors;
use crate::hir::*;
use crate::llvm::externs::effects_of;
use std::collections::HashSet;

/// May the instruction leave a pending exception?
pub fn instruction_throws(inst: &Instruction, do_not_throw: &HashSet<String>) -> bool {
    match inst {
        Instruction::CallStatic { symbol, .. } => !do_not_throw.contains(symbol),
        Instruction::CallRuntime { name, .. } | Instruction::DeclaredFieldCall { name, .. } => effects_of(name).throws,
        Instruction::CallInterface { .. }
        | Instruction::CallDynamic { .. }
        | Instruction::CallClosure { .. }
        | Instruction::CallSelector { .. }
        | Instruction::CallSelectorForward { .. }
        | Instruction::CallClosureForward { .. }
        | Instruction::NativeCall { .. }
        | Instruction::TypedCall { .. }
        | Instruction::CompositeNativeCall { .. } => true,
        // A box of the wrong type or null, null, division by zero, `toInt` of
        // NaN or infinity.
        Instruction::Unbox { to, .. } => !to.is_vector(),
        Instruction::CheckNotNull(_) | Instruction::SDiv(..) | Instruction::SRem(..) | Instruction::DoubleToInt(_) => true,
        _ => false,
    }
}

/// May the function throw, given the current set of functions that do not throw?
fn function_throws(func: &Function, do_not_throw: &HashSet<String>) -> bool {
    func.blocks.iter().any(|b| {
        matches!(b.terminator, Terminator::Throw(_)) || b.instructions.iter().any(|(_, i, _)| instruction_throws(i, do_not_throw))
    })
}

/// The functions of the module that do not throw.
///
/// The greatest fixpoint from above, computed with a worklist: a function
/// throws if it has an operation that throws by itself (with all the module's
/// functions assumed not to throw) or if it calls (`CallStatic`) one that throws. It starts
/// from those that throw by themselves and climbs the inverse call edges. It used to be
/// a scan of the whole module per round, one round per level of the call
/// chain — quadratic in a real program (new_sali/backend).
pub fn do_not_throw(module: &Module) -> HashSet<String> {
    use std::collections::HashMap;
    let all: HashSet<String> = module.functions.iter().map(|f| f.symbol.clone()).collect();
    let mut callers: HashMap<&str, Vec<usize>> = HashMap::new();
    for (k, f) in module.functions.iter().enumerate() {
        for b in &f.blocks {
            for (_, i, _) in &b.instructions {
                if let Instruction::CallStatic { symbol, .. } = i {
                    callers.entry(symbol.as_str()).or_default().push(k);
                }
            }
        }
    }
    let mut throws: HashSet<&str> = HashSet::new();
    let recursive = in_cycle(module);
    let mut queue: Vec<usize> = module
        .functions
        .iter()
        .enumerate()
        .filter(|(k, f)| recursive[*k] || canonical_cache_getter(f)
            || super::constant_specialize::is_specialized_symbol(&f.symbol)
            || module.simple_enum_constants.contains_key(&f.symbol)
            || function_throws(f, &all))
        .map(|(k, _)| k)
        .collect();
    while let Some(k) = queue.pop() {
        let s = module.functions[k].symbol.as_str();
        if !throws.insert(s) {
            continue;
        }
        if let Some(cs) = callers.get(s) {
            queue.extend(cs.iter().copied().filter(|&c| !throws.contains(module.functions[c].symbol.as_str())));
        }
    }
    all.into_iter().filter(|s| !throws.contains(s.as_str())).collect()
}

/// Generated constant getters can fail their emitted stack guard even without
/// recursion in their own bodies. Their callers must not publish a parent cache
/// after an unsuccessful child read. Other context-bearing functions require a
/// separate, general stack-guard effect inventory.
fn canonical_cache_getter(function: &Function) -> bool {
    function.symbol.starts_with("dfc.")
        && function.symbol.ends_with(".get")
        && function.params.is_empty()
        && function.return_ty == Type::Ref
}

/// The functions of the module that are in a cycle of direct calls
/// (`CallStatic`): the strongly connected components with more than one function,
/// or the function that calls itself (Tarjan, iterative).
fn in_cycle(module: &Module) -> Vec<bool> {
    use std::collections::HashMap;
    let n = module.functions.len();
    let index: HashMap<&str, usize> = module.functions.iter().enumerate().map(|(k, f)| (f.symbol.as_str(), k)).collect();
    let edges: Vec<Vec<usize>> = module
        .functions
        .iter()
        .map(|f| {
            f.blocks
                .iter()
                .flat_map(|b| b.instructions.iter())
                .filter_map(|(_, i, _)| match i {
                    Instruction::CallStatic { symbol, .. } => index.get(symbol.as_str()).copied(),
                    _ => None,
                })
                .collect()
        })
        .collect();
    let mut cycle = vec![false; n];
    for component in super::scc::components(&edges) {
        if component.len() > 1 || edges[component[0]].contains(&component[0]) {
            for v in component {
                cycle[v] = true;
            }
        }
    }
    cycle
}

/// Is every operation of the function that may throw checked right after? (The
/// SDK externals matched by name are checked at the end of the statement; with
/// them, an intermediate check may be the one that catches the exception.)
pub fn checks_all(func: &Function, do_not_throw: &HashSet<String>) -> bool {
    func.blocks.iter().all(|b| {
        let n = b.instructions.len();
        (0..n).all(|i| {
            !instruction_throws(&b.instructions[i].1, do_not_throw)
                || (i + 1 < n && is_check(&b.instructions[i + 1].1))
                || (i + 1 == n && successors(&b.terminator).is_empty())
        })
    })
}

pub fn is_check(inst: &Instruction) -> bool {
    matches!(inst, Instruction::CallRuntime { name, .. } | Instruction::DeclaredFieldCall { name, .. } if name == "dartforge_exception_pending")
}

#[cfg(test)]
mod scc_tests {
    use super::*;

    #[test]
    fn recursion_requires_stack_and_propagates_failure_without_contaminating_leaf() {
        let mut m = Module::new();
        for (symbol, dests) in [
            ("folha", vec![]), ("auto", vec!["auto"]),
            ("a", vec!["b", "folha"]), ("b", vec!["a"]),
            ("chamador", vec!["a"]), ("externo", vec!["ausente"]),
        ] {
            m.functions.push(Function {
                symbol: symbol.into(), name: symbol.into(), debug_info: None,
                params: vec![], return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: dests.into_iter().enumerate().map(|(i, dest)| (
                        ValueId(i as u32), Instruction::CallStatic {
                            symbol: dest.into(), args: vec![], ret_ty: Type::Void,
                        }, Type::Void,
                    )).collect(),
                    terminator: Terminator::Return(None),
                }],
            });
        }
        assert_eq!(in_cycle(&m), vec![false, true, true, true, false, false]);
        assert_eq!(do_not_throw(&m), HashSet::from(["folha".into()]));
    }
}

#[cfg(test)]
mod declared_field_effect_tests {
    use super::*;

    #[test]
    fn declaration_metadata_preserves_runtime_exception_effects() {
        for (name, args, ret_ty) in [
            ("dartforge_object_get", vec![(Operand::Constant(Constant::Null), Type::Ref),
                (Operand::Constant(Constant::Int(0)), Type::I64)], Type::Ref),
            ("dartforge_object_set", vec![(Operand::Constant(Constant::Null), Type::Ref),
                (Operand::Constant(Constant::Int(0)), Type::I64),
                (Operand::Constant(Constant::Null), Type::I64),
                (Operand::Constant(Constant::Int(1)), Type::I8)], Type::Void),
        ] {
            let call = Instruction::CallRuntime { name: name.into(), args, ret_ty };
            let field = FieldDeclaration { library_uri: "package:fixture/base.dart".into(),
                owner: "Base".into(), name: "_encoder".into(), is_final: true, is_late: false,
                layout_index: 0, storage_type: Type::Ref };
            let annotated = call.clone().with_field_declaration(Some(field));
            assert!(matches!(annotated, Instruction::DeclaredFieldCall { .. }));
            assert_eq!(instruction_throws(&call, &HashSet::new()),
                instruction_throws(&annotated, &HashSet::new()));
        }
    }
}
