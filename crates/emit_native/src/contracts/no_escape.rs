//! Conservative closed-HIR non-publication analysis for `NoEscape`.
//!
//! All managed inputs and reference-valued reads may alias the requested
//! parameter or its transitive object graph. This intentionally loses precision
//! rather than erase preexisting aliases. Only non-publishing operations and
//! transitively checked static bodies are admitted. A result is an HIR proof,
//! not certification of emitted helpers, exception cleanup, or runtime entry.

use crate::hir::{Function, Instruction as I, Module, Operand, Terminator, Type, ValueId};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Version of this deliberately restricted non-publication analysis.
pub const CHECKER_VERSION: &str = "noescape-readonly-hir-v2";

/// Structural body dependency, excluding unordered debug position maps.
/// Any transformation requires fresh verification; this is not an artifact hash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BodyDependency {
    pub symbol: String,
    pub hir_blake3: String,
}
/// Evidence for the closed HIR subset, not a complete backend certificate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HirProof {
    pub function: String,
    pub parameter_index: usize,
    pub checker_version: &'static str,
    pub dependencies: Vec<BodyDependency>,
    /// Generated helpers/ABI/cleanup must be checked before discharging metadata.
    pub emission_audit_required: bool,
    /// These obligations are not discharged by the HIR subset checker.
    pub outstanding_requirements: Vec<ProofRequirement>,
}
/// Conditions needed before HIR evidence can discharge the source annotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProofRequirement {
    ValidatedSourceParameterMapping,
    ValidTypedSsa,
    NativeScalarAliasProvenance,
    GeneratedAbiHelpersAndCleanup,
    RecheckAfterAnyTransformation,
}
/// A path explaining an unsupported operation or potential publication.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unproved {
    pub path: Vec<String>,
    pub reason: String,
}
/// No result grants ownership or allows removing RC/GC operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckResult {
    Proved(HirProof),
    Unproved(Unproved),
}

/// Checks a managed parameter against a conservative closed-HIR subset.
///
/// `parameter_index` is the actual HIR ABI index, including hidden receivers;
/// mapping a source formal must be validated separately. This checker validates
/// the admitted typed CFG/SSA subset. Recheck after every transformation and
/// audit emitted helper/cleanup effects before treating this evidence as a
/// complete `NoEscape` certificate. Heap and
/// global reads have no validated field representation witness here and remain
/// unproved. Managed references from any argument/read are conservatively
/// possibly aliases of the requested object; no preexisting alias is removed.
/// Native scalar/address ABI provenance is an additional caller obligation.
/// Recursion, indirect/native calls, allocation, memory mutation, local stack
/// cells without initialization witnesses, exceptional exits, and unsupported
/// instructions remain unproved.
///
/// # Errors
///
/// Returns `CheckResult::Unproved` for unavailable/malformed mappings, unsupported
/// effects, recursion, potential publication, or missing layout/ABI evidence.
/// The function never turns an annotation into a trusted summary.
///
/// ```
/// use dartforge_emit_native::{contracts::no_escape::{verify_parameter, CheckResult}, hir::Module};
/// assert!(matches!(verify_parameter(&Module::default(), "absent", 0), CheckResult::Unproved(_)));
/// ```
pub fn verify_parameter(module: &Module, symbol: &str, parameter_index: usize) -> CheckResult {
    let mut checker = Checker {
        module,
        functions: HashMap::new(),
        visiting: Vec::new(),
        done: BTreeMap::new(),
    };
    for function in &module.functions {
        if checker
            .functions
            .insert(function.symbol.as_str(), function)
            .is_some()
        {
            return failure(symbol, "duplicate function symbol; mapping is ambiguous");
        }
    }
    let Some(function) = checker.functions.get(symbol).copied() else {
        return failure(symbol, "lowered function is unavailable");
    };
    if function
        .params
        .get(parameter_index)
        .is_none_or(|p| p.2 != Type::Ref)
    {
        return failure(
            symbol,
            "requested source parameter has no validated managed-reference HIR mapping",
        );
    }
    match checker.function(symbol) {
        Err(reason) => CheckResult::Unproved(reason),
        Ok(()) => CheckResult::Proved(HirProof {
            function: symbol.into(),
            parameter_index,
            checker_version: CHECKER_VERSION,
            dependencies: checker.done.into_values().collect(),
            emission_audit_required: true,
            outstanding_requirements: vec![
                ProofRequirement::ValidatedSourceParameterMapping,
                ProofRequirement::NativeScalarAliasProvenance,
                ProofRequirement::GeneratedAbiHelpersAndCleanup,
                ProofRequirement::RecheckAfterAnyTransformation,
            ],
        }),
    }
}

