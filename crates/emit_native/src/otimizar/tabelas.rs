//! Exceções por tabelas (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13): o
//! passe que reescreve a HIR já otimizada do modelo "pendência conferida
//! depois de cada chamada" para o modelo "a chamada Dart desenrola".
//!
//! **O contrato do modo.** Uma função Dart nunca volta a quem a chamou com a
//! exceção pendente: ela desenrola a pilha (`@df.lancar`) até o pouso
//! (`landingpad`) de quem a trata. A exceção em si continua sendo a
//! pendência do runtime — o desenrolamento só transfere o controle —, então
//! o código de um tratador (`catch`, `finally`), que lê e limpa a pendência,
//! é o mesmo dos dois modos. O runtime não muda de protocolo: uma extern
//! que lança deixa a exceção pendente e retorna, e a conferência depois
//! dela continua na HIR.
//!
//! **O que o passe faz**, função a função, sem tocar o lowering nem os
//! outros passes (que continuam vendo só a forma de conferência):
//!
//! 1. acha cada *sítio* — a instrução que chama código Dart que pode
//!    desenrolar ([`e_sitio`]) — e decide o destino do desenrolamento:
//!    * chamada direta seguida da conferência, com tratador que só devolve
//!      o valor padrão ([`saida_pura`]): a conferência some e a chamada
//!      fica uma chamada comum, sem pouso — o desenrolamento atravessa o
//!      quadro (o pouso de quem trata restaura o topo da pilha-sombra);
//!    * chamada direta seguida da conferência, com tratador de verdade: a
//!      conferência some, a chamada vira `invoke` e o pouso (um bloco novo)
//!      desvia para o tratador;
//!    * chamada que também pode voltar com a exceção pendente (closure,
//!      seletor: a entrada inválida e a de `noSuchMethod` são do runtime)
//!      com tratador que só devolve o valor padrão: nada muda (a conferência
//!      pega a pendência; o desenrolamento atravessa o quadro);
//!    * todo o resto: **emulação exata** — a chamada vira `invoke` e o pouso
//!      continua no ponto seguinte à chamada, com o valor padrão como
//!      resultado e a exceção pendente, exatamente o que a chamada devolvia
//!      no modelo de conferência;
//! 2. calcula, por fluxo de dados, se a exceção está pendente em cada
//!    `Return` ([`Pendencia`]): limpa, retorna; ligada, desenrola
//!    ([`SaidaPorExcecao::Lanca`]); talvez, confere e decide
//!    ([`SaidaPorExcecao::Guarda`]).
//!
//! O resultado ([`TabelasDaFuncao`], em [`Module::tabelas`]) é o que o
//! emissor lê (`llvm/mod.rs`). Uma chamada com pouso é a última instrução do
//! bloco dela; o terminador do bloco é
//! `CondBranch { cond: false, then: pouso, else: continuação }`, que mantém
//! o pouso no grafo de fluxo (vivacidade das raízes, blocos alcançáveis) e
//! que o emissor escreve como o desvio à continuação.

//! Em ARC, os atalhos que deixam unwind atravessar um quadro sem pouso
//! ficam desabilitados: a aresta local é necessária à inserção de cleanup.
//! O passe expõe essa aresta; ainda não insere a limpeza de owners.

use super::cfg::sucessores;
use super::efeitos::{e_conferencia, instrucao_lanca, nao_lancam};
use super::operandos::{constante_padrao, maiores_ids, operandos, operandos_do_terminador, substituir};
use super::simplificar::tirar_inalcancaveis;
use crate::hir::*;
use crate::llvm::externs::efeitos_de;
use std::collections::{HashMap, HashSet};

/// Reescreve o módulo para as exceções por tabelas e grava a decisão de cada
/// função em [`Module::tabelas`]. Roda depois de [`super::otimizar`] (e
/// também com os passes desligados: só depende da forma que o lowering dá).
pub fn aplicar(module: &mut Module) {
    let plano = preparar(module);
    materializar(module, plano);
}

/// Inventário dos sítios preparados, ainda sem decisões sobre retornos.
/// Entre as etapas, preservar IDs e ordem das funções; transformações do
/// CFG precisam atualizar este inventário antes de materializar tabelas.
pub(super) struct PlanoExcecoes {
    funcoes: Vec<(String, TabelasDaFuncao)>,
}

