//! Binds port declarations to the verified bodies, contracts and dependencies.

use super::*;

/// Port inventory of a fixed AOT image, bound to the verified HIR.
///
/// Private fields prevent declaring Owned/readonly without verifying the bodies.
/// It is not a certificate for JIT reload nor for legacy indirect calls.
///
/// ```
/// use dartforge_emit_native::hir::Module;
/// assert!(Module::new().arc_aot_ports.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArcAotPorts {
    state: String,
    uniforms: Vec<String>,
    callbacks: Vec<(String, usize)>,
    typed_set: Vec<(String, i64)>,
}

fn state(m: &Module) -> String {
    // Also keeps the callees, layouts and tables: changing just one dependency
    // may invalidate the transfer even if the registered body does not change.
    format!(
        "{:?}/{:?}/{:?}/{:?}/{:?}",
        (
            &m.functions,
            &m.tables,
            m.arc_memory,
            m.excecoes_por_tabelas
        ),
        (
            &m.dart_scalar_parameters,
            &m.dart_rti_parameters,
            &m.dart_vector_parameters,
            &m.dart_ref_returns,
            &m.arc_uniform_returns
        ),
        (
            &m.classes,
            &m.arc_field_layouts,
            &m.globals,
            &m.arc_rti_caches
        ),
        (
            &m.helpers,
            &m.method_tables,
            m.sdk_mode,
            m.sdk_library
        ),
        (
            &m.entry_symbol,
            &m.record,
            &m.call_dart,
            &m.arc_shared_functions
        )
    )
}

impl ArcAotPorts {
    pub(crate) fn check(&self, m: &Module) -> Result<(), String> {
        if self.state != state(m) {
            return Err("portas ARC: certificado obsoleto da imagem AOT".into());
        }
        Ok(())
    }

    pub(crate) fn uniforms(&self) -> &[String] {
        &self.uniforms
    }
    pub(crate) fn callbacks(&self) -> &[(String, usize)] {
        &self.callbacks
    }
    pub(crate) fn typed_set(&self) -> &[(String, i64)] {
        &self.typed_set
    }
}

// The prepared constructor keeps the nominal address and the original descriptor.
// Do not register arbitrary words or deduce Owned from the width of the return.
// Typed calls find the body through the runtime registry, for the whole
// process. `outside_abis` brings ABIs registered by other images; `None`
// only collects the bodies, without checking the calls.
pub(super) fn check_typed(
    m: &Module,
    plans: &HashMap<String, DartFunctionPlan>,
    outside_abis: Option<&[i64]>,
) -> Result<Vec<(String, i64)>, String> {
    let functions: HashMap<_, _> = m.functions.iter().map(|f| (&f.symbol, f)).collect();
    let mut typed_set = Vec::new();
    for f in &m.functions {
        for (v, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
            let Instruction::CallRuntime { name, args, ret_ty } = inst else { continue };
            if name != "dartforge_arc_closure_owned_v1" { continue; }
            let error = || format!("portas ARC: closure tipada {} v{} sem corpo/ABI/retorno Owned verificado", f.symbol, v.0);
            let [
                (Operand::Constant(Constant::Function(uniform)), Type::I64),
                (_, Type::Ref),
                (alvo, Type::I64),
                (Operand::Constant(Constant::Int(abi)), Type::I64),
            ] = args.as_slice() else { return Err(error()); };
            if *ty != Type::Ref || *ret_ty != Type::Ref
                || !m.arc_uniform_returns.contains(uniform)
            { return Err(error()); }
            match alvo {
                Operand::Constant(Constant::Int(0)) if *abi == 0 => continue,
                Operand::Constant(Constant::Function(s)) => {
                    let body = functions.get(s).ok_or_else(error)?;
                    // ABI zero disables the typed path, including for scalar
                    // bodies that still use only the uniform entry.
                    if *abi == 0 { continue; }
                    let params: Vec<_> = body.params.iter().map(|(_, _, t)| *t).collect();
                    let ret_val = if body.return_ty == Type::Ref { TokenReturn::Owned } else { TokenReturn::Trivial };
                    if super::objects::typed_abi(&params, body.return_ty) != Some(*abi)
                        || plans.get(s).map(|p| p.tokens.ret) != Some(ret_val)
                    { return Err(error()); }
                    typed_set.push((s.clone(), *abi));
                }
                _ => return Err(error()),
            }
        }
    }
    typed_set.sort();
    typed_set.dedup();
    let Some(outside_abis) = outside_abis else { return Ok(typed_set) };
    for f in &m.functions {
        for (_, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if let Instruction::TypedCall { args, ret, .. } = inst {
                let params: Vec<_> = args.iter().map(|(_, t)| *t).collect();
                let abi = super::objects::typed_abi(&params, *ret);
                if !typed_set.iter().any(|(_, code)| Some(*code) == abi)
                    && !abi.is_some_and(|a| outside_abis.contains(&a))
                {
                    return Err(format!("portas ARC: {} chama ABI tipada sem corpo registrado na imagem", f.symbol));
                }
            }
        }
    }
    Ok(typed_set)
}