fn failure(symbol: &str, reason: &str) -> CheckResult {
    CheckResult::Unproved(Unproved {
        path: vec![symbol.into()],
        reason: reason.into(),
    })
}

struct Checker<'a> {
    module: &'a Module,
    functions: HashMap<&'a str, &'a Function>,
    visiting: Vec<String>,
    done: BTreeMap<String, BodyDependency>,
}
impl Checker<'_> {
    fn error(&self, location: &str, reason: impl Into<String>) -> Unproved {
        let mut path = self.visiting.clone();
        path.push(location.into());
        Unproved {
            path,
            reason: reason.into(),
        }
    }

    fn function(&mut self, symbol: &str) -> Result<(), Unproved> {
        if self.done.contains_key(symbol) {
            return Ok(());
        }
        if self.visiting.iter().any(|s| s == symbol) {
            return Err(self.error(
                symbol,
                "recursive call cycle has no proved fixed-point summary",
            ));
        }
        let Some(function) = self.functions.get(symbol).copied() else {
            return Err(self.error(
                symbol,
                "callee body or audited non-publication summary is unavailable",
            ));
        };
        self.visiting.push(symbol.into());
        let checked = self.body(function).and_then(|()| {
            super::ssa::validate(self.module, function)
                .map_err(|failure| {
                    self.error(
                        &format!(
                            "typed CFG/SSA {:?} block {:?} value {:?}",
                            failure.kind, failure.block, failure.value
                        ),
                        failure.reason,
                    )
                })
                .map(|_| ())
        });
        self.visiting.pop();
        checked?;
        self.done.insert(
            symbol.into(),
            BodyDependency {
                symbol: symbol.into(),
                hir_blake3: blake3::hash(body_fingerprint_input(function).as_bytes())
                    .to_hex()
                    .to_string(),
            },
        );
        Ok(())
    }

    fn body(&mut self, function: &Function) -> Result<(), Unproved> {
        let mut types = HashMap::new();
        for (id, _, ty) in &function.params {
            if *ty == Type::Ptr || types.insert(*id, *ty).is_some() {
                return Err(self.error(
                    "parameters",
                    "opaque pointer ABI or duplicate SSA parameter",
                ));
            }
        }
        let mut blocks = HashMap::new();
        for block in &function.blocks {
            if blocks.insert(block.id, block).is_some() {
                return Err(self.error("CFG", "duplicate block identity"));
            }
            for (id, _instruction, ty) in &block.instructions {
                if types.insert(*id, *ty).is_some() {
                    return Err(self.error("SSA", "duplicate value identity"));
                }
            }
        }
        let Some(entry) = function.blocks.first() else {
            return Err(self.error("CFG", "function body is unavailable"));
        };
        let mut pending = vec![entry.id];
        let mut visited = HashSet::new();
        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            let Some(block) = blocks.get(&id).copied() else {
                return Err(self.error("CFG", "branch target is unavailable"));
            };
            for (value, instruction, ty) in &block.instructions {
                let at = format!("block {} value {}", id.0, value.0);
                self.instruction(instruction, *ty, &types, &at)?;
            }
            let at = format!("block {} exit", id.0);
            match &block.terminator {
                Terminator::Return(Some(value)) => {
                    let ty = operand_type(value, &types)
                        .ok_or_else(|| self.error(&at, "return value has no SSA definition"))?;
                    if ty != function.return_ty {
                        return Err(self.error(
                            &at,
                            "implicit return conversion has unaudited generated effects",
                        ));
                    }
                    if ty == Type::Ptr || ty == Type::Ref && !is_null(value) {
                        return Err(self.error(&at, "return may publish a parameter, a transitive field, or a preexisting alias"));
                    }
                }
                Terminator::Return(None) if function.return_ty == Type::Void => {}
                Terminator::Return(None) => return Err(self.error(&at, "missing return value")),
                Terminator::Branch(target) => pending.push(*target),
                Terminator::CondBranch {
                    cond,
                    then_block,
                    else_block,
                } => {
                    self.scalar(cond, &types, &at)?;
                    pending.extend([*else_block, *then_block]);
                }
                Terminator::Switch {
                    val,
                    default,
                    cases,
                } => {
                    self.scalar(val, &types, &at)?;
                    pending.push(*default);
                    pending.extend(cases.iter().map(|(_, b)| *b));
                }
                Terminator::Throw(_) => {
                    return Err(self.error(
                        &at,
                        "exceptional publication/cleanup requires a separate proof",
                    ));
                }
                Terminator::Unreachable => {
                    return Err(self.error(
                        &at,
                        "unreachable exit or implicit exception path is not audited",
                    ));
                }
            }
        }
        Ok(())
    }

    fn scalar(
        &self,
        value: &Operand,
        types: &HashMap<ValueId, Type>,
        at: &str,
    ) -> Result<(), Unproved> {
        match operand_type(value, types) {
            Some(Type::Ref | Type::Ptr | Type::Void) | None => Err(self.error(at, "reference/address conversion could hide a derived reference; scalar typing is required")),
            _ => Ok(()),
        }
    }

    fn instruction(
        &mut self,
        instruction: &I,
        ty: Type,
        types: &HashMap<ValueId, Type>,
        at: &str,
    ) -> Result<(), Unproved> {
        match instruction {
            I::Const(crate::hir::Constant::Null) if ty == Type::Ref => {},
            I::Const(crate::hir::Constant::Int(_) | crate::hir::Constant::Double(_) | crate::hir::Constant::Bool(_)) if !matches!(ty, Type::Ref | Type::Ptr | Type::Void) => {},
            I::ICmp(crate::hir::ICmpOp::Eq | crate::hir::ICmpOp::Ne, a, b)
                if operand_type(a, types) == Some(Type::Ref) && operand_type(b, types) == Some(Type::Ref) && ty == Type::I1 => {},
            I::Add(a,b) | I::Sub(a,b) | I::Mul(a,b) | I::And(a,b) | I::Or(a,b) | I::Xor(a,b) | I::Shl(a,b) | I::AShr(a,b) | I::LShr(a,b)
            | I::FAdd(a,b) | I::FSub(a,b) | I::FMul(a,b) | I::FDiv(a,b) | I::ICmp(_,a,b) | I::FCmp(_,a,b) => { self.scalar(a, types, at)?; self.scalar(b, types, at)?; self.scalar_result(ty, at)?; }
            I::Neg(a) | I::Not(a) | I::FNeg(a) | I::LNot(a) | I::IntToDouble(a) | I::ZExt {op:a,..} | I::Trunc {op:a,..} | I::Bitcast {op:a,..} => { self.scalar(a, types, at)?; self.scalar_result(ty, at)?; }
            I::GetField { .. } | I::LoadGlobal { .. } => {
                return Err(self.error(at, "heap/global representation has no validated layout witness; a registered scalar type may expose raw reference or native-handle bits"));
            }
            I::Alloca(_) | I::Load { .. } | I::Store { .. } => {
                return Err(self.error(at, "local memory has no CFG definite-initialization/alias witness; stale stack bits may be references or native addresses"));
            }
            I::Phi { incoming, ty: declared } if *declared == ty && ty != Type::Ptr => {
                if incoming.iter().any(|(_, value)| operand_type(value, types) != Some(ty)) { return Err(self.error(at, "phi mixes incompatible reference/scalar representations")); }
            }
            I::CallStatic { symbol, args, ret_ty } => {
                let Some(callee) = self.functions.get(symbol.as_str()).copied() else { return Err(self.error(at, format!("opaque callee {symbol}; it may observe or publish preexisting aliases"))); };
                if *ret_ty != ty || ty != callee.return_ty || args.len() != callee.params.len() || args.iter().zip(&callee.params).any(|(a,p)| operand_type(a, types) != Some(p.2)) {
                    return Err(self.error(at, "callee ABI conversion/argument mapping is not audited"));
                }
                self.function(symbol).map_err(|mut error| { error.path.insert(error.path.len().saturating_sub(1), at.into()); error })?;
            }
            _ => return Err(self.error(at, format!("instruction requires an unavailable non-publication/effect summary: {instruction:?}"))),
        }
        Ok(())
    }

    fn scalar_result(&self, ty: Type, at: &str) -> Result<(), Unproved> {
        if matches!(ty, Type::Ref | Type::Ptr | Type::Void) {
            Err(self.error(
                at,
                "scalar instruction hides a reference or unaudited boxing",
            ))
        } else {
            Ok(())
        }
    }
}

