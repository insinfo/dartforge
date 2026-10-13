//! Tradução das externs auditadas para o plano de tokens da HIR.
//! Não presume contratos para símbolos ausentes nem certifica proveniência/borrows.

use super::OwnerOrigin;
use super::{
    TokenEffect, Ownership, ScopePlan, TokenPlan, verify_scopes, verify_tokens,
};
use crate::hir::*;
use dartforge_runtime::ownership::{Contrato, ModoParametro, ModoResultado};
use std::collections::{HashMap, HashSet, VecDeque};

/// Contratos dos ajudantes `@df.*` do prelúdio que o lowering chama por
/// `CallRuntime` (`llvm/{boxes,strings,typed_items}_ir.rs`). São IR em linha,
/// fora do catálogo do runtime: leem campos escalares do objeto emprestado
/// (e, em `@df.texto_gravar`, gravam unidades de uma string ainda não
/// publicada), sem alocar, reter, lançar ou chamar Dart — o que os efeitos
/// deles (`HELPER_EFFECTS`) dizem e o teste
/// `helpers_agree_with_effects` confere. `@df.tipada_base` empresta
/// a base da visão, que ela segura numa aresta imutável. Os endereços
/// derivados (`@df.tipada_dados`, `@df.nucleo_armazenamento`, lido por
/// `@df.nucleo_forma`) são escalares cujos acessos `derived_set.rs` segura com
/// `@df.keep_alive(list)`; `@df.palavra_ref` (um `Ref` lido do
/// armazenamento) fica de fora: sob ARC o lowering usa a leitura Owned.
pub const HELPER_CONTRACTS: &[Contrato] = &[
    Contrato { nome: "df.bit_smi", parametros: &[ModoParametro::Borrow], resultado: ModoResultado::ScalarI64, retencao_persistente: false, invalida_borrows: false, pode_falhar: false, chama_dart: false },
    Contrato { nome: "df.valor_smi", parametros: &[ModoParametro::Borrow], resultado: ModoResultado::ScalarI64, retencao_persistente: false, invalida_borrows: false, pode_falhar: false, chama_dart: false },
    Contrato { nome: "df.identico", parametros: &[ModoParametro::Borrow, ModoParametro::Borrow], resultado: ModoResultado::ScalarI1, retencao_persistente: false, invalida_borrows: false, pode_falhar: false, chama_dart: false },
    Contrato { nome: "df.nucleo_len", parametros: &[ModoParametro::Borrow], resultado: ModoResultado::ScalarI64, retencao_persistente: false, invalida_borrows: false, pode_falhar: false, chama_dart: false },
    Contrato { nome: "df.nucleo_len_gravavel", parametros: &[ModoParametro::Borrow, ModoParametro::Scalar], resultado: ModoResultado::ScalarI64, retencao_persistente: false, invalida_borrows: false, pode_falhar: false, chama_dart: false },
    Contrato { nome: "df.texto_gravar", parametros: &[ModoParametro::Borrow, ModoParametro::Scalar, ModoParametro::Scalar], resultado: ModoResultado::Void, retencao_persistente: false, invalida_borrows: false, pode_falhar: false, chama_dart: false },
    Contrato { nome: "df.texto_hash", parametros: &[ModoParametro::Borrow], resultado: ModoResultado::ScalarI64, retencao_persistente: false, invalida_borrows: false, pode_falhar: false, chama_dart: false },
    Contrato { nome: "df.texto_igual", parametros: &[ModoParametro::Borrow, ModoParametro::Borrow], resultado: ModoResultado::ScalarI1, retencao_persistente: false, invalida_borrows: false, pode_falhar: false, chama_dart: false },
    Contrato { nome: "df.texto_igual_a", parametros: &[ModoParametro::Borrow, ModoParametro::Borrow], resultado: ModoResultado::ScalarI1, retencao_persistente: false, invalida_borrows: false, pode_falhar: false, chama_dart: false },
    Contrato { nome: "df.texto_len", parametros: &[ModoParametro::Borrow], resultado: ModoResultado::ScalarI64, retencao_persistente: false, invalida_borrows: false, pode_falhar: false, chama_dart: false },
    Contrato { nome: "df.texto_unidade", parametros: &[ModoParametro::Borrow, ModoParametro::Scalar], resultado: ModoResultado::ScalarI64, retencao_persistente: false, invalida_borrows: false, pode_falhar: false, chama_dart: false },
    // `num` em Ref como double: o caminho que não é Smi/Mint desencaixa pelo
    // runtime (TypeError pendente), como as marcas do ajudante dizem.
    Contrato { nome: "df.num_para_double", parametros: &[ModoParametro::Borrow], resultado: ModoResultado::ScalarF64, retencao_persistente: false, invalida_borrows: true, pode_falhar: true, chama_dart: false },
    Contrato { nome: "df.tipada_base", parametros: &[ModoParametro::Borrow], resultado: ModoResultado::BorrowArg(0), retencao_persistente: false, invalida_borrows: false, pode_falhar: false, chama_dart: false },
    Contrato { nome: "df.tipada_bytes", parametros: &[ModoParametro::Borrow], resultado: ModoResultado::ScalarI64, retencao_persistente: false, invalida_borrows: false, pode_falhar: false, chama_dart: false },
    Contrato { nome: "df.tipada_cid", parametros: &[ModoParametro::Borrow], resultado: ModoResultado::ScalarI64, retencao_persistente: false, invalida_borrows: false, pode_falhar: false, chama_dart: false },
    Contrato { nome: "df.tipada_deslocamento", parametros: &[ModoParametro::Borrow], resultado: ModoResultado::ScalarI64, retencao_persistente: false, invalida_borrows: false, pode_falhar: false, chama_dart: false },
    Contrato { nome: "df.tipada_len", parametros: &[ModoParametro::Borrow], resultado: ModoResultado::ScalarI64, retencao_persistente: false, invalida_borrows: false, pode_falhar: false, chama_dart: false },
    Contrato { nome: "df.tipada_dados", parametros: &[ModoParametro::Borrow], resultado: ModoResultado::ScalarI64, retencao_persistente: false, invalida_borrows: false, pode_falhar: false, chama_dart: false },
    Contrato { nome: "df.nucleo_armazenamento", parametros: &[ModoParametro::Borrow], resultado: ModoResultado::ScalarI64, retencao_persistente: false, invalida_borrows: false, pode_falhar: false, chama_dart: false },
    Contrato { nome: "df.nucleo_forma", parametros: &[ModoParametro::Scalar], resultado: ModoResultado::ScalarI64, retencao_persistente: false, invalida_borrows: false, pode_falhar: false, chama_dart: false },
    Contrato { nome: "df.manter_vivo", parametros: &[ModoParametro::Borrow], resultado: ModoResultado::Void, retencao_persistente: false, invalida_borrows: false, pode_falhar: false, chama_dart: false },
    Contrato { nome: "df.tipada_len_gravavel", parametros: &[ModoParametro::Borrow, ModoParametro::Scalar], resultado: ModoResultado::ScalarI64, retencao_persistente: false, invalida_borrows: false, pode_falhar: false, chama_dart: false },
];

/// O contrato de ownership de `name`: o de um ajudante em IR
/// ([`HELPER_CONTRACTS`]) ou o do catálogo do runtime.
///
/// # Erros
/// Símbolo sem contrato explícito.
///
/// ```
/// use dartforge_emit_native::optimize::arc::contrato;
/// assert!(contrato("df.texto_len").is_ok());
/// assert!(contrato("dartforge_arc_lista_len_v1").is_ok());
/// assert!(contrato("df.palavra_ref").is_err());
/// ```
pub fn contract(name_str: &str) -> Result<&'static Contrato, &'static str> {
    if let Some(base) = name_str.strip_prefix(super::NATIVE_SCOPE_PREFIX) {
        return contract(base);
    }
    if let Some(base) = name_str.strip_prefix(super::GENERIC_OWNED_PREFIX) {
        // Só as marcas valem aqui; os parâmetros saem dos tipos da chamada
        // (`runtime_call_contract`).
        let e = crate::llvm::externs::effects_of(base);
        return Ok(&GENERIC_OWNED[usize::from(e.throws || e.calls_dart) * 2 + usize::from(e.calls_dart)]);
    }
    #[cfg(test)]
    if let Some(c) = TEST_CONTRACTS.iter().find(|c| c.nome == name_str) {
        return Ok(c);
    }
    match HELPER_CONTRACTS.iter().find(|c| c.nome == name_str) {
        Some(c) => Ok(c),
        None => dartforge_runtime::ownership::contrato(name_str),
    }
}

/// Formas de contrato que o catálogo real já não tem, mas que a preparação
/// continua a suportar: um resultado emprestado do argumento 0 que só
/// existe na saída normal.
#[cfg(test)]
pub(super) const TEST_CONTRACTS: &[Contrato] = &[Contrato {
    nome: "df.teste.campo_falivel",
    parametros: &[ModoParametro::Borrow, ModoParametro::Scalar],
    resultado: ModoResultado::BorrowArg(0),
    retencao_persistente: false,
    invalida_borrows: false,
    pode_falhar: true,
    chama_dart: false,
}];

/// As marcas dos thunks Owned genéricos, por (pode falhar, chama Dart).
const GENERIC_OWNED: [Contrato; 4] = {
    const fn c(may_fail: bool, calls_dart: bool) -> Contrato {
        Contrato { nome: "df.arc.owned.*", parametros: &[], resultado: ModoResultado::Owned,
            retencao_persistente: true, invalida_borrows: true, pode_falhar: may_fail, chama_dart: calls_dart }
    }
    [c(false, false), c(false, true), c(true, false), c(true, true)]
};

/// O contrato do thunk Owned genérico de `base` com os argumentos da
/// chamada: cada `Ref` emprestado (SSA ou null), os escalares por valor; o
/// resultado é um owner; falha e reentrada são as da base (`effects.tsv`).
fn generic_owned_contract(base: &str, args: &[(Operand, Type)]) -> Result<RuntimeCallContract, String> {
    let e = crate::llvm::externs::effects_of(base);
    for (n, (op, ty)) in args.iter().enumerate() {
        match (ty, op) {
            (Type::Ref, Operand::Val(_) | Operand::Constant(Constant::Null)) => {}
            (Type::Ref, _) => return Err(format!("{base}: argumento Ref {n} exige SSA ou null")),
            // Ptr: dados constantes do módulo (a tabela de um literal); um
            // endereço de local escalar não sai da função (`locals.rs`).
            (Type::I64 | Type::F64 | Type::I8 | Type::I1 | Type::Ptr, Operand::Val(_)) => {}
            (Type::I64 | Type::I8, Operand::Constant(Constant::Int(_) | Constant::Bool(_))) => {}
            (Type::I1, Operand::Constant(Constant::Bool(_))) => {}
            (Type::F64, Operand::Constant(Constant::Double(_))) => {}
            _ => return Err(format!("{base}: argumento {n} sem representação do thunk Owned")),
        }
    }
    Ok(RuntimeCallContract {
        effect: TokenEffect { may_fail: e.throws || e.calls_dart, ..Default::default() },
        result: Ownership::Owned,
        persistent_retention: true,
        invalidates_borrows: true,
        calls_dart: e.calls_dart,
    })
}

/// Produz contratos dos parâmetros Ref pela convenção de chamadas Dart (§20.1).
///
/// Ref é emprestado pelo chamador durante a invocação. Outros tipos exigem
/// produtores próprios: I64 não se torna escalar/referência pela largura.
/// Use somente para funções que seguem essa convenção, antes da produção
/// das instruções. Não certifica contratos de callees nem insere RC/cleanup.
///
/// # Erros
/// Parâmetro repetido ou contrato fornecido diferente de Borrowed(Chamador, 0)
/// para um parâmetro Ref. Nenhuma classe é publicada em caso de erro.
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
/// let mut classes = HashMap::new();
/// produzir_parametros_ref_dart(&f, &mut classes)?;
/// produzir_e_verificar_tokens(&f, &mut classes, &mut PlanoTokens::default(),
///     &TabelasDaFuncao::default(), &PlanoEscopos::default())?;
/// # Ok::<(), String>(())
/// ```
pub fn produce_dart_ref_parameters(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
) -> Result<(), String> {
    let expected = Ownership::Borrowed {
        owner: OwnerOrigin::Caller,
        scope: 0,
    };
    let mut ids = HashSet::new();
    let mut produced = Vec::new();
    for (v, _, ty) in &f.params {
        if !ids.insert(*v) {
            return Err(format!("parâmetro Dart v{} repetido", v.0));
        }
        if *ty == Type::Ref {
            if classes.get(v).is_some_and(|c| *c != expected) {
                return Err(format!(
                    "parâmetro Dart Ref v{} exige Borrowed(Chamador, 0)",
                    v.0
                ));
            }
            produced.push(*v);
        }
    }
    for v in produced {
        classes.insert(v, expected.clone());
    }
    Ok(())
}

/// Insere retenção nos retornos Ref Dart e publica somente a função verificada.
///
/// Retorno SSA Borrowed/Trivial recebe ArcCopy; Owned transfere seu token.
/// Null direto dispensa contagem, e literais permanentes diretos são avaliados
/// por Const antes da cópia. Exige plano de retorno Owned para Ref. Demais
/// limpezas, contratos de callees e metadados não cobertos são premissas do
/// lowering; não insere drops, cleanup excepcional ou proteção de suspensão.
///
/// # Erros
/// HIR/contratos inválidos, IDs esgotados ou falha do wrapper Dart completo.
/// Função, classes e plano permanecem intactos. Retorna o número de cópias.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let mut f = Function { symbol: "identidade".into(), name: "identidade".into(), depuracao: None,
///     params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] };
/// let mut plano = PlanoTokens { retorno: RetornoTokens::Owned, ..Default::default() };
/// assert_eq!(inserir_retencao_retornos_dart(&mut f, &mut HashMap::new(), &mut plano,
///     &TabelasDaFuncao::default(), &PlanoEscopos::default())?, 1);
/// # Ok::<(), String>(())
/// ```
pub fn insert_dart_return_retention(
    f: &mut Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
    tables: &FunctionTables,
    scopes: &ScopePlan,
) -> Result<usize, String> {
    insert_dart_exits(f, classes, plan, tables, scopes, false).map(|(copies, _)| copies)
}

