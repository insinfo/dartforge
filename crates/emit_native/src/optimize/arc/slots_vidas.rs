//! Vida dos pares quadro/índice em um conjunto fechado de corpos intermediários.
//! Confere saídas explícitas do CFG; unwind implícito e tokens exigem outro gate.

use super::super::operandos::{operandos, operandos_do_terminador};
use super::{RascunhoSlotsRefArc, auditar_slots_ref_compartilhados};
use crate::hir::*;
use std::collections::{HashMap, HashSet, VecDeque};

/// Verifica origem, repasse e vida explícita dos slots compartilhados baixados.
///
/// Reconstrói os papéis a partir dos endereços auditados do módulo original.
/// Confere assinaturas atuais, capacidade e atividade de quadros locais,
/// associação do índice importado, ausência de escape e todos os chamadores.
/// Ciclos de repasse precisam de uma origem local alcançável. O callee pode
/// ler/copiar o slot, mas não fechar, mover ou publicar o quadro do chamador.
/// Não altera o original nem o rascunho e não publica certificado de emissão.
/// Não verifica tokens Ref nem unwind estrangeiro fora das arestas do CFG.
/// Tracing exige corpos copiados sem transformações ARC.
///
/// # Erros
/// Metadados incompatíveis com a origem, assinatura/tipo inválido, escape,
/// repasse sem origem, índice errado, quadro fechado ou cleanup fora de LIFO.
///
/// ```
/// use dartforge_emit_native::{hir::Module, optimize::arc::*};
/// let mut m = Module::new(); m.memoria_arc = true;
/// let r = baixar_slots_ref_compartilhados(&m)?;
/// verificar_vidas_slots_ref_cfg(&m, &r)?;
/// # Ok::<(), String>(())
/// ```
pub fn verificar_vidas_slots_ref_cfg(m: &Module, r: &RascunhoSlotsRefArc) -> Result<(), String> {
    if !m.memoria_arc {
        if !r.emprestados.is_empty()
            || !r.locais.is_empty()
            || !r.simbolos.is_empty()
            || format!("{:?}", m.functions) != format!("{:?}", r.funcoes)
        {
            return Err("slots ARC: tracing não admite transformação de capturas".into());
        }
        return Ok(());
    }
    let origens = auditar_slots_ref_compartilhados(m)?;
    let mut novas = HashMap::new();
    for f in &r.funcoes {
        if novas.insert(f.symbol.as_str(), f).is_some() {
            return Err("slots ARC: símbolo novo repetido".into());
        }
    }
    let mut esperados = HashSet::new();
    let mut imports = HashSet::new();
    let mut locais = HashSet::new();
    let mut renomeados = HashSet::new();
    for f in &m.functions {
        let nome = r.simbolos.get(&f.symbol).unwrap_or(&f.symbol);
        if !esperados.insert(nome.as_str()) {
            return Err("slots ARC: colisão entre símbolos novos".into());
        }
        let novo = novas
            .get(nome.as_str())
            .ok_or_else(|| format!("slots ARC: corpo ausente {nome}"))?;
        let ids = origens.get(&f.symbol);
        let capturas: HashSet<_> = f
            .params
            .iter()
            .filter_map(|(v, _, _)| ids.is_some_and(|ss| ss.contains(v)).then_some(*v))
            .collect();
        let defs: HashSet<_> = f
            .params
            .iter()
            .map(|(v, _, _)| *v)
            .chain(
                f.blocks
                    .iter()
                    .flat_map(|b| &b.instructions)
                    .map(|(v, _, _)| *v),
            )
            .collect();
        let mut assinatura = Vec::new();
        if !capturas.is_empty() {
            imports.insert(nome.as_str());
            renomeados.insert(f.symbol.as_str());
            if nome == &f.symbol {
                return Err(format!("slots ARC: ABI nova reutiliza símbolo {nome}"));
            }
            let pares = r
                .emprestados
                .get(nome)
                .ok_or_else(|| format!("slots ARC: pares ausentes de {nome}"))?;
            if pares.keys().copied().collect::<HashSet<_>>() != capturas {
                return Err(format!(
                    "slots ARC: parâmetros não correspondem às capturas de {nome}"
                ));
            }
            for (v, _, ty) in &f.params {
                if let Some(i) = pares.get(v) {
                    if defs.contains(i) {
                        return Err(format!(
                            "slots ARC: índice de {nome} reutiliza SSA da fonte"
                        ));
                    }
                    assinatura.extend([(*v, Type::I64), (*i, Type::I64)]);
                } else {
                    assinatura.push((*v, *ty));
                }
            }
        } else {
            assinatura.extend(f.params.iter().map(|(v, _, t)| (*v, *t)));
        }
        if assinatura
            != novo
                .params
                .iter()
                .map(|(v, _, t)| (*v, *t))
                .collect::<Vec<_>>()
            || novo.return_ty != f.return_ty
        {
            return Err(format!(
                "slots ARC: assinatura diferente da origem de {nome}"
            ));
        }
        let quantidade = f
            .blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .filter(|(v, i, _)| {
                ids.is_some_and(|ss| ss.contains(v)) && matches!(i, Instruction::Alloca(Type::Ref))
            })
            .count();
        if quantidade != 0 {
            locais.insert(nome.as_str());
            let &(q, n) = r
                .locais
                .get(nome)
                .ok_or_else(|| format!("slots ARC: quadro local ausente de {nome}"))?;
            if n != quantidade
                || defs.contains(&q)
                || !novo.blocks.first().is_some_and(|b| {
                    b.instructions.iter().any(|(v, i, t)| {
                        *v == q
                            && *t == Type::I64
                            && matches!(i, Instruction::CallRuntime { name, args, ret_ty }
                    if name == "dartforge_arc_quadro_abrir_v1" && *ret_ty == Type::I64
                    && args == &vec![(Operand::Constant(Constant::Int(n as i64)), Type::I64)])
                    })
                })
            {
                return Err(format!(
                    "slots ARC: fábrica/capacidade do quadro local inválida em {nome}"
                ));
            }
        }
    }
    if novas.keys().copied().collect::<HashSet<_>>() != esperados
        || r.emprestados
            .keys()
            .map(String::as_str)
            .collect::<HashSet<_>>()
            != imports
        || r.locais.keys().map(String::as_str).collect::<HashSet<_>>() != locais
        || r.simbolos
            .keys()
            .map(String::as_str)
            .collect::<HashSet<_>>()
            != renomeados
    {
        return Err("slots ARC: inventário de corpos/papéis incompatível com a origem".into());
    }
    let mut alvos = HashMap::new();
    for (nome, pares) in &r.emprestados {
        let f = novas[nome.as_str()];
        let posicoes: HashMap<_, _> = f
            .params
            .iter()
            .enumerate()
            .map(|(p, (v, _, _))| (*v, p))
            .collect();
        let mut ordem: Vec<_> = pares
            .iter()
            .map(|(q, i)| (posicoes[q], posicoes[i]))
            .collect();
        ordem.sort_unstable();
        alvos.insert(nome.clone(), ordem);
    }
    let mut ancorados = HashSet::<(String, ValueId)>::new();
    let mut arestas = HashMap::<(String, ValueId), Vec<(String, ValueId)>>::new();
    for f in &r.funcoes {
        // Um endereço materializado reabre a fronteira mesmo se o original
        // era fechado. Verificar também funções sem slots locais/importados.
        for b in &f.blocks {
            let mut aberto = false;
            let mut constante = |o: &Operand| {
                aberto |=
                    matches!(o, Operand::Constant(Constant::Funcao(s)) if alvos.contains_key(s));
            };
            for (_, i, _) in &b.instructions {
                match i {
                    Instruction::Const(Constant::Funcao(s))
                    | Instruction::TearOff { code_symbol: s }
                    | Instruction::AllocClosure { code_symbol: s, .. } => {
                        if alvos.contains_key(s) {
                            return Err("slots ARC: entrada emprestada materializada".into());
                        }
                    }
                    Instruction::AllocClosureTipada {
                        code_symbol,
                        tipado,
                        ..
                    } if alvos.contains_key(code_symbol) || alvos.contains_key(tipado) => {
                        return Err("slots ARC: entrada emprestada materializada".into());
                    }
                    Instruction::TabelaDeFuncoes(ss)
                        if ss.iter().any(|s| alvos.contains_key(s)) =>
                    {
                        return Err("slots ARC: entrada emprestada materializada".into());
                    }
                    _ => {}
                }
                operandos(i, &mut constante);
            }
            operandos_do_terminador(&b.terminator, &mut constante);
            if aberto {
                return Err("slots ARC: entrada emprestada materializada".into());
            }
        }
        if !r.emprestados.contains_key(&f.symbol) && !r.locais.contains_key(&f.symbol)
            && !f.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i, Instruction::CallStatic { symbol, .. } if alvos.contains_key(symbol))) { continue; }
        super::ssa::verificar(f)?;
        let vazio = HashMap::new();
        let pares = r.emprestados.get(&f.symbol).unwrap_or(&vazio);
        let entradas = super::quadros::analisar_compartilhados(f, pares, &alvos)?;
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
        let tipo = |op: &Operand| match op {
            Operand::Val(v) => tipos.get(v).copied(),
            Operand::Constant(Constant::Null | Constant::String(_)) => Some(Type::Ref),
            Operand::Constant(Constant::Int(_)) => Some(Type::I64),
            Operand::Constant(Constant::Double(_)) => Some(Type::F64),
            Operand::Constant(Constant::Bool(_)) => Some(Type::I1),
            _ => None,
        };
        let proprios: HashSet<_> = f.blocks.iter().flat_map(|b| &b.instructions).filter_map(|(v, i, _)|
            matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_arc_quadro_abrir_v1").then_some(*v)).collect();
        let nativos: HashSet<_> = proprios
            .iter()
            .copied()
            .chain(pares.keys().copied())
            .chain(pares.values().copied())
            .collect();
        for b in &f.blocks {
            for (v, inst, ty) in &b.instructions {
                let mut permitidos = Vec::new();
                let mut permitir = |o: &Operand| {
                    if let Operand::Val(v) = o
                        && nativos.contains(v)
                    {
                        permitidos.push(*v);
                    }
                };
                match inst {
                    Instruction::CallRuntime { name, args, ret_ty }
                        if name.starts_with("dartforge_arc_quadro_") =>
                    {
                        super::contratos::contrato_chamada_runtime(inst)?;
                        if *ty != *ret_ty || args.iter().any(|(a, t)| tipo(a) != Some(*t)) {
                            return Err(format!(
                                "slots ARC: tipos reais da operação em {} v{}",
                                f.symbol, v.0
                            ));
                        }
                        if name != "dartforge_arc_quadro_abrir_v1" {
                            permitir(&args[0].0);
                            if let Operand::Val(q) = args[0].0
                                && let Some(i) = pares.get(&q)
                            {
                                if args.get(1).is_some_and(|a| a.0 == Operand::Val(*i)) {
                                    permitir(&args[1].0);
                                }
                            }
                            if name == "dartforge_arc_quadro_mover_v1" {
                                permitir(&args[2].0);
                            }
                        }
                    }
                    Instruction::ArcLoadStrong {
                        slot: SlotForte::Quadro { quadro, .. },
                    }
                    | Instruction::ArcStoreStrong {
                        slot: SlotForte::Quadro { quadro, .. },
                        ..
                    } => permitir(quadro),
                    Instruction::CallStatic {
                        symbol,
                        args,
                        ret_ty,
                    } if alvos.contains_key(symbol) => {
                        if !entradas.contains_key(&b.id) {
                            return Err("slots ARC: repasse em bloco sem origem alcançável".into());
                        }
                        let callee = novas[symbol.as_str()];
                        if args.len() != callee.params.len()
                            || *ty != *ret_ty
                            || *ret_ty != callee.return_ty
                            || args
                                .iter()
                                .zip(&callee.params)
                                .any(|(a, (_, _, t))| tipo(a) != Some(*t))
                        {
                            return Err(format!(
                                "slots ARC: assinatura/tipos reais da chamada de {} para {symbol}",
                                f.symbol
                            ));
                        }
                        for &(qi, ii) in &alvos[symbol] {
                            permitir(&args[qi]);
                            permitir(&args[ii]);
                            let destino = (symbol.clone(), callee.params[qi].0);
                            let Operand::Val(q) = args[qi] else {
                                return Err("slots ARC: quadro de chamada não SSA".into());
                            };
                            if pares.contains_key(&q) {
                                arestas
                                    .entry((f.symbol.clone(), q))
                                    .or_default()
                                    .push(destino);
                            } else if proprios.contains(&q) && entradas.contains_key(&b.id) {
                                ancorados.insert(destino);
                            } else {
                                return Err("slots ARC: repasse sem origem local alcançável".into());
                            }
                        }
                    }
                    _ => {}
                }
                let mut usados = Vec::new();
                operandos(inst, &mut |o| {
                    if let Operand::Val(v) = o
                        && nativos.contains(v)
                    {
                        usados.push(*v);
                    }
                });
                if usados != permitidos {
                    return Err(format!(
                        "slots ARC: identidade nativa escapa/ganha alias em {} v{}",
                        f.symbol, v.0
                    ));
                }
            }
            let mut escapa = false;
            operandos_do_terminador(&b.terminator, &mut |o| {
                escapa |= matches!(o, Operand::Val(v) if nativos.contains(v));
            });
            if escapa {
                return Err(format!(
                    "slots ARC: identidade nativa escapa no terminador de {}",
                    f.symbol
                ));
            }
        }
    }
    let mut fila: VecDeque<_> = ancorados.iter().cloned().collect();
    while let Some(origem) = fila.pop_front() {
        for destino in arestas.get(&origem).into_iter().flatten() {
            if ancorados.insert(destino.clone()) {
                fila.push_back(destino.clone());
            }
        }
    }
    if r.emprestados
        .iter()
        .any(|(s, ps)| ps.keys().any(|q| !ancorados.contains(&(s.clone(), *q))))
    {
        return Err("slots ARC: ciclo de quadros emprestados sem origem local alcançável".into());
    }
    Ok(())
}

