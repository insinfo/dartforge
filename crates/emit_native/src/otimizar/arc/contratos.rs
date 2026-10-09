//! Tradução das externs auditadas para o plano de tokens da HIR.
//! Não presume contratos para símbolos ausentes nem certifica proveniência/borrows.

use super::{EfeitoTokens, Ownership};
use crate::hir::*;
use dartforge_runtime::ownership::{ModoParametro, ModoResultado, contrato};

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
    let mut efeito = EfeitoTokens::default();
    for (n, (modo, (op, ty))) in c.parametros.iter().zip(args).enumerate() {
        let referencia = matches!(modo, ModoParametro::Borrow | ModoParametro::Consume);
        if *ty != if referencia { Type::Ref } else { Type::I64 } {
            return Err(format!("{name}: tipo do argumento {n} incompatível"));
        }
        if referencia {
            match op {
                Operand::Val(v) => {
                    if *modo == ModoParametro::Consume {
                        efeito.sempre.push(*v);
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
}
