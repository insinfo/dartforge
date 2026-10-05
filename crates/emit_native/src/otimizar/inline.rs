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
//! da chamada já foi dobrada (`simplificar::conferencias_mortas`). O
//! terminador `Throw` (lança e não volta) não é copiado.

use super::mem2reg::compativel;
use super::operandos::*;
use crate::hir::*;
use std::collections::HashMap;

/// Instruções de uma função que se copia.
pub const LIMITE_DO_CORPO: usize = 40;
/// Tamanho máximo de quem recebe as cópias.
pub const LIMITE_DE_QUEM_CHAMA: usize = 4000;

fn tamanho(f: &Function) -> usize {
    f.blocks.iter().map(|b| b.instructions.len() + 1).sum()
}

/// A função pode ser copiada no lugar da chamada?
pub fn copiavel(f: &Function) -> bool {
    tamanho(f) <= LIMITE_DO_CORPO
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

/// Copia, em `func`, as chamadas a funções de `copias` (com se a função
/// pode lançar). Devolve se mudou.
pub fn inlining(func: &mut Function, copias: &HashMap<String, (Function, bool)>) -> bool {
    let mut mudou = false;
    let mut orcamento = 64;
    while orcamento > 0 && tamanho(func) < LIMITE_DE_QUEM_CHAMA {
        let tipos = tipos_da_funcao(func);
        let achado = func.blocks.iter().enumerate().find_map(|(bi, b)| {
            b.instructions.iter().enumerate().find_map(|(ii, (_, inst, ty))| {
                let Instruction::CallStatic { symbol, args, .. } = inst else { return None };
                let (alvo, _) = copias.get(symbol)?;
                if *symbol == func.symbol || alvo.params.len() != args.len() {
                    return None;
                }
                let argumentos_ok = alvo.params.iter().zip(args).all(|((_, _, tp), a)| {
                    tipo_do_operando(a, &tipos).is_some_and(|ta| ta == *tp || (compativel(*tp, ta) && *tp != Type::Ref))
                });
                let tipos_alvo = tipos_da_funcao(alvo);
                let retorno_ok = *ty == Type::Void
                    || alvo.blocks.iter().all(|b| match &b.terminator {
                        Terminator::Return(Some(r)) => tipo_do_operando(r, &tipos_alvo)
                            .is_some_and(|tr| tr == *ty || (compativel(*ty, tr) && *ty != Type::Ref)),
                        Terminator::Return(None) => *ty == Type::Ref,
                        _ => true,
                    });
                (argumentos_ok && retorno_ok).then_some((bi, ii))
            })
        });
        let Some((bi, ii)) = achado else { break };
        copiar(func, bi, ii, copias);
        mudou = true;
        orcamento -= 1;
    }
    mudou
}

fn copiar(func: &mut Function, bi: usize, ii: usize, copias: &HashMap<String, (Function, bool)>) {
    let (mut prox_v, mut prox_b) = maiores_ids(func);
    let (chamada, args, ty) = match &func.blocks[bi].instructions[ii] {
        (v, Instruction::CallStatic { symbol, args, .. }, ty) => (*v, (symbol.clone(), args.clone()), *ty),
        _ => unreachable!("chamada estática esperada"),
    };
    let (simbolo, args) = args;
    let (alvo, lanca) = &copias[&simbolo];
    let lanca = *lanca;

    // Novos ids para os valores e blocos da cópia.
    let mut valores: HashMap<ValueId, Operand> = HashMap::new();
    for ((p, _, _), a) in alvo.params.iter().zip(&args) {
        valores.insert(*p, a.clone());
    }
    for b in &alvo.blocks {
        for (v, _, _) in &b.instructions {
            prox_v += 1;
            valores.insert(*v, Operand::Val(ValueId(prox_v)));
        }
    }
    let mut blocos: HashMap<BlockId, BlockId> = HashMap::new();
    for b in &alvo.blocks {
        prox_b += 1;
        blocos.insert(b.id, BlockId(prox_b));
    }
    prox_b += 1;
    let continuacao = BlockId(prox_b);
    let origem = func.blocks[bi].id;

    // Divide o bloco da chamada.
    let depois: Vec<(ValueId, Instruction, Type)> = func.blocks[bi].instructions.split_off(ii + 1);
    func.blocks[bi].instructions.pop();
    let terminador = std::mem::replace(&mut func.blocks[bi].terminator, Terminator::Branch(blocos[&alvo.blocks[0].id]));
    // Os sucessores do bloco original agora vêm da continuação.
    for s in super::cfg::sucessores(&terminador) {
        if let Some(b) = func.blocks.iter_mut().find(|b| b.id == s) {
            for (_, inst, _) in &mut b.instructions {
                if let Instruction::Phi { incoming, .. } = inst {
                    for (o, _) in incoming.iter_mut() {
                        if *o == origem {
                            *o = continuacao;
                        }
                    }
                }
            }
        }
    }

    let mut retornos: Vec<(BlockId, Operand)> = Vec::new();
    let mut novos: Vec<BasicBlock> = Vec::new();
    for b in &alvo.blocks {
        let id = blocos[&b.id];
        let mut instrucoes = Vec::with_capacity(b.instructions.len());
        for (v, inst, t) in &b.instructions {
            if !lanca && matches!(inst, Instruction::CallRuntime { name, .. } if name == "dartforge_exception_clear") {
                continue;
            }
            let mut inst = inst.clone();
            operandos_mut(&mut inst, &mut |o| {
                if let Operand::Val(x) = o
                    && let Some(n) = valores.get(x)
                {
                    *o = n.clone();
                }
            });
            if let Instruction::Phi { incoming, .. } = &mut inst {
                for (o, _) in incoming.iter_mut() {
                    *o = blocos[o];
                }
            }
            let Operand::Val(nv) = valores[v] else { unreachable!("valor novo") };
            instrucoes.push((nv, inst, *t));
        }
        let mut term = b.terminator.clone();
        operandos_do_terminador_mut(&mut term, &mut |o| {
            if let Operand::Val(x) = o
                && let Some(n) = valores.get(x)
            {
                *o = n.clone();
            }
        });
        super::operandos::sucessores_mut(&mut term, &mut |s| *s = blocos[s]);
        if let Terminator::Return(r) = &term {
            retornos.push((id, r.clone().unwrap_or(Operand::Constant(Constant::Null))));
            term = Terminator::Branch(continuacao);
        }
        novos.push(BasicBlock { id, instructions: instrucoes, terminator: term });
    }

    // O rastro simbólico (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.14):
    // as instruções copiadas guardam a posição na função copiada e o
    // contexto de inlining, para o quadro dela aparecer como a VM mostra os
    // quadros embutidos. Fica fora das posições das tabelas de linha (o
    // arquivo delas é o desta função). Quem chama sem posições (a entrada
    // uniforme de uma closure) ganha uma depuração oculta: só os quadros
    // copiados aparecem.
    if let Some(da) = alvo.depuracao.as_deref()
        && (func.depuracao.is_some() || crate::alvo::rastro_simbolico().unwrap_or(false))
    {
        let dc = func.depuracao.get_or_insert_with(|| {
            Box::new(DepuracaoDaFuncao {
                arquivo: da.arquivo.clone(),
                url: da.url.clone(),
                linha: da.linha,
                marcas: marcas_do_rastro::OCULTA,
                ..Default::default()
            })
        });
        let (posicao_da_chamada, pai) = match dc.posicoes_embutidas.get(&chamada) {
            Some(&(l, c, j)) => ((l, c), Some(j)),
            None => (dc.posicoes.get(&chamada).copied().unwrap_or((dc.linha, 0)), None),
        };
        let k = dc.embutidas.len() as u32;
        dc.embutidas.push(Embutida { nome: alvo.nome_do_rastro(), url: da.url.clone(), chamada: posicao_da_chamada, pai, marcas: da.marcas });
        for e in &da.embutidas {
            dc.embutidas.push(Embutida { pai: Some(e.pai.map_or(k, |p| k + 1 + p)), ..e.clone() });
        }
        for b in &alvo.blocks {
            for (v, _, _) in &b.instructions {
                let Some(Operand::Val(novo)) = valores.get(v) else { continue };
                let p = match (da.posicoes_embutidas.get(v), da.posicoes.get(v)) {
                    (Some(&(l, c, j)), _) => (l, c, k + 1 + j),
                    (None, Some(&(l, c))) => (l, c, k),
                    (None, None) => continue,
                };
                dc.posicoes_embutidas.insert(*novo, p);
            }
        }
    }

    // O resultado: o único retorno, ou um `phi` na continuação.
    let mut inicio = Vec::new();
    let resultado = match retornos.len() {
        0 => None,
        1 => Some(retornos[0].1.clone()),
        _ if ty == Type::Void => None,
        _ => {
            prox_v += 1;
            let phi = ValueId(prox_v);
            inicio.push((phi, Instruction::Phi { incoming: retornos.clone(), ty }, ty));
            Some(Operand::Val(phi))
        }
    };
    inicio.extend(depois);
    let cont = BasicBlock { id: continuacao, instructions: inicio, terminator: terminador };
    let pos = bi + 1;
    let n = novos.len();
    func.blocks.splice(pos..pos, novos);
    func.blocks.insert(pos + n, cont);
    let r = resultado.unwrap_or_else(|| constante_padrao(ty));
    substituir(func, &|v| (v == chamada).then(|| r.clone()));
}
