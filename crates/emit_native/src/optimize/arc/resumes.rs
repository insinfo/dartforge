//! Validation of the native pair that ARC cleanups hand back to the Itanium unwinder.
use super::*;

/// A cleanup that preserves the native pair: drops and an audited close without collection.
pub(crate) fn resume_instruction(i: &Instruction) -> bool {
    match i {
        Instruction::ArcDrop { .. } => true,
        Instruction::CallRuntime { name, args, ret_ty } => {
            name == "dartforge_arc_quadro_fechar_v1"
                && *ret_ty == Type::Void
                && matches!(args.as_slice(), [(Operand::Val(_), Type::I64)])
        }
        _ => false,
    }
}

// The emitter and the tokens share the contract before referencing the landing pad.
pub(crate) fn check(f: &Function, t: &FunctionTables) -> Result<(), String> {
    if !t
        .exits
        .values()
        .any(|mode| *mode == ExceptionExit::Resume)
    {
        return Ok(());
    }
    let mut resumes = t.clone();
    resumes
        .exits
        .retain(|_, mode| *mode == ExceptionExit::Resume);
    if resumes.exits.is_empty() {
        return Ok(());
    }
    tokens::check_exceptional_exits(f, &resumes, true)?;
    if let Some(c) = &t.shared_frames {
        c.check(f)?;
        super::frames::analyze_shared(f, c.pairs(), &c.targets())?;
    } else { super::frames::verify(f)?; }
    let mut invocations = HashMap::<BlockId, usize>::new();
    for id in t.invocations.values() {
        *invocations.entry(*id).or_default() += 1;
    }
    let mut arrivals = HashMap::<BlockId, usize>::new();
    for b in &f.blocks {
        for id in super::super::cfg::successors(&b.terminator) {
            if !resumes.exits.contains_key(&id) {
                continue;
            }
            let coherent = matches!(&b.terminator,
                Terminator::CondBranch { cond: Operand::Constant(Constant::Bool(false)), then_block, else_block }
                if *then_block == id && *else_block != id)
                && b.instructions.last().is_some_and(|(v, i, _)| {
                    t.invocations.get(v) == Some(&id) && matches!(i, Instruction::CallStatic { .. })
                });
            if !coherent {
                return Err(format!("Retoma b{} tem entrada sem invoke", id.0));
            }
            *arrivals.entry(id).or_default() += 1;
        }
    }
    let mut ids: Vec<_> = resumes.exits.keys().copied().collect();
    ids.sort_by_key(|id| id.0);
    for id in ids {
        if arrivals.get(&id) != Some(&1) || invocations.get(&id) != Some(&1) {
            return Err(format!("Retoma b{} exige exatamente um invoke", id.0));
        }
    }
    Ok(())
}
