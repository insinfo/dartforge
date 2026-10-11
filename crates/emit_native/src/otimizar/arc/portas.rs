//! Liga declarações de portas aos corpos, contratos e dependências verificados.

use super::*;

/// Inventário de portas de uma imagem AOT fixa, ligado à HIR verificada.
///
/// Campos privados impedem declarar Owned/readonly sem verificar os corpos.
/// Não é certificado de recarga JIT nem de chamadas indiretas legadas.
///
/// ```
/// use dartforge_emit_native::hir::Module;
/// assert!(Module::new().portas_arc_aot.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortasArcAot {
    estado: String,
    uniformes: Vec<String>,
    callbacks: Vec<(String, usize)>,
}

fn estado(m: &Module) -> String {
    // Guarda também os callees, layouts e tabelas: modificar só uma dependência
    // pode invalidar a transferência mesmo que o corpo registrado não mude.
    format!(
        "{:?}/{:?}/{:?}/{:?}/{:?}",
        (
            &m.functions,
            &m.tabelas,
            m.memoria_arc,
            m.excecoes_por_tabelas
        ),
        (
            &m.parametros_escalares_dart,
            &m.parametros_rti_dart,
            &m.parametros_vetores_dart,
            &m.retornos_ref_dart,
            &m.retornos_uniformes_arc
        ),
        (
            &m.classes,
            &m.layouts_campos_arc,
            &m.globais,
            &m.caches_rti_arc
        ),
        (
            &m.ajudantes,
            &m.tabelas_de_metodos,
            m.modo_sdk,
            m.biblioteca_sdk
        ),
        (
            &m.entry_symbol,
            &m.registro,
            &m.chamar_dart,
            &m.funcoes_compartilhadas_arc
        )
    )
}

impl PortasArcAot {
    pub(crate) fn conferir(&self, m: &Module) -> Result<(), String> {
        if self.estado != estado(m) {
            return Err("portas ARC: certificado obsoleto da imagem AOT".into());
        }
        Ok(())
    }

    pub(crate) fn uniformes(&self) -> &[String] {
        &self.uniformes
    }
    pub(crate) fn callbacks(&self) -> &[(String, usize)] {
        &self.callbacks
    }
}

// Cada endereço de vetor só pode ser lido ou repassado ao mesmo papel de um
// corpo local. Assim ciclos de chamadas não inventam readonly por declaração.
fn conferir_vetores(m: &Module) -> Result<(), String> {
    let funcoes: HashMap<_, _> = m.functions.iter().map(|f| (&f.symbol, f)).collect();
    for (s, papeis) in &m.parametros_vetores_dart {
        let f = funcoes
            .get(s)
            .ok_or_else(|| format!("portas ARC: vetor de função ausente {s}"))?;
        for (p, papel) in papeis {
            if f.params.iter().find(|(v, _, _)| v == p).map(|(_, _, t)| t) != Some(&Type::Ptr) {
                return Err(format!("portas ARC: vetor {s} v{} sem parâmetro Ptr", p.0));
            }
            for (v, i, t) in f.blocks.iter().flat_map(|b| &b.instructions) {
                let usa = |o: &Operand| matches!(o, Operand::Val(id) if id == p);
                let mut usado = false;
                super::super::operandos::operandos(i, &mut |o| usado |= usa(o));
                if !usado {
                    continue;
                }
                let permitido = match i {
                    Instruction::CallSeletorRepasse {
                        arc_owned: true,
                        recv,
                        args,
                        desc,
                        ..
                    }
                    | Instruction::CallClosureRepasse {
                        arc_owned: true,
                        closure: recv,
                        args,
                        desc,
                        ..
                    } => {
                        !usa(recv)
                            && match papel {
                                VetorEntradaDart::Argumentos => usa(args) && !usa(desc),
                                VetorEntradaDart::Descritor => usa(desc) && !usa(args),
                            }
                    }
                    Instruction::CallStatic { symbol, args, .. } => {
                        funcoes.get(symbol).is_some_and(|callee| {
                            args.len() == callee.params.len()
                                && args.iter().enumerate().all(|(j, o)| {
                                    !usa(o)
                                        || m.parametros_vetores_dart
                                            .get(symbol)
                                            .and_then(|p| p.get(&callee.params[j].0))
                                            == Some(papel)
                                })
                        })
                    }
                    Instruction::LoadIndexed { base, index } => {
                        *papel == VetorEntradaDart::Descritor
                            && usa(base)
                            && matches!(index, Operand::Constant(Constant::Int(0 | 1)))
                            && *t == Type::I64
                    }
                    Instruction::CallRuntime { name, args, .. } => {
                        *papel == VetorEntradaDart::Descritor
                            && matches!(
                                name.as_str(),
                                "dartforge_args_casam" | "dartforge_arg_indice"
                            )
                            && args.first().is_some_and(|(o, t)| usa(o) && *t == Type::Ptr)
                            && args.iter().skip(1).all(|(o, _)| !usa(o))
                    }
                    _ => false,
                };
                if !permitido {
                    return Err(format!(
                        "portas ARC: vetor {papel:?} {s} v{} sem uso readonly coberto",
                        v.0
                    ));
                }
            }
            for b in &f.blocks {
                let mut publicado = false;
                super::super::operandos::operandos_do_terminador(&b.terminator, &mut |o| {
                    publicado |= matches!(o, Operand::Val(v) if v == p);
                });
                if publicado {
                    return Err(format!("portas ARC: {s} publica vetor no terminador"));
                }
            }
        }
    }
    Ok(())
}

