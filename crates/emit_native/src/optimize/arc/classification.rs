//! Inventário tipado de ownership fornecido pelos produtores da HIR.
//! A cobertura e as dependências são conferidas antes da vivacidade;
//! contratos de externs, proveniência, escopo e consumo exigem outros passes.

use super::super::cfg::Cfg;
use super::{Liveness, liveness_with_borrows};
use crate::hir::*;
use std::collections::{HashMap, HashSet};

/// Quem sustenta um empréstimo: o chamador ou um valor desta função.
///
/// ```
/// use dartforge_emit_native::{hir::ValueId, optimize::arc::OrigemOwner};
/// let origem = OrigemOwner::Valor(ValueId(3));
/// assert_ne!(origem, OrigemOwner::Chamador);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerOrigin {
    /// Parâmetro emprestado pelo chamador durante a invocação.
    Caller,
    /// Valor gerenciado ou slot proprietário local, com proveniência explícita.
    Value(ValueId),
}

/// Classificação semântica de uma definição HIR (§20.1).
///
/// `Owned` descreve a obrigação de um token, não um incremento já emitido.
/// O número do escopo é uma identidade; seus limites ainda precisam ser
/// descritos e conferidos pelo verificador de empréstimos.
///
/// ```
/// use dartforge_emit_native::optimize::arc::{Ownership, OrigemOwner};
/// let parametro = Ownership::Borrowed { owner: OrigemOwner::Chamador, escopo: 0 };
/// assert_ne!(parametro, Ownership::Owned);
/// assert_ne!(parametro, Ownership::Trivial);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ownership {
    /// Não tem obrigação de ownership gerenciado.
    Trivial,
    /// Define um owner que deverá ser consumido, movido ou devolvido.
    Owned,
    /// Alias sustentado por um owner durante um escopo.
    Borrowed {
        /// Origem da sustentação, que não cria um token independente.
        owner: OwnerOrigin,
        /// Identidade do escopo de empréstimo.
        scope: u32,
    },
}

/// Consome um inventário completo e inclui dependências borrowed na vivacidade.
///
/// A classificação deve vir dos contratos e da proveniência semântica,
/// inclusive quando a representação for `I64` ou um slot `Ptr`. A função
/// confere cobertura, origem externa de parâmetros, dominância SSA dos owners
/// locais em blocos alcançáveis e cadeias de sustentação;
/// não prova a classificação, os limites dos escopos nem o consumo de tokens.
/// A função deve ter CFG/SSA válidos. Definições `Phi` são simultâneas na
/// entrada do bloco. A existência do resultado de `invoke` só no sucesso
/// ainda precisa ser conferida pela análise de tokens excepcional.
/// Incluir todas as definições e parâmetros, também `Void` como `Trivial`.
///
/// # Erros
/// Retorna diagnóstico com símbolo e ID se faltar classificação, houver ID
/// estranho, um resultado local alegar empréstimo do chamador, ou a definição
/// do owner não dominar a do alias. Dependências
/// inválidas mantêm o diagnóstico `ARC003` de [`liveness_with_borrows`].
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "f".into(), name: "f".into(), depuracao: None,
///     params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] };
/// let classes = HashMap::from([(ValueId(0), Ownership::Borrowed {
///     owner: OrigemOwner::Chamador, escopo: 0 })]);
/// let v = vivacidade_classificada(&f, &classes)?;
/// assert!(v.entrada[&BlockId(0)].contains(&ValueId(0)));
/// # Ok::<(), String>(())
/// ```
pub fn classified_liveness(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
) -> Result<Liveness, String> {
    let parameters: HashSet<ValueId> = f.params.iter().map(|(v, _, _)| *v).collect();
    let existing_set: HashSet<ValueId> = parameters
        .iter()
        .copied()
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| b.instructions.iter().map(|(v, _, _)| *v)),
        )
        .collect();
    let mut ids: Vec<ValueId> = existing_set.iter().copied().collect();
    ids.sort_by_key(|v| v.0);
    for id in ids {
        if !classes.contains_key(&id) {
            let def = f.blocks.iter().flat_map(|b| &b.instructions).find(|(v, _, _)| *v == id)
                .map_or_else(|| format!("parâmetro {:?}", f.params.iter().find(|(v, _, _)| *v == id).map(|p| p.2)),
                    |(_, i, t)| format!("{i:?}: {t:?}"));
            return Err(format!(
                "ownership em {}: v{} sem classificação ({def})",
                f.symbol, id.0
            ));
        }
    }
    let mut ids: Vec<ValueId> = classes.keys().copied().collect();
    ids.sort_by_key(|v| v.0);
    // As definições locais de tipo Ref (uma trivial dessas é null, constante
    // ou um Phi delas); parâmetros não entram.
    let ref_types: HashSet<ValueId> = f.blocks.iter().flat_map(|b| &b.instructions)
        .filter(|(_, _, t)| *t == Type::Ref).map(|(v, _, _)| *v).collect();
    let mut references = HashSet::new();
    let mut borrows = HashMap::new();
    for id in ids {
        if !existing_set.contains(&id) {
            return Err(format!(
                "ownership em {}: v{} não existe na função",
                f.symbol, id.0
            ));
        }
        match classes[&id] {
            Ownership::Trivial => {}
            Ownership::Owned => {
                references.insert(id);
            }
            Ownership::Borrowed { owner, .. } => {
                references.insert(id);
                match owner {
                    OwnerOrigin::Caller if !parameters.contains(&id) => {
                        return Err(format!(
                            "ownership em {}: v{} local não é parâmetro emprestado pelo chamador",
                            f.symbol, id.0
                        ));
                    }
                    OwnerOrigin::Caller => {}
                    // Emprestado de um valor trivial (null ou constante
                    // permanente, inclusive por um Phi só de nulls): nada a
                    // manter vivo.
                    OwnerOrigin::Value(owner) if classes.get(&owner) == Some(&Ownership::Trivial)
                        && ref_types.contains(&owner) => {}
                    OwnerOrigin::Value(owner) => {
                        borrows.insert(id, owner);
                    }
                }
            }
        }
    }
    check_dominance(f, &borrows)?;
    liveness_with_borrows(f, &references, &borrows).map_err(|e| e.to_string())
}

