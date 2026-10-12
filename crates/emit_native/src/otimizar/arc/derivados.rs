//! Endereços derivados de listas: os dados de uma lista tipada
//! (`@df.tipada_dados`) e o armazenamento de uma lista do núcleo
//! (`@df.nucleo_armazenamento`). Cada acesso por eles ganha, logo depois,
//! `@df.manter_vivo(lista)`: a lista fica emprestada e viva até ali, e o ARC
//! põe o drop depois do último acesso. Os dados de uma lista tipada não
//! mudam enquanto ela vive; o armazenamento do núcleo muda quando a lista
//! cresce, então nenhum caminho da leitura dele ao acesso passa por uma
//! barreira. Não prova limites nem a forma dos elementos (o lowering confere).

use super::super::cfg::Cfg;
use super::super::operandos::{maiores_ids, operandos, operandos_do_terminador};
use crate::hir::*;
use std::collections::{HashMap, HashSet};

/// O uso que segura a lista depois de um acesso pelo endereço derivado.
pub(crate) const MANTER_VIVO: &str = "df.manter_vivo";

#[derive(Clone, Copy, PartialEq)]
struct Origem {
    /// A lista da qual o endereço deriva.
    lista: ValueId,
    /// A leitura do endereço (o ajudante).
    raiz: ValueId,
    /// Armazenamento do núcleo (muda quando a lista cresce).
    nucleo: bool,
}

fn origens(f: &Function) -> HashMap<ValueId, Origem> {
    let mut der = HashMap::new();
    loop {
        let antes = der.len();
        for (v, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if der.contains_key(v) {
                continue;
            }
            let o = match inst {
                Instruction::CallRuntime { name, args, .. }
                    if matches!(name.as_str(), "df.tipada_dados" | "df.nucleo_armazenamento") =>
                {
                    match args.as_slice() {
                        [(Operand::Val(lista), Type::Ref)] => {
                            Some(Origem { lista: *lista, raiz: *v, nucleo: name == "df.nucleo_armazenamento" })
                        }
                        _ => None,
                    }
                }
                Instruction::Add(a, b) => [a, b].into_iter().find_map(|o| match o {
                    Operand::Val(x) => der.get(x).copied(),
                    _ => None,
                }),
                Instruction::Sub(Operand::Val(a), _) => der.get(a).copied(),
                _ => None,
            };
            if let Some(o) = o {
                der.insert(*v, o);
            }
        }
        if der.len() == antes {
            return der;
        }
    }
}

/// Os valores que são endereços derivados de listas.
pub(super) fn enderecos(f: &Function) -> HashSet<ValueId> {
    origens(f).into_keys().collect()
}

/// O endereço lido ou gravado por `inst`, se é um acesso à memória.
fn endereco(inst: &Instruction) -> Option<&Operand> {
    match inst {
        Instruction::CargaNativa { endereco, .. } | Instruction::GravacaoNativa { endereco, .. } => Some(endereco),
        Instruction::Simd { op: OpSimd::Carrega | OpSimd::Grava, args } => args.first(),
        Instruction::CallRuntime { name, args, .. } if name == "df.nucleo_forma" => args.first().map(|(o, _)| o),
        _ => None,
    }
}

fn segura(inst: Option<&Instruction>, lista: ValueId) -> bool {
    matches!(inst, Some(Instruction::CallRuntime { name, args, ret_ty: Type::Void })
        if name == MANTER_VIVO && matches!(args.as_slice(), [(Operand::Val(l), Type::Ref)] if *l == lista))
}

fn manter_vivo(lista: ValueId) -> Instruction {
    Instruction::CallRuntime {
        name: MANTER_VIVO.into(),
        args: vec![(Operand::Val(lista), Type::Ref)],
        ret_ty: Type::Void,
    }
}

/// Pode trocar o armazenamento de uma lista do núcleo (chama Dart ou o
/// runtime sem contrato que o exclua).
fn barreira(inst: &Instruction) -> bool {
    match inst {
        Instruction::CallRuntime { name, .. } => {
            name != MANTER_VIVO
                && super::contrato(name).map_or(true, |c| c.invalida_borrows || c.chama_dart)
        }
        Instruction::CallStatic { .. }
        | Instruction::CallSeletor { .. }
        | Instruction::CallSeletorRepasse { .. }
        | Instruction::CallClosure { .. }
        | Instruction::CallClosureRepasse { .. }
        | Instruction::ChamadaTipada { .. }
        | Instruction::ChamadaNativa { .. }
        | Instruction::ChamadaNativaComposta { .. }
        | Instruction::SetListElement { .. } => true,
        _ => false,
    }
}