/// Certifica portas Owned de corpos já preparados num módulo AOT fechado.
///
/// Verifica novamente todos os corpos/callees e exige preparação idempotente.
/// Entradas uniformes também exigem vetores readonly; leituras de argumentos
/// ainda precisam de ABI/bounds próprios e não recebem certificado aqui.
/// A imagem AOT permanece viva até terminar o processo. Recarga/retirada de
/// código JIT exige outro protocolo. Tracing não produz registros nem muda HIR.
///
/// # Erros
/// Corpo não preparado, ABI incompatível, dependência aberta, vetor publicado,
/// operação indireta/heap não auditada ou fronteira runtime ainda não escopada.
/// Nenhum certificado é publicado em caso de falha.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
/// use std::collections::HashMap;
/// let mut m = Module::new(); m.memoria_arc = true;
/// m.functions.push(Function { symbol: "helper".into(), name: "helper".into(),
///     depuracao: None, params: vec![], return_ty: Type::Ref,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) }] });
/// m.retornos_ref_dart.insert("helper".into());
/// m.ajudantes.push(("nome_runtime".into(), "helper".into()));
/// let mut planos = HashMap::from([("helper".into(), PlanoFuncaoDart::default())]);
/// preparar_arc_modulo_dart(&mut m, &mut planos)?;
/// assert_eq!(certificar_portas_arc_aot(&mut m, &planos)?, 1);
/// # Ok::<(), String>(())
/// ```
pub fn certificar_portas_arc_aot(
    m: &mut Module,
    planos: &HashMap<String, PlanoFuncaoDart>,
) -> Result<usize, String> {
    if !m.memoria_arc {
        return Ok(0);
    }
    super::seletores::conferir(m)?;
    conferir_vetores(m)?;
    let mut fs = m.functions.clone();
    let mut ps = planos.clone();
    produzir_parametros_escalares_do_lowering(m, &mut ps)?;
    produzir_parametros_rti_do_lowering(m, &mut ps)?;
    super::vetores::produzir(m, &mut ps)?;
    for (indice, f) in fs.iter().enumerate() {
        let tipos: HashMap<_, _> = f
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
        let referencia = |o: &Operand| {
            matches!(o, Operand::Val(v) if tipos.get(v) == Some(&Type::Ref))
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
            if m.tabelas.get(indice) != Some(&p.tabelas) {
                return Err(format!(
                    "portas ARC: tabelas emitidas divergentes em {}",
                    f.symbol
                ));
            }
        } else if !p.tabelas.invocacoes.is_empty()
            || p.tabelas
                .saidas
                .values()
                .any(|s| *s == SaidaPorExcecao::Retoma)
        {
            return Err(format!(
                "portas ARC: {} exige emissão de invokes/tabelas",
                f.symbol
            ));
        }
        for (_, i, t) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if matches!(i, Instruction::ICmp(_, a, b) if referencia(a) || referencia(b))
                || matches!(i, Instruction::StoreGlobal { val, ty, .. } if *ty == Type::Ref || referencia(val))
            {
                return Err(format!(
                    "portas ARC: {} exige operação explícita sem coerção Ref/escapes implícitos",
                    f.symbol
                ));
            }
            if matches!(
                i,
                Instruction::CallDynamic { .. }
                    | Instruction::CallInterface { .. }
                    | Instruction::CallClosure { .. }
                    | Instruction::CallClosureRepasse {
                        arc_owned: false,
                        ..
                    }
                    | Instruction::CallSeletor { .. }
                    | Instruction::CallSeletorRepasse {
                        arc_owned: false,
                        ..
                    }
                    | Instruction::ChamadaTipada { .. }
                    | Instruction::ChamadaNativa { .. }
                    | Instruction::ChamadaNativaComposta { .. }
                    | Instruction::GravacaoNativa { .. }
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
                        | Instruction::CallStatic { .. }
                        | Instruction::CallSeletorRepasse {
                            arc_owned: true,
                            ..
                        }
                        | Instruction::CallClosureRepasse {
                            arc_owned: true,
                            ..
                        }
                        | Instruction::CallRuntime { .. }
                ))
            {
                return Err(format!(
                    "portas ARC: {} contém operação sem ABI auditada",
                    f.symbol
                ));
            }
            if let Instruction::CallRuntime { name, .. } = i
                && crate::llvm::externs::efeitos_de(name).chama_dart
                && name != "dartforge_arc_invocar_uniforme_v1"
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
    let antes = format!("{fs:?}/{ps:?}");
    let alteracoes = preparar_arc_funcoes_dart(&mut fs, &mut ps)?;
    if alteracoes != (0, 0) || antes != format!("{fs:?}/{ps:?}") {
        return Err("portas ARC: corpos/planos ainda não preparados ou divergentes".into());
    }
    let mut uniformes: Vec<_> = m.retornos_uniformes_arc.iter().cloned().collect();
    uniformes.sort();
    // Revalida a assinatura nominal contra os corpos, sem confiar só no nome.
    produzir_retornos_ref_do_lowering(m, &mut ps)?;
    let por_nome: HashMap<_, _> = fs.iter().map(|f| (&f.symbol, f)).collect();
    let mut callbacks = Vec::new();
    for (_, s) in &m.ajudantes {
        let f = por_nome
            .get(s)
            .ok_or_else(|| format!("portas ARC: helper sem corpo {s}"))?;
        if f.return_ty != Type::Ref || uniformes.contains(s) {
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
    let total = callbacks.len() + uniformes.len();
    m.portas_arc_aot = Some(PortasArcAot {
        estado: estado(m),
        uniformes,
        callbacks,
    });
    Ok(total)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::llvm::LlvmEmitter;

    fn modulo(tabelas: bool) -> (Module, HashMap<String, PlanoFuncaoDart>) {
        let mut m = Module::new();
        m.memoria_arc = true;
        let f = Function {
            symbol: "folha".into(),
            name: "folha".into(),
            depuracao: None,
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
            m.retornos_uniformes_arc.insert(s.into());
            m.parametros_vetores_dart.insert(
                s.into(),
                HashMap::from([
                    (ValueId(1), VetorEntradaDart::Argumentos),
                    (ValueId(2), VetorEntradaDart::Descritor),
                ]),
            );
        }
        if tabelas {
            crate::otimizar::excecoes_por_tabelas(&mut m);
        }
        let mut p = HashMap::from([
            ("folha".into(), PlanoFuncaoDart::default()),
            ("repasse".into(), PlanoFuncaoDart::default()),
        ]);
        preparar_arc_modulo_dart(&mut m, &mut p).unwrap();
        (m, p)
    }

    #[test]
    fn repasse_por_seletor_prepara_owned_e_emite_adaptador_com_pending() {
        for closure in [false, true] {
            for caso in 0..4 {
                let (mut m, mut planos) = modulo(false);
                planos.insert("repasse".into(), PlanoFuncaoDart::default());
                let g = &mut m.functions[1];
                g.blocks.truncate(1);
                g.blocks[0].instructions = vec![
                    (
                        ValueId(3),
                        Instruction::CallSeletorRepasse {
                            arc_owned: false,
                            seletor: "c:identidade".into(),
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
                    g.blocks[0].instructions[0].1 = Instruction::CallClosureRepasse {
                        arc_owned: false,
                        closure: Operand::Val(ValueId(0)),
                        args: Operand::Val(ValueId(1)),
                        desc: Operand::Val(ValueId(2)),
                    };
                }
                if caso == 1 {
                    if let Instruction::CallSeletorRepasse { args, .. }
                    | Instruction::CallClosureRepasse { args, .. } =
                        &mut g.blocks[0].instructions[0].1
                    {
                        *args = Operand::Val(ValueId(2));
                    }
                } else if caso == 2 {
                    g.blocks[0].instructions.pop();
                } else if caso == 3 {
                    m.memoria_arc = false;
                }
                let antes = format!("{m:?}/{planos:?}");
                let resultado = preparar_arc_modulo_dart(&mut m, &mut planos);
                if matches!(caso, 1 | 2) {
                    assert!(resultado.is_err(), "{resultado:?}");
                    assert_eq!(antes, format!("{m:?}/{planos:?}"));
                    continue;
                }
                resultado.unwrap();
                if caso == 3 {
                    assert_eq!(antes, format!("{m:?}/{planos:?}"));
                    continue;
                }
                assert_eq!(planos["repasse"].classes[&ValueId(3)], Ownership::Owned);
                assert_eq!(planos["repasse"].tokens.pendencias[&ValueId(3)], BlockId(1));
                assert_eq!(
                    preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                    (0, 0)
                );
                certificar_portas_arc_aot(&mut m, &planos).unwrap();
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
    fn registro_aot_certificado_e_idempotente_e_fica_antes_do_dart() {
        for tabelas in [false, true] {
            let (mut m, p) = modulo(tabelas);
            assert_eq!(certificar_portas_arc_aot(&mut m, &p).unwrap(), 2);
            let certificado = m.portas_arc_aot.clone();
            assert_eq!(certificar_portas_arc_aot(&mut m, &p).unwrap(), 2);
            assert_eq!(m.portas_arc_aot, certificado);
            let ir = LlvmEmitter::new(&m).emit_all();
            assert_eq!(
                ir.matches(" = call i64 @dartforge_arc_registrar_uniforme_owned_v1")
                    .count(),
                2
            );
            assert!(ir.contains("df.arc.compativel:\n  call void @df.arc.registrar_portas()"));
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    LlvmEmitter::new(&m).com_mapas_no_jit(true).emit_all()
                }))
                .is_err()
            );
        }
    }

    #[test]
    fn emissao_recusa_corpo_dependencia_modo_e_metadados_obsoletos() {
        for caso in 0..7 {
            let (mut m, p) = modulo(true);
            certificar_portas_arc_aot(&mut m, &p).unwrap();
            match caso {
                0 => m.functions[0].blocks[0].instructions.clear(),
                1 => m.functions[1].name.push_str("_mudou"),
                2 => m.memoria_arc = false,
                3 => m.tabelas[0].confere_pilha = !m.tabelas[0].confere_pilha,
                4 => m.parametros_vetores_dart.clear(),
                5 => m.retornos_uniformes_arc.clear(),
                6 => m.ajudantes.push(("injetado".into(), "repasse".into())),
                _ => unreachable!(),
            }
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    LlvmEmitter::new(&m).emit_all()
                }))
                .is_err(),
                "caso {caso}"
            );
        }
    }

    #[test]
    fn vetores_nao_ganham_readonly_por_alias_publicacao_ou_chamada_aberta() {
        for caso in 0..4 {
            let (mut m, p) = modulo(false);
            m.functions[0].blocks[0].instructions.push((
                ValueId(50),
                match caso {
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
                        simbolo: "escape".into(),
                        val: Operand::Val(ValueId(1)),
                        ty: Type::Ptr,
                        raiz: None,
                    },
                    _ => unreachable!(),
                },
                Type::I64,
            ));
            let antes = format!("{m:?}/{p:?}");
            let erro = certificar_portas_arc_aot(&mut m, &p).unwrap_err();
            assert!(erro.contains("readonly"), "{erro}");
            assert_eq!(format!("{m:?}/{p:?}"), antes);
            assert!(m.portas_arc_aot.is_none());
        }
    }

    #[test]
    fn declaracao_own_sem_preparar_corpo_e_tracing_nao_publicam_portas() {
        let (mut m, p) = modulo(false);
        m.functions[0].blocks[0].instructions.clear();
        m.functions[0].blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(0))));
        let antes = format!("{m:?}");
        assert!(certificar_portas_arc_aot(&mut m, &p).is_err());
        assert_eq!(format!("{m:?}"), antes);
        m.memoria_arc = false;
        let antes = format!("{m:?}");
        assert_eq!(certificar_portas_arc_aot(&mut m, &p).unwrap(), 0);
        assert_eq!(format!("{m:?}"), antes);
        let ir = LlvmEmitter::new(&m).emit_all();
        assert!(!ir.contains("define private void @df.arc.registrar_portas"));
    }

    #[test]
    fn corpo_nao_certifica_unboxing_implicito_ou_alteracao_de_escopo_nativo() {
        for escopo in [false, true] {
            let (mut m, p) = modulo(false);
            m.functions[0].blocks[0].instructions.push((
                ValueId(50),
                if escopo {
                    Instruction::CallRuntime {
                        name: "dartforge_arc_escopo_nativo_abrir_v1".into(),
                        args: vec![],
                        ret_ty: Type::I64,
                    }
                } else {
                    Instruction::ICmp(
                        ICmpOp::Eq,
                        Operand::Val(ValueId(0)),
                        Operand::Constant(Constant::Null),
                    )
                },
                if escopo { Type::I64 } else { Type::I1 },
            ));
            let antes = format!("{m:?}");
            let erro = certificar_portas_arc_aot(&mut m, &p).unwrap_err();
            assert!(
                erro.contains(if escopo {
                    "sem protocolo verificado"
                } else {
                    "coerção Ref"
                }),
                "{erro}"
            );
            assert_eq!(format!("{m:?}"), antes);
        }
    }
}
