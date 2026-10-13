//! Exceções por tabelas (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13): o
//! passe que reescreve a HIR já otimizada do modelo "pendência conferida
//! depois de cada chamada" para o modelo "a chamada Dart desenrola".
//!
//! **O contrato do modo.** Uma função Dart nunca volta a quem a chamou com a
//! exceção pendente: ela desenrola a pilha (`@df.throw`) até o pouso
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
//!    desenrolar ([`is_site`]) — e decide o destino do desenrolamento:
//!    * chamada direta seguida da conferência, com tratador que só devolve
//!      o valor padrão ([`pure_exit`]): a conferência some e a chamada
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
//!    `Return` ([`Pending`]): limpa, retorna; ligada, desenrola
//!    ([`ExceptionExit::Throws`]); talvez, confere e decide
//!    ([`ExceptionExit::Guard`]).
//!
//! O resultado ([`FunctionTables`], em [`Module::tables`]) é o que o
//! emissor lê (`llvm/mod.rs`). Uma chamada com pouso é a última instrução do
//! bloco dela; o terminador do bloco é
//! `CondBranch { cond: false, then: landing_pad, else: continuesção }`, que mantém
//! o pouso no grafo de fluxo (vivacidade das raízes, blocos alcançáveis) e
//! que o emissor escreve como o desvio à continuação.

//! Em ARC, os atalhos que deixam unwind atravessar um quadro sem pouso
//! ficam desabilitados: a aresta local é necessária à inserção de cleanup.
//! O passe expõe essa aresta; ainda não insere a limpeza de owners.

use super::cfg::successors;
use super::effects::{is_check, instruction_throws, do_not_throw};
use super::operands::{default_constant, largest_ids, operands, terminator_operands, replace};
use super::simplify::remove_unreachable;
use crate::hir::*;
use crate::llvm::externs::effects_of;
use std::collections::{HashMap, HashSet};

/// Reescreve o módulo para as exceções por tabelas e grava a decisão de cada
/// função em [`Module::tables`]. Roda depois de [`super::optimize`] (e
/// também com os passes desligados: só depende da forma que o lowering dá).
pub fn apply(module: &mut Module) {
    let plan = prepare(module);
    materialize(module, plan);
}

/// Inventário dos sítios preparados, ainda sem decisões sobre retornos.
/// Entre as etapas, preservar IDs e ordem das funções; transformações do
/// CFG precisam atualizar este inventário antes de materializar tabelas.
pub(super) struct ExceptionPlan {
    functions: Vec<(String, FunctionTables)>,
}

impl ExceptionPlan {
    /// Consulta os sítios antes de publicar tabelas, conferindo a identidade
    /// da função. Os passes intermediários leem o plano sem habilitar LLVM.
    pub(super) fn function_sites(&self, index: usize, f: &Function) -> &FunctionTables {
        let (symbol, sites) = self.functions.get(index)
            .expect("bug do compilador: função ausente do plano excepcional");
        assert_eq!(&f.symbol, symbol, "bug do compilador: ordem das funções mudou depois da preparação excepcional");
        sites
    }
}

/// Expõe pousos e continuações no CFG sem publicar tabelas para o emissor.
pub(super) fn prepare(module: &mut Module) -> ExceptionPlan {
    assert!(!module.excecoes_por_tabelas, "bug do compilador: CFG excepcional já materializado");
    let nl = do_not_throw(module);
    // Sem prova de ausência de owners, ARC precisa de uma saída local
    // para inserir cleanup mesmo quando não há catch/finally na fonte.
    let preserve_cleanup = module.arc_memory;
    // Preparação ARC publicada: closures e seletores vão pela invocação
    // uniforme do runtime, cujas portas devolvem a exceção pendente; ficam
    // com a conferência, sem invoke.
    let uniforms = !(module.arc_memory && crate::alvo::arc_prepared().unwrap_or(false));
    let mut functions = Vec::with_capacity(module.functions.len());
    for f in &mut module.functions {
        let sites = if super::valid(f) { from_function(f, &nl, preserve_cleanup, uniforms) } else { FunctionTables::default() };
        functions.push((f.symbol.clone(), sites));
    }
    ExceptionPlan { functions }
}

