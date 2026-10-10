//! Tradução das externs auditadas para o plano de tokens da HIR.
//! Não presume contratos para símbolos ausentes nem certifica proveniência/borrows.

use super::OrigemOwner;
use super::{
    EfeitoTokens, Ownership, PlanoEscopos, PlanoTokens, verificar_escopos, verificar_tokens,
};
use crate::hir::*;
use dartforge_runtime::ownership::{ModoParametro, ModoResultado, contrato};
use std::collections::{HashMap, HashSet, VecDeque};

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
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
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
pub fn produzir_parametros_ref_dart(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
) -> Result<(), String> {
    let esperado = Ownership::Borrowed {
        owner: OrigemOwner::Chamador,
        escopo: 0,
    };
    let mut ids = HashSet::new();
    let mut produzidos = Vec::new();
    for (v, _, ty) in &f.params {
        if !ids.insert(*v) {
            return Err(format!("parâmetro Dart v{} repetido", v.0));
        }
        if *ty == Type::Ref {
            if classes.get(v).is_some_and(|c| *c != esperado) {
                return Err(format!(
                    "parâmetro Dart Ref v{} exige Borrowed(Chamador, 0)",
                    v.0
                ));
            }
            produzidos.push(*v);
        }
    }
    for v in produzidos {
        classes.insert(v, esperado.clone());
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
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
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
pub fn inserir_retencao_retornos_dart(
    f: &mut Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plano: &mut PlanoTokens,
    tabelas: &TabelasDaFuncao,
    escopos: &PlanoEscopos,
) -> Result<usize, String> {
    inserir_saidas_dart(f, classes, plano, tabelas, escopos, false).map(|(copias, _)| copias)
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
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
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
pub fn inserir_arc_saidas_dart(
    f: &mut Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plano: &mut PlanoTokens,
    tabelas: &TabelasDaFuncao,
    escopos: &PlanoEscopos,
) -> Result<(usize, usize), String> {
    inserir_saidas_dart(f, classes, plano, tabelas, escopos, true)
}

fn inserir_saidas_dart(
    f: &mut Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plano: &mut PlanoTokens,
    tabelas: &TabelasDaFuncao,
    escopos: &PlanoEscopos,
    cleanup: bool,
) -> Result<(usize, usize), String> {
    let mut nova = f.clone();
    super::tokens::normalizar_saidas_lanca(&mut nova, tabelas)?;
    let mut novas_classes = classes.clone();
    let mut novo_plano = plano.clone();
    produzir_parametros_ref_dart(&nova, &mut novas_classes)?;
    produzir_contratos_arc(&nova, &mut novas_classes, &mut novo_plano)?;
    super::vivacidade_classificada(&nova, &novas_classes)?;
    // Reserva também IDs de metadados: uma inserção não pode mascarar entrada
    // obsoleta fazendo-a coincidir com uma definição recém-criada.
    let mut proximo = nova
        .params
        .iter()
        .map(|(v, _, _)| v.0)
        .chain(
            nova.blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .map(|(v, _, _)| v.0),
        )
        .chain(novo_plano.instrucoes.keys().map(|v| v.0))
        .chain(novo_plano.pendencias.keys().map(|v| v.0))
        .chain(tabelas.invocacoes.keys().map(|v| v.0))
        .chain(escopos.antes.keys().map(|v| v.0))
        .max()
        .map_or(0, |v| u64::from(v) + 1);
    let mut id = || -> Result<ValueId, String> {
        let v = u32::try_from(proximo).map_err(|_| "IDs SSA esgotados ao preparar retorno Dart")?;
        proximo += 1;
        Ok(ValueId(v))
    };
    let mut copias = 0;
    if nova.return_ty == Type::Ref {
        for b in &mut nova.blocks {
            let Terminator::Return(Some(op)) = &b.terminator else {
                continue;
            };
            let mut op = op.clone();
            match &op {
                Operand::Val(v) if novas_classes[v] == Ownership::Owned => continue,
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
            copias += 1;
        }
    }
    let mut liberacoes = 0;
    if cleanup {
        produzir_contratos_arc(&nova, &mut novas_classes, &mut novo_plano)?;
        let saidas =
            super::tokens::saidas_para_cleanup(&nova, &novas_classes, tabelas, &novo_plano)?;
        for b in &mut nova.blocks {
            if let Some(restantes) = saidas.get(&b.id) {
                for de in restantes {
                    b.instructions.push((
                        id()?,
                        Instruction::ArcDrop {
                            value: Operand::Val(*de),
                        },
                        Type::Void,
                    ));
                    liberacoes += 1;
                }
            }
        }
    }
    produzir_e_verificar_tokens_dart(&nova, &mut novas_classes, &mut novo_plano, tabelas, escopos)?;
    *f = nova;
    *classes = novas_classes;
    *plano = novo_plano;
    Ok((copias, liberacoes))
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
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
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
pub fn produzir_e_verificar_tokens_dart(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plano: &mut PlanoTokens,
    tabelas: &TabelasDaFuncao,
    escopos: &PlanoEscopos,
) -> Result<HashMap<ValueId, ContratoChamadaRuntime>, String> {
    let retorno = if f.return_ty == Type::Ref {
        super::RetornoTokens::Owned
    } else {
        super::RetornoTokens::Trivial
    };
    if plano.retorno != retorno {
        return Err("convenção Dart exige resultado Ref Owned e demais resultados Trivial".into());
    }
    let mut novas_classes = classes.clone();
    let mut novo_plano = plano.clone();
    produzir_parametros_ref_dart(f, &mut novas_classes)?;
    let contratos =
        produzir_e_verificar_tokens(f, &mut novas_classes, &mut novo_plano, tabelas, escopos)?;
    *classes = novas_classes;
    *plano = novo_plano;
    Ok(contratos)
}

/// Produz metadados ARC e os publica após verificar CFG/SSA, tokens e escopos.
///
/// Parâmetros e instruções fora da cobertura do produtor exigem contratos
/// semânticos fornecidos pelo chamador. O CFG e os planos devem ser da mesma
/// versão da função. Quadros locais exigem abertura/fechamento LIFO e pilha
/// consistente por caminho; IDs importados/aliases de quadros são recusados.
/// Limites constantes são conferidos quando a capacidade é conhecida. Não insere
/// RC nem certifica limites dinâmicos, proveniência geral, invalidação ou Finalizable.
///
/// # Erros
/// CFG/SSA inválido, falha de produção, inventário incompleto, token indisponível/não consumido,
/// CFG excepcional incompatível ou empréstimo fora de escopo. Em qualquer
/// desses casos, classes e plano de tokens permanecem intactos.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
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
pub fn produzir_e_verificar_tokens(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plano: &mut PlanoTokens,
    tabelas: &TabelasDaFuncao,
    escopos: &PlanoEscopos,
) -> Result<HashMap<ValueId, ContratoChamadaRuntime>, String> {
    super::ssa::verificar(f)?;
    let mut novas_classes = classes.clone();
    let mut novo_plano = plano.clone();
    let contratos = produzir_contratos_arc(f, &mut novas_classes, &mut novo_plano)?;
    verificar_tokens(f, &novas_classes, tabelas, &novo_plano)?;
    verificar_escopos(f, &novas_classes, escopos)?;
    super::quadros::verificar(f)?;
    *classes = novas_classes;
    *plano = novo_plano;
    Ok(contratos)
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
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
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
pub fn produzir_contratos_runtime(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plano: &mut PlanoTokens,
) -> Result<HashMap<ValueId, ContratoChamadaRuntime>, String> {
    produzir(f, classes, plano, false)
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
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
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
pub fn produzir_contratos_arc(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plano: &mut PlanoTokens,
) -> Result<HashMap<ValueId, ContratoChamadaRuntime>, String> {
    let mut novas_classes = classes.clone();
    let mut novo_plano = plano.clone();
    let locais = super::locais::produzir(f, &mut novas_classes, &mut novo_plano)?;
    let contratos = produzir(f, &mut novas_classes, &mut novo_plano, true)?;
    produzir_phi(f, &mut novas_classes, &novo_plano)?;
    super::locais::verificar_valores(f, &locais, &novas_classes)?;
    super::puros::conferir_origens(f, &novas_classes)?;
    *classes = novas_classes;
    *plano = novo_plano;
    Ok(contratos)
}

/// Resolve a propriedade dos Phi Ref, inclusive ciclos com origem conhecida.
/// A disponibilidade e o consumo simultâneo por aresta são provados depois
/// pelo verificador de tokens, não pela conectividade deste grafo.
fn produzir_phi(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plano: &PlanoTokens,
) -> Result<(), String> {
    let defs: HashMap<_, _> = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .map(|(v, i, _)| (*v, i))
        .collect();
    let mut candidatos = HashSet::new();
    let tipos: HashMap<_, _> = f
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
    let mut exigem_owned: HashSet<_> = plano
        .instrucoes
        .values()
        .flat_map(|e| e.sempre.iter().chain(&e.sucesso).chain(&e.erro))
        .copied()
        .collect();
    for b in &f.blocks {
        for (v, inst, _) in &b.instructions {
            let op = match inst {
                Instruction::ArcMove { value }
                | Instruction::ArcDrop { value }
                | Instruction::ArcStoreStrong {
                    value,
                    modo: ModoStoreForte::Move,
                    ..
                } => Some(value),
                _ => None,
            };
            if let Some(Operand::Val(de)) = op {
                exigem_owned.insert(*de);
            }
            if classes.get(v) == Some(&Ownership::Owned) {
                exigem_owned.insert(*v);
            }
        }
        if plano.retorno == super::RetornoTokens::Owned
            && let Terminator::Return(Some(Operand::Val(v))) = b.terminator
        {
            exigem_owned.insert(v);
        }
    }
    loop {
        let anterior = exigem_owned.len();
        for v in exigem_owned.clone() {
            if let Some(Instruction::Phi { incoming, .. }) = defs.get(&v) {
                exigem_owned.extend(incoming.iter().filter_map(|(_, op)| {
                    if let Operand::Val(de) = op {
                        Some(*de)
                    } else {
                        None
                    }
                }));
            }
        }
        if anterior == exigem_owned.len() {
            break;
        }
    }
    loop {
        let mut mudou = false;
        for (v, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if *ty != Type::Ref || classes.contains_key(v) || exigem_owned.contains(v) {
                continue;
            }
            let Instruction::Phi { incoming, .. } = inst else {
                continue;
            };
            let mut ancora = false;
            let compativeis = !incoming.is_empty()
                && incoming.iter().all(|(_, op)| match op {
                    Operand::Constant(Constant::Null) => {
                        ancora = true;
                        true
                    }
                    Operand::Val(de) if tipos.get(de) == Some(&Type::Ref) => {
                        match classes.get(de) {
                            Some(Ownership::Trivial) => {
                                ancora = true;
                                true
                            }
                            None if !exigem_owned.contains(de)
                                && matches!(
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
            if compativeis && ancora {
                classes.insert(*v, Ownership::Trivial);
                mudou = true;
            }
        }
        if !mudou {
            break;
        }
    }
    // Resolve primeiro empréstimos com premissas conhecidas. Não transforma
    // entradas Owned em borrow implícito; um ciclo precisa de âncora externa.
    let mut emprestados = HashMap::new();
    loop {
        let mut mudou = false;
        for (v, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if *ty != Type::Ref || classes.contains_key(v) {
                continue;
            }
            let Instruction::Phi { incoming, .. } = inst else {
                continue;
            };
            let mut origem = None;
            let compativeis = !incoming.is_empty()
                && incoming.iter().all(|(_, op)| match op {
                    Operand::Constant(Constant::Null) => true,
                    Operand::Val(de) if tipos.get(de) == Some(&Type::Ref) => {
                        match classes.get(de) {
                            Some(c @ Ownership::Borrowed { .. }) => {
                                if let Some((_, anterior)) = &origem {
                                    anterior == c
                                } else {
                                    origem = Some((*de, c.clone()));
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
            if compativeis && let Some((de, mut classe)) = origem {
                emprestados.insert(*v, classe.clone());
                // Valores locais não declaram Chamador diretamente: conservam
                // a dependência no parâmetro que sustenta esse empréstimo.
                if let Ownership::Borrowed {
                    owner: OrigemOwner::Chamador,
                    escopo,
                } = classe
                {
                    classe = Ownership::Borrowed {
                        owner: OrigemOwner::Valor(de),
                        escopo,
                    };
                }
                classes.insert(*v, classe);
                mudou = true;
            }
        }
        if !mudou {
            break;
        }
    }
    // A âncora permite percorrer o ciclo, mas não dispensa verificar cada
    // entrada depois que o ponto fixo fornece as classes dos Phis dependentes.
    for (v, origem) in &emprestados {
        let Instruction::Phi { incoming, .. } = defs[v] else {
            unreachable!()
        };
        if incoming.iter().any(|(_, op)| match op {
            Operand::Constant(Constant::Null) => false,
            Operand::Val(de) if tipos.get(de) == Some(&Type::Ref) => !classes
                .get(de)
                .is_some_and(|c| *c == Ownership::Trivial || c == origem || c == &classes[v]),
            _ => true,
        }) {
            return Err(format!(
                "Phi borrowed v{}: entrada incompatível após propagação",
                v.0
            ));
        }
    }
    let mut ordem = Vec::new();
    let escalares: HashSet<_> = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .filter_map(|(v, i, ty)| matches!(i, Instruction::Phi { .. }).then_some((*v, *ty)))
        .filter(|(v, ty)| {
            matches!(ty, Type::I1 | Type::I64 | Type::F64)
                || (*ty == Type::Ref && classes.get(v) == Some(&Ownership::Trivial))
        })
        .map(|(v, _)| v)
        .collect();
    let mut pais_escalares: HashMap<ValueId, Vec<ValueId>> = HashMap::new();
    let mut fila_escalar = VecDeque::new();
    for (v, i, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
        if let Instruction::Phi { ty: declarado, .. } = i {
            if ty != declarado || plano.instrucoes.contains_key(v) {
                return Err(format!("Phi v{}: tipo ou plano incompatível", v.0));
            }
            if escalares.contains(v) {
                let Instruction::Phi { incoming, .. } = i else {
                    unreachable!()
                };
                if incoming.is_empty() || classes.get(v).is_some_and(|c| *c != Ownership::Trivial) {
                    return Err(format!(
                        "Phi v{}: classe ou entradas escalares incompatíveis",
                        v.0
                    ));
                }
                let mut ancora = false;
                for (_, op) in incoming {
                    match op {
                        Operand::Constant(Constant::Null) if *ty == Type::Ref => ancora = true,
                        Operand::Constant(Constant::Bool(_)) if *ty == Type::I1 => ancora = true,
                        Operand::Constant(Constant::Int(_)) if *ty == Type::I64 => ancora = true,
                        Operand::Constant(Constant::Double(_)) if *ty == Type::F64 => ancora = true,
                        Operand::Val(de) if tipos.get(de) == Some(ty) && escalares.contains(de) => {
                            pais_escalares.entry(*de).or_default().push(*v);
                        }
                        Operand::Val(de)
                            if tipos.get(de) == Some(ty)
                                && classes.get(de) == Some(&Ownership::Trivial) =>
                        {
                            ancora = true
                        }
                        _ => {
                            return Err(format!(
                                "Phi v{}: entrada sem contrato escalar compatível",
                                v.0
                            ));
                        }
                    }
                }
                if ancora {
                    fila_escalar.push_back(*v);
                }
            }
            if *ty == Type::Ref && classes.get(v).is_none_or(|c| *c == Ownership::Owned) {
                candidatos.insert(*v);
                ordem.push(*v);
            }
        }
    }
    // A largura física não prova ausência de ownership. Só propagamos contratos
    // a partir de entradas auditadas, rejeitando ciclos sem origem externa.
    let mut fundados_escalares = HashSet::new();
    while let Some(v) = fila_escalar.pop_front() {
        if fundados_escalares.insert(v) {
            if let Some(dependentes) = pais_escalares.get(&v) {
                fila_escalar.extend(dependentes);
            }
        }
    }
    for v in escalares {
        if !fundados_escalares.contains(&v) {
            return Err(format!("Phi v{} sem origem escalar fora do ciclo", v.0));
        }
        classes.insert(v, Ownership::Trivial);
    }
    let mut pais: HashMap<ValueId, Vec<ValueId>> = HashMap::new();
    let mut fila = VecDeque::new();
    for v in &ordem {
        let Instruction::Phi { incoming, .. } = defs[v] else {
            unreachable!()
        };
        if incoming.is_empty() {
            return Err(format!("Phi v{} sem entradas", v.0));
        }
        let mut ancora = false;
        for (_, op) in incoming {
            let mut op = op;
            let mut movimentos = HashSet::new();
            loop {
                match op {
                    Operand::Constant(Constant::Null) => {
                        ancora = true;
                        break;
                    }
                    Operand::Val(de) if tipos.get(de) != Some(&Type::Ref) => {
                        return Err(format!(
                            "Phi v{}: entrada v{} não tem representação Ref",
                            v.0, de.0
                        ));
                    }
                    Operand::Val(de) if candidatos.contains(de) => {
                        pais.entry(*de).or_default().push(*v);
                        break;
                    }
                    Operand::Val(de) if classes.get(de) == Some(&Ownership::Owned) => {
                        if let Some(Instruction::ArcMove { value }) = defs.get(de) {
                            if !movimentos.insert(*de) {
                                return Err(format!("Phi v{}: ciclo de moves sem origem", v.0));
                            }
                            op = value;
                        } else {
                            ancora = true;
                            break;
                        }
                    }
                    _ => {
                        return Err(format!(
                            "Phi v{}: entrada não é owned/null; copie o empréstimo na aresta",
                            v.0
                        ));
                    }
                }
            }
        }
        if ancora {
            fila.push_back(*v);
        }
    }
    let mut fundados = HashSet::new();
    while let Some(v) = fila.pop_front() {
        if fundados.insert(v) {
            if let Some(dependentes) = pais.get(&v) {
                fila.extend(dependentes);
            }
        }
    }
    for v in ordem {
        if !fundados.contains(&v) {
            return Err(format!("Phi v{} sem origem owned/null fora do ciclo", v.0));
        }
        classes.insert(v, Ownership::Owned);
    }
    Ok(())
}

// O helper LLVM mantém o ID e converte u8 em I1; o caminho lento tem
// exatamente o contrato da ABI auditada, incluindo TypeError/reentrada.
pub(super) fn chamada_unbox_bool(inst: &Instruction) -> Option<Instruction> {
    let Instruction::Unbox { op, to: Type::I1 } = inst else {
        return None;
    };
    Some(Instruction::CallRuntime {
        name: "dartforge_unbox_bool".into(),
        args: vec![(op.clone(), Type::Ref)],
        ret_ty: Type::I8,
    })
}

fn produzir(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plano: &mut PlanoTokens,
    incluir_arc: bool,
) -> Result<HashMap<ValueId, ContratoChamadaRuntime>, String> {
    let mut contratos = HashMap::new();
    let mut fixas = HashMap::new();
    let mut efeitos_puros = HashMap::new();
    // Inclui definições de todos os blocos: a ordem física não é dominância.
    // A disponibilidade por caminho continua a cargo do verificador SSA.
    let tipos: HashMap<_, _> = f
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
        if incluir_arc {
            if super::puros::conferir(*v, inst, ty, &tipos)? {
                let efeito = EfeitoTokens::default();
                if classes.get(v).is_some_and(|c| *c != Ownership::Trivial)
                    || plano.instrucoes.get(v).is_some_and(|e| *e != efeito)
                {
                    return Err(format!(
                        "v{}: contrato incompatível com constante/operação pura",
                        v.0
                    ));
                }
                fixas.insert(*v, Ownership::Trivial);
                efeitos_puros.insert(*v, efeito);
            }
            let fixa = match inst {
                Instruction::ArcCopy { .. }
                | Instruction::ArcMove { .. }
                | Instruction::ArcLoadStrong { .. } => Some((Ownership::Owned, Type::Ref)),
                Instruction::ArcDrop { .. } | Instruction::ArcStoreStrong { .. } => {
                    Some((Ownership::Trivial, Type::Void))
                }
                _ => None,
            };
            if let Some((classe, esperado)) = fixa {
                if *ty != esperado
                    || classes.get(v).is_some_and(|c| *c != classe)
                    || plano.instrucoes.contains_key(v)
                {
                    return Err(format!("v{}: contrato incompatível com operação ARC", v.0));
                }
                fixas.insert(*v, classe);
            }
        }
        let unbox_bool = if incluir_arc {
            chamada_unbox_bool(inst)
        } else {
            None
        };
        let chamada = unbox_bool.as_ref().unwrap_or(inst);
        if let Instruction::CallRuntime { ret_ty, args, .. } = chamada {
            if if unbox_bool.is_some() {
                *ty != Type::I1
            } else {
                ty != ret_ty
            } {
                return Err(format!("v{}: tipo do resultado incompatível", v.0));
            }
            let c = contrato_chamada_runtime(chamada).map_err(|e| format!("v{}: {e}", v.0))?;
            for (n, (op, ty)) in args.iter().enumerate() {
                if let Operand::Val(arg) = op
                    && tipos.get(arg) != Some(ty)
                {
                    return Err(format!(
                        "v{}: argumento {n} aponta para SSA v{} ausente ou de tipo incompatível",
                        v.0, arg.0
                    ));
                }
            }
            if classes.get(v).is_some_and(|classe| *classe != c.resultado)
                || plano.instrucoes.get(v).is_some_and(|e| *e != c.efeito)
            {
                return Err(format!(
                    "v{}: metadados conflitam com contrato runtime",
                    v.0
                ));
            }
            contratos.insert(*v, c);
        }
    }
    let mut pendencias = HashMap::new();
    for b in &f.blocks {
        if let Terminator::CondBranch { then_block, .. } = &b.terminator
            && let Some((call, _, _)) = b
                .instructions
                .len()
                .checked_sub(3)
                .map(|i| &b.instructions[i])
            && contratos.get(call).is_some_and(|c| c.efeito.pode_falhar)
            && super::classificacao::conferir_pendencia(b, *call, *then_block).is_some()
        {
            if plano.pendencias.get(call).is_some_and(|p| p != then_block) {
                return Err(format!("v{}: saída pending conflita com CFG", call.0));
            }
            pendencias.insert(*call, *then_block);
        }
    }
    classes.extend(fixas);
    plano.pendencias.extend(pendencias);
    plano.instrucoes.extend(efeitos_puros);
    for (v, c) in &contratos {
        classes.insert(*v, c.resultado);
        plano.instrucoes.insert(*v, c.efeito.clone());
    }
    Ok(contratos)
}

/// Contrato semântico de uma chamada ordinária auditada.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::contrato_chamada_runtime};
/// let i = Instruction::CallRuntime { name: "dartforge_arc_collect".into(), args: vec![], ret_ty: Type::Void };
/// let c = contrato_chamada_runtime(&i)?;
/// assert!(c.invalida_borrows && c.efeito.sempre.is_empty());
/// # Ok::<(), String>(())
/// ```
#[derive(Debug)]
pub struct ContratoChamadaRuntime {
    /// Consumos de argumentos SSA, preservando multiplicidade.
    pub efeito: EfeitoTokens,
    /// Classe do resultado, proveniente da extern e não da largura i64.
    pub resultado: Ownership,
    /// Exige considerar owners persistentes internos ao runtime.
    pub retencao_persistente: bool,
    /// Exige prova separada das dependências borrowed após a chamada.
    pub invalida_borrows: bool,
    /// Reentrada Dart auditada, inclusive construção de TypeError pelo SDK.
    pub chama_dart: bool,
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
/// use dartforge_emit_native::{hir::*, otimizar::arc::{contrato_chamada_runtime, Ownership}};
/// let i = Instruction::CallRuntime { name: "dartforge_arc_quadro_carregar_v1".into(),
///     args: vec![(Operand::Val(ValueId(0)), Type::I64), (Operand::Constant(Constant::Int(0)), Type::I64)],
///     ret_ty: Type::Ref };
/// assert_eq!(contrato_chamada_runtime(&i)?.resultado, Ownership::Owned);
/// # Ok::<(), String>(())
/// ```
pub fn contrato_chamada_runtime(inst: &Instruction) -> Result<ContratoChamadaRuntime, String> {
    let Instruction::CallRuntime { name, args, ret_ty } = inst else {
        return Err("contrato runtime exige CallRuntime".into());
    };
    let c = contrato(name).map_err(|e| format!("{name}: {e}"))?;
    // Retain produz um owner independente, mas a extern retorna void. A HIR
    // representa essa produção com ArcCopy; tratá-la como chamada borrowed
    // comum permitiria perder o token sem que o verificador percebesse.
    if name == "dartforge_arc_retain" {
        return Err("retain direto exige ArcCopy com resultado SSA owned".into());
    }
    if c.parametros.len() != args.len() {
        return Err(format!("{name}: aridade incompatível com ownership.tsv"));
    }
    let (ty, resultado) = match c.resultado {
        ModoResultado::Owned => (Type::Ref, Ownership::Owned),
        ModoResultado::BorrowArg(n) => (
            Type::Ref,
            match args[n].0 {
                Operand::Val(v) => Ownership::Borrowed {
                    owner: OrigemOwner::Valor(v),
                    escopo: 0,
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
        ModoResultado::Void => (Type::Void, Ownership::Trivial),
    };
    if *ret_ty != ty {
        return Err(format!(
            "{name}: resultado incompatível com contrato semântico"
        ));
    }
    let mut efeito = EfeitoTokens {
        pode_falhar: c.pode_falhar,
        ..Default::default()
    };
    for (n, (modo, (op, ty))) in c.parametros.iter().zip(args).enumerate() {
        let referencia = matches!(
            modo,
            ModoParametro::Borrow
                | ModoParametro::Consume
                | ModoParametro::ConsumeSuccess
                | ModoParametro::ConsumeError
        );
        let esperado = if referencia {
            Type::Ref
        } else if *modo == ModoParametro::ScalarF64 {
            Type::F64
        } else {
            Type::I64
        };
        if *ty != esperado {
            return Err(format!("{name}: tipo do argumento {n} incompatível"));
        }
        if referencia {
            match op {
                Operand::Val(v) => {
                    if *modo == ModoParametro::Consume {
                        efeito.sempre.push(*v);
                    }
                    if *modo == ModoParametro::ConsumeSuccess {
                        efeito.sucesso.push(*v);
                    }
                    if *modo == ModoParametro::ConsumeError {
                        efeito.erro.push(*v);
                    }
                }
                Operand::Constant(Constant::Null) => {}
                _ => return Err(format!("{name}: argumento Ref {n} exige SSA ou null")),
            }
        } else if let Operand::Constant(k) = op {
            // Um endereço de função só tem significado em parâmetro nativo;
            // a anotação I64 não transforma bool/double/literal em inteiro.
            let compativel = if *modo == ModoParametro::ScalarF64 {
                matches!(k, Constant::Double(_))
            } else {
                matches!(k, Constant::Int(_))
                    || (*modo == ModoParametro::Native && matches!(k, Constant::Funcao(_)))
            };
            if !compativel {
                return Err(format!(
                    "{name}: constante do argumento {n} incompatível com seu contrato"
                ));
            }
        }
    }
    Ok(ContratoChamadaRuntime {
        efeito,
        resultado,
        retencao_persistente: c.retencao_persistente,
        invalida_borrows: c.invalida_borrows,
        chama_dart: c.chama_dart,
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn unbox_empresta_referencia_e_devolve_escalar_falivel_sem_consumo() {
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
            let c = contrato_chamada_runtime(&inst).unwrap();
            assert_eq!(c.resultado, Ownership::Trivial);
            assert!(c.efeito.pode_falhar && c.invalida_borrows && c.chama_dart);
            assert!(!c.retencao_persistente);
            assert!(
                c.efeito.sempre.is_empty()
                    && c.efeito.sucesso.is_empty()
                    && c.efeito.erro.is_empty()
            );
            if let Instruction::CallRuntime { ret_ty: r, .. } = &mut inst {
                *r = Type::Ref;
            }
            assert!(contrato_chamada_runtime(&inst).is_err());
            if let Instruction::CallRuntime {
                args, ret_ty: r, ..
            } = &mut inst
            {
                *r = ret_ty;
                args[0].1 = Type::I64;
            }
            assert!(contrato_chamada_runtime(&inst).is_err());
        }
    }

    #[test]
    fn saida_lanca_libera_resultado_e_guarda_exige_cfg_sem_publicar() {
        let original = Function {
            symbol: "desenrolar".into(),
            name: "desenrolar".into(),
            depuracao: None,
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
        let mut plano = PlanoTokens {
            retorno: super::super::RetornoTokens::Owned,
            ..Default::default()
        };
        let mut tabelas = TabelasDaFuncao::default();
        tabelas.saidas.insert(BlockId(0), SaidaPorExcecao::Lanca);
        assert_eq!(
            inserir_arc_saidas_dart(
                &mut f,
                &mut classes,
                &mut plano,
                &tabelas,
                &PlanoEscopos::default()
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
        let mut modulo = Module::new();
        modulo.memoria_arc = true;
        modulo.excecoes_por_tabelas = true;
        modulo.functions.push(f.clone());
        modulo.tabelas.push(tabelas.clone());
        let ir = crate::llvm::LlvmEmitter::new(&modulo).emit_all();
        let funcao = ir
            .split("@desenrolar(")
            .nth(1)
            .unwrap()
            .split("\n}")
            .next()
            .unwrap();
        let liberacao = funcao
            .find("call void @dartforge_arc_release(i64 %v1)")
            .unwrap();
        // O primeiro lançamento pode ser o guard do prólogo, antes de criar
        // qualquer token; o último é a saída Lanca do corpo preparado.
        let desenrolar = funcao.rfind("call void @df.lancar()").unwrap();
        assert!(liberacao < desenrolar);
        assert_eq!(
            inserir_arc_saidas_dart(
                &mut f,
                &mut classes,
                &mut plano,
                &tabelas,
                &PlanoEscopos::default()
            )
            .unwrap(),
            (0, 0)
        );
        f = original;
        classes.clear();
        plano.instrucoes.clear();
        assert!(
            produzir_e_verificar_tokens_dart(
                &f,
                &mut classes,
                &mut plano,
                &tabelas,
                &PlanoEscopos::default()
            )
            .unwrap_err()
            .contains("canônico")
        );
        assert!(classes.is_empty() && plano.instrucoes.is_empty());
        tabelas.saidas.insert(BlockId(0), SaidaPorExcecao::Guarda);
        let antes = format!("{f:?}");
        assert!(
            inserir_arc_saidas_dart(
                &mut f,
                &mut classes,
                &mut plano,
                &tabelas,
                &PlanoEscopos::default()
            )
            .unwrap_err()
            .contains("CFG explícito")
        );
        assert_eq!(format!("{f:?}"), antes);
        assert!(classes.is_empty() && plano.instrucoes.is_empty());
    }

    #[test]
    fn cleanup_preserva_transferencia_e_retem_antes_de_liberar_outros_tokens() {
        let mut f = Function {
            symbol: "retornos_distintos".into(),
            name: "retornos_distintos".into(),
            depuracao: None,
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
        let mut plano = PlanoTokens {
            retorno: super::super::RetornoTokens::Owned,
            ..Default::default()
        };
        assert_eq!(
            inserir_arc_saidas_dart(
                &mut f,
                &mut classes,
                &mut plano,
                &TabelasDaFuncao::default(),
                &PlanoEscopos::default()
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
        for (inst, esperado) in b.instructions[1..].iter().zip([ValueId(1), ValueId(2)]) {
            assert!(
                matches!(inst.1, Instruction::ArcDrop { value: Operand::Val(v) } if v == esperado)
            );
        }
        let pronta = format!("{f:?}");
        assert_eq!(
            inserir_arc_saidas_dart(
                &mut f,
                &mut classes,
                &mut plano,
                &TabelasDaFuncao::default(),
                &PlanoEscopos::default()
            )
            .unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{f:?}"), pronta);
    }

    #[test]
    fn cleanup_saidas_cobre_erro_runtime_sem_reparar_consumo_duplicado() {
        let original = Function {
            symbol: "cleanup_pending".into(),
            name: "cleanup_pending".into(),
            depuracao: None,
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
        let mut plano = PlanoTokens::default();
        assert_eq!(
            inserir_arc_saidas_dart(
                &mut f,
                &mut classes,
                &mut plano,
                &TabelasDaFuncao::default(),
                &PlanoEscopos::default()
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
        let pronta = format!("{f:?}");
        assert_eq!(
            inserir_arc_saidas_dart(
                &mut f,
                &mut classes,
                &mut plano,
                &TabelasDaFuncao::default(),
                &PlanoEscopos::default()
            )
            .unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{f:?}"), pronta);
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
        plano = PlanoTokens::default();
        let antes = format!("{f:?}");
        assert!(
            inserir_arc_saidas_dart(
                &mut f,
                &mut classes,
                &mut plano,
                &TabelasDaFuncao::default(),
                &PlanoEscopos::default()
            )
            .unwrap_err()
            .contains("indisponível")
        );
        assert_eq!(format!("{f:?}"), antes);
        assert!(classes.is_empty() && plano.instrucoes.is_empty() && plano.pendencias.is_empty());
    }

    #[test]
    fn insercao_retornos_dart_e_idempotente_e_atomica_se_cleanup_falta() {
        let original = Function {
            symbol: "identidade".into(),
            name: "identidade".into(),
            depuracao: None,
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
        let mut plano = PlanoTokens {
            retorno: super::super::RetornoTokens::Owned,
            ..Default::default()
        };
        assert_eq!(
            inserir_retencao_retornos_dart(
                &mut f,
                &mut classes,
                &mut plano,
                &TabelasDaFuncao::default(),
                &PlanoEscopos::default()
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
        let pronta = format!("{f:?}");
        assert_eq!(
            inserir_retencao_retornos_dart(
                &mut f,
                &mut classes,
                &mut plano,
                &TabelasDaFuncao::default(),
                &PlanoEscopos::default()
            )
            .unwrap(),
            0
        );
        assert_eq!(format!("{f:?}"), pronta);
        f = original.clone();
        f.blocks[0].instructions.push((
            ValueId(5),
            Instruction::ArcCopy {
                value: Operand::Val(ValueId(0)),
            },
            Type::Ref,
        ));
        classes.clear();
        plano = PlanoTokens {
            retorno: super::super::RetornoTokens::Owned,
            ..Default::default()
        };
        let antes = format!("{f:?}");
        assert!(
            inserir_retencao_retornos_dart(
                &mut f,
                &mut classes,
                &mut plano,
                &TabelasDaFuncao::default(),
                &PlanoEscopos::default()
            )
            .unwrap_err()
            .contains("não consumidos")
        );
        assert_eq!(format!("{f:?}"), antes);
        assert!(classes.is_empty());
        assert!(plano.instrucoes.is_empty());
        for constante in [
            Constant::Null,
            Constant::String("retorno permanente".into()),
        ] {
            f = original.clone();
            f.blocks[0].terminator = Terminator::Return(Some(Operand::Constant(constante.clone())));
            classes.clear();
            plano = PlanoTokens {
                retorno: super::super::RetornoTokens::Owned,
                ..Default::default()
            };
            let quantidade = inserir_retencao_retornos_dart(
                &mut f,
                &mut classes,
                &mut plano,
                &TabelasDaFuncao::default(),
                &PlanoEscopos::default(),
            )
            .unwrap();
            assert_eq!(
                quantidade,
                usize::from(!matches!(constante, Constant::Null))
            );
        }
    }

    #[test]
    fn wrapper_dart_nao_publica_parametros_quando_transferencia_falha() {
        let mut f = Function {
            symbol: "identidade".into(),
            name: "identidade".into(),
            depuracao: None,
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
        let mut plano = PlanoTokens {
            retorno: super::super::RetornoTokens::Owned,
            ..Default::default()
        };
        assert!(
            produzir_e_verificar_tokens_dart(
                &f,
                &mut classes,
                &mut plano,
                &TabelasDaFuncao::default(),
                &PlanoEscopos::default()
            )
            .is_err()
        );
        assert!(classes.is_empty());
        assert!(plano.instrucoes.is_empty());
        f.blocks[0].instructions[0].1 = Instruction::ArcCopy {
            value: Operand::Val(ValueId(0)),
        };
        produzir_e_verificar_tokens_dart(
            &f,
            &mut classes,
            &mut plano,
            &TabelasDaFuncao::default(),
            &PlanoEscopos::default(),
        )
        .unwrap();
        assert_eq!(
            classes[&ValueId(0)],
            Ownership::Borrowed {
                owner: OrigemOwner::Chamador,
                escopo: 0
            }
        );
        assert_eq!(classes[&ValueId(1)], Ownership::Owned);
        let antes = classes.clone();
        plano.retorno = super::super::RetornoTokens::Borrowed;
        assert!(
            produzir_e_verificar_tokens_dart(
                &f,
                &mut classes,
                &mut plano,
                &TabelasDaFuncao::default(),
                &PlanoEscopos::default()
            )
            .is_err()
        );
        assert_eq!(classes, antes);
        assert_eq!(plano.retorno, super::super::RetornoTokens::Borrowed);
    }

    #[test]
    fn parametros_dart_nao_classificam_i64_e_rejeitam_conflito_atomicamente() {
        let mut f = Function {
            symbol: "parametros".into(),
            name: "parametros".into(),
            depuracao: None,
            params: vec![
                (ValueId(0), "ref".into(), Type::Ref),
                (ValueId(1), "bits".into(), Type::I64),
                (ValueId(2), "outra_ref".into(), Type::Ref),
            ],
            return_ty: Type::Void,
            blocks: vec![],
        };
        let mut classes = HashMap::from([(ValueId(2), Ownership::Owned)]);
        let antes = classes.clone();
        assert!(produzir_parametros_ref_dart(&f, &mut classes).is_err());
        assert_eq!(classes, antes);
        classes.clear();
        produzir_parametros_ref_dart(&f, &mut classes).unwrap();
        assert!(!classes.contains_key(&ValueId(1)));
        assert_eq!(classes.len(), 2);
        let antes = classes.clone();
        f.params.push((ValueId(0), "duplicado".into(), Type::Ref));
        assert!(produzir_parametros_ref_dart(&f, &mut classes).is_err());
        assert_eq!(classes, antes);
    }

    #[test]
    fn produtor_phi_ref_trivial_respeita_transferencia_owned_explicita() {
        let mut f = Function {
            symbol: "phi_trivial_auto".into(),
            name: "phi_trivial_auto".into(),
            depuracao: None,
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
        for retorno in [
            super::super::RetornoTokens::Trivial,
            super::super::RetornoTokens::Owned,
        ] {
            let mut classes = HashMap::new();
            let mut plano = PlanoTokens {
                retorno,
                ..Default::default()
            };
            produzir_e_verificar_tokens(
                &f,
                &mut classes,
                &mut plano,
                &TabelasDaFuncao::default(),
                &PlanoEscopos::default(),
            )
            .unwrap();
            assert_eq!(
                classes[&ValueId(1)],
                if retorno == super::super::RetornoTokens::Owned {
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
        let mut plano = PlanoTokens::default();
        produzir_e_verificar_tokens(
            &f,
            &mut classes,
            &mut plano,
            &TabelasDaFuncao::default(),
            &PlanoEscopos::default(),
        )
        .unwrap();
        assert_eq!(classes[&ValueId(1)], Ownership::Trivial);
    }

    #[test]
    fn produtor_confere_dependencia_pendente_em_ciclo_de_phis_borrowed() {
        let mut f = Function {
            symbol: "ciclo_borrowed".into(),
            name: "ciclo_borrowed".into(),
            depuracao: None,
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
        let inicial = HashMap::from([(
            ValueId(0),
            Ownership::Borrowed {
                owner: OrigemOwner::Chamador,
                escopo: 0,
            },
        )]);
        let mut classes = inicial.clone();
        let mut plano = PlanoTokens {
            retorno: super::super::RetornoTokens::Borrowed,
            ..Default::default()
        };
        produzir_e_verificar_tokens(
            &f,
            &mut classes,
            &mut plano,
            &TabelasDaFuncao::default(),
            &PlanoEscopos::default(),
        )
        .unwrap();
        for v in [ValueId(1), ValueId(2)] {
            assert_eq!(
                classes[&v],
                Ownership::Borrowed {
                    owner: OrigemOwner::Valor(ValueId(0)),
                    escopo: 0
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
        classes = inicial.clone();
        assert!(
            produzir_contratos_arc(&f, &mut classes, &mut plano)
                .unwrap_err()
                .contains("após propagação")
        );
        assert_eq!(classes, inicial);
        assert!(plano.instrucoes.is_empty());
    }

    #[test]
    fn produtor_preserva_emprestimo_em_phi_de_laco_ancorado() {
        let mut f = Function {
            symbol: "phi_borrowed_laco".into(),
            name: "phi_borrowed_laco".into(),
            depuracao: None,
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
                owner: OrigemOwner::Chamador,
                escopo: 0,
            },
        )]);
        let mut plano = PlanoTokens {
            retorno: super::super::RetornoTokens::Borrowed,
            ..Default::default()
        };
        produzir_e_verificar_tokens(
            &f,
            &mut classes,
            &mut plano,
            &TabelasDaFuncao::default(),
            &PlanoEscopos::default(),
        )
        .unwrap();
        assert_eq!(
            classes[&ValueId(1)],
            Ownership::Borrowed {
                owner: OrigemOwner::Valor(ValueId(0)),
                escopo: 0
            }
        );
        classes.remove(&ValueId(1));
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
            incoming[0].1 = Operand::Val(ValueId(1));
        }
        let antes = classes.clone();
        assert!(produzir_contratos_arc(&f, &mut classes, &mut plano).is_err());
        assert_eq!(classes, antes);
        assert!(plano.instrucoes.is_empty());
    }

    #[test]
    fn produtor_preserva_empréstimo_em_cadeia_de_phis_sem_classe_manual() {
        let f = Function {
            symbol: "phis_borrowed".into(),
            name: "phis_borrowed".into(),
            depuracao: None,
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
                owner: OrigemOwner::Chamador,
                escopo: 0,
            },
        )]);
        let mut plano = PlanoTokens {
            retorno: super::super::RetornoTokens::Borrowed,
            ..Default::default()
        };
        produzir_e_verificar_tokens(
            &f,
            &mut classes,
            &mut plano,
            &TabelasDaFuncao::default(),
            &PlanoEscopos::default(),
        )
        .unwrap();
        for v in [ValueId(1), ValueId(2)] {
            assert_eq!(
                classes[&v],
                Ownership::Borrowed {
                    owner: OrigemOwner::Valor(ValueId(0)),
                    escopo: 0
                }
            );
        }
        assert!(plano.instrucoes.is_empty());
    }

    #[test]
    fn phi_ref_trivial_nao_apaga_ownership_das_entradas() {
        let mut f = Function {
            symbol: "phi_ref_trivial".into(),
            name: "phi_ref_trivial".into(),
            depuracao: None,
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
        for classe in [
            Ownership::Owned,
            Ownership::Borrowed {
                owner: OrigemOwner::Chamador,
                escopo: 0,
            },
        ] {
            let mut classes =
                HashMap::from([(ValueId(0), classe), (ValueId(1), Ownership::Trivial)]);
            let antes = classes.clone();
            let mut plano = PlanoTokens::default();
            assert!(produzir_contratos_arc(&f, &mut classes, &mut plano).is_err());
            assert_eq!(classes, antes);
            assert!(plano.instrucoes.is_empty());
        }
        let mut classes = HashMap::from([
            (ValueId(0), Ownership::Trivial),
            (ValueId(1), Ownership::Trivial),
        ]);
        let mut plano = PlanoTokens::default();
        produzir_e_verificar_tokens(
            &f,
            &mut classes,
            &mut plano,
            &TabelasDaFuncao::default(),
            &PlanoEscopos::default(),
        )
        .unwrap();
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
            incoming[0].1 = Operand::Constant(Constant::Null);
        }
        produzir_e_verificar_tokens(
            &f,
            &mut classes,
            &mut plano,
            &TabelasDaFuncao::default(),
            &PlanoEscopos::default(),
        )
        .unwrap();
    }

    #[test]
    fn consultas_record_produzem_escalar_apenas_na_saida_normal() {
        for nome in [
            "dartforge_nativo_DartForge_record_numFields",
            "dartforge_nativo_DartForge_record_shape",
        ] {
            let mut f = Function {
                symbol: "consulta_record".into(),
                name: "consulta_record".into(),
                depuracao: None,
                params: vec![(ValueId(0), "record".into(), Type::Ref)],
                return_ty: Type::I64,
                blocks: vec![
                    BasicBlock {
                        id: BlockId(0),
                        instructions: vec![
                            (
                                ValueId(1),
                                Instruction::CallRuntime {
                                    name: nome.into(),
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
                    owner: OrigemOwner::Chamador,
                    escopo: 0,
                },
            )]);
            let mut plano = PlanoTokens::default();
            produzir_e_verificar_tokens(
                &f,
                &mut classes,
                &mut plano,
                &TabelasDaFuncao::default(),
                &PlanoEscopos::default(),
            )
            .unwrap();
            assert_eq!(classes[&ValueId(1)], Ownership::Trivial);
            assert_eq!(plano.pendencias[&ValueId(1)], BlockId(1));
            assert!(plano.instrucoes[&ValueId(1)].sempre.is_empty());
            let antes = classes.clone();
            let efeitos = plano.instrucoes.clone();
            let pendencias = plano.pendencias.clone();
            f.blocks[1].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
            assert!(
                produzir_e_verificar_tokens(
                    &f,
                    &mut classes,
                    &mut plano,
                    &TabelasDaFuncao::default(),
                    &PlanoEscopos::default()
                )
                .is_err()
            );
            assert_eq!(classes, antes);
            assert_eq!(plano.instrucoes, efeitos);
            assert_eq!(plano.pendencias, pendencias);
        }
    }

    #[test]
    fn aritmetica_tipificada_produz_plano_sem_sementes_e_recusa_ref() {
        let mut f = Function {
            symbol: "aritmetica".into(),
            name: "aritmetica".into(),
            depuracao: None,
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
        let mut plano = PlanoTokens::default();
        produzir_e_verificar_tokens(
            &f,
            &mut classes,
            &mut plano,
            &TabelasDaFuncao::default(),
            &PlanoEscopos::default(),
        )
        .unwrap();
        assert_eq!(classes.len(), 4);
        assert!(classes.values().all(|c| *c == Ownership::Trivial));
        let antes = classes.clone();
        let efeitos = plano.instrucoes.clone();
        f.blocks[0].instructions[1].1 = Instruction::Mul(
            Operand::Constant(Constant::Null),
            Operand::Constant(Constant::Int(6)),
        );
        assert!(produzir_contratos_arc(&f, &mut classes, &mut plano).is_err());
        assert_eq!(classes, antes);
        assert_eq!(plano.instrucoes, efeitos);
    }

    #[test]
    fn phi_ref_nao_aceita_escalar_mesmo_com_classe_owned() {
        let f = Function {
            symbol: "phi_tipo".into(),
            name: "phi_tipo".into(),
            depuracao: None,
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
        let antes = classes.clone();
        let mut plano = PlanoTokens::default();
        assert!(
            produzir_contratos_arc(&f, &mut classes, &mut plano)
                .unwrap_err()
                .contains("representação Ref")
        );
        assert_eq!(classes, antes);
        assert!(plano.instrucoes.is_empty());
        classes.insert(ValueId(0), Ownership::Trivial);
        classes.insert(ValueId(1), Ownership::Trivial);
        let antes = classes.clone();
        assert!(
            produzir_e_verificar_tokens(
                &f,
                &mut classes,
                &mut plano,
                &TabelasDaFuncao::default(),
                &PlanoEscopos::default()
            )
            .unwrap_err()
            .contains("SSA Ref ou null")
        );
        assert_eq!(classes, antes);
    }

    #[test]
    fn bitcast_escalar_exige_origem_e_independe_da_ordem_dos_blocos() {
        let mut f = Function {
            symbol: "bits".into(),
            name: "bits".into(),
            depuracao: None,
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
        super::super::ssa::verificar(&f).unwrap();
        let mut classes = HashMap::new();
        let mut plano = PlanoTokens::default();
        produzir_contratos_arc(&f, &mut classes, &mut plano).unwrap();
        for id in [ValueId(0), ValueId(1), ValueId(2)] {
            assert_eq!(classes[&id], Ownership::Trivial);
            assert_eq!(plano.instrucoes[&id], EfeitoTokens::default());
        }
        let mut modulo = Module::default();
        modulo.memoria_arc = true;
        modulo.functions.push(f.clone());
        let ir = crate::llvm::LlvmEmitter::new(&modulo).emit_all();
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
        for (tipo, classe) in [
            (Type::Ref, Some(Ownership::Trivial)),
            (Type::Ptr, Some(Ownership::Trivial)),
            (Type::I64, Some(Ownership::Owned)),
            (
                Type::I64,
                Some(Ownership::Borrowed {
                    owner: OrigemOwner::Chamador,
                    escopo: 0,
                }),
            ),
            (Type::I64, None),
            (Type::F64, Some(Ownership::Trivial)),
        ] {
            f.params = vec![(ValueId(0), "origem".into(), tipo)];
            let mut classes: HashMap<_, _> = classe.map(|c| (ValueId(0), c)).into_iter().collect();
            let antes = classes.clone();
            let mut plano = PlanoTokens::default();
            assert!(produzir_contratos_arc(&f, &mut classes, &mut plano).is_err());
            assert_eq!(classes, antes);
            assert!(plano.instrucoes.is_empty());
        }
        f.params = vec![(ValueId(0), "bits".into(), Type::I64)];
        let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
        produzir_contratos_arc(&f, &mut classes, &mut PlanoTokens::default()).unwrap();
        // Anotação divergente do destino também falha sem publicar o resultado.
        f.blocks[1].instructions[0].2 = Type::I64;
        let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
        let antes = classes.clone();
        assert!(produzir_contratos_arc(&f, &mut classes, &mut PlanoTokens::default()).is_err());
        assert_eq!(classes, antes);
    }

    #[test]
    fn aritmetica_nao_apaga_ownership_antes_do_bitcast() {
        let f = Function {
            symbol: "origem".into(),
            name: "origem".into(),
            depuracao: None,
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
        for classe in [
            None,
            Some(Ownership::Owned),
            Some(Ownership::Borrowed {
                owner: OrigemOwner::Chamador,
                escopo: 0,
            }),
        ] {
            let mut classes: HashMap<_, _> = classe.map(|c| (ValueId(0), c)).into_iter().collect();
            let antes = classes.clone();
            let mut plano = PlanoTokens::default();
            assert!(
                produzir_contratos_arc(&f, &mut classes, &mut plano)
                    .unwrap_err()
                    .contains("origem Trivial")
            );
            assert_eq!(classes, antes);
            assert!(plano.instrucoes.is_empty());
        }
        produzir_contratos_arc(
            &f,
            &mut HashMap::from([(ValueId(0), Ownership::Trivial)]),
            &mut PlanoTokens::default(),
        )
        .unwrap();
    }

    #[test]
    fn larguras_inteiras_preservam_contrato_escalar_e_rejeitam_handles() {
        for (from, to, ampliar) in [
            (Type::I1, Type::I8, true),
            (Type::I1, Type::I64, true),
            (Type::I8, Type::I64, true),
            (Type::I64, Type::I8, false),
            (Type::I64, Type::I1, false),
            (Type::I8, Type::I1, false),
        ] {
            let inst = if ampliar {
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
                depuracao: None,
                params: vec![(ValueId(0), "x".into(), from)],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(ValueId(1), inst, to)],
                    terminator: Terminator::Return(None),
                }],
            };
            let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
            let mut plano = PlanoTokens::default();
            produzir_e_verificar_tokens(
                &f,
                &mut classes,
                &mut plano,
                &TabelasDaFuncao::default(),
                &PlanoEscopos::default(),
            )
            .unwrap();
            assert_eq!(classes[&ValueId(1)], Ownership::Trivial);
            assert_eq!(plano.instrucoes[&ValueId(1)], EfeitoTokens::default());
            for (tipo, classe) in [
                (Type::Ref, Some(Ownership::Trivial)),
                (Type::Ptr, Some(Ownership::Trivial)),
                (from, Some(Ownership::Owned)),
                (
                    from,
                    Some(Ownership::Borrowed {
                        owner: OrigemOwner::Chamador,
                        escopo: 0,
                    }),
                ),
                (from, None),
            ] {
                f.params[0].2 = tipo;
                let mut classes: HashMap<_, _> =
                    classe.map(|c| (ValueId(0), c)).into_iter().collect();
                let antes = classes.clone();
                let mut plano = PlanoTokens::default();
                assert!(produzir_contratos_arc(&f, &mut classes, &mut plano).is_err());
                assert_eq!(classes, antes);
                assert!(plano.instrucoes.is_empty());
            }
            f.params[0].2 = from;
            // A operação oposta com as mesmas larguras é inválida.
            f.blocks[0].instructions[0].1 = if ampliar {
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
            assert!(produzir_contratos_arc(&f, &mut classes, &mut PlanoTokens::default()).is_err());
            assert_eq!(classes.len(), 1);
        }
        // Sequência do transporte de bool por byte e palavra, com boxing
        // permanente no fim: nenhum token Owned nem saída pending é criado.
        let f = Function {
            symbol: "bool_bits".into(),
            name: "bool_bits".into(),
            depuracao: None,
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
        let mut plano = PlanoTokens::default();
        produzir_e_verificar_tokens(
            &f,
            &mut classes,
            &mut plano,
            &TabelasDaFuncao::default(),
            &PlanoEscopos::default(),
        )
        .unwrap();
        assert_eq!(classes.len(), 5);
        assert!(classes.values().all(|c| *c == Ownership::Trivial));
        assert!(plano.pendencias.is_empty());
        let mut modulo = Module::default();
        modulo.memoria_arc = true;
        modulo.functions.push(f);
        let ir = crate::llvm::LlvmEmitter::new(&modulo).emit_all();
        assert!(ir.contains("zext i1 true to i8"));
        assert!(ir.contains("zext i8 %v0 to i64"));
        assert!(ir.contains("trunc i64 %v1 to i8"));
        assert!(ir.contains("trunc i8 %v2 to i1"));
    }

    #[test]
    fn verificador_recusa_operacao_pura_com_metadados_falseados() {
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
                depuracao: None,
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
                        owner: OrigemOwner::Chamador,
                        escopo: 0,
                    },
                ),
                (ValueId(1), Ownership::Trivial),
            ]);
            let mut plano = PlanoTokens::default();
            plano.instrucoes.insert(ValueId(1), EfeitoTokens::default());
            assert!(
                verificar_tokens(&f, &classes, &TabelasDaFuncao::default(), &plano).is_err(),
                "{:?}",
                f.blocks[0].instructions[0].1
            );
        }
    }

    #[test]
    fn operacao_pura_nao_reinterpreta_referencia_como_escalar() {
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
                depuracao: None,
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
                    owner: OrigemOwner::Chamador,
                    escopo: 0,
                },
            )]);
            let antes = classes.clone();
            let mut plano = PlanoTokens::default();
            assert!(
                produzir_contratos_arc(&f, &mut classes, &mut plano)
                    .unwrap_err()
                    .contains("escalares já avaliados")
            );
            assert_eq!(classes, antes);
            assert!(plano.instrucoes.is_empty());
        }
    }

    #[test]
    fn constantes_tipadas_e_literais_permanentes_nao_exigem_seed_manual() {
        let constantes = [
            Constant::Int(7),
            Constant::Double(1.0),
            Constant::Bool(true),
            Constant::Null,
            Constant::String("literal".into()),
            Constant::StringWtf8(vec![120]),
        ];
        let tipos = [
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
            depuracao: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: constantes
                    .into_iter()
                    .zip(tipos)
                    .enumerate()
                    .map(|(i, (k, t))| (ValueId(i as u32), Instruction::Const(k), t))
                    .collect(),
                terminator: Terminator::Return(None),
            }],
        };
        let mut classes = HashMap::new();
        let mut plano = PlanoTokens::default();
        f.blocks[0].instructions[0].2 = Type::Ref;
        assert!(produzir_contratos_arc(&f, &mut classes, &mut plano).is_err());
        assert!(classes.is_empty() && plano.instrucoes.is_empty());
        f.blocks[0].instructions[0].2 = Type::I64;
        produzir_e_verificar_tokens(
            &f,
            &mut classes,
            &mut plano,
            &TabelasDaFuncao::default(),
            &PlanoEscopos::default(),
        )
        .unwrap();
        assert_eq!(classes.len(), 6);
        assert!(classes.values().all(|c| *c == Ownership::Trivial));
    }

    #[test]
    fn phi_bool_de_laco_e_produzido_sem_inventar_owner() {
        let mut f = Function {
            symbol: "phi_bool".into(),
            name: "phi_bool".into(),
            depuracao: None,
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
        let mut plano = PlanoTokens::default();
        produzir_e_verificar_tokens(
            &f,
            &mut classes,
            &mut plano,
            &TabelasDaFuncao::default(),
            &PlanoEscopos::default(),
        )
        .unwrap();
        assert_eq!(classes[&ValueId(1)], Ownership::Trivial);
        assert!(!plano.instrucoes.contains_key(&ValueId(1)));
        let antes = classes.clone();
        let efeitos = plano.instrucoes.clone();
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
            incoming[0].1 = Operand::Constant(Constant::Int(1));
        }
        assert!(produzir_contratos_arc(&f, &mut classes, &mut plano).is_err());
        assert_eq!(classes, antes);
        assert_eq!(plano.instrucoes, efeitos);
    }

    #[test]
    fn phi_numerico_exige_origem_e_contrato_semantico() {
        for (ty, constante) in [
            (Type::I1, Constant::Bool(true)),
            (Type::I64, Constant::Int(7)),
            (Type::F64, Constant::Double(7.0)),
        ] {
            let mut f = Function {
                symbol: "phi_escalar".into(),
                name: "phi_escalar".into(),
                depuracao: None,
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
                                    (BlockId(0), Operand::Constant(constante)),
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
            let mut plano = PlanoTokens::default();
            produzir_e_verificar_tokens(
                &f,
                &mut classes,
                &mut plano,
                &TabelasDaFuncao::default(),
                &PlanoEscopos::default(),
            )
            .unwrap();
            assert_eq!(classes[&ValueId(1)], Ownership::Trivial);
            assert!(plano.instrucoes.is_empty());
            // Nem um contrato previamente fornecido funda um ciclo fechado.
            if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
                incoming[0].1 = Operand::Val(ValueId(1));
            }
            let antes = classes.clone();
            assert!(
                produzir_contratos_arc(&f, &mut classes, &mut plano)
                    .unwrap_err()
                    .contains("sem origem escalar")
            );
            assert_eq!(classes, antes);
            // Um parâmetro de mesma largura precisa de contrato explícito.
            f.params.push((ValueId(0), "entrada".into(), ty));
            if let Instruction::Phi { incoming, .. } = &mut f.blocks[1].instructions[0].1 {
                incoming[0].1 = Operand::Val(ValueId(0));
            }
            assert!(produzir_contratos_arc(&f, &mut classes, &mut plano).is_err());
            classes.insert(ValueId(0), Ownership::Owned);
            let antes = classes.clone();
            assert!(produzir_contratos_arc(&f, &mut classes, &mut plano).is_err());
            assert_eq!(classes, antes);
            assert!(plano.instrucoes.is_empty());
            classes.insert(ValueId(0), Ownership::Trivial);
            produzir_e_verificar_tokens(
                &f,
                &mut classes,
                &mut plano,
                &TabelasDaFuncao::default(),
                &PlanoEscopos::default(),
            )
            .unwrap();
        }
    }

    #[test]
    fn comparacao_produz_bool_sem_consumir_referencia() {
        let mut f = Function {
            symbol: "cmp".into(),
            name: "cmp".into(),
            depuracao: None,
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
        let inicial = HashMap::from([(
            ValueId(0),
            Ownership::Borrowed {
                owner: OrigemOwner::Chamador,
                escopo: 0,
            },
        )]);
        let mut classes = inicial.clone();
        let mut plano = PlanoTokens::default();
        f.blocks[0].instructions[0].2 = Type::Ref;
        assert!(produzir_contratos_arc(&f, &mut classes, &mut plano).is_err());
        assert_eq!(classes, inicial);
        assert!(plano.instrucoes.is_empty());
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
        produzir_e_verificar_tokens(
            &f,
            &mut classes,
            &mut plano,
            &TabelasDaFuncao::default(),
            &PlanoEscopos::default(),
        )
        .unwrap();
        assert_eq!(classes[&ValueId(1)], Ownership::Trivial);
        assert_eq!(plano.instrucoes[&ValueId(1)], EfeitoTokens::default());
        plano
            .instrucoes
            .get_mut(&ValueId(1))
            .unwrap()
            .sempre
            .push(ValueId(0));
        let antes = plano.instrucoes.clone();
        assert!(produzir_contratos_arc(&f, &mut classes, &mut plano).is_err());
        assert_eq!(plano.instrucoes, antes);
    }

    #[test]
    fn excecao_owned_tem_resultado_ref_independente_sem_falha_nova() {
        for nome in [
            "dartforge_arc_excecao_owned_v1",
            "dartforge_arc_rastro_owned_v1",
        ] {
            let i = Instruction::CallRuntime {
                name: nome.into(),
                args: vec![],
                ret_ty: Type::Ref,
            };
            let c = contrato_chamada_runtime(&i).unwrap();
            assert_eq!(c.resultado, Ownership::Owned);
            assert!(!c.efeito.pode_falhar);
            assert!(c.efeito.sempre.is_empty());
            let mut escalar = i.clone();
            if let Instruction::CallRuntime { ret_ty, .. } = &mut escalar {
                *ret_ty = Type::I64;
            }
            assert!(contrato_chamada_runtime(&escalar).is_err());
        }
    }

    #[test]
    fn lancamento_com_rastro_exige_dois_borrows_ref_sem_consumir_tokens() {
        let mut i = Instruction::CallRuntime {
            name: "dartforge_arc_lancar_com_rastro_ref_v1".into(),
            args: vec![
                (Operand::Val(ValueId(0)), Type::Ref),
                (Operand::Val(ValueId(1)), Type::Ref),
            ],
            ret_ty: Type::Void,
        };
        let c = contrato_chamada_runtime(&i).unwrap();
        assert!(c.efeito.pode_falhar);
        assert!(c.efeito.sempre.is_empty());
        assert!(c.efeito.sucesso.is_empty());
        assert!(c.efeito.erro.is_empty());
        if let Instruction::CallRuntime { args, .. } = &mut i {
            args[1].1 = Type::I64;
        }
        assert!(contrato_chamada_runtime(&i).is_err());
    }

    #[test]
    fn constantes_runtime_preservam_distincao_entre_escalar_e_endereco() {
        for k in [
            Constant::Bool(true),
            Constant::Double(1.0),
            Constant::Null,
            Constant::String("x".into()),
            Constant::StringWtf8(vec![120]),
            Constant::Funcao("getter".into()),
        ] {
            let i = Instruction::CallRuntime {
                name: "dartforge_arc_quadro_abrir_v1".into(),
                args: vec![(Operand::Constant(k), Type::I64)],
                ret_ty: Type::I64,
            };
            assert!(contrato_chamada_runtime(&i).is_err());
        }
        let mut i = Instruction::CallRuntime {
            name: "dartforge_marcar_constante".into(),
            args: vec![
                (Operand::Constant(Constant::Null), Type::Ref),
                (
                    Operand::Constant(Constant::Funcao("getter".into())),
                    Type::I64,
                ),
            ],
            ret_ty: Type::Void,
        };
        contrato_chamada_runtime(&i).unwrap();
        if let Instruction::CallRuntime { args, .. } = &mut i {
            args[1].0 = Operand::Constant(Constant::Bool(false));
        }
        assert!(contrato_chamada_runtime(&i).is_err());
    }

    #[test]
    fn contratos_nao_inferem_ref_por_largura_nem_escondem_tokens() {
        let mut i = Instruction::CallRuntime {
            name: "dartforge_arc_global_receber_v1".into(),
            args: vec![
                (Operand::Val(ValueId(0)), Type::I64),
                (Operand::Val(ValueId(1)), Type::Ref),
            ],
            ret_ty: Type::Void,
        };
        let c = contrato_chamada_runtime(&i).unwrap();
        assert_eq!(c.efeito.sempre, [ValueId(1)]);
        assert!(c.retencao_persistente && c.invalida_borrows);
        if let Instruction::CallRuntime { args, .. } = &mut i {
            args[1].1 = Type::I64;
        }
        assert!(contrato_chamada_runtime(&i).is_err());
        if let Instruction::CallRuntime { args, .. } = &mut i {
            args[1] = (Operand::Constant(Constant::Null), Type::Ref);
        }
        assert!(
            contrato_chamada_runtime(&i)
                .unwrap()
                .efeito
                .sempre
                .is_empty()
        );
        for nome in ["dartforge_arc_retain", "dartforge_alocar"] {
            let i = Instruction::CallRuntime {
                name: nome.into(),
                args: vec![],
                ret_ty: Type::Void,
            };
            assert!(contrato_chamada_runtime(&i).is_err());
        }
    }

    #[test]
    fn byte_da_abi_nao_e_booleano_i1() {
        let mut i = Instruction::CallRuntime {
            name: "dartforge_arc_verificar_abi".into(),
            args: vec![(Operand::Constant(Constant::Int(1)), Type::I64)],
            ret_ty: Type::I8,
        };
        assert_eq!(
            contrato_chamada_runtime(&i).unwrap().resultado,
            Ownership::Trivial
        );
        if let Instruction::CallRuntime { ret_ty, .. } = &mut i {
            *ret_ty = Type::I1;
        }
        assert!(contrato_chamada_runtime(&i).is_err());
    }

    #[test]
    fn publicar_global_copia_owner_sem_consumir_ssa() {
        let i = Instruction::CallRuntime {
            name: "dartforge_gc_global_root".into(),
            args: vec![
                (Operand::Val(ValueId(0)), Type::I64),
                (Operand::Val(ValueId(1)), Type::Ref),
            ],
            ret_ty: Type::Void,
        };
        let c = contrato_chamada_runtime(&i).unwrap();
        assert!(c.efeito.sempre.is_empty() && c.retencao_persistente && c.invalida_borrows);
        assert_eq!(c.resultado, Ownership::Trivial);
    }

    #[test]
    fn coleta_auditada_exige_saida_excepcional() {
        let i = Instruction::CallRuntime {
            name: "dartforge_gc_collect".into(),
            args: vec![],
            ret_ty: Type::Void,
        };
        let c = contrato_chamada_runtime(&i).unwrap();
        assert!(c.efeito.pode_falhar && c.invalida_borrows);
        assert!(
            c.efeito.sempre.is_empty() && c.efeito.sucesso.is_empty() && c.efeito.erro.is_empty()
        );
    }

    #[test]
    fn produtor_confere_tipo_ssa_real_antes_de_publicar() {
        let mut f = Function {
            symbol: "tipo_real".into(),
            name: "tipo_real".into(),
            depuracao: None,
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
        let inicial = classes.clone();
        let mut plano = PlanoTokens::default();
        let erro = produzir_contratos_runtime(&f, &mut classes, &mut plano).unwrap_err();
        assert!(
            erro.contains("argumento 0") && erro.contains("v0"),
            "{erro}"
        );
        assert_eq!(classes, inicial);
        assert!(plano.instrucoes.is_empty());
        f.params.clear();
        assert!(produzir_contratos_runtime(&f, &mut classes, &mut plano).is_err());
        assert_eq!(classes, inicial);
        assert!(plano.instrucoes.is_empty());
        f.params.push((ValueId(0), "quadro".into(), Type::I64));
        produzir_contratos_runtime(&f, &mut classes, &mut plano).unwrap();
        assert_eq!(classes[&ValueId(1)], Ownership::Owned);
    }

    #[test]
    fn produtor_e_atomico_e_produz_owned_sem_inferir_pela_largura() {
        let mut f = Function {
            symbol: "f".into(),
            name: "f".into(),
            depuracao: None,
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
        let mut plano = PlanoTokens::default();
        f.blocks[0].instructions.push((
            ValueId(1),
            Instruction::CallRuntime {
                name: "extern_sem_contrato".into(),
                args: vec![],
                ret_ty: Type::Void,
            },
            Type::Void,
        ));
        assert!(produzir_contratos_runtime(&f, &mut classes, &mut plano).is_err());
        assert!(classes.is_empty() && plano.instrucoes.is_empty());
        f.blocks[0].instructions.pop();
        produzir_contratos_runtime(&f, &mut classes, &mut plano).unwrap();
        assert_eq!(classes[&ValueId(0)], Ownership::Owned);
        classes.insert(ValueId(0), Ownership::Trivial);
        let antes = plano.instrucoes.clone();
        assert!(produzir_contratos_runtime(&f, &mut classes, &mut plano).is_err());
        assert_eq!(classes[&ValueId(0)], Ownership::Trivial);
        assert_eq!(plano.instrucoes, antes);
    }
}
