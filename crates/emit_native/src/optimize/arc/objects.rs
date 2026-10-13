//! Materializa alocação Owned e inicialização de campos com layout explícito.
//! Não deduz layout de receivers, guardas late/tipo ou versões do heap.
//! Só roda na transação privada do módulo ARC. O emissor conserva o registro
//! de métodos da fábrica; construtores e inicializadores continuam separados.

use super::{DartFunctionPlan, edges::next_val};
use crate::hir::*;
use std::{borrow::Cow, collections::HashMap};

pub(super) fn typed_abi(params: &[Type], ret: Type) -> Option<i64> {
    let (Type::Ref, args) = params.split_first()? else { return None };
    if args.len() > 12 { return None; }
    args.iter().copied().chain(std::iter::once(ret)).try_fold(1_i64, |abi, ty|
        Some(abi * 5 + match ty { Type::Ref => 1, Type::I64 => 2,
            Type::F64 => 3, Type::I1 => 4, _ => return None }))
}

pub(super) fn prepare(
    f: &mut Function,
    layouts: &HashMap<u32, Vec<Type>>,
    plan: &DartFunctionPlan,
    signatures: &HashMap<String, (Vec<Type>, Type)>,
) -> Result<(), String> {
    let types: HashMap<_, _> = f
        .params
        .iter()
        .map(|(v, _, t)| (*v, *t))
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| b.instructions.iter().map(|(v, _, t)| (*v, *t))),
        )
        .collect();
    let mut next_item = next_val(f, plan);
    let mut id = || -> Result<ValueId, String> {
        let v = u32::try_from(next_item)
            .map_err(|_| format!("IDs SSA esgotados ao inicializar objetos em {}", f.symbol))?;
        next_item += 1;
        Ok(ValueId(v))
    };
    let typ = |op: &Operand| match op {
        Operand::Val(v) => types.get(v).copied(),
        Operand::Constant(Constant::Int(_)) => Some(Type::I64),
        Operand::Constant(Constant::Double(_)) => Some(Type::F64),
        Operand::Constant(Constant::Bool(_)) => Some(Type::I1),
        Operand::Constant(Constant::Null) => Some(Type::Ref),
        // Materialização de literais/endereço nativo precisa estar explícita
        // antes da alocação; não esconder safepoints em argumentos dos setters.
        _ => None,
    };
    for b in &mut f.blocks {
        let mut instructions = Vec::new();
        for (v, inst, ty) in &b.instructions {
            if let Instruction::CallRuntime { name, args, ret_ty } = inst
                && name == "dartforge_object_new"
            {
                let error = || {
                    format!(
                        "alocação zerada ARC em {} v{}: classe/quantidade/layout incompatível",
                        f.symbol, v.0
                    )
                };
                let [
                    (Operand::Constant(Constant::Int(class)), Type::I64),
                    (Operand::Constant(Constant::Int(field_list)), Type::I64),
                ] = args.as_slice()
                else {
                    return Err(error());
                };
                let class = u32::try_from(*class).map_err(|_| error())?;
                let field_list = u16::try_from(*field_list).map_err(|_| error())?;
                if *ty != Type::Ref
                    || *ret_ty != Type::Ref
                    || i32::try_from(class).is_err()
                    || layouts
                        .get(&class)
                        .is_some_and(|l| l.len() != usize::from(field_list))
                {
                    return Err(error());
                }
                // Instância ainda em construção: apenas a alocação ganha um
                // token. Guardas, RTI, inicializadores e construtor continuam
                // nas operações seguintes. Não certifica seus campos zerados.
                // A fábrica é a mesma alocar_instancia da operação original:
                // zero não contém referências. Layout ausente não impede o
                // token; leituras/escritas físicas continuam exigindo prova.
                instructions.push((
                    *v,
                    Instruction::CallRuntime {
                        name: "dartforge_arc_objeto_owned_v1".into(),
                        args: args.clone(),
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ));
                continue;
            }
            if let Instruction::CellGet { cell } = inst && *ty == Type::Ref {
                if typ(cell) != Some(Type::Ref) {
                    return Err(format!("célula ARC em {} v{} exige receiver Ref", f.symbol, v.0));
                }
                // O mapa físico deve marcar o slot como Ref antes de emprestar;
                // não converter a palavra de uma célula escalar em referência.
                instructions.push((*v, Instruction::CallRuntime {
                    name: "dartforge_arc_ler_campo_ref_v1".into(),
                    args: vec![(cell.clone(), Type::Ref), (Operand::Constant(Constant::Int(0)), Type::I64)],
                    ret_ty: Type::Ref,
                }, Type::Ref));
                continue;
            }
            // A leitura escalar da célula: os bits do campo 0 pela entrada
            // auditada (que recusa um campo Ref) e a volta à representação,
            // o inverso da escrita abaixo.
            if let Instruction::CellGet { cell } = inst
                && matches!(ty, Type::I64 | Type::F64 | Type::I1 | Type::I8)
            {
                if typ(cell) != Some(Type::Ref) {
                    return Err(format!("célula ARC em {} v{} exige receiver Ref", f.symbol, v.0));
                }
                let bits = if *ty == Type::I64 { *v } else { id()? };
                instructions.push((bits, Instruction::CallRuntime {
                    name: "dartforge_arc_ler_campo_escalar_v1".into(),
                    args: vec![(cell.clone(), Type::Ref), (Operand::Constant(Constant::Int(0)), Type::I64)],
                    ret_ty: Type::I64,
                }, Type::I64));
                match ty {
                    Type::F64 => instructions.push((*v, Instruction::Bitcast { op: Operand::Val(bits), to: Type::F64 }, Type::F64)),
                    Type::I1 | Type::I8 => instructions.push((*v, Instruction::Trunc { op: Operand::Val(bits), from: Type::I64, to: *ty }, *ty)),
                    _ => {}
                }
                continue;
            }
            // A escrita da célula (campo 0) pelas entradas auditadas de campo,
            // na representação do valor; o runtime confere a marca do mapa.
            if let Instruction::CellSet { cell, value } = inst {
                if typ(cell) != Some(Type::Ref) || *ty != Type::Void {
                    return Err(format!("célula ARC em {} v{} exige receiver Ref", f.symbol, v.0));
                }
                let (name_str, val_ref, repr) = match typ(value) {
                    Some(Type::Ref) => ("dartforge_arc_gravar_campo_ref_v1", value.clone(), Type::Ref),
                    Some(Type::I64) => ("dartforge_arc_gravar_campo_escalar_v1", value.clone(), Type::I64),
                    Some(t @ (Type::F64 | Type::I1 | Type::I8)) => {
                        let bits = id()?;
                        let conversion = if t == Type::F64 {
                            Instruction::Bitcast { op: value.clone(), to: Type::I64 }
                        } else {
                            Instruction::ZExt { op: value.clone(), from: t, to: Type::I64 }
                        };
                        instructions.push((bits, conversion, Type::I64));
                        ("dartforge_arc_gravar_campo_escalar_v1", Operand::Val(bits), Type::I64)
                    }
                    _ => return Err(format!("célula ARC em {} v{}: valor sem representação", f.symbol, v.0)),
                };
                instructions.push((*v, Instruction::CallRuntime {
                    name: name_str.into(),
                    args: vec![(cell.clone(), Type::Ref), (Operand::Constant(Constant::Int(0)), Type::I64), (val_ref, repr)],
                    ret_ty: Type::Void,
                }, Type::Void));
                continue;
            }
            if let Instruction::EnvGet { env, index } = inst
                && *ty == Type::Ref
            {
                if typ(env) != Some(Type::Ref) {
                    return Err(format!("captura ARC em {} v{} exige ambiente Ref", f.symbol, v.0));
                }
                let idx_value = i64::try_from(*index).map_err(|_|
                    format!("captura ARC em {} v{}: índice fora da ABI", f.symbol, v.0))?;
                instructions.push((*v, Instruction::CallRuntime {
                    name: "dartforge_arc_env_ref_v1".into(),
                    args: vec![(env.clone(), Type::Ref),
                        (Operand::Constant(Constant::Int(idx_value)), Type::I64)],
                    ret_ty: Type::Ref,
                }, Type::Ref));
                continue;
            }
            // A captura escalar: os bits pela entrada auditada (que recusa
            // uma captura Ref) e a volta à representação gravada.
            if let Instruction::EnvGet { env, index } = inst
                && matches!(ty, Type::I64 | Type::F64 | Type::I1 | Type::I8)
            {
                if typ(env) != Some(Type::Ref) {
                    return Err(format!("captura ARC em {} v{} exige ambiente Ref", f.symbol, v.0));
                }
                let idx_value = i64::try_from(*index).map_err(|_|
                    format!("captura ARC em {} v{}: índice fora da ABI", f.symbol, v.0))?;
                let bits = if *ty == Type::I64 { *v } else { id()? };
                instructions.push((bits, Instruction::CallRuntime {
                    name: "dartforge_arc_env_escalar_v1".into(),
                    args: vec![(env.clone(), Type::Ref), (Operand::Constant(Constant::Int(idx_value)), Type::I64)],
                    ret_ty: Type::I64,
                }, Type::I64));
                match ty {
                    Type::F64 => instructions.push((*v, Instruction::Bitcast { op: Operand::Val(bits), to: Type::F64 }, Type::F64)),
                    Type::I1 | Type::I8 => instructions.push((*v, Instruction::Trunc { op: Operand::Val(bits), from: Type::I64, to: *ty }, *ty)),
                    _ => {}
                }
                continue;
            }
            let closure = match inst {
                Instruction::AllocClosure { code_symbol, env } =>
                    Some((code_symbol, env, None, 0)),
                Instruction::AllocClosureTyped { code_symbol, env, typed, abi, .. } =>
                    Some((code_symbol, env, Some(typed), *abi)),
                _ => None,
            };
            if let Some((code, env, typed, abi)) = closure {
                let uniform = signatures.get(code).is_some_and(|(ps, ret)|
                    ps == &[Type::Ref, Type::Ptr, Type::Ptr] && *ret == Type::Ref);
                let body = typed.is_none_or(|t| signatures.get(t).is_some_and(|(ps, ret)|
                    abi == 0 || typed_abi(ps, *ret) == Some(abi)));
                if *ty != Type::Ref || typ(env) != Some(Type::Ref)
                    || !uniform || !body {
                    return Err(format!("closure ARC em {} v{}: ABI, contexto ou corpo não coberto", f.symbol, v.0));
                }
                instructions.push((*v, Instruction::CallRuntime {
                    name: "dartforge_arc_closure_owned_v1".into(),
                    args: vec![
                        (Operand::Constant(Constant::Function(code.clone())), Type::I64),
                        (env.clone(), Type::Ref),
                        (typed.map_or(Operand::Constant(Constant::Int(0)), |t|
                            Operand::Constant(Constant::Function(t.clone()))), Type::I64),
                        (Operand::Constant(Constant::Int(abi)), Type::I64),
                    ], ret_ty: Type::Ref,
                }, Type::Ref));
                continue;
            }
            // Capturas e quadros suspensos têm mapas por instância, não um
            // layout nominal único por cid. Só a construção inicial é
            // conferida aqui; leituras, escritas e suspensão têm seus contratos.
            let (class_id, fields, synthetic): (u32, &[Operand], bool) = match inst {
                Instruction::AllocObject { class_id, fields } => (*class_id, fields,
                    *class_id == crate::lower::async_sm::ID_QUADRO_ASYNC && !layouts.contains_key(class_id)),
                Instruction::AllocCell { value } => (dartforge_runtime::layout::cid::CELULA as u32,
                    std::slice::from_ref(value), true),
                Instruction::AllocEnv { values } => (dartforge_runtime::layout::cid::CONTEXTO as u32,
                    values, true),
                _ => { instructions.push((*v, inst.clone(), *ty)); continue; }
            };
            if synthetic && fields.is_empty() && class_id != crate::lower::async_sm::ID_QUADRO_ASYNC {
                if *ty != Type::Ref {
                    return Err(format!("ambiente ARC em {} v{} exige Ref", f.symbol, v.0));
                }
                instructions.push((*v, Instruction::Const(Constant::Null), Type::Ref));
                continue;
            }
            let layout = if synthetic {
                let types = fields.iter().map(&typ).collect::<Option<Vec<_>>>().ok_or_else(||
                    format!("captura ARC em {} v{}: materialização implícita não coberta", f.symbol, v.0))?;
                if types.iter().any(|t| !matches!(t, Type::Ref | Type::I64 | Type::F64 | Type::I1 | Type::I8)) {
                    return Err(format!("captura ARC em {} v{}: representação não coberta", f.symbol, v.0));
                }
                if class_id == crate::lower::async_sm::ID_QUADRO_ASYNC
                    && !types.starts_with(&[Type::I64, Type::Ref, Type::Ref, Type::Ref, Type::Ref, Type::I64]) {
                    return Err(format!("quadro suspenso ARC em {} v{}: cabeçalho incompatível", f.symbol, v.0));
                }
                Cow::Owned(types)
            } else { Cow::Borrowed(layouts.get(&class_id).ok_or_else(|| {
                format!(
                    "alocação ARC em {} v{}: classe {class_id} sem layout",
                    f.symbol, v.0
                )
            })?) };
            if *ty != Type::Ref
                || i32::try_from(class_id).is_err()
                || u16::try_from(fields.len()).is_err()
                || layout.len() != fields.len()
            {
                return Err(format!(
                    "alocação ARC em {} v{}: representação ou quantidade incompatível",
                    f.symbol, v.0
                ));
            }
            for (i, (op, expected)) in fields.iter().zip(layout.iter()).enumerate() {
                if typ(op) != Some(*expected) {
                    return Err(format!(
                        "alocação ARC em {} v{}: campo {i} incompatível com {expected:?}",
                        f.symbol, v.0
                    ));
                }
            }
            instructions.push((
                *v,
                Instruction::CallRuntime {
                    name: "dartforge_arc_objeto_owned_v1".into(),
                    args: vec![
                        (
                            Operand::Constant(Constant::Int(i64::from(class_id))),
                            Type::I64,
                        ),
                        (
                            Operand::Constant(Constant::Int(fields.len() as i64)),
                            Type::I64,
                        ),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ));
            for (i, (op, repr)) in fields.iter().zip(layout.iter()).enumerate() {
                let (val_ref, argument, name_str) = if *repr == Type::Ref {
                    (op.clone(), Type::Ref, "dartforge_arc_gravar_campo_ref_v1")
                } else {
                    let bits = if *repr == Type::I64 {
                        op.clone()
                    } else {
                        let bits = id()?;
                        let conversion = if *repr == Type::F64 {
                            Instruction::Bitcast {
                                op: op.clone(),
                                to: Type::I64,
                            }
                        } else {
                            Instruction::ZExt {
                                op: op.clone(),
                                from: *repr,
                                to: Type::I64,
                            }
                        };
                        instructions.push((bits, conversion, Type::I64));
                        Operand::Val(bits)
                    };
                    (bits, Type::I64, "dartforge_arc_gravar_campo_escalar_v1")
                };
                instructions.push((
                    id()?,
                    Instruction::CallRuntime {
                        name: name_str.into(),
                        args: vec![
                            (Operand::Val(*v), Type::Ref),
                            (Operand::Constant(Constant::Int(i as i64)), Type::I64),
                            (val_ref, argument),
                        ],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ));
            }
        }
        b.instructions = instructions;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::optimize::arc::*;

    #[test]
    fn suspended_frames_initialize_map_per_instance_without_nominal_layout() {
        for case in 0..9 {
            let (mut m, mut plans) = module();
            m.classes.clear(); m.arc_field_layouts.clear();
            let mut field_list = vec![Operand::Constant(Constant::Int(0)),
                Operand::Constant(Constant::Null), Operand::Constant(Constant::Null),
                Operand::Val(ValueId(0)), Operand::Constant(Constant::Null),
                Operand::Constant(Constant::Int(0)), Operand::Constant(Constant::Double(-0.0)),
                Operand::Constant(Constant::Bool(true)), Operand::Val(ValueId(1))];
            match case {
                1 => field_list[0] = Operand::Constant(Constant::Null),
                2 => field_list[1] = Operand::Constant(Constant::Int(0)),
                3 => field_list.truncate(5),
                4 => field_list.push(Operand::Constant(Constant::String("implícita".into()))),
                5 => m.functions[0].params[1].2 = Type::Ptr,
                8 => field_list.clear(),
                _ => {}
            }
            let cid = if case == 6 { 124 } else { crate::lower::async_sm::ID_QUADRO_ASYNC };
            m.functions[0].blocks[0].instructions[0].1 = Instruction::AllocObject { class_id: cid, fields: field_list };
            if case == 7 { m.arc_memory = false; }
            if case == 0 {
                let mut other = m.functions[0].clone();
                other.symbol = "outro_quadro".into();
                if let Instruction::AllocObject { fields, .. } = &mut other.blocks[0].instructions[0].1 {
                    fields.resize(40, Operand::Val(ValueId(0)));
                }
                m.functions.push(other);
                plans.insert("outro_quadro".into(), plans["objeto"].clone());
            }
            let before = format!("{m:?}/{plans:?}");
            let result = prepare_arc_dart_module(&mut m, &mut plans);
            if case == 0 {
                result.unwrap();
                assert!(!m.arc_field_layouts.contains_key(&cid));
                for f in &m.functions {
                    assert_eq!(plans[&f.symbol].classes[&ValueId(2)], Ownership::Owned);
                    assert!(f.blocks[0].instructions.iter().any(|(_, i, _)| matches!(i,
                        Instruction::CallRuntime { name, args, .. } if name == "dartforge_arc_gravar_campo_ref_v1"
                            && args[1].0 == Operand::Constant(Constant::Int(3)))));
                }
                crate::llvm::LlvmEmitter::new(&m).emit_all();
                let prepared = format!("{m:?}/{plans:?}");
                assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
                assert_eq!(prepared, format!("{m:?}/{plans:?}"));
            } else {
                assert_eq!(result.is_ok(), case == 7, "{result:?}");
                assert_eq!(before, format!("{m:?}/{plans:?}"));
            }
        }
    }

    #[test]
    fn owned_captures_keep_map_code_and_cleanup_without_assuming_cid_layout() {
        for case in 0..14 {
            let mut m = Module::new();
            m.arc_memory = case != 7;
            m.sdk_mode = true;
            m.sdk_library = true;
            let mut values = vec![Operand::Val(ValueId(5)), Operand::Val(ValueId(1)),
                Operand::Val(ValueId(2)), Operand::Val(ValueId(3)), Operand::Val(ValueId(4))];
            values.resize(40, Operand::Val(ValueId(0)));
            if case == 2 { values.clear(); }
            if case == 5 { values[0] = Operand::Constant(Constant::String("implícita".into())); }
            if case == 10 { values[0] = Operand::Val(ValueId(8)); }
            let closure = if case == 1 {
                Instruction::AllocClosure { code_symbol: "entrada".into(), env: Operand::Val(ValueId(6)) }
            } else { Instruction::AllocClosureTyped {
                code_symbol: if case == 4 { "externo" } else { "entrada" }.into(),
                env: Operand::Val(ValueId(if case == 3 { 1 } else { 6 })),
                typed: "corpo".into(), abi: 7, direct: false,
            } };
            m.functions = vec![
                Function { symbol: "capturar".into(), name: "capturar".into(), debug_info: None,
                    params: vec![(ValueId(0), "ref".into(), Type::Ref),
                        (ValueId(1), "int".into(), Type::I64), (ValueId(2), "double".into(), Type::F64),
                        (ValueId(3), "bool".into(), Type::I1), (ValueId(4), "byte".into(), Type::I8),
                        (ValueId(8), "ptr".into(), Type::Ptr)], return_ty: Type::Ref,
                    blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                        (ValueId(5), Instruction::AllocCell { value: Operand::Val(ValueId(0)) },
                            if case == 8 { Type::I64 } else { Type::Ref }),
                        (ValueId(6), Instruction::AllocEnv { values }, Type::Ref),
                        (ValueId(7), closure, Type::Ref),
                    ], terminator: Terminator::Return(Some(Operand::Val(ValueId(7)))) }] },
                Function { symbol: "entrada".into(), name: "entrada".into(), debug_info: None,
                    params: vec![(ValueId(0), "closure".into(), Type::Ref),
                        (ValueId(1), "args".into(), Type::Ptr), (ValueId(2), "desc".into(), Type::Ptr)],
                    return_ty: Type::Ref, blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
                        terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) }] },
                Function { symbol: "corpo".into(), name: "corpo".into(), debug_info: None,
                    params: vec![(ValueId(0), "env".into(), Type::Ref)], return_ty: Type::I64,
                    blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
                        terminator: Terminator::Return(Some(Operand::Constant(Constant::Int(7)))) }] },
            ];
            if case == 11 { m.functions[1].params[1].2 = Type::I64; }
            if case == 12 {
                m.functions[2].return_ty = Type::F64;
                m.functions[2].blocks[0].terminator = Terminator::Return(Some(
                    Operand::Constant(Constant::Double(7.0))));
            }
            if case == 13 {
                if let Instruction::AllocClosureTyped { abi, .. } = &mut m.functions[0].blocks[0].instructions[2].1 {
                    *abi = 8;
                }
            }
            if case == 9 { m.functions.reverse(); }
            m.dart_ref_returns.extend(["capturar".into(), "entrada".into()]);
            if case != 6 { m.dart_scalar_parameters.insert("capturar".into(), HashSet::from([ValueId(1)])); }
            let mut plans: HashMap<_, _> = m.functions.iter()
                .map(|f| (f.symbol.clone(), DartFunctionPlan::default())).collect();
            // Proveniência explícita dos vetores da ABI uniforme, não da largura.
            plans.get_mut("entrada").unwrap().classes.extend([
                (ValueId(1), Ownership::Trivial), (ValueId(2), Ownership::Trivial)]);
            plans.get_mut("capturar").unwrap().classes.insert(ValueId(8), Ownership::Trivial);
            let before = format!("{m:?}/{plans:?}");
            let r = prepare_arc_dart_module(&mut m, &mut plans);
            if [3, 4, 5, 6, 8, 10, 11, 12, 13].contains(&case) {
                assert!(r.is_err(), "caso {case}");
                assert_eq!(format!("{m:?}/{plans:?}"), before);
                continue;
            }
            if case == 7 {
                assert_eq!(r.unwrap(), (0, 0));
                assert_eq!(format!("{m:?}/{plans:?}"), before);
                continue;
            }
            r.unwrap();
            let f = m.functions.iter().find(|f| f.symbol == "capturar").unwrap();
            let p = &plans["capturar"];
            assert_eq!(p.classes[&ValueId(5)], Ownership::Owned);
            assert_eq!(p.classes[&ValueId(7)], Ownership::Owned);
            assert_eq!(p.classes[&ValueId(6)], if case == 2 { Ownership::Trivial } else { Ownership::Owned });
            assert!(f.blocks.iter().flat_map(|b| &b.instructions).any(|(v, i, _)| *v == ValueId(7)
                && matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_arc_closure_owned_v1")));
            if case != 2 {
                assert!(f.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i,
                    Instruction::CallRuntime { name, args, .. }
                    if name == "dartforge_arc_gravar_campo_ref_v1"
                        && args[1].0 == Operand::Constant(Constant::Int(39)))));
            }
            let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
            assert!(ir.contains("@dartforge_arc_closure_owned_v1(i64 ptrtoint (ptr @entrada to i64)"));
            assert!(!ir.contains("call void @dartforge_arc_release(i64 %v7)"));
            let body_before = format!("{m:?}");
            let classes_before = p.classes.clone();
            assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
            assert_eq!(format!("{m:?}"), body_before);
            assert_eq!(plans["capturar"].classes, classes_before);
        }
    }

    #[test]
    fn table_pointer_without_provenance_gets_no_runtime_contract() {
        let (mut m, _) = module();
        m.functions[0].params.clear();
        m.functions[0].blocks[0].instructions = vec![(
            ValueId(2),
            Instruction::CallRuntime {
                name: "dartforge_arc_objeto_owned_t_v1".into(),
                args: vec![
                    (Operand::Constant(Constant::Int(123)), Type::I64),
                    (Operand::Constant(Constant::Int(5)), Type::I64),
                    (
                        Operand::Constant(Constant::Function("arbitraria".into())),
                        Type::Ptr,
                    ),
                ],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        )];
        let mut classes = HashMap::new();
        let mut tokens = TokenPlan::default();
        let before = format!("{classes:?}{tokens:?}");
        let error =
            produce_runtime_contracts(&m.functions[0], &mut classes, &mut tokens).unwrap_err();
        assert!(
            error.contains("tabela estática exige proveniência"),
            "{error}"
        );
        assert_eq!(format!("{classes:?}{tokens:?}"), before);
    }

    #[test]
    fn zeroed_instance_preserves_constructor_and_rejects_invalid_metadata_atomically() {
        for case in 0..9 {
            let (mut m, mut plans) = module();
            m.functions[0].params.clear();
            m.functions[0].blocks[0].instructions = vec![
                (
                    ValueId(2),
                    Instruction::CallRuntime {
                        name: "dartforge_object_new".into(),
                        args: vec![
                            (Operand::Constant(Constant::Int(123)), Type::I64),
                            (Operand::Constant(Constant::Int(5)), Type::I64),
                        ],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(3),
                    Instruction::CallStatic {
                        symbol: "construtor".into(),
                        args: vec![Operand::Val(ValueId(2))],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
            ];
            m.functions.push(Function {
                symbol: "construtor".into(),
                name: "construtor".into(),
                debug_info: None,
                params: vec![(ValueId(0), "this".into(), Type::Ref)],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                }],
            });
            plans.insert("construtor".into(), DartFunctionPlan::default());
            let Instruction::CallRuntime { args, ret_ty, .. } =
                &mut m.functions[0].blocks[0].instructions[0].1
            else {
                unreachable!()
            };
            match case {
                1 => args[1].0 = Operand::Constant(Constant::Int(4)),
                2 => args[0].0 = Operand::Constant(Constant::Int(i64::from(i32::MAX) + 1)),
                3 => args[1].1 = Type::Ref,
                4 => *ret_ty = Type::I64,
                5 => args[1].0 = Operand::Constant(Constant::Int(-1)),
                6 => {
                    m.arc_memory = false;
                }
                7 => m.arc_field_layouts.clear(),
                8 => {
                    m.arc_field_layouts.clear();
                    m.functions[0].blocks[0].instructions.push((
                        ValueId(4),
                        Instruction::GetField {
                            object: Operand::Val(ValueId(2)),
                            index: 0,
                        },
                        Type::Ref,
                    ));
                }
                _ => {}
            }
            let before = format!("{m:?}{plans:?}");
            let result = prepare_arc_dart_module(&mut m, &mut plans);
            if (1..6).contains(&case) || case == 8 {
                assert!(result.is_err(), "caso {case}");
                assert_eq!(format!("{m:?}{plans:?}"), before);
            } else if case == 6 {
                assert_eq!(result.unwrap(), (0, 0));
                assert_eq!(format!("{m:?}{plans:?}"), before);
            } else {
                result.unwrap();
                assert_eq!(plans["objeto"].classes[&ValueId(2)], Ownership::Owned);
                let ops: Vec<_> = m.functions[0]
                    .blocks
                    .iter()
                    .flat_map(|b| &b.instructions)
                    .collect();
                assert!(ops.iter().any(|(_, i, _)| matches!(i,
                    Instruction::CallRuntime { name, .. } if name == "dartforge_arc_objeto_owned_v1")));
                assert!(ops.iter().any(|(_, i, _)| matches!(i,
                    Instruction::CallStatic { symbol, args, .. }
                    if symbol == "construtor" && args == &vec![Operand::Val(ValueId(2))])));
                assert!(!ops.iter().any(|(_, i, _)| matches!(i,
                    Instruction::CallRuntime { name, .. } if name.starts_with("dartforge_arc_gravar_campo"))));
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    (0, 0)
                );
            }
        }
    }

    fn module() -> (Module, HashMap<String, DartFunctionPlan>) {
        let mut m = Module::new();
        m.arc_memory = true;
        m.classes.push(ClassDef {
            id: 123,
            name: "C".into(),
            field_count: 5,
            vtable: vec![],
            to_string_symbol: None,
        });
        m.arc_field_layouts.insert(
            123,
            vec![Type::Ref, Type::I64, Type::F64, Type::I1, Type::I8],
        );
        m.functions.push(Function {
            symbol: "objeto".into(),
            name: "objeto".into(),
            debug_info: None,
            params: vec![
                (ValueId(0), "filho".into(), Type::Ref),
                (ValueId(1), "byte".into(), Type::I8),
            ],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(2),
                    Instruction::AllocObject {
                        class_id: 123,
                        fields: vec![
                            Operand::Val(ValueId(0)),
                            Operand::Constant(Constant::Int(i64::MAX)),
                            Operand::Constant(Constant::Double(-0.0)),
                            Operand::Constant(Constant::Bool(true)),
                            Operand::Val(ValueId(1)),
                        ],
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))),
            }],
        });
        let mut p = DartFunctionPlan::default();
        p.tokens.ret = TokenReturn::Owned;
        (m, HashMap::from([("objeto".into(), p)]))
    }

    #[test]
    fn allocation_initializes_ref_and_bits_without_numeric_double_conversion() {
        let (mut m, mut plans) = module();
        assert_eq!(
            prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
            (0, 0)
        );
        let ops = &m.functions[0].blocks[0].instructions;
        assert!(matches!(&ops[0].1, Instruction::CallRuntime { name, .. }
            if name == "dartforge_arc_objeto_owned_v1"));
        assert_eq!(plans["objeto"].classes[&ValueId(2)], Ownership::Owned);
        assert_eq!(
            ops.iter()
                .filter(|(_, i, _)| matches!(i,
            Instruction::CallRuntime { name, .. } if name == "dartforge_arc_gravar_campo_ref_v1"))
                .count(),
            1
        );
        assert_eq!(ops.iter().filter(|(_, i, _)| matches!(i,
            Instruction::CallRuntime { name, .. } if name == "dartforge_arc_gravar_campo_escalar_v1")).count(), 4);
        assert!(ops.iter().any(|(_, i, _)| matches!(i, Instruction::Bitcast {
            op: Operand::Constant(Constant::Double(n)), to: Type::I64 } if n.to_bits() == (-0.0f64).to_bits())));
        let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
        assert!(ir.contains("bitcast double"));
        assert!(!ir.contains("call i64 @dartforge_object_new("));
        let before = format!("{m:?}/{plans:?}");
        assert_eq!(
            prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{m:?}/{plans:?}"), before);
    }

    #[test]
    fn invalid_allocation_or_later_failure_does_not_publish_transformation() {
        for case in 0..6 {
            let (mut m, mut plans) = module();
            match case {
                0 => m.arc_field_layouts.clear(),
                1 => {
                    let Instruction::AllocObject { fields, .. } =
                        &mut m.functions[0].blocks[0].instructions[0].1
                    else {
                        unreachable!()
                    };
                    fields[0] = Operand::Constant(Constant::Int(85));
                }
                2 => {
                    let Instruction::AllocObject { fields, .. } =
                        &mut m.functions[0].blocks[0].instructions[0].1
                    else {
                        unreachable!()
                    };
                    fields.pop();
                }
                3 => {
                    plans
                        .get_mut("objeto")
                        .unwrap()
                        .classes
                        .insert(ValueId(u32::MAX), Ownership::Trivial);
                }
                4 => {
                    m.functions[0].blocks[0].instructions.push((
                        ValueId(99),
                        Instruction::CallRuntime {
                            name: "extern_sem_contrato".into(),
                            args: vec![],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ));
                }
                _ => {
                    let mut second = m.functions[0].clone();
                    second.symbol = "segunda".into();
                    second.params[0].2 = Type::I64;
                    m.functions.push(second);
                    plans.insert("segunda".into(), DartFunctionPlan::default());
                }
            }
            let before = format!("{m:?}/{plans:?}");
            assert!(
                prepare_arc_dart_module(&mut m, &mut plans).is_err(),
                "caso {case}"
            );
            assert_eq!(format!("{m:?}/{plans:?}"), before, "caso {case}");
            m.arc_memory = false;
            let before = format!("{m:?}/{plans:?}");
            assert_eq!(
                prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                (0, 0)
            );
            assert_eq!(format!("{m:?}/{plans:?}"), before);
        }
    }

    #[test]
    fn owned_child_and_object_have_cleanup_after_initialization() {
        for quantity in [0, 1, 40] {
            let (mut m, mut plans) = module();
            m.classes[0].field_count = quantity;
            let mut layout = vec![Type::I64; quantity];
            let mut fields = vec![Operand::Constant(Constant::Int(85)); quantity];
            if quantity != 0 {
                layout[quantity - 1] = Type::Ref;
                fields[quantity - 1] = Operand::Val(ValueId(0));
            }
            m.arc_field_layouts.insert(123, layout);
            let f = &mut m.functions[0];
            f.params.clear();
            f.return_ty = Type::Void;
            f.blocks[0].instructions = vec![
                (
                    ValueId(0),
                    Instruction::Box {
                        op: Operand::Constant(Constant::Int(i64::MAX)),
                        from: Type::I64,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(1),
                    Instruction::AllocObject {
                        class_id: 123,
                        fields,
                    },
                    Type::Ref,
                ),
            ];
            f.blocks[0].terminator = Terminator::Return(None);
            plans.get_mut("objeto").unwrap().tokens.ret = TokenReturn::Trivial;
            assert_eq!(
                prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                (0, 2)
            );
            let ops = &m.functions[0].blocks[0].instructions;
            assert_eq!(
                ops.iter()
                    .filter(|(_, i, _)| matches!(i, Instruction::ArcDrop { .. }))
                    .count(),
                2
            );
            assert!(
                ops[..ops.len() - 2]
                    .iter()
                    .all(|(_, i, _)| !matches!(i, Instruction::ArcDrop { .. }))
            );
            if quantity != 0 {
                assert!(
                    matches!(&ops[ops.len() - 3].1, Instruction::CallRuntime { name, args, .. }
                    if name == "dartforge_arc_gravar_campo_ref_v1"
                    && args[1].0 == Operand::Constant(Constant::Int((quantity - 1) as i64))
                    && args[2] == (Operand::Val(ValueId(0)), Type::Ref))
                );
            }
            crate::llvm::LlvmEmitter::new(&m).emit_all();
        }
    }

    #[test]
    fn cell_ref_read_promotes_return_and_rejects_scalar_receiver() {
        for case in 0..3 {
            let (mut m, mut plans) = module();
            m.arc_memory = case != 1;
            let f = &mut m.functions[0];
            f.params.truncate(1);
            if case == 2 { f.params[0].2 = Type::I64; }
            f.blocks[0].instructions = vec![(ValueId(1), Instruction::CellGet { cell: Operand::Val(ValueId(0)) }, Type::Ref)];
            f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
            let before = format!("{m:?}"); let ps_antes = plans.clone();
            let result = prepare_arc_dart_module(&mut m, &mut plans);
            if case == 2 { assert!(result.unwrap_err().contains("receiver Ref")); assert_eq!(format!("{m:?}"), before); assert_eq!(plans, ps_antes); continue; }
            if case == 1 { assert_eq!(result.unwrap(), (0, 0)); assert_eq!(format!("{m:?}"), before); continue; }
            assert_eq!(result.unwrap(), (1, 0));
            let Terminator::Return(Some(Operand::Val(ret))) = m.functions[0].blocks[0].terminator else { panic!("retorno sem token SSA"); };
            assert_eq!(plans["objeto"].classes[&ret], Ownership::Owned);
            assert!(matches!(plans["objeto"].classes[&ValueId(1)], Ownership::Borrowed { owner: OwnerOrigin::Value(ValueId(0)), .. }));
            let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
            assert!(ir.contains("@dartforge_arc_ler_campo_ref_v1(i64 %v0, i64 0)"));
            assert!(ir.contains("call void @dartforge_arc_retain("));
        }
    }

    #[test]
    fn scalar_cell_read_reads_audited_bits_and_returns_to_representation() {
        for ty in [Type::I64, Type::F64, Type::I1] {
            let (mut m, mut plans) = module();
            let f = &mut m.functions[0];
            f.params.truncate(1);
            f.return_ty = ty;
            f.blocks[0].instructions = vec![(ValueId(1), Instruction::CellGet { cell: Operand::Val(ValueId(0)) }, ty)];
            f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
            plans.get_mut("objeto").unwrap().tokens.ret = TokenReturn::Trivial;
            prepare_arc_dart_module(&mut m, &mut plans).unwrap();
            let ops = &m.functions[0].blocks[0].instructions;
            assert!(matches!(&ops[0].1, Instruction::CallRuntime { name, ret_ty: Type::I64, .. }
                if name == "dartforge_arc_ler_campo_escalar_v1"));
            assert_eq!(ops.last().unwrap().0, ValueId(1));
            assert_eq!(ops.last().unwrap().2, ty);
            assert_eq!(plans["objeto"].classes[&ValueId(1)], Ownership::Trivial);
            let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
            assert!(ir.contains("@dartforge_arc_ler_campo_escalar_v1(i64 %v0, i64 0)"), "{ir}");
        }
    }

    #[test]
    fn scalar_capture_reads_audited_bits_and_returns_to_representation() {
        for ty in [Type::I64, Type::F64, Type::I8] {
            let (mut m, mut plans) = module();
            let f = &mut m.functions[0];
            f.params.truncate(1);
            f.return_ty = ty;
            f.blocks[0].instructions = vec![(ValueId(1), Instruction::EnvGet { env: Operand::Val(ValueId(0)), index: 2 }, ty)];
            f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
            plans.get_mut("objeto").unwrap().tokens.ret = TokenReturn::Trivial;
            prepare_arc_dart_module(&mut m, &mut plans).unwrap();
            let ops = &m.functions[0].blocks[0].instructions;
            assert!(matches!(&ops[0].1, Instruction::CallRuntime { name, ret_ty: Type::I64, .. }
                if name == "dartforge_arc_env_escalar_v1"));
            assert_eq!((ops.last().unwrap().0, ops.last().unwrap().2), (ValueId(1), ty));
            assert_eq!(plans["objeto"].classes[&ValueId(1)], Ownership::Trivial);
            let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
            assert!(ir.contains("@dartforge_arc_env_escalar_v1(i64 %v0, i64 2)"), "{ir}");
        }
    }

    #[test]
    fn ref_capture_gets_owner_before_collection_and_tracing_keeps_read() {
        for arc in [false, true] {
            let (mut m, mut plans) = module();
            m.arc_memory = arc;
            let f = &mut m.functions[0];
            f.params.truncate(1);
            f.blocks[0].instructions = vec![
                (ValueId(1), Instruction::EnvGet {
                    env: Operand::Val(ValueId(0)), index: 39,
                }, Type::Ref),
                (ValueId(2), Instruction::AllocEnv {
                    values: vec![Operand::Val(ValueId(1))],
                }, Type::Ref),
            ];
            let before = format!("{m:?}");
            let result = prepare_arc_dart_module(&mut m, &mut plans).unwrap();
            if !arc {
                assert_eq!(result, (0, 0));
                assert_eq!(format!("{m:?}"), before);
                continue;
            }
            assert_eq!(result, (1, 1));
            let ops = &m.functions[0].blocks[0].instructions;
            assert!(matches!(&ops[0].1, Instruction::CallRuntime { name, args, .. }
                if name == "dartforge_arc_env_ref_v1" && args[1].0 == Operand::Constant(Constant::Int(39))));
            assert_eq!(ops[1].0, ValueId(1));
            assert!(matches!(ops[1].1, Instruction::ArcCopy { .. }));
            assert!(matches!(&ops[2].1, Instruction::CallRuntime { name, .. }
                if name == "dartforge_arc_objeto_owned_v1"));
            assert_eq!(plans["objeto"].classes[&ValueId(1)], Ownership::Owned);
            let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
            assert!(ir.contains("@dartforge_arc_env_ref_v1(i64 %v0, i64 39)"));
        }
    }

    #[test]
    fn borrowed_field_gets_owner_before_allocation_safepoint() {
        let (mut m, mut plans) = module();
        m.classes[0].field_count = 1;
        m.arc_field_layouts.insert(123, vec![Type::Ref]);
        let f = &mut m.functions[0];
        f.params.truncate(1);
        f.blocks[0].instructions = vec![
            (
                ValueId(1),
                Instruction::CallRuntime {
                    name: "dartforge_arc_ler_campo_ref_v1".into(),
                    args: vec![
                        (Operand::Val(ValueId(0)), Type::Ref),
                        (Operand::Constant(Constant::Int(0)), Type::I64),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ),
            (
                ValueId(2),
                Instruction::AllocObject {
                    class_id: 123,
                    fields: vec![Operand::Val(ValueId(1))],
                },
                Type::Ref,
            ),
        ];
        assert_eq!(
            prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
            (1, 1)
        );
        let ops = &m.functions[0].blocks[0].instructions;
        assert_eq!(ops[1].0, ValueId(1));
        assert!(matches!(ops[1].1, Instruction::ArcCopy { .. }));
        assert!(matches!(&ops[2].1, Instruction::CallRuntime { name, .. }
            if name == "dartforge_arc_objeto_owned_v1"));
        assert!(matches!(
            ops.last().unwrap().1,
            Instruction::ArcDrop {
                value: Operand::Val(ValueId(1))
            }
        ));
        assert_eq!(plans["objeto"].classes[&ValueId(1)], Ownership::Owned);
        crate::llvm::LlvmEmitter::new(&m).emit_all();
    }
}