impl PlanoExcecoes {
    /// Consulta os sítios antes de publicar tabelas, conferindo a identidade
    /// da função. Os passes intermediários leem o plano sem habilitar LLVM.
    pub(super) fn sitios_da_funcao(&self, indice: usize, f: &Function) -> &TabelasDaFuncao {
        let (simbolo, sitios) = self.funcoes.get(indice)
            .expect("bug do compilador: função ausente do plano excepcional");
        assert_eq!(&f.symbol, simbolo, "bug do compilador: ordem das funções mudou depois da preparação excepcional");
        sitios
    }
}

/// Expõe pousos e continuações no CFG sem publicar tabelas para o emissor.
pub(super) fn preparar(module: &mut Module) -> PlanoExcecoes {
    assert!(!module.excecoes_por_tabelas, "bug do compilador: CFG excepcional já materializado");
    let nl = nao_lancam(module);
    // Sem prova de ausência de owners, ARC precisa de uma saída local
    // para inserir cleanup mesmo quando não há catch/finally na fonte.
    let preservar_cleanup = module.memoria_arc;
    let mut funcoes = Vec::with_capacity(module.functions.len());
    for f in &mut module.functions {
        let sitios = if super::valida(f) { da_funcao(f, &nl, preservar_cleanup) } else { TabelasDaFuncao::default() };
        funcoes.push((f.symbol.clone(), sitios));
    }
    PlanoExcecoes { funcoes }
}

/// Reconstrói decisões de saída sobre a HIR final e valida os sítios antes
/// de publicar as tabelas. Não roda simplificação genérica nos pousos.
pub(super) fn materializar(module: &mut Module, plano: PlanoExcecoes) {
    assert_eq!(plano.funcoes.len(), module.functions.len(), "bug do compilador: funções mudaram depois da preparação excepcional");
    let nl = nao_lancam(module);
    let mut tabelas = Vec::with_capacity(module.functions.len());
    for (indice, f) in module.functions.iter().enumerate() {
        let t = plano.sitios_da_funcao(indice, f);
        if super::valida(f) {
            conferir(f, t);
        }
    }
    for (f, (simbolo, mut t)) in module.functions.iter_mut().zip(plano.funcoes) {
        assert_eq!(f.symbol, simbolo, "bug do compilador: ordem das funções mudou depois da preparação excepcional");
        if super::valida(f) {
            saidas(f, &nl, &mut t);
        }
        tabelas.push(t);
    }
    module.tabelas = tabelas;
    module.excecoes_por_tabelas = true;
}

/// A instrução chama código Dart que pode desenrolar?
///
/// * `CallStatic` de uma função que pode lançar. Ficam de fora as que não
///   lançam (`efeitos::nao_lancam`), os símbolos do runtime (`dartforge_…`,
///   que seguem o protocolo da pendência) e o getter de um tipo
///   (`df.rti.…`, `lower/rti.rs`), que o lowering chama sem conferir: se ele
///   lançar, o desenrolamento atravessa o quadro;
/// * a chamada do corpo tipado de uma closure, a de um valor função e a por
///   seletor.
fn e_sitio(inst: &Instruction, nl: &HashSet<String>) -> bool {
    match inst {
        Instruction::CallStatic { symbol, .. } => {
            !nl.contains(symbol) && !symbol.starts_with("df.rti.") && !symbol.starts_with("dartforge_")
        }
        Instruction::ChamadaTipada { .. }
        | Instruction::CallClosure { .. }
        | Instruction::CallClosureRepasse { .. }
        | Instruction::CallSeletor { .. }
        | Instruction::CallSeletorRepasse { .. } => true,
        _ => false,
    }
}

/// O sítio só sai por desenrolamento (o alvo é sempre código Dart)? A
/// chamada de closure e a por seletor também podem voltar com a exceção
/// pendente (`@df_clo_invalido`, a entrada de `noSuchMethod`).
fn so_desenrola(inst: &Instruction) -> bool {
    matches!(inst, Instruction::CallStatic { .. } | Instruction::ChamadaTipada { .. })
}

/// A forma que `FnBuilder::emit_call_with_check` dá a um sítio na posição
/// `i`: `[chamada][p = pendente][c = p != 0]` no fim do bloco e
/// `CondBranch(c, tratador, continuação)`.
struct Conferencia {
    p: ValueId,
    c: ValueId,
    tratador: BlockId,
    continuacao: BlockId,
}