/// Reconstrói decisões de saída sobre a HIR final e valida os sítios antes
/// de publicar as tabelas. Não roda simplificação genérica nos pousos.
pub(super) fn materialize(module: &mut Module, plan: ExceptionPlan) {
    assert_eq!(plan.functions.len(), module.functions.len(), "bug do compilador: funções mudaram depois da preparação excepcional");
    let nl = do_not_throw(module);
    let mut tables = Vec::with_capacity(module.functions.len());
    for (index, f) in module.functions.iter().enumerate() {
        let t = plan.function_sites(index, f);
        if super::valid(f) {
            check(f, t);
        }
    }
    for (f, (symbol, mut t)) in module.functions.iter_mut().zip(plan.functions) {
        assert_eq!(f.symbol, symbol, "bug do compilador: ordem das funções mudou depois da preparação excepcional");
        if super::valid(f) {
            exits(f, &nl, &mut t);
        }
        if let Some(c) = &t.foreign_cleanup {
            c.check(f, &t).expect("cleanup estrangeiro obsoleto na materialização");
        }
        tables.push(t);
    }
    module.tables = tables;
    module.excecoes_por_tabelas = true;
}

/// A instrução chama código Dart que pode desenrolar?
///
/// * `CallStatic` de uma função que pode lançar. Ficam de fora as que não
///   lançam (`effects::do_not_throw`), os símbolos do runtime (`dartforge_…`,
///   que seguem o protocolo da pendência) e o getter de um tipo
///   (`df.rti.…`, `lower/rti.rs`), que o lowering chama sem conferir: se ele
///   lançar, o desenrolamento atravessa o quadro;
/// * a chamada do corpo tipado de uma closure, a de um valor função e a por
///   seletor.
fn is_site(inst: &Instruction, nl: &HashSet<String>, uniforms: bool) -> bool {
    match inst {
        Instruction::CallStatic { symbol, .. } => {
            !nl.contains(symbol) && !symbol.starts_with("df.rti.") && !symbol.starts_with("dartforge_")
        }
        Instruction::TypedCall { .. } => true,
        Instruction::CallClosure { .. }
        | Instruction::CallClosureForward { .. }
        | Instruction::CallSelector { .. }
        | Instruction::CallSelectorForward { .. } => uniforms,
        _ => false,
    }
}

/// O sítio só sai por desenrolamento (o alvo é sempre código Dart)? A
/// chamada de closure e a por seletor também podem voltar com a exceção
/// pendente (`@df_clo_invalido`, a entrada de `noSuchMethod`).
fn only_unwinds(inst: &Instruction) -> bool {
    matches!(inst, Instruction::CallStatic { .. } | Instruction::TypedCall { .. })
}

/// A forma que `FnBuilder::emit_call_with_check` dá a um sítio na posição
/// `i`: `[call][p = pending][c = p != 0]` no fim do bloco e
/// `CondBranch(c, handler, continuesção)`.
struct Check {
    p: ValueId,
    c: ValueId,
    handler: BlockId,
    continuation: BlockId,
}

fn site_check(b: &BasicBlock, i: usize) -> Option<Check> {
    if b.instructions.len() != i + 3 {
        return None;
    }
    let (p, inst_p, _) = &b.instructions[i + 1];
    if !is_check(inst_p) {
        return None;
    }
    let (c, inst_c, _) = &b.instructions[i + 2];
    let Instruction::ICmp(ICmpOp::Ne, Operand::Val(read_done), Operand::Constant(Constant::Int(0))) = inst_c else {
        return None;
    };
    if read_done != p {
        return None;
    }
    let Terminator::CondBranch { cond: Operand::Val(cond), then_block, else_block } = &b.terminator else {
        return None;
    };
    if cond != c || then_block == else_block {
        return None;
    }
    Some(Check { p: *p, c: *c, handler: *then_block, continuation: *else_block })
}