/// Insere retenção do retorno e libera tokens restantes em cada Return Dart.
///
/// Usa o mesmo fluxo do verificador: o token devolvido é transferido, e os
/// demais recebem ArcDrop antes do retorno, em ordem determinística de IDs.
/// Inclui retornos de caminhos excepcionais já preparados. Exige junções
/// com inventários compatíveis; não divide arestas, fecha quadros nem prepara
/// finally/cancelamento/suspensão. Contratos não cobertos continuam explícitos.
/// Saídas Lanca/Retoma devolvem apenas o placeholder e liberam tokens locais;
/// saída Guarda exige separar sucesso/erro no CFG antes deste passe.
///
/// # Erros
/// HIR/contratos inválidos, IDs esgotados ou falha de fluxo/escopo/quadros.
/// Função e mapas permanecem intactos. Retorna (retenções, liberações).
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let mut f = Function { symbol: "f".into(), name: "f".into(), depuracao: None,
///     params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
///         (ValueId(1), Instruction::ArcCopy { value: Operand::Val(ValueId(0)) }, Type::Ref)],
///         terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] };
/// let mut plano = PlanoTokens { retorno: RetornoTokens::Owned, ..Default::default() };
/// assert_eq!(inserir_arc_saidas_dart(&mut f, &mut HashMap::new(), &mut plano,
///     &TabelasDaFuncao::default(), &PlanoEscopos::default())?, (1, 1));
/// # Ok::<(), String>(())
/// ```
pub fn insert_arc_dart_exits(
    f: &mut Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
    tables: &FunctionTables,
    scopes: &ScopePlan,
) -> Result<(usize, usize), String> {
    insert_dart_exits(f, classes, plan, tables, scopes, true)
}

fn insert_dart_exits(
    f: &mut Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
    tables: &FunctionTables,
    scopes: &ScopePlan,
    cleanup: bool,
) -> Result<(usize, usize), String> {
    let mut new_one = f.clone();
    super::tokens::normalize_exits_throws(&mut new_one, tables)?;
    let mut new_classes = classes.clone();
    let mut new_plan = plan.clone();
    produce_dart_ref_parameters(&new_one, &mut new_classes)?;
    produce_arc_contracts(&new_one, &mut new_classes, &mut new_plan)?;
    super::classified_liveness(&new_one, &new_classes)?;
    // Reserva também IDs de metadados: uma inserção não pode mascarar entrada
    // obsoleta fazendo-a coincidir com uma definição recém-criada.
    let mut next_item = new_one
        .params
        .iter()
        .map(|(v, _, _)| v.0)
        .chain(
            new_one.blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .map(|(v, _, _)| v.0),
        )
        .chain(new_plan.instructions.keys().map(|v| v.0))
        .chain(new_plan.pendings.keys().map(|v| v.0))
        .chain(tables.invocations.keys().map(|v| v.0))
        .chain(scopes.before.keys().map(|v| v.0))
        .max()
        .map_or(0, |v| u64::from(v) + 1);
    let mut id = || -> Result<ValueId, String> {
        let v = u32::try_from(next_item).map_err(|_| "IDs SSA esgotados ao preparar retorno Dart")?;
        next_item += 1;
        Ok(ValueId(v))
    };
    let copies = copy_returns(&mut new_one, &new_classes, &mut id)?;
    let mut releases = 0;
    if cleanup {
        produce_arc_contracts(&new_one, &mut new_classes, &mut new_plan)?;
        let exits =
            super::tokens::cleanup_exits(&new_one, &new_classes, tables, &new_plan)?;
        for b in &mut new_one.blocks {
            if let Some(remaining) = exits.get(&b.id) {
                for de in remaining {
                    b.instructions.push((
                        id()?,
                        Instruction::ArcDrop {
                            value: Operand::Val(*de),
                        },
                        Type::Void,
                    ));
                    releases += 1;
                }
            }
        }
    }
    produce_and_verify_dart_tokens(&new_one, &mut new_classes, &mut new_plan, tables, scopes)?;
    *f = new_one;
    *classes = new_classes;
    *plan = new_plan;
    Ok((copies, releases))
}

pub(super) fn copy_returns(
    f: &mut Function,
    classes: &HashMap<ValueId, Ownership>,
    id: &mut impl FnMut() -> Result<ValueId, String>,
) -> Result<usize, String> {
    let mut copies = 0;
    if f.return_ty == Type::Ref {
        for b in &mut f.blocks {
            let Terminator::Return(Some(op)) = &b.terminator else {
                continue;
            };
            let mut op = op.clone();
            match &op {
                Operand::Val(v) => match classes.get(v) {
                    Some(Ownership::Owned) => continue,
                    Some(_) => {},
                    None => return Err(format!("retorno em {} b{}: v{} sem classificação", f.symbol, b.id.0, v.0)),
                },
                Operand::Constant(Constant::Null) => continue,
                Operand::Constant(c @ (Constant::String(_) | Constant::StringWtf8(_))) => {
                    let v = id()?;
                    b.instructions
                        .push((v, Instruction::Const(c.clone()), Type::Ref));
                    op = Operand::Val(v);
                }
                _ => {}
            }
            let v = id()?;
            b.instructions
                .push((v, Instruction::ArcCopy { value: op }, Type::Ref));
            b.terminator = Terminator::Return(Some(Operand::Val(v)));
            copies += 1;
        }
    }
    Ok(copies)
}

/// Produz e verifica parâmetros/instruções pela convenção Dart, atomicamente.
///
/// Parâmetros Ref são Borrowed(Chamador,0). Resultado Ref exige retorno Owned
/// no plano; resultados de outros tipos exigem retorno Trivial. O lowering
/// deve preparar transferências/cleanup antes desta chamada. Tipos não Ref
/// dos parâmetros e instruções fora da cobertura exigem contratos semânticos.
/// Não infere contratos de callees nem insere RC/cleanup.
///
/// # Erros
/// Convenção de retorno incompatível, conflito de parâmetro ou qualquer erro
/// de produzir_e_verificar_tokens. Classes e plano permanecem intactos.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "identidade".into(), name: "identidade".into(), depuracao: None,
///     params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
///         (ValueId(1), Instruction::ArcCopy { value: Operand::Val(ValueId(0)) }, Type::Ref)],
///         terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }] };
/// let mut plano = PlanoTokens { retorno: RetornoTokens::Owned, ..Default::default() };
/// produzir_e_verificar_tokens_dart(&f, &mut HashMap::new(), &mut plano,
///     &TabelasDaFuncao::default(), &PlanoEscopos::default())?;
/// # Ok::<(), String>(())
/// ```
pub fn produce_and_verify_dart_tokens(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
    tables: &FunctionTables,
    scopes: &ScopePlan,
) -> Result<HashMap<ValueId, RuntimeCallContract>, String> {
    let ret = if f.return_ty == Type::Ref {
        super::TokenReturn::Owned
    } else {
        super::TokenReturn::Trivial
    };
    if plan.ret != ret {
        return Err(format!(
            "convenção Dart em {} exige resultado Ref Owned e demais resultados Trivial ({:?} com {:?})",
            f.symbol, f.return_ty, plan.ret
        ));
    }
    let mut new_classes = classes.clone();
    let mut new_plan = plan.clone();
    produce_dart_ref_parameters(f, &mut new_classes)?;
    let contracts =
        produce_and_verify_tokens(f, &mut new_classes, &mut new_plan, tables, scopes)?;
    *classes = new_classes;
    *plan = new_plan;
    Ok(contracts)
}

/// Produz metadados ARC e os publica após verificar CFG/SSA, tokens e escopos.
///
/// Parâmetros e instruções fora da cobertura do produtor exigem contratos
/// semânticos fornecidos pelo chamador. O CFG e os planos devem ser da mesma
/// versão da função. Quadros locais exigem abertura/fechamento LIFO e pilha
/// consistente por caminho; IDs importados/aliases de quadros são recusados.
/// Limites constantes são conferidos quando a capacidade é conhecida. Não insere
/// RC nem certifica limites dinâmicos, proveniência geral, aliases de slots ou Finalizable.
/// Rejeita borrows derivados após barreiras conservadoras, sem produzir keepalive.
///
/// # Erros
/// CFG/SSA inválido, falha de produção, inventário incompleto, token indisponível/não consumido,
/// CFG excepcional incompatível ou empréstimo fora de escopo. Em qualquer
/// desses casos, classes e plano de tokens permanecem intactos.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "f".into(), name: "f".into(), depuracao: None,
///     params: vec![], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
///         (ValueId(0), Instruction::ArcCopy { value: Operand::Constant(Constant::Null) }, Type::Ref),
///         (ValueId(1), Instruction::ArcDrop { value: Operand::Val(ValueId(0)) }, Type::Void)],
///         terminator: Terminator::Return(None) }] };
/// produzir_e_verificar_tokens(&f, &mut HashMap::new(), &mut PlanoTokens::default(),
///     &TabelasDaFuncao::default(), &PlanoEscopos::default())?;
/// # Ok::<(), String>(())
/// ```
pub fn produce_and_verify_tokens(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
    tables: &FunctionTables,
    scopes: &ScopePlan,
) -> Result<HashMap<ValueId, RuntimeCallContract>, String> {
    super::ssa::verify(f)?;
    let mut new_classes = classes.clone();
    let mut new_plan = plan.clone();
    let contracts = produce_arc_contracts(f, &mut new_classes, &mut new_plan)?;
    verify_tokens(f, &new_classes, tables, &new_plan)?;
    verify_scopes(f, &new_classes, scopes)?;
    super::frames::verify_with_plan(f, &new_plan)?;
    *classes = new_classes;
    *plan = new_plan;
    Ok(contracts)
}

/// Produz classes de resultado e consumo de todas as chamadas runtime da função.
///
/// Aplica as alterações apenas depois de conferir todas as chamadas. Parâmetros,
/// operações ordinárias e chamadas Dart exigem metadados de outros produtores.
/// Retorna os contratos para análise posterior de retenção/invalidação.
/// Reconhece conferência explícita de pendência no fim do bloco e produz
/// o mapa de saídas runtime; formas não reconhecidas exigem outro produtor.
/// Confere tipos SSA dos argumentos, mas não sua dominância/proveniência.
/// Não insere contadores nem limpa escopos.
///
/// # Erros
/// Extern sem contrato, assinatura inválida, argumento SSA ausente ou de tipo
/// diferente da anotação da chamada, resultado incompatível com o tipo
/// da instrução, IDs repetidos ou conflito com classe/efeito já fornecido.
/// As duas entradas permanecem intactas em caso de erro.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "f".into(), name: "f".into(), depuracao: None,
///     params: vec![], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![(ValueId(0),
///         Instruction::CallRuntime { name: "dartforge_arc_collect".into(), args: vec![], ret_ty: Type::Void }, Type::Void)],
///         terminator: Terminator::Return(None) }] };
/// let mut classes = HashMap::new();
/// let mut plano = PlanoTokens::default();
/// produzir_contratos_runtime(&f, &mut classes, &mut plano)?;
/// assert_eq!(classes[&ValueId(0)], Ownership::Trivial);
/// assert!(!plano.instrucoes[&ValueId(0)].pode_falhar);
/// # Ok::<(), String>(())
/// ```
pub fn produce_runtime_contracts(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
) -> Result<HashMap<ValueId, RuntimeCallContract>, String> {
    produce(f, classes, plan, false)
}

/// Produz classes das operações ARC explícitas e contratos runtime auditados.
///
/// Copy/move/load produzem Owned; drop/store produzem Trivial. Operações ARC
/// não recebem entrada em PlanoTokens, pois o verificador possui suas regras.
/// ICmp/FCmp/LNot produzem bool Trivial sem consumo nem saída excepcional.
/// Unbox bool produz I1 Trivial com o contrato Borrow falível da ABI u8;
/// o helper LLVM converte o resultado e preserva a conferência de pendência.
/// Constantes escalares/null e literais permanentes também produzem Trivial,
/// com tipo determinado pela variante da constante, nunca por largura.
/// Aritmética inteira/flutuante e conversões numéricas produzem Trivial;
/// operandos devem ter contrato Trivial e representações escalares compatíveis. Guardas
/// de domínio/estouro e semântica Dart permanecem responsabilidade do lowering.
/// Bitcasts F64/I64 exigem origem escalar Trivial; largura não certifica ownership.
/// ZExt/Trunc entre I1/I8/I64 exigem origem Trivial, tipo exato e aumento/redução
/// de largura respectivamente; não convertem handles nem ponteiros em escalares.
/// Locais escalares na entrada exigem endereço sem escape, gravações tipadas
/// Trivial e inicialização em todos os caminhos até cada leitura.
/// Phi I1/I64/F64 exige entradas de mesmo tipo com contrato Trivial ou constantes
/// correspondentes, e origem conhecida fora do ciclo de Phis, inclusive em laços.
/// Phi Ref explicitamente Trivial exige entradas Trivial/null e origem externa;
/// a classe fornecida não permite apagar ownership de uma entrada gerenciada.
/// Phis Ref sem demanda Owned e com origem Trivial/null conhecida recebem
/// Trivial automaticamente, inclusive ciclos ancorados.
/// Phis Ref ainda não classificados preservam automaticamente contratos Borrowed
/// idênticos nas entradas conhecidas; cadeias e ciclos ancorados independem
/// da ordem dos blocos. Entradas pendentes são conferidas após a propagação.
/// Phi Ref ainda não classificado exige entradas owned/null e origem externa
/// ao ciclo de Phi/move. Parâmetros e demais operações exigem produtores próprios.
/// Não insere ARC nem certifica vida dos slots, proveniência ou cleanup.
///
/// # Erros
/// Os erros de produzir_contratos_runtime, tipo incompatível de operação ARC,
/// conflito de classe ou tentativa de sobrescrever sua regra no plano.
/// Phi owned com entrada emprestada/não classificada ou ciclo sem origem.
/// Nenhum mapa é alterado em caso de erro.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "f".into(), name: "f".into(), depuracao: None,
///     params: vec![], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
///         (ValueId(0), Instruction::ArcCopy { value: Operand::Constant(Constant::Null) }, Type::Ref),
///         (ValueId(1), Instruction::ArcDrop { value: Operand::Val(ValueId(0)) }, Type::Void)],
///         terminator: Terminator::Return(None) }] };
/// let mut classes = HashMap::new();
/// let mut plano = PlanoTokens::default();
/// produzir_contratos_arc(&f, &mut classes, &mut plano)?;
/// verificar_tokens(&f, &classes, &TabelasDaFuncao::default(), &plano)?;
/// # Ok::<(), String>(())
/// ```
pub fn produce_arc_contracts(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
) -> Result<HashMap<ValueId, RuntimeCallContract>, String> {
    let mut new_classes = classes.clone();
    let mut new_plan = plan.clone();
    let locals = super::locals::produce(f, &mut new_classes, &mut new_plan)?;
    let contracts = produce(f, &mut new_classes, &mut new_plan, true)?;
    produce_phi(f, &mut new_classes, &new_plan)?;
    check_typed_scalars(f, &new_classes)?;
    super::locals::verify_values(f, &locals, &new_classes)?;
    super::pure_set::check_origins(f, &new_classes)?;
    *classes = new_classes;
    *plan = new_plan;
    Ok(contracts)
}

/// [`produce_arc_contracts`] sem resolver os Phis nem as conferências que
/// dependem deles. Não publica nada em erro.
pub(super) fn produce_contracts_without_phi(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
) -> Result<(), String> {
    let mut new_classes = classes.clone();
    let mut new_plan = plan.clone();
    super::locals::produce(f, &mut new_classes, &mut new_plan)?;
    produce(f, &mut new_classes, &mut new_plan, true)?;
    *classes = new_classes;
    *plan = new_plan;
    Ok(())
}

/// Cada argumento escalar de uma `TypedCall` (inclusive os Phis, já
/// classificados) é Trivial: um word Dart não passa por um slot escalar.
fn check_typed_scalars(f: &Function, classes: &HashMap<ValueId, Ownership>) -> Result<(), String> {
    for (_, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
        if let Instruction::TypedCall { args, .. } = inst {
            for (op, t) in args {
                if *t != Type::Ref && let Operand::Val(v) = op && classes.get(v) != Some(&Ownership::Trivial) {
                    return Err(format!("v{}: argumento escalar tipado exige Trivial, sem word Dart", v.0));
                }
            }
        }
    }
    Ok(())
}

/// Resolve a propriedade dos Phi Ref, inclusive ciclos com origem conhecida.
/// A disponibilidade e o consumo simultâneo por aresta são provados depois
/// pelo verificador de tokens, não pela conectividade deste grafo.
fn produce_phi(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &TokenPlan,
) -> Result<(), String> {
    let defs: HashMap<_, _> = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .map(|(v, i, _)| (*v, i))
        .collect();
    let mut candidates = HashSet::new();
    let types: HashMap<_, _> = f
        .params
        .iter()
        .map(|(v, _, ty)| (*v, *ty))
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .map(|(v, _, ty)| (*v, *ty)),
        )
        .collect();
    // Demandas explícitas de transferência/consumo conservam o token lógico,
    // inclusive para null, cuja contagem física pode ser vazia.
    let mut require_owned: HashSet<_> = plan
        .instructions
        .values()
        .flat_map(|e| e.always.iter().chain(&e.success).chain(&e.error))
        .copied()
        .collect();
    for b in &f.blocks {
        for (v, inst, _) in &b.instructions {
            let op = match inst {
                Instruction::ArcMove { value }
                | Instruction::ArcDrop { value }
                | Instruction::ArcStoreStrong {
                    value,
                    mode: StrongStoreMode::Move,
                    ..
                } => Some(value),
                _ => None,
            };
            if let Some(Operand::Val(de)) = op {
                require_owned.insert(*de);
            }
            if classes.get(v) == Some(&Ownership::Owned) {
                require_owned.insert(*v);
            }
        }
        if plan.ret == super::TokenReturn::Owned
            && let Terminator::Return(Some(Operand::Val(v))) = b.terminator
        {
            require_owned.insert(v);
        }
    }
    loop {
        let previous = require_owned.len();
        for v in require_owned.clone() {
            if let Some(Instruction::Phi { incoming, .. }) = defs.get(&v) {
                require_owned.extend(incoming.iter().filter_map(|(_, op)| {
                    if let Operand::Val(de) = op {
                        Some(*de)
                    } else {
                        None
                    }
                }));
            }
        }
        if previous == require_owned.len() {
            break;
        }
    }
    // Uma âncora null não prova que todos os Phis ainda desconhecidos sejam
    // triviais. Primeiro elimina a componente que depende de Owned, borrow
    // ou origem sem contrato; só depois propaga as âncoras triviais restantes.
    let free_set: HashSet<_> = f.blocks.iter().flat_map(|b| &b.instructions)
        .filter_map(|(v, i, ty)| (*ty == Type::Ref && classes.get(v).is_none()
            && !require_owned.contains(v) && matches!(i, Instruction::Phi { .. })).then_some(*v))
        .collect();
    let mut trivial_parents: HashMap<ValueId, Vec<ValueId>> = HashMap::new();
    let mut invalid_set = VecDeque::new();
    let mut trivial_anchors = VecDeque::new();
    for v in &free_set {
        let Instruction::Phi { incoming, .. } = defs[v] else { unreachable!() };
        let mut compatible = !incoming.is_empty();
        let mut anchor = false;
        for (_, op) in incoming {
            match op {
                Operand::Constant(Constant::Null) => anchor = true,
                Operand::Val(de) if types.get(de) == Some(&Type::Ref)
                    && classes.get(de) == Some(&Ownership::Trivial) => anchor = true,
                Operand::Val(de) if free_set.contains(de) => {
                    trivial_parents.entry(*de).or_default().push(*v);
                },
                _ => compatible = false,
            }
        }
        if !compatible { invalid_set.push_back(*v); }
        if anchor { trivial_anchors.push_back(*v); }
    }
    let mut compatible_ones = free_set;
    while let Some(v) = invalid_set.pop_front() {
        if compatible_ones.remove(&v) {
            invalid_set.extend(trivial_parents.get(&v).into_iter().flatten().copied());
        }
    }
    while let Some(v) = trivial_anchors.pop_front() {
        if compatible_ones.contains(&v) && classes.get(&v).is_none() {
            classes.insert(v, Ownership::Trivial);
            trivial_anchors.extend(trivial_parents.get(&v).into_iter().flatten().copied());
        }
    }
    // Resolve primeiro empréstimos com premissas conhecidas. Não transforma
    // entradas Owned em borrow implícito; um ciclo precisa de âncora externa.
    let mut borrowed_set = HashMap::new();
    loop {
        let mut changed = false;
        for (v, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if *ty != Type::Ref || classes.contains_key(v) {
                continue;
            }
            let Instruction::Phi { incoming, .. } = inst else {
                continue;
            };
            let mut origin = None;
            let compatible_ones = !incoming.is_empty()
                && incoming.iter().all(|(_, op)| match op {
                    Operand::Constant(Constant::Null) => true,
                    Operand::Val(de) if types.get(de) == Some(&Type::Ref) => {
                        match classes.get(de) {
                            Some(c @ Ownership::Borrowed { .. }) => {
                                if let Some((_, previous)) = &origin {
                                    previous == c
                                } else {
                                    origin = Some((*de, c.clone()));
                                    true
                                }
                            }
                            Some(Ownership::Trivial) => true,
                            None if matches!(
                                defs.get(de),
                                Some(Instruction::Phi { ty: Type::Ref, .. })
                            ) =>
                            {
                                true
                            }
                            _ => false,
                        }
                    }
                    _ => false,
                });
            if compatible_ones && let Some((de, mut class)) = origin {
                borrowed_set.insert(*v, class.clone());
                // Valores locais não declaram Chamador diretamente: conservam
                // a dependência no parâmetro que sustenta esse empréstimo.
                if let Ownership::Borrowed {
                    owner: OwnerOrigin::Caller,
                    scope,
                } = class
                {
                    class = Ownership::Borrowed {
                        owner: OwnerOrigin::Value(de),
                        scope,
                    };
                }
                classes.insert(*v, class);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // A âncora permite percorrer o ciclo, mas não dispensa verificar cada
    // entrada depois que o ponto fixo fornece as classes dos Phis dependentes.
    for (v, origin) in &borrowed_set {
        let Instruction::Phi { incoming, .. } = defs[v] else {
            unreachable!()
        };
        if incoming.iter().any(|(_, op)| match op {
            Operand::Constant(Constant::Null) => false,
            Operand::Val(de) if types.get(de) == Some(&Type::Ref) => !classes
                .get(de)
                .is_some_and(|c| *c == Ownership::Trivial || c == origin || c == &classes[v]),
            _ => true,
        }) {
            return Err(format!(
                "Phi borrowed v{}: entrada incompatível após propagação",
                v.0
            ));
        }
    }
    let mut order = Vec::new();
    let scalars: HashSet<_> = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .filter_map(|(v, i, ty)| matches!(i, Instruction::Phi { .. }).then_some((*v, *ty)))
        .filter(|(v, ty)| {
            // Vetores SIMD sem caixa também: não guardam handle.
            matches!(ty, Type::I1 | Type::I8 | Type::I64 | Type::F64) || ty.is_vector()
                || (*ty == Type::Ref && classes.get(v) == Some(&Ownership::Trivial))
        })
        .map(|(v, _)| v)
        .collect();
    let mut scalar_parents: HashMap<ValueId, Vec<ValueId>> = HashMap::new();
    let mut scalar_queue = VecDeque::new();
    for (v, i, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
        if let Instruction::Phi { ty: declared, .. } = i {
            if ty != declared || plan.instructions.contains_key(v) {
                return Err(format!("Phi v{}: tipo ou plano incompatível", v.0));
            }
            if scalars.contains(v) {
                let Instruction::Phi { incoming, .. } = i else {
                    unreachable!()
                };
                if incoming.is_empty() || classes.get(v).is_some_and(|c| *c != Ownership::Trivial) {
                    return Err(format!(
                        "Phi v{}: classe ou entradas escalares incompatíveis",
                        v.0
                    ));
                }
                let mut anchor = false;
                for (_, op) in incoming {
                    match op {
                        Operand::Constant(Constant::Null) if *ty == Type::Ref => anchor = true,
                        Operand::Constant(Constant::Bool(_)) if *ty == Type::I1 => anchor = true,
                        Operand::Constant(Constant::Int(_)) if matches!(ty, Type::I64 | Type::I8) => anchor = true,
                        Operand::Constant(Constant::Double(_)) if *ty == Type::F64 => anchor = true,
                        Operand::Val(de) if types.get(de) == Some(ty) && scalars.contains(de) => {
                            scalar_parents.entry(*de).or_default().push(*v);
                        }
                        Operand::Val(de)
                            if types.get(de) == Some(ty)
                                && classes.get(de) == Some(&Ownership::Trivial) =>
                        {
                            anchor = true
                        }
                        _ => {
                            return Err(format!(
                                "Phi v{}: entrada sem contrato escalar compatível",
                                v.0
                            ));
                        }
                    }
                }
                if anchor {
                    scalar_queue.push_back(*v);
                }
            }
            if *ty == Type::Ref && classes.get(v).is_none_or(|c| *c == Ownership::Owned) {
                candidates.insert(*v);
                order.push(*v);
            }
        }
    }
    // A largura física não prova ausência de ownership. Só propagamos contratos
    // a partir de entradas auditadas, rejeitando ciclos sem origem externa.
    let mut scalar_founded = HashSet::new();
    while let Some(v) = scalar_queue.pop_front() {
        if scalar_founded.insert(v) {
            if let Some(dependents) = scalar_parents.get(&v) {
                scalar_queue.extend(dependents);
            }
        }
    }
    for v in scalars {
        if !scalar_founded.contains(&v) {
            return Err(format!("Phi v{} sem origem escalar fora do ciclo", v.0));
        }
        classes.insert(v, Ownership::Trivial);
    }
    let mut pais: HashMap<ValueId, Vec<ValueId>> = HashMap::new();
    let mut queue = VecDeque::new();
    for v in &order {
        let Instruction::Phi { incoming, .. } = defs[v] else {
            unreachable!()
        };
        if incoming.is_empty() {
            return Err(format!("Phi v{} sem entradas", v.0));
        }
        let mut anchor = false;
        for (_, op) in incoming {
            let mut op = op;
            let mut moves = HashSet::new();
            loop {
                match op {
                    Operand::Constant(Constant::Null) => {
                        anchor = true;
                        break;
                    }
                    Operand::Val(de) if types.get(de) != Some(&Type::Ref) => {
                        return Err(format!(
                            "Phi v{}: entrada v{} não tem representação Ref",
                            v.0, de.0
                        ));
                    }
                    Operand::Val(de) if candidates.contains(de) => {
                        pais.entry(*de).or_default().push(*v);
                        break;
                    }
                    Operand::Val(de) if classes.get(de) == Some(&Ownership::Owned) => {
                        if let Some(Instruction::ArcMove { value }) = defs.get(de) {
                            if !moves.insert(*de) {
                                return Err(format!("Phi v{}: ciclo de moves sem origem", v.0));
                            }
                            op = value;
                        } else {
                            anchor = true;
                            break;
                        }
                    }
                    _ => {
                        return Err(format!(
                            "Phi v{} em {}: entrada não é owned/null; copie o empréstimo na aresta",
                            v.0, f.symbol
                        ));
                    }
                }
            }
        }
        if anchor {
            queue.push_back(*v);
        }
    }
    let mut founded = HashSet::new();
    while let Some(v) = queue.pop_front() {
        if founded.insert(v) {
            if let Some(dependents) = pais.get(&v) {
                queue.extend(dependents);
            }
        }
    }
    for v in order {
        if !founded.contains(&v) {
            return Err(format!("Phi v{} sem origem owned/null fora do ciclo", v.0));
        }
        classes.insert(v, Ownership::Owned);
    }
    Ok(())
}

// O helper LLVM mantém o ID e converte u8 em I1; o caminho lento tem
// exatamente o contrato da ABI auditada, incluindo TypeError/reentrada.
pub(super) fn unbox_bool_call(inst: &Instruction) -> Option<Instruction> {
    let Instruction::Unbox { op, to: Type::I1 } = inst else {
        return None;
    };
    Some(Instruction::CallRuntime {
        name: "dartforge_unbox_bool".into(),
        args: vec![(op.clone(), Type::Ref)],
        ret_ty: Type::I8,
    })
}

fn produce(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
    include_arc: bool,
) -> Result<HashMap<ValueId, RuntimeCallContract>, String> {
    let mut contracts = HashMap::new();
    let mut fixed_set = HashMap::new();
    let mut pure_effects = HashMap::new();
    // Inclui definições de todos os blocos: a ordem física não é dominância.
    // A disponibilidade por caminho continua a cargo do verificador SSA.
    let types: HashMap<_, _> = f
        .params
        .iter()
        .map(|(v, _, ty)| (*v, *ty))
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .map(|(v, _, ty)| (*v, *ty)),
        )
        .collect();
    let mut ids = std::collections::HashSet::new();
    for (v, _, _) in &f.params {
        if !ids.insert(*v) {
            return Err(format!("v{} repetido", v.0));
        }
    }
    for (v, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
        if !ids.insert(*v) {
            return Err(format!("v{} repetido", v.0));
        }
        if include_arc {
            if let Instruction::AllocList { elements } = inst {
                if *ty != Type::Ref { return Err(format!("v{}: literal de lista exige resultado Ref", v.0)); }
                for (op, _) in elements {
                    // As pistas de tags não certificam representação. A emissão
                    // deriva as tags dos tipos reais e monta buffers locais.
                    let t = match op {
                        Operand::Val(arg) => types.get(arg).copied(),
                        Operand::Constant(Constant::Int(_)) => Some(Type::I64),
                        Operand::Constant(Constant::Double(_)) => Some(Type::F64),
                        Operand::Constant(Constant::Bool(_)) => Some(Type::I1),
                        Operand::Constant(Constant::Null | Constant::String(_) | Constant::StringWtf8(_)) => Some(Type::Ref),
                        Operand::Constant(Constant::Function(_)) => None,
                    };
                    if !matches!(t, Some(Type::Ref | Type::I64 | Type::F64 | Type::I1)) {
                        return Err(format!("v{}: elemento de lista sem representação Dart auditada", v.0));
                    }
                }
                let c = runtime_call_contract(&Instruction::CallRuntime {
                    name: "dartforge_arc_lista_nova_owned_v1".into(),
                    args: vec![(Operand::Val(ValueId(0)), Type::Ptr),
                        (Operand::Constant(Constant::Int(elements.len() as i64)), Type::I64),
                        (Operand::Constant(Constant::Int(10)), Type::I64),
                        (Operand::Constant(Constant::Int(0)), Type::I64)],
                    ret_ty: Type::Ref,
                })?;
                if classes.get(v).is_some_and(|class| *class != c.result)
                    || plan.instructions.get(v).is_some_and(|e| *e != c.effect) {
                    return Err(format!("v{}: contrato conflitante de literal de lista", v.0));
                }
                contracts.insert(*v, c);
            }
            if let Instruction::AllocRecord { elements } = inst {
                // Sob ARC todo record literal vai à fábrica Owned, que só
                // recebe referências (o lowering encaixota os escalares).
                let reference = |op: &Operand| match op {
                    Operand::Val(arg) => types.get(arg) == Some(&Type::Ref),
                    Operand::Constant(Constant::Null) => true,
                    _ => false,
                };
                if *ty != Type::Ref || !elements.iter().all(|(op, _)| reference(op)) {
                    return Err(format!("v{}: record literal exige campos Ref em SSA", v.0));
                }
                let c = runtime_call_contract(&Instruction::CallRuntime {
                    name: "dartforge_arc_record_novo_owned_v1".into(),
                    args: vec![(Operand::Val(ValueId(0)), Type::Ptr),
                        (Operand::Constant(Constant::Int(elements.len() as i64)), Type::I64)],
                    ret_ty: Type::Ref,
                })?;
                if classes.get(v).is_some_and(|class| *class != c.result)
                    || plan.instructions.get(v).is_some_and(|e| *e != c.effect) {
                    return Err(format!("v{}: contrato conflitante de record literal", v.0));
                }
                contracts.insert(*v, c);
            }
            if let Instruction::ConcatStrings { parts } = inst {
                // A emissão deriva a espécie de cada par do tipo real: texto
                // Ref ou `int` sem caixa. Textos chegam por `Const` (o emissor
                // não materializa string em operando); outra forma não tem ABI.
                let representable = |op: &Operand| match op {
                    Operand::Val(arg) => matches!(types.get(arg), Some(Type::Ref | Type::I64)),
                    Operand::Constant(Constant::Int(_)) => true,
                    _ => false,
                };
                if *ty != Type::Ref || !parts.iter().all(representable) {
                    return Err(format!("v{}: interpolação sem partes texto/int auditadas", v.0));
                }
                let c = runtime_call_contract(&Instruction::CallRuntime {
                    name: "dartforge_arc_string_juntar_owned_v1".into(),
                    args: vec![(Operand::Val(ValueId(0)), Type::Ptr),
                        (Operand::Constant(Constant::Int(parts.len() as i64)), Type::I64)],
                    ret_ty: Type::Ref,
                })?;
                if classes.get(v).is_some_and(|class| *class != c.result)
                    || plan.instructions.get(v).is_some_and(|e| *e != c.effect) {
                    return Err(format!("v{}: contrato conflitante de interpolação", v.0));
                }
                contracts.insert(*v, c);
            }
            if let Instruction::Box { op, from } = inst && from.is_vector() {
                if *ty != Type::Ref || !matches!(op, Operand::Val(v) if types.get(v) == Some(from)) {
                    return Err(format!("v{}: caixa SIMD exige vetor de mesmo tipo", v.0));
                }
                // A emissão separa os bits em palavras nativas; a fábrica
                // possui os mesmos efeitos independentemente das pistas.
                let c = runtime_call_contract(&Instruction::CallRuntime {
                    name: "dartforge_arc_box_simd_owned_v1".into(),
                    args: vec![(Operand::Constant(Constant::Int(0)), Type::I64); 3],
                    ret_ty: Type::Ref,
                })?;
                if classes.get(v).is_some_and(|class| *class != c.result)
                    || plan.instructions.get(v).is_some_and(|e| *e != c.effect) {
                    return Err(format!("v{}: contrato conflitante de caixa SIMD", v.0));
                }
                contracts.insert(*v, c);
            }
            if super::pure_set::check(*v, inst, ty, &types)? {
                let effect = TokenEffect::default();
                if classes.get(v).is_some_and(|c| *c != Ownership::Trivial)
                    || plan.instructions.get(v).is_some_and(|e| *e != effect)
                {
                    return Err(format!(
                        "v{}: contrato incompatível com constante/operação pura",
                        v.0
                    ));
                }
                fixed_set.insert(*v, Ownership::Trivial);
                pure_effects.insert(*v, effect);
            }
            let fixed = match inst {
                Instruction::ArcCopy { .. }
                | Instruction::ArcMove { .. }
                | Instruction::ArcLoadStrong { .. } => Some((Ownership::Owned, Type::Ref)),
                Instruction::ArcDrop { .. } | Instruction::ArcStoreStrong { .. } => {
                    Some((Ownership::Trivial, Type::Void))
                }
                _ => None,
            };
            if let Some((class, expected)) = fixed {
                if *ty != expected
                    || classes.get(v).is_some_and(|c| *c != class)
                    || plan.instructions.contains_key(v)
                {
                    return Err(format!("v{}: contrato incompatível com operação ARC", v.0));
                }
                fixed_set.insert(*v, class);
            }
        }
        if include_arc && (super::typed_set::check(f, inst, *ty)?
            || super::selectors::check_local_vector(f, inst, *ty, None)?) {
            let c = RuntimeCallContract {
                effect: TokenEffect { may_fail: true, ..Default::default() },
                result: if *ty == Type::Ref { Ownership::Owned } else { Ownership::Trivial }, persistent_retention: true,
                invalidates_borrows: true, calls_dart: true,
            };
            if classes.get(v).is_some_and(|class| *class != c.result)
                || plan.instructions.get(v).is_some_and(|e| *e != c.effect) {
                return Err(format!("v{}: contrato conflitante da chamada tipada ARC", v.0));
            }
            contracts.insert(*v, c);
        }
        // A chamada C com composto por valor: os compostos vão e voltam por
        // endereço (o destino), escalar como os demais argumentos.
        if include_arc && let Instruction::CompositeNativeCall { alvo, args, ret, dest, .. } = inst {
            let scalar = |op: &Operand| match op {
                Operand::Val(arg) => types.get(arg).is_some_and(|t| *t != Type::Ref && *t != Type::Void),
                Operand::Constant(Constant::Int(_) | Constant::Double(_) | Constant::Bool(_)) => true,
                _ => false,
            };
            let expected = match ret {
                crate::hir::NativeType::Prim(tc) => tc.hir_type(),
                _ => Type::Void,
            };
            if *ty != expected || !scalar(alvo) || !args.iter().all(|(op, _)| scalar(op))
                || dest.as_ref().is_some_and(|d| !scalar(d)) {
                return Err(format!("v{}: chamada nativa composta sem ABI escalar auditada", v.0));
            }
            let c = RuntimeCallContract {
                effect: TokenEffect::default(), result: Ownership::Trivial,
                persistent_retention: false, invalidates_borrows: true, calls_dart: true,
            };
            if classes.get(v).is_some_and(|class| *class != c.result)
                || plan.instructions.get(v).is_some_and(|e| *e != c.effect) {
                return Err(format!("v{}: contrato conflitante de chamada nativa composta", v.0));
            }
            contracts.insert(*v, c);
        }
        if include_arc && let Instruction::NativeCall { alvo, args, ret } = inst {
            // Só escalares atravessam a ABI C (a HIR converte Pointer em
            // endereço). Um callback Dart pode reentrar, mas a saída C limpa
            // a pendência (`dartforge_ffi_callback_sair`): nada fica pendente.
            let scalar = |op: &Operand| match op {
                Operand::Val(arg) => types.get(arg).is_some_and(|t| *t != Type::Ref && *t != Type::Void),
                Operand::Constant(Constant::Int(_) | Constant::Double(_) | Constant::Bool(_)) => true,
                _ => false,
            };
            if *ty != ret.hir_type() || *ty == Type::Ref || !scalar(alvo)
                || !args.iter().all(|(op, _)| scalar(op)) {
                return Err(format!("v{}: chamada nativa sem ABI escalar auditada", v.0));
            }
            let c = RuntimeCallContract {
                effect: TokenEffect::default(), result: Ownership::Trivial,
                persistent_retention: false, invalidates_borrows: true, calls_dart: true,
            };
            if classes.get(v).is_some_and(|class| *class != c.result)
                || plan.instructions.get(v).is_some_and(|e| *e != c.effect) {
                return Err(format!("v{}: contrato conflitante de chamada nativa", v.0));
            }
            contracts.insert(*v, c);
        }
        if include_arc && matches!(inst, Instruction::CallSelectorForward { arc_owned: true, .. } | Instruction::CallClosureForward { arc_owned: true, .. }) {
            let (recv, args, desc) = match inst {
                Instruction::CallSelectorForward { recv, args, desc, .. }
                | Instruction::CallClosureForward { closure: recv, args, desc, .. } => (recv, args, desc),
                _ => unreachable!(),
            };
            let compatible = |op: &Operand, t| match op {
                Operand::Val(v) => types.get(v) == Some(&t),
                Operand::Constant(Constant::Null) => t == Type::Ref,
                _ => false,
            };
            if *ty != Type::Ref || !compatible(recv, Type::Ref)
                || !compatible(args, Type::Ptr) || !compatible(desc, Type::Ptr) {
                return Err(format!("v{}: ABI do repasse ARC incompatível", v.0));
            }
            let c = RuntimeCallContract {
                effect: TokenEffect { may_fail: true, ..Default::default() },
                result: Ownership::Owned, persistent_retention: true,
                invalidates_borrows: true, calls_dart: true,
            };
            if classes.get(v).is_some_and(|class| *class != c.result)
                || plan.instructions.get(v).is_some_and(|e| *e != c.effect) {
                return Err(format!("v{}: contrato conflitante do repasse ARC", v.0));
            }
            contracts.insert(*v, c);
        }
        let unbox_bool = if include_arc {
            unbox_bool_call(inst)
        } else {
            None
        };
        let call_inst = unbox_bool.as_ref().unwrap_or(inst);
        if let Instruction::CallRuntime { ret_ty, args, .. } = call_inst {
            if if unbox_bool.is_some() {
                *ty != Type::I1
            } else {
                ty != ret_ty
            } {
                return Err(format!("v{}: tipo do resultado incompatível", v.0));
            }
            super::fields::check_call(f, call_inst, None)?;
            let c = runtime_call_contract(call_inst).map_err(|e| format!("v{}: {e}", v.0))?;
            for (n, (op, ty)) in args.iter().enumerate() {
                if let Operand::Val(arg) = op
                    && types.get(arg) != Some(ty)
                {
                    return Err(format!(
                        "v{}: argumento {n} aponta para SSA v{} ausente ou de tipo incompatível",
                        v.0, arg.0
                    ));
                }
            }
            if classes.get(v).is_some_and(|class| *class != c.result)
                || plan.instructions.get(v).is_some_and(|e| *e != c.effect)
            {
                return Err(format!(
                    "v{}: metadados conflitam com contrato runtime",
                    v.0
                ));
            }
            contracts.insert(*v, c);
        }
    }
    let mut pendings = HashMap::new();
    for b in &f.blocks {
        if let Terminator::CondBranch { then_block, .. } = &b.terminator
            && let Some((call, _, _)) = b
                .instructions
                .len()
                .checked_sub(3)
                .map(|i| &b.instructions[i])
            && contracts.get(call).is_some_and(|c| c.effect.may_fail)
            && super::classification::check_pending(b, *call, *then_block).is_some()
        {
            if plan.pendings.get(call).is_some_and(|p| p != then_block) {
                return Err(format!("v{}: saída pending conflita com CFG", call.0));
            }
            pendings.insert(*call, *then_block);
        }
    }
    if include_arc {
        // Os Phis ainda não têm classe aqui: `check_typed_scalars`
        // os confere depois de `produce_phi`.
        let phis: HashSet<ValueId> = f.blocks.iter().flat_map(|b| &b.instructions)
            .filter(|(_, i, _)| matches!(i, Instruction::Phi { .. })).map(|(v, _, _)| *v).collect();
        for (_, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if let Instruction::TypedCall { args, .. } = inst {
                for (op, t) in args {
                    if *t != Type::Ref && let Operand::Val(v) = op && !phis.contains(v) {
                        let class = contracts.get(v).map(|c| &c.result)
                            .or_else(|| fixed_set.get(v)).or_else(|| classes.get(v));
                        if class != Some(&Ownership::Trivial) {
                            return Err(format!("v{}: argumento escalar tipado exige Trivial, sem word Dart", v.0));
                        }
                    }
                }
            }
        }
    }
    classes.extend(fixed_set);
    plan.pendings.extend(pendings);
    plan.instructions.extend(pure_effects);
    for (v, c) in &contracts {
        classes.insert(*v, c.result);
        plan.instructions.insert(*v, c.effect.clone());
    }
    Ok(contracts)
}

/// Contrato semântico de uma chamada ordinária auditada.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::contrato_chamada_runtime};
/// let i = Instruction::CallRuntime { name: "dartforge_arc_collect".into(), args: vec![], ret_ty: Type::Void };
/// let c = contrato_chamada_runtime(&i)?;
/// assert!(c.invalida_borrows && c.efeito.sempre.is_empty());
/// # Ok::<(), String>(())
/// ```
#[derive(Debug)]
pub struct RuntimeCallContract {
    /// Consumos de argumentos SSA, preservando multiplicidade.
    pub effect: TokenEffect,
    /// Classe do resultado, proveniente da extern e não da largura i64.
    pub result: Ownership,
    /// Exige considerar owners persistentes internos ao runtime.
    pub persistent_retention: bool,
    /// Exige prova separada das dependências borrowed após a chamada.
    pub invalidates_borrows: bool,
    /// Reentrada Dart auditada, inclusive construção de TypeError pelo SDK.
    pub calls_dart: bool,
}

/// Traduz uma chamada auditada para efeitos de consumo e classe do resultado.
///
/// Confere aridade, tipos declarados e constantes: I64 exige inteiro, ou
/// endereço de função quando o parâmetro é nativo. O verificador HIR continua
/// responsável por SSA, dominância e tipos reais dos operandos. Não insere RC,
/// não certifica owners de slots e não resolve invalidação de empréstimos.
/// Null não tem token físico; outras referências devem estar avaliadas em SSA.
///
/// # Erros
/// Instrução não runtime, extern ausente, assinatura incompatível, referência
/// não avaliada ou retain direto, que cria token sem resultado SSA explícito.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::{contrato_chamada_runtime, Ownership}};
/// let i = Instruction::CallRuntime { name: "dartforge_arc_quadro_carregar_v1".into(),
///     args: vec![(Operand::Val(ValueId(0)), Type::I64), (Operand::Constant(Constant::Int(0)), Type::I64)],
///     ret_ty: Type::Ref };
/// assert_eq!(contrato_chamada_runtime(&i)?.resultado, Ownership::Owned);
/// # Ok::<(), String>(())
/// ```
pub fn runtime_call_contract(inst: &Instruction) -> Result<RuntimeCallContract, String> {
    let Instruction::CallRuntime { name, args, ret_ty } = inst else {
        return Err("contrato runtime exige CallRuntime".into());
    };
    if let Some(base) = name.strip_prefix(super::NATIVE_SCOPE_PREFIX) {
        return runtime_call_contract(&Instruction::CallRuntime {
            name: base.to_string(),
            args: args.clone(),
            ret_ty: *ret_ty,
        });
    }
    if let Some(base) = name.strip_prefix(super::GENERIC_OWNED_PREFIX) {
        if *ret_ty != Type::Ref {
            return Err(format!("{name}: thunk Owned exige resultado Ref"));
        }
        return generic_owned_contract(base, args);
    }
    let c = contract(name).map_err(|e| format!("{name}: {e}"))?;
    // Retain produz um owner independente, mas a extern retorna void. A HIR
    // representa essa produção com ArcCopy; tratá-la como chamada borrowed
    // comum permitiria perder o token sem que o verificador percebesse.
    if name == "dartforge_arc_retain" {
        return Err("retain direto exige ArcCopy com resultado SSA owned".into());
    }
    if c.parametros.len() != args.len() {
        return Err(format!("{name}: aridade incompatível com ownership.tsv"));
    }
    let (ty, result) = match c.resultado {
        ModoResultado::Owned => (Type::Ref, Ownership::Owned),
        ModoResultado::BorrowArg(n) => (
            Type::Ref,
            match args[n].0 {
                Operand::Val(v) => Ownership::Borrowed {
                    owner: OwnerOrigin::Value(v),
                    scope: 0,
                },
                Operand::Constant(Constant::Null) => Ownership::Trivial,
                _ => {
                    return Err(format!(
                        "{name}: owner do resultado borrowed exige SSA ou null"
                    ));
                }
            },
        ),
        ModoResultado::ScalarI64 => (Type::I64, Ownership::Trivial),
        ModoResultado::ScalarF64 => (Type::F64, Ownership::Trivial),
        ModoResultado::ScalarI8 => (Type::I8, Ownership::Trivial),
        ModoResultado::ScalarI1 => (Type::I1, Ownership::Trivial),
        ModoResultado::Void => (Type::Void, Ownership::Trivial),
    };
    if *ret_ty != ty {
        return Err(format!(
            "{name}: resultado incompatível com contrato semântico"
        ));
    }
    let mut effect = TokenEffect {
        may_fail: c.pode_falhar,
        ..Default::default()
    };
    for (n, (mode, (op, ty))) in c.parametros.iter().zip(args).enumerate() {
        if *mode == ModoParametro::TabelaEstatica {
            // A seleção LLVM usa apenas o thunk registrado do módulo. O
            // produtor genérico não tem essa proveniência: um Ptr ou endereço
            // de função arbitrário não prova ausência de Dart/GC/exceções.
            return Err(format!("{name}: tabela estática exige proveniência do emissor"));
        }
        let reference = matches!(
            mode,
            ModoParametro::Borrow
                | ModoParametro::Consume
                | ModoParametro::ConsumeSuccess
                | ModoParametro::ConsumeError
        );
        let expected = if reference {
            Type::Ref
        } else if matches!(mode, ModoParametro::VetorEstatico | ModoParametro::EnderecoNative) {
            Type::Ptr
        } else if *mode == ModoParametro::ScalarF64 {
            Type::F64
        } else if *mode == ModoParametro::ScalarI8 {
            Type::I8
        } else {
            Type::I64
        };
        if *ty != expected || (matches!(mode, ModoParametro::VetorEstatico | ModoParametro::EnderecoNative) && !matches!(op, Operand::Val(_))) {
            return Err(format!("{name}: tipo do argumento {n} incompatível"));
        }
        if reference {
            match op {
                Operand::Val(v) => {
                    if *mode == ModoParametro::Consume {
                        effect.always.push(*v);
                    }
                    if *mode == ModoParametro::ConsumeSuccess {
                        effect.success.push(*v);
                    }
                    if *mode == ModoParametro::ConsumeError {
                        effect.error.push(*v);
                    }
                }
                Operand::Constant(Constant::Null) => {}
                _ => return Err(format!("{name}: argumento Ref {n} exige SSA ou null")),
            }
        } else if let Operand::Constant(k) = op {
            // Um endereço de função só tem significado em parâmetro nativo;
            // a anotação I64 não transforma bool/double/literal em inteiro.
            let compatible = if *mode == ModoParametro::ScalarF64 {
                matches!(k, Constant::Double(_))
            } else if *mode == ModoParametro::ScalarI8 {
                matches!(k, Constant::Int(0..=255) | Constant::Bool(_))
            } else {
                matches!(k, Constant::Int(_))
                    || (*mode == ModoParametro::Native && matches!(k, Constant::Function(_)))
            };
            if !compatible {
                return Err(format!(
                    "{name}: constante do argumento {n} incompatível com seu contrato"
                ));
            }
        }
    }
    Ok(RuntimeCallContract {
        effect,
        result,
        persistent_retention: c.retencao_persistente,
        invalidates_borrows: c.invalida_borrows,
        calls_dart: c.chama_dart,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers_agree_with_effects() {
        let mut names = HashSet::new();
        for c in HELPER_CONTRACTS {
            assert!(names.insert(c.nome), "{} repetido", c.nome);
            assert!(c.nome.starts_with("df."), "{}", c.nome);
            assert!(dartforge_runtime::ownership::contrato(c.nome).is_err(), "{} também no runtime", c.nome);
            // O contrato segue as marcas do ajudante: falha se lança, invalida
            // empréstimos se aloca, nunca retém.
            let e = crate::llvm::externs::effects_of(c.nome);
            assert_eq!((c.pode_falhar, c.chama_dart), (e.throws || e.calls_dart, e.calls_dart), "{}: {e:?}", c.nome);
            assert!(!e.allocates || c.invalida_borrows, "{}: {e:?}", c.nome);
            assert!(!c.retencao_persistente, "{}", c.nome);
            assert!(c.parametros.iter().all(|m| matches!(m, ModoParametro::Borrow | ModoParametro::Scalar)));
        }
        // A leitura Ref do armazenamento e o que aloca ficam sem contrato.
        for name_str in ["df.palavra_ref", "df.texto_alocar"] {
            assert!(contract(name_str).is_err(), "{name_str}");
        }
        let rt = |name: &str, args, ret_ty| Instruction::CallRuntime { name: name.into(), args, ret_ty };
        let text = rt("df.texto_igual_a", vec![(Operand::Val(ValueId(1)), Type::Ref), (Operand::Constant(Constant::Null), Type::Ref)], Type::I1);
        let c = runtime_call_contract(&text).unwrap();
        assert_eq!(c.result, Ownership::Trivial);
        assert!(!c.effect.may_fail && !c.invalidates_borrows);
        let base = rt("df.tipada_base", vec![(Operand::Val(ValueId(1)), Type::Ref)], Type::Ref);
        assert_eq!(runtime_call_contract(&base).unwrap().result,
            Ownership::Borrowed { owner: OwnerOrigin::Value(ValueId(1)), scope: 0 });
        // A largura errada do resultado é recusada.
        let wrong = rt("df.texto_len", vec![(Operand::Val(ValueId(1)), Type::Ref)], Type::I8);
        assert!(runtime_call_contract(&wrong).is_err());
    }

    #[test]
    fn unbox_borrows_reference_and_returns_fallible_scalar_without_consumption() {
        for (name, ret_ty) in [
            ("dartforge_unbox_int", Type::I64),
            ("dartforge_unbox_double", Type::F64),
            ("dartforge_unbox_bool", Type::I8),
        ] {
            let mut inst = Instruction::CallRuntime {
                name: name.into(),
                args: vec![(Operand::Val(ValueId(0)), Type::Ref)],
                ret_ty,
            };
            let c = runtime_call_contract(&inst).unwrap();
            assert_eq!(c.result, Ownership::Trivial);
            assert!(c.effect.may_fail && c.invalidates_borrows && c.calls_dart);
            assert!(!c.persistent_retention);
            assert!(
                c.effect.always.is_empty()
                    && c.effect.success.is_empty()
                    && c.effect.error.is_empty()
            );
            if let Instruction::CallRuntime { ret_ty: r, .. } = &mut inst {
                *r = Type::Ref;
            }
            assert!(runtime_call_contract(&inst).is_err());
            if let Instruction::CallRuntime {
                args, ret_ty: r, ..
            } = &mut inst
            {
                *r = ret_ty;
                args[0].1 = Type::I64;
            }
            assert!(runtime_call_contract(&inst).is_err());
        }
    }

    #[test]
    fn throwing_exit_releases_result_and_guard_requires_cfg_without_publishing() {
        let original = Function {
            symbol: "desenrolar".into(),
            name: "desenrolar".into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(1),
                    Instruction::ArcCopy {
                        value: Operand::Val(ValueId(0)),
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
            }],
        };
        let mut f = original.clone();
        let mut classes = HashMap::new();
        let mut plan = TokenPlan {
            ret: super::super::TokenReturn::Owned,
            ..Default::default()
        };
        let mut tables = FunctionTables::default();
        tables.exits.insert(BlockId(0), ExceptionExit::Throws);
        assert_eq!(
            insert_arc_dart_exits(
                &mut f,
                &mut classes,
                &mut plan,
                &tables,
                &ScopePlan::default()
            )
            .unwrap(),
            (0, 1)
        );
        assert!(matches!(
            f.blocks[0].terminator,
            Terminator::Return(Some(Operand::Constant(Constant::Null)))
        ));
        assert!(matches!(
            f.blocks[0].instructions[1].1,
            Instruction::ArcDrop {
                value: Operand::Val(ValueId(1))
            }
        ));
        let mut module = Module::new();
        module.arc_memory = true;
        module.excecoes_por_tabelas = true;
        module.functions.push(f.clone());
        module.tables.push(tables.clone());
        let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
        let function = ir
            .split("@desenrolar(")
            .nth(1)
            .unwrap()
            .split("\n}")
            .next()
            .unwrap();
        let release = function
            .find("call void @dartforge_arc_release(i64 %v1)")
            .unwrap();
        // O primeiro lançamento pode ser o guard do prólogo, antes de criar
        // qualquer token; o último é a saída Lanca do corpo preparado.
        let unwind = function.rfind("call void @df.lancar()").unwrap();
        assert!(release < unwind);
        assert_eq!(
            insert_arc_dart_exits(
                &mut f,
                &mut classes,
                &mut plan,
                &tables,
                &ScopePlan::default()
            )
            .unwrap(),
            (0, 0)
        );
        f = original;
        classes.clear();
        plan.instructions.clear();
        assert!(
            produce_and_verify_dart_tokens(
                &f,
                &mut classes,
                &mut plan,
                &tables,
                &ScopePlan::default()
            )
            .unwrap_err()
            .contains("canônico")
        );
        assert!(classes.is_empty() && plan.instructions.is_empty());
        tables.exits.insert(BlockId(0), ExceptionExit::Guard);
        let before = format!("{f:?}");
        assert!(
            insert_arc_dart_exits(
                &mut f,
                &mut classes,
                &mut plan,
                &tables,
                &ScopePlan::default()
            )
            .unwrap_err()
            .contains("CFG explícito")
        );
        assert_eq!(format!("{f:?}"), before);
        assert!(classes.is_empty() && plan.instructions.is_empty());
    }

    #[test]
    fn cleanup_preserves_transfer_and_retains_before_releasing_other_tokens() {
        let mut f = Function {
            symbol: "retornos_distintos".into(),
            name: "retornos_distintos".into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (
                            ValueId(1),
                            Instruction::ArcCopy {
                                value: Operand::Val(ValueId(0)),
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(2),
                            Instruction::ArcCopy {
                                value: Operand::Val(ValueId(0)),
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(3),
                            Instruction::Const(Constant::Bool(true)),
                            Type::I1,
                        ),
                    ],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(3)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
                },
            ],
        };
        let mut classes = HashMap::new();
        let mut plan = TokenPlan {
            ret: super::super::TokenReturn::Owned,
            ..Default::default()
        };
        assert_eq!(
            insert_arc_dart_exits(
                &mut f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .unwrap(),
            (1, 3)
        );
        // Transferir v1 exige liberar somente v2 nesta saída.
        assert!(matches!(
            f.blocks[1].terminator,
            Terminator::Return(Some(Operand::Val(ValueId(1))))
        ));
        assert_eq!(f.blocks[1].instructions.len(), 1);
        assert!(matches!(
            f.blocks[1].instructions[0].1,
            Instruction::ArcDrop {
                value: Operand::Val(ValueId(2))
            }
        ));
        // Na outra saída, reter o retorno precede o cleanup dos dois owners.
        let b = &f.blocks[2];
        assert_eq!(b.instructions.len(), 3);
        assert!(matches!(
            b.instructions[0].1,
            Instruction::ArcCopy {
                value: Operand::Val(ValueId(0))
            }
        ));
        assert!(
            matches!(b.terminator, Terminator::Return(Some(Operand::Val(v))) if v == b.instructions[0].0)
        );
        for (inst, expected) in b.instructions[1..].iter().zip([ValueId(1), ValueId(2)]) {
            assert!(
                matches!(inst.1, Instruction::ArcDrop { value: Operand::Val(v) } if v == expected)
            );
        }
        let ready = format!("{f:?}");
        assert_eq!(
            insert_arc_dart_exits(
                &mut f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{f:?}"), ready);
    }

    #[test]
    fn cleanup_exits_cover_runtime_error_without_repairing_double_consumption() {
        let original = Function {
            symbol: "cleanup_pending".into(),
            name: "cleanup_pending".into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Void,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (
                            ValueId(1),
                            Instruction::ArcCopy {
                                value: Operand::Val(ValueId(0)),
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(2),
                            Instruction::CallRuntime {
                                name: "dartforge_gc_collect".into(),
                                args: vec![],
                                ret_ty: Type::Void,
                            },
                            Type::Void,
                        ),
                        (
                            ValueId(3),
                            Instruction::CallRuntime {
                                name: "dartforge_exception_pending".into(),
                                args: vec![],
                                ret_ty: Type::I8,
                            },
                            Type::I8,
                        ),
                        (
                            ValueId(4),
                            Instruction::ICmp(
                                ICmpOp::Ne,
                                Operand::Val(ValueId(3)),
                                Operand::Constant(Constant::Int(0)),
                            ),
                            Type::I1,
                        ),
                    ],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(4)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                },
            ],
        };
        let mut f = original.clone();
        let mut classes = HashMap::new();
        let mut plan = TokenPlan::default();
        assert_eq!(
            insert_arc_dart_exits(
                &mut f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .unwrap(),
            (0, 2)
        );
        for b in &f.blocks[1..] {
            assert!(matches!(
                b.instructions[0].1,
                Instruction::ArcDrop {
                    value: Operand::Val(ValueId(1))
                }
            ));
        }
        let ready = format!("{f:?}");
        assert_eq!(
            insert_arc_dart_exits(
                &mut f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{f:?}"), ready);
        f = original;
        for id in [99, 100] {
            f.blocks[0].instructions.insert(
                1,
                (
                    ValueId(id),
                    Instruction::ArcDrop {
                        value: Operand::Val(ValueId(1)),
                    },
                    Type::Void,
                ),
            );
        }
        classes.clear();
        plan = TokenPlan::default();
        let before = format!("{f:?}");
        assert!(
            insert_arc_dart_exits(
                &mut f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .unwrap_err()
            .contains("indisponível")
        );
        assert_eq!(format!("{f:?}"), before);
        assert!(classes.is_empty() && plan.instructions.is_empty() && plan.pendings.is_empty());
    }

    #[test]
    fn dart_return_insertion_is_idempotent_and_atomic_if_cleanup_missing() {
        let original = Function {
            symbol: "identidade".into(),
            name: "identidade".into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        };
        let mut f = original.clone();
        let mut classes = HashMap::new();
        let mut plan = TokenPlan {
            ret: super::super::TokenReturn::Owned,
            ..Default::default()
        };
        assert_eq!(
            insert_dart_return_retention(
                &mut f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .unwrap(),
            1
        );
        assert!(matches!(
            f.blocks[0].instructions[0].1,
            Instruction::ArcCopy {
                value: Operand::Val(ValueId(0))
            }
        ));
        let ready = format!("{f:?}");
        assert_eq!(
            insert_dart_return_retention(
                &mut f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .unwrap(),
            0
        );
        assert_eq!(format!("{f:?}"), ready);
        f = original.clone();
        f.blocks[0].instructions.push((
            ValueId(5),
            Instruction::ArcCopy {
                value: Operand::Val(ValueId(0)),
            },
            Type::Ref,
        ));
        classes.clear();
        plan = TokenPlan {
            ret: super::super::TokenReturn::Owned,
            ..Default::default()
        };
        let before = format!("{f:?}");
        assert!(
            insert_dart_return_retention(
                &mut f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .unwrap_err()
            .contains("não consumidos")
        );
        assert_eq!(format!("{f:?}"), before);
        assert!(classes.is_empty());
        assert!(plan.instructions.is_empty());
        for constant in [
            Constant::Null,
            Constant::String("retorno permanente".into()),
        ] {
            f = original.clone();
            f.blocks[0].terminator = Terminator::Return(Some(Operand::Constant(constant.clone())));
            classes.clear();
            plan = TokenPlan {
                ret: super::super::TokenReturn::Owned,
                ..Default::default()
            };
            let quantity = insert_dart_return_retention(
                &mut f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default(),
            )
            .unwrap();
            assert_eq!(
                quantity,
                usize::from(!matches!(constant, Constant::Null))
            );
        }
    }

    #[test]
    fn dart_wrapper_does_not_publish_parameters_when_transfer_fails() {
        let mut f = Function {
            symbol: "identidade".into(),
            name: "identidade".into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(1),
                    Instruction::ArcMove {
                        value: Operand::Val(ValueId(0)),
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
            }],
        };
        let mut classes = HashMap::new();
        let mut plan = TokenPlan {
            ret: super::super::TokenReturn::Owned,
            ..Default::default()
        };
        assert!(
            produce_and_verify_dart_tokens(
                &f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .is_err()
        );
        assert!(classes.is_empty());
        assert!(plan.instructions.is_empty());
        f.blocks[0].instructions[0].1 = Instruction::ArcCopy {
            value: Operand::Val(ValueId(0)),
        };
        produce_and_verify_dart_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        assert_eq!(
            classes[&ValueId(0)],
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                scope: 0
            }
        );
        assert_eq!(classes[&ValueId(1)], Ownership::Owned);
        let before = classes.clone();
        plan.ret = super::super::TokenReturn::Borrowed;
        assert!(
            produce_and_verify_dart_tokens(
                &f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .is_err()
        );
        assert_eq!(classes, before);
        assert_eq!(plan.ret, super::super::TokenReturn::Borrowed);
    }

    #[test]
    fn dart_parameters_do_not_classify_i64_and_reject_conflict_atomically() {
        let mut f = Function {
            symbol: "parametros".into(),
            name: "parametros".into(),
            debug_info: None,
            params: vec![
                (ValueId(0), "ref".into(), Type::Ref),
                (ValueId(1), "bits".into(), Type::I64),
                (ValueId(2), "outra_ref".into(), Type::Ref),
            ],
            return_ty: Type::Void,
            blocks: vec![],
        };
        let mut classes = HashMap::from([(ValueId(2), Ownership::Owned)]);
        let before = classes.clone();
        assert!(produce_dart_ref_parameters(&f, &mut classes).is_err());
        assert_eq!(classes, before);
        classes.clear();
        produce_dart_ref_parameters(&f, &mut classes).unwrap();
        assert!(!classes.contains_key(&ValueId(1)));
        assert_eq!(classes.len(), 2);
        let before = classes.clone();
        f.params.push((ValueId(0), "duplicado".into(), Type::Ref));
        assert!(produce_dart_ref_parameters(&f, &mut classes).is_err());
        assert_eq!(classes, before);
    }

    #[test]
    fn trivial_ref_phi_producer_respects_explicit_owned_transfer() {
        let mut f = Function {
            symbol: "phi_trivial_auto".into(),
            name: "phi_trivial_auto".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(1)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::Phi {
                            ty: Type::Ref,
                            incoming: vec![
                                (BlockId(0), Operand::Constant(Constant::Null)),
                                (BlockId(1), Operand::Val(ValueId(1))),
                            ],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Constant(Constant::Bool(false)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                },
            ],
        };
        for ret in [
            super::super::TokenReturn::Trivial,
            super::super::TokenReturn::Owned,
        ] {
            let mut classes = HashMap::new();
            let mut plan = TokenPlan {
                ret,
                ..Default::default()
            };
            produce_and_verify_tokens(
                &f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default(),
            )
            .unwrap();
            assert_eq!(
                classes[&ValueId(1)],
                if ret == super::super::TokenReturn::Owned {
                    Ownership::Owned
                } else {
                    Ownership::Trivial
                }
            );
        }
        f.blocks[0].instructions.push((
            ValueId(0),
            Instruction::Const(Constant::String("permanente".into())),
            Type::Ref,
        ));
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
            incoming[0].1 = Operand::Val(ValueId(0));
        }
        let mut classes = HashMap::new();
        let mut plan = TokenPlan::default();
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        assert_eq!(classes[&ValueId(1)], Ownership::Trivial);
    }

    #[test]
    fn producer_checks_pending_dependency_in_borrowed_phi_cycle() {
        let mut f = Function {
            symbol: "ciclo_borrowed".into(),
            name: "ciclo_borrowed".into(),
            debug_info: None,
            params: vec![(ValueId(0), "entrada".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(1)),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![(
                        ValueId(2),
                        Instruction::Phi {
                            ty: Type::Ref,
                            incoming: vec![(BlockId(1), Operand::Val(ValueId(1)))],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Branch(BlockId(1)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::Phi {
                            ty: Type::Ref,
                            incoming: vec![
                                (BlockId(0), Operand::Val(ValueId(0))),
                                (BlockId(2), Operand::Val(ValueId(2))),
                            ],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Constant(Constant::Bool(false)),
                        then_block: BlockId(2),
                        else_block: BlockId(3),
                    },
                },
                BasicBlock {
                    id: BlockId(3),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                },
            ],
        };
        let initial = HashMap::from([(
            ValueId(0),
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                scope: 0,
            },
        )]);
        let mut classes = initial.clone();
        let mut plan = TokenPlan {
            ret: super::super::TokenReturn::Borrowed,
            ..Default::default()
        };
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        for v in [ValueId(1), ValueId(2)] {
            assert_eq!(
                classes[&v],
                Ownership::Borrowed {
                    owner: OwnerOrigin::Value(ValueId(0)),
                    scope: 0
                }
            );
        }
        // O primeiro Phi pode receber provisoriamente a âncora, mas sua outra
        // entrada não pode terminar em um Phi que só tem fonte Owned.
        f.blocks[0].instructions.push((
            ValueId(5),
            Instruction::ArcCopy {
                value: Operand::Val(ValueId(0)),
            },
            Type::Ref,
        ));
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
            incoming[0].1 = Operand::Val(ValueId(5));
        }
        classes = initial.clone();
        assert!(
            produce_arc_contracts(&f, &mut classes, &mut plan)
                .unwrap_err()
                .contains("após propagação")
        );
        assert_eq!(classes, initial);
        assert!(plan.instructions.is_empty());
    }

    #[test]
    fn producer_preserves_borrow_in_anchored_loop_phi() {
        let mut f = Function {
            symbol: "phi_borrowed_laco".into(),
            name: "phi_borrowed_laco".into(),
            debug_info: None,
            params: vec![(ValueId(0), "entrada".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(1)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::Phi {
                            ty: Type::Ref,
                            incoming: vec![
                                (BlockId(0), Operand::Val(ValueId(0))),
                                (BlockId(1), Operand::Val(ValueId(1))),
                            ],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Constant(Constant::Bool(false)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                },
            ],
        };
        let mut classes = HashMap::from([(
            ValueId(0),
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                scope: 0,
            },
        )]);
        let mut plan = TokenPlan {
            ret: super::super::TokenReturn::Borrowed,
            ..Default::default()
        };
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        assert_eq!(
            classes[&ValueId(1)],
            Ownership::Borrowed {
                owner: OwnerOrigin::Value(ValueId(0)),
                scope: 0
            }
        );
        classes.remove(&ValueId(1));
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
            incoming[0].1 = Operand::Val(ValueId(1));
        }
        let before = classes.clone();
        assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
        assert_eq!(classes, before);
        assert!(plan.instructions.is_empty());
    }

    #[test]
    fn producer_preserves_borrow_in_phi_chain_without_manual_class() {
        let f = Function {
            symbol: "phis_borrowed".into(),
            name: "phis_borrowed".into(),
            debug_info: None,
            params: vec![(ValueId(0), "entrada".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(1)),
                },
                // Ordem física inversa exige mais de uma rodada de propagação.
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![(
                        ValueId(2),
                        Instruction::Phi {
                            ty: Type::Ref,
                            incoming: vec![(BlockId(1), Operand::Val(ValueId(1)))],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::Phi {
                            ty: Type::Ref,
                            incoming: vec![(BlockId(0), Operand::Val(ValueId(0)))],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Branch(BlockId(2)),
                },
            ],
        };
        let mut classes = HashMap::from([(
            ValueId(0),
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                scope: 0,
            },
        )]);
        let mut plan = TokenPlan {
            ret: super::super::TokenReturn::Borrowed,
            ..Default::default()
        };
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        for v in [ValueId(1), ValueId(2)] {
            assert_eq!(
                classes[&v],
                Ownership::Borrowed {
                    owner: OwnerOrigin::Value(ValueId(0)),
                    scope: 0
                }
            );
        }
        assert!(plan.instructions.is_empty());
    }

    #[test]
    fn trivial_ref_phi_does_not_erase_entry_ownership() {
        let mut f = Function {
            symbol: "phi_ref_trivial".into(),
            name: "phi_ref_trivial".into(),
            debug_info: None,
            params: vec![(ValueId(0), "entrada".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(1)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::Phi {
                            ty: Type::Ref,
                            incoming: vec![(BlockId(0), Operand::Val(ValueId(0)))],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                },
            ],
        };
        for class in [
            Ownership::Owned,
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                scope: 0,
            },
        ] {
            let mut classes =
                HashMap::from([(ValueId(0), class), (ValueId(1), Ownership::Trivial)]);
            let before = classes.clone();
            let mut plan = TokenPlan::default();
            assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
            assert_eq!(classes, before);
            assert!(plan.instructions.is_empty());
        }
        let mut classes = HashMap::from([
            (ValueId(0), Ownership::Trivial),
            (ValueId(1), Ownership::Trivial),
        ]);
        let mut plan = TokenPlan::default();
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
            incoming[0].1 = Operand::Constant(Constant::Null);
        }
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
    }

    #[test]
    fn fallible_query_produces_scalar_only_on_normal_exit() {
        // As consultas de record não falham; o endereço da closure sim.
        for name_str in ["dartforge_ffi_endereco_da_closure"] {
            let mut f = Function {
                symbol: "consulta_record".into(),
                name: "consulta_record".into(),
                debug_info: None,
                params: vec![(ValueId(0), "record".into(), Type::Ref)],
                return_ty: Type::I64,
                blocks: vec![
                    BasicBlock {
                        id: BlockId(0),
                        instructions: vec![
                            (
                                ValueId(1),
                                Instruction::CallRuntime {
                                    name: name_str.into(),
                                    args: vec![(Operand::Val(ValueId(0)), Type::Ref)],
                                    ret_ty: Type::I64,
                                },
                                Type::I64,
                            ),
                            (
                                ValueId(2),
                                Instruction::CallRuntime {
                                    name: "dartforge_exception_pending".into(),
                                    args: vec![],
                                    ret_ty: Type::I8,
                                },
                                Type::I8,
                            ),
                            (
                                ValueId(3),
                                Instruction::ICmp(
                                    ICmpOp::Ne,
                                    Operand::Val(ValueId(2)),
                                    Operand::Constant(Constant::Int(0)),
                                ),
                                Type::I1,
                            ),
                        ],
                        terminator: Terminator::CondBranch {
                            cond: Operand::Val(ValueId(3)),
                            then_block: BlockId(1),
                            else_block: BlockId(2),
                        },
                    },
                    BasicBlock {
                        id: BlockId(1),
                        instructions: vec![],
                        terminator: Terminator::Return(Some(Operand::Constant(Constant::Int(0)))),
                    },
                    BasicBlock {
                        id: BlockId(2),
                        instructions: vec![],
                        terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                    },
                ],
            };
            let mut classes = HashMap::from([(
                ValueId(0),
                Ownership::Borrowed {
                    owner: OwnerOrigin::Caller,
                    scope: 0,
                },
            )]);
            let mut plan = TokenPlan::default();
            produce_and_verify_tokens(
                &f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default(),
            )
            .unwrap();
            assert_eq!(classes[&ValueId(1)], Ownership::Trivial);
            assert_eq!(plan.pendings[&ValueId(1)], BlockId(1));
            assert!(plan.instructions[&ValueId(1)].always.is_empty());
            let before = classes.clone();
            let effects = plan.instructions.clone();
            let pendings = plan.pendings.clone();
            f.blocks[1].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
            assert!(
                produce_and_verify_tokens(
                    &f,
                    &mut classes,
                    &mut plan,
                    &FunctionTables::default(),
                    &ScopePlan::default()
                )
                .is_err()
            );
            assert_eq!(classes, before);
            assert_eq!(plan.instructions, effects);
            assert_eq!(plan.pendings, pendings);
        }
    }

    #[test]
    fn typed_arithmetic_produces_plan_without_seeds_and_rejects_ref() {
        let mut f = Function {
            symbol: "aritmetica".into(),
            name: "aritmetica".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::F64,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (ValueId(0), Instruction::Const(Constant::Int(7)), Type::I64),
                    (
                        ValueId(1),
                        Instruction::Mul(
                            Operand::Val(ValueId(0)),
                            Operand::Constant(Constant::Int(6)),
                        ),
                        Type::I64,
                    ),
                    (
                        ValueId(2),
                        Instruction::IntToDouble(Operand::Val(ValueId(1))),
                        Type::F64,
                    ),
                    (
                        ValueId(3),
                        Instruction::FDiv(
                            Operand::Val(ValueId(2)),
                            Operand::Constant(Constant::Double(2.0)),
                        ),
                        Type::F64,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
            }],
        };
        let mut classes = HashMap::new();
        let mut plan = TokenPlan::default();
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        assert_eq!(classes.len(), 4);
        assert!(classes.values().all(|c| *c == Ownership::Trivial));
        let before = classes.clone();
        let effects = plan.instructions.clone();
        f.blocks[0].instructions[1].1 = Instruction::Mul(
            Operand::Constant(Constant::Null),
            Operand::Constant(Constant::Int(6)),
        );
        assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
        assert_eq!(classes, before);
        assert_eq!(plan.instructions, effects);
    }

    #[test]
    fn ref_phi_does_not_accept_scalar_even_with_owned_class() {
        let f = Function {
            symbol: "phi_tipo".into(),
            name: "phi_tipo".into(),
            debug_info: None,
            params: vec![(ValueId(0), "bits".into(), Type::I64)],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(1)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::Phi {
                            ty: Type::Ref,
                            incoming: vec![(BlockId(0), Operand::Val(ValueId(0)))],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                },
            ],
        };
        let mut classes = HashMap::from([(ValueId(0), Ownership::Owned)]);
        let before = classes.clone();
        let mut plan = TokenPlan::default();
        assert!(
            produce_arc_contracts(&f, &mut classes, &mut plan)
                .unwrap_err()
                .contains("representação Ref")
        );
        assert_eq!(classes, before);
        assert!(plan.instructions.is_empty());
        classes.insert(ValueId(0), Ownership::Trivial);
        classes.insert(ValueId(1), Ownership::Trivial);
        let before = classes.clone();
        assert!(
            produce_and_verify_tokens(
                &f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default()
            )
            .unwrap_err()
            .contains("SSA Ref ou null")
        );
        assert_eq!(classes, before);
    }

    #[test]
    fn scalar_bitcast_requires_origin_and_is_block_order_independent() {
        let mut f = Function {
            symbol: "bits".into(),
            name: "bits".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::F64,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(2)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(2),
                        Instruction::Bitcast {
                            op: Operand::Val(ValueId(1)),
                            to: Type::F64,
                        },
                        Type::F64,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![
                        (
                            ValueId(0),
                            Instruction::Const(Constant::Double(-0.0)),
                            Type::F64,
                        ),
                        (
                            ValueId(1),
                            Instruction::Bitcast {
                                op: Operand::Val(ValueId(0)),
                                to: Type::I64,
                            },
                            Type::I64,
                        ),
                    ],
                    terminator: Terminator::Branch(BlockId(1)),
                },
            ],
        };
        super::super::ssa::verify(&f).unwrap();
        let mut classes = HashMap::new();
        let mut plan = TokenPlan::default();
        produce_arc_contracts(&f, &mut classes, &mut plan).unwrap();
        for id in [ValueId(0), ValueId(1), ValueId(2)] {
            assert_eq!(classes[&id], Ownership::Trivial);
            assert_eq!(plan.instructions[&id], TokenEffect::default());
        }
        let mut module = Module::default();
        module.arc_memory = true;
        module.functions.push(f.clone());
        let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
        assert!(ir.contains("bitcast double %v0 to i64"));
        assert!(ir.contains("bitcast i64 %v1 to double"));
        // O sucesso escalar anterior não autoriza a reinterpretar um handle
        // nem a confiar numa representação cujo produtor não deu contrato.
        f.blocks.remove(2);
        f.blocks[0].terminator = Terminator::Branch(BlockId(1));
        f.blocks[1].instructions.remove(0);
        f.blocks[1].instructions.push((
            ValueId(2),
            Instruction::Bitcast {
                op: Operand::Val(ValueId(0)),
                to: Type::F64,
            },
            Type::F64,
        ));
        for (ty, class) in [
            (Type::Ref, Some(Ownership::Trivial)),
            (Type::Ptr, Some(Ownership::Trivial)),
            (Type::I64, Some(Ownership::Owned)),
            (
                Type::I64,
                Some(Ownership::Borrowed {
                    owner: OwnerOrigin::Caller,
                    scope: 0,
                }),
            ),
            (Type::I64, None),
            (Type::F64, Some(Ownership::Trivial)),
        ] {
            f.params = vec![(ValueId(0), "origem".into(), ty)];
            let mut classes: HashMap<_, _> = class.map(|c| (ValueId(0), c)).into_iter().collect();
            let before = classes.clone();
            let mut plan = TokenPlan::default();
            assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
            assert_eq!(classes, before);
            assert!(plan.instructions.is_empty());
        }
        f.params = vec![(ValueId(0), "bits".into(), Type::I64)];
        let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
        produce_arc_contracts(&f, &mut classes, &mut TokenPlan::default()).unwrap();
        // Anotação divergente do destino também falha sem publicar o resultado.
        f.blocks[1].instructions[0].2 = Type::I64;
        let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
        let before = classes.clone();
        assert!(produce_arc_contracts(&f, &mut classes, &mut TokenPlan::default()).is_err());
        assert_eq!(classes, before);
    }

    #[test]
    fn arithmetic_does_not_erase_ownership_before_bitcast() {
        let f = Function {
            symbol: "origem".into(),
            name: "origem".into(),
            debug_info: None,
            params: vec![(ValueId(0), "origem".into(), Type::I64)],
            return_ty: Type::F64,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(1),
                        Instruction::Add(
                            Operand::Val(ValueId(0)),
                            Operand::Constant(Constant::Int(0)),
                        ),
                        Type::I64,
                    ),
                    (
                        ValueId(2),
                        Instruction::Bitcast {
                            op: Operand::Val(ValueId(1)),
                            to: Type::F64,
                        },
                        Type::F64,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))),
            }],
        };
        for class in [
            None,
            Some(Ownership::Owned),
            Some(Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                scope: 0,
            }),
        ] {
            let mut classes: HashMap<_, _> = class.map(|c| (ValueId(0), c)).into_iter().collect();
            let before = classes.clone();
            let mut plan = TokenPlan::default();
            assert!(
                produce_arc_contracts(&f, &mut classes, &mut plan)
                    .unwrap_err()
                    .contains("origem Trivial")
            );
            assert_eq!(classes, before);
            assert!(plan.instructions.is_empty());
        }
        produce_arc_contracts(
            &f,
            &mut HashMap::from([(ValueId(0), Ownership::Trivial)]),
            &mut TokenPlan::default(),
        )
        .unwrap();
    }

    #[test]
    fn integer_widths_preserve_scalar_contract_and_reject_handles() {
        for (from, to, widen) in [
            (Type::I1, Type::I8, true),
            (Type::I1, Type::I64, true),
            (Type::I8, Type::I64, true),
            (Type::I64, Type::I8, false),
            (Type::I64, Type::I1, false),
            (Type::I8, Type::I1, false),
        ] {
            let inst = if widen {
                Instruction::ZExt {
                    op: Operand::Val(ValueId(0)),
                    from,
                    to,
                }
            } else {
                Instruction::Trunc {
                    op: Operand::Val(ValueId(0)),
                    from,
                    to,
                }
            };
            let mut f = Function {
                symbol: "largura".into(),
                name: "largura".into(),
                debug_info: None,
                params: vec![(ValueId(0), "x".into(), from)],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(ValueId(1), inst, to)],
                    terminator: Terminator::Return(None),
                }],
            };
            let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
            let mut plan = TokenPlan::default();
            produce_and_verify_tokens(
                &f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default(),
            )
            .unwrap();
            assert_eq!(classes[&ValueId(1)], Ownership::Trivial);
            assert_eq!(plan.instructions[&ValueId(1)], TokenEffect::default());
            for (ty, class) in [
                (Type::Ref, Some(Ownership::Trivial)),
                (Type::Ptr, Some(Ownership::Trivial)),
                (from, Some(Ownership::Owned)),
                (
                    from,
                    Some(Ownership::Borrowed {
                        owner: OwnerOrigin::Caller,
                        scope: 0,
                    }),
                ),
                (from, None),
            ] {
                f.params[0].2 = ty;
                let mut classes: HashMap<_, _> =
                    class.map(|c| (ValueId(0), c)).into_iter().collect();
                let before = classes.clone();
                let mut plan = TokenPlan::default();
                assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
                assert_eq!(classes, before);
                assert!(plan.instructions.is_empty());
            }
            f.params[0].2 = from;
            // A operação oposta com as mesmas larguras é inválida.
            f.blocks[0].instructions[0].1 = if widen {
                Instruction::Trunc {
                    op: Operand::Val(ValueId(0)),
                    from,
                    to,
                }
            } else {
                Instruction::ZExt {
                    op: Operand::Val(ValueId(0)),
                    from,
                    to,
                }
            };
            let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
            assert!(produce_arc_contracts(&f, &mut classes, &mut TokenPlan::default()).is_err());
            assert_eq!(classes.len(), 1);
        }
        // Sequência do transporte de bool por byte e palavra, com boxing
        // permanente no fim: nenhum token Owned nem saída pending é criado.
        let f = Function {
            symbol: "bool_bits".into(),
            name: "bool_bits".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(0),
                        Instruction::ZExt {
                            op: Operand::Constant(Constant::Bool(true)),
                            from: Type::I1,
                            to: Type::I8,
                        },
                        Type::I8,
                    ),
                    (
                        ValueId(1),
                        Instruction::ZExt {
                            op: Operand::Val(ValueId(0)),
                            from: Type::I8,
                            to: Type::I64,
                        },
                        Type::I64,
                    ),
                    (
                        ValueId(2),
                        Instruction::Trunc {
                            op: Operand::Val(ValueId(1)),
                            from: Type::I64,
                            to: Type::I8,
                        },
                        Type::I8,
                    ),
                    (
                        ValueId(3),
                        Instruction::Trunc {
                            op: Operand::Val(ValueId(2)),
                            from: Type::I8,
                            to: Type::I1,
                        },
                        Type::I1,
                    ),
                    (
                        ValueId(4),
                        Instruction::Box {
                            op: Operand::Val(ValueId(3)),
                            from: Type::I1,
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::Return(None),
            }],
        };
        let mut classes = HashMap::new();
        let mut plan = TokenPlan::default();
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        assert_eq!(classes.len(), 5);
        assert!(classes.values().all(|c| *c == Ownership::Trivial));
        assert!(plan.pendings.is_empty());
        let mut module = Module::default();
        module.arc_memory = true;
        module.functions.push(f);
        let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
        assert!(ir.contains("zext i1 true to i8"));
        assert!(ir.contains("zext i8 %v0 to i64"));
        assert!(ir.contains("trunc i64 %v1 to i8"));
        assert!(ir.contains("trunc i8 %v2 to i1"));
    }

    #[test]
    fn verifier_rejects_pure_operation_with_forged_metadata() {
        for (inst, ty) in [
            (
                Instruction::Bitcast {
                    op: Operand::Constant(Constant::Double(0.0)),
                    to: Type::Ref,
                },
                Type::Ref,
            ),
            (
                Instruction::Bitcast {
                    op: Operand::Val(ValueId(0)),
                    to: Type::F64,
                },
                Type::F64,
            ),
            (
                Instruction::ZExt {
                    op: Operand::Val(ValueId(0)),
                    from: Type::Ref,
                    to: Type::I64,
                },
                Type::I64,
            ),
            (
                Instruction::Trunc {
                    op: Operand::Val(ValueId(0)),
                    from: Type::Ref,
                    to: Type::I8,
                },
                Type::I8,
            ),
            (
                Instruction::Add(
                    Operand::Val(ValueId(0)),
                    Operand::Constant(Constant::Int(1)),
                ),
                Type::I64,
            ),
            (
                Instruction::FCmp(
                    FCmpOp::Eq,
                    Operand::Val(ValueId(0)),
                    Operand::Constant(Constant::Double(0.0)),
                ),
                Type::I1,
            ),
            (Instruction::LNot(Operand::Val(ValueId(0))), Type::I1),
            (
                Instruction::Box {
                    op: Operand::Val(ValueId(0)),
                    from: Type::I1,
                },
                Type::Ref,
            ),
            (Instruction::Const(Constant::Int(7)), Type::Ref),
        ] {
            let f = Function {
                symbol: "pura_falseada".into(),
                name: "pura_falseada".into(),
                debug_info: None,
                params: vec![(ValueId(0), "ref".into(), Type::Ref)],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(ValueId(1), inst, ty)],
                    terminator: Terminator::Return(None),
                }],
            };
            let classes = HashMap::from([
                (
                    ValueId(0),
                    Ownership::Borrowed {
                        owner: OwnerOrigin::Caller,
                        scope: 0,
                    },
                ),
                (ValueId(1), Ownership::Trivial),
            ]);
            let mut plan = TokenPlan::default();
            plan.instructions.insert(ValueId(1), TokenEffect::default());
            assert!(
                verify_tokens(&f, &classes, &FunctionTables::default(), &plan).is_err(),
                "{:?}",
                f.blocks[0].instructions[0].1
            );
        }
    }

    #[test]
    fn audited_scalar_field_produces_bits_without_classifying_generic_get() {
        let mut f = Function {
            symbol: "campo_escalar".into(),
            name: "campo_escalar".into(),
            debug_info: None,
            params: vec![(ValueId(0), "objeto".into(), Type::Ref)],
            return_ty: Type::F64,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(1),
                        Instruction::CallRuntime {
                            name: "dartforge_arc_ler_campo_escalar_v1".into(),
                            args: vec![
                                (Operand::Val(ValueId(0)), Type::Ref),
                                (Operand::Constant(Constant::Int(2)), Type::I64),
                            ],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    ),
                    (
                        ValueId(2),
                        Instruction::Bitcast {
                            op: Operand::Val(ValueId(1)),
                            to: Type::F64,
                        },
                        Type::F64,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))),
            }],
        };
        let origin = HashMap::from([(
            ValueId(0),
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                scope: 0,
            },
        )]);
        let mut classes = origin.clone();
        let mut plan = TokenPlan::default();
        let contracts = produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        let c = &contracts[&ValueId(1)];
        assert_eq!(c.result, Ownership::Trivial);
        assert_eq!(c.effect, TokenEffect::default());
        assert!(!c.invalidates_borrows && !c.calls_dart);
        assert_eq!(classes[&ValueId(2)], Ownership::Trivial);
        // O get genérico pode devolver Ref; nenhuma anotação I64 o certifica.
        if let Instruction::CallRuntime { name, .. } = &mut f.blocks[0].instructions[0].1 {
            *name = "dartforge_object_get".into();
        }
        let mut classes = origin.clone();
        let mut plan = TokenPlan::default();
        assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
        assert_eq!(classes, origin);
        assert!(plan.instructions.is_empty());
        // A ABI escalar não aceita ser anotada como produtora de Ref.
        if let Instruction::CallRuntime { name, ret_ty, .. } = &mut f.blocks[0].instructions[0].1 {
            *name = "dartforge_arc_ler_campo_escalar_v1".into();
            *ret_ty = Type::Ref;
        }
        f.blocks[0].instructions[0].2 = Type::Ref;
        assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
        assert_eq!(classes, origin);
        assert!(plan.instructions.is_empty());
    }

    #[test]
    fn pure_operation_does_not_reinterpret_reference_as_scalar() {
        for i in [
            Instruction::FCmp(
                FCmpOp::Eq,
                Operand::Val(ValueId(0)),
                Operand::Constant(Constant::Double(0.0)),
            ),
            Instruction::LNot(Operand::Val(ValueId(0))),
        ] {
            let f = Function {
                symbol: "pura".into(),
                name: "pura".into(),
                debug_info: None,
                params: vec![(ValueId(0), "x".into(), Type::Ref)],
                return_ty: Type::I1,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(ValueId(1), i, Type::I1)],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                }],
            };
            let mut classes = HashMap::from([(
                ValueId(0),
                Ownership::Borrowed {
                    owner: OwnerOrigin::Caller,
                    scope: 0,
                },
            )]);
            let before = classes.clone();
            let mut plan = TokenPlan::default();
            assert!(
                produce_arc_contracts(&f, &mut classes, &mut plan)
                    .unwrap_err()
                    .contains("escalares já avaliados")
            );
            assert_eq!(classes, before);
            assert!(plan.instructions.is_empty());
        }
    }

    #[test]
    fn typed_constants_and_permanent_literals_need_no_manual_seed() {
        let constants = [
            Constant::Int(7),
            Constant::Double(1.0),
            Constant::Bool(true),
            Constant::Null,
            Constant::String("literal".into()),
            Constant::StringWtf8(vec![120]),
        ];
        let types = [
            Type::I64,
            Type::F64,
            Type::I1,
            Type::Ref,
            Type::Ref,
            Type::Ref,
        ];
        let mut f = Function {
            symbol: "constantes".into(),
            name: "constantes".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: constants
                    .into_iter()
                    .zip(types)
                    .enumerate()
                    .map(|(i, (k, t))| (ValueId(i as u32), Instruction::Const(k), t))
                    .collect(),
                terminator: Terminator::Return(None),
            }],
        };
        let mut classes = HashMap::new();
        let mut plan = TokenPlan::default();
        f.blocks[0].instructions[0].2 = Type::Ref;
        assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
        assert!(classes.is_empty() && plan.instructions.is_empty());
        f.blocks[0].instructions[0].2 = Type::I64;
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        assert_eq!(classes.len(), 6);
        assert!(classes.values().all(|c| *c == Ownership::Trivial));
    }

    #[test]
    fn loop_bool_phi_is_produced_without_inventing_owner() {
        let mut f = Function {
            symbol: "phi_bool".into(),
            name: "phi_bool".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::I1,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(1)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![
                        (
                            ValueId(1),
                            Instruction::Phi {
                                ty: Type::I1,
                                incoming: vec![
                                    (BlockId(0), Operand::Constant(Constant::Bool(true))),
                                    (BlockId(1), Operand::Val(ValueId(2))),
                                ],
                            },
                            Type::I1,
                        ),
                        (
                            ValueId(2),
                            Instruction::LNot(Operand::Val(ValueId(1))),
                            Type::I1,
                        ),
                    ],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(2)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                },
            ],
        };
        let mut classes = HashMap::new();
        let mut plan = TokenPlan::default();
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        assert_eq!(classes[&ValueId(1)], Ownership::Trivial);
        assert!(!plan.instructions.contains_key(&ValueId(1)));
        let before = classes.clone();
        let effects = plan.instructions.clone();
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
            incoming[0].1 = Operand::Constant(Constant::Int(1));
        }
        assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
        assert_eq!(classes, before);
        assert_eq!(plan.instructions, effects);
    }

    #[test]
    fn numeric_phi_requires_origin_and_semantic_contract() {
        for (ty, constant) in [
            (Type::I1, Constant::Bool(true)),
            (Type::I64, Constant::Int(7)),
            (Type::I8, Constant::Int(2)),
            (Type::F64, Constant::Double(7.0)),
        ] {
            let mut f = Function {
                symbol: "phi_escalar".into(),
                name: "phi_escalar".into(),
                debug_info: None,
                params: vec![],
                return_ty: ty,
                blocks: vec![
                    BasicBlock {
                        id: BlockId(0),
                        instructions: vec![],
                        terminator: Terminator::Branch(BlockId(1)),
                    },
                    BasicBlock {
                        id: BlockId(1),
                        instructions: vec![(
                            ValueId(1),
                            Instruction::Phi {
                                ty,
                                incoming: vec![
                                    (BlockId(0), Operand::Constant(constant)),
                                    (BlockId(1), Operand::Val(ValueId(1))),
                                ],
                            },
                            ty,
                        )],
                        terminator: Terminator::CondBranch {
                            cond: Operand::Constant(Constant::Bool(false)),
                            then_block: BlockId(1),
                            else_block: BlockId(2),
                        },
                    },
                    BasicBlock {
                        id: BlockId(2),
                        instructions: vec![],
                        terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                    },
                ],
            };
            let mut classes = HashMap::new();
            let mut plan = TokenPlan::default();
            produce_and_verify_tokens(
                &f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default(),
            )
            .unwrap();
            assert_eq!(classes[&ValueId(1)], Ownership::Trivial);
            assert!(plan.instructions.is_empty());
            // Nem um contrato previamente fornecido funda um ciclo fechado.
            if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
                incoming[0].1 = Operand::Val(ValueId(1));
            }
            let before = classes.clone();
            assert!(
                produce_arc_contracts(&f, &mut classes, &mut plan)
                    .unwrap_err()
                    .contains("sem origem escalar")
            );
            assert_eq!(classes, before);
            // Um parâmetro de mesma largura precisa de contrato explícito.
            f.params.push((ValueId(0), "entrada".into(), ty));
            if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
                incoming[0].1 = Operand::Val(ValueId(0));
            }
            assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
            classes.insert(ValueId(0), Ownership::Owned);
            let before = classes.clone();
            assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
            assert_eq!(classes, before);
            assert!(plan.instructions.is_empty());
            classes.insert(ValueId(0), Ownership::Trivial);
            produce_and_verify_tokens(
                &f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default(),
            )
            .unwrap();
        }
    }

    #[test]
    fn comparison_produces_bool_without_consuming_reference() {
        let mut f = Function {
            symbol: "cmp".into(),
            name: "cmp".into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::I1,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(1),
                    Instruction::ICmp(
                        ICmpOp::Eq,
                        Operand::Val(ValueId(0)),
                        Operand::Constant(Constant::Null),
                    ),
                    Type::I1,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
            }],
        };
        let initial = HashMap::from([(
            ValueId(0),
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                scope: 0,
            },
        )]);
        let mut classes = initial.clone();
        let mut plan = TokenPlan::default();
        f.blocks[0].instructions[0].2 = Type::Ref;
        assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
        assert_eq!(classes, initial);
        assert!(plan.instructions.is_empty());
        f.blocks[0].instructions[0].2 = Type::I1;
        f.blocks[0].instructions.extend([
            (
                ValueId(2),
                Instruction::FCmp(
                    FCmpOp::Eq,
                    Operand::Constant(Constant::Double(1.0)),
                    Operand::Constant(Constant::Double(1.0)),
                ),
                Type::I1,
            ),
            (
                ValueId(3),
                Instruction::LNot(Operand::Val(ValueId(2))),
                Type::I1,
            ),
        ]);
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        assert_eq!(classes[&ValueId(1)], Ownership::Trivial);
        assert_eq!(plan.instructions[&ValueId(1)], TokenEffect::default());
        plan
            .instructions
            .get_mut(&ValueId(1))
            .unwrap()
            .always
            .push(ValueId(0));
        let before = plan.instructions.clone();
        assert!(produce_arc_contracts(&f, &mut classes, &mut plan).is_err());
        assert_eq!(plan.instructions, before);
    }

    #[test]
    fn owned_exception_has_independent_ref_result_without_new_failure() {
        for name_str in [
            "dartforge_arc_excecao_owned_v1",
            "dartforge_arc_rastro_owned_v1",
        ] {
            let i = Instruction::CallRuntime {
                name: name_str.into(),
                args: vec![],
                ret_ty: Type::Ref,
            };
            let c = runtime_call_contract(&i).unwrap();
            assert_eq!(c.result, Ownership::Owned);
            assert!(!c.effect.may_fail);
            assert!(c.effect.always.is_empty());
            let mut scalar = i.clone();
            if let Instruction::CallRuntime { ret_ty, .. } = &mut scalar {
                *ret_ty = Type::I64;
            }
            assert!(runtime_call_contract(&scalar).is_err());
        }
    }

    #[test]
    fn throw_with_trace_requires_two_ref_borrows_without_consuming_tokens() {
        let mut i = Instruction::CallRuntime {
            name: "dartforge_arc_lancar_com_rastro_ref_v1".into(),
            args: vec![
                (Operand::Val(ValueId(0)), Type::Ref),
                (Operand::Val(ValueId(1)), Type::Ref),
            ],
            ret_ty: Type::Void,
        };
        let c = runtime_call_contract(&i).unwrap();
        assert!(c.effect.may_fail);
        assert!(c.effect.always.is_empty());
        assert!(c.effect.success.is_empty());
        assert!(c.effect.error.is_empty());
        if let Instruction::CallRuntime { args, .. } = &mut i {
            args[1].1 = Type::I64;
        }
        assert!(runtime_call_contract(&i).is_err());
    }

    #[test]
    fn runtime_constants_preserve_scalar_address_distinction() {
        for k in [
            Constant::Bool(true),
            Constant::Double(1.0),
            Constant::Null,
            Constant::String("x".into()),
            Constant::StringWtf8(vec![120]),
            Constant::Function("getter".into()),
        ] {
            let i = Instruction::CallRuntime {
                name: "dartforge_arc_quadro_abrir_v1".into(),
                args: vec![(Operand::Constant(k), Type::I64)],
                ret_ty: Type::I64,
            };
            assert!(runtime_call_contract(&i).is_err());
        }
        let mut i = Instruction::CallRuntime {
            name: "dartforge_marcar_constante".into(),
            args: vec![
                (Operand::Constant(Constant::Null), Type::Ref),
                (
                    Operand::Constant(Constant::Function("getter".into())),
                    Type::I64,
                ),
            ],
            ret_ty: Type::Void,
        };
        runtime_call_contract(&i).unwrap();
        if let Instruction::CallRuntime { args, .. } = &mut i {
            args[1].0 = Operand::Constant(Constant::Bool(false));
        }
        assert!(runtime_call_contract(&i).is_err());
    }

    #[test]
    fn contracts_do_not_infer_ref_by_width_or_hide_tokens() {
        let mut i = Instruction::CallRuntime {
            name: "dartforge_arc_global_receber_v1".into(),
            args: vec![
                (Operand::Val(ValueId(0)), Type::I64),
                (Operand::Val(ValueId(1)), Type::Ref),
            ],
            ret_ty: Type::Void,
        };
        let c = runtime_call_contract(&i).unwrap();
        assert_eq!(c.effect.always, [ValueId(1)]);
        assert!(c.persistent_retention && c.invalidates_borrows);
        if let Instruction::CallRuntime { args, .. } = &mut i {
            args[1].1 = Type::I64;
        }
        assert!(runtime_call_contract(&i).is_err());
        if let Instruction::CallRuntime { args, .. } = &mut i {
            args[1] = (Operand::Constant(Constant::Null), Type::Ref);
        }
        assert!(
            runtime_call_contract(&i)
                .unwrap()
                .effect
                .always
                .is_empty()
        );
        for name_str in ["dartforge_arc_retain", "dartforge_alocar"] {
            let i = Instruction::CallRuntime {
                name: name_str.into(),
                args: vec![],
                ret_ty: Type::Void,
            };
            assert!(runtime_call_contract(&i).is_err());
        }
    }

    #[test]
    fn interrupt_and_byte_without_owners_but_inexact_abi_is_rejected() {
        // A carga atômica do controle não toca o heap nem atende mensagens.
        // Isso não certifica o ponto seguro que executa o caminho lento.
        let i = Instruction::CallRuntime {
            name: "dartforge_interrupcao_pendente".into(),
            args: vec![],
            ret_ty: Type::I8,
        };
        let c = runtime_call_contract(&i).unwrap();
        assert_eq!(c.result, Ownership::Trivial);
        assert_eq!(c.effect, TokenEffect::default());
        assert!(!c.invalidates_borrows && !c.persistent_retention);
        for ty in [Type::I1, Type::I64, Type::Ref, Type::Void] {
            let mut wrong = i.clone();
            if let Instruction::CallRuntime { ret_ty, .. } = &mut wrong {
                *ret_ty = ty;
            }
            assert!(runtime_call_contract(&wrong).is_err());
        }
        let mut wrong = i;
        if let Instruction::CallRuntime { args, .. } = &mut wrong {
            args.push((Operand::Constant(Constant::Null), Type::Ref));
        }
        assert!(runtime_call_contract(&wrong).is_err());
        let slow = runtime_call_contract(&Instruction::CallRuntime {
            name: "dartforge_ponto_seguro".into(), args: vec![], ret_ty: Type::Void,
        }).unwrap();
        assert!(slow.effect.may_fail && slow.invalidates_borrows && slow.calls_dart);
    }

    #[test]
    fn rti_define_borrows_object_without_turning_type_id_into_ref() {
        let i = Instruction::CallRuntime {
            name: "dartforge_rti_definir".into(),
            args: vec![(Operand::Val(ValueId(0)), Type::Ref),
                (Operand::Val(ValueId(1)), Type::I64)], ret_ty: Type::Void,
        };
        let c = runtime_call_contract(&i).unwrap();
        assert_eq!(c.result, Ownership::Trivial);
        assert_eq!(c.effect, TokenEffect::default());
        assert!(c.invalidates_borrows);
        assert!(!c.calls_dart && !c.persistent_retention);
        for case in 0..4 {
            let mut wrong = i.clone();
            if let Instruction::CallRuntime { args, ret_ty, .. } = &mut wrong {
                match case {
                    0 => args[0].1 = Type::I64,
                    1 => args[1].1 = Type::Ref,
                    2 => args[1].0 = Operand::Constant(Constant::Function("codigo".into())),
                    _ => *ret_ty = Type::Ref,
                }
            }
            assert!(runtime_call_contract(&wrong).is_err());
        }
    }

    #[test]
    fn abi_byte_is_not_i1_boolean() {
        let mut i = Instruction::CallRuntime {
            name: "dartforge_arc_verificar_abi".into(),
            args: vec![(Operand::Constant(Constant::Int(1)), Type::I64)],
            ret_ty: Type::I8,
        };
        assert_eq!(
            runtime_call_contract(&i).unwrap().result,
            Ownership::Trivial
        );
        if let Instruction::CallRuntime { ret_ty, .. } = &mut i {
            *ret_ty = Type::I1;
        }
        assert!(runtime_call_contract(&i).is_err());
    }

    #[test]
    fn capture_borrows_env_and_rejects_incompatible_signatures() {
        let i = Instruction::CallRuntime { name: "dartforge_arc_env_ref_v1".into(),
            args: vec![(Operand::Val(ValueId(0)), Type::Ref),
                (Operand::Constant(Constant::Int(39)), Type::I64)], ret_ty: Type::Ref };
        let c = runtime_call_contract(&i).unwrap();
        assert_eq!(c.result, Ownership::Borrowed { owner: OwnerOrigin::Value(ValueId(0)), scope: 0 });
        assert!(!c.persistent_retention && !c.invalidates_borrows && !c.calls_dart && !c.effect.may_fail);
        assert!(c.effect.always.is_empty());
        for case in 0..5 {
            let mut invalidates = i.clone();
            if let Instruction::CallRuntime { args, ret_ty, .. } = &mut invalidates {
                match case {
                    0 => args[0].1 = Type::I64,
                    1 => args[1].1 = Type::Ref,
                    2 => *ret_ty = Type::I64,
                    3 => { args.pop(); },
                    4 => args.push((Operand::Constant(Constant::Int(0)), Type::I64)),
                    _ => unreachable!(),
                }
            }
            assert!(runtime_call_contract(&invalidates).is_err(), "caso {case}");
        }
    }

    #[test]
    fn rti_recipe_borrows_string_and_returns_id_without_inventing_owned_ref() {
        let i = Instruction::CallRuntime { name: "dartforge_rti_receita".into(),
            args: vec![(Operand::Val(ValueId(0)), Type::Ref)], ret_ty: Type::I64 };
        let c = runtime_call_contract(&i).unwrap();
        assert_eq!(c.result, Ownership::Trivial);
        assert!(!c.persistent_retention && !c.invalidates_borrows && !c.calls_dart && !c.effect.may_fail);
        assert!(c.effect.always.is_empty());
        for case in 0..4 {
            let mut invalidates = i.clone();
            if let Instruction::CallRuntime { args, ret_ty, .. } = &mut invalidates {
                match case {
                    0 => args[0].1 = Type::I64,
                    1 => *ret_ty = Type::Ref,
                    2 => args.clear(),
                    3 => args.push((Operand::Constant(Constant::Int(0)), Type::I64)),
                    _ => unreachable!(),
                }
            }
            assert!(runtime_call_contract(&invalidates).is_err(), "caso {case}");
        }
    }

    #[test]
    fn publish_global_copies_owner_without_consuming_ssa() {
        let i = Instruction::CallRuntime {
            name: "dartforge_gc_global_root".into(),
            args: vec![
                (Operand::Val(ValueId(0)), Type::I64),
                (Operand::Val(ValueId(1)), Type::Ref),
            ],
            ret_ty: Type::Void,
        };
        let c = runtime_call_contract(&i).unwrap();
        assert!(c.effect.always.is_empty() && c.persistent_retention && c.invalidates_borrows);
        assert_eq!(c.result, Ownership::Trivial);
    }

    #[test]
    fn audited_collection_requires_exceptional_exit() {
        let i = Instruction::CallRuntime {
            name: "dartforge_gc_collect".into(),
            args: vec![],
            ret_ty: Type::Void,
        };
        let c = runtime_call_contract(&i).unwrap();
        assert!(c.effect.may_fail && c.invalidates_borrows);
        assert!(
            c.effect.always.is_empty() && c.effect.success.is_empty() && c.effect.error.is_empty()
        );
    }

    #[test]
    fn producer_checks_real_ssa_type_before_publishing() {
        let mut f = Function {
            symbol: "tipo_real".into(),
            name: "tipo_real".into(),
            debug_info: None,
            params: vec![(ValueId(0), "quadro".into(), Type::Ref)],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(1),
                    Instruction::CallRuntime {
                        name: "dartforge_arc_quadro_carregar_v1".into(),
                        args: vec![
                            (Operand::Val(ValueId(0)), Type::I64),
                            (Operand::Constant(Constant::Int(0)), Type::I64),
                        ],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(None),
            }],
        };
        let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
        let initial = classes.clone();
        let mut plan = TokenPlan::default();
        let error = produce_runtime_contracts(&f, &mut classes, &mut plan).unwrap_err();
        assert!(
            error.contains("argumento 0") && error.contains("v0"),
            "{error}"
        );
        assert_eq!(classes, initial);
        assert!(plan.instructions.is_empty());
        f.params.clear();
        assert!(produce_runtime_contracts(&f, &mut classes, &mut plan).is_err());
        assert_eq!(classes, initial);
        assert!(plan.instructions.is_empty());
        f.params.push((ValueId(0), "quadro".into(), Type::I64));
        produce_runtime_contracts(&f, &mut classes, &mut plan).unwrap();
        assert_eq!(classes[&ValueId(1)], Ownership::Owned);
    }

    #[test]
    fn producer_is_atomic_and_produces_owned_without_inferring_by_width() {
        let mut f = Function {
            symbol: "f".into(),
            name: "f".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(0),
                    Instruction::CallRuntime {
                        name: "dartforge_arc_quadro_carregar_v1".into(),
                        args: vec![
                            (Operand::Constant(Constant::Int(1)), Type::I64),
                            (Operand::Constant(Constant::Int(0)), Type::I64),
                        ],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(None),
            }],
        };
        let mut classes = HashMap::new();
        let mut plan = TokenPlan::default();
        f.blocks[0].instructions.push((
            ValueId(1),
            Instruction::CallRuntime {
                name: "extern_sem_contrato".into(),
                args: vec![],
                ret_ty: Type::Void,
            },
            Type::Void,
        ));
        assert!(produce_runtime_contracts(&f, &mut classes, &mut plan).is_err());
        assert!(classes.is_empty() && plan.instructions.is_empty());
        f.blocks[0].instructions.pop();
        produce_runtime_contracts(&f, &mut classes, &mut plan).unwrap();
        assert_eq!(classes[&ValueId(0)], Ownership::Owned);
        classes.insert(ValueId(0), Ownership::Trivial);
        let before = plan.instructions.clone();
        assert!(produce_runtime_contracts(&f, &mut classes, &mut plan).is_err());
        assert_eq!(classes[&ValueId(0)], Ownership::Trivial);
        assert_eq!(plan.instructions, before);
    }
}
