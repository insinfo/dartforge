//! Análises e inserção inicial de ownership ARC na HIR (§20 da especificação).
//!
//! A vivacidade recebe o inventário semântico de referências/slots; não
//! classifica externs nem prova consumo de tokens por si só. Produtores,
//! verificadores e inserção em retornos/saídas existem como passes explícitos;
//! ainda precisam de cobertura completa e integração no pipeline padrão.

use super::cfg::Cfg;
use super::operandos::{operandos, operandos_do_terminador};
use crate::hir::*;
use std::collections::{HashMap, HashSet};

mod classificacao;
mod caixas;
mod escopos;
mod tokens;
mod contratos;
mod chamadas;
mod funcoes;
mod parametros;
pub use parametros::produzir_parametros_escalares_do_lowering;
mod invocacoes;
mod saidas;
mod ssa;
mod quadros;
mod cleanup_estrangeiro;
pub use cleanup_estrangeiro::CleanupEstrangeiro;
pub use contratos::{ContratoChamadaRuntime, contrato_chamada_runtime, produzir_contratos_runtime, produzir_contratos_arc};
pub use contratos::produzir_e_verificar_tokens;
pub use contratos::produzir_parametros_ref_dart;
pub use contratos::produzir_e_verificar_tokens_dart;
pub use contratos::inserir_retencao_retornos_dart;
pub use contratos::inserir_arc_saidas_dart;
pub use chamadas::{ContratoFuncaoDart, verificar_contrato_funcao_dart, produzir_chamadas_dart};
pub use funcoes::{PlanoFuncaoDart, inserir_arc_funcoes_dart, preparar_arc_funcoes_dart};
pub use tokens::{EfeitoTokens, PlanoTokens, RetornoTokens, verificar_tokens, tokens_na_entrada_dos_pousos};
pub use escopos::{AlteracaoEscopo, PlanoEscopos, verificar_escopos};
pub use classificacao::{
    OrigemOwner, Ownership, vivacidade_classificada, vivacidade_classificada_com_excecoes,
};

/// Vivacidade dos valores classificados pelo chamador, só em blocos alcançáveis.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::Vivacidade;
/// let vazia = Vivacidade::default();
/// assert!(vazia.entrada.is_empty() && vazia.arestas.is_empty());
/// ```
#[derive(Debug, Default)]
pub struct Vivacidade {
    /// Antes das definições de `Phi` na entrada do bloco.
    pub entrada: HashMap<BlockId, HashSet<ValueId>>,
    /// Depois do terminador, união das necessidades das arestas de saída.
    pub saida: HashMap<BlockId, HashSet<ValueId>>,
    /// Antes de cada instrução; entradas de `Phi` pertencem às arestas.
    pub antes: HashMap<ValueId, HashSet<ValueId>>,
    /// Depois de cada instrução.
    pub depois: HashMap<ValueId, HashSet<ValueId>>,
    /// Necessidade específica de `(predecessor, sucessor)`, incluindo apenas
    /// as entradas de `Phi` desse predecessor.
    pub arestas: HashMap<(BlockId, BlockId), HashSet<ValueId>>,
}

struct Inventario<'a> {
    referencias: &'a HashSet<ValueId>,
    emprestimos: &'a HashMap<ValueId, ValueId>,
}

fn usos(o: &Operand, referencias: &Inventario<'_>, vivos: &mut HashSet<ValueId>) {
    if let Operand::Val(v) = o
        && referencias.referencias.contains(v)
    {
        let mut atual = *v;
        loop {
            vivos.insert(atual);
            let Some(&owner) = referencias.emprestimos.get(&atual) else {
                break;
            };
            atual = owner;
        }
    }
}

fn na_aresta(
    f: &Function,
    origem: usize,
    destino: usize,
    entrada: &[HashSet<ValueId>],
    referencias: &Inventario<'_>,
) -> HashSet<ValueId> {
    let mut vivos = entrada[destino].clone();
    for (v, inst, _) in &f.blocks[destino].instructions {
        if let Instruction::Phi { incoming, .. } = inst {
            vivos.remove(v);
            for (de, op) in incoming {
                if *de == f.blocks[origem].id {
                    usos(op, referencias, &mut vivos);
                }
            }
        }
    }
    vivos
}

