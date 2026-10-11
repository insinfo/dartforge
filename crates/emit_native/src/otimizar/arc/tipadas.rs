//! Proveniência do PC tipado e contexto da mesma closure, sem coerção de words.

use super::*;

const ALVO: &str = "dartforge_arc_alvo_tipada_v1";
const CONTEXTO: &str = "dartforge_arc_contexto_closure_v1";

fn definicao<'a>(f: &'a Function, op: &Operand) -> Option<&'a Instruction> {
    let Operand::Val(v) = op else { return None };
    f.blocks.iter().flat_map(|b| &b.instructions).find(|(id, _, _)| id == v).map(|(_, i, _)| i)
}

fn contexto(f: &Function, op: &Operand) -> Option<Operand> {
    let mut op = op.clone();
    let mut vistos = HashSet::new();
    loop {
        let Operand::Val(v) = &op else { return None };
        if !vistos.insert(*v) { return None; }
        match definicao(f, &op)? {
            Instruction::ArcCopy { value } | Instruction::ArcMove { value } => op = value.clone(),
            Instruction::CallRuntime { name, args, ret_ty: Type::Ref } if name == CONTEXTO => {
                let [(closure, Type::Ref)] = args.as_slice() else { return None };
                return Some(closure.clone());
            }
            _ => return None,
        }
    }
}

fn assinatura(args: &[(Operand, Type)], ret: Type) -> Option<i64> {
    if !matches!(ret, Type::Ref | Type::I64 | Type::F64 | Type::I1) { return None; }
    super::objetos::abi_tipada(&args.iter().map(|(_, t)| *t).collect::<Vec<_>>(), ret)
}

pub(super) fn conferir(f: &Function, inst: &Instruction, ty: Type) -> Result<bool, String> {
    let Instruction::ChamadaTipada { alvo, args, ret } = inst else { return Ok(false) };
    let erro = || format!("chamada tipada ARC em {}: PC, contexto, assinatura ou retorno sem proveniência verificada", f.symbol);
    let abi = assinatura(args, *ret).ok_or_else(erro)?;
    if ty != *ret { return Err(erro()); }
    let Some(Instruction::CallRuntime { name, args: origem, ret_ty: Type::I64 }) = definicao(f, alvo)
        else { return Err(erro()) };
    let [(closure, Type::Ref), (Operand::Constant(Constant::Int(codigo)), Type::I64)] = origem.as_slice()
        else { return Err(erro()) };
    if name != ALVO || *codigo != abi || contexto(f, &args[0].0).as_ref() != Some(closure) {
        return Err(erro());
    }
    let tipos: HashMap<_, _> = f.params.iter().map(|(v, _, t)| (*v, *t))
        .chain(f.blocks.iter().flat_map(|b| &b.instructions).map(|(v, _, t)| (*v, *t))).collect();
    for (op, t) in args {
        let compativel = match op {
            Operand::Val(v) => tipos.get(v) == Some(t),
            Operand::Constant(Constant::Null) => *t == Type::Ref,
            Operand::Constant(Constant::Int(_)) => *t == Type::I64,
            Operand::Constant(Constant::Double(_)) => *t == Type::F64,
            Operand::Constant(Constant::Bool(_)) => *t == Type::I1,
            _ => false,
        };
        if !compativel { return Err(erro()); }
    }
    Ok(true)
}