fn conferencia_do_sitio(b: &BasicBlock, i: usize) -> Option<Conferencia> {
    if b.instructions.len() != i + 3 {
        return None;
    }
    let (p, inst_p, _) = &b.instructions[i + 1];
    if !e_conferencia(inst_p) {
        return None;
    }
    let (c, inst_c, _) = &b.instructions[i + 2];
    let Instruction::ICmp(ICmpOp::Ne, Operand::Val(lido), Operand::Constant(Constant::Int(0))) = inst_c else {
        return None;
    };
    if lido != p {
        return None;
    }
    let Terminator::CondBranch { cond: Operand::Val(cond), then_block, else_block } = &b.terminator else {
        return None;
    };
    if cond != c || then_block == else_block {
        return None;
    }
    Some(Conferencia { p: *p, c: *c, tratador: *then_block, continuacao: *else_block })
}

/// O tratador só devolve o valor padrão (a saída por exceção de uma função
/// sem `catch` nem `finally` em volta)? Blocos sem instrução além de `phi`,
/// encadeados por desvio incondicional até um `Return`. Um bloco de junção
/// que só reconfere a pendência (`phi`, `p = pendente`, `c = p != 0`,
/// `CondBranch(c, …)`; a forma que a entrada uniforme `$ent` dá à junção do
/// caminho normal com o da exceção) é atravessado pelo lado da exceção:
/// vindo do tratador, a pendência está ligada e a reconferência sempre vai
/// para lá. Sem isto, todo `$ent` ganhava um pouso que pega tudo e devolve a
/// pendência, e a chamada dinâmica do modo tabelas custava mais que a do
/// modo de conferência.
fn saida_pura(f: &Function, pos: &HashMap<BlockId, usize>, de: BlockId) -> bool {
    let mut atual = de;
    for _ in 0..64 {
        let Some(&k) = pos.get(&atual) else { return false };
        let b = &f.blocks[k];
        let resto: Vec<&(ValueId, Instruction, Type)> = b.instructions.iter().filter(|(_, i, _)| !matches!(i, Instruction::Phi { .. })).collect();
        match (&resto[..], &b.terminator) {
            ([], Terminator::Return(_)) => return true,
            ([], Terminator::Branch(n)) => atual = *n,
            ([(p, ip, _), (c, ic, _)], Terminator::CondBranch { cond: Operand::Val(cond), then_block, .. })
                if e_conferencia(ip)
                    && cond == c
                    && matches!(ic, Instruction::ICmp(ICmpOp::Ne, Operand::Val(l), Operand::Constant(Constant::Int(0))) if l == p) =>
            {
                atual = *then_block;
            }
            _ => return false,
        }
    }
    false
}

/// Quantas vezes cada valor é lido (operandos, entradas de `phi`,
/// terminadores).
fn contar_usos(f: &Function) -> HashMap<ValueId, u32> {
    let mut usos: HashMap<ValueId, u32> = HashMap::new();
    {
        let mut conta = |o: &Operand| {
            if let Operand::Val(v) = o {
                *usos.entry(*v).or_insert(0) += 1;
            }
        };
        for b in &f.blocks {
            for (_, inst, _) in &b.instructions {
                operandos(inst, &mut conta);
            }
            operandos_do_terminador(&b.terminator, &mut conta);
        }
    }
    usos
}

fn menos(usos: &mut HashMap<ValueId, u32>, v: ValueId) {
    if let Some(n) = usos.get_mut(&v) {
        *n = n.saturating_sub(1);
    }
}

fn sem_uso(usos: &HashMap<ValueId, u32>, v: ValueId) -> bool {
    usos.get(&v).copied().unwrap_or(0) == 0
}

/// Tira a comparação e a leitura da pendência que ficaram sem leitor.
fn tirar_conferencia(b: &mut BasicBlock, usos: &mut HashMap<ValueId, u32>, cf: &Conferencia) {
    if !sem_uso(usos, cf.c) {
        return;
    }
    b.instructions.retain(|(v, _, _)| *v != cf.c);
    menos(usos, cf.p);
    if sem_uso(usos, cf.p) {
        b.instructions.retain(|(v, _, _)| *v != cf.p);
    }
}

