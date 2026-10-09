//! Inventário tipado de ownership fornecido pelos produtores da HIR.
//! A cobertura e as dependências são conferidas antes da vivacidade;
//! contratos de externs, proveniência, escopo e consumo exigem outros passes.

use super::super::cfg::Cfg;
use super::{Vivacidade, vivacidade_com_emprestimos};
use crate::hir::*;
use std::collections::{HashMap, HashSet};

/// Quem sustenta um empréstimo: o chamador ou um valor desta função.
///
/// ```
/// use dartforge_emit_native::{hir::ValueId, otimizar::arc::OrigemOwner};
/// let origem = OrigemOwner::Valor(ValueId(3));
/// assert_ne!(origem, OrigemOwner::Chamador);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrigemOwner {
    /// Parâmetro emprestado pelo chamador durante a invocação.
    Chamador,
    /// Valor gerenciado ou slot proprietário local, com proveniência explícita.
    Valor(ValueId),
}

/// Classificação semântica de uma definição HIR (§20.1).
///
/// `Owned` descreve a obrigação de um token, não um incremento já emitido.
/// O número do escopo é uma identidade; seus limites ainda precisam ser
/// descritos e conferidos pelo verificador de empréstimos.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::{Ownership, OrigemOwner};
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
        owner: OrigemOwner,
        /// Identidade do escopo de empréstimo.
        escopo: u32,
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
/// inválidas mantêm o diagnóstico `ARC003` de [`vivacidade_com_emprestimos`].
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
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
pub fn vivacidade_classificada(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
) -> Result<Vivacidade, String> {
    let parametros: HashSet<ValueId> = f.params.iter().map(|(v, _, _)| *v).collect();
    let existentes: HashSet<ValueId> = parametros
        .iter()
        .copied()
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| b.instructions.iter().map(|(v, _, _)| *v)),
        )
        .collect();
    let mut ids: Vec<ValueId> = existentes.iter().copied().collect();
    ids.sort_by_key(|v| v.0);
    for id in ids {
        if !classes.contains_key(&id) {
            return Err(format!(
                "ownership em {}: v{} sem classificação",
                f.symbol, id.0
            ));
        }
    }
    let mut ids: Vec<ValueId> = classes.keys().copied().collect();
    ids.sort_by_key(|v| v.0);
    let mut referencias = HashSet::new();
    let mut emprestimos = HashMap::new();
    for id in ids {
        if !existentes.contains(&id) {
            return Err(format!(
                "ownership em {}: v{} não existe na função",
                f.symbol, id.0
            ));
        }
        match classes[&id] {
            Ownership::Trivial => {}
            Ownership::Owned => {
                referencias.insert(id);
            }
            Ownership::Borrowed { owner, .. } => {
                referencias.insert(id);
                match owner {
                    OrigemOwner::Chamador if !parametros.contains(&id) => {
                        return Err(format!(
                            "ownership em {}: v{} local não é parâmetro emprestado pelo chamador",
                            f.symbol, id.0
                        ));
                    }
                    OrigemOwner::Chamador => {}
                    OrigemOwner::Valor(owner) => {
                        emprestimos.insert(id, owner);
                    }
                }
            }
        }
    }
    conferir_dominancia(f, &emprestimos)?;
    vivacidade_com_emprestimos(f, &referencias, &emprestimos).map_err(|e| e.to_string())
}