#[cfg(test)]
pub(super) mod testes {
    use super::super::baixar_slots_ref_compartilhados;
    use super::*;

    fn v(n: u32) -> Operand {
        Operand::Val(ValueId(n))
    }
    fn call(nome: &str, args: Vec<Operand>) -> Instruction {
        Instruction::CallStatic {
            symbol: nome.into(),
            args,
            ret_ty: Type::Void,
        }
    }
    fn f(nome: &str, params: bool, instructions: Vec<(ValueId, Instruction, Type)>) -> Function {
        Function {
            symbol: nome.into(),
            name: nome.into(),
            depuracao: None,
            params: if params {
                vec![
                    (ValueId(0), "a".into(), Type::Ptr),
                    (ValueId(1), "b".into(), Type::Ptr),
                ]
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
    pub(in crate::optimize::arc) fn origem() -> Module {
        let mut m = Module::new();
        m.memoria_arc = true;
        m.functions = vec![
            f(
                "raiz",
                false,
                vec![
                    (ValueId(0), Instruction::Alloca(Type::Ref), Type::Ptr),
                    (ValueId(1), Instruction::Alloca(Type::Ref), Type::Ptr),
                    (
                        ValueId(2),
                        Instruction::Store {
                            ptr: v(0),
                            val: Operand::Constant(Constant::Null),
                        },
                        Type::Void,
                    ),
                    (
                        ValueId(3),
                        Instruction::Store {
                            ptr: v(1),
                            val: Operand::Constant(Constant::Null),
                        },
                        Type::Void,
                    ),
                    (ValueId(4), call("a", vec![v(0), v(1)]), Type::Void),
                ],
            ),
            f(
                "a",
                true,
                vec![
                    (
                        ValueId(2),
                        Instruction::Load {
                            ptr: v(0),
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(3),
                        Instruction::Store {
                            ptr: v(1),
                            val: v(2),
                        },
                        Type::Void,
                    ),
                    (ValueId(4), call("b", vec![v(0), v(1)]), Type::Void),
                ],
            ),
            f(
                "b",
                true,
                vec![(ValueId(2), call("a", vec![v(0), v(1)]), Type::Void)],
            ),
        ];
        m
    }

    #[test]
    fn pares_recursivos_ancorados_sao_verificados_sem_publicar_ownership() {
        let m = origem();
        let mut r = baixar_slots_ref_compartilhados(&m).unwrap();
        let antes = format!("{r:?}");
        verificar_vidas_slots_ref_cfg(&m, &r).unwrap();
        assert_eq!(format!("{r:?}"), antes);
        r.funcoes.reverse();
        verificar_vidas_slots_ref_cfg(&m, &r).unwrap();
        // A pilha de cleanup de um callee não contém quadros emprestados.
        let a = r
            .funcoes
            .iter()
            .find(|f| f.symbol == r.simbolos["a"])
            .unwrap();
        let alvos = HashMap::new();
        let pilhas =
            super::super::quadros::analisar_compartilhados(a, &r.emprestados[&a.symbol], &alvos)
                .unwrap();
        assert!(pilhas.values().all(Vec::is_empty));
    }

    #[test]
    fn rejeita_quadro_fechado_ou_indice_fabricado_mesmo_com_i64_correto() {
        for caso in 0..4 {
            let m = origem();
            let mut r = baixar_slots_ref_compartilhados(&m).unwrap();
            let b = &mut r.funcoes[0].blocks[0];
            if caso == 0 {
                let fechar = b.instructions.pop().unwrap();
                let p = b
                    .instructions
                    .iter()
                    .position(|(_, i, _)| matches!(i, Instruction::CallStatic { .. }))
                    .unwrap();
                b.instructions.insert(p, fechar);
            } else if caso == 3 {
                b.instructions.pop();
            } else {
                let Instruction::CallStatic { args, .. } = &mut b
                    .instructions
                    .iter_mut()
                    .find(|(_, i, _)| matches!(i, Instruction::CallStatic { .. }))
                    .unwrap()
                    .1
                else {
                    unreachable!()
                };
                if caso == 1 {
                    args[0] = Operand::Constant(Constant::Int(123));
                } else {
                    args[1] = Operand::Constant(Constant::Int(2));
                }
            }
            let antes = format!("{r:?}");
            assert!(
                verificar_vidas_slots_ref_cfg(&m, &r).is_err(),
                "caso {caso}"
            );
            assert_eq!(format!("{r:?}"), antes);
        }
    }

    #[test]
    fn callee_nao_fecha_quadro_nem_mistura_indices_de_capturas() {
        for caso in 0..3 {
            let m = origem();
            let mut r = baixar_slots_ref_compartilhados(&m).unwrap();
            let a = &mut r.funcoes[1];
            match caso {
                0 => a.blocks[0].instructions.push((
                    ValueId(100),
                    Instruction::CallRuntime {
                        name: "dartforge_arc_quadro_fechar_v1".into(),
                        args: vec![(v(0), Type::I64)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                )),
                1 => {
                    let Instruction::CallStatic { args, .. } =
                        &mut a.blocks[0].instructions.last_mut().unwrap().1
                    else {
                        unreachable!()
                    };
                    args[1] = args[3].clone();
                }
                _ => {
                    let Instruction::CallRuntime { args, .. } = &mut a.blocks[0].instructions[0].1
                    else {
                        unreachable!()
                    };
                    args[1].0 = Operand::Constant(Constant::Int(0));
                }
            }
            assert!(
                verificar_vidas_slots_ref_cfg(&m, &r).is_err(),
                "caso {caso}"
            );
        }
    }

    #[test]
    fn identidade_nativa_nao_vira_valor_dart_alias_ou_endereco_publicado() {
        for caso in 0..4 {
            let m = origem();
            let mut r = baixar_slots_ref_compartilhados(&m).unwrap();
            let destino = r.simbolos["b"].clone();
            let a = &mut r.funcoes[1];
            match caso {
                0 => a.blocks[0].instructions.push((
                    ValueId(100),
                    Instruction::Bitcast {
                        op: v(0),
                        to: Type::I64,
                    },
                    Type::I64,
                )),
                1 => a.blocks[0].terminator = Terminator::Return(Some(v(0))),
                2 => a.blocks[0].instructions.push((
                    ValueId(100),
                    Instruction::CallRuntime {
                        name: "opaco".into(),
                        args: vec![(v(0), Type::I64)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                )),
                _ => a.blocks[0].instructions.push((
                    ValueId(100),
                    Instruction::Const(Constant::Funcao(destino)),
                    Type::Ptr,
                )),
            }
            assert!(
                verificar_vidas_slots_ref_cfg(&m, &r).is_err(),
                "caso {caso}"
            );
        }
    }

    #[test]
    fn ciclos_precisam_de_origem_local_alcancavel_no_corpo_atual() {
        let m = origem();
        let mut r = baixar_slots_ref_compartilhados(&m).unwrap();
        r.funcoes[0].blocks[0]
            .instructions
            .retain(|(_, i, _)| !matches!(i, Instruction::CallStatic { .. }));
        assert!(
            verificar_vidas_slots_ref_cfg(&m, &r)
                .unwrap_err()
                .contains("sem origem local alcançável")
        );
    }

    #[test]
    fn papeis_nao_sao_inferidos_pela_largura_ou_por_metadados_fabricados() {
        for caso in 0..3 {
            let m = origem();
            let mut r = baixar_slots_ref_compartilhados(&m).unwrap();
            match caso {
                0 => {
                    r.emprestados
                        .get_mut(&r.simbolos["a"])
                        .unwrap()
                        .insert(ValueId(0), ValueId(1));
                }
                1 => r.locais.get_mut("raiz").unwrap().1 = 1,
                _ => {
                    r.emprestados
                        .get_mut(&r.simbolos["a"])
                        .unwrap()
                        .remove(&ValueId(0));
                }
            }
            assert!(
                verificar_vidas_slots_ref_cfg(&m, &r).is_err(),
                "caso {caso}"
            );
        }
        let mut m = origem();
        m.memoria_arc = false;
        let mut r = baixar_slots_ref_compartilhados(&m).unwrap();
        verificar_vidas_slots_ref_cfg(&m, &r).unwrap();
        r.locais.insert("raiz".into(), (ValueId(0), 1));
        assert!(verificar_vidas_slots_ref_cfg(&m, &r).is_err());
    }
}
