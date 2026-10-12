//! Substituição escalar dos objetos que não escapam.
//!
//! Um objeto criado na função (`dartforge_object_new`) cujo único uso é
//! ler e gravar os próprios campos por índice constante
//! (`dartforge_object_get`/`dartforge_object_set`, `GetField`/`SetField`)
//! não precisa existir: ninguém observa a identidade dele (nem `==`, nem
//! `is`, nem o coletor), e cada campo vira um local — que o `mem2reg`
//! promove a valor SSA em seguida. Um uso qualquer além desses (argumento,
//! retorno, `phi`, gravação em outro objeto, tipo, RTI) conta como fuga, e
//! o objeto fica como está. Depois do inlining do construtor e dos
//! métodos, os temporários do tipo `Ponto(a, b).x` somem.

use super::operandos::*;
use crate::hir::*;
use std::collections::HashMap;

/// Um acesso ao campo `indice` de um objeto candidato.
enum Acesso {
    /// Leitura: representação lógica tipada ou bits legados em `I64`.
    Le(Type),
    /// Gravação: o valor e se é referência.
    Grava(Type, bool),
}

fn indice(o: &Operand) -> Option<usize> {
    match o {
        Operand::Constant(Constant::Int(i)) if *i >= 0 => Some(*i as usize),
        _ => None,
    }
}

fn flag(o: &Operand) -> Option<bool> {
    match o {
        Operand::Constant(Constant::Int(n)) => Some(*n != 0),
        Operand::Constant(Constant::Bool(b)) => Some(*b),
        _ => None,
    }
}