/// Põe `@df.manter_vivo(lista)` depois de cada acesso por endereço derivado
/// que ainda não o tem (repetir não muda nada) e confere a forma resultante
/// ([`verificar`]).
pub(super) fn manter_vivas(f: &mut Function) -> Result<(), String> {
    let der = origens(f);
    if der.is_empty() {
        return Ok(());
    }
    let mut proximo = maiores_ids(f).0;
    for b in &mut f.blocks {
        let mut novas = Vec::with_capacity(b.instructions.len());
        let antigas = std::mem::take(&mut b.instructions);
        for (k, (v, inst, ty)) in antigas.iter().enumerate() {
            let lista = match endereco(inst) {
                Some(Operand::Val(d)) => der.get(d).map(|o| o.lista),
                _ => None,
            }
            .filter(|l| !segura(antigas.get(k + 1).map(|(_, i, _)| i), *l));
            novas.push((*v, inst.clone(), *ty));
            if let Some(lista) = lista {
                proximo = proximo.checked_add(1).ok_or("IDs SSA esgotados ao manter listas vivas")?;
                novas.push((ValueId(proximo), manter_vivo(lista), Type::Void));
            }
        }
        b.instructions = novas;
    }
    verificar(f)
}

/// As leituras de armazenamento do núcleo ainda válidas na entrada de cada
/// bloco: valem em todo caminho desde a leitura, sem barreira no meio.
fn armazenamentos_validos(f: &Function, der: &HashMap<ValueId, Origem>) -> Vec<HashSet<ValueId>> {
    let raizes: HashSet<ValueId> = der.values().filter(|o| o.nucleo).map(|o| o.raiz).collect();
    let cfg = Cfg::novo(f);
    let transferir = |bi: usize, mut estado: HashSet<ValueId>| {
        for (v, inst, _) in &f.blocks[bi].instructions {
            if barreira(inst) {
                estado.clear();
            }
            if raizes.contains(v) {
                estado.insert(*v);
            }
        }
        estado
    };
    let mut entradas = vec![raizes.clone(); f.blocks.len()];
    let mut saidas = vec![raizes.clone(); f.blocks.len()];
    if let Some(&e) = cfg.rpo.first() {
        entradas[e].clear();
    }
    loop {
        let mut mudou = false;
        for (k, &bi) in cfg.rpo.iter().enumerate() {
            let mut estado = if k == 0 { HashSet::new() } else { raizes.clone() };
            for &p in &cfg.predecessores[bi] {
                if cfg.idom[p] != usize::MAX {
                    estado.retain(|r| saidas[p].contains(r));
                }
            }
            let saida = transferir(bi, estado.clone());
            mudou |= entradas[bi] != estado || saidas[bi] != saida;
            entradas[bi] = estado;
            saidas[bi] = saida;
        }
        if !mudou {
            return entradas;
        }
    }
}