fn falso() -> Operand {
    Operand::Constant(Constant::Bool(false))
}

fn da_funcao(f: &mut Function, nl: &HashSet<String>, preservar_cleanup: bool) -> TabelasDaFuncao {
    let mut t = TabelasDaFuncao::default();
    t.confere_pilha = f.blocks.iter().any(|b| b.instructions.iter().any(|(_, inst, _)| e_sitio(inst, nl)));
    let (mut prox_v, mut prox_b) = maiores_ids(f);
    let mut usos = contar_usos(f);
    let mut pos: HashMap<BlockId, usize> = f.blocks.iter().enumerate().map(|(k, b)| (b.id, k)).collect();
    // Um sítio por volta: o primeiro do bloco. Ou ele é o último do bloco (a
    // forma conferida), ou o resto do bloco vai para um bloco novo, no fim
    // da lista, visitado adiante.
    let mut bi = 0;
    while bi < f.blocks.len() {
        let Some(i) = f.blocks[bi].instructions.iter().position(|(_, inst, _)| e_sitio(inst, nl)) else {
            bi += 1;
            continue;
        };
        let origem = f.blocks[bi].id;
        let (resultado, tipo, direta) = {
            let (v, inst, ty) = &f.blocks[bi].instructions[i];
            (*v, crate::llvm::LlvmEmitter::tipo_do_resultado(inst, *ty), so_desenrola(inst))
        };
        let conferida = conferencia_do_sitio(&f.blocks[bi], i);
        let pura = !preservar_cleanup && conferida.as_ref().is_some_and(|cf| saida_pura(f, &pos, cf.tratador));
        match conferida {
            // A conferência some: a chamada desenrola e ninguém aqui trata.
            Some(cf) if direta && pura => {
                f.blocks[bi].terminator = Terminator::Branch(cf.continuacao);
                menos(&mut usos, cf.c);
                if let Some(&kt) = pos.get(&cf.tratador) {
                    for (_, inst, _) in &mut f.blocks[kt].instructions {
                        if let Instruction::Phi { incoming, .. } = inst {
                            incoming.retain(|(o, op)| {
                                if *o != origem {
                                    return true;
                                }
                                if let Operand::Val(v) = op {
                                    menos(&mut usos, *v);
                                }
                                false
                            });
                        }
                    }
                }
                tirar_conferencia(&mut f.blocks[bi], &mut usos, &cf);
            }
            // A conferência some: a chamada é `invoke` e o pouso vai ao
            // tratador, com a exceção pendente — como a conferência ia.
            Some(cf) if direta => {
                prox_b += 1;
                let pouso = BlockId(prox_b);
                f.blocks[bi].terminator =
                    Terminator::CondBranch { cond: falso(), then_block: pouso, else_block: cf.continuacao };
                menos(&mut usos, cf.c);
                if let Some(&kt) = pos.get(&cf.tratador) {
                    for (_, inst, _) in &mut f.blocks[kt].instructions {
                        let Instruction::Phi { incoming, ty } = inst else { continue };
                        for (o, op) in incoming.iter_mut() {
                            if *o != origem {
                                continue;
                            }
                            *o = pouso;
                            let lido = match &*op {
                                Operand::Val(v) => Some(*v),
                                Operand::Constant(_) => None,
                            };
                            // No pouso não existem o resultado da chamada
                            // nem a conferência: os valores que eles teriam
                            // (o padrão; pendente; verdadeiro).
                            if lido == Some(resultado) {
                                *op = constante_padrao(*ty);
                            } else if lido == Some(cf.p) {
                                menos(&mut usos, cf.p);
                                *op = Operand::Constant(Constant::Int(1));
                            } else if lido == Some(cf.c) {
                                menos(&mut usos, cf.c);
                                *op = Operand::Constant(Constant::Bool(true));
                            }
                        }
                    }
                }
                pos.insert(pouso, f.blocks.len());
                f.blocks.push(BasicBlock {
                    id: pouso,
                    instructions: Vec::new(),
                    terminator: Terminator::Branch(cf.tratador),
                });
                tirar_conferencia(&mut f.blocks[bi], &mut usos, &cf);
                t.invocacoes.insert(resultado, pouso);
                t.pousos.insert(pouso);
            }
            // Pode voltar pendente e quem trata só devolve o padrão: a
            // conferência fica e o desenrolamento atravessa o quadro.
            Some(_) if pura => {}
            // Emulação exata: o pouso continua depois da chamada, com o
            // valor padrão e a exceção pendente.
            _ => {
                prox_b += 2;
                let pouso = BlockId(prox_b - 1);
                let resto = BlockId(prox_b);
                let cauda = f.blocks[bi].instructions.split_off(i + 1);
                let terminador = std::mem::replace(
                    &mut f.blocks[bi].terminator,
                    Terminator::CondBranch { cond: falso(), then_block: pouso, else_block: resto },
                );
                // Os sucessores do bloco passam a ser sucessores do resto.
                for s in sucessores(&terminador) {
                    let Some(&k) = pos.get(&s) else { continue };
                    for (_, inst, _) in &mut f.blocks[k].instructions {
                        if let Instruction::Phi { incoming, .. } = inst {
                            for (o, _) in incoming.iter_mut() {
                                if *o == origem {
                                    *o = resto;
                                }
                            }
                        }
                    }
                }
                pos.insert(pouso, f.blocks.len());
                f.blocks.push(BasicBlock { id: pouso, instructions: Vec::new(), terminator: Terminator::Branch(resto) });
                pos.insert(resto, f.blocks.len());
                f.blocks.push(BasicBlock { id: resto, instructions: cauda, terminator: terminador });
                if tipo != Type::Void {
                    // O resultado, daqui em diante, é o da chamada ou o
                    // padrão (o que a função que lançou devolvia).
                    prox_v += 1;
                    let novo = ValueId(prox_v);
                    substituir(f, &|v| (v == resultado).then_some(Operand::Val(novo)));
                    let k = pos[&resto];
                    f.blocks[k].instructions.insert(
                        0,
                        (
                            novo,
                            Instruction::Phi {
                                incoming: vec![(origem, Operand::Val(resultado)), (pouso, constante_padrao(tipo))],
                                ty: tipo,
                            },
                            tipo,
                        ),
                    );
                }
                t.invocacoes.insert(resultado, pouso);
                t.pousos.insert(pouso);
            }
        }
        bi += 1;
    }
    // Os tratadores que ficaram sem quem chegue a eles.
    tirar_inalcancaveis(f);
    let vivos: HashSet<BlockId> = f.blocks.iter().map(|b| b.id).collect();
    t.pousos.retain(|b| vivos.contains(b));
    t.invocacoes.retain(|_, b| vivos.contains(&*b));
    conferir(f, &t);
    t
}