// O registro runtime confere classe, mapa físico, ABI e PC antes da chamada.
// A leitura do descritor permanece escalar e ganha bounds/mapa conferidos.
// Opera apenas na cópia privada da preparação ARC, antes dos campos/contratos.
pub(super) fn preparar(f: &mut Function) -> Result<(), String> {
    let mut alvos = HashMap::new();
    let mut closures = Vec::new();
    for (_, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
        let Instruction::ChamadaTipada { alvo, args, ret } = inst else { continue };
        if conferir(f, inst, *ty).unwrap_or(false) {
            let closure = contexto(f, &args[0].0).unwrap();
            if !closures.contains(&closure) { closures.push(closure); }
            continue;
        }
        let erro = || format!("chamada tipada ARC em {}: leitura legada sem PC/contexto/ABI cobertos", f.symbol);
        let abi = assinatura(args, *ret).ok_or_else(erro)?;
        let Operand::Val(v) = alvo else { return Err(erro()) };
        let Some(Instruction::CallRuntime { name, args: origem, ret_ty: Type::I64 }) = definicao(f, alvo)
            else { return Err(erro()) };
        let [(closure, Type::Ref), (Operand::Constant(Constant::Int(2)), Type::I64)] = origem.as_slice()
            else { return Err(erro()) };
        if name != "dartforge_object_get" || contexto(f, &args[0].0).as_ref() != Some(closure) {
            return Err(erro());
        }
        if alvos.insert(*v, (closure.clone(), abi)).is_some_and(|antigo| antigo != (closure.clone(), abi)) {
            return Err(erro());
        }
        if !closures.contains(closure) { closures.push(closure.clone()); }
    }
    for (v, inst, _) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
        if let Some((closure, abi)) = alvos.get(v) {
            *inst = Instruction::CallRuntime { name: ALVO.into(), args: vec![
                (closure.clone(), Type::Ref), (Operand::Constant(Constant::Int(*abi)), Type::I64),
            ], ret_ty: Type::I64 };
        } else if let Instruction::CallRuntime { name, args, ret_ty: Type::I64 } = inst
            && name == "dartforge_object_get"
            && let [(closure, Type::Ref), (Operand::Constant(Constant::Int(3)), Type::I64)] = args.as_slice()
            && closures.contains(closure)
        {
            *name = "dartforge_arc_ler_campo_escalar_v1".into();
        }
    }
    for (_, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
        conferir(f, inst, *ty)?;
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn retornos_escalares_exigem_abi_e_plano_trivial_do_corpo() {
        for (ty, valor, abi) in [(Type::I64, Constant::Int(i64::MIN), 32),
            (Type::F64, Constant::Double(-0.0), 33), (Type::I1, Constant::Bool(true), 34)] {
            let (mut m, mut p) = modulo();
            m.retornos_ref_dart.remove("corpo"); m.retornos_ref_dart.remove("invocar");
            m.functions[1].return_ty = ty;
            m.functions[1].blocks[0].terminator = Terminator::Return(Some(Operand::Constant(valor.clone())));
            m.functions[0].return_ty = ty;
            m.functions[0].blocks[1].terminator = Terminator::Return(Some(Operand::Constant(valor)));
            m.functions[0].blocks[0].instructions[3].2 = ty;
            if let Instruction::ChamadaTipada { ret, .. } = &mut m.functions[0].blocks[0].instructions[3].1 { *ret = ty; }
            if let Instruction::AllocClosureTipada { abi: codigo, .. } = &mut m.functions[3].blocks[0].instructions[0].1 { *codigo = abi; }
            preparar_arc_modulo_dart(&mut m, &mut p).unwrap();
            assert_eq!(p["invocar"].classes[&ValueId(4)], Ownership::Trivial);
            assert_eq!(p["corpo"].tokens.retorno, RetornoTokens::Trivial);
            certificar_portas_arc_aot(&mut m, &p).unwrap();
            let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
            assert!(ir.contains(&format!("call {} %ct4(", ty.llvm_ir())));
            let pronto = p.clone();
            assert_eq!(preparar_arc_modulo_dart(&mut m, &mut p).unwrap(), (0, 0)); assert_eq!(p, pronto);
            p.get_mut("corpo").unwrap().tokens.retorno = RetornoTokens::Owned;
            assert!(certificar_portas_arc_aot(&mut m, &p).is_err());
        }
    }

    fn modulo() -> (Module, HashMap<String, PlanoFuncaoDart>) {
        let mut m = Module::new(); m.memoria_arc = true;
        m.functions = vec![
            Function { symbol: "invocar".into(), name: "invocar".into(), depuracao: None,
                params: vec![(ValueId(0), "closure".into(), Type::Ref), (ValueId(1), "x".into(), Type::Ref),
                    (ValueId(8), "outra".into(), Type::Ref)], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(10), Instruction::CallRuntime { name: "dartforge_object_get".into(), args: vec![
                        (Operand::Val(ValueId(0)), Type::Ref), (Operand::Constant(Constant::Int(3)), Type::I64),
                    ], ret_ty: Type::I64 }, Type::I64),
                    (ValueId(2), Instruction::CallRuntime { name: "dartforge_object_get".into(), args: vec![
                        (Operand::Val(ValueId(0)), Type::Ref), (Operand::Constant(Constant::Int(2)), Type::I64),
                    ], ret_ty: Type::I64 }, Type::I64),
                    (ValueId(3), Instruction::CallRuntime { name: CONTEXTO.into(), args: vec![
                        (Operand::Val(ValueId(0)), Type::Ref),
                    ], ret_ty: Type::Ref }, Type::Ref),
                    (ValueId(4), Instruction::ChamadaTipada { alvo: Operand::Val(ValueId(2)), args: vec![
                        (Operand::Val(ValueId(3)), Type::Ref), (Operand::Val(ValueId(1)), Type::Ref),
                    ], ret: Type::Ref }, Type::Ref),
                    (ValueId(5), Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                    (ValueId(6), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(5)), Operand::Constant(Constant::Int(0))), Type::I1),
                ], terminator: Terminator::CondBranch { cond: Operand::Val(ValueId(6)), then_block: BlockId(1), else_block: BlockId(2) } },
                    BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
                    BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(4)))) },
                ] },
            Function { symbol: "corpo".into(), name: "corpo".into(), depuracao: None,
                params: vec![(ValueId(0), "env".into(), Type::Ref), (ValueId(1), "x".into(), Type::Ref)], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }] },
            Function { symbol: "entrada".into(), name: "entrada".into(), depuracao: None,
                params: vec![(ValueId(0), "closure".into(), Type::Ref), (ValueId(1), "args".into(), Type::Ptr), (ValueId(2), "desc".into(), Type::Ptr)],
                return_ty: Type::Ref, blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) }] },
            Function { symbol: "capturar".into(), name: "capturar".into(), depuracao: None, params: vec![], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(0), Instruction::AllocClosureTipada { code_symbol: "entrada".into(), env: Operand::Constant(Constant::Null), tipado: "corpo".into(), abi: 31, direto: false }, Type::Ref),
                ], terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] },
        ];
        m.retornos_ref_dart.extend(["invocar".into(), "corpo".into(), "capturar".into()]);
        m.retornos_uniformes_arc.insert("entrada".into());
        m.parametros_vetores_dart.insert("entrada".into(), HashMap::from([
            (ValueId(1), VetorEntradaDart::Argumentos), (ValueId(2), VetorEntradaDart::Descritor),
        ]));
        let p = m.functions.iter().map(|f| (f.symbol.clone(), PlanoFuncaoDart::default())).collect();
        (m, p)
    }

    #[test]
    fn legado_vira_pc_conferido_com_contexto_retido_e_pending_sem_owner_no_erro() {
        for caso in 0..17 {
            let (mut m, mut p) = modulo();
            match caso {
                0 => {},
                1 | 6 => m.functions[0].blocks[0].instructions[1].1 = Instruction::CallRuntime {
                    name: ALVO.into(), args: vec![(Operand::Val(ValueId(0)), Type::Ref),
                        (Operand::Constant(Constant::Int(if caso == 1 { 31 } else { 41 })), Type::I64)], ret_ty: Type::I64 },
                2 => if let Instruction::ChamadaTipada { alvo, .. } = &mut m.functions[0].blocks[0].instructions[3].1 { *alvo = Operand::Constant(Constant::Int(123)); },
                3 => if let Instruction::CallRuntime { args, .. } = &mut m.functions[0].blocks[0].instructions[2].1 { args[0].0 = Operand::Val(ValueId(8)); },
                4 => if let Instruction::ChamadaTipada { ret, .. } = &mut m.functions[0].blocks[0].instructions[3].1 { *ret = Type::I64; },
                5 => { m.functions[0].blocks[0].instructions.truncate(4); m.functions[0].blocks[0].terminator = Terminator::Branch(BlockId(2)); },
                7 => m.functions[0].blocks[1].terminator = Terminator::Return(Some(Operand::Val(ValueId(4)))),
                8 => m.memoria_arc = false,
                9..=16 => {
                    m.functions[0].blocks[0].instructions.drain(0..3);
                    m.functions[0].blocks[0].instructions[0].1 = Instruction::CallClosure {
                        closure: Operand::Val(ValueId(0)), args: vec![Operand::Val(ValueId(1))],
                        nomes: vec![], tupla_tipos: Operand::Constant(Constant::Int(0)), ret_ty: Type::Ref,
                    };
                    if let Instruction::CallClosure { args, nomes, tupla_tipos, .. } = &mut m.functions[0].blocks[0].instructions[0].1 {
                        match caso {
                            10 => args[0] = Operand::Constant(Constant::Int(123)),
                            11 => *tupla_tipos = Operand::Constant(Constant::Int(1)),
                            12 => { args.push(Operand::Val(ValueId(8))); *nomes = vec!["a".into(), "a".into()]; },
                            _ => {},
                        }
                    }
                    if caso == 13 {
                        let mut t = TabelasDaFuncao::default(); t.invocacoes.insert(ValueId(4), BlockId(1));
                        m.tabelas.push(t);
                    }
                    if caso >= 14 {
                        let inst = if caso == 15 { Instruction::Const(Constant::Int(1)) } else {
                            Instruction::CallRuntime { name: "dartforge_rti_avaliar".into(), args: vec![
                                (Operand::Constant(Constant::Int(0)), Type::I64),
                                (Operand::Constant(Constant::Null), if caso == 16 { Type::I64 } else { Type::Ref }),
                                (Operand::Constant(Constant::Int(0)), Type::I64), (Operand::Constant(Constant::Int(0)), Type::I64),
                            ], ret_ty: Type::I64 }
                        };
                        m.functions[0].blocks[0].instructions.insert(0, (ValueId(9), inst, Type::I64));
                        if let Instruction::CallClosure { tupla_tipos, .. } = &mut m.functions[0].blocks[0].instructions[1].1 {
                            *tupla_tipos = Operand::Val(ValueId(9));
                        }
                    }
                },
                _ => unreachable!(),
            }
            let antes = format!("{m:?}/{p:?}");
            let resultado = preparar_arc_modulo_dart(&mut m, &mut p);
            if matches!(caso, 0 | 1 | 9 | 14) {
                resultado.unwrap();
                assert_eq!(p["invocar"].classes[&ValueId(4)], Ownership::Owned);
                assert_eq!(p["invocar"].tokens.pendencias[&ValueId(4)], BlockId(1));
                if caso < 9 { assert_eq!(p["invocar"].classes[&ValueId(3)], Ownership::Owned); }
                assert!(!m.functions[0].blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i,
                    Instruction::CallRuntime { name, .. } if name == "dartforge_object_get")));
                let preparado = format!("{m:?}"); let planos_preparados = p.clone();
                assert_eq!(preparar_arc_modulo_dart(&mut m, &mut p).unwrap(), (0, 0));
                assert_eq!(preparado, format!("{m:?}")); assert_eq!(planos_preparados, p);
                certificar_portas_arc_aot(&mut m, &p).unwrap();
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                if caso >= 9 {
                    assert!(ir.contains("call i64 @dartforge_arc_invocar_closure_uniforme_v1(i64 %v0"));
                    assert!(!ir.contains("call i64 %cf4("));
                } else {
                    assert!(ir.contains("@dartforge_arc_alvo_tipada_v1(i64 %v0, i64 31)"));
                    assert!(ir.contains("call i64 %ct4("));
                    let (mut sem_registro, _) = modulo();
                    sem_registro.functions = m.functions.clone();
                    sem_registro.functions[3].blocks[0].instructions.clear();
                    sem_registro.functions[3].blocks[0].terminator = Terminator::Return(Some(Operand::Constant(Constant::Null)));
                    sem_registro.portas_arc_aot = None;
                    let mut novos = HashMap::from_iter(sem_registro.functions.iter().map(|f| (f.symbol.clone(), PlanoFuncaoDart::default())));
                    // Os corpos preparados mantêm efeitos próprios; reutiliza
                    // seus planos, exceto a fábrica que agora retorna null.
                    for s in ["invocar", "corpo", "entrada"] { novos.insert(s.into(), p[s].clone()); }
                    preparar_arc_modulo_dart(&mut sem_registro, &mut novos).unwrap();
                    assert!(certificar_portas_arc_aot(&mut sem_registro, &novos).unwrap_err().contains("sem corpo registrado"));
                }
            } else {
                assert_eq!(resultado.is_ok(), caso == 8, "caso {caso}: {resultado:?}");
                assert_eq!(antes, format!("{m:?}/{p:?}"));
            }
        }
    }
}