/// `(block, posição, phi)`: parâmetros ficam fora dos blocos e dominam
/// suas instruções; phis são produzidos simultaneamente na entrada.
fn check_dominance(
    f: &Function,
    borrows: &HashMap<ValueId, ValueId>,
) -> Result<(), String> {
    let cfg = Cfg::new(f);
    let mut definitions: HashMap<ValueId, (Option<usize>, usize, bool)> = f
        .params
        .iter()
        .map(|(v, _, _)| (*v, (None, 0, false)))
        .collect();
    for (bi, b) in f.blocks.iter().enumerate() {
        for (i, (v, inst, _)) in b.instructions.iter().enumerate() {
            definitions.insert(*v, (Some(bi), i, matches!(inst, Instruction::Phi { .. })));
        }
    }
    let mut aliases: Vec<ValueId> = borrows.keys().copied().collect();
    aliases.sort_by_key(|v| v.0);
    for alias in aliases {
        let owner = borrows[&alias];
        // IDs ausentes recebem o diagnóstico da validação de dependências.
        let (Some(&(b_alias, i_alias, phi_alias)), Some(&(b_owner, i_owner, phi_owner))) =
            (definitions.get(&alias), definitions.get(&owner))
        else {
            continue;
        };
        let dominates = match (b_owner, b_alias) {
            (None, _) => true,
            (Some(_), None) => false,
            (_, Some(b)) if !cfg.reachable(b) => true,
            (Some(o), Some(b)) if o == b => {
                if phi_alias {
                    phi_owner
                } else {
                    phi_owner || i_owner < i_alias
                }
            }
            (Some(o), Some(mut b)) => loop {
                if o == b {
                    break true;
                }
                if b == cfg.idom[b] {
                    break false;
                }
                b = cfg.idom[b];
            },
        };
        if !dominates {
            let block = b_alias.map_or_else(
                || "parâmetro".into(),
                |b| format!("b{} instrução {}", f.blocks[b].id.0, i_alias),
            );
            return Err(format!(
                "ARC003 em {}: {block}: v{alias_id} depende de owner v{owner_id} cuja definição não domina o alias; caminho [{alias_id}, {owner_id}]",
                f.symbol,
                alias_id = alias.0,
                owner_id = owner.0
            ));
        }
    }
    Ok(())
}

