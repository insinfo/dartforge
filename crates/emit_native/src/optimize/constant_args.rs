//! Propagation of a constant argument into the parameter of its callee.
//!
//! In the closed world of a pruned program (`lib.rs`, after
//! `pruning::prune_hir`), a function whose every static call site passes the
//! **same constant** in a position — or omits an optional parameter whose
//! default is a constant, which the lowering materializes at the call site
//! (`members::match_args`) — reads that constant in every activation. The pass
//! substitutes the constant for the parameter inside the body (the parameter
//! stays in the signature: the ABI does not change, the call sites do not
//! change) and lets the existing folding prune what the constant decides:
//! `x ?? y` with `x` a constant null, `==` between two constant strings
//! ([`fold_string_equality`]), a constant branch. It is the counterpart of the
//! VM's TFA for `@pragma("vm:platform-const")`: `operatingSystem ??
//! Platform.operatingSystem` followed by `platform == 'windows'` leaves the
//! other systems' backends unreachable once `Platform.operatingSystem` folds
//! (`lower/platform.rs`), and the second pruning removes them
//! (dart_ui's `PlatformBackendResolver.defaultBackends`: 4.6 MB of win32,
//! x11, wayland, vulkan, gl, d3d11, d3d12, android and ios in the File
//! Explorer of macOS).
//!
//! Only constants travel — a null, a boolean, an integer, a double or a string
//! literal —, never a value computed at the call site, so the order of
//! evaluation of the arguments is untouched. A function whose address is taken
//! anywhere (tear-off, closure, function table, method table, FFI callback,
//! runtime root) is left alone: those calls are not static call sites. Not in
//! the SDK libraries (open world), in the JIT (no pruning) nor under ARC or
//! debug information. `DARTFORGE_NO_CONSTANT_ARGS=1` turns it off.

use super::{mem2reg, operands::*, representation_constants, simplify};
use crate::hir::*;
use std::collections::{HashMap, HashSet};

/// Rounds of propagation: a folded body may turn an argument of its own calls
/// into a constant.
const ROUNDS: usize = 4;

/// A constant argument, comparable across call sites.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Arg {
    Null,
    Bool(bool),
    Int(i64),
    Double(u64),
    Str(Vec<u8>),
}

impl Arg {
    /// The constant as an argument of a parameter of representation `ty`;
    /// `None` when the constant does not fit the representation as is.
    fn read(constant: &Constant, ty: Type) -> Option<Arg> {
        Some(match (constant, ty) {
            (Constant::Null, Type::Ref) => Arg::Null,
            (Constant::Bool(b), Type::I1) => Arg::Bool(*b),
            (Constant::Int(n), Type::I64) => Arg::Int(*n),
            (Constant::Double(d), Type::F64) => Arg::Double(d.to_bits()),
            (Constant::String(s), Type::Ref) => Arg::Str(s.as_bytes().to_vec()),
            (Constant::StringWtf8(b), Type::Ref) => Arg::Str(b.clone()),
            _ => return None,
        })
    }
}

/// The constant definitions of a function: the values of its `Const`
/// instructions whose recorded type is the natural one of the constant.
fn constant_definitions(function: &Function) -> HashMap<ValueId, (Constant, Type)> {
    function
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .filter_map(|(v, inst, ty)| match inst {
            Instruction::Const(c)
                if operand_type(&Operand::Constant(c.clone()), &HashMap::new()) == Some(*ty) =>
            {
                Some((*v, (c.clone(), *ty)))
            }
            _ => None,
        })
        .collect()
}

/// The constant an argument operand carries into a parameter of type `ty`.
fn argument(
    operand: &Operand,
    ty: Type,
    definitions: &HashMap<ValueId, (Constant, Type)>,
) -> Option<Arg> {
    match operand {
        Operand::Constant(c) => Arg::read(c, ty),
        Operand::Val(v) => {
            let (c, defined) = definitions.get(v)?;
            (*defined == ty).then(|| Arg::read(c, ty)).flatten()
        }
    }
}