/// As invariantes que o emissor supõe, conferidas em toda função (um defeito
/// aqui viraria IR que o LLVM recusa, ou um pouso alcançado por um desvio
/// comum):
///
/// * o bloco com um `invoke` tem um só, e termina em
///   `CondBranch { cond: false, then: pouso dele, else: continuação }`;
/// * nenhum outro desvio leva a um pouso, e cada pouso tem exatamente um
///   `invoke`;
/// * o pouso não tem instruções e desvia sem condição.
fn conferir(f: &Function, t: &TabelasDaFuncao) {
    let mut chegadas: HashMap<BlockId, u32> = HashMap::new();
    for b in &f.blocks {
        let invocadas: Vec<BlockId> = b.instructions.iter().filter_map(|(v, _, _)| t.invocacoes.get(v).copied()).collect();
        let desvio = match &b.terminator {
            Terminator::CondBranch { cond: Operand::Constant(Constant::Bool(false)), then_block, else_block }
                if t.pousos.contains(then_block) && then_block != else_block =>
            {
                Some(*then_block)
            }
            _ => None,
        };
        let coerente = match (invocadas.as_slice(), desvio) {
            ([], None) => true,
            ([p], Some(d)) => *p == d,
            _ => false,
        };
        assert!(
            coerente,
            "bug do compilador (exceções por tabelas): em {}, o bloco b{} não casa o `invoke` com o pouso",
            f.symbol,
            b.id.0
        );
        if desvio.is_some() {
            assert!(
                b.instructions.last().is_some_and(|(v, _, _)| t.invocacoes.contains_key(v)),
                "bug do compilador (exceções por tabelas): em {}, o bloco b{} tem instrução depois do `invoke`",
                f.symbol,
                b.id.0
            );
        }
        for s in sucessores(&b.terminator) {
            if !t.pousos.contains(&s) {
                continue;
            }
            assert!(
                desvio == Some(s),
                "bug do compilador (exceções por tabelas): em {}, o bloco b{} desvia ao pouso b{} sem `invoke`",
                f.symbol,
                b.id.0,
                s.0
            );
            *chegadas.entry(s).or_insert(0) += 1;
        }
        if t.pousos.contains(&b.id) {
            assert!(
                b.instructions.is_empty() && matches!(b.terminator, Terminator::Branch(_)),
                "bug do compilador (exceções por tabelas): em {}, o pouso b{} não é só um desvio",
                f.symbol,
                b.id.0
            );
        }
    }
    for p in &t.pousos {
        assert!(
            chegadas.get(p) == Some(&1),
            "bug do compilador (exceções por tabelas): em {}, o pouso b{} não tem exatamente um `invoke`",
            f.symbol,
            p.0
        );
    }
}

