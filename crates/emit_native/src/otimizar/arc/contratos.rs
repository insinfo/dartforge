//! Tradução das externs auditadas para o plano de tokens da HIR.
//! Não presume contratos para símbolos ausentes nem certifica proveniência/borrows.

use super::{EfeitoTokens, Ownership, PlanoTokens};
use crate::hir::*;
use dartforge_runtime::ownership::{ModoParametro, ModoResultado, contrato};
use std::collections::{HashMap, HashSet, VecDeque};

/// Produz classes de resultado e consumo de todas as chamadas runtime da função.
///
/// Aplica as alterações apenas depois de conferir todas as chamadas. Parâmetros,
/// operações ordinárias e chamadas Dart exigem metadados de outros produtores.
/// Retorna os contratos para análise posterior de retenção/invalidação.
/// Não insere contadores, limpa escopos ou certifica argumentos/proveniência.
///
/// # Erros
/// Extern sem contrato, assinatura inválida, resultado incompatível com o tipo
/// da instrução, IDs repetidos ou conflito com classe/efeito já fornecido.
/// As duas entradas permanecem intactas em caso de erro.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "f".into(), name: "f".into(), depuracao: None,
///     params: vec![], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![(ValueId(0),
///         Instruction::CallRuntime { name: "dartforge_arc_collect".into(), args: vec![], ret_ty: Type::Void }, Type::Void)],
///         terminator: Terminator::Return(None) }] };
/// let mut classes = HashMap::new();
/// let mut plano = PlanoTokens::default();
/// produzir_contratos_runtime(&f, &mut classes, &mut plano)?;
/// assert_eq!(classes[&ValueId(0)], Ownership::Trivial);
/// assert!(!plano.instrucoes[&ValueId(0)].pode_falhar);
/// # Ok::<(), String>(())
/// ```
pub fn produzir_contratos_runtime(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plano: &mut PlanoTokens,
) -> Result<HashMap<ValueId, ContratoChamadaRuntime>, String> {
    produzir(f, classes, plano, false)
}

/// Produz classes das operações ARC explícitas e contratos runtime auditados.
///
/// Copy/move/load produzem Owned; drop/store produzem Trivial. Operações ARC
/// não recebem entrada em PlanoTokens, pois o verificador possui suas regras.
/// Phi Ref ainda não classificado exige entradas owned/null e origem externa
/// ao ciclo de Phi/move. Parâmetros e demais operações exigem produtores próprios.
/// Não insere ARC nem certifica vida dos slots, proveniência ou cleanup.
///
/// # Erros
/// Os erros de produzir_contratos_runtime, tipo incompatível de operação ARC,
/// conflito de classe ou tentativa de sobrescrever sua regra no plano.
/// Phi owned com entrada emprestada/não classificada ou ciclo sem origem.
/// Nenhum mapa é alterado em caso de erro.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "f".into(), name: "f".into(), depuracao: None,
///     params: vec![], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
///         (ValueId(0), Instruction::ArcCopy { value: Operand::Constant(Constant::Null) }, Type::Ref),
///         (ValueId(1), Instruction::ArcDrop { value: Operand::Val(ValueId(0)) }, Type::Void)],
///         terminator: Terminator::Return(None) }] };
/// let mut classes = HashMap::new();
/// let mut plano = PlanoTokens::default();
/// produzir_contratos_arc(&f, &mut classes, &mut plano)?;
/// verificar_tokens(&f, &classes, &TabelasDaFuncao::default(), &plano)?;
/// # Ok::<(), String>(())
/// ```
pub fn produzir_contratos_arc(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plano: &mut PlanoTokens,
) -> Result<HashMap<ValueId, ContratoChamadaRuntime>, String> {
    let mut novas_classes = classes.clone();
    let mut novo_plano = plano.clone();
    let contratos = produzir(f, &mut novas_classes, &mut novo_plano, true)?;
    produzir_phi(f, &mut novas_classes, &novo_plano)?;
    *classes = novas_classes;
    *plano = novo_plano;
    Ok(contratos)
}

