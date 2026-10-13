//! Disponibilidade linear de tokens no CFG preparado, com Phi por aresta.
//! O inventário e os efeitos são contratos semânticos fornecidos pelo produtor.
//! Barreiras de borrows derivados são conservadoras; externs, aliases de
//! slots, regiões, keepalive e versões/pins continuam exigindo produtores.

use super::super::{
    cfg::Cfg,
    operands::{operands, terminator_operands},
};
use super::{OwnerOrigin, Ownership};
use crate::hir::*;
use std::collections::{HashMap, HashSet, VecDeque};

/// Consumo explícito de argumentos de uma instrução ordinária.
///
/// As listas preservam multiplicidade: consumir o mesmo token duas vezes
/// exige duas cópias distintas. Não há contrato implícito para chamada ausente.
///
/// ```
/// use dartforge_emit_native::optimize::arc::EfeitoTokens;
/// let efeito = EfeitoTokens::default();
/// assert!(!efeito.pode_falhar && efeito.erro.is_empty());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TokenEffect {
    /// Consumo tanto no sucesso quanto na falha, antes do resultado.
    pub always: Vec<ValueId>,
    /// Consumo apenas no sucesso.
    pub success: Vec<ValueId>,
    /// Consumo apenas na falha de invoke ou conferência de pendência runtime.
    pub error: Vec<ValueId>,
    /// Exige saída excepcional em invoke ou no mapa de pendências do plano.
    pub may_fail: bool,
}

/// Contrato de transferência do resultado ao chamador.
///
/// ```
/// use dartforge_emit_native::optimize::arc::RetornoTokens;
/// assert_ne!(RetornoTokens::Owned, RetornoTokens::Borrowed);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TokenReturn {
    /// Resultado sem token gerenciado (inclusive funções void).
    #[default]
    Trivial,
    /// Transfere um token owned; null não exige contagem física.
    Owned,
    /// Empréstimo sustentado pelo chamador, nunca por owner local.
    Borrowed,
}

/// Contratos na mesma versão do CFG e do inventário de ownership.
///
/// Fornecer uma entrada por instrução ordinária, também sem consumo; as
/// operações ARC e Phi têm regras próprias e não aceitam sobrescrita.
/// Um plano ausente não significa que argumentos sejam emprestados.
///
/// ```
/// use dartforge_emit_native::optimize::arc::PlanoTokens;
/// assert!(PlanoTokens::default().instrucoes.is_empty());
/// assert!(PlanoTokens::default().pendencias.is_empty());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TokenPlan {
    /// Efeitos exatos antes do resultado e nas saídas de invoke/pendência.
    pub instructions: HashMap<ValueId, TokenEffect>,
    /// Convenção do retorno, não deduzida da largura da representação.
    pub ret: TokenReturn,
    /// Chamada runtime pending e bloco de erro após conferência explícita.
    /// Não são pousos LLVM. Exige sufixo exception_pending/ICmp Ne zero e
    /// CondBranch para erro/sucesso, sem outra operação entre chamada e teste.
    pub pendings: HashMap<ValueId, BlockId>,
    /// Papéis nativos de pares quadro/índice de uma família fechada verificada.
    /// Não é um owner Ref nem substitui o certificado de unwind estrangeiro.
    pub shared_frames: Option<super::SharedFramesCfgContract>,
    /// Parâmetros `Ptr` de função local direta que recebem o endereço de um
    /// local escalar de quem a declara (captura por endereço), com o tipo
    /// dele; inicializado na entrada (`locals::produce_addresses`).
    pub received_addresses: HashMap<ValueId, Type>,
    /// Chamada a função local direta → posição e tipo dos endereços de locais
    /// escalares que ela cede ao callee.
    pub yielded_addresses: HashMap<ValueId, Vec<(usize, Type)>>,
}