/// The symbols whose address something takes, or that the runtime calls by
/// itself: never candidates (their calls are not static call sites).
fn address_taken(module: &Module) -> HashSet<String> {
    let mut taken: HashSet<String> = HashSet::new();
    taken.extend(module.entry_symbol.iter().cloned());
    taken.extend(module.call_dart.iter().cloned());
    taken.extend(module.start_rti.iter().cloned());
    taken.extend(module.record.iter().cloned());
    taken.extend(module.sdk_records.iter().cloned());
    taken.extend(module.source_roots.iter().cloned());
    taken.extend(module.helpers.iter().map(|(_, symbol)| symbol.clone()));
    taken.extend(module.table_functions.values().cloned());
    taken.extend(
        module
            .ffi_trampolines
            .iter()
            .map(|(_, symbol)| symbol.clone()),
    );
    taken.extend(module.ffi_callbacks.iter().map(|c| c.body.clone()));
    taken.extend(
        module
            .stack_trace_tearoffs
            .iter()
            .map(|(symbol, _)| symbol.clone()),
    );
    taken.extend(
        module
            .async_frame_layouts
            .iter()
            .map(|(_, _, body, _)| body.clone()),
    );
    taken.extend(module.arc_shared_functions.iter().cloned());
    taken.extend(module.arc_uniform_returns.iter().cloned());
    for (_, _, pairs) in &module.method_tables {
        taken.extend(pairs.iter().map(|(_, symbol)| symbol.clone()));
    }
    for class in &module.classes {
        taken.extend(class.vtable.iter().map(|(_, symbol)| symbol.clone()));
        taken.extend(class.to_string_symbol.iter().cloned());
    }
    for function in &module.functions {
        for block in &function.blocks {
            for (_, inst, _) in &block.instructions {
                match inst {
                    Instruction::TearOff { code_symbol }
                    | Instruction::AllocClosure { code_symbol, .. } => {
                        taken.insert(code_symbol.clone());
                    }
                    Instruction::AllocClosureTyped {
                        code_symbol, typed, ..
                    } => {
                        taken.insert(code_symbol.clone());
                        taken.insert(typed.clone());
                    }
                    Instruction::FunctionTable(symbols) => taken.extend(symbols.iter().cloned()),
                    Instruction::Const(Constant::Function(symbol)) => {
                        taken.insert(symbol.clone());
                    }
                    _ => {}
                }
                operands(inst, &mut |o| {
                    if let Operand::Constant(Constant::Function(symbol)) = o {
                        taken.insert(symbol.clone());
                    }
                });
            }
            terminator_operands(&block.terminator, &mut |o| {
                if let Operand::Constant(Constant::Function(symbol)) = o {
                    taken.insert(symbol.clone());
                }
            });
        }
    }
    taken
}

/// Whether the pass applies to the module (see the module documentation).
fn applies(module: &Module) -> bool {
    !(module.sdk_library
        || module.arc_memory
        || module.arc_prepared
        || module.arc_aot_ports.is_some()
        || module.dwarf
        || std::env::var("DARTFORGE_NO_CONSTANT_ARGS").is_ok_and(|v| v == "1"))
}

/// Propagates the constant arguments of the module. Returns how many
/// parameters received a constant (zero: nothing changed).
pub(super) fn propagate(module: &mut Module) -> usize {
    if !applies(module) {
        return 0;
    }
    let mut total = 0;
    for _ in 0..ROUNDS {
        let n = round(module);
        if n == 0 {
            break;
        }
        total += n;
    }
    total
}