/// Confere também que owners produzidos por `invoke` existem no caminho do alias.
///
/// Exige CFG/SSA válidos e o plano excepcional da mesma HIR preparada.
/// A disponibilidade exige
/// dominância da aresta de sucesso, inclusive quando pouso e sucesso se juntam.
/// Usos diretos do resultado também são conferidos; entradas de Phi usam
/// a disponibilidade na sua aresta, não a do bloco de destino.
/// Não prova consumo de tokens nem resultados de chamadas sem `invoke`.
///
/// # Erros
/// Retorna erro se o plano não corresponder à forma preparada, ou se um alias
/// puder ser definido sem atravessar o sucesso de seu owner local.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "f".into(), name: "f".into(), depuracao: None,
///     params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
///     id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
/// vivacidade_classificada_com_excecoes(&f, &HashMap::new(), &TabelasDaFuncao::default())?;
/// # Ok::<(), String>(())
/// ```
pub fn classified_liveness_with_exceptions(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    tables: &FunctionTables,
) -> Result<Liveness, String> {
    liveness_with_exits(f, classes, tables, &HashMap::new())
}

pub(super) fn liveness_with_exits(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    tables: &FunctionTables,
    pendings: &HashMap<ValueId, BlockId>,
) -> Result<Liveness, String> {
    let v = classified_liveness(f, classes)?;
    let cfg = Cfg::new(f);
    let pos: HashMap<BlockId, usize> = f
        .blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id, i))
        .collect();
    let defs: HashMap<ValueId, usize> = f
        .blocks
        .iter()
        .enumerate()
        .flat_map(|(i, b)| b.instructions.iter().map(move |(v, _, _)| (*v, i)))
        .collect();
    if pendings
        .keys()
        .any(|v| tables.invocations.contains_key(v))
    {
        return Err(format!(
            "ownership em {}: saída pending duplicada como invoke",
            f.symbol
        ));
    }
    let mut calls: Vec<_> = tables.invocations.iter().chain(pendings).collect();
    calls.sort_by_key(|(v, _)| v.0);
    for (&call, &landing_pad) in calls {
        let Some(&origin) = defs.get(&call) else {
            return Err(format!(
                "ownership em {}: invoke v{} ausente",
                f.symbol, call.0
            ));
        };
        let b = &f.blocks[origin];
        if tables.invocations.contains_key(&call)
            && b.instructions
                .iter()
                .any(|(v, inst, _)| *v == call && matches!(inst, Instruction::CallRuntime { .. }))
        {
            return Err(format!(
                "ownership em {}: runtime v{} exige conferência de pendência, não invoke LLVM",
                f.symbol, call.0
            ));
        }
        let success = if pendings.contains_key(&call) {
            check_pending(b, call, landing_pad).and_then(|s| pos.get(&s).copied())
        } else {
            match &b.terminator {
                Terminator::CondBranch {
                    cond: Operand::Constant(Constant::Bool(false)),
                    then_block,
                    else_block,
                } if *then_block == landing_pad
                    && *else_block != landing_pad
                    && b.instructions.last().is_some_and(|(v, _, _)| *v == call)
                    && tables.landing_pads.contains(&landing_pad) =>
                {
                    pos.get(else_block).copied()
                }
                _ => None,
            }
        }
        .ok_or_else(|| {
            format!(
                "ownership em {}: invoke v{} sem aresta de sucesso válida",
                f.symbol, call.0
            )
        })?;
        // Retirar a aresta distingue dominância de bloco e de resultado.
        let without_success = reachable_without_edge(&cfg, (origin, success));
        let mut aliases: Vec<_> = classes.iter().filter_map(|(&alias,class)| {
            matches!(class, Ownership::Borrowed { owner: OwnerOrigin::Value(owner), .. } if *owner == call)
                .then_some(alias)
        }).collect();
        aliases.sort_by_key(|v| v.0);
        for alias in aliases {
            if let Some(&block) = defs.get(&alias)
                && cfg.reachable(block)
                && without_success.contains(&block)
            {
                return Err(format!(
                    "ARC003 em {}: b{}: v{} depende de resultado v{} indisponível no caminho excepcional; caminho [{}, {}]",
                    f.symbol, f.blocks[block].id.0, alias.0, call.0, alias.0, call.0
                ));
            }
        }
        check_result_uses(f, &cfg, &pos, call, (origin, success), &without_success)?;
    }
    Ok(v)
}