fn own(inst: &Instruction) -> bool {
    matches!(
        inst,
        Instruction::ArcCopy { .. }
            | Instruction::ArcMove { .. }
            | Instruction::ArcDrop { .. }
            | Instruction::ArcLoadStrong { .. }
            | Instruction::ArcStoreStrong { .. }
            | Instruction::Phi { .. }
    )
}

fn check(
    v: ValueId,
    classes: &HashMap<ValueId, Ownership>,
    active_set: &HashSet<ValueId>,
) -> Result<(), String> {
    let mut current = v;
    loop {
        match classes[&current] {
            Ownership::Trivial => return Ok(()),
            Ownership::Owned if active_set.contains(&current) => return Ok(()),
            Ownership::Owned => {
                return Err(format!(
                    "v{} depende do token indisponível v{}",
                    v.0, current.0
                ));
            }
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                ..
            } => return Ok(()),
            Ownership::Borrowed {
                owner: OwnerOrigin::Value(owner),
                ..
            } => current = owner,
        }
    }
}

fn use_value(
    op: &Operand,
    classes: &HashMap<ValueId, Ownership>,
    active_set: &HashSet<ValueId>,
) -> Result<(), String> {
    if let Operand::Val(v) = op {
        check(*v, classes, active_set)?;
    }
    Ok(())
}

fn consume(
    v: ValueId,
    classes: &HashMap<ValueId, Ownership>,
    active_set: &mut HashSet<ValueId>,
) -> Result<(), String> {
    if classes[&v] != Ownership::Owned {
        return Err(format!("consumo de v{} exige Owned", v.0));
    }
    if !active_set.remove(&v) {
        return Err(format!("token v{} já consumido ou indisponível", v.0));
    }
    Ok(())
}

fn consume_operand(
    op: &Operand,
    classes: &HashMap<ValueId, Ownership>,
    active_set: &mut HashSet<ValueId>,
) -> Result<(), String> {
    match op {
        Operand::Val(v) => consume(*v, classes, active_set),
        Operand::Constant(Constant::Null) => Ok(()),
        _ => Err("consumo exige token SSA ou null".into()),
    }
}

fn apply(
    ids: &[ValueId],
    classes: &HashMap<ValueId, Ownership>,
    active_set: &mut HashSet<ValueId>,
) -> Result<(), String> {
    for &v in ids {
        consume(v, classes, active_set)?;
    }
    Ok(())
}

fn produce(
    v: ValueId,
    classes: &HashMap<ValueId, Ownership>,
    active_set: &mut HashSet<ValueId>,
) -> Result<(), String> {
    if classes[&v] == Ownership::Owned && !active_set.insert(v) {
        return Err(format!(
            "nova produção de v{} com token anterior ainda disponível",
            v.0
        ));
    }
    check(v, classes, active_set)
}

fn of_caller(mut v: ValueId, classes: &HashMap<ValueId, Ownership>) -> bool {
    loop {
        match classes[&v] {
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                ..
            } => return true,
            Ownership::Borrowed {
                owner: OwnerOrigin::Value(owner),
                ..
            } => v = owner,
            _ => return false,
        }
    }
}

/// O inventário já excluiu ciclos; aliases emprestados conservam a raiz.
fn owner_root(mut v: ValueId, classes: &HashMap<ValueId, Ownership>) -> Option<OwnerOrigin> {
    loop {
        match classes[&v] {
            Ownership::Trivial => return None,
            Ownership::Owned => return Some(OwnerOrigin::Value(v)),
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                ..
            } => return Some(OwnerOrigin::Caller),
            Ownership::Borrowed {
                owner: OwnerOrigin::Value(next_item),
                ..
            } => v = next_item,
        }
    }
}

/// Limites léxicos herdados também precisam sobreviver à criação de um alias.
fn owner_scopes(mut v: ValueId, classes: &HashMap<ValueId, Ownership>) -> HashSet<u32> {
    let mut scopes = HashSet::new();
    while let Ownership::Borrowed { owner, scope } = classes[&v] {
        if scope != 0 {
            scopes.insert(scope);
        }
        match owner {
            OwnerOrigin::Caller => break,
            OwnerOrigin::Value(next_item) => v = next_item,
        }
    }
    scopes
}