/// A exceção está pendente neste ponto?
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pendencia {
    Limpa,
    Ligada,
    Talvez,
}

/// O que uma instrução faz com a pendência.
enum Efeito {
    Nada,
    /// Pode deixar a exceção pendente (e nunca a limpa).
    PodeLigar,
    Limpa,
    Liga,
}

fn efeito(inst: &Instruction, nl: &HashSet<String>) -> Efeito {
    match inst {
        Instruction::CallRuntime { name, .. } => match name.as_str() {
            // O emissor desenrola logo depois da limpeza que não limpou (o
            // desenrolar do isolado): no caminho normal, está limpa.
            "dartforge_exception_clear" => Efeito::Limpa,
            "dartforge_exception_throw" => Efeito::Liga,
            "dartforge_exception_pending"
            | "dartforge_exception_peek_ref"
            | "dartforge_exception_capturavel"
            | "dartforge_stack_trace_get" => Efeito::Nada,
            _ => {
                let e = efeitos_de(name);
                if e.lanca || e.chama_dart { Efeito::PodeLigar } else { Efeito::Nada }
            }
        },
        // Uma função Dart que retorna, retorna sem exceção pendente (a que
        // lança desenrola); um símbolo do runtime segue a pendência.
        Instruction::CallStatic { symbol, .. } => {
            if symbol.starts_with("dartforge_") && !nl.contains(symbol) { Efeito::PodeLigar } else { Efeito::Nada }
        }
        Instruction::ChamadaTipada { .. } => Efeito::Nada,
        // Emitidas como instruções do LLVM, sem chamada: não mexem na
        // pendência (`efeitos::instrucao_lanca` as conta por quem as cerca).
        Instruction::SDiv(..) | Instruction::SRem(..) | Instruction::DoubleToInt(_) | Instruction::CheckNotNull(_) => {
            Efeito::Nada
        }
        outra => {
            if instrucao_lanca(outra, nl) { Efeito::PodeLigar } else { Efeito::Nada }
        }
    }
}

fn depois(e: Pendencia, ef: &Efeito) -> Pendencia {
    match ef {
        Efeito::Nada => e,
        Efeito::Limpa => Pendencia::Limpa,
        Efeito::Liga => Pendencia::Ligada,
        Efeito::PodeLigar => {
            if e == Pendencia::Ligada { Pendencia::Ligada } else { Pendencia::Talvez }
        }
    }
}

/// A pendência no fim do bloco, dada a da entrada, e — quando o bloco
/// termina desviando por uma conferência que nada depois dela altera — os
/// sucessores `(com a exceção pendente, sem ela)`.
fn percorrer(b: &BasicBlock, entrada: Pendencia, nl: &HashSet<String>) -> (Pendencia, Option<(BlockId, BlockId)>) {
    let mut e = entrada;
    let mut conferencia: Option<ValueId> = None;
    for (v, inst, _) in &b.instructions {
        if e_conferencia(inst) {
            conferencia = Some(*v);
            continue;
        }
        let ef = efeito(inst, nl);
        if !matches!(ef, Efeito::Nada) {
            conferencia = None;
        }
        e = depois(e, &ef);
    }
    let desvio = match (&b.terminator, conferencia) {
        (Terminator::CondBranch { cond: Operand::Val(c), then_block, else_block }, Some(p)) if then_block != else_block => {
            b.instructions.iter().find(|(v, _, _)| v == c).and_then(|(_, inst, _)| match inst {
                Instruction::ICmp(ICmpOp::Ne, Operand::Val(x), Operand::Constant(Constant::Int(0))) if *x == p => {
                    Some((*then_block, *else_block))
                }
                Instruction::ICmp(ICmpOp::Eq, Operand::Val(x), Operand::Constant(Constant::Int(0))) if *x == p => {
                    Some((*else_block, *then_block))
                }
                _ => None,
            })
        }
        _ => None,
    };
    (e, desvio)
}