/// One round: the agreement of the call sites, then the substitution and the
/// folding in each callee that received a constant.
fn round(module: &mut Module) -> usize {
    // The unique symbols: a duplicated one is never a candidate.
    let mut index: HashMap<&str, Option<usize>> = HashMap::new();
    for (position, function) in module.functions.iter().enumerate() {
        index
            .entry(function.symbol.as_str())
            .and_modify(|entry| *entry = None)
            .or_insert(Some(position));
    }
    let taken = address_taken(module);
    let with_errors: HashSet<&str> = module
        .source_errors
        .iter()
        .filter(|(_, errors)| !errors.is_empty())
        .map(|(symbol, _)| symbol.as_str())
        .collect();
    // Per callee: the constant of each parameter so far (`None`: already
    // disagreed or not a constant), and whether some call site was seen.
    let mut agreement: HashMap<usize, Vec<Option<Arg>>> = HashMap::new();
    let mut excluded: HashSet<usize> = HashSet::new();
    for caller in &module.functions {
        let definitions = constant_definitions(caller);
        for (_, inst, _) in caller.blocks.iter().flat_map(|b| &b.instructions) {
            let Instruction::CallStatic { symbol, args, .. } = inst else {
                continue;
            };
            let Some(Some(callee)) = index.get(symbol.as_str()).copied() else {
                continue;
            };
            if excluded.contains(&callee) {
                continue;
            }
            let target = &module.functions[callee];
            if taken.contains(symbol)
                || with_errors.contains(symbol.as_str())
                || target.params.len() != args.len()
                || target.blocks.first().is_none_or(|b| b.id.0 != 0)
                || module.dart_vector_parameters.contains_key(symbol)
            {
                excluded.insert(callee);
                agreement.remove(&callee);
                continue;
            }
            let facts: Vec<Option<Arg>> = args
                .iter()
                .zip(&target.params)
                .map(|(a, (_, _, ty))| argument(a, *ty, &definitions))
                .collect();
            match agreement.get_mut(&callee) {
                None => {
                    agreement.insert(callee, facts);
                }
                Some(current) => {
                    for (fact, seen) in current.iter_mut().zip(facts) {
                        if *fact != seen {
                            *fact = None;
                        }
                    }
                }
            }
        }
    }
    let mut plans: Vec<(usize, Vec<(ValueId, Arg)>)> = agreement
        .into_iter()
        .filter_map(|(callee, facts)| {
            let constants: Vec<(ValueId, Arg)> = module.functions[callee]
                .params
                .iter()
                .zip(facts)
                .filter_map(|((value, _, _), fact)| fact.map(|fact| (*value, fact)))
                .collect();
            (!constants.is_empty()).then_some((callee, constants))
        })
        .collect();
    plans.sort_by_key(|(callee, _)| *callee);
    let mut propagated = 0;
    for (callee, constants) in plans {
        let function = &mut module.functions[callee];
        // A parameter nobody reads: nothing to substitute (and nothing to count).
        let used: HashSet<ValueId> = {
            let mut used = HashSet::new();
            for block in &function.blocks {
                for (_, inst, _) in &block.instructions {
                    operands(inst, &mut |o| {
                        if let Operand::Val(v) = o {
                            used.insert(*v);
                        }
                    });
                }
                terminator_operands(&block.terminator, &mut |o| {
                    if let Operand::Val(v) = o {
                        used.insert(*v);
                    }
                });
            }
            used
        };
        let constants: Vec<(ValueId, Arg)> = constants
            .into_iter()
            .filter(|(v, _)| used.contains(v))
            .collect();
        if constants.is_empty() {
            continue;
        }
        substitute(function, &constants);
        // Lowered locals still live in stack slots at this phase. Expose the
        // substituted parameter through their stores and loads before pruning.
        mem2reg::promote(function);
        fold(function);
        propagated += constants.len();
    }
    propagated
}

/// Replaces the uses of each parameter with its constant. Scalars and null go
/// as immediate operands; a string becomes a `Const` at the entry (the
/// emitter materializes string constants only from a `Const`).
fn substitute(function: &mut Function, constants: &[(ValueId, Arg)]) {
    let (mut next_value, _) = largest_ids(function);
    let mut swap: HashMap<ValueId, Operand> = HashMap::new();
    let mut strings: Vec<(ValueId, Instruction, Type)> = Vec::new();
    for (parameter, arg) in constants {
        let operand = match arg {
            Arg::Null => Operand::Constant(Constant::Null),
            Arg::Bool(b) => Operand::Constant(Constant::Bool(*b)),
            Arg::Int(n) => Operand::Constant(Constant::Int(*n)),
            Arg::Double(bits) => Operand::Constant(Constant::Double(f64::from_bits(*bits))),
            Arg::Str(bytes) => {
                next_value += 1;
                let value = ValueId(next_value);
                strings.push((
                    value,
                    Instruction::Const(Constant::StringWtf8(bytes.clone())),
                    Type::Ref,
                ));
                Operand::Val(value)
            }
        };
        swap.insert(*parameter, operand);
    }
    if !strings.is_empty() {
        // The entry block (id 0, the first) has no `phi`: the constants go first.
        let entry = &mut function.blocks[0].instructions;
        strings.append(entry);
        *entry = strings;
    }
    replace(function, &|v| swap.get(&v).cloned());
}