fn path(f: &Function, pais: &[Option<usize>], mut b: usize) -> Vec<u32> {
    let mut r = vec![f.blocks[b].id.0];
    while let Some(parent) = pais[b] {
        b = parent;
        r.push(f.blocks[b].id.0);
    }
    r.reverse();
    r
}

fn flow_error(f: &Function, pais: &[Option<usize>], b: usize, message: String) -> String {
    format!(
        "tokens em {} b{}: {message}; caminho {:?}",
        f.symbol,
        f.blocks[b].id.0,
        path(f, pais, b)
    )
}

/// Confere disponibilidade, transferência e consumo único no CFG preparado.
///
/// Requer CFG/SSA e representações válidos, inventário semântico completo e
/// contratos previamente certificados pelo produtor. Invoke só produz token
/// no sucesso. Phi owned consome entradas simultaneamente na aresta, inclusive
/// trocas em backedges; null produz uma obrigação lógica sem RC físico.
/// Junções exigem inventários iguais, sem união que esconda consumo condicional.
/// Saídas devem consumir todos os tokens locais, transferindo o retorno owned.
/// Borrowed só pode voltar quando sua cadeia termina no chamador.
/// Reconstrói contratos de locais escalares privados no corpo atual, incluindo
/// ausência de escape, tipagem, inicialização e valores gravados Trivial.
/// Confere também as operações puras cobertas, seu tipo, efeito vazio e
/// origem Trivial dos operandos escalares, pelas mesmas regras do produtor.
///
/// Não insere operações, certifica os demais contratos, verifica escopos léxicos,
/// aliases/invalidação de slots, regiões ou limpeza de estados suspensos.
/// Rejeita aliases derivados após barreiras e como argumentos de reentrada;
/// não insere keepalive e depende da classificação/proveniência fornecida.
/// Essas obrigações continuam necessárias para aceitar o pipeline ARC inteiro.
///
/// # Erros
/// Retorna símbolo, ponto e caminho para uso/consumo indisponível, fuga de token,
/// junção incompatível, retorno inválido ou metadado ausente/obsoleto. Também
/// propaga diagnósticos de cobertura, dependências e forma excepcional.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "f".into(), name: "f".into(), depuracao: None,
///     params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
///         (ValueId(1), Instruction::ArcCopy { value: Operand::Val(ValueId(0)) }, Type::Ref),
///         (ValueId(2), Instruction::ArcDrop { value: Operand::Val(ValueId(1)) }, Type::Void)],
///         terminator: Terminator::Return(None) }] };
/// let classes = HashMap::from([(ValueId(0), Ownership::Borrowed {
///     owner: OrigemOwner::Chamador, escopo: 0 }), (ValueId(1), Ownership::Owned),
///     (ValueId(2), Ownership::Trivial)]);
/// verificar_tokens(&f, &classes, &TabelasDaFuncao::default(), &PlanoTokens::default())?;
/// # Ok::<(), String>(())
/// ```
pub fn verify_tokens(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    tables: &FunctionTables,
    plan: &TokenPlan,
) -> Result<(), String> {
    if plan.shared_frames != tables.shared_frames {
        return Err("tokens ARC: contratos de quadros compartilhados divergem das tabelas".into());
    }
    analyze_tokens(f, classes, tables, plan, false, false).map(|_| ())
}

/// Confere o mesmo fluxo, admitindo somente tokens restantes em Return.
/// Não relaxa consumo duplicado, disponibilidade, convenção ou junções.
pub(super) fn cleanup_exits(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    tables: &FunctionTables,
    plan: &TokenPlan,
) -> Result<HashMap<BlockId, Vec<ValueId>>, String> {
    analyze_tokens(f, classes, tables, plan, true, false).map(|flow| flow.exits)
}

