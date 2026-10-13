//! Provenance of Ref slots borrowed by closed direct functions.
//! The address is not an owner: these facts do not authorize a raw Ref Load nor
//! replace retention, initialization or cleanup of the owner frame.

use super::super::operands::{operands, terminator_operands};
use crate::hir::*;
use std::collections::{HashMap, HashSet, VecDeque};

/// Rebuilds the local Ref slots and the parameters that only forward them.
///
/// Starts from a Ref Alloca at the entry and follows arguments of direct calls,
/// recursive ones included. Requires the closed set in the same module and checks
/// every caller of that set for each parameter,
/// real SSA types, reads, writes and absence of escape of the address.
/// Entries materialized as a function/closure or published in tables
/// are not closed. Produces no ownership contracts, nor proves that a
/// slot is initialized or that its content is still alive.
/// Tracing returns an empty inventory. Changes no bodies or metadata.
///
/// # Errors
/// Repeated symbols, invalid SSA, incompatible type, a caller without a local
/// origin or an address exposed to an uncovered operation.
///
/// ```
/// use dartforge_emit_native::{hir::Module, optimize::arc::*};
/// let mut m = Module::new(); m.arc_memory = true;
/// assert!(audit_shared_ref_slots(&m)?.is_empty());
/// # Ok::<(), String>(())
/// ```
pub fn audit_shared_ref_slots(
    m: &Module,
) -> Result<HashMap<String, HashSet<ValueId>>, String> {
    if !m.arc_memory {
        return Ok(HashMap::new());
    }
    let mut names = HashMap::new();
    for (fi, f) in m.functions.iter().enumerate() {
        if names.insert(f.symbol.as_str(), fi).is_some() {
            return Err(format!("ARC Ref slots: repeated symbol {}", f.symbol));
        }
    }
    let mut open_set: HashSet<&str> = m
        .arc_uniform_returns
        .iter()
        .map(String::as_str)
        .chain(m.helpers.iter().map(|(_, s)| s.as_str()))
        .chain(m.classes.iter().flat_map(|c| {
            c.vtable
                .iter()
                .map(|(_, s)| s.as_str())
                .chain(c.to_string_symbol.iter().map(String::as_str))
        }))
        .chain(m.entry_symbol.iter().map(String::as_str))
        .chain(m.call_dart.iter().map(String::as_str))
        .chain(m.ffi_callbacks.iter().map(|c| c.body.as_str()))
        .chain(m.source_roots.iter().map(String::as_str))
        .chain(m.table_functions.values().map(String::as_str))
        .chain(
            m.method_tables
                .iter()
                .flat_map(|(_, _, t)| t.iter().map(|(_, s)| s.as_str())),
        )
        .collect();
    for f in &m.functions {
        for (_, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            match i {
                Instruction::Const(Constant::Function(s))
                | Instruction::TearOff { code_symbol: s }
                | Instruction::AllocClosure { code_symbol: s, .. } => {
                    open_set.insert(s.as_str());
                }
                Instruction::AllocClosureTyped {
                    code_symbol,
                    typed,
                    ..
                } => {
                    open_set.insert(code_symbol.as_str());
                    open_set.insert(typed.as_str());
                }
                Instruction::FunctionTable(ss) => {
                    open_set.extend(ss.iter().map(String::as_str));
                }
                _ => {}
            }
        }
    }
    // Includes function constants in any position, returns included.
    let mut operand_symbols = HashSet::new();
    for f in &m.functions {
        let mut constant = |o: &Operand| {
            if let Operand::Constant(Constant::Function(s)) = o {
                operand_symbols.insert(s.clone());
            }
        };
        for b in &f.blocks {
            for (_, i, _) in &b.instructions {
                operands(i, &mut constant);
            }
            terminator_operands(&b.terminator, &mut constant);
        }
    }
    let mut slots = HashSet::new();
    let mut queue = VecDeque::new();
    for (fi, f) in m.functions.iter().enumerate() {
        let roots: HashSet<_> = f
            .blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .filter_map(|(v, i, _)| matches!(i, Instruction::Alloca(Type::Ref)).then_some(*v))
            .collect();
        for (_, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if let Instruction::CallStatic { args, .. } = i {
                for op in args {
                    if let Operand::Val(v) = op
                        && roots.contains(v)
                        && slots.insert((fi, *v))
                    {
                        queue.push_back((fi, *v));
                    }
                }
            }
        }
    }
    while let Some((fi, p)) = queue.pop_front() {
        let f = &m.functions[fi];
        for (_, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if let Instruction::CallStatic { symbol, args, .. } = i {
                for (ai, op) in args.iter().enumerate() {
                    if op != &Operand::Val(p) {
                        continue;
                    }
                    let ci = *names.get(symbol.as_str()).ok_or_else(|| {
                        format!("ARC Ref slots in {}: target without a body {symbol}", f.symbol)
                    })?;
                    let callee = &m.functions[ci];
                    if open_set.contains(symbol.as_str()) || operand_symbols.contains(symbol) {
                        return Err(format!("ARC Ref slots: open entry {symbol}"));
                    }
                    if args.len() != callee.params.len() || callee.params[ai].2 != Type::Ptr {
                        return Err(format!(
                            "ARC Ref slots: incompatible signature of {symbol}"
                        ));
                    }
                    let key = (ci, callee.params[ai].0);
                    if slots.insert(key) {
                        queue.push_back(key);
                    }
                }
            }
        }
    }
    for (fi, f) in m.functions.iter().enumerate() {
        let locals: HashSet<_> = slots
            .iter()
            .filter_map(|(i, v)| (*i == fi).then_some(*v))
            .collect();
        if locals.is_empty() {
            continue;
        }
        super::ssa::verify(f)?;
        let types: HashMap<_, _> = f
            .params
            .iter()
            .map(|(v, _, t)| (*v, *t))
            .chain(
                f.blocks
                    .iter()
                    .flat_map(|b| &b.instructions)
                    .map(|(v, _, t)| (*v, *t)),
            )
            .collect();
        let error = |v: ValueId, msg: &str| format!("ARC Ref slots in {} v{}: {msg}", f.symbol, v.0);
        for (bi, b) in f.blocks.iter().enumerate() {
            for (v, i, ty) in &b.instructions {
                if locals.contains(v)
                    && (!matches!(i, Instruction::Alloca(Type::Ref)) || bi != 0 || *ty != Type::Ptr)
                {
                    return Err(error(*v, "origin requires an Alloca Ref Ptr at the entry"));
                }
                let mut allowed = Vec::new();
                match i {
                    Instruction::Load {
                        ptr: Operand::Val(p),
                        ty: read_done,
                    } if locals.contains(p) => {
                        if *read_done != Type::Ref || *ty != Type::Ref {
                            return Err(error(*v, "read requires Ref"));
                        }
                        allowed.push(*p);
                    }
                    Instruction::Store {
                        ptr: Operand::Val(p),
                        val,
                    } if locals.contains(p) => {
                        let real_ref = match val {
                            Operand::Val(v) => types.get(v) == Some(&Type::Ref),
                            Operand::Constant(Constant::Null) => true,
                            _ => false,
                        };
                        if !real_ref || *ty != Type::Void {
                            return Err(error(*v, "write requires a real Ref"));
                        }
                        allowed.push(*p);
                    }
                    Instruction::CallStatic {
                        symbol,
                        args,
                        ret_ty,
                    } => {
                        for (ai, op) in args.iter().enumerate() {
                            if let Operand::Val(p) = op
                                && locals.contains(p)
                            {
                                let ci = names[symbol.as_str()];
                                let callee = &m.functions[ci];
                                if *ty != callee.return_ty
                                    || *ret_ty != callee.return_ty
                                    || !slots.contains(&(ci, callee.params[ai].0))
                                {
                                    return Err(error(*v, "call diverges from the body"));
                                }
                                allowed.push(*p);
                            }
                        }
                    }
                    _ => {}
                }
                let mut used_set = Vec::new();
                operands(i, &mut |o| {
                    if let Operand::Val(p) = o
                        && locals.contains(p)
                    {
                        used_set.push(*p);
                    }
                });
                if used_set != allowed {
                    return Err(error(*v, "address escapes or gains an alias"));
                }
            }
            let mut escapes = false;
            terminator_operands(&b.terminator, &mut |o| {
                escapes |= matches!(o, Operand::Val(p) if locals.contains(p));
            });
            if escapes {
                return Err(error(ValueId(0), "address escapes through the terminator"));
            }
        }
    }
    // The origin must hold in every caller, not only on the edge that
    // discovered the parameter. An external edge invalidates the whole borrow.
    for (fi, f) in m.functions.iter().enumerate() {
        for (_, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if let Instruction::CallStatic {
                symbol,
                args,
                ret_ty,
            } = i
                && let Some(&ci) = names.get(symbol.as_str())
            {
                let callee = &m.functions[ci];
                for (ai, (p, _, _)) in callee.params.iter().enumerate() {
                    if !slots.contains(&(ci, *p)) {
                        continue;
                    }
                    if args.len() != callee.params.len()
                        || *ret_ty != callee.return_ty
                        || !matches!(args.get(ai), Some(Operand::Val(v)) if slots.contains(&(fi, *v)))
                    {
                        return Err(format!(
                            "ARC Ref slots: caller {} without a closed origin for {symbol}",
                            f.symbol
                        ));
                    }
                }
            }
        }
    }
    let mut result = HashMap::<String, HashSet<ValueId>>::new();
    for (fi, v) in slots {
        result
            .entry(m.functions[fi].symbol.clone())
            .or_default()
            .insert(v);
    }
    Ok(result)
}