// Keep unordered debug-position maps out of dependency identity. Scalar bit
// patterns are appended for admitted operations because Debug collapses NaNs.
fn body_fingerprint_input(function: &Function) -> String {
    let mut input = format!(
        "{:?}",
        (
            &function.symbol,
            &function.params,
            function.return_ty,
            &function.blocks
        )
    );
    let mut append = |value: &Operand| {
        if let Operand::Constant(crate::hir::Constant::Double(value)) = value {
            input.push_str(&format!(";f64:{:016x}", value.to_bits()));
        }
    };
    for block in &function.blocks {
        for (_, instruction, _) in &block.instructions {
            match instruction {
                I::Const(crate::hir::Constant::Double(value)) => {
                    append(&Operand::Constant(crate::hir::Constant::Double(*value)))
                }
                I::Add(a, b)
                | I::Sub(a, b)
                | I::Mul(a, b)
                | I::And(a, b)
                | I::Or(a, b)
                | I::Xor(a, b)
                | I::Shl(a, b)
                | I::AShr(a, b)
                | I::LShr(a, b)
                | I::FAdd(a, b)
                | I::FSub(a, b)
                | I::FMul(a, b)
                | I::FDiv(a, b)
                | I::ICmp(_, a, b)
                | I::FCmp(_, a, b) => {
                    append(a);
                    append(b);
                }
                I::Neg(a)
                | I::Not(a)
                | I::FNeg(a)
                | I::LNot(a)
                | I::IntToDouble(a)
                | I::ZExt { op: a, .. }
                | I::Trunc { op: a, .. }
                | I::Bitcast { op: a, .. } => append(a),
                I::Phi { incoming, .. } => {
                    for (_, value) in incoming {
                        append(value);
                    }
                }
                I::CallStatic { args, .. } => {
                    for value in args {
                        append(value);
                    }
                }
                _ => {}
            }
        }
        match &block.terminator {
            Terminator::Return(Some(value)) | Terminator::Throw(value) => append(value),
            Terminator::CondBranch { cond, .. } => append(cond),
            Terminator::Switch { val, .. } => append(val),
            _ => {}
        }
    }
    input
}

fn is_null(value: &Operand) -> bool {
    matches!(value, Operand::Constant(crate::hir::Constant::Null))
}
fn operand_type(value: &Operand, types: &HashMap<ValueId, Type>) -> Option<Type> {
    Some(match value {
        Operand::Val(value) => *types.get(value)?,
        Operand::Constant(crate::hir::Constant::Null) => Type::Ref,
        Operand::Constant(
            crate::hir::Constant::String(_)
            | crate::hir::Constant::StringWtf8(_)
            | crate::hir::Constant::Function(_),
        ) => return None,
        Operand::Constant(crate::hir::Constant::Int(_)) => Type::I64,
        Operand::Constant(crate::hir::Constant::Double(_)) => Type::F64,
        Operand::Constant(crate::hir::Constant::Bool(_)) => Type::I1,
    })
}

#[cfg(test)]
mod tests;