// O modo de planejamento admite somente descarte de tokens mortos por aresta.
// O verificador público continua exigindo inventários idênticos sem descarte.
pub(super) fn cleanup_edges(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    tables: &FunctionTables,
    plan: &TokenPlan,
) -> Result<HashMap<(BlockId, BlockId), Vec<ValueId>>, String> {
    analyze_tokens(f, classes, tables, plan, true, true).map(|flow| flow.edges)
}

/// Entrega o inventário linear validado na entrada de cada pouso alcançável.
///
/// Aplica os consumos da aresta de erro e exclui resultados exclusivos do
/// sucesso. Inclui tokens Phi transferidos pela aresta; ordena por ValueId.
/// Não gera cleanup estrangeiro nem substitui a conferência dos escopos.
/// Deve ser recalculado após qualquer mudança do CFG ou dos contratos.
///
/// # Erros
/// Falha de verificação de tokens ou pouso sem entrada alcançável.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "vazia".into(), name: "vazia".into(), depuracao: None,
///     params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
///     id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
/// assert!(tokens_na_entrada_dos_pousos(&f, &HashMap::new(),
///     &TabelasDaFuncao::default(), &PlanoTokens::default())?.is_empty());
/// # Ok::<(), String>(())
/// ```
pub fn tokens_at_landing_pad_entry(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    tables: &FunctionTables,
    plan: &TokenPlan,
) -> Result<HashMap<BlockId, Vec<ValueId>>, String> {
    let entries = analyze_tokens(f, classes, tables, plan, false, false)?.entries;
    let mut landing_pads: Vec<_> = tables.landing_pads.iter().copied().collect();
    landing_pads.sort_by_key(|b| b.0);
    let mut result = HashMap::new();
    for b in landing_pads {
        let owners = entries.get(&b).ok_or_else(|| {
            format!(
                "tokens em {}: pouso b{} sem entrada alcançável",
                f.symbol, b.0
            )
        })?;
        result.insert(b, owners.clone());
    }
    Ok(result)
}

pub(super) fn check_exceptional_exits(
    f: &Function,
    tables: &FunctionTables,
    require_canonical: bool,
) -> Result<(), String> {
    if tables.exits.is_empty() {
        return Ok(());
    }
    let blocks: HashMap<_, _> = f.blocks.iter().map(|b| (b.id, b)).collect();
    let dests: HashSet<_> = tables.invocations.values().copied().collect();
    let mut exits: Vec<_> = tables.exits.iter().collect();
    exits.sort_by_key(|(b, _)| b.0);
    for (id, mode) in exits {
        let b = blocks
            .get(id)
            .ok_or_else(|| format!("saída excepcional obsoleta b{}", id.0))?;
        let Terminator::Return(op) = &b.terminator else {
            return Err(format!("saída excepcional b{} não é Return", id.0));
        };
        if *mode == ExceptionExit::Guard {
            return Err(format!(
                "saída Guarda b{} exige CFG explícito antes do ARC",
                id.0
            ));
        }
        if *mode == ExceptionExit::Resume {
            if !tables.landing_pads.contains(id)
                || !dests.contains(id)
                || f.blocks.first().is_some_and(|entry| entry.id == *id)
            {
                return Err(format!("saída Retoma b{} exige pouso de invoke", id.0));
            }
            if b.instructions
                .iter()
                .any(|(_, i, _)| !super::resume_instruction(i))
            {
                return Err(format!(
                    "saída Retoma b{} só admite drops ARC e fechamento de quadros locais",
                    id.0
                ));
            }
        }
        let expected = if f.return_ty == Type::Void {
            None
        } else {
            Some(super::super::operands::default_constant(f.return_ty))
        };
        if require_canonical && *op != expected {
            return Err(format!(
                "saída {mode:?} b{} exige retorno canônico sem transferência",
                id.0
            ));
        }
    }
    Ok(())
}