// Each vector address may only be read or forwarded to the same role of a
// local body. That way call cycles do not invent readonly by declaration.
fn check_vectors(m: &Module) -> Result<(), String> {
    let functions: HashMap<_, _> = m.functions.iter().map(|f| (&f.symbol, f)).collect();
    for (s, roles) in &m.dart_vector_parameters {
        let f = functions
            .get(s)
            .ok_or_else(|| format!("portas ARC: vetor de função ausente {s}"))?;
        for (p, role) in roles {
            if f.params.iter().find(|(v, _, _)| v == p).map(|(_, _, t)| t) != Some(&Type::Ptr) {
                return Err(format!("portas ARC: vetor {s} v{} sem parâmetro Ptr", p.0));
            }
            for (v, i, t) in f.blocks.iter().flat_map(|b| &b.instructions) {
                let uses = |o: &Operand| matches!(o, Operand::Val(id) if id == p);
                let mut used = false;
                super::super::operands::operands(i, &mut |o| used |= uses(o));
                if !used {
                    continue;
                }
                let allowed = match i {
                    Instruction::CallSelectorForward {
                        arc_owned: true,
                        recv,
                        args,
                        desc,
                        ..
                    }
                    | Instruction::CallClosureForward {
                        arc_owned: true,
                        closure: recv,
                        args,
                        desc,
                        ..
                    } => {
                        !uses(recv)
                            && match role {
                                DartEntryVector::Arguments => uses(args) && !uses(desc),
                                DartEntryVector::Descriptor => uses(desc) && !uses(args),
                            }
                    }
                    Instruction::CallStatic { symbol, args, .. } => {
                        functions.get(symbol).is_some_and(|callee| {
                            args.len() == callee.params.len()
                                && args.iter().enumerate().all(|(j, o)| {
                                    !uses(o)
                                        || m.dart_vector_parameters
                                            .get(symbol)
                                            .and_then(|p| p.get(&callee.params[j].0))
                                            == Some(role)
                                })
                        })
                    }
                    Instruction::LoadIndexed { base, index } => {
                        (*role == DartEntryVector::Descriptor
                            && uses(base)
                            && matches!(index, Operand::Constant(Constant::Int(0 | 1)))
                            && *t == Type::I64)
                            // The slot of the type tuple, through the descriptor.
                            || (*role == DartEntryVector::Arguments
                                && super::vectors::tuple_read(f, i, *t, roles))
                    }
                    Instruction::CallRuntime { name, args, .. } => {
                        super::vectors::checked_read(i, roles) || (*role == DartEntryVector::Descriptor
                            && matches!(
                                name.as_str(),
                                "dartforge_args_casam" | "dartforge_arg_indice"
                            )
                            && args.first().is_some_and(|(o, t)| uses(o) && *t == Type::Ptr)
                            && args.iter().skip(1).all(|(o, _)| !uses(o)))
                    }
                    _ => false,
                };
                if !allowed {
                    return Err(format!(
                        "portas ARC: vetor {role:?} {s} v{} sem uso readonly coberto: {i:?}",
                        v.0
                    ));
                }
            }
            for b in &f.blocks {
                let mut published = false;
                super::super::operands::terminator_operands(&b.terminator, &mut |o| {
                    published |= matches!(o, Operand::Val(v) if v == p);
                });
                if published {
                    return Err(format!("portas ARC: {s} publica vetor no terminador"));
                }
            }
        }
    }
    Ok(())
}