/// O tratador só devolve o valor padrão (a saída por exceção de uma função
/// sem `catch` nem `finally` em volta)? Blocos sem instrução além de `phi`,
/// encadeados por desvio incondicional até um `Return`. Um bloco de junção
/// que só reconfere a pendência (`phi`, `p = pending`, `c = p != 0`,
/// `CondBranch(c, …)`; a forma que a entrada uniforme `$ent` dá à junção do
/// caminho normal com o da exceção) é atravessado pelo lado da exceção:
/// vindo do tratador, a pendência está ligada e a reconferência sempre vai
/// para lá. Sem isto, todo `$ent` ganhava um pouso que pega tudo e devolve a
/// pendência, e a chamada dinâmica do modo tabelas custava mais que a do
/// modo de conferência.
fn pure_exit(f: &Function, pos: &HashMap<BlockId, usize>, de: BlockId) -> bool {
    let mut current = de;
    for _ in 0..64 {
        let Some(&k) = pos.get(&current) else { return false };
        let b = &f.blocks[k];
        let rest: Vec<&(ValueId, Instruction, Type)> = b.instructions.iter().filter(|(_, i, _)| !matches!(i, Instruction::Phi { .. })).collect();
        match (&rest[..], &b.terminator) {
            ([], Terminator::Return(_)) => return true,
            ([], Terminator::Branch(n)) => current = *n,
            ([(p, ip, _), (c, ic, _)], Terminator::CondBranch { cond: Operand::Val(cond), then_block, .. })
                if is_check(ip)
                    && cond == c
                    && matches!(ic, Instruction::ICmp(ICmpOp::Ne, Operand::Val(l), Operand::Constant(Constant::Int(0))) if l == p) =>
            {
                current = *then_block;
            }
            _ => return false,
        }
    }
    false
}

/// Quantas vezes cada valor é lido (operandos, entradas de `phi`,
/// terminadores).
fn count_uses(f: &Function) -> HashMap<ValueId, u32> {
    let mut uses: HashMap<ValueId, u32> = HashMap::new();
    {
        let mut counts = |o: &Operand| {
            if let Operand::Val(v) = o {
                *uses.entry(*v).or_insert(0) += 1;
            }
        };
        for b in &f.blocks {
            for (_, inst, _) in &b.instructions {
                operands(inst, &mut counts);
            }
            terminator_operands(&b.terminator, &mut counts);
        }
    }
    uses
}

fn minus(uses: &mut HashMap<ValueId, u32>, v: ValueId) {
    if let Some(n) = uses.get_mut(&v) {
        *n = n.saturating_sub(1);
    }
}

fn unused(uses: &HashMap<ValueId, u32>, v: ValueId) -> bool {
    uses.get(&v).copied().unwrap_or(0) == 0
}

/// Tira a comparação e a leitura da pendência que ficaram sem leitor.
fn remove_check(b: &mut BasicBlock, uses: &mut HashMap<ValueId, u32>, cf: &Check) {
    if !unused(uses, cf.c) {
        return;
    }
    b.instructions.retain(|(v, _, _)| *v != cf.c);
    minus(uses, cf.p);
    if unused(uses, cf.p) {
        b.instructions.retain(|(v, _, _)| *v != cf.p);
    }
}

fn false_operand() -> Operand {
    Operand::Constant(Constant::Bool(false))
}