/// Folds `df.string_equals_object`/`df.string_equals` between two constant
/// strings (or a constant string and a constant null) into the boolean. The
/// lowering emits them for `==` with a `String` receiver
/// (`lower/operators.rs`); the general folding knows nothing about strings.
pub(super) fn fold_string_equality(function: &mut Function) -> bool {
    let definitions = constant_definitions(function);
    let text = |o: &Operand| -> Option<Option<Vec<u8>>> {
        let c = match o {
            Operand::Constant(c) => c.clone(),
            Operand::Val(v) => definitions
                .get(v)
                .filter(|(_, ty)| *ty == Type::Ref)?
                .0
                .clone(),
        };
        match c {
            Constant::Null => Some(None),
            Constant::String(s) => Some(Some(s.into_bytes())),
            Constant::StringWtf8(b) => Some(Some(b)),
            _ => None,
        }
    };
    let mut folded: HashMap<ValueId, Operand> = HashMap::new();
    for (v, inst, ty) in function.blocks.iter().flat_map(|b| &b.instructions) {
        // A materialized string is non-null even though its handle is not an
        // immediate constant. Fold the `os ?? platform` guard so the selected
        // string can reach the following phi and equality test.
        if let Instruction::ICmp(op @ (ICmpOp::Eq | ICmpOp::Ne), a, b) = inst {
            if *ty == Type::I1
                && let (Some(a), Some(b)) = (text(a), text(b))
                && a.is_none() != b.is_none()
            {
                folded.insert(*v, Operand::Constant(Constant::Bool(*op == ICmpOp::Ne)));
            }
            continue;
        }
        let Instruction::CallRuntime {
            name,
            args,
            ret_ty: Type::I1,
        } = inst
        else {
            continue;
        };
        if *ty != Type::I1
            || !matches!(
                name.as_str(),
                "df.string_equals_object" | "df.string_equals"
            )
        {
            continue;
        }
        let [(a, Type::Ref), (b, Type::Ref)] = args.as_slice() else {
            continue;
        };
        let (Some(a), Some(b)) = (text(a), text(b)) else {
            continue;
        };
        // A string never equals null; two strings compare by their code units.
        let equal = match (a, b) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        };
        folded.insert(*v, Operand::Constant(Constant::Bool(equal)));
    }
    if folded.is_empty() {
        return false;
    }
    for block in &mut function.blocks {
        block
            .instructions
            .retain(|(v, _, _)| !folded.contains_key(v));
    }
    replace(function, &|v| folded.get(&v).cloned());
    true
}