/// Resolve a propriedade dos Phi Ref, inclusive ciclos com origem conhecida.
/// A disponibilidade e o consumo simultâneo por aresta são provados depois
/// pelo verificador de tokens, não pela conectividade deste grafo.
fn produzir_phi(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plano: &PlanoTokens,
) -> Result<(), String> {
    let defs: HashMap<_, _> = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .map(|(v, i, _)| (*v, i))
        .collect();
    let mut candidatos = HashSet::new();
    let mut ordem = Vec::new();
    for (v, i, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
        if let Instruction::Phi { ty: declarado, .. } = i {
            if ty != declarado || plano.instrucoes.contains_key(v) {
                return Err(format!("Phi v{}: tipo ou plano incompatível", v.0));
            }
            if *ty == Type::Ref && classes.get(v).is_none_or(|c| *c == Ownership::Owned) {
                candidatos.insert(*v);
                ordem.push(*v);
            }
        }
    }
    let mut pais: HashMap<ValueId, Vec<ValueId>> = HashMap::new();
    let mut fila = VecDeque::new();
    for v in &ordem {
        let Instruction::Phi { incoming, .. } = defs[v] else {
            unreachable!()
        };
        if incoming.is_empty() {
            return Err(format!("Phi v{} sem entradas", v.0));
        }
        let mut ancora = false;
        for (_, op) in incoming {
            let mut op = op;
            let mut movimentos = HashSet::new();
            loop {
                match op {
                    Operand::Constant(Constant::Null) => {
                        ancora = true;
                        break;
                    }
                    Operand::Val(de) if candidatos.contains(de) => {
                        pais.entry(*de).or_default().push(*v);
                        break;
                    }
                    Operand::Val(de) if classes.get(de) == Some(&Ownership::Owned) => {
                        if let Some(Instruction::ArcMove { value }) = defs.get(de) {
                            if !movimentos.insert(*de) {
                                return Err(format!("Phi v{}: ciclo de moves sem origem", v.0));
                            }
                            op = value;
                        } else {
                            ancora = true;
                            break;
                        }
                    }
                    _ => {
                        return Err(format!(
                            "Phi v{}: entrada não é owned/null; copie o empréstimo na aresta",
                            v.0
                        ));
                    }
                }
            }
        }
        if ancora {
            fila.push_back(*v);
        }
    }
    let mut fundados = HashSet::new();
    while let Some(v) = fila.pop_front() {
        if fundados.insert(v) {
            if let Some(dependentes) = pais.get(&v) {
                fila.extend(dependentes);
            }
        }
    }
    for v in ordem {
        if !fundados.contains(&v) {
            return Err(format!("Phi v{} sem origem owned/null fora do ciclo", v.0));
        }
        classes.insert(v, Ownership::Owned);
    }
    Ok(())
}

fn produzir(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plano: &mut PlanoTokens,
    incluir_arc: bool,
) -> Result<HashMap<ValueId, ContratoChamadaRuntime>, String> {
    let mut contratos = HashMap::new();
    let mut fixas = HashMap::new();
    let mut ids = std::collections::HashSet::new();
    for (v, _, _) in &f.params {
        if !ids.insert(*v) {
            return Err(format!("v{} repetido", v.0));
        }
    }
    for (v, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
        if !ids.insert(*v) {
            return Err(format!("v{} repetido", v.0));
        }
        if incluir_arc {
            let fixa = match inst {
                Instruction::ArcCopy { .. }
                | Instruction::ArcMove { .. }
                | Instruction::ArcLoadStrong { .. } => Some((Ownership::Owned, Type::Ref)),
                Instruction::ArcDrop { .. } | Instruction::ArcStoreStrong { .. } => {
                    Some((Ownership::Trivial, Type::Void))
                }
                _ => None,
            };
            if let Some((classe, esperado)) = fixa {
                if *ty != esperado
                    || classes.get(v).is_some_and(|c| *c != classe)
                    || plano.instrucoes.contains_key(v)
                {
                    return Err(format!("v{}: contrato incompatível com operação ARC", v.0));
                }
                fixas.insert(*v, classe);
            }
        }
        if let Instruction::CallRuntime { ret_ty, .. } = inst {
            if ty != ret_ty {
                return Err(format!("v{}: tipo do resultado incompatível", v.0));
            }
            let c = contrato_chamada_runtime(inst).map_err(|e| format!("v{}: {e}", v.0))?;
            if classes.get(v).is_some_and(|classe| *classe != c.resultado)
                || plano.instrucoes.get(v).is_some_and(|e| *e != c.efeito)
            {
                return Err(format!(
                    "v{}: metadados conflitam com contrato runtime",
                    v.0
                ));
            }
            contratos.insert(*v, c);
        }
    }
    classes.extend(fixas);
    for (v, c) in &contratos {
        classes.insert(*v, c.resultado);
        plano.instrucoes.insert(*v, c.efeito.clone());
    }
    Ok(contratos)
}