/// Confere o sufixo de leitura de pendência sem criar um pouso LLVM.
pub(super) fn check_pending(b: &BasicBlock, call: ValueId, error: BlockId) -> Option<BlockId> {
    let n = b.instructions.len();
    if n < 3 {
        return None;
    }
    let (v, call_inst, _) = &b.instructions[n - 3];
    let (p, read, ty_p) = &b.instructions[n - 2];
    let (c, comparison, ty_c) = &b.instructions[n - 1];
    if *v != call
        || !matches!(
            call_inst,
            Instruction::CallRuntime { .. } | Instruction::CallStatic { .. }
                | Instruction::CallSelectorForward { arc_owned: true, .. }
                | Instruction::CallClosureForward { arc_owned: true, .. }
                | Instruction::TypedCall { .. }
                | Instruction::CallClosure { .. }
                | Instruction::CallSelector { .. }
                | Instruction::Unbox { to: Type::I1, .. }
        )
        || *ty_p != Type::I8
        || *ty_c != Type::I1
        || !matches!(read, Instruction::CallRuntime { name, args, ret_ty }
            if name == "dartforge_exception_pending" && args.is_empty() && *ret_ty == Type::I8)
        || !matches!(comparison, Instruction::ICmp(ICmpOp::Ne, Operand::Val(q), Operand::Constant(Constant::Int(0))) if q == p)
    {
        return None;
    }
    match &b.terminator {
        Terminator::CondBranch {
            cond: Operand::Val(q),
            then_block,
            else_block,
        } if q == c && *then_block == error && *else_block != error => Some(*else_block),
        _ => None,
    }
}

/// O resultado de invoke não existe antes de atravessar sua aresta de sucesso.
/// Phi pode juntar sucesso e erro se a entrada excepcional não usar o resultado.
fn check_result_uses(
    f: &Function,
    cfg: &Cfg,
    pos: &HashMap<BlockId, usize>,
    call: ValueId,
    success: (usize, usize),
    without_success: &HashSet<usize>,
) -> Result<(), String> {
    let uses = |o: &Operand| matches!(o, Operand::Val(v) if *v == call);
    for (i, b) in f.blocks.iter().enumerate() {
        if !cfg.reachable(i) {
            continue;
        }
        let mut invalid = false;
        for (_, inst, _) in &b.instructions {
            if let Instruction::Phi { incoming, .. } = inst {
                for (de, op) in incoming {
                    let Some(&p) = pos.get(de) else { continue };
                    // A aresta de sucesso publica o resultado; todas as
                    // outras precisam ser dominadas por essa publicação.
                    if uses(op) && cfg.reachable(p) && without_success.contains(&p) && (p, i) != success
                    {
                        invalid = true;
                    }
                }
            } else if without_success.contains(&i) {
                super::super::operands::operands(inst, &mut |o| invalid |= uses(o));
            }
        }
        if without_success.contains(&i) {
            super::super::operands::terminator_operands(&b.terminator, &mut |o| {
                invalid |= uses(o)
            });
        }
        if invalid {
            return Err(format!(
                "ARC003 em {}: b{}: resultado v{} usado sem atravessar sua aresta de sucesso",
                f.symbol, b.id.0, call.0
            ));
        }
    }
    Ok(())
}

fn reachable_without_edge(cfg: &Cfg, removed: (usize, usize)) -> HashSet<usize> {
    let mut seen_set = HashSet::new();
    let mut queue = if cfg.successors.is_empty() {
        vec![]
    } else {
        vec![0]
    };
    while let Some(b) = queue.pop() {
        if seen_set.insert(b) {
            queue.extend(
                cfg.successors[b]
                    .iter()
                    .copied()
                    .filter(|&s| (b, s) != removed),
            );
        }
    }
    seen_set
}

#[cfg(test)]
mod availability_tests {
    use super::*;

    #[test]
    fn success_must_dominate_join_even_when_its_block_dominates() {
        let cfg = Cfg {
            successors: vec![vec![1, 2], vec![3], vec![1], vec![]],
            predecessors: vec![],
            rpo: vec![],
            idom: vec![],
        };
        let remaining = reachable_without_edge(&cfg, (0, 1));
        assert!(remaining.contains(&1) && remaining.contains(&3));
        let cfg = Cfg {
            successors: vec![vec![1, 2], vec![3], vec![], vec![]],
            predecessors: vec![],
            rpo: vec![],
            idom: vec![],
        };
        let remaining = reachable_without_edge(&cfg, (0, 1));
        assert!(!remaining.contains(&1) && !remaining.contains(&3));
    }
}