/// The folding that a constant parameter enables, to a fixed point.
pub(super) fn fold(function: &mut Function) {
    loop {
        let mut changed = fold_string_equality(function);
        changed |= representation_constants::propagate_literals(function);
        changed |= representation_constants::normalize(function);
        changed |= simplify::fold_constants(function);
        changed |= representation_constants::fold_booleans(function);
        changed |= simplify::remove_unreachable(function);
        changed |= simplify::join_blocks(function);
        if !changed {
            break;
        }
    }
    simplify::remove_dead(function);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(n: u32) -> ValueId {
        ValueId(n)
    }

    /// `resolve(String? os)`: `platform = os ?? "macos"; if (platform ==
    /// "windows") windows() else other()`, the shape of dart_ui's
    /// `PlatformBackendResolver.defaultBackends`.
    fn resolver() -> Function {
        Function {
            symbol: "resolve".into(),
            name: "resolve".into(),
            debug_info: None,
            params: vec![(v(0), "os".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        v(1),
                        Instruction::ICmp(
                            ICmpOp::Eq,
                            Operand::Val(v(0)),
                            Operand::Constant(Constant::Int(0)),
                        ),
                        Type::I1,
                    )],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(v(1)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        v(2),
                        Instruction::Const(Constant::StringWtf8(b"macos".to_vec())),
                        Type::Ref,
                    )],
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(3),
                    instructions: vec![
                        (
                            v(3),
                            Instruction::Phi {
                                incoming: vec![
                                    (BlockId(1), Operand::Val(v(2))),
                                    (BlockId(2), Operand::Val(v(0))),
                                ],
                                ty: Type::Ref,
                            },
                            Type::Ref,
                        ),
                        (
                            v(4),
                            Instruction::Const(Constant::StringWtf8(b"windows".to_vec())),
                            Type::Ref,
                        ),
                        (
                            v(5),
                            Instruction::CallRuntime {
                                name: "df.string_equals_object".into(),
                                args: vec![
                                    (Operand::Val(v(3)), Type::Ref),
                                    (Operand::Val(v(4)), Type::Ref),
                                ],
                                ret_ty: Type::I1,
                            },
                            Type::I1,
                        ),
                    ],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(v(5)),
                        then_block: BlockId(4),
                        else_block: BlockId(5),
                    },
                },
                BasicBlock {
                    id: BlockId(4),
                    instructions: vec![(
                        v(6),
                        Instruction::CallStatic {
                            symbol: "windows".into(),
                            args: vec![],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(v(6)))),
                },
                BasicBlock {
                    id: BlockId(5),
                    instructions: vec![(
                        v(7),
                        Instruction::CallStatic {
                            symbol: "other".into(),
                            args: vec![],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(v(7)))),
                },
            ],
        }
    }

    fn leaf(symbol: &str) -> Function {
        Function {
            symbol: symbol.into(),
            name: symbol.into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
            }],
        }
    }

    fn caller(symbol: &str, argument: Operand) -> Function {
        Function {
            symbol: symbol.into(),
            name: symbol.into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    v(0),
                    Instruction::CallStatic {
                        symbol: "resolve".into(),
                        args: vec![argument],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(Some(Operand::Val(v(0)))),
            }],
        }
    }

    fn calls(function: &Function) -> Vec<String> {
        function
            .blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .filter_map(|(_, i, _)| match i {
                Instruction::CallStatic { symbol, .. } => Some(symbol.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn omitted_optional_folds_the_platform_branch() {
        let mut module = Module {
            functions: vec![
                caller("main", Operand::Constant(Constant::Null)),
                resolver(),
                leaf("windows"),
                leaf("other"),
            ],
            ..Default::default()
        };
        assert_eq!(propagate(&mut module), 1);
        let resolve = &module.functions[1];
        assert_eq!(
            calls(resolve),
            vec!["other".to_string()],
            "{:?}",
            resolve.blocks
        );
        assert_eq!(resolve.params.len(), 1, "the ABI does not change");
        assert!(
            resolve
                .blocks
                .iter()
                .all(|b| !matches!(b.terminator, Terminator::CondBranch { .. })),
            "{:?}",
            resolve.blocks
        );
    }

    #[test]
    fn explicit_string_argument_selects_the_other_branch() {
        let mut main = caller("main", Operand::Val(v(9)));
        main.blocks[0].instructions.insert(
            0,
            (
                v(9),
                Instruction::Const(Constant::StringWtf8(b"windows".to_vec())),
                Type::Ref,
            ),
        );
        let mut module = Module {
            functions: vec![main, resolver(), leaf("windows"), leaf("other")],
            ..Default::default()
        };
        assert_eq!(propagate(&mut module), 1);
        let resolve = &module.functions[1];
        assert_eq!(
            calls(resolve),
            vec!["windows".to_string()],
            "{:?}",
            resolve.blocks
        );
        // Once its null guard and equality fold, the materialized string may
        // itself be dead. The ABI stays intact and every branch is resolved.
        assert_eq!(resolve.params.len(), 1);
        assert!(
            resolve
                .blocks
                .iter()
                .all(|b| !matches!(b.terminator, Terminator::CondBranch { .. }))
        );
    }

    #[test]
    fn constant_parameter_flows_through_a_lowered_local() {
        let mut resolve = resolver();
        let block = &mut resolve.blocks[3];
        block.instructions.splice(
            1..1,
            [
                (v(8), Instruction::Alloca(Type::Ref), Type::Ptr),
                (
                    v(9),
                    Instruction::Store {
                        ptr: Operand::Val(v(8)),
                        val: Operand::Val(v(3)),
                    },
                    Type::Void,
                ),
                (
                    v(10),
                    Instruction::Load {
                        ptr: Operand::Val(v(8)),
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
            ],
        );
        let (_, Instruction::CallRuntime { args, .. }, _) = block.instructions.last_mut().unwrap()
        else {
            panic!("equality")
        };
        args[0].0 = Operand::Val(v(10));
        let mut module = Module {
            functions: vec![
                caller("main", Operand::Constant(Constant::Null)),
                resolve,
                leaf("windows"),
                leaf("other"),
            ],
            ..Default::default()
        };
        assert_eq!(propagate(&mut module), 1);
        assert_eq!(calls(&module.functions[1]), vec!["other".to_string()]);
    }

    #[test]
    fn disagreeing_sites_tear_offs_and_sdk_modules_keep_the_body() {
        let intact = |module: &Module| {
            let resolve = module
                .functions
                .iter()
                .find(|f| f.symbol == "resolve")
                .unwrap();
            let mut names = calls(resolve);
            names.sort();
            assert_eq!(
                names,
                vec!["other".to_string(), "windows".to_string()],
                "{:?}",
                resolve.blocks
            );
        };
        // Two call sites, two constants.
        let mut second = caller("second", Operand::Val(v(9)));
        second.blocks[0].instructions.insert(
            0,
            (
                v(9),
                Instruction::Const(Constant::StringWtf8(b"linux".to_vec())),
                Type::Ref,
            ),
        );
        let mut module = Module {
            functions: vec![
                caller("main", Operand::Constant(Constant::Null)),
                second,
                resolver(),
                leaf("windows"),
                leaf("other"),
            ],
            ..Default::default()
        };
        assert_eq!(propagate(&mut module), 0);
        intact(&module);
        // A computed argument.
        let mut computed = caller("main", Operand::Val(v(9)));
        computed.blocks[0].instructions.insert(
            0,
            (
                v(9),
                Instruction::CallStatic {
                    symbol: "other".into(),
                    args: vec![],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ),
        );
        let mut module = Module {
            functions: vec![computed, resolver(), leaf("windows"), leaf("other")],
            ..Default::default()
        };
        assert_eq!(propagate(&mut module), 0);
        intact(&module);
        // The address taken by a tear-off.
        let mut main = caller("main", Operand::Constant(Constant::Null));
        main.blocks[0].instructions.push((
            v(1),
            Instruction::TearOff {
                code_symbol: "resolve".into(),
            },
            Type::Ref,
        ));
        let mut module = Module {
            functions: vec![main, resolver(), leaf("windows"), leaf("other")],
            ..Default::default()
        };
        assert_eq!(propagate(&mut module), 0);
        intact(&module);
        // An SDK library module: open world.
        let mut module = Module {
            functions: vec![
                caller("main", Operand::Constant(Constant::Null)),
                resolver(),
                leaf("windows"),
                leaf("other"),
            ],
            sdk_library: true,
            ..Default::default()
        };
        assert_eq!(propagate(&mut module), 0);
        intact(&module);
    }

    #[test]
    fn string_equality_folds_only_between_constants() {
        let mut function = resolver();
        // `os` unknown: nothing folds.
        assert!(!fold_string_equality(&mut function));
        // Two constants: the comparison becomes `false`, and null against a string too.
        let equals = |a: Operand, b: Operand| Function {
            symbol: "eq".into(),
            name: "eq".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::I1,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        v(1),
                        Instruction::Const(Constant::StringWtf8(b"a".to_vec())),
                        Type::Ref,
                    ),
                    (
                        v(2),
                        Instruction::Const(Constant::String("a".into())),
                        Type::Ref,
                    ),
                    (
                        v(3),
                        Instruction::CallRuntime {
                            name: "df.string_equals_object".into(),
                            args: vec![(a, Type::Ref), (b, Type::Ref)],
                            ret_ty: Type::I1,
                        },
                        Type::I1,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(v(3)))),
            }],
        };
        let mut same = equals(Operand::Val(v(1)), Operand::Val(v(2)));
        assert!(fold_string_equality(&mut same));
        assert!(matches!(
            same.blocks[0].terminator,
            Terminator::Return(Some(Operand::Constant(Constant::Bool(true))))
        ));
        let mut null = equals(Operand::Val(v(1)), Operand::Constant(Constant::Null));
        assert!(fold_string_equality(&mut null));
        assert!(matches!(
            null.blocks[0].terminator,
            Terminator::Return(Some(Operand::Constant(Constant::Bool(false))))
        ));
    }
}