/// Contrato semântico de uma chamada ordinária auditada.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::contrato_chamada_runtime};
/// let i = Instruction::CallRuntime { name: "dartforge_arc_collect".into(), args: vec![], ret_ty: Type::Void };
/// let c = contrato_chamada_runtime(&i)?;
/// assert!(c.invalida_borrows && c.efeito.sempre.is_empty());
/// # Ok::<(), String>(())
/// ```
#[derive(Debug)]
pub struct ContratoChamadaRuntime {
    /// Consumos de argumentos SSA, preservando multiplicidade.
    pub efeito: EfeitoTokens,
    /// Classe do resultado, proveniente da extern e não da largura i64.
    pub resultado: Ownership,
    /// Exige considerar owners persistentes internos ao runtime.
    pub retencao_persistente: bool,
    /// Exige prova separada das dependências borrowed após a chamada.
    pub invalida_borrows: bool,
}

/// Traduz uma chamada auditada para efeitos de consumo e classe do resultado.
///
/// Confere aridade e tipos declarados na chamada; o verificador HIR continua
/// responsável por SSA, dominância e tipos reais dos operandos. Não insere RC,
/// não certifica owners de slots e não resolve invalidação de empréstimos.
/// Null não tem token físico; outras referências devem estar avaliadas em SSA.
///
/// # Erros
/// Instrução não runtime, extern ausente, assinatura incompatível, referência
/// não avaliada ou retain direto, que cria token sem resultado SSA explícito.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::{contrato_chamada_runtime, Ownership}};
/// let i = Instruction::CallRuntime { name: "dartforge_arc_quadro_carregar_v1".into(),
///     args: vec![(Operand::Val(ValueId(0)), Type::I64), (Operand::Constant(Constant::Int(0)), Type::I64)],
///     ret_ty: Type::Ref };
/// assert_eq!(contrato_chamada_runtime(&i)?.resultado, Ownership::Owned);
/// # Ok::<(), String>(())
/// ```
pub fn contrato_chamada_runtime(inst: &Instruction) -> Result<ContratoChamadaRuntime, String> {
    let Instruction::CallRuntime { name, args, ret_ty } = inst else {
        return Err("contrato runtime exige CallRuntime".into());
    };
    let c = contrato(name).map_err(|e| format!("{name}: {e}"))?;
    // Retain produz um owner independente, mas a extern retorna void. A HIR
    // representa essa produção com ArcCopy; tratá-la como chamada borrowed
    // comum permitiria perder o token sem que o verificador percebesse.
    if name == "dartforge_arc_retain" {
        return Err("retain direto exige ArcCopy com resultado SSA owned".into());
    }
    if c.parametros.len() != args.len() {
        return Err(format!("{name}: aridade incompatível com ownership.tsv"));
    }
    let (ty, resultado) = match c.resultado {
        ModoResultado::Owned => (Type::Ref, Ownership::Owned),
        ModoResultado::ScalarI64 => (Type::I64, Ownership::Trivial),
        ModoResultado::ScalarI8 => (Type::I8, Ownership::Trivial),
        ModoResultado::Void => (Type::Void, Ownership::Trivial),
    };
    if *ret_ty != ty {
        return Err(format!(
            "{name}: resultado incompatível com contrato semântico"
        ));
    }
    let mut efeito = EfeitoTokens {
        pode_falhar: c.pode_falhar,
        ..Default::default()
    };
    for (n, (modo, (op, ty))) in c.parametros.iter().zip(args).enumerate() {
        let referencia = matches!(
            modo,
            ModoParametro::Borrow
                | ModoParametro::Consume
                | ModoParametro::ConsumeSuccess
                | ModoParametro::ConsumeError
        );
        if *ty != if referencia { Type::Ref } else { Type::I64 } {
            return Err(format!("{name}: tipo do argumento {n} incompatível"));
        }
        if referencia {
            match op {
                Operand::Val(v) => {
                    if *modo == ModoParametro::Consume {
                        efeito.sempre.push(*v);
                    }
                    if *modo == ModoParametro::ConsumeSuccess {
                        efeito.sucesso.push(*v);
                    }
                    if *modo == ModoParametro::ConsumeError {
                        efeito.erro.push(*v);
                    }
                }
                Operand::Constant(Constant::Null) => {}
                _ => return Err(format!("{name}: argumento Ref {n} exige SSA ou null")),
            }
        }
    }
    Ok(ContratoChamadaRuntime {
        efeito,
        resultado,
        retencao_persistente: c.retencao_persistente,
        invalida_borrows: c.invalida_borrows,
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn contratos_nao_inferem_ref_por_largura_nem_escondem_tokens() {
        let mut i = Instruction::CallRuntime {
            name: "dartforge_arc_global_receber_v1".into(),
            args: vec![
                (Operand::Val(ValueId(0)), Type::I64),
                (Operand::Val(ValueId(1)), Type::Ref),
            ],
            ret_ty: Type::Void,
        };
        let c = contrato_chamada_runtime(&i).unwrap();
        assert_eq!(c.efeito.sempre, [ValueId(1)]);
        assert!(c.retencao_persistente && c.invalida_borrows);
        if let Instruction::CallRuntime { args, .. } = &mut i {
            args[1].1 = Type::I64;
        }
        assert!(contrato_chamada_runtime(&i).is_err());
        if let Instruction::CallRuntime { args, .. } = &mut i {
            args[1] = (Operand::Constant(Constant::Null), Type::Ref);
        }
        assert!(
            contrato_chamada_runtime(&i)
                .unwrap()
                .efeito
                .sempre
                .is_empty()
        );
        for nome in ["dartforge_arc_retain", "dartforge_alocar"] {
            let i = Instruction::CallRuntime {
                name: nome.into(),
                args: vec![],
                ret_ty: Type::Void,
            };
            assert!(contrato_chamada_runtime(&i).is_err());
        }
    }

    #[test]
    fn byte_da_abi_nao_e_booleano_i1() {
        let mut i = Instruction::CallRuntime {
            name: "dartforge_arc_verificar_abi".into(),
            args: vec![(Operand::Constant(Constant::Int(1)), Type::I64)],
            ret_ty: Type::I8,
        };
        assert_eq!(
            contrato_chamada_runtime(&i).unwrap().resultado,
            Ownership::Trivial
        );
        if let Instruction::CallRuntime { ret_ty, .. } = &mut i {
            *ret_ty = Type::I1;
        }
        assert!(contrato_chamada_runtime(&i).is_err());
    }

    #[test]
    fn publicar_global_copia_owner_sem_consumir_ssa() {
        let i = Instruction::CallRuntime {
            name: "dartforge_gc_global_root".into(),
            args: vec![
                (Operand::Val(ValueId(0)), Type::I64),
                (Operand::Val(ValueId(1)), Type::Ref),
            ],
            ret_ty: Type::Void,
        };
        let c = contrato_chamada_runtime(&i).unwrap();
        assert!(c.efeito.sempre.is_empty() && c.retencao_persistente && c.invalida_borrows);
        assert_eq!(c.resultado, Ownership::Trivial);
    }

    #[test]
    fn coleta_auditada_exige_saida_excepcional() {
        let i = Instruction::CallRuntime {
            name: "dartforge_gc_collect".into(),
            args: vec![],
            ret_ty: Type::Void,
        };
        let c = contrato_chamada_runtime(&i).unwrap();
        assert!(c.efeito.pode_falhar && c.invalida_borrows);
        assert!(
            c.efeito.sempre.is_empty() && c.efeito.sucesso.is_empty() && c.efeito.erro.is_empty()
        );
    }

    #[test]
    fn produtor_e_atomico_e_produz_owned_sem_inferir_pela_largura() {
        let mut f = Function {
            symbol: "f".into(),
            name: "f".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(0),
                    Instruction::CallRuntime {
                        name: "dartforge_arc_quadro_carregar_v1".into(),
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
        let mut plano = PlanoTokens::default();
        f.blocks[0].instructions.push((
            ValueId(1),
            Instruction::CallRuntime {
                name: "extern_sem_contrato".into(),
                args: vec![],
                ret_ty: Type::Void,
            },
            Type::Void,
        ));
        assert!(produzir_contratos_runtime(&f, &mut classes, &mut plano).is_err());
        assert!(classes.is_empty() && plano.instrucoes.is_empty());
        f.blocks[0].instructions.pop();
        produzir_contratos_runtime(&f, &mut classes, &mut plano).unwrap();
        assert_eq!(classes[&ValueId(0)], Ownership::Owned);
        classes.insert(ValueId(0), Ownership::Trivial);
        let antes = plano.instrucoes.clone();
        assert!(produzir_contratos_runtime(&f, &mut classes, &mut plano).is_err());
        assert_eq!(classes[&ValueId(0)], Ownership::Trivial);
        assert_eq!(plano.instrucoes, antes);
    }
}