fn from_function(f: &mut Function, nl: &HashSet<String>, preserve_cleanup: bool, uniforms: bool) -> FunctionTables {
    let mut t = FunctionTables::default();
    t.checks_stack = f.blocks.iter().any(|b| b.instructions.iter().any(|(_, inst, _)| is_site(inst, nl, uniforms)));
    let (mut next_v, mut next_b) = largest_ids(f);
    let mut uses = count_uses(f);
    let mut pos: HashMap<BlockId, usize> = f.blocks.iter().enumerate().map(|(k, b)| (b.id, k)).collect();
    // Um sítio por volta: o primeiro do bloco. Ou ele é o último do bloco (a
    // forma conferida), ou o resto do bloco vai para um bloco novo, no fim
    // da lista, visitado adiante.
    let mut bi = 0;
    while bi < f.blocks.len() {
        let Some(i) = f.blocks[bi].instructions.iter().position(|(_, inst, _)| is_site(inst, nl, uniforms)) else {
            bi += 1;
            continue;
        };
        let origin = f.blocks[bi].id;
        let (result, typ, direct) = {
            let (v, inst, ty) = &f.blocks[bi].instructions[i];
            (*v, crate::llvm::LlvmEmitter::result_type(inst, *ty), only_unwinds(inst))
        };
        let checked = site_check(&f.blocks[bi], i);
        let pure = !preserve_cleanup && checked.as_ref().is_some_and(|cf| pure_exit(f, &pos, cf.handler));
        match checked {
            // A conferência some: a chamada desenrola e ninguém aqui trata.
            Some(cf) if direct && pure => {
                f.blocks[bi].terminator = Terminator::Branch(cf.continuation);
                minus(&mut uses, cf.c);
                if let Some(&kt) = pos.get(&cf.handler) {
                    for (_, inst, _) in &mut f.blocks[kt].instructions {
                        if let Instruction::Phi { incoming, .. } = inst {
                            incoming.retain(|(o, op)| {
                                if *o != origin {
                                    return true;
                                }
                                if let Operand::Val(v) = op {
                                    minus(&mut uses, *v);
                                }
                                false
                            });
                        }
                    }
                }
                remove_check(&mut f.blocks[bi], &mut uses, &cf);
            }
            // A conferência some: a chamada é `invoke` e o pouso vai ao
            // tratador, com a exceção pendente — como a conferência ia.
            Some(cf) if direct => {
                next_b += 1;
                let landing_pad = BlockId(next_b);
                f.blocks[bi].terminator =
                    Terminator::CondBranch { cond: false_operand(), then_block: landing_pad, else_block: cf.continuation };
                minus(&mut uses, cf.c);
                if let Some(&kt) = pos.get(&cf.handler) {
                    for (_, inst, _) in &mut f.blocks[kt].instructions {
                        let Instruction::Phi { incoming, ty } = inst else { continue };
                        for (o, op) in incoming.iter_mut() {
                            if *o != origin {
                                continue;
                            }
                            *o = landing_pad;
                            let read_done = match &*op {
                                Operand::Val(v) => Some(*v),
                                Operand::Constant(_) => None,
                            };
                            // No pouso não existem o resultado da chamada
                            // nem a conferência: os valores que eles teriam
                            // (o padrão; pendente; verdadeiro).
                            if read_done == Some(result) {
                                *op = default_constant(*ty);
                            } else if read_done == Some(cf.p) {
                                minus(&mut uses, cf.p);
                                *op = Operand::Constant(Constant::Int(1));
                            } else if read_done == Some(cf.c) {
                                minus(&mut uses, cf.c);
                                *op = Operand::Constant(Constant::Bool(true));
                            }
                        }
                    }
                }
                pos.insert(landing_pad, f.blocks.len());
                f.blocks.push(BasicBlock {
                    id: landing_pad,
                    instructions: Vec::new(),
                    terminator: Terminator::Branch(cf.handler),
                });
                remove_check(&mut f.blocks[bi], &mut uses, &cf);
                t.invocations.insert(result, landing_pad);
                t.landing_pads.insert(landing_pad);
            }
            // Pode voltar pendente e quem trata só devolve o padrão: a
            // conferência fica e o desenrolamento atravessa o quadro.
            Some(_) if pure => {}
            // Emulação exata: o pouso continua depois da chamada, com o
            // valor padrão e a exceção pendente.
            _ => {
                next_b += 2;
                let landing_pad = BlockId(next_b - 1);
                let rest = BlockId(next_b);
                let tail = f.blocks[bi].instructions.split_off(i + 1);
                let terminator = std::mem::replace(
                    &mut f.blocks[bi].terminator,
                    Terminator::CondBranch { cond: false_operand(), then_block: landing_pad, else_block: rest },
                );
                // Os sucessores do bloco passam a ser sucessores do resto.
                for s in successors(&terminator) {
                    let Some(&k) = pos.get(&s) else { continue };
                    for (_, inst, _) in &mut f.blocks[k].instructions {
                        if let Instruction::Phi { incoming, .. } = inst {
                            for (o, _) in incoming.iter_mut() {
                                if *o == origin {
                                    *o = rest;
                                }
                            }
                        }
                    }
                }
                pos.insert(landing_pad, f.blocks.len());
                f.blocks.push(BasicBlock { id: landing_pad, instructions: Vec::new(), terminator: Terminator::Branch(rest) });
                pos.insert(rest, f.blocks.len());
                f.blocks.push(BasicBlock { id: rest, instructions: tail, terminator: terminator });
                if typ != Type::Void {
                    // O resultado, daqui em diante, é o da chamada ou o
                    // padrão (o que a função que lançou devolvia).
                    next_v += 1;
                    let new = ValueId(next_v);
                    replace(f, &|v| (v == result).then_some(Operand::Val(new)));
                    let k = pos[&rest];
                    f.blocks[k].instructions.insert(
                        0,
                        (
                            new,
                            Instruction::Phi {
                                incoming: vec![(origin, Operand::Val(result)), (landing_pad, default_constant(typ))],
                                ty: typ,
                            },
                            typ,
                        ),
                    );
                }
                t.invocations.insert(result, landing_pad);
                t.landing_pads.insert(landing_pad);
            }
        }
        bi += 1;
    }
    // Os tratadores que ficaram sem quem chegue a eles.
    remove_unreachable(f);
    let live_set: HashSet<BlockId> = f.blocks.iter().map(|b| b.id).collect();
    t.landing_pads.retain(|b| live_set.contains(b));
    t.invocations.retain(|_, b| live_set.contains(&*b));
    check(f, &t);
    t
}