/// Substitui os objetos que não escapam de `func`. Devolve se mudou.
pub fn substituir_objetos(func: &mut Function) -> bool {
    let tipos = tipos_da_funcao(func);
    let mut candidatos: HashMap<ValueId, Vec<(usize, Acesso)>> = HashMap::new();
    for b in &func.blocks {
        for (v, inst, _) in &b.instructions {
            // Só objetos do programa (cid ≥ 128): as classes do runtime têm
            // layout e identidade que o runtime conhece (§2.4).
            if let Instruction::CallRuntime { name, args, .. } = inst
                && name == "dartforge_object_new"
                && matches!(args.first(), Some((Operand::Constant(Constant::Int(c)), _))
                    if *c >= dartforge_runtime::layout::PRIMEIRO_CID_LIVRE)
            {
                candidatos.insert(*v, Vec::new());
            }
        }
    }
    if candidatos.is_empty() {
        return false;
    }
    let mut fugiu: std::collections::HashSet<ValueId> = std::collections::HashSet::new();
    for b in &func.blocks {
        for (_, inst, ty) in &b.instructions {
            // O acesso reconhecido: o objeto e o campo, fora de outra posição.
            let reconhecido = match inst {
                Instruction::CallRuntime { name, args, .. } if name == "dartforge_object_get" && args.len() == 2 => {
                    match (&args[0].0, indice(&args[1].0)) {
                        (Operand::Val(o), Some(i)) if candidatos.contains_key(o) => Some((*o, i, Acesso::Le(*ty), None)),
                        _ => None,
                    }
                }
                Instruction::CallRuntime { name, args, .. } if name == "dartforge_object_set" && args.len() == 4 => {
                    match (&args[0].0, indice(&args[1].0), flag(&args[3].0)) {
                        (Operand::Val(o), Some(i), Some(r)) if candidatos.contains_key(o) => {
                            let t = tipo_do_operando(&args[2].0, &tipos).unwrap_or(Type::Void);
                            Some((*o, i, Acesso::Grava(t, r), Some(&args[2].0)))
                        }
                        _ => None,
                    }
                }
                Instruction::GetField { object: Operand::Val(o), index } if candidatos.contains_key(o) => {
                    Some((*o, *index, Acesso::Le(*ty), None))
                }
                Instruction::SetField { object: Operand::Val(o), index, value } if candidatos.contains_key(o) => {
                    let t = tipo_do_operando(value, &tipos).unwrap_or(Type::Void);
                    Some((*o, *index, Acesso::Grava(t, t == Type::Ref), Some(value)))
                }
                _ => None,
            };
            match reconhecido {
                Some((o, i, acesso, valor)) => {
                    // O próprio objeto gravado num campo (dele ou de outro).
                    if let Some(Operand::Val(x)) = valor
                        && candidatos.contains_key(x)
                    {
                        fugiu.insert(*x);
                    }
                    candidatos.get_mut(&o).expect("candidato").push((i, acesso));
                }
                None => operandos(inst, &mut |op| {
                    if let Operand::Val(x) = op
                        && candidatos.contains_key(x)
                        && !matches!(inst, Instruction::CallRuntime { name, .. } if name == "dartforge_object_new")
                    {
                        fugiu.insert(*x);
                    }
                }),
            }
        }
        operandos_do_terminador(&b.terminator, &mut |op| {
            if let Operand::Val(x) = op
                && candidatos.contains_key(x)
            {
                fugiu.insert(*x);
            }
        });
    }

    // Todos os acessos devem concordar na representação do campo;
    // leituras tipadas não podem carregar uma gravação legada em bits.
    let mut locais: HashMap<(ValueId, usize), Type> = HashMap::new();
    let mut ordem: Vec<ValueId> = candidatos.keys().copied().filter(|v| !fugiu.contains(v)).collect();
    ordem.sort_by_key(|v| v.0);
    let mut aceitos = Vec::new();
    'objetos: for v in ordem {
        let mut campos: HashMap<usize, Type> = HashMap::new();
        for (i, a) in &candidatos[&v] {
            let t = match a {
                Acesso::Le(t) => *t,
                Acesso::Grava(t, true) if *t == Type::Ref => Type::Ref,
                Acesso::Grava(t, false) if matches!(t, Type::I64 | Type::F64 | Type::I1 | Type::I8) => *t,
                _ => continue 'objetos,
            };
            if !matches!(t, Type::Ref | Type::I64 | Type::F64 | Type::I1 | Type::I8) || *campos.entry(*i).or_insert(t) != t {
                continue 'objetos;
            }
        }
        for (i, t) in campos {
            locais.insert((v, i), t);
        }
        aceitos.push(v);
    }
    if aceitos.is_empty() {
        return false;
    }

    // Um local por campo, na entrada.
    let (mut prox, _) = maiores_ids(func);
    let mut chaves: Vec<(ValueId, usize)> = locais.keys().copied().collect();
    chaves.sort_by_key(|(v, i)| (v.0, *i));
    let mut enderecos: HashMap<(ValueId, usize), ValueId> = HashMap::new();
    let mut allocas = Vec::new();
    let mut inicializacoes: HashMap<ValueId, Vec<(ValueId, Instruction, Type)>> = HashMap::new();
    for k in chaves {
        prox += 1;
        enderecos.insert(k, ValueId(prox));
        allocas.push((ValueId(prox), Instruction::Alloca(locais[&k]), Type::Ptr));
        if matches!(locais[&k], Type::F64 | Type::I1 | Type::I8) {
            // Reinicializa no ponto da alocação, não na entrada: uma alocação
            // dentro de um laço começa zerada em cada iteração.
            let val = match locais[&k] {
                Type::F64 => Operand::Constant(Constant::Double(0.0)),
                Type::I1 => Operand::Constant(Constant::Bool(false)),
                _ => Operand::Constant(Constant::Int(0)),
            };
            prox += 1;
            inicializacoes.entry(k.0).or_default().push((ValueId(prox), Instruction::Store {
                ptr: Operand::Val(enderecos[&k]), val,
            }, Type::Void));
        }
    }
    let aceitos: std::collections::HashSet<ValueId> = aceitos.into_iter().collect();
    for b in &mut func.blocks {
        b.instructions = std::mem::take(&mut b.instructions).into_iter().flat_map(|inst| {
            if aceitos.contains(&inst.0) {
                inicializacoes.remove(&inst.0).unwrap_or_default()
            } else { vec![inst] }
        }).collect();
        for (_, inst, ty) in &mut b.instructions {
            let novo = match inst {
                Instruction::CallRuntime { name, args, .. } if name == "dartforge_object_get" && args.len() == 2 => {
                    match (&args[0].0, indice(&args[1].0)) {
                        (Operand::Val(o), Some(i)) if aceitos.contains(o) => {
                            Some(Instruction::Load { ptr: Operand::Val(enderecos[&(*o, i)]), ty: *ty })
                        }
                        _ => None,
                    }
                }
                Instruction::CallRuntime { name, args, .. } if name == "dartforge_object_set" && args.len() == 4 => {
                    match (&args[0].0, indice(&args[1].0)) {
                        (Operand::Val(o), Some(i)) if aceitos.contains(o) => {
                            Some(Instruction::Store { ptr: Operand::Val(enderecos[&(*o, i)]), val: args[2].0.clone() })
                        }
                        _ => None,
                    }
                }
                Instruction::GetField { object: Operand::Val(o), index } if aceitos.contains(o) => {
                    Some(Instruction::Load { ptr: Operand::Val(enderecos[&(*o, *index)]), ty: *ty })
                }
                Instruction::SetField { object: Operand::Val(o), index, value } if aceitos.contains(o) => {
                    Some(Instruction::Store { ptr: Operand::Val(enderecos[&(*o, *index)]), val: value.clone() })
                }
                _ => None,
            };
            if let Some(n) = novo {
                if matches!(n, Instruction::Store { .. }) {
                    *ty = Type::Void;
                }
                *inst = n;
            }
        }
    }
    func.blocks[0].instructions.splice(0..0, allocas);
    true
}