/// Confere que o endereço derivado só serve a derivações e acessos, que
/// cada acesso é seguido de `@df.manter_vivo` da lista dele, e que nenhum
/// caminho da leitura do armazenamento do núcleo ao acesso tem barreira.
pub(super) fn verificar(f: &Function) -> Result<(), String> {
    let der = origens(f);
    if der.is_empty() {
        return Ok(());
    }
    let validos = armazenamentos_validos(f, &der);
    for (bi, b) in f.blocks.iter().enumerate() {
        let mut estado = validos[bi].clone();
        for (i, (v, inst, _)) in b.instructions.iter().enumerate() {
            let acesso = endereco(inst).and_then(|o| match o {
                Operand::Val(d) => der.get(d).copied(),
                _ => None,
            });
            let mut erro = None;
            let derivacao = der.get(v).is_some_and(|o| o.raiz != *v);
            let mut ocorrencias = 0;
            operandos(inst, &mut |o| {
                if let Operand::Val(x) = o
                    && der.contains_key(x)
                {
                    ocorrencias += 1;
                    if !derivacao && acesso.is_none() {
                        erro = Some(*x);
                    }
                }
            });
            // Num acesso, o endereço é o único operando derivado.
            if acesso.is_some() && ocorrencias != 1 {
                erro = Some(*v);
            }
            if let Some(x) = erro {
                return Err(format!("v{}: endereço derivado de lista v{} escapa em {}", v.0, x.0, f.symbol));
            }
            if barreira(inst) {
                estado.clear();
            }
            if der.get(v).is_some_and(|o| o.nucleo && o.raiz == *v) {
                estado.insert(*v);
            }
            let Some(o) = acesso else { continue };
            let seguinte = b.instructions.get(i + 1).map(|(_, inst, _)| inst);
            if !segura(seguinte, o.lista) {
                return Err(format!("v{}: acesso por endereço derivado sem manter v{} viva em {}", v.0, o.lista.0, f.symbol));
            }
            if o.nucleo && !estado.contains(&o.raiz) {
                return Err(format!("v{}: barreira entre a leitura do armazenamento de v{} e o acesso em {}", v.0, o.lista.0, f.symbol));
            }
        }
        let mut escapa = None;
        operandos_do_terminador(&b.terminator, &mut |o| {
            if let Operand::Val(x) = o
                && der.contains_key(x)
            {
                escapa = Some(*x);
            }
        });
        if let Some(x) = escapa {
            return Err(format!("endereço derivado de lista v{} escapa no terminador de b{} em {}", x.0, b.id.0, f.symbol));
        }
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    fn rt(nome: &str, args: Vec<(Operand, Type)>, ret_ty: Type) -> Instruction {
        Instruction::CallRuntime { name: nome.into(), args, ret_ty }
    }

    fn funcao(instrucoes: Vec<(ValueId, Instruction, Type)>) -> Function {
        Function {
            symbol: "f".into(),
            name: "f".into(),
            depuracao: None,
            params: vec![(ValueId(0), "l".into(), Type::Ref), (ValueId(1), "i".into(), Type::I64)],
            return_ty: Type::Void,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: instrucoes, terminator: Terminator::Return(None) }],
        }
    }

    fn val(n: u32) -> Operand {
        Operand::Val(ValueId(n))
    }

    #[test]
    fn acesso_a_tipada_ganha_manter_vivo_da_lista() {
        let mut f = funcao(vec![
            (ValueId(2), rt("df.tipada_dados", vec![(val(0), Type::Ref)], Type::I64), Type::I64),
            (ValueId(3), Instruction::CargaNativa { endereco: val(2), indice: val(1), tipo: TipoC::F64 }, Type::F64),
            (ValueId(4), Instruction::Add(val(2), Operand::Constant(Constant::Int(8))), Type::I64),
            (ValueId(5), Instruction::GravacaoNativa { endereco: val(4), indice: val(1), tipo: TipoC::F64, valor: val(3) }, Type::Void),
        ]);
        manter_vivas(&mut f).unwrap();
        let nomes: Vec<_> = f.blocks[0].instructions.iter().map(|(v, i, _)| match i {
            Instruction::CallRuntime { name, args, .. } if name == MANTER_VIVO => {
                assert_eq!(args, &vec![(val(0), Type::Ref)]);
                format!("manter{}", v.0)
            }
            _ => format!("v{}", v.0),
        }).collect();
        assert_eq!(nomes, ["v2", "v3", "manter6", "v4", "v5", "manter7"]);
        // Repetir não muda nada.
        let antes = format!("{f:?}");
        manter_vivas(&mut f).unwrap();
        assert_eq!(format!("{f:?}"), antes);
        // Sem o manter_vivo depois do acesso, recusa.
        f.blocks[0].instructions.remove(2);
        assert!(verificar(&f).unwrap_err().contains("sem manter"));
    }

    #[test]
    fn endereco_derivado_nao_escapa_nem_atravessa_barreira_do_nucleo() {
        // Passar o endereço a uma chamada.
        let mut f = funcao(vec![
            (ValueId(2), rt("df.tipada_dados", vec![(val(0), Type::Ref)], Type::I64), Type::I64),
            (ValueId(3), Instruction::CallStatic { symbol: "g".into(), args: vec![val(2)], ret_ty: Type::Void }, Type::Void),
        ]);
        assert!(manter_vivas(&mut f).unwrap_err().contains("escapa"));
        // Uma chamada entre a leitura do armazenamento e o acesso.
        let mut f = funcao(vec![
            (ValueId(2), rt("df.nucleo_armazenamento", vec![(val(0), Type::Ref)], Type::I64), Type::I64),
            (ValueId(3), Instruction::CallStatic { symbol: "g".into(), args: vec![], ret_ty: Type::Void }, Type::Void),
            (ValueId(4), Instruction::CargaNativa { endereco: val(2), indice: val(1), tipo: TipoC::I64 }, Type::I64),
        ]);
        assert!(manter_vivas(&mut f).unwrap_err().contains("barreira"));
        // Sem barreira, passa; a forma também é um acesso.
        let mut f = funcao(vec![
            (ValueId(2), rt("df.nucleo_armazenamento", vec![(val(0), Type::Ref)], Type::I64), Type::I64),
            (ValueId(3), rt("df.nucleo_forma", vec![(val(2), Type::I64)], Type::I64), Type::I64),
            (ValueId(4), Instruction::Add(val(2), Operand::Constant(Constant::Int(22))), Type::I64),
            (ValueId(5), Instruction::CargaNativa { endereco: val(4), indice: val(1), tipo: TipoC::I64 }, Type::I64),
        ]);
        manter_vivas(&mut f).unwrap();
        assert_eq!(f.blocks[0].instructions.len(), 6);
    }
}