// Só normaliza corpos temporários: o chamador publica após verificar tudo.
pub(super) fn normalize_exits_throws(
    f: &mut Function,
    tables: &FunctionTables,
) -> Result<(), String> {
    super::ssa::verify(f)?;
    check_exceptional_exits(f, tables, false)?;
    for b in &mut f.blocks {
        if matches!(
            tables.exits.get(&b.id),
            Some(ExceptionExit::Throws | ExceptionExit::Resume)
        ) {
            b.terminator = Terminator::Return(if f.return_ty == Type::Void {
                None
            } else {
                Some(super::super::operands::default_constant(f.return_ty))
            });
        }
    }
    Ok(())
}

struct TokenFlow {
    exits: HashMap<BlockId, Vec<ValueId>>,
    entries: HashMap<BlockId, Vec<ValueId>>,
    edges: HashMap<(BlockId, BlockId), Vec<ValueId>>,
}

fn analyze_tokens(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    tables: &FunctionTables,
    plan: &TokenPlan,
    allow_cleanup: bool,
    clear_edges: bool,
) -> Result<TokenFlow, String> {
    super::locals::verify(f, classes, plan)?;
    super::pure_set::verify(f, classes, plan)?;
    check_exceptional_exits(f, tables, true)?;
    super::check_resumes(f, tables)?;
    let live_set = super::classification::liveness_with_exits(f, classes, tables, &plan.pendings)?;
    let mut edges = HashMap::new();
    let meta_error = |m: String| format!("tokens em {}: {m}", f.symbol);
    let mut expected_set = HashSet::new();
    for b in &f.blocks {
        for (v, inst, _) in &b.instructions {
            if own(inst) {
                if let Instruction::ArcLoadStrong {
                    slot:
                        StrongSlot::Quadro {
                            frame: Operand::Val(q),
                            ..
                        },
                }
                | Instruction::ArcStoreStrong {
                    slot:
                        StrongSlot::Quadro {
                            frame: Operand::Val(q),
                            ..
                        },
                    ..
                } = inst
                {
                    if classes[q] != Ownership::Trivial {
                        return Err(meta_error(format!(
                            "v{}: ID de quadro deve ser Trivial",
                            v.0
                        )));
                    }
                }
                let class = if matches!(
                    inst,
                    Instruction::ArcDrop { .. } | Instruction::ArcStoreStrong { .. }
                ) {
                    Some(Ownership::Trivial)
                } else if matches!(
                    inst,
                    Instruction::ArcCopy { .. }
                        | Instruction::ArcMove { .. }
                        | Instruction::ArcLoadStrong { .. }
                ) {
                    Some(Ownership::Owned)
                } else {
                    None
                };
                if class.is_some_and(|c| classes[v] != c) || tables.invocations.contains_key(v) {
                    return Err(meta_error(format!(
                        "v{}: contrato incompatível com operação ARC/Phi",
                        v.0
                    )));
                }
                continue;
            }
            expected_set.insert(*v);
            let e = plan
                .instructions
                .get(v)
                .ok_or_else(|| meta_error(format!("v{} sem contrato de consumo", v.0)))?;
            let unbox_bool = super::contracts::unbox_bool_call(inst);
            let call = unbox_bool.as_ref().unwrap_or(inst);
            if let Instruction::CallRuntime { name, .. } = call {
                if super::contract(name).is_ok() {
                    let c = super::runtime_call_contract(call).map_err(meta_error)?;
                    if classes[v] != c.result
                        || e.always != c.effect.always
                        || e.success != c.effect.success
                        || e.error != c.effect.error
                        || e.may_fail != c.effect.may_fail
                    {
                        return Err(meta_error(format!(
                            "v{}: plano incompatível com ownership.tsv para {name}",
                            v.0
                        )));
                    }
                }
            }
            // Um invoke de callee que não falha (a análise conjunta sabe mais
            // que a do módulo que pôs o pouso) só deixa um pouso morto.
            let invoke = tables.invocations.contains_key(v);
            let pending_state = plan.pendings.contains_key(v);
            if (e.may_fail && !invoke && !pending_state)
                || (!e.may_fail && pending_state)
                || (!e.may_fail && !e.error.is_empty())
            {
                let name_str = match inst {
                    Instruction::CallRuntime { name, .. } | Instruction::CallStatic { symbol: name, .. } => name.clone(),
                    another => format!("{another:?}").chars().take(80).collect(),
                };
                return Err(meta_error(format!(
                    "v{}: contrato não corresponde às saídas excepcionais {name_str} (pode falhar: {})",
                    v.0, e.may_fail
                )));
            }
            let mut args = HashSet::new();
            operands(inst, &mut |o| {
                if let Operand::Val(v) = o {
                    args.insert(*v);
                }
            });
            for &c in e.always.iter().chain(&e.success).chain(&e.error) {
                if !args.contains(&c) {
                    return Err(meta_error(format!(
                        "v{}: consumo de v{} não é argumento",
                        v.0, c.0
                    )));
                }
            }
        }
    }
    let mut extras: Vec<_> = plan
        .instructions
        .keys()
        .filter(|v| !expected_set.contains(v))
        .copied()
        .collect();
    extras.sort_by_key(|v| v.0);
    if let Some(v) = extras.first() {
        return Err(meta_error(format!(
            "v{}: contrato obsoleto ou sobrescreve regra ARC/Phi",
            v.0
        )));
    }
    if f.blocks.is_empty() {
        return Err(meta_error("função sem entrada".into()));
    }
    let cfg = Cfg::new(f);
    let mut exits = HashMap::new();
    let mut entries: Vec<Option<HashSet<ValueId>>> = vec![None; f.blocks.len()];
    entries[0] = Some(
        f.params
            .iter()
            .filter(|(v, _, _)| classes[v] == Ownership::Owned)
            .map(|(v, _, _)| *v)
            .collect(),
    );
    let mut pais = vec![None; f.blocks.len()];
    let mut queue = VecDeque::from([0]);
    while let Some(i) = queue.pop_front() {
        let b = &f.blocks[i];
        let mut active_set = entries[i].clone().unwrap();
        let mut invoke = None;
        for (v, inst, _) in &b.instructions {
            let result = (|| -> Result<(), String> {
                if matches!(inst, Instruction::Phi { .. }) {
                    return check(*v, classes, &active_set);
                }
                let mut failure = None;
                operands(inst, &mut |o| {
                    if failure.is_none() {
                        failure = use_value(o, classes, &active_set).err();
                    }
                });
                if let Some(m) = failure {
                    return Err(m);
                }
                match inst {
                    Instruction::ArcCopy { .. } | Instruction::ArcLoadStrong { .. } => {
                        produce(*v, classes, &mut active_set)
                    }
                    Instruction::ArcStoreStrong { value, mode, .. } => {
                        if *mode == StrongStoreMode::Move {
                            consume_operand(value, classes, &mut active_set)
                        } else {
                            Ok(())
                        }
                    }
                    Instruction::ArcMove { value } => {
                        consume_operand(value, classes, &mut active_set)?;
                        produce(*v, classes, &mut active_set)
                    }
                    Instruction::ArcDrop { value } => {
                        consume_operand(value, classes, &mut active_set)
                    }
                    _ => {
                        let e = &plan.instructions[v];
                        apply(&e.always, classes, &mut active_set)?;
                        // Um invoke (mesmo de callee que não falha) só
                        // produz o resultado na aresta de sucesso.
                        if e.may_fail || tables.invocations.contains_key(v) {
                            invoke = Some(*v);
                            Ok(())
                        } else {
                            apply(&e.success, classes, &mut active_set)?;
                            produce(*v, classes, &mut active_set)
                        }
                    }
                }
            })();
            result.map_err(|m| flow_error(f, &pais, i, format!("v{}: {m}", v.0)))?;
        }
        let mut failure = None;
        terminator_operands(&b.terminator, &mut |o| {
            if failure.is_none() {
                failure = use_value(o, classes, &active_set).err();
            }
        });
        if let Some(m) = failure {
            return Err(flow_error(f, &pais, i, m));
        }
        if let Terminator::Return(val_ref) = &b.terminator
            && !matches!(
                tables.exits.get(&b.id),
                Some(ExceptionExit::Throws | ExceptionExit::Resume)
            )
        {
            match (plan.ret, val_ref) {
                (TokenReturn::Owned, Some(v)) => consume_operand(v, classes, &mut active_set)
                    .map_err(|m| flow_error(f, &pais, i, m))?,
                (TokenReturn::Borrowed, Some(Operand::Val(v))) if of_caller(*v, classes) => {}
                (TokenReturn::Borrowed, Some(Operand::Constant(Constant::Null))) => {}
                (TokenReturn::Trivial, None) => {}
                (TokenReturn::Trivial, Some(Operand::Val(v)))
                    if classes[v] == Ownership::Trivial => {}
                (TokenReturn::Trivial, Some(Operand::Constant(c)))
                    if !matches!(c, Constant::String(_) | Constant::StringWtf8(_)) => {}
                _ => {
                    let class = match val_ref {
                        Some(Operand::Val(v)) => format!("v{} {:?}", v.0, classes.get(v)),
                        other => format!("{other:?}"),
                    };
                    return Err(flow_error(
                        f,
                        &pais,
                        i,
                        format!("retorno não satisfaz a convenção de tokens {:?} ({class})", plan.ret),
                    ));
                }
            }
        }
        // `Unreachable` não continua (o caminho morto depois de um lançamento,
        // o fim de quem encerra o processo): um token vivo ali não vaza.
        if cfg.successors[i].is_empty() && !active_set.is_empty() && !matches!(b.terminator, Terminator::Unreachable) {
            if allow_cleanup && matches!(b.terminator, Terminator::Return(_)) {
                let mut remaining: Vec<_> = active_set.iter().copied().collect();
                remaining.sort_unstable_by_key(|v| v.0);
                exits.insert(b.id, remaining);
                continue;
            }
            let mut ids: Vec<_> = active_set.iter().map(|v| v.0).collect();
            ids.sort_unstable();
            return Err(flow_error(
                f,
                &pais,
                i,
                format!("tokens não consumidos na saída {ids:?}"),
            ));
        }
        for &s in &cfg.successors[i] {
            let dest = &f.blocks[s];
            let next_one = (|| -> Result<HashSet<ValueId>, String> {
                let mut r = active_set.clone();
                if let Some(v) = invoke {
                    let e = &plan.instructions[&v];
                    if tables
                        .invocations
                        .get(&v)
                        .or_else(|| plan.pendings.get(&v))
                        == Some(&dest.id)
                    {
                        apply(&e.error, classes, &mut r)?;
                    } else {
                        apply(&e.success, classes, &mut r)?;
                        produce(v, classes, &mut r)?;
                    }
                }
                let mut consumed = Vec::new();
                let mut produced = Vec::new();
                for (v, inst, _) in &dest.instructions {
                    if let Instruction::Phi { incoming, .. } = inst {
                        for (de, op) in incoming {
                            if *de != b.id {
                                continue;
                            }
                            use_value(op, classes, &r)?;
                            if matches!(classes[v], Ownership::Borrowed { .. }) {
                                let compatible = match op {
                                    Operand::Constant(Constant::Null) => true,
                                    Operand::Val(entry) => {
                                        classes[entry] == Ownership::Trivial
                                            || (owner_root(*entry, classes)
                                                == owner_root(*v, classes)
                                                && owner_scopes(*entry, classes)
                                                    .is_subset(&owner_scopes(*v, classes)))
                                    }
                                    _ => false,
                                };
                                if !compatible {
                                    return Err(format!(
                                        "Phi borrowed v{}: owner ou escopo incompatível na entrada",
                                        v.0
                                    ));
                                }
                            }
                            if classes[v] == Ownership::Owned {
                                match op {
                                    Operand::Val(entry)
                                        if classes[entry] == Ownership::Owned =>
                                    {
                                        consumed.push(*entry)
                                    }
                                    Operand::Constant(Constant::Null) => {}
                                    _ => {
                                        return Err(format!(
                                            "Phi v{} exige entrada owned ou null",
                                            v.0
                                        ));
                                    }
                                }
                                produced.push(*v);
                            }
                        }
                    }
                }
                apply(&consumed, classes, &mut r)?;
                // Resultados Phi nascem no destino. Tokens transferidos já
                // saíram de r; nenhum dos dois recebe drop no predecessor.
                // Os mortos saem antes das produções: o valor antigo de um
                // Phi de laço que não volta para ele (foi copiado na aresta)
                // é morto aqui, e o novo nasce com o mesmo ID.
                let error = invoke.is_some_and(|v| tables.invocations.get(&v)
                    .or_else(|| plan.pendings.get(&v)) == Some(&dest.id));
                if clear_edges && !error && !tables.landing_pads.contains(&dest.id) {
                    let mut used_set = live_set.entry[&dest.id].clone();
                    for (_, inst, _) in &dest.instructions {
                        if let Instruction::Phi { incoming, .. } = inst {
                            for (de, op) in incoming {
                                if *de != b.id { continue; }
                                if let Operand::Val(v) = op {
                                    let mut current = *v;
                                    let mut seen_set = HashSet::new();
                                    while seen_set.insert(current) {
                                        used_set.insert(current);
                                        if let Some(Ownership::Borrowed { owner: OwnerOrigin::Value(p), .. }) = classes.get(&current) {
                                            current = *p;
                                        } else { break; }
                                    }
                                }
                            }
                        }
                    }
                    let mut dead_set: Vec<_> = r.iter().copied()
                        .filter(|v| !used_set.contains(v) && (!produced.contains(v) || !consumed.contains(v))).collect();
                    dead_set.sort_unstable_by_key(|v| v.0);
                    for v in &dead_set { r.remove(v); }
                    if !dead_set.is_empty() { edges.insert((b.id, dest.id), dead_set); }
                }
                for &v in &produced {
                    produce(v, classes, &mut r)?;
                }
                Ok(r)
            })()
            .map_err(|m| {
                flow_error(
                    f,
                    &pais,
                    i,
                    format!("aresta b{} -> b{}: {m}", b.id.0, dest.id.0),
                )
            })?;
            match &entries[s] {
                Some(previous) if *previous != next_one => {
                    let mut only_before: Vec<_> = previous.difference(&next_one).map(|v| v.0).collect();
                    let mut only_now: Vec<_> = next_one.difference(previous).map(|v| v.0).collect();
                    only_before.sort_unstable();
                    only_now.sort_unstable();
                    return Err(flow_error(
                        f,
                        &pais,
                        i,
                        format!(
                            "junção b{} com tokens diferentes (só antes {only_before:?}, só agora {only_now:?}); caminho anterior {:?}",
                            dest.id.0,
                            path(f, &pais, s)
                        ),
                    ));
                }
                Some(_) => {}
                None => {
                    entries[s] = Some(next_one);
                    pais[s] = Some(i);
                    queue.push_back(s);
                }
            }
        }
    }
    let entries = f
        .blocks
        .iter()
        .zip(entries)
        .filter_map(|(b, owners)| {
            owners.map(|owners| {
                let mut owners: Vec<_> = owners.into_iter().collect();
                owners.sort_by_key(|v| v.0);
                (b.id, owners)
            })
        })
        .collect();
    super::borrows::verify(f, classes)?;
    Ok(TokenFlow { exits, entries, edges })
}

#[cfg(test)]
mod tests;
