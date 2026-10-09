//! Resumos de convenção Dart vinculados à verificação do corpo do callee.
//! Contratos de externs e escopos fornecidos continuam sendo premissas.

use super::*;
use std::collections::{HashMap, HashSet};

/// Convenção de uma versão verificada de uma função Dart.
///
/// Campos privados impedem fabricar um resumo sem verificar o corpo.
/// Refaça o resumo após mudar a função ou seus contratos. Não certifica
/// proveniência, invalidação de borrows nem os contratos externos fornecidos.
/// Chamadas diretas no corpo são conservadoramente consideradas falíveis.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "vazia".into(), name: "vazia".into(), depuracao: None,
///     params: vec![], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(None) }] };
/// let resumo = verificar_contrato_funcao_dart(&f, &HashMap::new(), &PlanoTokens::default(),
///     &TabelasDaFuncao::default(), &PlanoEscopos::default())?;
/// # let _ = resumo;
/// # Ok::<(), String>(())
/// ```
#[derive(Debug, Clone)]
pub struct ContratoFuncaoDart {
    simbolo: String,
    parametros: Vec<Type>,
    retorno: Type,
    pode_falhar: bool,
}

/// Verifica a convenção Dart do corpo e extrai um resumo para chamadas diretas.
///
/// Os mapas fornecidos não são alterados. Parâmetros não Ref precisam de
/// classificação Trivial explícita/produzida; não são escalares pela largura.
/// O chamador deve usar o resumo apenas com esta versão do corpo e ABI.
///
/// # Erros
/// Falha do verificador Dart completo ou parâmetro não Ref não Trivial.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "vazia".into(), name: "vazia".into(), depuracao: None,
///     params: vec![], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(None) }] };
/// verificar_contrato_funcao_dart(&f, &HashMap::new(), &PlanoTokens::default(),
///     &TabelasDaFuncao::default(), &PlanoEscopos::default())?;
/// # Ok::<(), String>(())
/// ```
pub fn verificar_contrato_funcao_dart(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    plano: &PlanoTokens,
    tabelas: &TabelasDaFuncao,
    escopos: &PlanoEscopos,
) -> Result<ContratoFuncaoDart, String> {
    let mut classes = classes.clone();
    let mut plano = plano.clone();
    produzir_e_verificar_tokens_dart(f, &mut classes, &mut plano, tabelas, escopos)?;
    for (v, _, ty) in &f.params {
        if *ty != Type::Ref && classes[v] != Ownership::Trivial {
            return Err(format!("parâmetro não Ref v{} não é Trivial", v.0));
        }
    }
    let tipos: HashMap<_, _> = f
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
    for b in &f.blocks {
        if let Terminator::Return(op) = &b.terminator {
            let compativel = match op {
                None => f.return_ty == Type::Void,
                Some(op) => operando_tem_tipo(op, f.return_ty, &tipos),
            };
            if !compativel {
                return Err(format!(
                    "representação de retorno Dart incompatível: {}",
                    f.symbol
                ));
            }
        }
    }
    let desconhecidas = HashSet::new();
    let pode_falhar = f.blocks.iter().any(|b| {
        matches!(b.terminator, Terminator::Throw(_))
            || b.instructions
                .iter()
                .any(|(_, inst, _)| super::super::efeitos::instrucao_lanca(inst, &desconhecidas))
    });
    Ok(ContratoFuncaoDart {
        simbolo: f.symbol.clone(),
        parametros: f.params.iter().map(|(_, _, ty)| *ty).collect(),
        retorno: f.return_ty,
        pode_falhar,
    })
}

fn operando_tem_tipo(op: &Operand, esperado: Type, tipos: &HashMap<ValueId, Type>) -> bool {
    match op {
        Operand::Val(v) => tipos.get(v) == Some(&esperado),
        Operand::Constant(Constant::Null | Constant::String(_) | Constant::StringWtf8(_)) => {
            esperado == Type::Ref
        }
        Operand::Constant(Constant::Int(_)) => esperado == Type::I64,
        Operand::Constant(Constant::Bool(_)) => esperado == Type::I1,
        Operand::Constant(Constant::Double(_)) => esperado == Type::F64,
        _ => false,
    }
}