fn na_saida(
    f: &Function,
    cfg: &Cfg,
    b: usize,
    entrada: &[HashSet<ValueId>],
    referencias: &Inventario<'_>,
) -> HashSet<ValueId> {
    let mut vivos = HashSet::new();
    for &s in &cfg.sucessores[b] {
        vivos.extend(na_aresta(f, b, s, entrada, referencias));
    }
    vivos
}

fn recuar(
    v: ValueId,
    inst: &Instruction,
    referencias: &Inventario<'_>,
    vivos: &mut HashSet<ValueId>,
) {
    vivos.remove(&v);
    if !matches!(inst, Instruction::Phi { .. }) {
        operandos(inst, &mut |o| usos(o, referencias, vivos));
    }
}

/// Calcula vivacidade reversa em ponto fixo, com `Phi` por predecessor.
///
/// A função deve ter CFG/SSA válidos. `referencias` vem da classificação
/// semântica: não deduzimos ownership de `Type::I64` nem de `Type::Ptr`.
/// Incluir aliases com proveniência gerenciada e slots proprietários, quando
/// classificados. Dependências de borrows e extensões de vida precisam ser
/// materializadas como usos antes desta análise. No modo tabelas, executar
/// depois da preparação excepcional. Conjuntos não têm ordem de emissão;
/// ordenar IDs antes de materializar operações para manter determinismo.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::vivacidade};
/// use std::collections::HashSet;
/// let f = Function {
///     symbol: "identidade".into(), name: "identidade".into(),
///     params: vec![(ValueId(0), "x".into(), Type::Ref)],
///     return_ty: Type::Ref, depuracao: None,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }],
/// };
/// let vivos = vivacidade(&f, &HashSet::from([ValueId(0)]));
/// assert!(vivos.entrada[&BlockId(0)].contains(&ValueId(0)));
/// assert!(vivos.saida[&BlockId(0)].is_empty());
/// ```
pub fn vivacidade(f: &Function, referencias: &HashSet<ValueId>) -> Vivacidade {
    analisar(
        f,
        &Inventario {
            referencias,
            emprestimos: &HashMap::new(),
        },
    )
}

/// Dependência de empréstimo inválida, com origem e caminho (`ARC003`).
///
/// ```
/// use dartforge_emit_native::{hir::ValueId, otimizar::arc::ErroEmprestimo};
/// let erro = ErroEmprestimo { funcao: "f".into(), origem: ValueId(2),
///     caminho: vec![ValueId(2), ValueId(2)], motivo: "dependência cíclica" };
/// assert!(erro.to_string().starts_with("ARC003"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroEmprestimo {
    /// Símbolo da função analisada.
    pub funcao: String,
    /// Alias que iniciou a verificação desta cadeia.
    pub origem: ValueId,
    /// Cadeia de aliases/owners; um ciclo termina repetindo seu primeiro ID.
    pub caminho: Vec<ValueId>,
    /// Razão da rejeição do inventário.
    pub motivo: &'static str,
}

impl std::fmt::Display for ErroEmprestimo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ARC003 em {}: v{}: {}; caminho {:?}",
            self.funcao, self.origem.0, self.motivo, self.caminho
        )
    }
}

impl std::error::Error for ErroEmprestimo {}