/// As invariantes que o emissor supõe, conferidas em toda função (um defeito
/// aqui viraria IR que o LLVM recusa, ou um pouso alcançado por um desvio
/// comum):
///
/// * o bloco com um `invoke` tem um só, e termina em
///   `CondBranch { cond: false, then: landing_pad dele, else: continuesção }`;
/// * nenhum outro desvio leva a um pouso, e cada pouso tem exatamente um
///   `invoke`;
/// * o pouso não tem instruções e desvia sem condição.
fn check(f: &Function, t: &FunctionTables) {
    super::arc::check_resumes(f, t).expect("bug do compilador: Retoma inválida");
    let mut arrivals: HashMap<BlockId, u32> = HashMap::new();
    for b in &f.blocks {
        let invoked: Vec<BlockId> = b.instructions.iter().filter_map(|(v, _, _)| t.invocations.get(v).copied()).collect();
        let branch = match &b.terminator {
            Terminator::CondBranch { cond: Operand::Constant(Constant::Bool(false)), then_block, else_block }
                if t.landing_pads.contains(then_block) && then_block != else_block =>
            {
                Some(*then_block)
            }
            _ => None,
        };
        let coherent = match (invoked.as_slice(), branch) {
            ([], None) => true,
            ([p], Some(d)) => *p == d,
            _ => false,
        };
        assert!(
            coherent,
            "bug do compilador (exceções por tabelas): em {}, o bloco b{} não casa o `invoke` com o pouso",
            f.symbol,
            b.id.0
        );
        if branch.is_some() {
            assert!(
                b.instructions.last().is_some_and(|(v, _, _)| t.invocations.contains_key(v)),
                "bug do compilador (exceções por tabelas): em {}, o bloco b{} tem instrução depois do `invoke`",
                f.symbol,
                b.id.0
            );
        }
        for s in successors(&b.terminator) {
            if !t.landing_pads.contains(&s) {
                continue;
            }
            assert!(
                branch == Some(s),
                "bug do compilador (exceções por tabelas): em {}, o bloco b{} desvia ao pouso b{} sem `invoke`",
                f.symbol,
                b.id.0,
                s.0
            );
            *arrivals.entry(s).or_insert(0) += 1;
        }
        if t.landing_pads.contains(&b.id) {
            assert!(
                (b.instructions.is_empty() && matches!(b.terminator, Terminator::Branch(_)))
                    || (t.exits.get(&b.id) == Some(&ExceptionExit::Resume)
                        && b.instructions.iter().all(|(_, i, _)| super::arc::resume_instruction(i))
                        && matches!(b.terminator, Terminator::Return(_))),
                "bug do compilador (exceções por tabelas): em {}, o pouso b{} não é só um desvio",
                f.symbol,
                b.id.0
            );
        }
    }
    for p in &t.landing_pads {
        assert!(
            arrivals.get(p) == Some(&1),
            "bug do compilador (exceções por tabelas): em {}, o pouso b{} não tem exatamente um `invoke`",
            f.symbol,
            p.0
        );
    }
}