/// Certifies Owned ports of already prepared bodies in a closed AOT module.
///
/// Verifies all the bodies/callees again and requires an idempotent preparation.
/// Uniform entries also require readonly vectors; Ref reads of
/// arguments use the adapter that checks the index and produces an owner. The real
/// extent of the vectors remains a precondition of the caller's ABI.
/// The AOT image stays alive until the process ends. Reloading/removing
/// JIT code requires another protocol. Tracing produces no registrations nor changes the HIR.
///
/// # Errors
/// Unprepared body, incompatible ABI, open dependency, published vector,
/// unaudited indirect/heap operation or a runtime boundary not yet scoped.
/// No certificate is published on failure.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let mut m = Module::new(); m.arc_memory = true;
/// m.functions.push(Function { symbol: "helper".into(), name: "helper".into(),
///     debug_info: None, params: vec![], return_ty: Type::Ref,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) }] });
/// m.dart_ref_returns.insert("helper".into());
/// m.helpers.push(("runtime_name".into(), "helper".into()));
/// let mut plans = HashMap::from([("helper".into(), DartFunctionPlan::default())]);
/// prepare_arc_dart_module(&mut m, &mut plans)?;
/// assert_eq!(certify_arc_aot_ports(&mut m, &plans)?, 1);
/// # Ok::<(), String>(())
/// ```
pub fn certify_arc_aot_ports(
    m: &mut Module,
    plans: &HashMap<String, DartFunctionPlan>,
) -> Result<usize, String> {
    certify(m, plans, None)
}

