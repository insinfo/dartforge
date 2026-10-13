//! Inlining de funções pequenas do módulo.
//!
//! Uma exceção é um estado pendente do runtime que quem chama confere logo
//! depois da chamada (`FnBuilder::emit_call_with_check`): quem lança deixa a
//! exceção pendente e **retorna**. Por isso o corpo copiado não precisa ser
//! religado aos `catch`/`finally` de quem chama: a saída excepcional da
//! cópia é um `return` como os outros, que vira desvio para a continuação,
//! e a conferência que seguia a chamada — que fica na continuação — leva a
//! exceção ao tratador de quem chama, como antes. O corpo roda igual, com o
//! mesmo estado do runtime; só a fronteira da chamada some.
//!
//! Os argumentos entram no lugar dos parâmetros; cada `return` vira um
//! desvio para a continuação, com um `phi` se houver mais de um. O
//! `dartforge_exception_clear` do `return` (que descarta uma exceção
//! pendente ao sair de um `finally`) fica na cópia de quem pode lançar; na
//! de quem não lança some (não há o que descartar), e a conferência depois
//! da chamada já foi dobrada (`simplify::dead_checks`). O
//! terminador `Throw` (lança e não volta) não é copiado.

use super::mem2reg::compatible;
use super::operands::*;
use crate::hir::*;
use std::collections::HashMap;

/// Instruções de uma função que se copia.
pub const BODY_LIMIT: usize = 40;
/// Tamanho máximo de quem recebe as cópias.
pub const CALLER_LIMIT: usize = 4000;

fn size(f: &Function) -> usize {
    f.blocks.iter().map(|b| b.instructions.len() + 1).sum()
}

/// A função pode ser copiada no lugar da chamada?
pub fn copyable(f: &Function) -> bool {
    size(f) <= BODY_LIMIT
        && f.blocks.first().is_some_and(|b| b.id.0 == 0)
        && f.blocks.iter().all(|b| {
            !matches!(b.terminator, Terminator::Throw(_))
                && b.instructions.iter().all(|(_, i, _)| match i {
                    // Um local ficaria num bloco que pode estar num laço.
                    Instruction::Alloca(_) => false,
                    // A própria função (recursão).
                    Instruction::CallStatic { symbol, .. } => *symbol != f.symbol,
                    // Os adaptadores da convenção uniforme leem os
                    // parâmetros da própria entrada.
                    Instruction::LoadIndexed { .. } => false,
                    // O getter preguiçoso de um global ou de um valor de
                    // enum (grava o valor na primeira leitura): copiá-lo
                    // duplica a inicialização inteira, que roda uma vez, em
                    // cada leitura. Numa tabela de 16 mil `Cat.lu` (o `bidi`
                    // do `pdf_plus` no new_sali/backend) eram ~135 linhas de
                    // IR por elemento — 97 MB num getter de constante
                    // (docs/NATIVO-PROJETOS-REAIS.md, C8). A VM também lê o
                    // campo estático e só chama o inicializador na primeira
                    // vez (`LoadStaticField` + `InitStaticField`).
                    Instruction::StoreGlobal { .. } => false,
                    _ => true,
                })
        })
}

/// Copia, em `func`, as chamadas a funções de `copies` (com se a função
/// pode lançar). Devolve se mudou.
pub fn inlining(func: &mut Function, copies: &HashMap<String, (Function, bool)>) -> bool {
    inlining_with_origins(func, copies, &HashMap::new(), &mut HashMap::new())
}