/// A exceção está pendente neste ponto?
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pending {
    Cleans,
    Ligada,
    Maybe,
}

/// O que uma instrução faz com a pendência.
enum Effect {
    Nada,
    /// Pode deixar a exceção pendente (e nunca a limpa).
    MayLink,
    Cleans,
    Links,
}

fn effect(inst: &Instruction, nl: &HashSet<String>) -> Effect {
    match inst {
        Instruction::CallRuntime { name, .. } => match name.as_str() {
            // O emissor desenrola logo depois da limpeza que não limpou (o
            // desenrolar do isolado): no caminho normal, está limpa.
            "dartforge_exception_clear" => Effect::Cleans,
            "dartforge_exception_throw" => Effect::Links,
            "dartforge_exception_pending"
            | "dartforge_exception_peek_ref"
            | "dartforge_exception_capturavel"
            | "dartforge_stack_trace_get" => Effect::Nada,
            _ => {
                let e = effects_of(name);
                if e.throws || e.calls_dart { Effect::MayLink } else { Effect::Nada }
            }
        },
        // Uma função Dart que retorna, retorna sem exceção pendente (a que
        // lança desenrola); um símbolo do runtime segue a pendência.
        Instruction::CallStatic { symbol, .. } => {
            if symbol.starts_with("dartforge_") && !nl.contains(symbol) { Effect::MayLink } else { Effect::Nada }
        }
        Instruction::TypedCall { .. } => Effect::Nada,
        // Emitidas como instruções do LLVM, sem chamada: não mexem na
        // pendência (`effects::instruction_throws` as conta por quem as cerca).
        Instruction::SDiv(..) | Instruction::SRem(..) | Instruction::DoubleToInt(_) | Instruction::CheckNotNull(_) => {
            Effect::Nada
        }
        another => {
            if instruction_throws(another, nl) { Effect::MayLink } else { Effect::Nada }
        }
    }
}

fn after(e: Pending, ef: &Effect) -> Pending {
    match ef {
        Effect::Nada => e,
        Effect::Cleans => Pending::Cleans,
        Effect::Links => Pending::Ligada,
        Effect::MayLink => {
            if e == Pending::Ligada { Pending::Ligada } else { Pending::Maybe }
        }
    }
}