/// [`certify_arc_aot_ports`]. With the layouts of a set, the caller
/// has already proven that the preparation of the whole set is idempotent
/// (`modules::certify_arc_module_ports`), and the checked writes
/// use those layouts.
pub(super) fn certify(
    m: &mut Module,
    plans: &HashMap<String, DartFunctionPlan>,
    set_layouts: Option<&HashMap<u32, Vec<Type>>>,
) -> Result<usize, String> {
    let idempotence = set_layouts.is_none();
    // A program prepared against the prepared SDK: its layouts too.
    let with_externals;
    let set_layouts = if set_layouts.is_none() && !m.arc_external_layouts.is_empty() {
        let mut l = m.arc_external_layouts.clone();
        l.extend(m.arc_field_layouts.iter().map(|(c, x)| (*c, x.clone())));
        with_externals = l;
        Some(&with_externals)
    } else {
        set_layouts
    };
    if !m.arc_memory {
        return Ok(0);
    }
    for f in &m.functions {
        for (_, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            super::fields::check_call(f, inst, Some(set_layouts.unwrap_or(&m.arc_field_layouts)))?;
        }
    }
    let getters_rti = super::rti_caches::getters_rti(m, plans);
    super::selectors::check(m, &getters_rti)?;
    check_vectors(m)?;
    let mut fs = m.functions.clone();
    let mut ps = plans.clone();
    produce_lowering_scalar_parameters(m, &mut ps)?;
    produce_lowering_rti_parameters(m, &mut ps)?;
    super::vectors::produce(m, &mut ps)?;
    for (index, f) in fs.iter().enumerate() {
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
        let reference = |o: &Operand| {
            matches!(o, Operand::Val(v) if types.get(v) == Some(&Type::Ref))
                || matches!(
                    o,
                    Operand::Constant(
                        Constant::Null | Constant::String(_) | Constant::StringWtf8(_)
                    )
                )
        };
        let p = ps
            .get(&f.symbol)
            .ok_or_else(|| format!("portas ARC: plano ausente {}", f.symbol))?;
        if m.excecoes_por_tabelas {
            if m.tables.get(index) != Some(&p.tables) {
                return Err(format!(
                    "portas ARC: tabelas emitidas divergentes em {}",
                    f.symbol
                ));
            }
        } else if !p.tables.invocations.is_empty()
            || p.tables
                .exits
                .values()
                .any(|s| *s == ExceptionExit::Resume)
        {
            let calls: Vec<String> = f.blocks.iter().flat_map(|b| &b.instructions)
                .filter(|(v, _, _)| p.tables.invocations.contains_key(v))
                .map(|(v, i, _)| format!("v{} {}", v.0, match i {
                    Instruction::CallStatic { symbol, .. } => symbol.as_str(),
                    _ => "?",
                }))
                .collect();
            return Err(format!(
                "portas ARC: {} exige emissão de invokes/tabelas: {calls:?}",
                f.symbol
            ));
        }
        // Writes into list elements, through the address that
        // `derived_set::verify` holds; those through FFI pointers do not pass.
        super::derived_set::verify(f)?;
        let list_addresses = super::derived_set::addresses(f);
        for (_, i, t) in f.blocks.iter().flat_map(|b| &b.instructions) {
            super::typed_set::check(f, i, *t)?;
            super::selectors::check_local_vector(f, i, *t, Some(&getters_rti))?;
            // Identity (`==`/`!=` of handles, the null test) does not convert
            // the reference; ordering over handles would be a coercion.
            if matches!(i, Instruction::ICmp(op, a, b)
                if (reference(a) || reference(b)) && !matches!(op, ICmpOp::Eq | ICmpOp::Ne))
                || matches!(i, Instruction::StoreGlobal { val, ty, .. } if *ty == Type::Ref || reference(val))
            {
                return Err(format!(
                    "portas ARC: {} exige operação explícita sem coerção Ref/escapes implícitos",
                    f.symbol
                ));
            }
            let list_write = matches!(i, Instruction::NativeWrite { address: Operand::Val(d), .. }
                if list_addresses.contains(d));
            if !list_write && matches!(
                i,
                Instruction::CallDynamic { .. }
                    | Instruction::CallInterface { .. }
                    | Instruction::CallClosureForward {
                        arc_owned: false,
                        ..
                    }
                    | Instruction::CallSelectorForward {
                        arc_owned: false,
                        ..
                    }
                    // C calls (`NativeCall*`) have a scalar ABI
                    // checked by the preparation (`contracts::produce`); the
                    // reentry through a callback runs in the native scope of the stack.
                    | Instruction::NativeWrite { .. }
                    | Instruction::GetField { .. }
                    | Instruction::SetField { .. }
                    | Instruction::CellSet { .. }
            ) || (*t == Type::Ref
                && !matches!(
                    i,
                    Instruction::Const(
                        Constant::Null | Constant::String(_) | Constant::StringWtf8(_)
                    ) | Instruction::Phi { .. }
                        | Instruction::ArcCopy { .. }
                        | Instruction::ArcMove { .. }
                        | Instruction::ArcLoadStrong { .. }
                        | Instruction::CallStatic { .. }
                        | Instruction::CallSelectorForward {
                            arc_owned: true,
                            ..
                        }
                        | Instruction::CallClosureForward {
                            arc_owned: true,
                            ..
                        }
                        | Instruction::CallRuntime { .. }
                        | Instruction::TypedCall { .. }
                        | Instruction::CallClosure { .. }
                        | Instruction::CallSelector { .. }
                        // The Owned factories that the preparation contracts
                        // (`contracts::produce`): literals, interpolation and
                        // the SIMD box.
                        | Instruction::AllocList { .. }
                        | Instruction::AllocRecord { .. }
                        | Instruction::ConcatStrings { .. }
                        | Instruction::Box { .. }
                ))
            {
                return Err(format!(
                    "portas ARC: {} contém operação sem ABI auditada: {i:?}",
                    f.symbol
                ));
            }
            if let Instruction::CallRuntime { name, .. } = i
                && crate::llvm::externs::effects_of(name).calls_dart
                // The `dartforge_arc_*` ABIs that run Dart open the native
                // scope internally (`em_escopo_nativo`/`owner_em_escopo_nativo`).
                && !name.starts_with("dartforge_arc_")
                && !name.starts_with(super::NATIVE_SCOPE_PREFIX)
                && !name.starts_with(super::GENERIC_OWNED_PREFIX)
            {
                return Err(format!(
                    "portas ARC: {} tem fronteira runtime não escopada {name}",
                    f.symbol
                ));
            }
            if let Instruction::CallRuntime { name, .. } = i
                && (name.starts_with("dartforge_arc_escopo_nativo_")
                    || name.starts_with("dartforge_arc_registrar_")
                    || name == "dartforge_arc_retirar_retorno_owned_v1")
            {
                return Err(format!(
                    "portas ARC: {} manipula escopos/registros nativos sem protocolo verificado",
                    f.symbol
                ));
            }
        }
    }
    if idempotence {
        let before = format!("{fs:?}");
        let plans_before = ps.clone();
        let changes = super::functions::prepare_functions(&mut fs, &mut ps, m.arc_external_callees)?;
        if changes != (0, 0) || before != format!("{fs:?}") || plans_before != ps {
            return Err("portas ARC: corpos/planos ainda não preparados ou divergentes".into());
        }
    }
    let mut uniforms: Vec<_> = m.arc_uniform_returns.iter().cloned().collect();
    uniforms.sort();
    // Revalidates the nominal signature against the bodies, without trusting the name alone.
    produce_lowering_ref_returns(m, &mut ps)?;
    let by_name: HashMap<_, _> = fs.iter().map(|f| (&f.symbol, f)).collect();
    let mut callbacks = Vec::new();
    for (_, s) in &m.helpers {
        let f = by_name
            .get(s)
            .ok_or_else(|| format!("portas ARC: helper sem corpo {s}"))?;
        if f.return_ty != Type::Ref || uniforms.contains(s) {
            continue;
        }
        let n = f.params.len();
        if !matches!(n, 0..=5 | 7)
            || f.params
                .iter()
                .any(|(_, _, t)| !matches!(t, Type::Ref | Type::I64 | Type::Ptr))
        {
            return Err(format!("portas ARC: ABI de helper não coberta {s}"));
        }
        callbacks.push((s.clone(), n));
    }
    callbacks.sort();
    callbacks.dedup();
    // In a set, or with callees from another image, the typed body may come
    // from another image (a program closure passed to the SDK, or the
    // reverse): the runtime registry checks the body of each closure
    // before the call (`dartforge_arc_alvo_tipada_v1`).
    let typed_set = check_typed(m, plans, (idempotence && !m.arc_external_callees).then_some(&[][..]))?;
    let total = callbacks.len() + uniforms.len() + typed_set.len();
    m.arc_aot_ports = Some(ArcAotPorts {
        state: state(m),
        uniforms,
        callbacks,
        typed_set,
    });
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llvm::LlvmEmitter;

    fn module(tables: bool) -> (Module, HashMap<String, DartFunctionPlan>) {
        let mut m = Module::new();
        m.arc_memory = true;
        let f = Function {
            symbol: "folha".into(),
            name: "folha".into(),
            debug_info: None,
            params: vec![
                (ValueId(0), "recv".into(), Type::Ref),
                (ValueId(1), "args".into(), Type::Ptr),
                (ValueId(2), "desc".into(), Type::Ptr),
            ],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        };
        let mut g = f.clone();
        g.symbol = "repasse".into();
        g.name = "repasse".into();
        g.blocks[0].instructions.push((
            ValueId(3),
            Instruction::CallStatic {
                symbol: "folha".into(),
                args: (0..3).map(|i| Operand::Val(ValueId(i))).collect(),
                ret_ty: Type::Ref,
            },
            Type::Ref,
        ));
        g.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(3))));
        m.functions = vec![f, g];
        for s in ["folha", "repasse"] {
            m.arc_uniform_returns.insert(s.into());
            m.dart_vector_parameters.insert(
                s.into(),
                HashMap::from([
                    (ValueId(1), DartEntryVector::Arguments),
                    (ValueId(2), DartEntryVector::Descriptor),
                ]),
            );
        }
        if tables {
            crate::optimize::excecoes_por_tabelas(&mut m);
        }
        let mut p = HashMap::from([
            ("folha".into(), DartFunctionPlan::default()),
            ("repasse".into(), DartFunctionPlan::default()),
        ]);
        prepare_arc_dart_module(&mut m, &mut p).unwrap();
        (m, p)
    }

    #[test]
    fn selector_forward_prepares_owned_and_emits_adapter_with_pending() {
        for closure in [false, true] {
            for case in 0..4 {
                let (mut m, mut plans) = module(false);
                plans.insert("repasse".into(), DartFunctionPlan::default());
                let g = &mut m.functions[1];
                g.blocks.truncate(1);
                g.blocks[0].instructions = vec![
                    (
                        ValueId(3),
                        Instruction::CallSelectorForward {
                            arc_owned: false,
                            selector: "c:identidade".into(),
                            recv: Operand::Val(ValueId(0)),
                            args: Operand::Val(ValueId(1)),
                            desc: Operand::Val(ValueId(2)),
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(4),
                        Instruction::CallRuntime {
                            name: "dartforge_exception_pending".into(),
                            args: vec![],
                            ret_ty: Type::I8,
                        },
                        Type::I8,
                    ),
                    (
                        ValueId(5),
                        Instruction::ICmp(
                            ICmpOp::Ne,
                            Operand::Val(ValueId(4)),
                            Operand::Constant(Constant::Int(0)),
                        ),
                        Type::I1,
                    ),
                ];
                g.blocks[0].terminator = Terminator::CondBranch {
                    cond: Operand::Val(ValueId(5)),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                };
                g.blocks.extend([
                    BasicBlock {
                        id: BlockId(1),
                        instructions: vec![],
                        terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
                    },
                    BasicBlock {
                        id: BlockId(2),
                        instructions: vec![],
                        terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
                    },
                ]);
                if closure {
                    g.blocks[0].instructions[0].1 = Instruction::CallClosureForward {
                        arc_owned: false,
                        closure: Operand::Val(ValueId(0)),
                        args: Operand::Val(ValueId(1)),
                        desc: Operand::Val(ValueId(2)),
                    };
                }
                if case == 1 {
                    if let Instruction::CallSelectorForward { args, .. }
                    | Instruction::CallClosureForward { args, .. } =
                        &mut g.blocks[0].instructions[0].1
                    {
                        *args = Operand::Val(ValueId(2));
                    }
                } else if case == 2 {
                    g.blocks[0].instructions.pop();
                } else if case == 3 {
                    m.arc_memory = false;
                }
                let before = format!("{m:?}/{plans:?}");
                let result = prepare_arc_dart_module(&mut m, &mut plans);
                if matches!(case, 1 | 2) {
                    assert!(result.is_err(), "{result:?}");
                    assert_eq!(before, format!("{m:?}/{plans:?}"));
                    continue;
                }
                result.unwrap();
                if case == 3 {
                    assert_eq!(before, format!("{m:?}/{plans:?}"));
                    continue;
                }
                assert_eq!(plans["repasse"].classes[&ValueId(3)], Ownership::Owned);
                assert_eq!(plans["repasse"].tokens.pendings[&ValueId(3)], BlockId(1));
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    (0, 0)
                );
                certify_arc_aot_ports(&mut m, &plans).unwrap();
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                if closure {
                    assert!(
                        ir.contains("call i64 @dartforge_arc_invocar_closure_uniforme_v1(i64 %v0")
                    );
                    assert!(!ir.contains("call i64 %cf3("));
                } else {
                    assert!(ir.contains("call i64 @dartforge_arc_invocar_uniforme_v1(i64 %sc3"));
                }
                assert!(!ir.contains("call i64 %sf3("));
            }
        }
    }

    #[test]
    fn typed_derive_from_constructor_and_require_owned_body_and_exact_abi() {
        for case in 0..10 {
            let (mut m, mut p) = module(false);
            m.functions.push(Function {
                symbol: "tipada".into(), name: "tipada".into(), debug_info: None,
                params: vec![(ValueId(0), "contexto".into(), Type::Ref),
                    (ValueId(1), "x".into(), Type::Ref)], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }],
            });
            m.dart_ref_returns.insert("tipada".into());
            m.functions.push(Function {
                symbol: "capturar".into(), name: "capturar".into(), debug_info: None,
                params: vec![], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(0), Instruction::AllocClosureTyped {
                        code_symbol: "folha".into(), env: Operand::Constant(Constant::Null),
                        typed: "tipada".into(), abi: 31, direct: false,
                    }, Type::Ref),
                ], terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }],
            });
            m.dart_ref_returns.insert("capturar".into());
            p.insert("tipada".into(), DartFunctionPlan::default());
            p.insert("capturar".into(), DartFunctionPlan::default());
            prepare_arc_dart_module(&mut m, &mut p).unwrap();
            // Duplicating the construction cannot duplicate the registration of the same PC/ABI.
            let inst = m.functions[3].blocks[0].instructions[0].1.clone();
            m.functions[3].blocks[0].instructions.insert(0, (ValueId(5), inst, Type::Ref));
            prepare_arc_dart_module(&mut m, &mut p).unwrap();
            if case == 7 {
                certify_arc_aot_ports(&mut m, &p).unwrap();
                m.functions[2].params[1].2 = Type::F64;
                assert!(m.arc_aot_ports.as_ref().unwrap().check(&m).is_err());
                continue;
            }
            if case == 8 {
                m.functions[2].blocks[0].instructions.retain(|(_, i, _)| !matches!(i, Instruction::ArcCopy { .. }));
                m.functions[2].blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
            } else if case == 9 {
                // The nominal producer cannot replace a proof of a Borrowed
                // return after the verification of the bodies by the certificate.
                p.get_mut("tipada").unwrap().tokens.ret = TokenReturn::Borrowed;
            } else if case != 0 {
                for (_, inst, _) in &mut m.functions[3].blocks[0].instructions {
                    if let Instruction::CallRuntime { name, args, .. } = inst
                        && name == "dartforge_arc_closure_owned_v1"
                    {
                        match case {
                            1 => args[2].0 = Operand::Constant(Constant::Int(123)),
                            2 => args[2].0 = Operand::Constant(Constant::Function("ausente".into())),
                            3 => args[3].0 = Operand::Constant(Constant::Int(41)),
                            4 => args[0].0 = Operand::Constant(Constant::Function("tipada".into())),
                            5 => args[2].0 = Operand::Constant(Constant::Int(0)),
                            6 => args[3].0 = Operand::Constant(Constant::Int(0)),
                            _ => unreachable!(),
                        }
                    }
                }
            }
            let before = format!("{m:?}/{p:?}");
            let result = certify_arc_aot_ports(&mut m, &p);
            if matches!(case, 0 | 6) {
                assert_eq!(result.unwrap(), if case == 0 { 3 } else { 2 });
                let certificate = m.arc_aot_ports.clone();
                assert_eq!(certificate.as_ref().unwrap().typed_set().len(), usize::from(case == 0));
                certify_arc_aot_ports(&mut m, &p).unwrap();
                assert_eq!(m.arc_aot_ports, certificate);
                let ir = LlvmEmitter::new(&m).emit_all();
                assert_eq!(ir.matches(" = call i64 @dartforge_arc_registrar_tipada_v1").count(), usize::from(case == 0));
                if case == 0 { assert!(ir.contains("i64 %t0, i64 31)")); }
            } else {
                assert!(result.is_err(), "caso {case}: {result:?}");
                assert_eq!(before, format!("{m:?}/{p:?}"));
            }
        }
    }

    #[test]
    fn certified_aot_registration_is_idempotent_and_stays_before_dart() {
        for tables in [false, true] {
            let (mut m, p) = module(tables);
            assert_eq!(certify_arc_aot_ports(&mut m, &p).unwrap(), 2);
            let certificate = m.arc_aot_ports.clone();
            assert_eq!(certify_arc_aot_ports(&mut m, &p).unwrap(), 2);
            assert_eq!(m.arc_aot_ports, certificate);
            let ir = LlvmEmitter::new(&m).emit_all();
            assert_eq!(
                ir.matches(" = call i64 @dartforge_arc_registrar_uniforme_owned_v1")
                    .count(),
                2
            );
            assert!(ir.contains("df.arc.compativel:\n  call void @df.arc.registrar_portas()"));
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    LlvmEmitter::new(&m).with_maps_in_jit(true).emit_all()
                }))
                .is_err()
            );
        }
    }

    #[test]
    fn emission_rejects_stale_body_dependency_mode_and_metadata() {
        for case in 0..7 {
            let (mut m, p) = module(true);
            certify_arc_aot_ports(&mut m, &p).unwrap();
            match case {
                0 => m.functions[0].blocks[0].instructions.clear(),
                1 => m.functions[1].name.push_str("_mudou"),
                2 => m.arc_memory = false,
                3 => m.tables[0].checks_stack = !m.tables[0].checks_stack,
                4 => m.dart_vector_parameters.clear(),
                5 => m.arc_uniform_returns.clear(),
                6 => m.helpers.push(("injetado".into(), "repasse".into())),
                _ => unreachable!(),
            }
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    LlvmEmitter::new(&m).emit_all()
                }))
                .is_err(),
                "caso {case}"
            );
        }
    }

    #[test]
    fn vectors_do_not_get_readonly_by_alias_publication_or_open_call() {
        for case in 0..4 {
            let (mut m, p) = module(false);
            m.functions[0].blocks[0].instructions.push((
                ValueId(50),
                match case {
                    0 => Instruction::Bitcast {
                        op: Operand::Val(ValueId(2)),
                        to: Type::I64,
                    },
                    1 => Instruction::LoadIndexed {
                        base: Operand::Val(ValueId(1)),
                        index: Operand::Constant(Constant::Int(0)),
                    },
                    2 => Instruction::CallStatic {
                        symbol: "fora_do_modulo".into(),
                        args: vec![Operand::Val(ValueId(2))],
                        ret_ty: Type::I64,
                    },
                    3 => Instruction::StoreGlobal {
                        symbol: "escape".into(),
                        val: Operand::Val(ValueId(1)),
                        ty: Type::Ptr,
                        root: None,
                    },
                    _ => unreachable!(),
                },
                Type::I64,
            ));
            let before = format!("{m:?}/{p:?}");
            let error = certify_arc_aot_ports(&mut m, &p).unwrap_err();
            assert!(error.contains("readonly"), "{error}");
            assert_eq!(format!("{m:?}/{p:?}"), before);
            assert!(m.arc_aot_ports.is_none());
        }
    }

    #[test]
    fn own_declaration_without_preparing_body_and_tracing_do_not_publish_ports() {
        let (mut m, p) = module(false);
        m.functions[0].blocks[0].instructions.clear();
        m.functions[0].blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(0))));
        let before = format!("{m:?}");
        assert!(certify_arc_aot_ports(&mut m, &p).is_err());
        assert_eq!(format!("{m:?}"), before);
        m.arc_memory = false;
        let before = format!("{m:?}");
        assert_eq!(certify_arc_aot_ports(&mut m, &p).unwrap(), 0);
        assert_eq!(format!("{m:?}"), before);
        let ir = LlvmEmitter::new(&m).emit_all();
        assert!(!ir.contains("define private void @df.arc.registrar_portas"));
    }

    #[test]
    fn body_does_not_certify_implicit_unboxing_or_native_scope_change() {
        for scope in [false, true] {
            let (mut m, p) = module(false);
            m.functions[0].blocks[0].instructions.push((
                ValueId(50),
                if scope {
                    Instruction::CallRuntime {
                        name: "dartforge_arc_escopo_nativo_abrir_v1".into(),
                        args: vec![],
                        ret_ty: Type::I64,
                    }
                } else {
                    // Ordering over handles is a coercion; identity (`==`) is not.
                    Instruction::ICmp(
                        ICmpOp::Slt,
                        Operand::Val(ValueId(0)),
                        Operand::Constant(Constant::Null),
                    )
                },
                if scope { Type::I64 } else { Type::I1 },
            ));
            let before = format!("{m:?}");
            let error = certify_arc_aot_ports(&mut m, &p).unwrap_err();
            assert!(
                error.contains(if scope {
                    "sem protocolo verificado"
                } else {
                    "coerção Ref"
                }),
                "{error}"
            );
            assert_eq!(format!("{m:?}"), before);
        }
    }
}