/// `(bloco, posição, phi)`: parâmetros ficam fora dos blocos e dominam
/// suas instruções; phis são produzidos simultaneamente na entrada.
fn conferir_dominancia(
    f: &Function,
    emprestimos: &HashMap<ValueId, ValueId>,
) -> Result<(), String> {
    let cfg = Cfg::novo(f);
    let mut definicoes: HashMap<ValueId, (Option<usize>, usize, bool)> = f
        .params
        .iter()
        .map(|(v, _, _)| (*v, (None, 0, false)))
        .collect();
    for (bi, b) in f.blocks.iter().enumerate() {
        for (i, (v, inst, _)) in b.instructions.iter().enumerate() {
            definicoes.insert(*v, (Some(bi), i, matches!(inst, Instruction::Phi { .. })));
        }
    }
    let mut aliases: Vec<ValueId> = emprestimos.keys().copied().collect();
    aliases.sort_by_key(|v| v.0);
    for alias in aliases {
        let owner = emprestimos[&alias];
        // IDs ausentes recebem o diagnóstico da validação de dependências.
        let (Some(&(b_alias, i_alias, phi_alias)), Some(&(b_owner, i_owner, phi_owner))) =
            (definicoes.get(&alias), definicoes.get(&owner))
        else {
            continue;
        };
        let domina = match (b_owner, b_alias) {
            (None, _) => true,
            (Some(_), None) => false,
            (_, Some(b)) if !cfg.alcancavel(b) => true,
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
        if !domina {
            let bloco = b_alias.map_or_else(
                || "parâmetro".into(),
                |b| format!("b{} instrução {}", f.blocks[b].id.0, i_alias),
            );
            return Err(format!(
                "ARC003 em {}: {bloco}: v{alias_id} depende de owner v{owner_id} cuja definição não domina o alias; caminho [{alias_id}, {owner_id}]",
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
/// Não prova consumo de tokens nem resultados de chamadas sem `invoke`.
///
/// # Erros
/// Retorna erro se o plano não corresponder à forma preparada, ou se um alias
/// puder ser definido sem atravessar o sucesso de seu owner local.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "f".into(), name: "f".into(), depuracao: None,
///     params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
///     id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
/// vivacidade_classificada_com_excecoes(&f, &HashMap::new(), &TabelasDaFuncao::default())?;
/// # Ok::<(), String>(())
/// ```
pub fn vivacidade_classificada_com_excecoes(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    tabelas: &TabelasDaFuncao,
) -> Result<Vivacidade, String> {
    let v = vivacidade_classificada(f, classes)?;
    let cfg = Cfg::novo(f);
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
    let mut chamadas: Vec<_> = tabelas.invocacoes.iter().collect();
    chamadas.sort_by_key(|(v, _)| v.0);
    for (&call, &pouso) in chamadas {
        let Some(&origem) = defs.get(&call) else {
            return Err(format!(
                "ownership em {}: invoke v{} ausente",
                f.symbol, call.0
            ));
        };
        let b = &f.blocks[origem];
        let sucesso = match &b.terminator {
            Terminator::CondBranch {
                cond: Operand::Constant(Constant::Bool(false)),
                then_block,
                else_block,
            } if *then_block == pouso
                && *else_block != pouso
                && b.instructions.last().is_some_and(|(v, _, _)| *v == call)
                && tabelas.pousos.contains(&pouso) =>
            {
                pos.get(else_block).copied()
            }
            _ => None,
        }
        .ok_or_else(|| {
            format!(
                "ownership em {}: invoke v{} sem aresta de sucesso válida",
                f.symbol, call.0
            )
        })?;
        // Retirar a aresta distingue dominância de bloco e de resultado.
        let sem_sucesso = alcancaveis_sem_aresta(&cfg, (origem, sucesso));
        let mut aliases: Vec<_> = classes.iter().filter_map(|(&alias,classe)| {
            matches!(classe, Ownership::Borrowed { owner: OrigemOwner::Valor(owner), .. } if *owner == call)
                .then_some(alias)
        }).collect();
        aliases.sort_by_key(|v| v.0);
        for alias in aliases {
            if let Some(&bloco) = defs.get(&alias)
                && cfg.alcancavel(bloco)
                && sem_sucesso.contains(&bloco)
            {
                return Err(format!(
                    "ARC003 em {}: v{} depende de resultado v{} indisponível no caminho excepcional",
                    f.symbol, alias.0, call.0
                ));
            }
        }
    }
    Ok(v)
}

fn alcancaveis_sem_aresta(cfg: &Cfg, removida: (usize, usize)) -> HashSet<usize> {
    let mut vistos = HashSet::new();
    let mut fila = if cfg.sucessores.is_empty() {
        vec![]
    } else {
        vec![0]
    };
    while let Some(b) = fila.pop() {
        if vistos.insert(b) {
            fila.extend(
                cfg.sucessores[b]
                    .iter()
                    .copied()
                    .filter(|&s| (b, s) != removida),
            );
        }
    }
    vistos
}

#[cfg(test)]
mod disponibilidade_testes {
    use super::*;

    #[test]
    fn sucesso_precisa_dominar_a_juncao_mesmo_quando_seu_bloco_domina() {
        let cfg = Cfg {
            sucessores: vec![vec![1, 2], vec![3], vec![1], vec![]],
            predecessores: vec![],
            rpo: vec![],
            idom: vec![],
        };
        let restantes = alcancaveis_sem_aresta(&cfg, (0, 1));
        assert!(restantes.contains(&1) && restantes.contains(&3));
        let cfg = Cfg {
            sucessores: vec![vec![1, 2], vec![3], vec![], vec![]],
            predecessores: vec![],
            rpo: vec![],
            idom: vec![],
        };
        let restantes = alcancaveis_sem_aresta(&cfg, (0, 1));
        assert!(!restantes.contains(&1) && !restantes.contains(&3));
    }
}