fn juntar(a: Option<Pendencia>, b: Pendencia) -> Pendencia {
    match a {
        None => b,
        Some(x) if x == b => b,
        Some(_) => Pendencia::Talvez,
    }
}

/// Como cada `Return` sai ([`TabelasDaFuncao::saidas`]): o ponto fixo da
/// pendência na entrada de cada bloco. A função começa sem exceção pendente
/// (quem chama só chama com ela limpa) e um pouso começa com ela ligada (o
/// desenrolamento só acontece com a exceção pendente).
fn saidas(f: &mut Function, nl: &HashSet<String>, t: &mut TabelasDaFuncao) {
    let n = f.blocks.len();
    if n == 0 {
        return;
    }
    let pos: HashMap<BlockId, usize> = f.blocks.iter().enumerate().map(|(k, b)| (b.id, k)).collect();
    let mut entrada: Vec<Option<Pendencia>> = vec![None; n];
    entrada[0] = Some(Pendencia::Limpa);
    for (k, b) in f.blocks.iter().enumerate() {
        if t.pousos.contains(&b.id) {
            entrada[k] = Some(Pendencia::Ligada);
        }
    }
    let mut fila: Vec<usize> = (0..n).filter(|k| entrada[*k].is_some()).collect();
    while let Some(k) = fila.pop() {
        let Some(e) = entrada[k] else { continue };
        let b = &f.blocks[k];
        let (saida, desvio) = percorrer(b, e, nl);
        let arestas: Vec<(BlockId, Pendencia)> = match desvio {
            Some((pendente, limpo)) => vec![(pendente, Pendencia::Ligada), (limpo, Pendencia::Limpa)],
            None => sucessores(&b.terminator).into_iter().map(|s| (s, saida)).collect(),
        };
        for (alvo, estado) in arestas {
            // O pouso só é alcançado pelo desenrolamento: sempre ligada.
            if t.pousos.contains(&alvo) {
                continue;
            }
            let Some(&j) = pos.get(&alvo) else { continue };
            let novo = juntar(entrada[j], estado);
            if entrada[j] != Some(novo) {
                entrada[j] = Some(novo);
                fila.push(j);
            }
        }
    }
    for (k, b) in f.blocks.iter().enumerate() {
        let Some(e) = entrada[k] else { continue };
        if !matches!(b.terminator, Terminator::Return(_)) {
            continue;
        }
        match percorrer(b, e, nl).0 {
            Pendencia::Limpa => {}
            Pendencia::Ligada => {
                t.saidas.insert(b.id, SaidaPorExcecao::Lanca);
            }
            Pendencia::Talvez => {
                t.saidas.insert(b.id, SaidaPorExcecao::Guarda);
            }
        }
    }
    // A limpeza da pendência onde ela está comprovadamente limpa não faz
    // nada: sai (o lowering põe uma antes de cada `return` e de cada salto, e
    // no modelo de conferência só a função que nunca lança se livrava
    // delas). Tirá-la não muda a pendência de ponto nenhum, então as saídas
    // calculadas acima continuam valendo.
    let mut sem_efeito: HashSet<ValueId> = HashSet::new();
    for (k, b) in f.blocks.iter().enumerate() {
        let Some(mut e) = entrada[k] else { continue };
        for (v, inst, _) in &b.instructions {
            if e_conferencia(inst) {
                continue;
            }
            let ef = efeito(inst, nl);
            if matches!(ef, Efeito::Limpa) && e == Pendencia::Limpa {
                sem_efeito.insert(*v);
            }
            e = depois(e, &ef);
        }
    }
    if !sem_efeito.is_empty() {
        for b in &mut f.blocks {
            b.instructions.retain(|(v, _, _)| !sem_efeito.contains(v));
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn modulo() -> Module {
        Module {
            functions: vec![Function {
                symbol: "chamador".into(),
                name: "chamador".into(),
                depuracao: None,
                params: Vec::new(),
                return_ty: Type::Ref,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(0),
                        Instruction::CallStatic { symbol: "pode_lancar".into(), args: Vec::new(), ret_ty: Type::Ref },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
                }],
            }],
            ..Default::default()
        }
    }

    #[test]
    fn prepara_arestas_sem_publicar_tabelas() {
        let mut m = modulo();
        let plano = preparar(&mut m);
        assert!(!m.excecoes_por_tabelas && m.tabelas.is_empty());
        let t = plano.sitios_da_funcao(0, &m.functions[0]);
        let pouso = t.invocacoes[&ValueId(0)];
        assert!(t.saidas.is_empty());
        assert!(matches!(m.functions[0].blocks[0].terminator,
            Terminator::CondBranch { then_block, .. } if then_block == pouso));
        let retorno = m.functions[0].blocks.iter().find(|b| matches!(b.terminator, Terminator::Return(_))).unwrap().id;
        materializar(&mut m, plano);
        assert_eq!(m.tabelas[0].saidas[&retorno], SaidaPorExcecao::Guarda);
    }

    #[test]
    fn arc_expoe_unwind_local_mesmo_sem_tratador_da_fonte() {
        for direta in [true, false] {
            for arc in [false, true] {
                let mut m = modulo();
                m.memoria_arc = arc;
                let f = &mut m.functions[0];
                if !direta {
                    f.params.push((ValueId(3), "alvo".into(), Type::Ref));
                    f.blocks[0].instructions[0].1 = Instruction::CallClosure {
                        closure: Operand::Val(ValueId(3)), args: vec![], nomes: vec![], ret_ty: Type::Ref,
                        tupla_tipos: Operand::Constant(Constant::Null),
                    };
                }
                f.blocks[0].instructions.extend([
                    (ValueId(1), Instruction::CallRuntime {
                        name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8,
                    }, Type::I8),
                    (ValueId(2), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(1)), Operand::Constant(Constant::Int(0))), Type::I1),
                ]);
                f.blocks[0].terminator = Terminator::CondBranch {
                    cond: Operand::Val(ValueId(2)), then_block: BlockId(1), else_block: BlockId(2),
                };
                f.blocks.extend([
                    BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
                    BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) },
                ]);
                assert!(super::super::valida(f));
                let plano = preparar(&mut m);
                let t = plano.sitios_da_funcao(0, &m.functions[0]);
                assert_eq!(t.invocacoes.contains_key(&ValueId(0)), arc);
                if arc {
                    let pouso = t.invocacoes[&ValueId(0)];
                    assert!(t.pousos.contains(&pouso));
                    assert!(super::super::valida(&m.functions[0]));
                }
                materializar(&mut m, plano);
                if arc { assert!(m.tabelas[0].saidas.contains_key(&BlockId(1))); }
            }
        }
    }

    #[test]
    #[should_panic(expected = "ordem das funções mudou")]
    fn consulta_rejeita_funcao_trocada_antes_da_materializacao() {
        let mut m = modulo();
        let plano = preparar(&mut m);
        m.functions[0].symbol = "outra_funcao".into();
        plano.sitios_da_funcao(0, &m.functions[0]);
    }

    #[test]
    fn materializacao_recalcula_pendencia_depois_de_alterar_continuacao() {
        let mut m = modulo();
        let plano = preparar(&mut m);
        let retorno = m.functions[0].blocks.iter_mut().find(|b| matches!(b.terminator, Terminator::Return(_))).unwrap();
        retorno.instructions.push((ValueId(100), Instruction::CallRuntime {
            name: "dartforge_exception_clear".into(), args: Vec::new(), ret_ty: Type::Void,
        }, Type::Void));
        materializar(&mut m, plano);
        assert!(m.tabelas[0].saidas.is_empty(), "a continuação agora limpa a pendência nos dois caminhos");
    }

    #[test]
    #[should_panic(expected = "tem instrução depois do `invoke`")]
    fn recusa_sitio_que_deixou_de_terminar_o_bloco() {
        let mut m = modulo();
        let plano = preparar(&mut m);
        m.functions[0].blocks[0].instructions.push((ValueId(100), Instruction::Alloca(Type::Ref), Type::Ptr));
        materializar(&mut m, plano);
    }
}