/// A pendência no fim do bloco, dada a da entrada, e — quando o bloco
/// termina desviando por uma conferência que nada depois dela altera — os
/// sucessores `(com a exceção pending, without ela)`.
fn walk(b: &BasicBlock, entry: Pending, nl: &HashSet<String>) -> (Pending, Option<(BlockId, BlockId)>) {
    let mut e = entry;
    let mut check: Option<ValueId> = None;
    for (v, inst, _) in &b.instructions {
        if is_check(inst) {
            check = Some(*v);
            continue;
        }
        let ef = effect(inst, nl);
        if !matches!(ef, Effect::Nada) {
            check = None;
        }
        e = after(e, &ef);
    }
    let branch = match (&b.terminator, check) {
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
    (e, branch)
}

fn join(a: Option<Pending>, b: Pending) -> Pending {
    match a {
        None => b,
        Some(x) if x == b => b,
        Some(_) => Pending::Maybe,
    }
}

/// Como cada `Return` sai ([`FunctionTables::exits`]): o ponto fixo da
/// pendência na entrada de cada bloco. A função começa sem exceção pendente
/// (quem chama só chama com ela limpa) e um pouso começa com ela ligada (o
/// desenrolamento só acontece com a exceção pendente).
fn exits(f: &mut Function, nl: &HashSet<String>, t: &mut FunctionTables) {
    let n = f.blocks.len();
    if n == 0 {
        return;
    }
    let pos: HashMap<BlockId, usize> = f.blocks.iter().enumerate().map(|(k, b)| (b.id, k)).collect();
    let mut entry: Vec<Option<Pending>> = vec![None; n];
    entry[0] = Some(Pending::Cleans);
    for (k, b) in f.blocks.iter().enumerate() {
        if t.landing_pads.contains(&b.id) {
            entry[k] = Some(Pending::Ligada);
        }
    }
    let mut queue: Vec<usize> = (0..n).filter(|k| entry[*k].is_some()).collect();
    while let Some(k) = queue.pop() {
        let Some(e) = entry[k] else { continue };
        let b = &f.blocks[k];
        let (exit, branch) = walk(b, e, nl);
        let edges: Vec<(BlockId, Pending)> = match branch {
            Some((pending, clean)) => vec![(pending, Pending::Ligada), (clean, Pending::Cleans)],
            None => successors(&b.terminator).into_iter().map(|s| (s, exit)).collect(),
        };
        for (alvo, state) in edges {
            // O pouso só é alcançado pelo desenrolamento: sempre ligada.
            if t.landing_pads.contains(&alvo) {
                continue;
            }
            let Some(&j) = pos.get(&alvo) else { continue };
            let new = join(entry[j], state);
            if entry[j] != Some(new) {
                entry[j] = Some(new);
                queue.push(j);
            }
        }
    }
    for (k, b) in f.blocks.iter().enumerate() {
        let Some(e) = entry[k] else { continue };
        if !matches!(b.terminator, Terminator::Return(_)) {
            continue;
        }
        if t.exits.get(&b.id) == Some(&ExceptionExit::Resume) { continue; }
        match walk(b, e, nl).0 {
            Pending::Cleans => {}
            Pending::Ligada => {
                t.exits.insert(b.id, ExceptionExit::Throws);
            }
            Pending::Maybe => {
                t.exits.insert(b.id, ExceptionExit::Guard);
            }
        }
    }
    // A limpeza da pendência onde ela está comprovadamente limpa não faz
    // nada: sai (o lowering põe uma antes de cada `return` e de cada salto, e
    // no modelo de conferência só a função que nunca lança se livrava
    // delas). Tirá-la não muda a pendência de ponto nenhum, então as saídas
    // calculadas acima continuam valendo.
    let mut without_effect: HashSet<ValueId> = HashSet::new();
    for (k, b) in f.blocks.iter().enumerate() {
        let Some(mut e) = entry[k] else { continue };
        for (v, inst, _) in &b.instructions {
            if is_check(inst) {
                continue;
            }
            let ef = effect(inst, nl);
            if matches!(ef, Effect::Cleans) && e == Pending::Cleans {
                without_effect.insert(*v);
            }
            e = after(e, &ef);
        }
    }
    if !without_effect.is_empty() {
        for b in &mut f.blocks {
            b.instructions.retain(|(v, _, _)| !without_effect.contains(v));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materialization_preserves_resume_and_checks_cleanup_before_publishing() {
        let mut m = module();
        m.functions[0].blocks[0].terminator = Terminator::CondBranch {
            cond: Operand::Constant(Constant::Bool(false)), then_block: BlockId(2), else_block: BlockId(1),
        };
        m.functions[0].blocks.extend([
            BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) },
            BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
        ]);
        let t = FunctionTables {
            invocations: [(ValueId(0), BlockId(2))].into(), landing_pads: [BlockId(2)].into(),
            exits: [(BlockId(2), ExceptionExit::Resume)].into(), ..Default::default()
        };
        let plan = ExceptionPlan { functions: vec![(m.functions[0].symbol.clone(), t.clone())] };
        materialize(&mut m, plan);
        assert_eq!(m.tables[0].exits[&BlockId(2)], ExceptionExit::Resume);
        m.functions[0].blocks[2].terminator = Terminator::Return(Some(Operand::Val(ValueId(0))));
        assert!(std::panic::catch_unwind(|| check(&m.functions[0], &t)).is_err());
    }

    fn module() -> Module {
        Module {
            functions: vec![Function {
                symbol: "chamador".into(),
                name: "chamador".into(),
                debug_info: None,
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
    fn prepares_edges_without_publishing_tables() {
        let mut m = module();
        let plan = prepare(&mut m);
        assert!(!m.excecoes_por_tabelas && m.tables.is_empty());
        let t = plan.function_sites(0, &m.functions[0]);
        let landing_pad = t.invocations[&ValueId(0)];
        assert!(t.exits.is_empty());
        assert!(matches!(m.functions[0].blocks[0].terminator,
            Terminator::CondBranch { then_block, .. } if then_block == landing_pad));
        let ret = m.functions[0].blocks.iter().find(|b| matches!(b.terminator, Terminator::Return(_))).unwrap().id;
        materialize(&mut m, plan);
        assert_eq!(m.tables[0].exits[&ret], ExceptionExit::Guard);
    }

    #[test]
    fn arc_exposes_local_unwind_even_without_source_handler() {
        for direct in [true, false] {
            for arc in [false, true] {
                let mut m = module();
                m.arc_memory = arc;
                let f = &mut m.functions[0];
                if !direct {
                    f.params.push((ValueId(3), "alvo".into(), Type::Ref));
                    f.blocks[0].instructions[0].1 = Instruction::CallClosure {
                        closure: Operand::Val(ValueId(3)), args: vec![], names: vec![], ret_ty: Type::Ref,
                        tuple_types: Operand::Constant(Constant::Null),
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
                assert!(super::super::valid(f));
                let plan = prepare(&mut m);
                let t = plan.function_sites(0, &m.functions[0]);
                assert_eq!(t.invocations.contains_key(&ValueId(0)), arc);
                if arc {
                    let landing_pad = t.invocations[&ValueId(0)];
                    assert!(t.landing_pads.contains(&landing_pad));
                    assert!(super::super::valid(&m.functions[0]));
                }
                materialize(&mut m, plan);
                if arc { assert!(m.tables[0].exits.contains_key(&BlockId(1))); }
            }
        }
    }

    #[test]
    #[should_panic(expected = "ordem das funções mudou")]
    fn query_rejects_swapped_function_before_materialization() {
        let mut m = module();
        let plan = prepare(&mut m);
        m.functions[0].symbol = "outra_funcao".into();
        plan.function_sites(0, &m.functions[0]);
    }

    #[test]
    fn materialization_recomputes_pending_after_changing_continuation() {
        let mut m = module();
        let plan = prepare(&mut m);
        let ret = m.functions[0].blocks.iter_mut().find(|b| matches!(b.terminator, Terminator::Return(_))).unwrap();
        ret.instructions.push((ValueId(100), Instruction::CallRuntime {
            name: "dartforge_exception_clear".into(), args: Vec::new(), ret_ty: Type::Void,
        }, Type::Void));
        materialize(&mut m, plan);
        assert!(m.tables[0].exits.is_empty(), "a continuação agora limpa a pendência nos dois caminhos");
    }

    #[test]
    #[should_panic(expected = "tem instrução depois do `invoke`")]
    fn rejects_site_that_stopped_terminating_block() {
        let mut m = module();
        let plan = prepare(&mut m);
        m.functions[0].blocks[0].instructions.push((ValueId(100), Instruction::Alloca(Type::Ref), Type::Ptr));
        materialize(&mut m, plan);
    }
}