/// Intermediate bodies with strong slots and captures by frame/index.
///
/// The producer verifies the origin and the explicit lifetime of the imported pairs.
/// Later changes require repeating that verification. It still needs
/// tokens and exceptional preparation before emission; it is not an ARC
/// certificate. Local frames appear separately from borrowed ones, because only
/// the function that opens a frame may close it.
///
/// ```
/// use dartforge_emit_native::{hir::Module, optimize::arc::*};
/// let mut m = Module::new(); m.arc_memory = true;
/// let r: ArcRefSlotsDraft = lower_shared_ref_slots(&m)?;
/// assert!(r.locals.is_empty() && r.borrowed_set.is_empty());
/// # Ok::<(), String>(())
/// ```
#[derive(Debug)]
pub struct ArcRefSlotsDraft {
    /// New bodies; the input module stays intact.
    pub functions: Vec<Function>,
    /// Per function, the frame parameter → the parameter of its index.
    pub borrowed_set: HashMap<String, HashMap<ValueId, ValueId>>,
    /// Per function, the owner frame opened at the entry and its capacity.
    pub locals: HashMap<String, (ValueId, usize)>,
    /// Symbols whose capture signature changed; all calls are redone.
    pub symbols: HashMap<String, String>,
}

/// Lowers the audited slots to the ABI of ARC owner frames.
///
/// Opens one frame per activation that declares slots, initialized with null by the
/// runtime. Load produces an Owned reference independent of the slot; Store
/// copies into it before releasing the previous content. Captures receive
/// two native i64s, frame and index, instead of the stack address. The new
/// symbol prevents reusing the old signature at link or reload time.
/// Forwarding the same capture keeps the same pair, including in recursion.
/// Closes the local frame at the explicit CFG exits, without closing imported ones.
///
/// The result is intermediate: it does not publish the module/plans, does not classify
/// the native parameters as Dart scalars and does not certify implicit unwinding,
/// tokens or cross-module calls. Checks the lifetime of the pairs on the explicit
/// edges of the CFG. The other protocols must be completed before
/// the returned bodies are emitted.
/// Tracing copies the bodies without changing instructions or signatures.
///
/// # Errors
/// Failure of the origin audit, an entry with predecessors, exhausted IDs,
/// incompatible real call types or a collision of the symbol of the new ABI.
/// The given module stays intact.
///
/// ```
/// use dartforge_emit_native::{hir::Module, optimize::arc::*};
/// let mut m = Module::new(); m.arc_memory = true;
/// let r = lower_shared_ref_slots(&m)?;
/// assert!(r.functions.is_empty() && r.borrowed_set.is_empty());
/// # Ok::<(), String>(())
/// ```
pub fn lower_shared_ref_slots(m: &Module) -> Result<ArcRefSlotsDraft, String> {
    let slots = audit_shared_ref_slots(m)?;
    let mut r = ArcRefSlotsDraft {
        functions: m.functions.clone(),
        borrowed_set: HashMap::new(),
        locals: HashMap::new(),
        symbols: HashMap::new(),
    };
    if slots.is_empty() {
        return Ok(r);
    }
    let names: HashSet<_> = m.functions.iter().map(|f| f.symbol.as_str()).collect();
    let mut linkages = HashMap::<String, HashMap<ValueId, (Operand, Operand)>>::new();
    for f in &mut r.functions {
        let Some(ids) = slots.get(&f.symbol) else {
            continue;
        };
        let mut next_item = f
            .params
            .iter()
            .map(|(v, _, _)| u64::from(v.0))
            .chain(
                f.blocks
                    .iter()
                    .flat_map(|b| &b.instructions)
                    .map(|(v, _, _)| u64::from(v.0)),
            )
            .max()
            .map_or(0, |v| v + 1);
        let mut new = || -> Result<ValueId, String> {
            let id = u32::try_from(next_item)
                .map_err(|_| format!("ARC Ref slots in {}: IDs exhausted", f.symbol))?;
            next_item += 1;
            Ok(ValueId(id))
        };
        let mut pairs = HashMap::new();
        let mut parameters = Vec::new();
        for (v, name_str, ty) in &f.params {
            if ids.contains(v) {
                let index = new()?;
                parameters.push((*v, format!("{name_str}$quadro"), Type::I64));
                parameters.push((index, format!("{name_str}$indice"), Type::I64));
                pairs.insert(*v, (Operand::Val(*v), Operand::Val(index)));
                r.borrowed_set
                    .entry(f.symbol.clone())
                    .or_default()
                    .insert(*v, index);
            } else {
                parameters.push((*v, name_str.clone(), *ty));
            }
        }
        f.params = parameters;
        let mut roots: Vec<_> = f.blocks[0]
            .instructions
            .iter()
            .filter_map(|(v, i, _)| {
                (ids.contains(v) && matches!(i, Instruction::Alloca(Type::Ref))).then_some(*v)
            })
            .collect();
        roots.sort_by_key(|v| v.0);
        if !roots.is_empty() {
            let cfg = super::super::cfg::Cfg::new(f);
            if cfg.successors.iter().any(|ss| ss.contains(&0)) {
                return Err(format!(
                    "ARC Ref slots in {}: entry with predecessors",
                    f.symbol
                ));
            }
            let frame = new()?;
            for (index, v) in roots.iter().enumerate() {
                pairs.insert(
                    *v,
                    (
                        Operand::Val(frame),
                        Operand::Constant(Constant::Int(index as i64)),
                    ),
                );
            }
            r.locals.insert(f.symbol.clone(), (frame, roots.len()));
            f.blocks[0].instructions.insert(
                0,
                (
                    frame,
                    Instruction::CallRuntime {
                        name: "dartforge_arc_quadro_abrir_v1".into(),
                        args: vec![(
                            Operand::Constant(Constant::Int(roots.len() as i64)),
                            Type::I64,
                        )],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                ),
            );
            for b in &mut f.blocks {
                if super::super::cfg::successors(&b.terminator).is_empty() {
                    b.instructions.push((
                        new()?,
                        Instruction::CallRuntime {
                            name: "dartforge_arc_quadro_fechar_v1".into(),
                            args: vec![(Operand::Val(frame), Type::I64)],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ));
                }
            }
        }
        linkages.insert(f.symbol.clone(), pairs);
        if r.borrowed_set.contains_key(&f.symbol) {
            let name_str = format!("{}$arcslots_v1", f.symbol);
            if names.contains(name_str.as_str()) {
                return Err(format!(
                    "ARC Ref slots: symbol of the new ABI already exists {name_str}"
                ));
            }
            r.symbols.insert(f.symbol.clone(), name_str);
        }
    }
    let old_ones: HashMap<_, _> = m.functions.iter().map(|f| (f.symbol.as_str(), f)).collect();
    for f in &mut r.functions {
        if !slots.contains_key(&f.symbol) {
            continue;
        }
        let pairs = linkages.get(&f.symbol);
        for (v, i, ty) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
            match i {
                Instruction::Alloca(Type::Ref) if pairs.is_some_and(|p| p.contains_key(v)) => {
                    // The old ID no longer is an address. Keeping it
                    // as a constant also avoids recycling metadata IDs.
                    let index = pairs.unwrap()[v].1.clone();
                    let Operand::Constant(c) = index else {
                        unreachable!()
                    };
                    *i = Instruction::Const(c);
                    *ty = Type::I64;
                }
                Instruction::Load {
                    ptr: Operand::Val(p),
                    ..
                } if pairs.is_some_and(|ps| ps.contains_key(p)) => {
                    let (frame, index) = &pairs.unwrap()[p];
                    *i = Instruction::CallRuntime {
                        name: "dartforge_arc_quadro_carregar_v1".into(),
                        args: vec![(frame.clone(), Type::I64), (index.clone(), Type::I64)],
                        ret_ty: Type::Ref,
                    };
                }
                Instruction::Store {
                    ptr: Operand::Val(p),
                    val,
                } if pairs.is_some_and(|ps| ps.contains_key(p)) => {
                    let (frame, index) = &pairs.unwrap()[p];
                    *i = Instruction::CallRuntime {
                        name: "dartforge_arc_quadro_copiar_v1".into(),
                        args: vec![
                            (frame.clone(), Type::I64),
                            (index.clone(), Type::I64),
                            (val.clone(), Type::Ref),
                        ],
                        ret_ty: Type::Void,
                    };
                }
                Instruction::CallStatic { symbol, args, .. } if r.symbols.contains_key(symbol) => {
                    let callee = old_ones[symbol.as_str()];
                    let mut new_set = Vec::new();
                    for (arg, (p, _, _)) in args.iter().zip(&callee.params) {
                        if slots.get(symbol).is_some_and(|ss| ss.contains(p)) {
                            let Operand::Val(origin) = arg else {
                                unreachable!()
                            };
                            let (q, index) = &pairs.unwrap()[origin];
                            new_set.extend([q.clone(), index.clone()]);
                        } else {
                            new_set.push(arg.clone());
                        }
                    }
                    *args = new_set;
                    *symbol = r.symbols[symbol].clone();
                }
                _ => {}
            }
        }
        if let Some(name_str) = r.symbols.get(&f.symbol) {
            f.symbol = name_str.clone();
        }
        super::ssa::verify(f)?;
        for (_, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if let Instruction::CallRuntime { name, .. } = i
                && name.starts_with("dartforge_arc_quadro_")
            {
                super::contracts::runtime_call_contract(i)?;
            }
        }
    }
    r.borrowed_set = r
        .borrowed_set
        .into_iter()
        .map(|(s, p)| (r.symbols.get(&s).cloned().unwrap_or(s), p))
        .collect();
    r.locals = r
        .locals
        .into_iter()
        .map(|(s, p)| (r.symbols.get(&s).cloned().unwrap_or(s), p))
        .collect();
    let new_ones: HashMap<_, _> = r.functions.iter().map(|f| (f.symbol.as_str(), f)).collect();
    let targets: HashSet<_> = r.symbols.values().map(String::as_str).collect();
    for f in &r.functions {
        let types: HashMap<_, _> = f
            .params
            .iter()
            .map(|(v, _, t)| (*v, *t))
            .chain(
                f.blocks
                    .iter()
                    .flat_map(|b| &b.instructions)
                    .map(|(v, _, t)| (*v, *t)),
            )
            .collect();
        for (_, i, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if let Instruction::CallStatic {
                symbol,
                args,
                ret_ty,
            } = i
                && targets.contains(symbol.as_str())
            {
                let callee = new_ones[symbol.as_str()];
                let typ = |o: &Operand| match o {
                    Operand::Val(v) => types.get(v).copied(),
                    Operand::Constant(Constant::Null | Constant::String(_)) => Some(Type::Ref),
                    Operand::Constant(Constant::Int(_)) => Some(Type::I64),
                    Operand::Constant(Constant::Double(_)) => Some(Type::F64),
                    Operand::Constant(Constant::Bool(_)) => Some(Type::I1),
                    _ => None,
                };
                if args.len() != callee.params.len()
                    || *ret_ty != callee.return_ty
                    || *ty != *ret_ty
                    || args
                        .iter()
                        .zip(&callee.params)
                        .any(|(a, (_, _, t))| typ(a) != Some(*t))
                {
                    return Err(format!(
                        "ARC Ref slots: incompatible real types of the call from {} to {symbol}",
                        f.symbol
                    ));
                }
            }
        }
    }
    super::slot_lives::verify_cfg_ref_slot_lives(m, &r)?;
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn val(n: u32) -> Operand {
        Operand::Val(ValueId(n))
    }

    fn call(name_str: &str, args: Vec<Operand>) -> Instruction {
        Instruction::CallStatic {
            symbol: name_str.into(),
            args,
            ret_ty: Type::Void,
        }
    }

    fn function(
        name_str: &str,
        parameter: bool,
        instructions: Vec<(ValueId, Instruction, Type)>,
    ) -> Function {
        Function {
            symbol: name_str.into(),
            name: name_str.into(),
            debug_info: None,
            params: if parameter {
                vec![(ValueId(0), "slot".into(), Type::Ptr)]
            } else {
                vec![]
            },
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions,
                terminator: Terminator::Return(None),
            }],
        }
    }

    fn module() -> Module {
        let mut m = Module::new();
        m.arc_memory = true;
        m.functions.push(function(
            "raiz",
            false,
            vec![
                (ValueId(0), Instruction::Alloca(Type::Ref), Type::Ptr),
                (
                    ValueId(1),
                    Instruction::Store {
                        ptr: val(0),
                        val: Operand::Constant(Constant::Null),
                    },
                    Type::Void,
                ),
                (ValueId(2), call("direta", vec![val(0)]), Type::Void),
            ],
        ));
        m.functions.push(function(
            "direta",
            true,
            vec![
                (
                    ValueId(1),
                    Instruction::Load {
                        ptr: val(0),
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(2),
                    Instruction::Store {
                        ptr: val(0),
                        val: val(1),
                    },
                    Type::Void,
                ),
            ],
        ));
        m
    }

    #[test]
    fn recursive_rebuild_is_order_independent_and_produces_no_owner() {
        let mut m = module();
        m.functions[1].blocks[0].instructions.push((
            ValueId(3),
            call("repasse", vec![val(0)]),
            Type::Void,
        ));
        m.functions.push(function(
            "repasse",
            true,
            vec![(ValueId(1), call("direta", vec![val(0)]), Type::Void)],
        ));
        let expected = HashMap::from([
            ("raiz".into(), HashSet::from([ValueId(0)])),
            ("direta".into(), HashSet::from([ValueId(0)])),
            ("repasse".into(), HashSet::from([ValueId(0)])),
        ]);
        let before = format!("{m:?}");
        assert_eq!(audit_shared_ref_slots(&m).unwrap(), expected);
        assert_eq!(format!("{m:?}"), before);
        m.functions.reverse();
        assert_eq!(audit_shared_ref_slots(&m).unwrap(), expected);
        // Even a closed cycle of Ptr with a Ref Load does not create an origin:
        // only the activation that allocates the slot anchors this inventory.
        m.functions.retain(|f| f.symbol != "raiz");
        assert!(audit_shared_ref_slots(&m).unwrap().is_empty());
    }

    #[test]
    fn any_opaque_caller_invalidates_recursive_parameter() {
        for case in 0..3 {
            let mut m = module();
            let args = match case {
                0 => vec![val(0)],
                1 => vec![Operand::Constant(Constant::Int(0))],
                _ => vec![],
            };
            m.functions.push(function(
                "externa",
                true,
                vec![(ValueId(1), call("direta", args), Type::Void)],
            ));
            let before = format!("{m:?}");
            assert!(
                audit_shared_ref_slots(&m)
                    .unwrap_err()
                    .contains("caller"),
                "caso {case}"
            );
            assert_eq!(format!("{m:?}"), before);
        }
    }

    #[test]
    fn materialized_symbol_does_not_become_closed_capture() {
        for case in 0..8 {
            let mut m = module();
            match case {
                0 => m.helpers.push(("nativo".into(), "direta".into())),
                1 => m.entry_symbol = Some("direta".into()),
                2 => m.functions[0].blocks[0].instructions.push((
                    ValueId(3),
                    Instruction::Const(Constant::Function("direta".into())),
                    Type::Ptr,
                )),
                3 => m.functions[0].blocks[0].instructions.push((
                    ValueId(3),
                    Instruction::TearOff {
                        code_symbol: "direta".into(),
                    },
                    Type::Ref,
                )),
                4 => m.functions[0].blocks[0].instructions.push((
                    ValueId(3),
                    Instruction::FunctionTable(vec!["direta".into()]),
                    Type::Ptr,
                )),
                5 => {
                    m.arc_uniform_returns.insert("direta".into());
                }
                6 => m.source_roots.push("direta".into()),
                _ => {
                    m.table_functions.insert(42, "direta".into());
                }
            }
            assert!(
                audit_shared_ref_slots(&m)
                    .unwrap_err()
                    .contains("open entry"),
                "caso {case}"
            );
        }
    }

    #[test]
    fn rejects_alias_escape_and_integer_as_ref_interpretation() {
        for case in 0..8 {
            let mut m = module();
            let b = &mut m.functions[1].blocks[0];
            match case {
                0 => {
                    b.instructions[0].1 = Instruction::Load {
                        ptr: val(0),
                        ty: Type::I64,
                    }
                }
                1 => b.instructions[0].2 = Type::I64,
                2 => {
                    b.instructions[1].1 = Instruction::Store {
                        ptr: val(0),
                        val: Operand::Constant(Constant::Int(0)),
                    }
                }
                3 => {
                    b.instructions[1].1 = Instruction::Store {
                        ptr: val(0),
                        val: val(0),
                    }
                }
                4 => b.instructions.push((
                    ValueId(3),
                    Instruction::Bitcast {
                        op: val(0),
                        to: Type::I64,
                    },
                    Type::I64,
                )),
                5 => b.terminator = Terminator::Return(Some(val(0))),
                6 => b.instructions.push((
                    ValueId(3),
                    Instruction::CallRuntime {
                        name: "opaco".into(),
                        args: vec![(val(0), Type::Ptr)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                )),
                _ => {
                    b.instructions
                        .push((ValueId(3), call("sem_corpo", vec![val(0)]), Type::Void))
                }
            }
            assert!(audit_shared_ref_slots(&m).is_err(), "caso {case}");
        }
    }

    #[test]
    fn signature_and_ssa_origin_are_checked_without_trusting_ptr() {
        for case in 0..5 {
            let mut m = module();
            match case {
                0 => m.functions[1].params[0].2 = Type::I64,
                1 => m.functions[0].blocks[0].instructions[0].2 = Type::I64,
                2 => m.functions[0].blocks[0].instructions[2].2 = Type::Ref,
                3 => {
                    m.functions[0].blocks[0].instructions[2].1 = Instruction::CallStatic {
                        symbol: "direta".into(),
                        args: vec![val(0)],
                        ret_ty: Type::Ref,
                    }
                }
                _ => m.functions[1]
                    .params
                    .push((ValueId(0), "duplicado".into(), Type::Ptr)),
            }
            assert!(audit_shared_ref_slots(&m).is_err(), "caso {case}");
        }
    }

    #[test]
    fn tracing_preserves_body_without_inferring_arc_role() {
        let mut m = module();
        m.arc_memory = false;
        let before = format!("{m:?}");
        assert!(audit_shared_ref_slots(&m).unwrap().is_empty());
        assert_eq!(format!("{m:?}"), before);
    }

    #[test]
    fn lowering_uses_one_frame_and_preserves_identity_of_aliased_captures() {
        for alias in [false, true] {
            let mut m = module();
            if !alias {
                m.functions[0].blocks[0]
                    .instructions
                    .insert(1, (ValueId(3), Instruction::Alloca(Type::Ref), Type::Ptr));
            }
            let Instruction::CallStatic { args, .. } =
                &mut m.functions[0].blocks[0].instructions.last_mut().unwrap().1
            else {
                unreachable!()
            };
            args.push(val(if alias { 0 } else { 3 }));
            m.functions[1]
                .params
                .push((ValueId(4), "segundo".into(), Type::Ptr));
            m.functions[1].blocks[0].instructions.push((
                ValueId(3),
                Instruction::Load {
                    ptr: val(4),
                    ty: Type::Ref,
                },
                Type::Ref,
            ));
            let before = format!("{m:?}");
            let r = lower_shared_ref_slots(&m).unwrap();
            assert_eq!(format!("{m:?}"), before);
            let root = &r.functions[0];
            let direct = &r.functions[1];
            assert_eq!(r.locals.len(), 1);
            assert_eq!(r.locals["raiz"].1, if alias { 1 } else { 2 });
            assert_eq!(direct.symbol, "direta$arcslots_v1");
            assert_eq!(
                direct.params.iter().map(|(_, _, t)| *t).collect::<Vec<_>>(),
                vec![Type::I64; 4]
            );
            assert_eq!(r.borrowed_set[&direct.symbol].len(), 2);
            let (q, _) = r.locals["raiz"];
            let call = root.blocks[0]
                .instructions
                .iter()
                .find_map(|(_, i, _)| {
                    if let Instruction::CallStatic { symbol, args, .. } = i {
                        Some((symbol, args))
                    } else {
                        None
                    }
                })
                .unwrap();
            assert_eq!(call.0, &direct.symbol);
            assert_eq!(
                call.1,
                &vec![
                    Operand::Val(q),
                    Operand::Constant(Constant::Int(0)),
                    Operand::Val(q),
                    Operand::Constant(Constant::Int(if alias { 0 } else { 1 }))
                ]
            );
            assert!(
                r.functions
                    .iter()
                    .flat_map(|f| &f.blocks)
                    .flat_map(|b| &b.instructions)
                    .all(|(_, i, _)| !matches!(
                        i,
                        Instruction::Alloca(Type::Ref)
                            | Instruction::Load { .. }
                            | Instruction::Store { .. }
                    ))
            );
            let reads: Vec<_> = direct.blocks[0]
                .instructions
                .iter()
                .filter_map(|(_, i, _)| {
                    if let Instruction::CallRuntime { name, args, .. } = i
                        && name == "dartforge_arc_quadro_carregar_v1"
                    {
                        Some(args)
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(reads.len(), 2);
            for (_, inst, _) in &direct.blocks[0].instructions {
                if matches!(inst, Instruction::CallRuntime { name, .. } if name == "dartforge_arc_quadro_carregar_v1")
                {
                    assert_eq!(
                        super::super::contracts::runtime_call_contract(inst)
                            .unwrap()
                            .result,
                        super::super::Ownership::Owned
                    );
                }
            }
            for (li, args) in reads.iter().enumerate() {
                assert_eq!(args[0], (Operand::Val(direct.params[2 * li].0), Type::I64));
                assert_eq!(
                    args[1],
                    (Operand::Val(direct.params[2 * li + 1].0), Type::I64)
                );
            }
            assert!(direct.blocks[0].instructions.iter().all(|(_, i, _)|
                !matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_arc_quadro_fechar_v1")));
            super::super::frames::verify(root).unwrap();
        }
    }

    #[test]
    fn lowering_forwards_recursive_pairs_and_closes_each_explicit_exit() {
        let mut m = module();
        m.functions[1].blocks[0].instructions.push((
            ValueId(3),
            call("direta", vec![val(0)]),
            Type::Void,
        ));
        m.functions[0].blocks[0].terminator = Terminator::CondBranch {
            cond: Operand::Constant(Constant::Bool(true)),
            then_block: BlockId(1),
            else_block: BlockId(2),
        };
        for id in [1, 2] {
            m.functions[0].blocks.push(BasicBlock {
                id: BlockId(id),
                instructions: vec![],
                terminator: Terminator::Return(None),
            });
        }
        let r = lower_shared_ref_slots(&m).unwrap();
        let root = &r.functions[0];
        let direct = &r.functions[1];
        let pairs = &r.borrowed_set[&direct.symbol];
        assert!(matches!(&direct.blocks[0].instructions.last().unwrap().1,
            Instruction::CallStatic { symbol, args, .. } if symbol == &direct.symbol && args == &vec![val(0), Operand::Val(pairs[&ValueId(0)])]));
        for b in &root.blocks[1..] {
            assert!(matches!(&b.instructions.last().unwrap().1,
                Instruction::CallRuntime { name, .. } if name == "dartforge_arc_quadro_fechar_v1"));
        }
        super::super::frames::verify(root).unwrap();
    }

    #[test]
    fn lowering_rejects_collision_exhausted_ids_and_frame_reopened_in_loop() {
        for case in 0..4 {
            let mut m = module();
            match case {
                0 => m
                    .functions
                    .push(function("direta$arcslots_v1", false, vec![])),
                1 => m.functions[0].blocks[0].instructions.push((
                    ValueId(u32::MAX),
                    Instruction::Const(Constant::Null),
                    Type::Ref,
                )),
                2 => m.functions[0].blocks[0].terminator = Terminator::Branch(BlockId(0)),
                _ => {
                    m.functions[1]
                        .params
                        .push((ValueId(4), "objeto".into(), Type::Ref));
                    let Instruction::CallStatic { args, .. } =
                        &mut m.functions[0].blocks[0].instructions[2].1
                    else {
                        unreachable!()
                    };
                    args.push(Operand::Constant(Constant::Int(0)));
                    // The address is closed, but that does not authorize passing
                    // a scalar word as another Ref argument.
                    assert!(audit_shared_ref_slots(&m).is_ok());
                }
            }
            let before = format!("{m:?}");
            assert!(lower_shared_ref_slots(&m).is_err(), "caso {case}");
            assert_eq!(format!("{m:?}"), before);
        }
        let mut m = module();
        m.arc_memory = false;
        let r = lower_shared_ref_slots(&m).unwrap();
        assert_eq!(format!("{:?}", r.functions), format!("{:?}", m.functions));
        assert!(r.locals.is_empty() && r.borrowed_set.is_empty() && r.symbols.is_empty());
    }
}