/// Inclui a cadeia de sustentação de cada alias borrowed nos seus usos.
///
/// `emprestimos` associa alias a owner imediato; cadeias incluem também os
/// aliases intermediários. Vale o contrato de CFG/SSA de [`vivacidade`].
/// Escopos, dominância e escape ainda precisam do verificador de ownership.
/// A análise de `Phi` trata cada entrada na aresta do seu predecessor;
/// transferências de tokens devem usar essas arestas, não pontos entre `Phi`.
///
/// # Erros
/// Retorna `ARC003` se alias/owner não existe na função ou no inventário,
/// ou se a cadeia de sustentação tem ciclo. Verifica em ordem de ID para
/// produzir o mesmo primeiro diagnóstico independentemente da ordem do mapa.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::vivacidade_com_emprestimos};
/// use std::collections::{HashMap, HashSet};
/// let f = Function { symbol: "f".into(), name: "f".into(), depuracao: None,
///     params: vec![(ValueId(0), "owner".into(), Type::Ref),
///                  (ValueId(1), "alias".into(), Type::Ref)], return_ty: Type::Ref,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }] };
/// let refs = HashSet::from([ValueId(0), ValueId(1)]);
/// let v = vivacidade_com_emprestimos(&f, &refs, &HashMap::from([(ValueId(1), ValueId(0))]))?;
/// assert_eq!(v.entrada[&BlockId(0)], refs);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn vivacidade_com_emprestimos(
    f: &Function,
    referencias: &HashSet<ValueId>,
    emprestimos: &HashMap<ValueId, ValueId>,
) -> Result<Vivacidade, ErroEmprestimo> {
    let existentes: HashSet<ValueId> = f
        .params
        .iter()
        .map(|(v, _, _)| *v)
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| b.instructions.iter().map(|(v, _, _)| *v)),
        )
        .collect();
    let mut origens: Vec<ValueId> = emprestimos.keys().copied().collect();
    origens.sort_by_key(|v| v.0);
    let erro = |origem, caminho, motivo| ErroEmprestimo {
        funcao: f.symbol.clone(),
        origem,
        caminho,
        motivo,
    };
    for &alias in &origens {
        let owner = emprestimos[&alias];
        if [alias, owner]
            .iter()
            .any(|v| !referencias.contains(v) || !existentes.contains(v))
        {
            return Err(erro(
                alias,
                vec![alias, owner],
                "alias/owner ausente da função ou do inventário",
            ));
        }
    }
    let mut verificados = HashSet::new();
    for origem in origens {
        let mut caminho = Vec::new();
        let mut posicoes = HashMap::new();
        let mut atual = origem;
        while !verificados.contains(&atual) {
            if posicoes.insert(atual, caminho.len()).is_some() {
                caminho.push(atual);
                return Err(erro(origem, caminho, "dependência cíclica"));
            }
            caminho.push(atual);
            let Some(&owner) = emprestimos.get(&atual) else {
                break;
            };
            atual = owner;
        }
        verificados.extend(caminho);
    }
    Ok(analisar(
        f,
        &Inventario {
            referencias,
            emprestimos,
        },
    ))
}

fn analisar(f: &Function, referencias: &Inventario<'_>) -> Vivacidade {
    let cfg = Cfg::novo(f);
    let mut entrada = vec![HashSet::new(); f.blocks.len()];
    let mut fila: Vec<usize> = cfg.rpo.clone();
    let mut na_fila = vec![false; f.blocks.len()];
    for &b in &fila {
        na_fila[b] = true;
    }
    while let Some(b) = fila.pop() {
        na_fila[b] = false;
        let mut vivos = na_saida(f, &cfg, b, &entrada, referencias);
        operandos_do_terminador(&f.blocks[b].terminator, &mut |o| {
            usos(o, referencias, &mut vivos)
        });
        for (v, inst, _) in f.blocks[b].instructions.iter().rev() {
            recuar(*v, inst, referencias, &mut vivos);
        }
        if vivos != entrada[b] {
            entrada[b] = vivos;
            for &p in &cfg.predecessores[b] {
                if cfg.alcancavel(p) && !na_fila[p] {
                    fila.push(p);
                    na_fila[p] = true;
                }
            }
        }
    }
    let mut resultado = Vivacidade::default();
    for &b in &cfg.rpo {
        let bloco = &f.blocks[b];
        let mut vivos = na_saida(f, &cfg, b, &entrada, referencias);
        resultado.entrada.insert(bloco.id, entrada[b].clone());
        resultado.saida.insert(bloco.id, vivos.clone());
        for &s in &cfg.sucessores[b] {
            resultado.arestas.insert(
                (bloco.id, f.blocks[s].id),
                na_aresta(f, b, s, &entrada, referencias),
            );
        }
        operandos_do_terminador(&bloco.terminator, &mut |o| usos(o, referencias, &mut vivos));
        for (v, inst, _) in bloco.instructions.iter().rev() {
            resultado.depois.insert(*v, vivos.clone());
            recuar(*v, inst, referencias, &mut vivos);
            resultado.antes.insert(*v, vivos.clone());
        }
    }
    resultado
}

#[cfg(test)]
mod testes;

mod retomas;
pub(crate) use retomas::{conferir as conferir_retomas, instrucao_de_retoma};