/// Produz contratos de CallStatic usando resumos de corpos Dart verificados.
///
/// Argumentos Ref são emprestados durante a chamada; resultado Ref é Owned.
/// Demais argumentos SSA exigem classe Trivial, além do tipo exato da ABI.
/// Resultado não Ref é Trivial pela convenção verificada, não pela largura.
/// Falha conservadora exige CFG excepcional no verificador posterior.
/// Não cobre dispatch dinâmico, stubs importados ou retenções ocultas externas.
///
/// # Erros
/// Símbolo ausente/repetido, assinatura incompatível ou conflito nos mapas.
/// Nenhuma entrada é publicada em caso de erro. Não verifica o fluxo do caller.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
/// use std::collections::HashMap;
/// let callee = Function { symbol: "vazia".into(), name: "vazia".into(), depuracao: None,
///     params: vec![], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(None) }] };
/// let resumo = verificar_contrato_funcao_dart(&callee, &HashMap::new(), &PlanoTokens::default(),
///     &TabelasDaFuncao::default(), &PlanoEscopos::default())?;
/// let mut caller = callee.clone();
/// caller.symbol = "caller".into();
/// caller.blocks[0].instructions.push((ValueId(0), Instruction::CallStatic {
///     symbol: "vazia".into(), args: vec![], ret_ty: Type::Void }, Type::Void));
/// produzir_chamadas_dart(&caller, &[resumo], &mut HashMap::new(), &mut PlanoTokens::default())?;
/// # Ok::<(), String>(())
/// ```
pub fn produzir_chamadas_dart(
    f: &Function,
    resumos: &[ContratoFuncaoDart],
    classes: &mut HashMap<ValueId, Ownership>,
    plano: &mut PlanoTokens,
) -> Result<(), String> {
    super::ssa::verificar(f)?;
    let mut por_simbolo = HashMap::new();
    for resumo in resumos {
        if por_simbolo.insert(&resumo.simbolo, resumo).is_some() {
            return Err(format!("resumo Dart repetido: {}", resumo.simbolo));
        }
    }
    let tipos: HashMap<_, _> = f
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
    let mut novas_classes = classes.clone();
    let mut novo_plano = plano.clone();
    // A convenção dos parâmetros Ref não depende dos contratos das chamadas.
    produzir_parametros_ref_dart(f, &mut novas_classes)?;
    for b in &f.blocks {
        for (v, inst, ty) in &b.instructions {
            let Instruction::CallStatic {
                symbol,
                args,
                ret_ty,
            } = inst
            else {
                continue;
            };
            let resumo = por_simbolo
                .get(symbol)
                .ok_or_else(|| format!("callee Dart sem resumo: {symbol}"))?;
            if args.len() != resumo.parametros.len() || *ret_ty != resumo.retorno || ty != ret_ty {
                return Err(format!("assinatura Dart incompatível: {symbol}"));
            }
            for (op, esperado) in args.iter().zip(&resumo.parametros) {
                let compativel = operando_tem_tipo(op, *esperado, &tipos)
                    && match op {
                        Operand::Val(arg) => {
                            tipos.get(arg) == Some(esperado)
                                && (*esperado == Type::Ref
                                    || novas_classes.get(arg) == Some(&Ownership::Trivial))
                        }
                        Operand::Constant(_) => true,
                        _ => false,
                    };
                if !compativel {
                    return Err(format!("argumento Dart incompatível: {symbol}"));
                }
            }
            let classe = if *ret_ty == Type::Ref {
                Ownership::Owned
            } else {
                Ownership::Trivial
            };
            let efeito = EfeitoTokens {
                pode_falhar: resumo.pode_falhar,
                ..Default::default()
            };
            if novas_classes.get(v).is_some_and(|c| *c != classe)
                || novo_plano.instrucoes.get(v).is_some_and(|e| *e != efeito)
            {
                return Err(format!("contrato Dart conflitante em v{}", v.0));
            }
            novas_classes.insert(*v, classe);
            novo_plano.instrucoes.insert(*v, efeito);
        }
    }
    *classes = novas_classes;
    *plano = novo_plano;
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn resumo_confere_retorno_escalar_e_chamada_falivel_exige_saida() {
        let (mut callee, mut plano) = identidade();
        callee.params.clear();
        callee.return_ty = Type::I64;
        callee.blocks[0].instructions[0] =
            (ValueId(1), Instruction::Const(Constant::Int(42)), Type::I64);
        plano.retorno = RetornoTokens::Trivial;
        verificar_contrato_funcao_dart(
            &callee,
            &HashMap::new(),
            &plano,
            &TabelasDaFuncao::default(),
            &PlanoEscopos::default(),
        )
        .unwrap();
        callee.blocks[0].instructions[0] = (
            ValueId(1),
            Instruction::Const(Constant::Bool(true)),
            Type::I1,
        );
        assert!(
            verificar_contrato_funcao_dart(
                &callee,
                &HashMap::new(),
                &plano,
                &TabelasDaFuncao::default(),
                &PlanoEscopos::default()
            )
            .unwrap_err()
            .contains("representação")
        );
        let (mut callee, plano) = identidade();
        callee.blocks[0].instructions[0].1 = Instruction::CallStatic {
            symbol: "externa_com_contrato_fornecido".into(),
            args: vec![Operand::Val(ValueId(0))],
            ret_ty: Type::Ref,
        };
        let mut plano_callee = plano.clone();
        plano_callee
            .instrucoes
            .insert(ValueId(1), EfeitoTokens::default());
        let resumo = verificar_contrato_funcao_dart(
            &callee,
            &HashMap::from([(ValueId(1), Ownership::Owned)]),
            &plano_callee,
            &TabelasDaFuncao::default(),
            &PlanoEscopos::default(),
        )
        .unwrap();
        assert!(resumo.pode_falhar);
        let (mut caller, mut plano) = identidade();
        caller.symbol = "caller".into();
        caller.blocks[0].instructions[0].1 = Instruction::CallStatic {
            symbol: callee.symbol,
            args: vec![Operand::Val(ValueId(0))],
            ret_ty: Type::Ref,
        };
        let mut classes = HashMap::new();
        produzir_chamadas_dart(&caller, &[resumo], &mut classes, &mut plano).unwrap();
        assert!(plano.instrucoes[&ValueId(1)].pode_falhar);
        assert!(
            produzir_e_verificar_tokens_dart(
                &caller,
                &mut classes,
                &mut plano,
                &TabelasDaFuncao::default(),
                &PlanoEscopos::default()
            )
            .is_err()
        );
    }

    fn identidade() -> (Function, PlanoTokens) {
        (
            Function {
                symbol: "identidade".into(),
                name: "identidade".into(),
                depuracao: None,
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
            },
            PlanoTokens {
                retorno: RetornoTokens::Owned,
                ..Default::default()
            },
        )
    }

    #[test]
    fn chamada_direta_recebe_owned_do_callee_verificado_sem_contrato_manual() {
        let (callee, plano_callee) = identidade();
        let resumo = verificar_contrato_funcao_dart(
            &callee,
            &HashMap::new(),
            &plano_callee,
            &TabelasDaFuncao::default(),
            &PlanoEscopos::default(),
        )
        .unwrap();
        assert!(!resumo.pode_falhar);
        let (mut caller, mut plano) = identidade();
        caller.symbol = "caller".into();
        caller.blocks[0].instructions[0].1 = Instruction::CallStatic {
            symbol: callee.symbol.clone(),
            args: vec![Operand::Val(ValueId(0))],
            ret_ty: Type::Ref,
        };
        let mut classes = HashMap::new();
        produzir_chamadas_dart(&caller, &[resumo.clone()], &mut classes, &mut plano).unwrap();
        assert_eq!(classes[&ValueId(1)], Ownership::Owned);
        assert_eq!(
            inserir_arc_saidas_dart(
                &mut caller,
                &mut classes,
                &mut plano,
                &TabelasDaFuncao::default(),
                &PlanoEscopos::default()
            )
            .unwrap(),
            (0, 0)
        );
        // Uma chamada desconhecida posterior não publica nem a primeira chamada.
        caller.blocks[0].instructions.push((
            ValueId(2),
            Instruction::CallStatic {
                symbol: "ausente".into(),
                args: vec![],
                ret_ty: Type::Void,
            },
            Type::Void,
        ));
        classes.clear();
        plano.instrucoes.clear();
        assert!(
            produzir_chamadas_dart(&caller, &[resumo], &mut classes, &mut plano)
                .unwrap_err()
                .contains("sem resumo")
        );
        assert!(classes.is_empty() && plano.instrucoes.is_empty());
    }

    #[test]
    fn corpo_borrowed_e_argumento_com_mesma_largura_nao_certificam_chamada() {
        let (mut callee, plano) = identidade();
        callee.blocks[0].instructions.clear();
        callee.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(0))));
        assert!(
            verificar_contrato_funcao_dart(
                &callee,
                &HashMap::new(),
                &plano,
                &TabelasDaFuncao::default(),
                &PlanoEscopos::default()
            )
            .is_err()
        );
        let (callee, plano) = identidade();
        let resumo = verificar_contrato_funcao_dart(
            &callee,
            &HashMap::new(),
            &plano,
            &TabelasDaFuncao::default(),
            &PlanoEscopos::default(),
        )
        .unwrap();
        let (mut caller, mut plano) = identidade();
        caller.params[0].2 = Type::I64;
        caller.blocks[0].instructions[0].1 = Instruction::CallStatic {
            symbol: callee.symbol.clone(),
            args: vec![Operand::Val(ValueId(0))],
            ret_ty: Type::Ref,
        };
        let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
        assert!(
            produzir_chamadas_dart(&caller, &[resumo], &mut classes, &mut plano)
                .unwrap_err()
                .contains("argumento")
        );
        assert_eq!(classes.len(), 1);
        assert!(plano.instrucoes.is_empty());
    }
}
