//! Translation of the audited externs into the token plan of the HIR.
//! Does not presume contracts for absent symbols nor certify provenance/borrows.

mod trivial_phis;

use super::OwnerOrigin;
use super::{
    TokenEffect, Ownership, ScopePlan, TokenPlan, verify_scopes, verify_tokens,
};
use crate::hir::*;
use dartforge_runtime::ownership::{Contract, ParamMode, ResultMode};
use std::collections::{HashMap, HashSet, VecDeque};

/// Contracts of the prelude `@df.*` helpers that the lowering calls through
/// `CallRuntime` (`llvm/ir_{boxes,strings,lists}.rs`). They are inline IR,
/// outside the runtime catalog: they read scalar fields of the borrowed object
/// (and, in `@df.string_write`, write units of a string not yet
/// published), without allocating, retaining, throwing or calling Dart — what their
/// effects (`HELPER_EFFECTS`) say and what the test
/// `helpers_agree_with_effects` checks. `@df.typed_base` borrows
/// the base of the view, which it holds in an immutable edge. The derived
/// addresses (`@df.typed_data`, `@df.core_storage`, read by
/// `@df.core_shape`) are scalars whose accesses `derived_set.rs` holds with
/// `@df.keep_alive(list)`; `@df.word_ref` (a `Ref` read from the
/// storage) stays out: under ARC the lowering uses the Owned read.
pub const HELPER_CONTRACTS: &[Contract] = &[
    Contract { name: "df.bit_smi", parameters: &[ParamMode::Borrow], result: ResultMode::ScalarI64, persistent_retention: false, invalidates_borrows: false, may_fail: false, calls_dart: false },
    Contract { name: "df.smi_value", parameters: &[ParamMode::Borrow], result: ResultMode::ScalarI64, persistent_retention: false, invalidates_borrows: false, may_fail: false, calls_dart: false },
    Contract { name: "df.identical", parameters: &[ParamMode::Borrow, ParamMode::Borrow], result: ResultMode::ScalarI1, persistent_retention: false, invalidates_borrows: false, may_fail: false, calls_dart: false },
    Contract { name: "df.core_len", parameters: &[ParamMode::Borrow], result: ResultMode::ScalarI64, persistent_retention: false, invalidates_borrows: false, may_fail: false, calls_dart: false },
    Contract { name: "df.core_len_writable", parameters: &[ParamMode::Borrow, ParamMode::Scalar], result: ResultMode::ScalarI64, persistent_retention: false, invalidates_borrows: false, may_fail: false, calls_dart: false },
    Contract { name: "df.string_write", parameters: &[ParamMode::Borrow, ParamMode::Scalar, ParamMode::Scalar], result: ResultMode::Void, persistent_retention: false, invalidates_borrows: false, may_fail: false, calls_dart: false },
    Contract { name: "df.string_hash", parameters: &[ParamMode::Borrow], result: ResultMode::ScalarI64, persistent_retention: false, invalidates_borrows: false, may_fail: false, calls_dart: false },
    Contract { name: "df.string_equals", parameters: &[ParamMode::Borrow, ParamMode::Borrow], result: ResultMode::ScalarI1, persistent_retention: false, invalidates_borrows: false, may_fail: false, calls_dart: false },
    Contract { name: "df.string_equals_object", parameters: &[ParamMode::Borrow, ParamMode::Borrow], result: ResultMode::ScalarI1, persistent_retention: false, invalidates_borrows: false, may_fail: false, calls_dart: false },
    Contract { name: "df.string_len", parameters: &[ParamMode::Borrow], result: ResultMode::ScalarI64, persistent_retention: false, invalidates_borrows: false, may_fail: false, calls_dart: false },
    Contract { name: "df.string_code_unit", parameters: &[ParamMode::Borrow, ParamMode::Scalar], result: ResultMode::ScalarI64, persistent_retention: false, invalidates_borrows: false, may_fail: false, calls_dart: false },
    // `num` in Ref as double: the path that is not Smi/Mint unboxes through the
    // runtime (a pending TypeError), as the marks of the helper say.
    Contract { name: "df.num_to_double", parameters: &[ParamMode::Borrow], result: ResultMode::ScalarF64, persistent_retention: false, invalidates_borrows: true, may_fail: true, calls_dart: false },
    Contract { name: "df.typed_base", parameters: &[ParamMode::Borrow], result: ResultMode::BorrowArg(0), persistent_retention: false, invalidates_borrows: false, may_fail: false, calls_dart: false },
    Contract { name: "df.typed_bytes", parameters: &[ParamMode::Borrow], result: ResultMode::ScalarI64, persistent_retention: false, invalidates_borrows: false, may_fail: false, calls_dart: false },
    Contract { name: "df.typed_cid", parameters: &[ParamMode::Borrow], result: ResultMode::ScalarI64, persistent_retention: false, invalidates_borrows: false, may_fail: false, calls_dart: false },
    Contract { name: "df.typed_offset", parameters: &[ParamMode::Borrow], result: ResultMode::ScalarI64, persistent_retention: false, invalidates_borrows: false, may_fail: false, calls_dart: false },
    Contract { name: "df.typed_len", parameters: &[ParamMode::Borrow], result: ResultMode::ScalarI64, persistent_retention: false, invalidates_borrows: false, may_fail: false, calls_dart: false },
    Contract { name: "df.typed_data", parameters: &[ParamMode::Borrow], result: ResultMode::ScalarI64, persistent_retention: false, invalidates_borrows: false, may_fail: false, calls_dart: false },
    Contract { name: "df.core_storage", parameters: &[ParamMode::Borrow], result: ResultMode::ScalarI64, persistent_retention: false, invalidates_borrows: false, may_fail: false, calls_dart: false },
    Contract { name: "df.core_shape", parameters: &[ParamMode::Scalar], result: ResultMode::ScalarI64, persistent_retention: false, invalidates_borrows: false, may_fail: false, calls_dart: false },
    Contract { name: "df.keep_alive", parameters: &[ParamMode::Borrow], result: ResultMode::Void, persistent_retention: false, invalidates_borrows: false, may_fail: false, calls_dart: false },
    Contract { name: "df.typed_len_writable", parameters: &[ParamMode::Borrow, ParamMode::Scalar], result: ResultMode::ScalarI64, persistent_retention: false, invalidates_borrows: false, may_fail: false, calls_dart: false },
];

/// The ownership contract of `name`: that of an IR helper
/// ([`HELPER_CONTRACTS`]) or that of the runtime catalog.
///
/// # Errors
/// A symbol without an explicit contract.
///
/// ```
/// use dartforge_emit_native::optimize::arc::contract;
/// assert!(contract("df.string_len").is_ok());
/// assert!(contract("dartforge_arc_list_len_v1").is_ok());
/// assert!(contract("df.word_ref").is_err());
/// ```
pub fn contract(name_str: &str) -> Result<&'static Contract, &'static str> {
    if let Some(base) = name_str.strip_prefix(super::NATIVE_SCOPE_PREFIX) {
        return contract(base);
    }
    if let Some(base) = name_str.strip_prefix(super::GENERIC_OWNED_PREFIX) {
        // Only the marks matter here; the parameters come from the types of the call
        // (`runtime_call_contract`).
        let e = crate::llvm::externs::effects_of(base);
        return Ok(&GENERIC_OWNED[usize::from(e.throws || e.calls_dart) * 2 + usize::from(e.calls_dart)]);
    }
    #[cfg(test)]
    if let Some(c) = TEST_CONTRACTS.iter().find(|c| c.name == name_str) {
        return Ok(c);
    }
    match HELPER_CONTRACTS.iter().find(|c| c.name == name_str) {
        Some(c) => Ok(c),
        None => dartforge_runtime::ownership::contract(name_str),
    }
}

/// Contract shapes that the real catalog no longer has, but that the preparation
/// keeps supporting: a result borrowed from argument 0 that only
/// exists on the normal exit.
#[cfg(test)]
pub(super) const TEST_CONTRACTS: &[Contract] = &[Contract {
    name: "df.test.fallible_field",
    parameters: &[ParamMode::Borrow, ParamMode::Scalar],
    result: ResultMode::BorrowArg(0),
    persistent_retention: false,
    invalidates_borrows: false,
    may_fail: true,
    calls_dart: false,
}];

/// The marks of the generic Owned thunks, by (may fail, calls Dart).
const GENERIC_OWNED: [Contract; 4] = {
    const fn c(may_fail: bool, calls_dart: bool) -> Contract {
        Contract { name: "df.arc.owned.*", parameters: &[], result: ResultMode::Owned,
            persistent_retention: true, invalidates_borrows: true, may_fail: may_fail, calls_dart: calls_dart }
    }
    [c(false, false), c(false, true), c(true, false), c(true, true)]
};

/// The contract of the generic Owned thunk of `base` with the arguments of the
/// call: each borrowed `Ref` (SSA or null), the scalars by value; the
/// result is an owner; failure and reentry are those of the base (`effects.tsv`).
fn generic_owned_contract(base: &str, args: &[(Operand, Type)]) -> Result<RuntimeCallContract, String> {
    let e = crate::llvm::externs::effects_of(base);
    for (n, (op, ty)) in args.iter().enumerate() {
        match (ty, op) {
            (Type::Ref, Operand::Val(_) | Operand::Constant(Constant::Null)) => {}
            (Type::Ref, _) => return Err(format!("{base}: Ref argument {n} requires SSA or null")),
            // Ptr: constant data of the module (the table of a literal); an
            // address of a scalar local does not leave the function (`locals.rs`).
            (Type::I64 | Type::F64 | Type::I8 | Type::I1 | Type::Ptr, Operand::Val(_)) => {}
            (Type::I64 | Type::I8, Operand::Constant(Constant::Int(_) | Constant::Bool(_))) => {}
            (Type::I1, Operand::Constant(Constant::Bool(_))) => {}
            (Type::F64, Operand::Constant(Constant::Double(_))) => {}
            _ => return Err(format!("{base}: argument {n} without a representation of the Owned thunk")),
        }
    }
    Ok(RuntimeCallContract {
        effect: TokenEffect { may_fail: e.throws || e.calls_dart, ..Default::default() },
        result: Ownership::Owned,
        persistent_retention: true,
        invalidates_borrows: true,
        calls_dart: e.calls_dart,
    })
}

/// Produces contracts of the Ref parameters by the Dart calling convention (§20.1).
///
/// A Ref is borrowed from the caller during the invocation. Other types require
/// their own producers: I64 does not become a scalar/reference by its width.
/// Use only for functions that follow this convention, before the production
/// of the instructions. Does not certify contracts of callees nor insert RC/cleanup.
///
/// # Errors
/// A repeated parameter or a supplied contract other than Borrowed(Caller, 0)
/// for a Ref parameter. No class is published on error.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "f".into(), name: "f".into(), debug_info: None,
///     params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
///         (ValueId(1), Instruction::ArcCopy { value: Operand::Val(ValueId(0)) }, Type::Ref),
///         (ValueId(2), Instruction::ArcDrop { value: Operand::Val(ValueId(1)) }, Type::Void)],
///         terminator: Terminator::Return(None) }] };
/// let mut classes = HashMap::new();
/// produce_dart_ref_parameters(&f, &mut classes)?;
/// produce_and_verify_tokens(&f, &mut classes, &mut TokenPlan::default(),
///     &FunctionTables::default(), &ScopePlan::default())?;
/// # Ok::<(), String>(())
/// ```
pub fn produce_dart_ref_parameters(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
) -> Result<(), String> {
    let expected = Ownership::Borrowed {
        owner: OwnerOrigin::Caller,
        scope: 0,
    };
    let mut ids = HashSet::new();
    let mut produced = Vec::new();
    for (v, _, ty) in &f.params {
        if !ids.insert(*v) {
            return Err(format!("repeated Dart parameter v{}", v.0));
        }
        if *ty == Type::Ref {
            if classes.get(v).is_some_and(|c| *c != expected) {
                return Err(format!(
                    "Dart Ref parameter v{} requires Borrowed(Caller, 0)",
                    v.0
                ));
            }
            produced.push(*v);
        }
    }
    for v in produced {
        classes.insert(v, expected.clone());
    }
    Ok(())
}

/// Inserts retention at the Dart Ref returns and publishes only the verified function.
///
/// A Borrowed/Trivial SSA return gets an ArcCopy; Owned transfers its token.
/// A direct null needs no counting, and direct permanent literals are evaluated
/// by Const before the copy. Requires an Owned return plan for Ref. The other
/// cleanups, contracts of callees and uncovered metadata are premises of the
/// lowering; does not insert drops, exceptional cleanup or suspension protection.
///
/// # Errors
/// Invalid HIR/contracts, exhausted IDs or a failure of the complete Dart wrapper.
/// Function, classes and plan stay intact. Returns the number of copies.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let mut f = Function { symbol: "identity".into(), name: "identity".into(), debug_info: None,
///     params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] };
/// let mut plan = TokenPlan { ret: TokenReturn::Owned, ..Default::default() };
/// assert_eq!(insert_dart_return_retention(&mut f, &mut HashMap::new(), &mut plan,
///     &FunctionTables::default(), &ScopePlan::default())?, 1);
/// # Ok::<(), String>(())
/// ```
pub fn insert_dart_return_retention(
    f: &mut Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
    tables: &FunctionTables,
    scopes: &ScopePlan,
) -> Result<usize, String> {
    insert_dart_exits(f, classes, plan, tables, scopes, false).map(|(copies, _)| copies)
}