pub(super) fn inlining_with_origins(
    func: &mut Function, copies: &HashMap<String, (Function, bool)>,
    sources: &HashMap<String, HashMap<ValueId, crate::optimize::arc::analysis::model::ArcSite>>,
    sites: &mut HashMap<ValueId, crate::optimize::arc::analysis::model::ArcSite>,
) -> bool {
    let mut changed = false;
    let mut budget = 64;
    while budget > 0 && size(func) < CALLER_LIMIT {
        let types = function_types(func);
        let found = func.blocks.iter().enumerate().find_map(|(bi, b)| {
            b.instructions.iter().enumerate().find_map(|(ii, (_, inst, ty))| {
                let Instruction::CallStatic { symbol, args, .. } = inst else { return None };
                let (alvo, _) = copies.get(symbol)?;
                if *symbol == func.symbol || alvo.params.len() != args.len() {
                    return None;
                }
                let arguments_ok = alvo.params.iter().zip(args).all(|((_, _, tp), a)| {
                    operand_type(a, &types).is_some_and(|ta| ta == *tp || (compatible(*tp, ta) && *tp != Type::Ref))
                });
                let target_types = function_types(alvo);
                let return_ok = *ty == Type::Void
                    || alvo.blocks.iter().all(|b| match &b.terminator {
                        Terminator::Return(Some(r)) => operand_type(r, &target_types)
                            .is_some_and(|tr| tr == *ty || (compatible(*ty, tr) && *ty != Type::Ref)),
                        Terminator::Return(None) => *ty == Type::Ref,
                        _ => true,
                    });
                (arguments_ok && return_ok).then_some((bi, ii))
            })
        });
        let Some((bi, ii)) = found else { break };
        copy(func, bi, ii, copies, sources, sites);
        changed = true;
        budget -= 1;
    }
    changed
}