/// Inserts the retention of the return and releases the remaining tokens at each Dart Return.
///
/// Uses the same flow as the verifier: the returned token is transferred, and the
/// others get an ArcDrop before the return, in deterministic ID order.
/// Includes returns of already prepared exceptional paths. Requires joins
/// with compatible inventories; does not split edges, close frames or prepare
/// finally/cancellation/suspension. Uncovered contracts stay explicit.
/// Throw/Resume exits return only the placeholder and release local tokens;
/// a Guard exit requires splitting success/error in the CFG before this pass.
///
/// # Errors
/// Invalid HIR/contracts, exhausted IDs or a flow/scope/frame failure.
/// Function and maps stay intact. Returns (retentions, releases).
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let mut f = Function { symbol: "f".into(), name: "f".into(), debug_info: None,
///     params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
///         (ValueId(1), Instruction::ArcCopy { value: Operand::Val(ValueId(0)) }, Type::Ref)],
///         terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] };
/// let mut plan = TokenPlan { ret: TokenReturn::Owned, ..Default::default() };
/// assert_eq!(insert_arc_dart_exits(&mut f, &mut HashMap::new(), &mut plan,
///     &FunctionTables::default(), &ScopePlan::default())?, (1, 1));
/// # Ok::<(), String>(())
/// ```
pub fn insert_arc_dart_exits(
    f: &mut Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
    tables: &FunctionTables,
    scopes: &ScopePlan,
) -> Result<(usize, usize), String> {
    insert_dart_exits(f, classes, plan, tables, scopes, true)
}

fn insert_dart_exits(
    f: &mut Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
    tables: &FunctionTables,
    scopes: &ScopePlan,
    cleanup: bool,
) -> Result<(usize, usize), String> {
    let mut new_one = f.clone();
    super::tokens::normalize_exits_throws(&mut new_one, tables)?;
    let mut new_classes = classes.clone();
    let mut new_plan = plan.clone();
    produce_dart_ref_parameters(&new_one, &mut new_classes)?;
    produce_arc_contracts(&new_one, &mut new_classes, &mut new_plan)?;
    super::classified_liveness(&new_one, &new_classes)?;
    // Also reserves metadata IDs: an insertion cannot mask a stale entry
    // by making it coincide with a newly created definition.
    let mut next_item = new_one
        .params
        .iter()
        .map(|(v, _, _)| v.0)
        .chain(
            new_one.blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .map(|(v, _, _)| v.0),
        )
        .chain(new_plan.instructions.keys().map(|v| v.0))
        .chain(new_plan.pendings.keys().map(|v| v.0))
        .chain(tables.invocations.keys().map(|v| v.0))
        .chain(scopes.before.keys().map(|v| v.0))
        .max()
        .map_or(0, |v| u64::from(v) + 1);
    let mut id = || -> Result<ValueId, String> {
        let v = u32::try_from(next_item).map_err(|_| "SSA IDs exhausted while preparing a Dart return")?;
        next_item += 1;
        Ok(ValueId(v))
    };
    let copies = copy_returns(&mut new_one, &new_classes, &mut id)?;
    let mut releases = 0;
    if cleanup {
        produce_arc_contracts(&new_one, &mut new_classes, &mut new_plan)?;
        let exits =
            super::tokens::cleanup_exits(&new_one, &new_classes, tables, &new_plan)?;
        for b in &mut new_one.blocks {
            if let Some(remaining) = exits.get(&b.id) {
                for of in remaining {
                    b.instructions.push((
                        id()?,
                        Instruction::ArcDrop {
                            value: Operand::Val(*of),
                        },
                        Type::Void,
                    ));
                    releases += 1;
                }
            }
        }
    }
    produce_and_verify_dart_tokens(&new_one, &mut new_classes, &mut new_plan, tables, scopes)?;
    *f = new_one;
    *classes = new_classes;
    *plan = new_plan;
    Ok((copies, releases))
}

pub(super) fn copy_returns(
    f: &mut Function,
    classes: &HashMap<ValueId, Ownership>,
    id: &mut impl FnMut() -> Result<ValueId, String>,
) -> Result<usize, String> {
    let mut copies = 0;
    if f.return_ty == Type::Ref {
        for b in &mut f.blocks {
            let Terminator::Return(Some(op)) = &b.terminator else {
                continue;
            };
            let mut op = op.clone();
            match &op {
                Operand::Val(v) => match classes.get(v) {
                    Some(Ownership::Owned) => continue,
                    Some(_) => {},
                    None => return Err(format!("return in {} b{}: v{} without classification", f.symbol, b.id.0, v.0)),
                },
                Operand::Constant(Constant::Null) => continue,
                Operand::Constant(c @ (Constant::String(_) | Constant::StringWtf8(_))) => {
                    let v = id()?;
                    b.instructions
                        .push((v, Instruction::Const(c.clone()), Type::Ref));
                    op = Operand::Val(v);
                }
                _ => {}
            }
            let v = id()?;
            b.instructions
                .push((v, Instruction::ArcCopy { value: op }, Type::Ref));
            b.terminator = Terminator::Return(Some(Operand::Val(v)));
            copies += 1;
        }
    }
    Ok(copies)
}

/// Produces and verifies parameters/instructions by the Dart convention, atomically.
///
/// Ref parameters are Borrowed(Caller,0). A Ref result requires an Owned return
/// in the plan; results of other types require a Trivial return. The lowering
/// must prepare transfers/cleanup before this call. Non-Ref types
/// of the parameters and instructions outside the coverage require semantic contracts.
/// Does not infer contracts of callees nor insert RC/cleanup.
///
/// # Errors
/// An incompatible return convention, a parameter conflict or any error
/// of produce_and_verify_tokens. Classes and plan stay intact.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "identity".into(), name: "identity".into(), debug_info: None,
///     params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
///         (ValueId(1), Instruction::ArcCopy { value: Operand::Val(ValueId(0)) }, Type::Ref)],
///         terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }] };
/// let mut plan = TokenPlan { ret: TokenReturn::Owned, ..Default::default() };
/// produce_and_verify_dart_tokens(&f, &mut HashMap::new(), &mut plan,
///     &FunctionTables::default(), &ScopePlan::default())?;
/// # Ok::<(), String>(())
/// ```
pub fn produce_and_verify_dart_tokens(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
    tables: &FunctionTables,
    scopes: &ScopePlan,
) -> Result<HashMap<ValueId, RuntimeCallContract>, String> {
    let ret = if f.return_ty == Type::Ref {
        super::TokenReturn::Owned
    } else {
        super::TokenReturn::Trivial
    };
    if plan.ret != ret {
        return Err(format!(
            "Dart convention in {} requires an Owned Ref result and Trivial other results ({:?} with {:?})",
            f.symbol, f.return_ty, plan.ret
        ));
    }
    let mut new_classes = classes.clone();
    let mut new_plan = plan.clone();
    produce_dart_ref_parameters(f, &mut new_classes)?;
    let contracts =
        produce_and_verify_tokens(f, &mut new_classes, &mut new_plan, tables, scopes)?;
    *classes = new_classes;
    *plan = new_plan;
    Ok(contracts)
}

/// Produces ARC metadata and publishes it after verifying CFG/SSA, tokens and scopes.
///
/// Parameters and instructions outside the coverage of the producer require semantic
/// contracts supplied by the caller. The CFG and the plans must be of the same
/// version of the function. Local frames require LIFO opening/closing and a
/// consistent stack per path; imported IDs/aliases of frames are rejected.
/// Constant limits are checked when the capacity is known. Does not insert
/// RC nor certify dynamic limits, general provenance, slot aliases or Finalizable.
/// Rejects derived borrows after conservative barriers, without producing a keepalive.
///
/// # Errors
/// Invalid CFG/SSA, a production failure, an incomplete inventory, an unavailable/unconsumed token,
/// an incompatible exceptional CFG or a borrow out of scope. In any
/// of those cases, classes and token plan stay intact.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "f".into(), name: "f".into(), debug_info: None,
///     params: vec![], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
///         (ValueId(0), Instruction::ArcCopy { value: Operand::Constant(Constant::Null) }, Type::Ref),
///         (ValueId(1), Instruction::ArcDrop { value: Operand::Val(ValueId(0)) }, Type::Void)],
///         terminator: Terminator::Return(None) }] };
/// produce_and_verify_tokens(&f, &mut HashMap::new(), &mut TokenPlan::default(),
///     &FunctionTables::default(), &ScopePlan::default())?;
/// # Ok::<(), String>(())
/// ```
pub fn produce_and_verify_tokens(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
    tables: &FunctionTables,
    scopes: &ScopePlan,
) -> Result<HashMap<ValueId, RuntimeCallContract>, String> {
    super::ssa::verify(f)?;
    let mut new_classes = classes.clone();
    let mut new_plan = plan.clone();
    let contracts = produce_arc_contracts(f, &mut new_classes, &mut new_plan)?;
    verify_tokens(f, &new_classes, tables, &new_plan)?;
    verify_scopes(f, &new_classes, scopes)?;
    super::frames::verify_with_plan(f, &new_plan)?;
    *classes = new_classes;
    *plan = new_plan;
    Ok(contracts)
}

/// Produces result and consumption classes of all the runtime calls of the function.
///
/// Applies the changes only after checking all the calls. Parameters,
/// ordinary operations and Dart calls require metadata from other producers.
/// Returns the contracts for later retention/invalidation analysis.
/// Recognizes an explicit pending check at the end of the block and produces
/// the map of runtime exits; unrecognized shapes require another producer.
/// Checks the SSA types of the arguments, but not their dominance/provenance.
/// Does not insert counters nor clean scopes.
///
/// # Errors
/// An extern without a contract, an invalid signature, an absent SSA argument or one of a type
/// different from the annotation of the call, a result incompatible with the type
/// of the instruction, repeated IDs or a conflict with an already supplied class/effect.
/// Both inputs stay intact on error.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "f".into(), name: "f".into(), debug_info: None,
///     params: vec![], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![(ValueId(0),
///         Instruction::CallRuntime { name: "dartforge_arc_collect".into(), args: vec![], ret_ty: Type::Void }, Type::Void)],
///         terminator: Terminator::Return(None) }] };
/// let mut classes = HashMap::new();
/// let mut plan = TokenPlan::default();
/// produce_runtime_contracts(&f, &mut classes, &mut plan)?;
/// assert_eq!(classes[&ValueId(0)], Ownership::Trivial);
/// assert!(!plan.instructions[&ValueId(0)].may_fail);
/// # Ok::<(), String>(())
/// ```
pub fn produce_runtime_contracts(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
) -> Result<HashMap<ValueId, RuntimeCallContract>, String> {
    produce(f, classes, plan, false)
}

/// Produces classes of the explicit ARC operations and audited runtime contracts.
///
/// Copy/move/load produce Owned; drop/store produce Trivial. ARC operations
/// get no entry in TokenPlan, since the verifier has its own rules.
/// ICmp/FCmp/LNot produce a Trivial bool without consumption or an exceptional exit.
/// Unbox bool produces a Trivial I1 with the fallible Borrow contract of the u8 ABI;
/// the LLVM helper converts the result and preserves the pending check.
/// Scalar/null constants and permanent literals also produce Trivial,
/// with the type determined by the variant of the constant, never by width.
/// Integer/floating arithmetic and numeric conversions produce Trivial;
/// operands must have a Trivial contract and compatible scalar representations. Domain/overflow
/// guards and Dart semantics remain the responsibility of the lowering.
/// F64/I64 bitcasts require a Trivial scalar origin; width does not certify ownership.
/// ZExt/Trunc between I1/I8/I64 require a Trivial origin, the exact type and a width
/// increase/decrease respectively; they do not convert handles or pointers into scalars.
/// Scalar locals at the entry require a non-escaping address, Trivial typed
/// stores and initialization on all paths up to each read.
/// Phi I1/I64/F64 requires entries of the same type with a Trivial contract or matching
/// constants, and a known origin outside the Phi cycle, including in loops.
/// An explicitly Trivial Ref Phi requires Trivial/null entries and an external origin;
/// the supplied class does not allow erasing the ownership of a managed entry.
/// Ref Phis without an Owned demand and with a known Trivial/null origin get
/// Trivial automatically, including anchored cycles.
/// Ref Phis not yet classified automatically preserve identical Borrowed contracts
/// in the known entries; chains and anchored cycles do not depend
/// on the order of the blocks. Pending entries are checked after the propagation.
/// A Ref Phi not yet classified requires owned/null entries and an origin external
/// to the Phi/move cycle. Parameters and other operations require their own producers.
/// Does not insert ARC nor certify the life of the slots, provenance or cleanup.
///
/// # Errors
/// The errors of produce_runtime_contracts, an incompatible type of an ARC operation,
/// a class conflict or an attempt to overwrite its rule in the plan.
/// An owned Phi with a borrowed/unclassified entry or a cycle without an origin.
/// No map is changed on error.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "f".into(), name: "f".into(), debug_info: None,
///     params: vec![], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
///         (ValueId(0), Instruction::ArcCopy { value: Operand::Constant(Constant::Null) }, Type::Ref),
///         (ValueId(1), Instruction::ArcDrop { value: Operand::Val(ValueId(0)) }, Type::Void)],
///         terminator: Terminator::Return(None) }] };
/// let mut classes = HashMap::new();
/// let mut plan = TokenPlan::default();
/// produce_arc_contracts(&f, &mut classes, &mut plan)?;
/// verify_tokens(&f, &classes, &FunctionTables::default(), &plan)?;
/// # Ok::<(), String>(())
/// ```
pub fn produce_arc_contracts(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
) -> Result<HashMap<ValueId, RuntimeCallContract>, String> {
    let mut new_classes = classes.clone();
    let mut new_plan = plan.clone();
    let locals = super::locals::produce(f, &mut new_classes, &mut new_plan)?;
    let contracts = produce(f, &mut new_classes, &mut new_plan, true)?;
    produce_phi(f, &mut new_classes, &new_plan)?;
    check_typed_scalars(f, &new_classes)?;
    super::locals::verify_values(f, &locals, &new_classes)?;
    super::pure_set::check_origins(f, &new_classes)?;
    *classes = new_classes;
    *plan = new_plan;
    Ok(contracts)
}

/// [`produce_arc_contracts`] without resolving the Phis nor the checks that
/// depend on them. Publishes nothing on error.
pub(super) fn produce_contracts_without_phi(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
) -> Result<(), String> {
    let mut new_classes = classes.clone();
    let mut new_plan = plan.clone();
    super::locals::produce(f, &mut new_classes, &mut new_plan)?;
    produce(f, &mut new_classes, &mut new_plan, true)?;
    *classes = new_classes;
    *plan = new_plan;
    Ok(())
}

/// Each scalar argument of a `TypedCall` (including the Phis, already
/// classified) is Trivial: a Dart word does not go through a scalar slot.
fn check_typed_scalars(f: &Function, classes: &HashMap<ValueId, Ownership>) -> Result<(), String> {
    for (_, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
        if let Instruction::TypedCall { args, .. } = inst {
            for (op, t) in args {
                if *t != Type::Ref && let Operand::Val(v) = op && classes.get(v) != Some(&Ownership::Trivial) {
                    return Err(format!("v{}: typed scalar argument requires Trivial, without a Dart word", v.0));
                }
            }
        }
    }
    Ok(())
}

/// Resolves the ownership of the Ref Phis, including cycles with a known origin.
/// The availability and the simultaneous consumption per edge are proven later
/// by the token verifier, not by the connectivity of this graph.
fn produce_phi(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &TokenPlan,
) -> Result<(), String> {
    let defs: HashMap<_, _> = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .map(|(v, i, _)| (*v, i))
        .collect();
    let mut candidates = HashSet::new();
    let types: HashMap<_, _> = f
        .params
        .iter()
        .map(|(v, _, ty)| (*v, *ty))
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .map(|(v, _, ty)| (*v, *ty)),
        )
        .collect();
    // Explicit transfer/consumption demands keep the logical token,
    // including for null, whose physical count may be empty.
    let mut require_owned: HashSet<_> = plan
        .instructions
        .values()
        .flat_map(|e| e.always.iter().chain(&e.success).chain(&e.error))
        .copied()
        .collect();
    for b in &f.blocks {
        for (v, inst, _) in &b.instructions {
            let op = match inst {
                Instruction::ArcMove { value }
                | Instruction::ArcDrop { value }
                | Instruction::ArcStoreStrong {
                    value,
                    mode: StrongStoreMode::Move,
                    ..
                } => Some(value),
                _ => None,
            };
            if let Some(Operand::Val(of)) = op {
                require_owned.insert(*of);
            }
            if classes.get(v) == Some(&Ownership::Owned) {
                require_owned.insert(*v);
            }
        }
        if plan.ret == super::TokenReturn::Owned
            && let Terminator::Return(Some(Operand::Val(v))) = b.terminator
        {
            require_owned.insert(v);
        }
    }
    // Return demand on a mixed parent does not manufacture tokens for a
    // separately proved null/permanent-only child. Direct transfers and returns
    // still demand their own logical token, including physically empty null.
    let trivial_children = trivial_phis::prove(f, classes, &require_owned);
    loop {
        let previous = require_owned.len();
        for v in require_owned.clone() {
            if let Some(Instruction::Phi { incoming, .. }) = defs.get(&v) {
                require_owned.extend(incoming.iter().filter_map(|(_, op)| {
                    if let Operand::Val(of) = op {
                        (!trivial_children.contains(of)).then_some(*of)
                    } else {
                        None
                    }
                }));
            }
        }
        if previous == require_owned.len() {
            break;
        }
    }
    // A null anchor does not prove that all the still unknown Phis are
    // trivial. First it eliminates the component that depends on Owned, a borrow
    // or an origin without a contract; only then it propagates the remaining trivial anchors.
    let free_set: HashSet<_> = f.blocks.iter().flat_map(|b| &b.instructions)
        .filter_map(|(v, i, ty)| (*ty == Type::Ref && classes.get(v).is_none()
            && !require_owned.contains(v) && matches!(i, Instruction::Phi { .. })).then_some(*v))
        .collect();
    let mut trivial_parents: HashMap<ValueId, Vec<ValueId>> = HashMap::new();
    let mut invalid_set = VecDeque::new();
    let mut trivial_anchors = VecDeque::new();
    for v in &free_set {
        let Instruction::Phi { incoming, .. } = defs[v] else { unreachable!() };
        let mut compatible = !incoming.is_empty();
        let mut anchor = false;
        for (_, op) in incoming {
            match op {
                Operand::Constant(Constant::Null) => anchor = true,
                Operand::Val(of) if types.get(of) == Some(&Type::Ref)
                    && classes.get(of) == Some(&Ownership::Trivial) => anchor = true,
                Operand::Val(of) if free_set.contains(of) => {
                    trivial_parents.entry(*of).or_default().push(*v);
                },
                _ => compatible = false,
            }
        }
        if !compatible { invalid_set.push_back(*v); }
        if anchor { trivial_anchors.push_back(*v); }
    }
    let mut compatible_ones = free_set;
    while let Some(v) = invalid_set.pop_front() {
        if compatible_ones.remove(&v) {
            invalid_set.extend(trivial_parents.get(&v).into_iter().flatten().copied());
        }
    }
    while let Some(v) = trivial_anchors.pop_front() {
        if compatible_ones.contains(&v) && classes.get(&v).is_none() {
            classes.insert(v, Ownership::Trivial);
            trivial_anchors.extend(trivial_parents.get(&v).into_iter().flatten().copied());
        }
    }
    // Resolves first the borrows with known premises. Does not turn
    // Owned entries into an implicit borrow; a cycle needs an external anchor.
    let mut borrowed_set = HashMap::new();
    loop {
        let mut changed = false;
        for (v, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if *ty != Type::Ref || classes.contains_key(v) {
                continue;
            }
            let Instruction::Phi { incoming, .. } = inst else {
                continue;
            };
            let mut origin = None;
            let compatible_ones = !incoming.is_empty()
                && incoming.iter().all(|(_, op)| match op {
                    Operand::Constant(Constant::Null) => true,
                    Operand::Val(of) if types.get(of) == Some(&Type::Ref) => {
                        match classes.get(of) {
                            Some(c @ Ownership::Borrowed { .. }) => {
                                if let Some((_, previous)) = &origin {
                                    previous == c
                                } else {
                                    origin = Some((*of, c.clone()));
                                    true
                                }
                            }
                            Some(Ownership::Trivial) => true,
                            None if matches!(
                                defs.get(of),
                                Some(Instruction::Phi { ty: Type::Ref, .. })
                            ) =>
                            {
                                true
                            }
                            _ => false,
                        }
                    }
                    _ => false,
                });
            if compatible_ones && let Some((of, mut class)) = origin {
                borrowed_set.insert(*v, class.clone());
                // Local values do not declare Caller directly: they keep
                // the dependency on the parameter that sustains that borrow.
                if let Ownership::Borrowed {
                    owner: OwnerOrigin::Caller,
                    scope,
                } = class
                {
                    class = Ownership::Borrowed {
                        owner: OwnerOrigin::Value(of),
                        scope,
                    };
                }
                classes.insert(*v, class);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // The anchor allows walking the cycle, but does not exempt checking each
    // entry after the fixed point supplies the classes of the dependent Phis.
    for (v, origin) in &borrowed_set {
        let Instruction::Phi { incoming, .. } = defs[v] else {
            unreachable!()
        };
        if incoming.iter().any(|(_, op)| match op {
            Operand::Constant(Constant::Null) => false,
            Operand::Val(of) if types.get(of) == Some(&Type::Ref) => !classes
                .get(of)
                .is_some_and(|c| *c == Ownership::Trivial || c == origin || c == &classes[v]),
            _ => true,
        }) {
            return Err(format!(
                "borrowed Phi v{}: incompatible entry after propagation",
                v.0
            ));
        }
    }
    let mut order = Vec::new();
    let scalars: HashSet<_> = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .filter_map(|(v, i, ty)| matches!(i, Instruction::Phi { .. }).then_some((*v, *ty)))
        .filter(|(v, ty)| {
            // Unboxed SIMD vectors too: they do not keep a handle.
            matches!(ty, Type::I1 | Type::I8 | Type::I64 | Type::F64) || ty.is_vector()
                || (*ty == Type::Ref && classes.get(v) == Some(&Ownership::Trivial))
        })
        .map(|(v, _)| v)
        .collect();
    let mut scalar_parents: HashMap<ValueId, Vec<ValueId>> = HashMap::new();
    let mut scalar_queue = VecDeque::new();
    for (v, i, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
        if let Instruction::Phi { ty: declared, .. } = i {
            if ty != declared || plan.instructions.contains_key(v) {
                return Err(format!("Phi v{}: incompatible type or plan", v.0));
            }
            if scalars.contains(v) {
                let Instruction::Phi { incoming, .. } = i else {
                    unreachable!()
                };
                if incoming.is_empty() || classes.get(v).is_some_and(|c| *c != Ownership::Trivial) {
                    return Err(format!(
                        "Phi v{}: incompatible class or scalar entries",
                        v.0
                    ));
                }
                let mut anchor = false;
                for (_, op) in incoming {
                    match op {
                        Operand::Constant(Constant::Null) if *ty == Type::Ref => anchor = true,
                        Operand::Constant(Constant::Bool(_)) if *ty == Type::I1 => anchor = true,
                        Operand::Constant(Constant::Int(_)) if matches!(ty, Type::I64 | Type::I8) => anchor = true,
                        Operand::Constant(Constant::Double(_)) if *ty == Type::F64 => anchor = true,
                        Operand::Val(of) if types.get(of) == Some(ty) && scalars.contains(of) => {
                            scalar_parents.entry(*of).or_default().push(*v);
                        }
                        Operand::Val(of)
                            if types.get(of) == Some(ty)
                                && classes.get(of) == Some(&Ownership::Trivial) =>
                        {
                            anchor = true
                        }
                        _ => {
                            return Err(format!(
                                "Phi v{}: entry without a compatible scalar contract",
                                v.0
                            ));
                        }
                    }
                }
                if anchor {
                    scalar_queue.push_back(*v);
                }
            }
            if *ty == Type::Ref && classes.get(v).is_none_or(|c| *c == Ownership::Owned) {
                candidates.insert(*v);
                order.push(*v);
            }
        }
    }
    // The physical width does not prove the absence of ownership. We only propagate contracts
    // from audited entries, rejecting cycles without an external origin.
    let mut scalar_founded = HashSet::new();
    while let Some(v) = scalar_queue.pop_front() {
        if scalar_founded.insert(v) {
            if let Some(dependents) = scalar_parents.get(&v) {
                scalar_queue.extend(dependents);
            }
        }
    }
    for v in scalars {
        if !scalar_founded.contains(&v) {
            return Err(format!("Phi v{} without a scalar origin outside the cycle", v.0));
        }
        classes.insert(v, Ownership::Trivial);
    }
    let mut parents: HashMap<ValueId, Vec<ValueId>> = HashMap::new();
    let mut queue = VecDeque::new();
    for v in &order {
        let Instruction::Phi { incoming, .. } = defs[v] else {
            unreachable!()
        };
        if incoming.is_empty() {
            return Err(format!("Phi v{} without entries", v.0));
        }
        let mut anchor = false;
        for (_, op) in incoming {
            let mut op = op;
            let mut moves = HashSet::new();
            loop {
                match op {
                    Operand::Constant(Constant::Null) => {
                        anchor = true;
                        break;
                    }
                    Operand::Val(of) if types.get(of) != Some(&Type::Ref) => {
                        return Err(format!(
                            "Phi v{}: entry v{} has no Ref representation",
                            v.0, of.0
                        ));
                    }
                    Operand::Val(of) if candidates.contains(of) => {
                        parents.entry(*of).or_default().push(*v);
                        break;
                    }
                    Operand::Val(of) if classes.get(of) == Some(&Ownership::Owned) => {
                        if let Some(Instruction::ArcMove { value }) = defs.get(of) {
                            if !moves.insert(*of) {
                                return Err(format!("Phi v{}: cycle of moves without an origin", v.0));
                            }
                            op = value;
                        } else {
                            anchor = true;
                            break;
                        }
                    }
                    _ => {
                        return Err(format!(
                            "Phi v{} in {}: entry is not owned/null; copy the borrow on the edge",
                            v.0, f.symbol
                        ));
                    }
                }
            }
        }
        if anchor {
            queue.push_back(*v);
        }
    }
    let mut founded = HashSet::new();
    while let Some(v) = queue.pop_front() {
        if founded.insert(v) {
            if let Some(dependents) = parents.get(&v) {
                queue.extend(dependents);
            }
        }
    }
    for v in order {
        if !founded.contains(&v) {
            return Err(format!("Phi v{} without an owned/null origin outside the cycle", v.0));
        }
        classes.insert(v, Ownership::Owned);
    }
    Ok(())
}

// The LLVM helper keeps the ID and converts u8 into I1; the slow path has
// exactly the contract of the audited ABI, including TypeError/reentry.
pub(super) fn unbox_bool_call(inst: &Instruction) -> Option<Instruction> {
    let Instruction::Unbox { op, to: Type::I1 } = inst else {
        return None;
    };
    Some(Instruction::CallRuntime {
        name: "dartforge_unbox_bool".into(),
        args: vec![(op.clone(), Type::Ref)],
        ret_ty: Type::I8,
    })
}

fn produce(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
    include_arc: bool,
) -> Result<HashMap<ValueId, RuntimeCallContract>, String> {
    let mut contracts = HashMap::new();
    let mut fixed_set = HashMap::new();
    let mut pure_effects = HashMap::new();
    // Includes definitions of all the blocks: the physical order is not dominance.
    // Availability per path remains up to the SSA verifier.
    let types: HashMap<_, _> = f
        .params
        .iter()
        .map(|(v, _, ty)| (*v, *ty))
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .map(|(v, _, ty)| (*v, *ty)),
        )
        .collect();
    let mut ids = std::collections::HashSet::new();
    for (v, _, _) in &f.params {
        if !ids.insert(*v) {
            return Err(format!("repeated v{}", v.0));
        }
    }
    for (v, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
        if !ids.insert(*v) {
            return Err(format!("repeated v{}", v.0));
        }
        if include_arc {
            if let Instruction::AllocList { elements } = inst {
                if *ty != Type::Ref { return Err(format!("v{}: list literal requires a Ref result", v.0)); }
                for (op, _) in elements {
                    // The tag hints do not certify representation. The emission
                    // derives the tags from the real types and builds local buffers.
                    let t = match op {
                        Operand::Val(arg) => types.get(arg).copied(),
                        Operand::Constant(Constant::Int(_)) => Some(Type::I64),
                        Operand::Constant(Constant::Double(_)) => Some(Type::F64),
                        Operand::Constant(Constant::Bool(_)) => Some(Type::I1),
                        Operand::Constant(Constant::Null | Constant::String(_) | Constant::StringWtf8(_)) => Some(Type::Ref),
                        Operand::Constant(Constant::Function(_)) => None,
                    };
                    if !matches!(t, Some(Type::Ref | Type::I64 | Type::F64 | Type::I1)) {
                        return Err(format!("v{}: list element without an audited Dart representation", v.0));
                    }
                }
                let c = runtime_call_contract(&Instruction::CallRuntime {
                    name: "dartforge_arc_list_new_owned_v1".into(),
                    args: vec![(Operand::Val(ValueId(0)), Type::Ptr),
                        (Operand::Constant(Constant::Int(elements.len() as i64)), Type::I64),
                        (Operand::Constant(Constant::Int(10)), Type::I64),
                        (Operand::Constant(Constant::Int(0)), Type::I64)],
                    ret_ty: Type::Ref,
                })?;
                if classes.get(v).is_some_and(|class| *class != c.result)
                    || plan.instructions.get(v).is_some_and(|e| *e != c.effect) {
                    return Err(format!("v{}: conflicting list literal contract", v.0));
                }
                contracts.insert(*v, c);
            }
            if let Instruction::AllocRecord { elements } = inst {
                // Under ARC every record literal goes to the Owned factory, which only
                // receives references (the lowering boxes the scalars).
                let reference = |op: &Operand| match op {
                    Operand::Val(arg) => types.get(arg) == Some(&Type::Ref),
                    Operand::Constant(Constant::Null) => true,
                    _ => false,
                };
                if *ty != Type::Ref || !elements.iter().all(|(op, _)| reference(op)) {
                    return Err(format!("v{}: record literal requires Ref fields in SSA", v.0));
                }
                let c = runtime_call_contract(&Instruction::CallRuntime {
                    name: "dartforge_arc_record_from_refs_owned_v1".into(),
                    args: vec![(Operand::Val(ValueId(0)), Type::Ptr),
                        (Operand::Constant(Constant::Int(elements.len() as i64)), Type::I64)],
                    ret_ty: Type::Ref,
                })?;
                if classes.get(v).is_some_and(|class| *class != c.result)
                    || plan.instructions.get(v).is_some_and(|e| *e != c.effect) {
                    return Err(format!("v{}: conflicting record literal contract", v.0));
                }
                contracts.insert(*v, c);
            }
            if let Instruction::ConcatStrings { parts } = inst {
                // The emission derives the kind of each pair from the real type: a Ref
                // text or an unboxed `int`. Texts arrive through `Const` (the emitter
                // does not materialize a string in an operand); another shape has no ABI.
                let representable = |op: &Operand| match op {
                    Operand::Val(arg) => matches!(types.get(arg), Some(Type::Ref | Type::I64)),
                    Operand::Constant(Constant::Int(_)) => true,
                    _ => false,
                };
                if *ty != Type::Ref || !parts.iter().all(representable) {
                    return Err(format!("v{}: interpolation without audited text/int parts", v.0));
                }
                let c = runtime_call_contract(&Instruction::CallRuntime {
                    name: "dartforge_arc_string_join_owned_v1".into(),
                    args: vec![(Operand::Val(ValueId(0)), Type::Ptr),
                        (Operand::Constant(Constant::Int(parts.len() as i64)), Type::I64)],
                    ret_ty: Type::Ref,
                })?;
                if classes.get(v).is_some_and(|class| *class != c.result)
                    || plan.instructions.get(v).is_some_and(|e| *e != c.effect) {
                    return Err(format!("v{}: conflicting interpolation contract", v.0));
                }
                contracts.insert(*v, c);
            }
            if let Instruction::Box { op, from } = inst && from.is_vector() {
                if *ty != Type::Ref || !matches!(op, Operand::Val(v) if types.get(v) == Some(from)) {
                    return Err(format!("v{}: SIMD box requires a vector of the same type", v.0));
                }
                // The emission splits the bits into native words; the factory
                // has the same effects regardless of the hints.
                let c = runtime_call_contract(&Instruction::CallRuntime {
                    name: "dartforge_arc_box_simd_owned_v1".into(),
                    args: vec![(Operand::Constant(Constant::Int(0)), Type::I64); 3],
                    ret_ty: Type::Ref,
                })?;
                if classes.get(v).is_some_and(|class| *class != c.result)
                    || plan.instructions.get(v).is_some_and(|e| *e != c.effect) {
                    return Err(format!("v{}: conflicting SIMD box contract", v.0));
                }
                contracts.insert(*v, c);
            }
            if super::pure_set::check(*v, inst, ty, &types)? {
                let effect = TokenEffect::default();
                if classes.get(v).is_some_and(|c| *c != Ownership::Trivial)
                    || plan.instructions.get(v).is_some_and(|e| *e != effect)
                {
                    return Err(format!(
                        "v{}: contract incompatible with constant/pure operation",
                        v.0
                    ));
                }
                fixed_set.insert(*v, Ownership::Trivial);
                pure_effects.insert(*v, effect);
            }
            let fixed = match inst {
                Instruction::ArcCopy { .. }
                | Instruction::ArcMove { .. }
                | Instruction::ArcLoadStrong { .. } => Some((Ownership::Owned, Type::Ref)),
                Instruction::ArcDrop { .. } | Instruction::ArcStoreStrong { .. } => {
                    Some((Ownership::Trivial, Type::Void))
                }
                _ => None,
            };
            if let Some((class, expected)) = fixed {
                if *ty != expected
                    || classes.get(v).is_some_and(|c| *c != class)
                    || plan.instructions.contains_key(v)
                {
                    return Err(format!("v{}: contract incompatible with ARC operation", v.0));
                }
                fixed_set.insert(*v, class);
            }
        }
        if include_arc && (super::typed_set::check(f, inst, *ty)?
            || super::selectors::check_local_vector(f, inst, *ty, None)?) {
            let c = RuntimeCallContract {
                effect: TokenEffect { may_fail: true, ..Default::default() },
                result: if *ty == Type::Ref { Ownership::Owned } else { Ownership::Trivial }, persistent_retention: true,
                invalidates_borrows: true, calls_dart: true,
            };
            if classes.get(v).is_some_and(|class| *class != c.result)
                || plan.instructions.get(v).is_some_and(|e| *e != c.effect) {
                return Err(format!("v{}: conflicting contract of the ARC typed call", v.0));
            }
            contracts.insert(*v, c);
        }
        // The C call with a compound by value: the compounds go and come back by
        // address (the destination), a scalar like the other arguments.
        if include_arc && let Instruction::CompositeNativeCall { target, args, ret, dest, .. } = inst {
            let scalar = |op: &Operand| match op {
                Operand::Val(arg) => types.get(arg).is_some_and(|t| *t != Type::Ref && *t != Type::Void),
                Operand::Constant(Constant::Int(_) | Constant::Double(_) | Constant::Bool(_)) => true,
                _ => false,
            };
            let expected = match ret {
                crate::hir::NativeType::Prim(tc) => tc.hir_type(),
                _ => Type::Void,
            };
            if *ty != expected || !scalar(target) || !args.iter().all(|(op, _)| scalar(op))
                || dest.as_ref().is_some_and(|d| !scalar(d)) {
                return Err(format!("v{}: compound native call without an audited scalar ABI", v.0));
            }
            let c = RuntimeCallContract {
                effect: TokenEffect::default(), result: Ownership::Trivial,
                persistent_retention: false, invalidates_borrows: true, calls_dart: true,
            };
            if classes.get(v).is_some_and(|class| *class != c.result)
                || plan.instructions.get(v).is_some_and(|e| *e != c.effect) {
                return Err(format!("v{}: conflicting contract of compound native call", v.0));
            }
            contracts.insert(*v, c);
        }
        if include_arc && let Instruction::NativeCall { target, args, ret } = inst {
            // Only scalars cross the C ABI (the HIR converts Pointer into an
            // address). A Dart callback may reenter, but the C exit clears
            // the pending flag (`dartforge_ffi_callback_exit`): nothing stays pending.
            let scalar = |op: &Operand| match op {
                Operand::Val(arg) => types.get(arg).is_some_and(|t| *t != Type::Ref && *t != Type::Void),
                Operand::Constant(Constant::Int(_) | Constant::Double(_) | Constant::Bool(_)) => true,
                _ => false,
            };
            if *ty != ret.hir_type() || *ty == Type::Ref || !scalar(target)
                || !args.iter().all(|(op, _)| scalar(op)) {
                return Err(format!("v{}: native call without an audited scalar ABI", v.0));
            }
            let c = RuntimeCallContract {
                effect: TokenEffect::default(), result: Ownership::Trivial,
                persistent_retention: false, invalidates_borrows: true, calls_dart: true,
            };
            if classes.get(v).is_some_and(|class| *class != c.result)
                || plan.instructions.get(v).is_some_and(|e| *e != c.effect) {
                return Err(format!("v{}: conflicting native call contract", v.0));
            }
            contracts.insert(*v, c);
        }
        if include_arc && matches!(inst, Instruction::CallSelectorForward { arc_owned: true, .. } | Instruction::CallClosureForward { arc_owned: true, .. }) {
            let (recv, args, desc) = match inst {
                Instruction::CallSelectorForward { recv, args, desc, .. }
                | Instruction::CallClosureForward { closure: recv, args, desc, .. } => (recv, args, desc),
                _ => unreachable!(),
            };
            let compatible = |op: &Operand, t| match op {
                Operand::Val(v) => types.get(v) == Some(&t),
                Operand::Constant(Constant::Null) => t == Type::Ref,
                _ => false,
            };
            if *ty != Type::Ref || !compatible(recv, Type::Ref)
                || !compatible(args, Type::Ptr) || !compatible(desc, Type::Ptr) {
                return Err(format!("v{}: incompatible ARC forwarding ABI", v.0));
            }
            let c = RuntimeCallContract {
                effect: TokenEffect { may_fail: true, ..Default::default() },
                result: Ownership::Owned, persistent_retention: true,
                invalidates_borrows: true, calls_dart: true,
            };
            if classes.get(v).is_some_and(|class| *class != c.result)
                || plan.instructions.get(v).is_some_and(|e| *e != c.effect) {
                return Err(format!("v{}: conflicting ARC forwarding contract", v.0));
            }
            contracts.insert(*v, c);
        }
        let unbox_bool = if include_arc {
            unbox_bool_call(inst)
        } else {
            None
        };
        let call_inst = unbox_bool.as_ref().unwrap_or(inst);
        if let Instruction::CallRuntime { ret_ty, args, .. } = call_inst {
            if if unbox_bool.is_some() {
                *ty != Type::I1
            } else {
                ty != ret_ty
            } {
                return Err(format!("v{}: incompatible result type", v.0));
            }
            super::fields::check_call(f, call_inst, None)?;
            let c = runtime_call_contract(call_inst).map_err(|e| format!("v{}: {e}", v.0))?;
            for (n, (op, ty)) in args.iter().enumerate() {
                if let Operand::Val(arg) = op
                    && types.get(arg) != Some(ty)
                {
                    return Err(format!(
                        "v{}: argument {n} points to an absent or incompatibly typed SSA v{}",
                        v.0, arg.0
                    ));
                }
            }
            if classes.get(v).is_some_and(|class| *class != c.result)
                || plan.instructions.get(v).is_some_and(|e| *e != c.effect)
            {
                return Err(format!(
                    "v{}: metadata conflicts with runtime contract",
                    v.0
                ));
            }
            contracts.insert(*v, c);
        }
    }
    let mut pendings = HashMap::new();
    for b in &f.blocks {
        if let Terminator::CondBranch { then_block, .. } = &b.terminator
            && let Some((call, _, _)) = b
                .instructions
                .len()
                .checked_sub(3)
                .map(|i| &b.instructions[i])
            && contracts.get(call).is_some_and(|c| c.effect.may_fail)
            && super::classification::check_pending(b, *call, *then_block).is_some()
        {
            if plan.pendings.get(call).is_some_and(|p| p != then_block) {
                return Err(format!("v{}: pending exit conflicts with the CFG", call.0));
            }
            pendings.insert(*call, *then_block);
        }
    }
    if include_arc {
        // The Phis do not have a class here yet: `check_typed_scalars`
        // checks them after `produce_phi`.
        let phis: HashSet<ValueId> = f.blocks.iter().flat_map(|b| &b.instructions)
            .filter(|(_, i, _)| matches!(i, Instruction::Phi { .. })).map(|(v, _, _)| *v).collect();
        for (_, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if let Instruction::TypedCall { args, .. } = inst {
                for (op, t) in args {
                    if *t != Type::Ref && let Operand::Val(v) = op && !phis.contains(v) {
                        let class = contracts.get(v).map(|c| &c.result)
                            .or_else(|| fixed_set.get(v)).or_else(|| classes.get(v));
                        if class != Some(&Ownership::Trivial) {
                            return Err(format!("v{}: typed scalar argument requires Trivial, without a Dart word", v.0));
                        }
                    }
                }
            }
        }
    }
    classes.extend(fixed_set);
    plan.pendings.extend(pendings);
    plan.instructions.extend(pure_effects);
    for (v, c) in &contracts {
        classes.insert(*v, c.result);
        plan.instructions.insert(*v, c.effect.clone());
    }
    Ok(contracts)
}

/// Semantic contract of an audited ordinary call.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::runtime_call_contract};
/// let i = Instruction::CallRuntime { name: "dartforge_arc_collect".into(), args: vec![], ret_ty: Type::Void };
/// let c = runtime_call_contract(&i)?;
/// assert!(c.invalidates_borrows && c.effect.always.is_empty());
/// # Ok::<(), String>(())
/// ```
#[derive(Debug)]
pub struct RuntimeCallContract {
    /// Consumptions of SSA arguments, preserving multiplicity.
    pub effect: TokenEffect,
    /// Class of the result, coming from the extern and not from the i64 width.
    pub result: Ownership,
    /// Requires considering persistent owners internal to the runtime.
    pub persistent_retention: bool,
    /// Requires a separate proof of the borrowed dependencies after the call.
    pub invalidates_borrows: bool,
    /// Audited Dart reentry, including the construction of TypeError by the SDK.
    pub calls_dart: bool,
}

/// Translates an audited call into consumption effects and the class of the result.
///
/// Checks arity, declared types and constants: I64 requires an integer, or a
/// function address when the parameter is native. The HIR verifier remains
/// responsible for SSA, dominance and the real types of the operands. Does not insert RC,
/// does not certify owners of slots and does not resolve borrow invalidation.
/// Null has no physical token; other references must be evaluated in SSA.
///
/// # Errors
/// A non-runtime instruction, an absent extern, an incompatible signature, an
/// unevaluated reference or a direct retain, which creates a token without an explicit SSA result.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::{runtime_call_contract, Ownership}};
/// let i = Instruction::CallRuntime { name: "dartforge_arc_frame_load_v1".into(),
///     args: vec![(Operand::Val(ValueId(0)), Type::I64), (Operand::Constant(Constant::Int(0)), Type::I64)],
///     ret_ty: Type::Ref };
/// assert_eq!(runtime_call_contract(&i)?.result, Ownership::Owned);
/// # Ok::<(), String>(())
/// ```
pub fn runtime_call_contract(inst: &Instruction) -> Result<RuntimeCallContract, String> {
    let Instruction::CallRuntime { name, args, ret_ty } = inst else {
        return Err("runtime contract requires CallRuntime".into());
    };
    if let Some(base) = name.strip_prefix(super::NATIVE_SCOPE_PREFIX) {
        return runtime_call_contract(&Instruction::CallRuntime {
            name: base.to_string(),
            args: args.clone(),
            ret_ty: *ret_ty,
        });
    }
    if let Some(base) = name.strip_prefix(super::GENERIC_OWNED_PREFIX) {
        if *ret_ty != Type::Ref {
            return Err(format!("{name}: Owned thunk requires a Ref result"));
        }
        return generic_owned_contract(base, args);
    }
    let c = contract(name).map_err(|e| format!("{name}: {e}"))?;
    // Retain produces an independent owner, but the extern returns void. The HIR
    // represents that production with ArcCopy; treating it as a common borrowed
    // call would allow losing the token without the verifier noticing.
    if name == "dartforge_arc_retain" {
        return Err("direct retain requires ArcCopy with an owned SSA result".into());
    }
    if c.parameters.len() != args.len() {
        return Err(format!("{name}: arity incompatible with ownership.tsv"));
    }
    let (ty, result) = match c.result {
        ResultMode::Owned => (Type::Ref, Ownership::Owned),
        ResultMode::BorrowArg(n) => (
            Type::Ref,
            match args[n].0 {
                Operand::Val(v) => Ownership::Borrowed {
                    owner: OwnerOrigin::Value(v),
                    scope: 0,
                },
                Operand::Constant(Constant::Null) => Ownership::Trivial,
                _ => {
                    return Err(format!(
                        "{name}: owner of the borrowed result requires SSA or null"
                    ));
                }
            },
        ),
        ResultMode::ScalarI64 => (Type::I64, Ownership::Trivial),
        ResultMode::ScalarF64 => (Type::F64, Ownership::Trivial),
        ResultMode::ScalarI8 => (Type::I8, Ownership::Trivial),
        ResultMode::ScalarI1 => (Type::I1, Ownership::Trivial),
        ResultMode::Void => (Type::Void, Ownership::Trivial),
    };
    if *ret_ty != ty {
        return Err(format!(
            "{name}: result incompatible with the semantic contract"
        ));
    }
    let mut effect = TokenEffect {
        may_fail: c.may_fail,
        ..Default::default()
    };
    for (n, (mode, (op, ty))) in c.parameters.iter().zip(args).enumerate() {
        if *mode == ParamMode::StaticTable {
            // The LLVM selection uses only the registered thunk of the module. The
            // generic producer does not have that provenance: an arbitrary Ptr or function
            // address does not prove the absence of Dart/GC/exceptions.
            return Err(format!("{name}: static table requires provenance from the emitter"));
        }
        let reference = matches!(
            mode,
            ParamMode::Borrow
                | ParamMode::Consume
                | ParamMode::ConsumeSuccess
                | ParamMode::ConsumeError
        );
        let expected = if reference {
            Type::Ref
        } else if matches!(mode, ParamMode::StaticVector | ParamMode::NativeAddress) {
            Type::Ptr
        } else if *mode == ParamMode::ScalarF64 {
            Type::F64
        } else if *mode == ParamMode::ScalarI8 {
            Type::I8
        } else {
            Type::I64
        };
        if *ty != expected || (matches!(mode, ParamMode::StaticVector | ParamMode::NativeAddress) && !matches!(op, Operand::Val(_))) {
            return Err(format!("{name}: type of argument {n} incompatible"));
        }
        if reference {
            match op {
                Operand::Val(v) => {
                    if *mode == ParamMode::Consume {
                        effect.always.push(*v);
                    }
                    if *mode == ParamMode::ConsumeSuccess {
                        effect.success.push(*v);
                    }
                    if *mode == ParamMode::ConsumeError {
                        effect.error.push(*v);
                    }
                }
                Operand::Constant(Constant::Null) => {}
                _ => return Err(format!("{name}: Ref argument {n} requires SSA or null")),
            }
        } else if let Operand::Constant(k) = op {
            // A function address only has meaning in a native parameter;
            // the I64 annotation does not turn bool/double/literal into an integer.
            let compatible = if *mode == ParamMode::ScalarF64 {
                matches!(k, Constant::Double(_))
            } else if *mode == ParamMode::ScalarI8 {
                matches!(k, Constant::Int(0..=255) | Constant::Bool(_))
            } else {
                matches!(k, Constant::Int(_))
                    || (*mode == ParamMode::Native && matches!(k, Constant::Function(_)))
            };
            if !compatible {
                return Err(format!(
                    "{name}: constant of argument {n} incompatible with its contract"
                ));
            }
        }
    }
    Ok(RuntimeCallContract {
        effect,
        result,
        persistent_retention: c.persistent_retention,
        invalidates_borrows: c.invalidates_borrows,
        calls_dart: c.calls_dart,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers_agree_with_effects() {
        let mut names = HashSet::new();
        for c in HELPER_CONTRACTS {
            assert!(names.insert(c.name), "repeated {}", c.name);
            assert!(c.name.starts_with("df."), "{}", c.name);
            assert!(dartforge_runtime::ownership::contract(c.name).is_err(), "{} also in the runtime", c.name);
            // The contract follows the marks of the helper: fails if it throws, invalidates
            // borrows if it allocates, never retains.
            let e = crate::llvm::externs::effects_of(c.name);
            assert_eq!((c.may_fail, c.calls_dart), (e.throws || e.calls_dart, e.calls_dart), "{}: {e:?}", c.name);
            assert!(!e.allocates || c.invalidates_borrows, "{}: {e:?}", c.name);
            assert!(!c.persistent_retention, "{}", c.name);
            assert!(c.parameters.iter().all(|m| matches!(m, ParamMode::Borrow | ParamMode::Scalar)));
        }
        // The Ref read from the storage and whatever allocates stay without a contract.
        for name_str in ["df.word_ref", "df.string_allocate"] {
            assert!(contract(name_str).is_err(), "{name_str}");
        }
        let rt = |name: &str, args, ret_ty| Instruction::CallRuntime { name: name.into(), args, ret_ty };
        let text = rt("df.string_equals_object", vec![(Operand::Val(ValueId(1)), Type::Ref), (Operand::Constant(Constant::Null), Type::Ref)], Type::I1);
        let c = runtime_call_contract(&text).unwrap();
        assert_eq!(c.result, Ownership::Trivial);
        assert!(!c.effect.may_fail && !c.invalidates_borrows);
        let base = rt("df.typed_base", vec![(Operand::Val(ValueId(1)), Type::Ref)], Type::Ref);
        assert_eq!(runtime_call_contract(&base).unwrap().result,
            Ownership::Borrowed { owner: OwnerOrigin::Value(ValueId(1)), scope: 0 });
        // A wrong width of the result is rejected.
        let wrong = rt("df.string_len", vec![(Operand::Val(ValueId(1)), Type::Ref)], Type::I8);
        assert!(runtime_call_contract(&wrong).is_err());
    }

    #[test]
    fn unbox_borrows_reference_and_returns_fallible_scalar_without_consumption() {
        for (name, ret_ty) in [
            ("dartforge_unbox_int", Type::I64),
            ("dartforge_unbox_double", Type::F64),
            ("dartforge_unbox_bool", Type::I8),
        ] {
            let mut inst = Instruction::CallRuntime {
                name: name.into(),
                args: vec![(Operand::Val(ValueId(0)), Type::Ref)],
                ret_ty,
            };
            let c = runtime_call_contract(&inst).unwrap();
            assert_eq!(c.result, Ownership::Trivial);
            assert!(c.effect.may_fail && c.invalidates_borrows && c.calls_dart);
            assert!(!c.persistent_retention);
            assert!(
                c.effect.always.is_empty()
                    && c.effect.success.is_empty()
                    && c.effect.error.is_empty()
            );
            if let Instruction::CallRuntime { ret_ty: r, .. } = &mut inst {
                *r = Type::Ref;
            }
            assert!(runtime_call_contract(&inst).is_err());
            if let Instruction::CallRuntime {
                args, ret_ty: r, ..
            } = &mut inst
            {
                *r = ret_ty;
                args[0].1 = Type::I64;
            }
            assert!(runtime_call_contract(&inst).is_err());
        }
    }

    #[test]
    fn throwing_exit_releases_result_and_guard_requires_cfg_without_publishing() {
        let original = Function {
            symbol: "desenrolar".into(),
            name: "desenrolar".into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(1),
                    Instruction::ArcCopy {
                        value: Operand::Val(ValueId(0)),
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
            }],
        };
        let mut f = original.clone();
        let mut classes = HashMap::new();
        let mut plan = TokenPlan {
            ret: super::super::TokenReturn::Owned,
            ..Default::default()
        };
        let mut tables = FunctionTables::default();
        tables.exits.insert(BlockId(0), ExceptionExit::Throws);
        assert_eq!(
            insert_arc_dart_exits(
                &mut f,
                &mut classes,
                &mut plan,
                &tables,
                &ScopePlan::default()
            )
            .unwrap(),
            (0, 1)
        );
        assert!(matches!(
            f.blocks[0].terminator,
            Terminator::Return(Some(Operand::Constant(Constant::Null)))
        ));
        assert!(matches!(
            f.blocks[0].instructions[1].1,
            Instruction::ArcDrop {
                value: Operand::Val(ValueId(1))
            }
        ));
        let mut module = Module::new();
        module.arc_memory = true;
        module.table_exceptions = true;
        module.functions.push(f.clone());
        module.tables.push(tables.clone());
        let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
        let function = ir
            .split("@desenrolar(")
            .nth(1)
            .unwrap()
            .split("\n}")
            .next()
            .unwrap();
        let release = function
            .find("call void @df.arc_release_ref(i64 %v1)")
            .unwrap();
        // The first throw may be the prologue guard, before creating
        // any token; the last one is the Throw exit of the prepared body.
        let unwind = function.rfind("call void @df.throw()").unwrap();
        assert!(release < unwind);
        assert_eq!(
            insert_arc_dart_exits(
                &mut f,
                &mut classes,
                &mut plan,
                &tables,
                &ScopePlan::default()
            )
            .unwrap(),
            (0, 0)
        );
        f = original;
        classes.clear();
        plan.instructions.clear();
        assert!(
            produce_and_verify_dart_tokens(
                &f,
                &mut classes,
                &mut plan,
                &tables,
                &ScopePlan::default()
            )
            .unwrap_err()
            .contains("canonical")
        );
        assert!(classes.is_empty() && plan.instructions.is_empty());
        tables.exits.insert(BlockId(0), ExceptionExit::Guard);
        let before = format!("{f:?}");
        assert!(
            insert_arc_dart_exits(
                &mut f,
                &mut classes,
                &mut plan,
                &tables,
                &ScopePlan::default()
            )
            .unwrap_err()
            .contains("explicit CFG")
        );
        assert_eq!(format!("{f:?}"), before);
        assert!(classes.is_empty() && plan.instructions.is_empty());
    }

    #[test]
    fn cleanup_preserves_transfer_and_retains_before_releasing_other_tokens() {
        let mut f = Function {
            symbol: "retornos_distintos".into(),
            name: "retornos_distintos".into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (
                            ValueId(1),
                            Instruction::ArcCopy {
                                value: Operand::Val(ValueId(0)),
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(2),
                            Instruction::ArcCopy {
                                value: Operand::Val(ValueId(0)),
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(3),
                            Instruction::Const(Constant::Bool(true)),
                            Type::I1,
                        ),
                    ],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(3)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
                },
            ],
        };
        let mut classes = HashMap::new();
        let mut plan = TokenPlan {
            ret: super::super::TokenReturn::Owned,
            ..Default::default()
        };
        assert_eq!(
            insert_arc_dart_exits(
                &mut f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .unwrap(),
            (1, 3)
        );
        // Transferring v1 requires releasing only v2 at this exit.
        assert!(matches!(
            f.blocks[1].terminator,
            Terminator::Return(Some(Operand::Val(ValueId(1))))
        ));
        assert_eq!(f.blocks[1].instructions.len(), 1);
        assert!(matches!(
            f.blocks[1].instructions[0].1,
            Instruction::ArcDrop {
                value: Operand::Val(ValueId(2))
            }
        ));
        // At the other exit, retaining the return precedes the cleanup of the two owners.
        let b = &f.blocks[2];
        assert_eq!(b.instructions.len(), 3);
        assert!(matches!(
            b.instructions[0].1,
            Instruction::ArcCopy {
                value: Operand::Val(ValueId(0))
            }
        ));
        assert!(
            matches!(b.terminator, Terminator::Return(Some(Operand::Val(v))) if v == b.instructions[0].0)
        );
        for (inst, expected) in b.instructions[1..].iter().zip([ValueId(1), ValueId(2)]) {
            assert!(
                matches!(inst.1, Instruction::ArcDrop { value: Operand::Val(v) } if v == expected)
            );
        }
        let ready = format!("{f:?}");
        assert_eq!(
            insert_arc_dart_exits(
                &mut f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{f:?}"), ready);
    }

    #[test]
    fn cleanup_exits_cover_runtime_error_without_repairing_double_consumption() {
        let original = Function {
            symbol: "cleanup_pending".into(),
            name: "cleanup_pending".into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Void,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (
                            ValueId(1),
                            Instruction::ArcCopy {
                                value: Operand::Val(ValueId(0)),
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(2),
                            Instruction::CallRuntime {
                                name: "dartforge_gc_collect".into(),
                                args: vec![],
                                ret_ty: Type::Void,
                            },
                            Type::Void,
                        ),
                        (
                            ValueId(3),
                            Instruction::CallRuntime {
                                name: "dartforge_exception_pending".into(),
                                args: vec![],
                                ret_ty: Type::I8,
                            },
                            Type::I8,
                        ),
                        (
                            ValueId(4),
                            Instruction::ICmp(
                                ICmpOp::Ne,
                                Operand::Val(ValueId(3)),
                                Operand::Constant(Constant::Int(0)),
                            ),
                            Type::I1,
                        ),
                    ],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(4)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                },
            ],
        };
        let mut f = original.clone();
        let mut classes = HashMap::new();
        let mut plan = TokenPlan::default();
        assert_eq!(
            insert_arc_dart_exits(
                &mut f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .unwrap(),
            (0, 2)
        );
        for b in &f.blocks[1..] {
            assert!(matches!(
                b.instructions[0].1,
                Instruction::ArcDrop {
                    value: Operand::Val(ValueId(1))
                }
            ));
        }
        let ready = format!("{f:?}");
        assert_eq!(
            insert_arc_dart_exits(
                &mut f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{f:?}"), ready);
        f = original;
        for id in [99, 100] {
            f.blocks[0].instructions.insert(
                1,
                (
                    ValueId(id),
                    Instruction::ArcDrop {
                        value: Operand::Val(ValueId(1)),
                    },
                    Type::Void,
                ),
            );
        }
        classes.clear();
        plan = TokenPlan::default();
        let before = format!("{f:?}");
        assert!(
            insert_arc_dart_exits(
                &mut f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .unwrap_err()
            .contains("unavailable")
        );
        assert_eq!(format!("{f:?}"), before);
        assert!(classes.is_empty() && plan.instructions.is_empty() && plan.pendings.is_empty());
    }

    #[test]
    fn dart_return_insertion_is_idempotent_and_atomic_if_cleanup_missing() {
        let original = Function {
            symbol: "identidade".into(),
            name: "identidade".into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        };
        let mut f = original.clone();
        let mut classes = HashMap::new();
        let mut plan = TokenPlan {
            ret: super::super::TokenReturn::Owned,
            ..Default::default()
        };
        assert_eq!(
            insert_dart_return_retention(
                &mut f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .unwrap(),
            1
        );
        assert!(matches!(
            f.blocks[0].instructions[0].1,
            Instruction::ArcCopy {
                value: Operand::Val(ValueId(0))
            }
        ));
        let ready = format!("{f:?}");
        assert_eq!(
            insert_dart_return_retention(
                &mut f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .unwrap(),
            0
        );
        assert_eq!(format!("{f:?}"), ready);
        f = original.clone();
        f.blocks[0].instructions.push((
            ValueId(5),
            Instruction::ArcCopy {
                value: Operand::Val(ValueId(0)),
            },
            Type::Ref,
        ));
        classes.clear();
        plan = TokenPlan {
            ret: super::super::TokenReturn::Owned,
            ..Default::default()
        };
        let before = format!("{f:?}");
        assert!(
            insert_dart_return_retention(
                &mut f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .unwrap_err()
            .contains("not consumed")
        );
        assert_eq!(format!("{f:?}"), before);
        assert!(classes.is_empty());
        assert!(plan.instructions.is_empty());
        for constant in [
            Constant::Null,
            Constant::String("retorno permanente".into()),
        ] {
            f = original.clone();
            f.blocks[0].terminator = Terminator::Return(Some(Operand::Constant(constant.clone())));
            classes.clear();
            plan = TokenPlan {
                ret: super::super::TokenReturn::Owned,
                ..Default::default()
            };
            let quantity = insert_dart_return_retention(
                &mut f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default(),
            )
            .unwrap();
            assert_eq!(
                quantity,
                usize::from(!matches!(constant, Constant::Null))
            );
        }
    }

    #[test]
    fn dart_wrapper_does_not_publish_parameters_when_transfer_fails() {
        let mut f = Function {
            symbol: "identidade".into(),
            name: "identidade".into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(1),
                    Instruction::ArcMove {
                        value: Operand::Val(ValueId(0)),
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
            }],
        };
        let mut classes = HashMap::new();
        let mut plan = TokenPlan {
            ret: super::super::TokenReturn::Owned,
            ..Default::default()
        };
        assert!(
            produce_and_verify_dart_tokens(
                &f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .is_err()
        );
        assert!(classes.is_empty());
        assert!(plan.instructions.is_empty());
        f.blocks[0].instructions[0].1 = Instruction::ArcCopy {
            value: Operand::Val(ValueId(0)),
        };
        produce_and_verify_dart_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        assert_eq!(
            classes[&ValueId(0)],
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                scope: 0
            }
        );
        assert_eq!(classes[&ValueId(1)], Ownership::Owned);
        let before = classes.clone();
        plan.ret = super::super::TokenReturn::Borrowed;
        assert!(
            produce_and_verify_dart_tokens(
                &f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .is_err()
        );
        assert_eq!(classes, before);
        assert_eq!(plan.ret, super::super::TokenReturn::Borrowed);
    }

    #[test]
    fn dart_parameters_do_not_classify_i64_and_reject_conflict_atomically() {
        let mut f = Function {
            symbol: "parametros".into(),
            name: "parametros".into(),
            debug_info: None,
            params: vec![
                (ValueId(0), "ref".into(), Type::Ref),
                (ValueId(1), "bits".into(), Type::I64),
                (ValueId(2), "outra_ref".into(), Type::Ref),
            ],
            return_ty: Type::Void,
            blocks: vec![],
        };
        let mut classes = HashMap::from([(ValueId(2), Ownership::Owned)]);
        let before = classes.clone();
        assert!(produce_dart_ref_parameters(&f, &mut classes).is_err());
        assert_eq!(classes, before);
        classes.clear();
        produce_dart_ref_parameters(&f, &mut classes).unwrap();
        assert!(!classes.contains_key(&ValueId(1)));
        assert_eq!(classes.len(), 2);
        let before = classes.clone();
        f.params.push((ValueId(0), "duplicado".into(), Type::Ref));
        assert!(produce_dart_ref_parameters(&f, &mut classes).is_err());
        assert_eq!(classes, before);
    }

    #[test]
    fn trivial_ref_phi_producer_respects_explicit_owned_transfer() {
        let mut f = Function {
            symbol: "phi_trivial_auto".into(),
            name: "phi_trivial_auto".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(1)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::Phi {
                            ty: Type::Ref,
                            incoming: vec![
                                (BlockId(0), Operand::Constant(Constant::Null)),
                                (BlockId(1), Operand::Val(ValueId(1))),
                            ],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Constant(Constant::Bool(false)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                },
            ],
        };
        for ret in [
            super::super::TokenReturn::Trivial,
            super::super::TokenReturn::Owned,
        ] {
            let mut classes = HashMap::new();
            let mut plan = TokenPlan {
                ret,
                ..Default::default()
            };
            produce_and_verify_tokens(
                &f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default(),
            )
            .unwrap();
            assert_eq!(
                classes[&ValueId(1)],
                if ret == super::super::TokenReturn::Owned {
                    Ownership::Owned
                } else {
                    Ownership::Trivial
                }
            );
        }
        f.blocks[0].instructions.push((
            ValueId(0),
            Instruction::Const(Constant::String("permanente".into())),
            Type::Ref,
        ));
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
            incoming[0].1 = Operand::Val(ValueId(0));
        }
        let mut classes = HashMap::new();
        let mut plan = TokenPlan::default();
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        assert_eq!(classes[&ValueId(1)], Ownership::Trivial);
    }

    #[test]
    fn producer_checks_pending_dependency_in_borrowed_phi_cycle() {
        let mut f = Function {
            symbol: "ciclo_borrowed".into(),
            name: "ciclo_borrowed".into(),
            debug_info: None,
            params: vec![(ValueId(0), "entrada".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(1)),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![(
                        ValueId(2),
                        Instruction::Phi {
                            ty: Type::Ref,
                            incoming: vec![(BlockId(1), Operand::Val(ValueId(1)))],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Branch(BlockId(1)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::Phi {
                            ty: Type::Ref,
                            incoming: vec![
                                (BlockId(0), Operand::Val(ValueId(0))),
                                (BlockId(2), Operand::Val(ValueId(2))),
                            ],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Constant(Constant::Bool(false)),
                        then_block: BlockId(2),
                        else_block: BlockId(3),
                    },
                },
                BasicBlock {
                    id: BlockId(3),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                },
            ],
        };
        let initial = HashMap::from([(
            ValueId(0),
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                scope: 0,
            },
        )]);
        let mut classes = initial.clone();
        let mut plan = TokenPlan {
            ret: super::super::TokenReturn::Borrowed,
            ..Default::default()
        };
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        for v in [ValueId(1), ValueId(2)] {
            assert_eq!(
                classes[&v],
                Ownership::Borrowed {
                    owner: OwnerOrigin::Value(ValueId(0)),
                    scope: 0
                }
            );
        }
        // The first Phi may provisionally receive the anchor, but its other
        // entry cannot end at a Phi that only has an Owned source.
        f.blocks[0].instructions.push((
            ValueId(5),
            Instruction::ArcCopy {
                value: Operand::Val(ValueId(0)),
            },
            Type::Ref,
        ));
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
            incoming[0].1 = Operand::Val(ValueId(5));
        }
        classes = initial.clone();
        assert!(
            produce_arc_contracts(&f, &mut classes, &mut plan)
                .unwrap_err()
                .contains("after propagation")
        );
        assert_eq!(classes, initial);
        assert!(plan.instructions.is_empty());
    }

    #[test]
    fn producer_preserves_borrow_in_anchored_loop_phi() {
        let mut f = Function {
            symbol: "phi_borrowed_laco".into(),
            name: "phi_borrowed_laco".into(),
            debug_info: None,
            params: vec![(ValueId(0), "entrada".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(1)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::Phi {
                            ty: Type::Ref,
                            incoming: vec![
                                (BlockId(0), Operand::Val(ValueId(0))),
                                (BlockId(1), Operand::Val(ValueId(1))),
                            ],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Constant(Constant::Bool(false)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                },
            ],
        };
        let mut classes = HashMap::from([(
            ValueId(0),
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                scope: 0,
            },
        )]);
        let mut plan = TokenPlan {
            ret: super::super::TokenReturn::Borrowed,
            ..Default::default()
        };
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        assert_eq!(
            classes[&ValueId(1)],
            Ownership::Borrowed {
                owner: OwnerOrigin::Value(ValueId(0)),
                scope: 0
            }
        );
        classes.remove(&ValueId(1));
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
            incoming[0].1 = Operand::Val(ValueId(1));
        }
        let before = classes.clone();
        assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
        assert_eq!(classes, before);
        assert!(plan.instructions.is_empty());
    }

    #[test]
    fn producer_preserves_borrow_in_phi_chain_without_manual_class() {
        let f = Function {
            symbol: "phis_borrowed".into(),
            name: "phis_borrowed".into(),
            debug_info: None,
            params: vec![(ValueId(0), "entrada".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(1)),
                },
                // Reverse physical order requires more than one propagation round.
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![(
                        ValueId(2),
                        Instruction::Phi {
                            ty: Type::Ref,
                            incoming: vec![(BlockId(1), Operand::Val(ValueId(1)))],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::Phi {
                            ty: Type::Ref,
                            incoming: vec![(BlockId(0), Operand::Val(ValueId(0)))],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Branch(BlockId(2)),
                },
            ],
        };
        let mut classes = HashMap::from([(
            ValueId(0),
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                scope: 0,
            },
        )]);
        let mut plan = TokenPlan {
            ret: super::super::TokenReturn::Borrowed,
            ..Default::default()
        };
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        for v in [ValueId(1), ValueId(2)] {
            assert_eq!(
                classes[&v],
                Ownership::Borrowed {
                    owner: OwnerOrigin::Value(ValueId(0)),
                    scope: 0
                }
            );
        }
        assert!(plan.instructions.is_empty());
    }

    #[test]
    fn trivial_ref_phi_does_not_erase_entry_ownership() {
        let mut f = Function {
            symbol: "phi_ref_trivial".into(),
            name: "phi_ref_trivial".into(),
            debug_info: None,
            params: vec![(ValueId(0), "entrada".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(1)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::Phi {
                            ty: Type::Ref,
                            incoming: vec![(BlockId(0), Operand::Val(ValueId(0)))],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                },
            ],
        };
        for class in [
            Ownership::Owned,
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                scope: 0,
            },
        ] {
            let mut classes =
                HashMap::from([(ValueId(0), class), (ValueId(1), Ownership::Trivial)]);
            let before = classes.clone();
            let mut plan = TokenPlan::default();
            assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
            assert_eq!(classes, before);
            assert!(plan.instructions.is_empty());
        }
        let mut classes = HashMap::from([
            (ValueId(0), Ownership::Trivial),
            (ValueId(1), Ownership::Trivial),
        ]);
        let mut plan = TokenPlan::default();
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
            incoming[0].1 = Operand::Constant(Constant::Null);
        }
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
    }

    #[test]
    fn fallible_query_produces_scalar_only_on_normal_exit() {
        // The record queries do not fail; the address of the closure does.
        for name_str in ["dartforge_ffi_closure_address"] {
            let mut f = Function {
                symbol: "consulta_record".into(),
                name: "consulta_record".into(),
                debug_info: None,
                params: vec![(ValueId(0), "record".into(), Type::Ref)],
                return_ty: Type::I64,
                blocks: vec![
                    BasicBlock {
                        id: BlockId(0),
                        instructions: vec![
                            (
                                ValueId(1),
                                Instruction::CallRuntime {
                                    name: name_str.into(),
                                    args: vec![(Operand::Val(ValueId(0)), Type::Ref)],
                                    ret_ty: Type::I64,
                                },
                                Type::I64,
                            ),
                            (
                                ValueId(2),
                                Instruction::CallRuntime {
                                    name: "dartforge_exception_pending".into(),
                                    args: vec![],
                                    ret_ty: Type::I8,
                                },
                                Type::I8,
                            ),
                            (
                                ValueId(3),
                                Instruction::ICmp(
                                    ICmpOp::Ne,
                                    Operand::Val(ValueId(2)),
                                    Operand::Constant(Constant::Int(0)),
                                ),
                                Type::I1,
                            ),
                        ],
                        terminator: Terminator::CondBranch {
                            cond: Operand::Val(ValueId(3)),
                            then_block: BlockId(1),
                            else_block: BlockId(2),
                        },
                    },
                    BasicBlock {
                        id: BlockId(1),
                        instructions: vec![],
                        terminator: Terminator::Return(Some(Operand::Constant(Constant::Int(0)))),
                    },
                    BasicBlock {
                        id: BlockId(2),
                        instructions: vec![],
                        terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                    },
                ],
            };
            let mut classes = HashMap::from([(
                ValueId(0),
                Ownership::Borrowed {
                    owner: OwnerOrigin::Caller,
                    scope: 0,
                },
            )]);
            let mut plan = TokenPlan::default();
            produce_and_verify_tokens(
                &f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default(),
            )
            .unwrap();
            assert_eq!(classes[&ValueId(1)], Ownership::Trivial);
            assert_eq!(plan.pendings[&ValueId(1)], BlockId(1));
            assert!(plan.instructions[&ValueId(1)].always.is_empty());
            let before = classes.clone();
            let effects = plan.instructions.clone();
            let pendings = plan.pendings.clone();
            f.blocks[1].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
            assert!(
                produce_and_verify_tokens(
                    &f,
                    &mut classes,
                    &mut plan,
                    &FunctionTables::default(),
                    &ScopePlan::default()
                )
                .is_err()
            );
            assert_eq!(classes, before);
            assert_eq!(plan.instructions, effects);
            assert_eq!(plan.pendings, pendings);
        }
    }

    #[test]
    fn typed_arithmetic_produces_plan_without_seeds_and_rejects_ref() {
        let mut f = Function {
            symbol: "aritmetica".into(),
            name: "aritmetica".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::F64,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (ValueId(0), Instruction::Const(Constant::Int(7)), Type::I64),
                    (
                        ValueId(1),
                        Instruction::Mul(
                            Operand::Val(ValueId(0)),
                            Operand::Constant(Constant::Int(6)),
                        ),
                        Type::I64,
                    ),
                    (
                        ValueId(2),
                        Instruction::IntToDouble(Operand::Val(ValueId(1))),
                        Type::F64,
                    ),
                    (
                        ValueId(3),
                        Instruction::FDiv(
                            Operand::Val(ValueId(2)),
                            Operand::Constant(Constant::Double(2.0)),
                        ),
                        Type::F64,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
            }],
        };
        let mut classes = HashMap::new();
        let mut plan = TokenPlan::default();
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        assert_eq!(classes.len(), 4);
        assert!(classes.values().all(|c| *c == Ownership::Trivial));
        let before = classes.clone();
        let effects = plan.instructions.clone();
        f.blocks[0].instructions[1].1 = Instruction::Mul(
            Operand::Constant(Constant::Null),
            Operand::Constant(Constant::Int(6)),
        );
        assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
        assert_eq!(classes, before);
        assert_eq!(plan.instructions, effects);
    }

    #[test]
    fn ref_phi_does_not_accept_scalar_even_with_owned_class() {
        let f = Function {
            symbol: "phi_tipo".into(),
            name: "phi_tipo".into(),
            debug_info: None,
            params: vec![(ValueId(0), "bits".into(), Type::I64)],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(1)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::Phi {
                            ty: Type::Ref,
                            incoming: vec![(BlockId(0), Operand::Val(ValueId(0)))],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                },
            ],
        };
        let mut classes = HashMap::from([(ValueId(0), Ownership::Owned)]);
        let before = classes.clone();
        let mut plan = TokenPlan::default();
        assert!(
            produce_arc_contracts(&f, &mut classes, &mut plan)
                .unwrap_err()
                .contains("Ref representation")
        );
        assert_eq!(classes, before);
        assert!(plan.instructions.is_empty());
        classes.insert(ValueId(0), Ownership::Trivial);
        classes.insert(ValueId(1), Ownership::Trivial);
        let before = classes.clone();
        assert!(
            produce_and_verify_tokens(
                &f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .unwrap_err()
            .contains("SSA Ref or null")
        );
        assert_eq!(classes, before);
    }

    #[test]
    fn scalar_bitcast_requires_origin_and_is_block_order_independent() {
        let mut f = Function {
            symbol: "bits".into(),
            name: "bits".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::F64,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(2)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(2),
                        Instruction::Bitcast {
                            op: Operand::Val(ValueId(1)),
                            to: Type::F64,
                        },
                        Type::F64,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![
                        (
                            ValueId(0),
                            Instruction::Const(Constant::Double(-0.0)),
                            Type::F64,
                        ),
                        (
                            ValueId(1),
                            Instruction::Bitcast {
                                op: Operand::Val(ValueId(0)),
                                to: Type::I64,
                            },
                            Type::I64,
                        ),
                    ],
                    terminator: Terminator::Branch(BlockId(1)),
                },
            ],
        };
        super::super::ssa::verify(&f).unwrap();
        let mut classes = HashMap::new();
        let mut plan = TokenPlan::default();
        produce_arc_contracts(&f, &mut classes, &mut plan).unwrap();
        for id in [ValueId(0), ValueId(1), ValueId(2)] {
            assert_eq!(classes[&id], Ownership::Trivial);
            assert_eq!(plan.instructions[&id], TokenEffect::default());
        }
        let mut module = Module::default();
        module.arc_memory = true;
        module.functions.push(f.clone());
        let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
        assert!(ir.contains("bitcast double %v0 to i64"));
        assert!(ir.contains("bitcast i64 %v1 to double"));
        // The previous scalar success does not authorize reinterpreting a handle
        // nor trusting a representation whose producer gave no contract.
        f.blocks.remove(2);
        f.blocks[0].terminator = Terminator::Branch(BlockId(1));
        f.blocks[1].instructions.remove(0);
        f.blocks[1].instructions.push((
            ValueId(2),
            Instruction::Bitcast {
                op: Operand::Val(ValueId(0)),
                to: Type::F64,
            },
            Type::F64,
        ));
        for (ty, class) in [
            (Type::Ref, Some(Ownership::Trivial)),
            (Type::Ptr, Some(Ownership::Trivial)),
            (Type::I64, Some(Ownership::Owned)),
            (
                Type::I64,
                Some(Ownership::Borrowed {
                    owner: OwnerOrigin::Caller,
                    scope: 0,
                }),
            ),
            (Type::I64, None),
            (Type::F64, Some(Ownership::Trivial)),
        ] {
            f.params = vec![(ValueId(0), "origem".into(), ty)];
            let mut classes: HashMap<_, _> = class.map(|c| (ValueId(0), c)).into_iter().collect();
            let before = classes.clone();
            let mut plan = TokenPlan::default();
            assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
            assert_eq!(classes, before);
            assert!(plan.instructions.is_empty());
        }
        f.params = vec![(ValueId(0), "bits".into(), Type::I64)];
        let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
        produce_arc_contracts(&f, &mut classes, &mut TokenPlan::default()).unwrap();
        // A divergent annotation of the destination also fails without publishing the result.
        f.blocks[1].instructions[0].2 = Type::I64;
        let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
        let before = classes.clone();
        assert!(produce_arc_contracts(&f, &mut classes, &mut TokenPlan::default()).is_err());
        assert_eq!(classes, before);
    }

    #[test]
    fn arithmetic_does_not_erase_ownership_before_bitcast() {
        let f = Function {
            symbol: "origem".into(),
            name: "origem".into(),
            debug_info: None,
            params: vec![(ValueId(0), "origem".into(), Type::I64)],
            return_ty: Type::F64,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(1),
                        Instruction::Add(
                            Operand::Val(ValueId(0)),
                            Operand::Constant(Constant::Int(0)),
                        ),
                        Type::I64,
                    ),
                    (
                        ValueId(2),
                        Instruction::Bitcast {
                            op: Operand::Val(ValueId(1)),
                            to: Type::F64,
                        },
                        Type::F64,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))),
            }],
        };
        for class in [
            None,
            Some(Ownership::Owned),
            Some(Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                scope: 0,
            }),
        ] {
            let mut classes: HashMap<_, _> = class.map(|c| (ValueId(0), c)).into_iter().collect();
            let before = classes.clone();
            let mut plan = TokenPlan::default();
            assert!(
                produce_arc_contracts(&f, &mut classes, &mut plan)
                    .unwrap_err()
                    .contains("Trivial origin")
            );
            assert_eq!(classes, before);
            assert!(plan.instructions.is_empty());
        }
        produce_arc_contracts(
            &f,
            &mut HashMap::from([(ValueId(0), Ownership::Trivial)]),
            &mut TokenPlan::default(),
        )
        .unwrap();
    }

    #[test]
    fn integer_widths_preserve_scalar_contract_and_reject_handles() {
        for (from, to, widen) in [
            (Type::I1, Type::I8, true),
            (Type::I1, Type::I64, true),
            (Type::I8, Type::I64, true),
            (Type::I64, Type::I8, false),
            (Type::I64, Type::I1, false),
            (Type::I8, Type::I1, false),
        ] {
            let inst = if widen {
                Instruction::ZExt {
                    op: Operand::Val(ValueId(0)),
                    from,
                    to,
                }
            } else {
                Instruction::Trunc {
                    op: Operand::Val(ValueId(0)),
                    from,
                    to,
                }
            };
            let mut f = Function {
                symbol: "largura".into(),
                name: "largura".into(),
                debug_info: None,
                params: vec![(ValueId(0), "x".into(), from)],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(ValueId(1), inst, to)],
                    terminator: Terminator::Return(None),
                }],
            };
            let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
            let mut plan = TokenPlan::default();
            produce_and_verify_tokens(
                &f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default(),
            )
            .unwrap();
            assert_eq!(classes[&ValueId(1)], Ownership::Trivial);
            assert_eq!(plan.instructions[&ValueId(1)], TokenEffect::default());
            for (ty, class) in [
                (Type::Ref, Some(Ownership::Trivial)),
                (Type::Ptr, Some(Ownership::Trivial)),
                (from, Some(Ownership::Owned)),
                (
                    from,
                    Some(Ownership::Borrowed {
                        owner: OwnerOrigin::Caller,
                        scope: 0,
                    }),
                ),
                (from, None),
            ] {
                f.params[0].2 = ty;
                let mut classes: HashMap<_, _> =
                    class.map(|c| (ValueId(0), c)).into_iter().collect();
                let before = classes.clone();
                let mut plan = TokenPlan::default();
                assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
                assert_eq!(classes, before);
                assert!(plan.instructions.is_empty());
            }
            f.params[0].2 = from;
            // The opposite operation with the same widths is invalid.
            f.blocks[0].instructions[0].1 = if widen {
                Instruction::Trunc {
                    op: Operand::Val(ValueId(0)),
                    from,
                    to,
                }
            } else {
                Instruction::ZExt {
                    op: Operand::Val(ValueId(0)),
                    from,
                    to,
                }
            };
            let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
            assert!(produce_arc_contracts(&f, &mut classes, &mut TokenPlan::default()).is_err());
            assert_eq!(classes.len(), 1);
        }
        // Sequence of the bool transport through byte and word, with permanent
        // boxing at the end: no Owned token nor pending exit is created.
        let f = Function {
            symbol: "bool_bits".into(),
            name: "bool_bits".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(0),
                        Instruction::ZExt {
                            op: Operand::Constant(Constant::Bool(true)),
                            from: Type::I1,
                            to: Type::I8,
                        },
                        Type::I8,
                    ),
                    (
                        ValueId(1),
                        Instruction::ZExt {
                            op: Operand::Val(ValueId(0)),
                            from: Type::I8,
                            to: Type::I64,
                        },
                        Type::I64,
                    ),
                    (
                        ValueId(2),
                        Instruction::Trunc {
                            op: Operand::Val(ValueId(1)),
                            from: Type::I64,
                            to: Type::I8,
                        },
                        Type::I8,
                    ),
                    (
                        ValueId(3),
                        Instruction::Trunc {
                            op: Operand::Val(ValueId(2)),
                            from: Type::I8,
                            to: Type::I1,
                        },
                        Type::I1,
                    ),
                    (
                        ValueId(4),
                        Instruction::Box {
                            op: Operand::Val(ValueId(3)),
                            from: Type::I1,
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::Return(None),
            }],
        };
        let mut classes = HashMap::new();
        let mut plan = TokenPlan::default();
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        assert_eq!(classes.len(), 5);
        assert!(classes.values().all(|c| *c == Ownership::Trivial));
        assert!(plan.pendings.is_empty());
        let mut module = Module::default();
        module.arc_memory = true;
        module.functions.push(f);
        let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
        assert!(ir.contains("zext i1 true to i8"));
        assert!(ir.contains("zext i8 %v0 to i64"));
        assert!(ir.contains("trunc i64 %v1 to i8"));
        assert!(ir.contains("trunc i8 %v2 to i1"));
    }

    #[test]
    fn verifier_rejects_pure_operation_with_forged_metadata() {
        for (inst, ty) in [
            (
                Instruction::Bitcast {
                    op: Operand::Constant(Constant::Double(0.0)),
                    to: Type::Ref,
                },
                Type::Ref,
            ),
            (
                Instruction::Bitcast {
                    op: Operand::Val(ValueId(0)),
                    to: Type::F64,
                },
                Type::F64,
            ),
            (
                Instruction::ZExt {
                    op: Operand::Val(ValueId(0)),
                    from: Type::Ref,
                    to: Type::I64,
                },
                Type::I64,
            ),
            (
                Instruction::Trunc {
                    op: Operand::Val(ValueId(0)),
                    from: Type::Ref,
                    to: Type::I8,
                },
                Type::I8,
            ),
            (
                Instruction::Add(
                    Operand::Val(ValueId(0)),
                    Operand::Constant(Constant::Int(1)),
                ),
                Type::I64,
            ),
            (
                Instruction::FCmp(
                    FCmpOp::Eq,
                    Operand::Val(ValueId(0)),
                    Operand::Constant(Constant::Double(0.0)),
                ),
                Type::I1,
            ),
            (Instruction::LNot(Operand::Val(ValueId(0))), Type::I1),
            (
                Instruction::Box {
                    op: Operand::Val(ValueId(0)),
                    from: Type::I1,
                },
                Type::Ref,
            ),
            (Instruction::Const(Constant::Int(7)), Type::Ref),
        ] {
            let f = Function {
                symbol: "pura_falseada".into(),
                name: "pura_falseada".into(),
                debug_info: None,
                params: vec![(ValueId(0), "ref".into(), Type::Ref)],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(ValueId(1), inst, ty)],
                    terminator: Terminator::Return(None),
                }],
            };
            let classes = HashMap::from([
                (
                    ValueId(0),
                    Ownership::Borrowed {
                        owner: OwnerOrigin::Caller,
                        scope: 0,
                    },
                ),
                (ValueId(1), Ownership::Trivial),
            ]);
            let mut plan = TokenPlan::default();
            plan.instructions.insert(ValueId(1), TokenEffect::default());
            assert!(
                verify_tokens(&f, &classes, &FunctionTables::default(), &plan).is_err(),
                "{:?}",
                f.blocks[0].instructions[0].1
            );
        }
    }

    #[test]
    fn audited_scalar_field_produces_bits_without_classifying_generic_get() {
        let mut f = Function {
            symbol: "campo_escalar".into(),
            name: "campo_escalar".into(),
            debug_info: None,
            params: vec![(ValueId(0), "objeto".into(), Type::Ref)],
            return_ty: Type::F64,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(1),
                        Instruction::CallRuntime {
                            name: "dartforge_arc_read_field_scalar_v1".into(),
                            args: vec![
                                (Operand::Val(ValueId(0)), Type::Ref),
                                (Operand::Constant(Constant::Int(2)), Type::I64),
                            ],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    ),
                    (
                        ValueId(2),
                        Instruction::Bitcast {
                            op: Operand::Val(ValueId(1)),
                            to: Type::F64,
                        },
                        Type::F64,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))),
            }],
        };
        let origin = HashMap::from([(
            ValueId(0),
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                scope: 0,
            },
        )]);
        let mut classes = origin.clone();
        let mut plan = TokenPlan::default();
        let contracts = produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        let c = &contracts[&ValueId(1)];
        assert_eq!(c.result, Ownership::Trivial);
        assert_eq!(c.effect, TokenEffect::default());
        assert!(!c.invalidates_borrows && !c.calls_dart);
        assert_eq!(classes[&ValueId(2)], Ownership::Trivial);
        // The generic get may return Ref; no I64 annotation certifies it.
        if let Instruction::CallRuntime { name, .. } = &mut f.blocks[0].instructions[0].1 {
            *name = "dartforge_object_get".into();
        }
        let mut classes = origin.clone();
        let mut plan = TokenPlan::default();
        assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
        assert_eq!(classes, origin);
        assert!(plan.instructions.is_empty());
        // The scalar ABI does not accept being annotated as a producer of Ref.
        if let Instruction::CallRuntime { name, ret_ty, .. } = &mut f.blocks[0].instructions[0].1 {
            *name = "dartforge_arc_read_field_scalar_v1".into();
            *ret_ty = Type::Ref;
        }
        f.blocks[0].instructions[0].2 = Type::Ref;
        assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
        assert_eq!(classes, origin);
        assert!(plan.instructions.is_empty());
    }

    #[test]
    fn pure_operation_does_not_reinterpret_reference_as_scalar() {
        for i in [
            Instruction::FCmp(
                FCmpOp::Eq,
                Operand::Val(ValueId(0)),
                Operand::Constant(Constant::Double(0.0)),
            ),
            Instruction::LNot(Operand::Val(ValueId(0))),
        ] {
            let f = Function {
                symbol: "pura".into(),
                name: "pura".into(),
                debug_info: None,
                params: vec![(ValueId(0), "x".into(), Type::Ref)],
                return_ty: Type::I1,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(ValueId(1), i, Type::I1)],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                }],
            };
            let mut classes = HashMap::from([(
                ValueId(0),
                Ownership::Borrowed {
                    owner: OwnerOrigin::Caller,
                    scope: 0,
                },
            )]);
            let before = classes.clone();
            let mut plan = TokenPlan::default();
            assert!(
                produce_arc_contracts(&f, &mut classes, &mut plan)
                    .unwrap_err()
                    .contains("already evaluated scalars")
            );
            assert_eq!(classes, before);
            assert!(plan.instructions.is_empty());
        }
    }

    #[test]
    fn typed_constants_and_permanent_literals_need_no_manual_seed() {
        let constants = [
            Constant::Int(7),
            Constant::Double(1.0),
            Constant::Bool(true),
            Constant::Null,
            Constant::String("literal".into()),
            Constant::StringWtf8(vec![120]),
        ];
        let types = [
            Type::I64,
            Type::F64,
            Type::I1,
            Type::Ref,
            Type::Ref,
            Type::Ref,
        ];
        let mut f = Function {
            symbol: "constantes".into(),
            name: "constantes".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: constants
                    .into_iter()
                    .zip(types)
                    .enumerate()
                    .map(|(i, (k, t))| (ValueId(i as u32), Instruction::Const(k), t))
                    .collect(),
                terminator: Terminator::Return(None),
            }],
        };
        let mut classes = HashMap::new();
        let mut plan = TokenPlan::default();
        f.blocks[0].instructions[0].2 = Type::Ref;
        assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
        assert!(classes.is_empty() && plan.instructions.is_empty());
        f.blocks[0].instructions[0].2 = Type::I64;
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        assert_eq!(classes.len(), 6);
        assert!(classes.values().all(|c| *c == Ownership::Trivial));
    }

    #[test]
    fn loop_bool_phi_is_produced_without_inventing_owner() {
        let mut f = Function {
            symbol: "phi_bool".into(),
            name: "phi_bool".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::I1,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(1)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![
                        (
                            ValueId(1),
                            Instruction::Phi {
                                ty: Type::I1,
                                incoming: vec![
                                    (BlockId(0), Operand::Constant(Constant::Bool(true))),
                                    (BlockId(1), Operand::Val(ValueId(2))),
                                ],
                            },
                            Type::I1,
                        ),
                        (
                            ValueId(2),
                            Instruction::LNot(Operand::Val(ValueId(1))),
                            Type::I1,
                        ),
                    ],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(2)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                },
            ],
        };
        let mut classes = HashMap::new();
        let mut plan = TokenPlan::default();
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        assert_eq!(classes[&ValueId(1)], Ownership::Trivial);
        assert!(!plan.instructions.contains_key(&ValueId(1)));
        let before = classes.clone();
        let effects = plan.instructions.clone();
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
            incoming[0].1 = Operand::Constant(Constant::Int(1));
        }
        assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
        assert_eq!(classes, before);
        assert_eq!(plan.instructions, effects);
    }

    #[test]
    fn numeric_phi_requires_origin_and_semantic_contract() {
        for (ty, constant) in [
            (Type::I1, Constant::Bool(true)),
            (Type::I64, Constant::Int(7)),
            (Type::I8, Constant::Int(2)),
            (Type::F64, Constant::Double(7.0)),
        ] {
            let mut f = Function {
                symbol: "phi_escalar".into(),
                name: "phi_escalar".into(),
                debug_info: None,
                params: vec![],
                return_ty: ty,
                blocks: vec![
                    BasicBlock {
                        id: BlockId(0),
                        instructions: vec![],
                        terminator: Terminator::Branch(BlockId(1)),
                    },
                    BasicBlock {
                        id: BlockId(1),
                        instructions: vec![(
                            ValueId(1),
                            Instruction::Phi {
                                ty,
                                incoming: vec![
                                    (BlockId(0), Operand::Constant(constant)),
                                    (BlockId(1), Operand::Val(ValueId(1))),
                                ],
                            },
                            ty,
                        )],
                        terminator: Terminator::CondBranch {
                            cond: Operand::Constant(Constant::Bool(false)),
                            then_block: BlockId(1),
                            else_block: BlockId(2),
                        },
                    },
                    BasicBlock {
                        id: BlockId(2),
                        instructions: vec![],
                        terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                    },
                ],
            };
            let mut classes = HashMap::new();
            let mut plan = TokenPlan::default();
            produce_and_verify_tokens(
                &f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default(),
            )
            .unwrap();
            assert_eq!(classes[&ValueId(1)], Ownership::Trivial);
            assert!(plan.instructions.is_empty());
            // Not even a previously supplied contract founds a closed cycle.
            if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
                incoming[0].1 = Operand::Val(ValueId(1));
            }
            let before = classes.clone();
            assert!(
                produce_arc_contracts(&f, &mut classes, &mut plan)
                    .unwrap_err()
                    .contains("without a scalar origin")
            );
            assert_eq!(classes, before);
            // A parameter of the same width needs an explicit contract.
            f.params.push((ValueId(0), "entrada".into(), ty));
            if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
                incoming[0].1 = Operand::Val(ValueId(0));
            }
            assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
            classes.insert(ValueId(0), Ownership::Owned);
            let before = classes.clone();
            assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
            assert_eq!(classes, before);
            assert!(plan.instructions.is_empty());
            classes.insert(ValueId(0), Ownership::Trivial);
            produce_and_verify_tokens(
                &f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default(),
            )
            .unwrap();
        }
    }

    #[test]
    fn comparison_produces_bool_without_consuming_reference() {
        let mut f = Function {
            symbol: "cmp".into(),
            name: "cmp".into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::I1,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(1),
                    Instruction::ICmp(
                        ICmpOp::Eq,
                        Operand::Val(ValueId(0)),
                        Operand::Constant(Constant::Null),
                    ),
                    Type::I1,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
            }],
        };
        let initial = HashMap::from([(
            ValueId(0),
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                scope: 0,
            },
        )]);
        let mut classes = initial.clone();
        let mut plan = TokenPlan::default();
        f.blocks[0].instructions[0].2 = Type::Ref;
        assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
        assert_eq!(classes, initial);
        assert!(plan.instructions.is_empty());
        f.blocks[0].instructions[0].2 = Type::I1;
        f.blocks[0].instructions.extend([
            (
                ValueId(2),
                Instruction::FCmp(
                    FCmpOp::Eq,
                    Operand::Constant(Constant::Double(1.0)),
                    Operand::Constant(Constant::Double(1.0)),
                ),
                Type::I1,
            ),
            (
                ValueId(3),
                Instruction::LNot(Operand::Val(ValueId(2))),
                Type::I1,
            ),
        ]);
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        assert_eq!(classes[&ValueId(1)], Ownership::Trivial);
        assert_eq!(plan.instructions[&ValueId(1)], TokenEffect::default());
        plan
            .instructions
            .get_mut(&ValueId(1))
            .unwrap()
            .always
            .push(ValueId(0));
        let before = plan.instructions.clone();
        assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
        assert_eq!(plan.instructions, before);
    }

    #[test]
    fn owned_exception_has_independent_ref_result_without_new_failure() {
        for name_str in [
            "dartforge_arc_exception_owned_v1",
            "dartforge_arc_stack_trace_owned_v1",
        ] {
            let i = Instruction::CallRuntime {
                name: name_str.into(),
                args: vec![],
                ret_ty: Type::Ref,
            };
            let c = runtime_call_contract(&i).unwrap();
            assert_eq!(c.result, Ownership::Owned);
            assert!(!c.effect.may_fail);
            assert!(c.effect.always.is_empty());
            let mut scalar = i.clone();
            if let Instruction::CallRuntime { ret_ty, .. } = &mut scalar {
                *ret_ty = Type::I64;
            }
            assert!(runtime_call_contract(&scalar).is_err());
        }
    }

    #[test]
    fn throw_with_trace_requires_two_ref_borrows_without_consuming_tokens() {
        let mut i = Instruction::CallRuntime {
            name: "dartforge_arc_throw_with_stack_trace_ref_v1".into(),
            args: vec![
                (Operand::Val(ValueId(0)), Type::Ref),
                (Operand::Val(ValueId(1)), Type::Ref),
            ],
            ret_ty: Type::Void,
        };
        let c = runtime_call_contract(&i).unwrap();
        assert!(c.effect.may_fail);
        assert!(c.effect.always.is_empty());
        assert!(c.effect.success.is_empty());
        assert!(c.effect.error.is_empty());
        if let Instruction::CallRuntime { args, .. } = &mut i {
            args[1].1 = Type::I64;
        }
        assert!(runtime_call_contract(&i).is_err());
    }

    #[test]
    fn runtime_constants_preserve_scalar_address_distinction() {
        for k in [
            Constant::Bool(true),
            Constant::Double(1.0),
            Constant::Null,
            Constant::String("x".into()),
            Constant::StringWtf8(vec![120]),
            Constant::Function("getter".into()),
        ] {
            let i = Instruction::CallRuntime {
                name: "dartforge_arc_frame_open_v1".into(),
                args: vec![(Operand::Constant(k), Type::I64)],
                ret_ty: Type::I64,
            };
            assert!(runtime_call_contract(&i).is_err());
        }
        let mut i = Instruction::CallRuntime {
            name: "dartforge_mark_constant".into(),
            args: vec![
                (Operand::Constant(Constant::Null), Type::Ref),
                (
                    Operand::Constant(Constant::Function("getter".into())),
                    Type::I64,
                ),
            ],
            ret_ty: Type::Void,
        };
        runtime_call_contract(&i).unwrap();
        if let Instruction::CallRuntime { args, .. } = &mut i {
            args[1].0 = Operand::Constant(Constant::Bool(false));
        }
        assert!(runtime_call_contract(&i).is_err());
    }

    #[test]
    fn contracts_do_not_infer_ref_by_width_or_hide_tokens() {
        let mut i = Instruction::CallRuntime {
            name: "dartforge_arc_global_receive_v1".into(),
            args: vec![
                (Operand::Val(ValueId(0)), Type::I64),
                (Operand::Val(ValueId(1)), Type::Ref),
            ],
            ret_ty: Type::Void,
        };
        let c = runtime_call_contract(&i).unwrap();
        assert_eq!(c.effect.always, [ValueId(1)]);
        assert!(c.persistent_retention && c.invalidates_borrows);
        if let Instruction::CallRuntime { args, .. } = &mut i {
            args[1].1 = Type::I64;
        }
        assert!(runtime_call_contract(&i).is_err());
        if let Instruction::CallRuntime { args, .. } = &mut i {
            args[1] = (Operand::Constant(Constant::Null), Type::Ref);
        }
        assert!(
            runtime_call_contract(&i)
                .unwrap()
                .effect
                .always
                .is_empty()
        );
        for name_str in ["dartforge_arc_retain", "dartforge_allocate"] {
            let i = Instruction::CallRuntime {
                name: name_str.into(),
                args: vec![],
                ret_ty: Type::Void,
            };
            assert!(runtime_call_contract(&i).is_err());
        }
    }

    #[test]
    fn interrupt_and_byte_without_owners_but_inexact_abi_is_rejected() {
        // The atomic load of the control does not touch the heap nor serve messages.
        // This does not certify the safepoint that runs the slow path.
        let i = Instruction::CallRuntime {
            name: "dartforge_pending_interrupt".into(),
            args: vec![],
            ret_ty: Type::I8,
        };
        let c = runtime_call_contract(&i).unwrap();
        assert_eq!(c.result, Ownership::Trivial);
        assert_eq!(c.effect, TokenEffect::default());
        assert!(!c.invalidates_borrows && !c.persistent_retention);
        for ty in [Type::I1, Type::I64, Type::Ref, Type::Void] {
            let mut wrong = i.clone();
            if let Instruction::CallRuntime { ret_ty, .. } = &mut wrong {
                *ret_ty = ty;
            }
            assert!(runtime_call_contract(&wrong).is_err());
        }
        let mut wrong = i;
        if let Instruction::CallRuntime { args, .. } = &mut wrong {
            args.push((Operand::Constant(Constant::Null), Type::Ref));
        }
        assert!(runtime_call_contract(&wrong).is_err());
        let slow = runtime_call_contract(&Instruction::CallRuntime {
            name: "dartforge_safepoint".into(), args: vec![], ret_ty: Type::Void,
        }).unwrap();
        assert!(slow.effect.may_fail && slow.invalidates_borrows && slow.calls_dart);
    }

    #[test]
    fn rti_define_borrows_object_without_turning_type_id_into_ref() {
        let i = Instruction::CallRuntime {
            name: "dartforge_rti_set".into(),
            args: vec![(Operand::Val(ValueId(0)), Type::Ref),
                (Operand::Val(ValueId(1)), Type::I64)], ret_ty: Type::Void,
        };
        let c = runtime_call_contract(&i).unwrap();
        assert_eq!(c.result, Ownership::Trivial);
        assert_eq!(c.effect, TokenEffect::default());
        assert!(c.invalidates_borrows);
        assert!(!c.calls_dart && !c.persistent_retention);
        for case in 0..4 {
            let mut wrong = i.clone();
            if let Instruction::CallRuntime { args, ret_ty, .. } = &mut wrong {
                match case {
                    0 => args[0].1 = Type::I64,
                    1 => args[1].1 = Type::Ref,
                    2 => args[1].0 = Operand::Constant(Constant::Function("codigo".into())),
                    _ => *ret_ty = Type::Ref,
                }
            }
            assert!(runtime_call_contract(&wrong).is_err());
        }
    }

    #[test]
    fn abi_byte_is_not_i1_boolean() {
        let mut i = Instruction::CallRuntime {
            name: "dartforge_arc_verify_abi".into(),
            args: vec![(Operand::Constant(Constant::Int(1)), Type::I64)],
            ret_ty: Type::I8,
        };
        assert_eq!(
            runtime_call_contract(&i).unwrap().result,
            Ownership::Trivial
        );
        if let Instruction::CallRuntime { ret_ty, .. } = &mut i {
            *ret_ty = Type::I1;
        }
        assert!(runtime_call_contract(&i).is_err());
    }

    #[test]
    fn capture_borrows_env_and_rejects_incompatible_signatures() {
        let i = Instruction::CallRuntime { name: "dartforge_arc_env_ref_v1".into(),
            args: vec![(Operand::Val(ValueId(0)), Type::Ref),
                (Operand::Constant(Constant::Int(39)), Type::I64)], ret_ty: Type::Ref };
        let c = runtime_call_contract(&i).unwrap();
        assert_eq!(c.result, Ownership::Borrowed { owner: OwnerOrigin::Value(ValueId(0)), scope: 0 });
        assert!(!c.persistent_retention && !c.invalidates_borrows && !c.calls_dart && !c.effect.may_fail);
        assert!(c.effect.always.is_empty());
        for case in 0..5 {
            let mut invalidates = i.clone();
            if let Instruction::CallRuntime { args, ret_ty, .. } = &mut invalidates {
                match case {
                    0 => args[0].1 = Type::I64,
                    1 => args[1].1 = Type::Ref,
                    2 => *ret_ty = Type::I64,
                    3 => { args.pop(); },
                    4 => args.push((Operand::Constant(Constant::Int(0)), Type::I64)),
                    _ => unreachable!(),
                }
            }
            assert!(runtime_call_contract(&invalidates).is_err(), "caso {case}");
        }
    }

    #[test]
    fn rti_recipe_borrows_string_and_returns_id_without_inventing_owned_ref() {
        let i = Instruction::CallRuntime { name: "dartforge_rti_recipe".into(),
            args: vec![(Operand::Val(ValueId(0)), Type::Ref)], ret_ty: Type::I64 };
        let c = runtime_call_contract(&i).unwrap();
        assert_eq!(c.result, Ownership::Trivial);
        assert!(!c.persistent_retention && !c.invalidates_borrows && !c.calls_dart && !c.effect.may_fail);
        assert!(c.effect.always.is_empty());
        for case in 0..4 {
            let mut invalidates = i.clone();
            if let Instruction::CallRuntime { args, ret_ty, .. } = &mut invalidates {
                match case {
                    0 => args[0].1 = Type::I64,
                    1 => *ret_ty = Type::Ref,
                    2 => args.clear(),
                    3 => args.push((Operand::Constant(Constant::Int(0)), Type::I64)),
                    _ => unreachable!(),
                }
            }
            assert!(runtime_call_contract(&invalidates).is_err(), "caso {case}");
        }
    }

    #[test]
    fn publish_global_copies_owner_without_consuming_ssa() {
        let i = Instruction::CallRuntime {
            name: "dartforge_gc_global_root".into(),
            args: vec![
                (Operand::Val(ValueId(0)), Type::I64),
                (Operand::Val(ValueId(1)), Type::Ref),
            ],
            ret_ty: Type::Void,
        };
        let c = runtime_call_contract(&i).unwrap();
        assert!(c.effect.always.is_empty() && c.persistent_retention && c.invalidates_borrows);
        assert_eq!(c.result, Ownership::Trivial);
    }

    #[test]
    fn audited_collection_requires_exceptional_exit() {
        let i = Instruction::CallRuntime {
            name: "dartforge_gc_collect".into(),
            args: vec![],
            ret_ty: Type::Void,
        };
        let c = runtime_call_contract(&i).unwrap();
        assert!(c.effect.may_fail && c.invalidates_borrows);
        assert!(
            c.effect.always.is_empty() && c.effect.success.is_empty() && c.effect.error.is_empty()
        );
    }

    #[test]
    fn producer_checks_real_ssa_type_before_publishing() {
        let mut f = Function {
            symbol: "tipo_real".into(),
            name: "tipo_real".into(),
            debug_info: None,
            params: vec![(ValueId(0), "quadro".into(), Type::Ref)],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(1),
                    Instruction::CallRuntime {
                        name: "dartforge_arc_frame_load_v1".into(),
                        args: vec![
                            (Operand::Val(ValueId(0)), Type::I64),
                            (Operand::Constant(Constant::Int(0)), Type::I64),
                        ],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(None),
            }],
        };
        let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
        let initial = classes.clone();
        let mut plan = TokenPlan::default();
        let error = produce_runtime_contracts(&f, &mut classes, &mut plan).unwrap_err();
        assert!(
            error.contains("argument 0") && error.contains("v0"),
            "{error}"
        );
        assert_eq!(classes, initial);
        assert!(plan.instructions.is_empty());
        f.params.clear();
        assert!(produce_runtime_contracts(&f, &mut classes, &mut plan).is_err());
        assert_eq!(classes, initial);
        assert!(plan.instructions.is_empty());
        f.params.push((ValueId(0), "quadro".into(), Type::I64));
        produce_runtime_contracts(&f, &mut classes, &mut plan).unwrap();
        assert_eq!(classes[&ValueId(1)], Ownership::Owned);
    }

    #[test]
    fn producer_is_atomic_and_produces_owned_without_inferring_by_width() {
        let mut f = Function {
            symbol: "f".into(),
            name: "f".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(0),
                    Instruction::CallRuntime {
                        name: "dartforge_arc_frame_load_v1".into(),
                        args: vec![
                            (Operand::Constant(Constant::Int(1)), Type::I64),
                            (Operand::Constant(Constant::Int(0)), Type::I64),
                        ],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(None),
            }],
        };
        let mut classes = HashMap::new();
        let mut plan = TokenPlan::default();
        f.blocks[0].instructions.push((
            ValueId(1),
            Instruction::CallRuntime {
                name: "extern_sem_contrato".into(),
                args: vec![],
                ret_ty: Type::Void,
            },
            Type::Void,
        ));
        assert!(produce_runtime_contracts(&f, &mut classes, &mut plan).is_err());
        assert!(classes.is_empty() && plan.instructions.is_empty());
        f.blocks[0].instructions.pop();
        produce_runtime_contracts(&f, &mut classes, &mut plan).unwrap();
        assert_eq!(classes[&ValueId(0)], Ownership::Owned);
        classes.insert(ValueId(0), Ownership::Trivial);
        let before = plan.instructions.clone();
        assert!(produce_runtime_contracts(&f, &mut classes, &mut plan).is_err());
        assert_eq!(classes[&ValueId(0)], Ownership::Trivial);
        assert_eq!(plan.instructions, before);
    }
}