fn copy(func: &mut Function, bi: usize, ii: usize, copies: &HashMap<String, (Function, bool)>,
    sources: &HashMap<String, HashMap<ValueId, crate::optimize::arc::analysis::model::ArcSite>>,
    sites: &mut HashMap<ValueId, crate::optimize::arc::analysis::model::ArcSite>,
) {
    let (mut next_v, mut next_b) = largest_ids(func);
    let (call, args, ty) = match &func.blocks[bi].instructions[ii] {
        (v, Instruction::CallStatic { symbol, args, .. }, ty) => (*v, (symbol.clone(), args.clone()), *ty),
        _ => unreachable!("chamada estática esperada"),
    };
    let (sym_name, args) = args;
    let (alvo, throws) = &copies[&sym_name];
    let throws = *throws;

    // Novos ids para os valores e blocos da cópia.
    let mut values: HashMap<ValueId, Operand> = HashMap::new();
    for ((p, _, _), a) in alvo.params.iter().zip(&args) {
        values.insert(*p, a.clone());
    }
    for b in &alvo.blocks {
        for (v, _, _) in &b.instructions {
            next_v += 1;
            values.insert(*v, Operand::Val(ValueId(next_v)));
        }
    }
    // A identidade pertence à origem; somente o índice SSA da cópia muda.
    // Contextos são unidos conservadoramente, sem inferir singleton por clone.
    if let Some(origins) = sources.get(&sym_name) {
        for (v, site) in origins {
            if let Some(Operand::Val(new)) = values.get(v) {
                sites.insert(*new, site.clone());
            }
        }
    }
    sites.remove(&call);
    let mut blocks: HashMap<BlockId, BlockId> = HashMap::new();
    for b in &alvo.blocks {
        next_b += 1;
        blocks.insert(b.id, BlockId(next_b));
    }
    next_b += 1;
    let continuation = BlockId(next_b);
    let origin = func.blocks[bi].id;

    // Divide o bloco da chamada.
    let after: Vec<(ValueId, Instruction, Type)> = func.blocks[bi].instructions.split_off(ii + 1);
    func.blocks[bi].instructions.pop();
    let terminator = std::mem::replace(&mut func.blocks[bi].terminator, Terminator::Branch(blocks[&alvo.blocks[0].id]));
    // Os sucessores do bloco original agora vêm da continuação.
    for s in super::cfg::successors(&terminator) {
        if let Some(b) = func.blocks.iter_mut().find(|b| b.id == s) {
            for (_, inst, _) in &mut b.instructions {
                if let Instruction::Phi { incoming, .. } = inst {
                    for (o, _) in incoming.iter_mut() {
                        if *o == origin {
                            *o = continuation;
                        }
                    }
                }
            }
        }
    }

    let mut returns: Vec<(BlockId, Operand)> = Vec::new();
    let mut new_set: Vec<BasicBlock> = Vec::new();
    for b in &alvo.blocks {
        let id = blocks[&b.id];
        let mut instructions = Vec::with_capacity(b.instructions.len());
        for (v, inst, t) in &b.instructions {
            if !throws && matches!(inst, Instruction::CallRuntime { name, .. } if name == "dartforge_exception_clear") {
                continue;
            }
            let mut inst = inst.clone();
            operands_mut(&mut inst, &mut |o| {
                if let Operand::Val(x) = o
                    && let Some(n) = values.get(x)
                {
                    *o = n.clone();
                }
            });
            if let Instruction::Phi { incoming, .. } = &mut inst {
                for (o, _) in incoming.iter_mut() {
                    *o = blocks[o];
                }
            }
            let Operand::Val(nv) = values[v] else { unreachable!("valor novo") };
            instructions.push((nv, inst, *t));
        }
        let mut term = b.terminator.clone();
        terminator_operands_mut(&mut term, &mut |o| {
            if let Operand::Val(x) = o
                && let Some(n) = values.get(x)
            {
                *o = n.clone();
            }
        });
        super::operands::successors_mut(&mut term, &mut |s| *s = blocks[s]);
        if let Terminator::Return(r) = &term {
            returns.push((id, r.clone().unwrap_or(Operand::Constant(Constant::Null))));
            term = Terminator::Branch(continuation);
        }
        new_set.push(BasicBlock { id, instructions: instructions, terminator: term });
    }

    // O rastro simbólico (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.14):
    // as instruções copiadas guardam a posição na função copiada e o
    // contexto de inlining, para o quadro dela aparecer como a VM mostra os
    // quadros embutidos. Fica fora das posições das tabelas de linha (o
    // arquivo delas é o desta função). Quem chama sem posições (a entrada
    // uniforme de uma closure) ganha uma depuração oculta: só os quadros
    // copiados aparecem.
    if let Some(da) = alvo.debug_info.as_deref()
        && (func.debug_info.is_some() || crate::alvo::symbolic_stack_trace().unwrap_or(false))
    {
        let dc = func.debug_info.get_or_insert_with(|| {
            Box::new(FunctionDebugInfo {
                file: da.file.clone(),
                url: da.url.clone(),
                line: da.line,
                marks: stack_trace_marks::HIDDEN,
                ..Default::default()
            })
        });
        let (call_position, parent) = match dc.embedded_positions.get(&call) {
            Some(&(l, c, j)) => ((l, c), Some(j)),
            None => (dc.positions.get(&call).copied().unwrap_or((dc.line, 0)), None),
        };
        let k = dc.embedded_set.len() as u32;
        dc.embedded_set.push(Embedded { name: alvo.stack_trace_name(), url: da.url.clone(), call: call_position, parent, marks: da.marks });
        for e in &da.embedded_set {
            dc.embedded_set.push(Embedded { parent: Some(e.parent.map_or(k, |p| k + 1 + p)), ..e.clone() });
        }
        for b in &alvo.blocks {
            for (v, _, _) in &b.instructions {
                let Some(Operand::Val(new)) = values.get(v) else { continue };
                let p = match (da.embedded_positions.get(v), da.positions.get(v)) {
                    (Some(&(l, c, j)), _) => (l, c, k + 1 + j),
                    (None, Some(&(l, c))) => (l, c, k),
                    (None, None) => continue,
                };
                dc.embedded_positions.insert(*new, p);
            }
        }
    }

    // O resultado: o único retorno, ou um `phi` na continuação.
    let mut start = Vec::new();
    let result = match returns.len() {
        0 => None,
        1 => Some(returns[0].1.clone()),
        _ if ty == Type::Void => None,
        _ => {
            next_v += 1;
            let phi = ValueId(next_v);
            start.push((phi, Instruction::Phi { incoming: returns.clone(), ty }, ty));
            Some(Operand::Val(phi))
        }
    };
    start.extend(after);
    let cont = BasicBlock { id: continuation, instructions: start, terminator: terminator };
    let pos = bi + 1;
    let n = new_set.len();
    func.blocks.splice(pos..pos, new_set);
    func.blocks.insert(pos + n, cont);
    let r = result.unwrap_or_else(|| default_constant(ty));
    replace(func, &|v| (v == call).then(|| r.clone()));
}
