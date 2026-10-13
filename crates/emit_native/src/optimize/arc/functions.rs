//! Preparação atômica de um conjunto fechado de funções com ABI Dart.
//! Metadados e CFG excepcional devem corresponder aos corpos após otimização.

use super::*;
use std::collections::{HashMap, HashSet};

/// Metadados de uma função antes da inserção ARC nas saídas Dart.
///
/// Contratos não cobertos pelos produtores e limites lexicais são premissas
/// explícitas. O retorno Ref exige `tokens.ret = TokenReturn::Owned`.
/// Tabelas descrevem o CFG preparado, antes de materializar a emissão LLVM.
///
/// ```
/// use dartforge_emit_native::optimize::arc::{PlanoFuncaoDart, RetornoTokens};
/// let mut plano = PlanoFuncaoDart::default();
/// plano.tokens.retorno = RetornoTokens::Owned;
/// assert!(plano.classes.is_empty());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DartFunctionPlan {
    /// Classificações semânticas fornecidas; os produtores completam cobertura.
    pub classes: HashMap<ValueId, Ownership>,
    /// Consumos e convenção de retorno da mesma versão do CFG.
    pub tokens: TokenPlan,
    /// Invokes/pousos excepcionais preparados para a função.
    pub tables: FunctionTables,
    /// Limites lexicais dos empréstimos, sem inferência por último uso.
    pub scopes: ScopePlan,
    /// Inventário validado após inserção; inclui somente owners da aresta de erro.
    /// Recalculado atomicamente e ligado ao certificado de cleanup estrangeiro.
    pub owners_at_landing_pad: HashMap<BlockId, Vec<ValueId>>,
}

/// Insere e verifica retornos/cleanup de um conjunto fechado de funções Dart.
///
/// Cria contratos provisórios das chamadas internas e confere todos os corpos
/// antes de publicar qualquer função ou mapa. Usa o resumo de exceções do
/// pipeline, incluindo ciclos de chamadas e conferência de pilha. A ordem
/// das funções não define a disponibilidade dos contratos.
///
/// Exige um plano por símbolo e somente callees do conjunto. Argumentos
/// passam pela classificação ARC após classificar resultados das chamadas:
/// constantes/aritmética/Phis/runtime cobertos não exigem mapas manuais.
/// Boxing escalar explícito vira fábrica Owned para int/double; bool usa caixas
/// estáticas Trivial. SIMD e outras representações exigem produtores próprios.
/// Unbox int/double vira chamada Borrow falível auditada; exige saída de erro.
/// Unbox bool conserva o helper LLVM que converte u8/I1, com o mesmo contrato
/// Borrow falível e sua checagem de pendência, sem mudar IDs do CFG.
/// Parâmetros F64/I1/I8 são escalares Trivial. I64/Ptr e operações não
/// cobertas exigem contratos prévios; largura não prova semântica gerenciada.
/// Separa saídas Guarda em sucesso/erro e transporta os limites de saída.
/// Demais CFG/escopos precisam estar preparados. Não divide arestas gerais,
/// resolve finally/cancelamento/suspensão nem materializa tabelas.
/// Descritores de slots e proveniência continuam premissas do lowering.
/// Pousos catch ganham inventário certificado para cleanup estrangeiro Unix;
/// quadros locais são fechados em LIFO nesse braço, segundo a pilha validada.
/// Aliases e quadros importados permanecem fora desse protocolo.
///
/// # Erros
/// Símbolo repetido, plano ausente/obsoleto ou falha de produção/inserção/
/// verificação de qualquer corpo. Todos os corpos e mapas ficam intactos.
/// Retorna os totais de (retenções de retorno, liberações nas saídas).
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let mut funcoes = vec![Function { symbol: "identidade".into(), name: "identidade".into(),
///     depuracao: None, params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] }];
/// let mut plano = PlanoFuncaoDart::default();
/// plano.tokens.retorno = RetornoTokens::Owned;
/// let mut planos = HashMap::from([("identidade".into(), plano)]);
/// assert_eq!(inserir_arc_funcoes_dart(&mut funcoes, &mut planos)?, (1, 0));
/// # Ok::<(), String>(())
/// ```
pub fn insert_arc_dart_functions(
    functions: &mut [Function],
    plans: &mut HashMap<String, DartFunctionPlan>,
) -> Result<(usize, usize), String> {
    insert(functions, plans, false, None, &HashMap::new(), &HashMap::new(), &HashSet::new(), true, false)
}

/// Prepara invokes ausentes e insere ARC num conjunto fechado de funções Dart.
///
/// Chamadas diretas falíveis sem pouso fornecido ganham uma continuação
/// normal e um pouso que libera owners. Em Unix, propaga por Retoma usando
/// o objeto Itanium original; em Windows, usa Lanca até a integração SEH.
/// Um sítio já preparado conserva seu tratador. Phis e limites lexicais
/// seguem o terminador original; a saída excepcional fecha os escopos ativos.
/// Conferência de pendência logo após chamada exige sítio preparado, pois
/// seu desvio pode representar catch/finally que não pode ser omitido.
/// Não cria catch/finally nem fecha quadros proprietários abertos: esses
/// protocolos precisam de cleanup explícito. Também não cobre dispatch
/// indireto, cancelamento, suspensão ou classificação semântica do lowering.
/// Conferência de pilha por quadros de raízes calculados por vivacidade
/// exige a marca `tables.checks_stack` enquanto não integrar esse inventário.
/// Os demais contratos são os de [`insert_arc_dart_functions`].
/// Protege resultados Ref borrowed de externs auditadas quando uma barreira
/// exige Owned, preservando IDs dos usos. Em chamadas falíveis, retém somente
/// na aresta de sucesso de uma guarda de pendência conferida. Phis protegem
/// todas as origens emprestadas antes de reconstruir a transferência Owned.
/// Divide arestas normais para liberar tokens mortos por vivacidade, inclusive
/// nas voltas de laços, preservando transferências Phi e dependências de borrows.
/// Release pode exigir outro keepalive; a preparação repete até estabilizar.
/// Extensões de vida semânticas precisam estar materializadas como usos na HIR;
/// não infere limites de Finalizable, capturas ou suspensão por ausência de uso.
/// Origens não auditadas e cleanup em arestas excepcionais/pousos continuam
/// exigindo preparação explícita; nenhum plano parcial é publicado.
/// Retorna (retenções de retorno/keepalive, liberações em arestas/saídas).
///
/// # Erros
/// CFG/SSA ou limites inválidos, IDs esgotados ou qualquer erro de produção
/// e verificação ARC. Nenhum corpo ou plano é publicado em caso de erro.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let folha = Function { symbol: "folha".into(), name: "folha".into(), depuracao: None,
///     params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
///     id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
/// let mut caller = folha.clone();
/// caller.symbol = "caller".into();
/// caller.blocks[0].instructions.push((ValueId(0), Instruction::CallStatic {
///     symbol: "folha".into(), args: vec![], ret_ty: Type::Void }, Type::Void));
/// let mut p = PlanoFuncaoDart::default();
/// p.tabelas.confere_pilha = true;
/// let mut planos = HashMap::from([("folha".into(), p),
///     ("caller".into(), PlanoFuncaoDart::default())]);
/// preparar_arc_funcoes_dart(&mut [caller, folha], &mut planos)?;
/// assert_eq!(planos["caller"].tabelas.invocacoes.len(), 1);
/// # Ok::<(), String>(())
/// ```
pub fn prepare_arc_dart_functions(
    functions: &mut [Function],
    plans: &mut HashMap<String, DartFunctionPlan>,
) -> Result<(usize, usize), String> {
    prepare_functions(functions, plans, false)
}

/// [`prepare_arc_dart_functions`], aceitando callees de outra imagem
/// preparada quando `externals` (`Module::arc_external_callees`).
pub(super) fn prepare_functions(
    functions: &mut [Function],
    plans: &mut HashMap<String, DartFunctionPlan>,
    externals: bool,
) -> Result<(usize, usize), String> {
    insert(functions, plans, true, None, &HashMap::new(), &HashMap::new(), &HashSet::new(), true, externals)
}

/// Prepara atomicamente os fatos do lowering e o conjunto fechado de um módulo ARC.
///
/// Tracing devolve (0, 0) sem tocar corpos ou planos. Em ARC, produz parâmetros
/// escalares/RTI nas cópias dos planos e prepara/verifica todas as funções antes
/// de publicar. Os planos devem descrever o CFG final após as otimizações.
/// Confere também os layouts de campos declarados fornecidos pelo lowering:
/// classe única, quantidade coerente e representações válidas. Isso não prova
/// o tipo/forma física dos receivers nem a versão/pins de recarga.
/// AllocObject com layout completo vira fábrica Owned e setters auditados:
/// Ref mantém uma aresta; escalares gravam bits, com bitcast de double e
/// extensão sem sinal de bool/byte. Literais em argumentos precisam estar
/// materializados na HIR. A fábrica zerada object_new, com classe/quantidade
/// constantes e layout válido, também ganha token Owned. Inicializadores,
/// RTI e construtor continuam nas operações seguintes. A emissão conserva
/// o registro preguiçoso da tabela de métodos pela fábrica Owned específica.
/// A alocação zerada não certifica os campos ainda em construção.
/// GetField/SetField de alocações locais não publicadas escolhem as ABIs
/// auditadas pela representação de todos os layouts candidatos. Cópias,
/// moves e Phis conservam conjuntos de origens, inclusive entradas opacas;
/// escalar estreito usa conversão de bits explícita e mantém o ID do uso.
/// Publicação/contenção, chamada sem resumo fechado e layout ambíguo recusam
/// a prova; não certifica receivers recebidos por parâmetro ou guardas late.
/// Fatos nominais do lowering produzem a convenção Owned dos retornos Ref
/// de corpos Dart. Outros produtores continuam exigindo essa convenção
/// explícita; operações não cobertas exigem contratos próprios.
/// Se `excecoes_por_tabelas` já estiver selecionado, publica também
/// as tabelas verificadas, na ordem final das funções, na mesma transação.
/// Não seleciona esse modo nem chama este passe no pipeline padrão.
/// Demais limites são os de [`prepare_arc_dart_functions`].
///
/// # Erros
/// Fatos inválidos ou qualquer erro de preparação do conjunto. Nenhum corpo
/// ou plano é publicado, incluindo fatos de parâmetros previamente válidos.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::{HashMap, HashSet};
/// let mut m = Module::new(); m.memoria_arc = true;
/// m.functions.push(Function { symbol: "caixa".into(), name: "caixa".into(),
///     depuracao: None, params: vec![(ValueId(0), "numero".into(), Type::I64)],
///     return_ty: Type::Void, blocks: vec![BasicBlock { id: BlockId(0),
///     instructions: vec![(ValueId(1), Instruction::Box { op: Operand::Val(ValueId(0)),
///     from: Type::I64 }, Type::Ref)], terminator: Terminator::Return(None) }] });
/// m.parametros_escalares_dart.insert("caixa".into(), HashSet::from([ValueId(0)]));
/// let mut planos = HashMap::from([("caixa".into(), PlanoFuncaoDart::default())]);
/// assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos)?, (0, 1));
/// # Ok::<(), String>(())
/// ```
pub fn prepare_arc_dart_module(
    module: &mut Module,
    plans: &mut HashMap<String, DartFunctionPlan>,
) -> Result<(usize, usize), String> {
    prepare_with_variants(module, plans, &HashMap::new())
}

// Cada alias é um corpo real de uma definição compartilhada. Chamadas ao
// símbolo público precisam considerar falhas de qualquer variante verificada.
pub(super) fn prepare_with_variants(
    module: &mut Module,
    plans: &mut HashMap<String, DartFunctionPlan>,
    variants: &HashMap<String, Vec<String>>,
) -> Result<(usize, usize), String> {
    if !module.arc_memory {
        return Ok((0, 0));
    }
    let mut origin = module.clone();
    // Os acessos por endereço derivado de lista seguram a lista até ali.
    for f in &mut origin.functions {
        super::derived_set::keep_live(f)?;
    }
    let mut new_set = plans.clone();
    super::cfg_classes::prepare(&mut origin, &mut new_set)?;
    let mut slots = super::lower_shared_ref_slots(&origin)?;
    let mut work = origin.clone();
    work.functions = slots.functions.clone();
    super::slot_plans::remap_facts(&mut work, &slots.symbols);
    super::slot_plans::remap_plans(&mut new_set, &slots.symbols);
    super::produce_cfg_ref_slot_plans(&origin, &slots, &mut new_set)?;
    let variants: HashMap<_, _> = variants.iter().map(|(s, vs)| (
        slots.symbols.get(s).cloned().unwrap_or_else(|| s.clone()),
        vs.iter().map(|v| slots.symbols.get(v).cloned().unwrap_or_else(|| v.clone())).collect::<Vec<_>>()
    )).collect();
    for (s, vs) in &variants {
        let signature = |name_str: &str| work.functions.iter().find(|f| f.symbol == name_str)
            .map(|f| (f.params.iter().map(|(_, _, t)| *t).collect::<Vec<_>>(), f.return_ty));
        let base = signature(s);
        if vs.iter().any(|v| signature(v) != base) {
            return Err(format!("slots ARC: ABI incompatível em variantes de {s}"));
        }
    }
    // Convenções novas só são publicadas junto à preparação completa.
    let getters_rti = super::rti_caches::getters_rti(&work, &new_set);
    super::selectors::check(&work, &getters_rti)?;
    super::globals::prepare(&mut work)?;
    let mut prepared_set = work.functions.clone();
    super::selectors::prepare(&mut prepared_set);
    super::layouts::verify(&work)?;
    produce_lowering_scalar_parameters(&work, &mut new_set)?;
    super::locals::produce_addresses(&work, &mut new_set)?;
    super::vectors::produce(&work, &mut new_set)?;
    super::vectors::prepare_reads(&work, &mut prepared_set)?;
    super::rti_caches::produce(&work, &mut new_set)?;
    super::globals::produce_scalars(&work, &mut new_set)?;
    produce_lowering_rti_parameters(&work, &mut new_set)?;
    produce_lowering_ref_returns(&work, &mut new_set)?;
    // O conjunto já mantém os corpos privados até verificar todos eles.
    // Após seu sucesso, a atribuição dos planos não introduz nova falha.
    let receivers = super::receivers::produce(&work);
    let preserve_fields = super::rti_caches::preserve_fields(&work, &new_set);
    // Os layouts do SDK preparado valem para conferir os campos de objetos
    // dele (os do módulo prevalecem).
    let mut layouts = work.arc_external_layouts.clone();
    layouts.extend(module.arc_field_layouts.iter().map(|(c, l)| (*c, l.clone())));
    let total = insert(
        &mut prepared_set,
        &mut new_set,
        true,
        Some(&layouts),
        &variants,
        &receivers,
        &preserve_fields,
        work.excecoes_por_tabelas,
        work.arc_external_callees,
    )?;
    slots.functions = prepared_set.clone();
    super::verify_cfg_ref_slot_lives(&origin, &slots)?;
    work.functions = prepared_set;
    // Os IDs de invokes/pousos e os certificados pertencem aos corpos novos.
    // Não deixar o emissor consultar um inventário da versão anterior do CFG.
    if module.excecoes_por_tabelas {
        work.tables = work
            .functions
            .iter()
            .map(|f| new_set[&f.symbol].tables.clone())
            .collect();
    }
    work.arc_prepared = true;
    *module = work;
    *plans = new_set;
    Ok(total)
}

fn insert(
    functions: &mut [Function],
    plans: &mut HashMap<String, DartFunctionPlan>,
    prepare_calls: bool,
    layouts: Option<&HashMap<u32, Vec<Type>>>,
    variants: &HashMap<String, Vec<String>>,
    receivers: &HashMap<String, HashMap<ValueId, u32>>,
    preserve_fields: &HashSet<String>,
    by_tables: bool,
    externals: bool,
) -> Result<(usize, usize), String> {
    let mut symbols = HashSet::new();
    for f in functions.iter() {
        if !symbols.insert(f.symbol.clone()) {
            return Err(format!("função Dart repetida: {}", f.symbol));
        }
        if !plans.contains_key(&f.symbol) {
            return Err(format!("função Dart sem plano: {}", f.symbol));
        }
    }
    if let Some(sym_name) = plans.keys().filter(|s| !symbols.contains(*s)).min() {
        return Err(format!("plano Dart obsoleto: {sym_name}"));
    }
    let mut module = Module::new();
    module.functions = functions.to_vec();
    let mut new_plans = plans.clone();
    let signatures: HashMap<_, _> = functions.iter().map(|f| (f.symbol.clone(),
        (f.params.iter().map(|(_, _, ty)| *ty).collect::<Vec<_>>(), f.return_ty))).collect();
    for f in &mut module.functions {
        // Um bloco sem retorno explícito (o fim do corpo, o placeholder do
        // lowering) numa função não-void devolve o valor padrão, como o
        // emissor LLVM já faz; aqui ele fica explícito para os tokens.
        if matches!(f.return_ty, Type::Ref | Type::I64 | Type::I8 | Type::I1 | Type::F64) {
            let pattern = super::super::operands::default_constant(f.return_ty);
            for b in &mut f.blocks {
                if let Terminator::Return(r @ None) = &mut b.terminator {
                    *r = Some(pattern.clone());
                }
            }
        }
        super::typed_set::prepare(f)?;
        super::natives::prepare(f)?;
        if let Some(layouts) = layouts {
            let plan = new_plans.get_mut(&f.symbol).unwrap();
            super::fields::prepare(f, layouts, plan, receivers.get(&f.symbol), preserve_fields)?;
            super::objects::prepare(f, layouts, plan, &signatures)?;
        }
        super::boxes::prepare(f)?;
        if prepare_calls {
            super::exits::prepare_ref_throws(f, new_plans.get_mut(&f.symbol).unwrap())?;
        }
        super::natives::scope(f);
    }
    let mut do_not_throw = super::super::effects::do_not_throw(&module);
    // Contexto explícito implica conferência de pilha no LLVM. Inclui
    // leituras/limpeza de pendência e pousos, além das marcas nas tabelas.
    // Propaga esta falha do prólogo aos chamadores em O(V+E).
    let failure_in_prologue_or_exit = |f: &Function| {
        let t = &plans[&f.symbol].tables;
        crate::llvm::LlvmEmitter::requires_explicit_context(f, Some(t)) || !t.exits.is_empty()
    };
    if !variants.is_empty() || module.functions.iter().any(failure_in_prologue_or_exit) {
        let mut callers: HashMap<&str, Vec<&str>> = HashMap::new();
        for f in &module.functions {
            for (_, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
                if let Instruction::CallStatic { symbol, .. } = inst {
                    callers.entry(symbol).or_default().push(&f.symbol);
                }
            }
        }
        for (public, alternatives) in variants {
            if !symbols.contains(public) || alternatives.iter().any(|s| !symbols.contains(s)) {
                return Err(format!("variantes ARC fora do conjunto: {public}"));
            }
            for alternative in alternatives { callers.entry(alternative).or_default().push(public); }
        }
        let mut queue: Vec<_> = module.functions.iter()
            .filter(|f| !do_not_throw.contains(&f.symbol) || failure_in_prologue_or_exit(f))
            .map(|f| f.symbol.as_str()).collect();
        let mut visited = HashSet::new();
        while let Some(sym_name) = queue.pop() {
            if !visited.insert(sym_name) { continue; }
            do_not_throw.remove(sym_name);
            if let Some(previous_ones) = callers.get(sym_name) { queue.extend(previous_ones.iter().copied()); }
        }
    }
    for f in &module.functions {
        let classes = &mut new_plans.get_mut(&f.symbol).unwrap().classes;
        for (v, _, ty) in &f.params {
            // Essas representações não transportam handles nem slots fortes.
            // I64 e Ptr continuam dependentes da proveniência do lowering.
            if matches!(ty, Type::F64 | Type::I1 | Type::I8) || ty.is_vector() {
                if classes.get(v).is_some_and(|c| *c != Ownership::Trivial) {
                    return Err(format!(
                        "parâmetro escalar {} v{} exige Trivial",
                        f.symbol, v.0
                    ));
                }
                classes.insert(*v, Ownership::Trivial);
            }
        }
    }
    if prepare_calls {
        for f in &mut module.functions {
            super::invocations::prepare(f, new_plans.get_mut(&f.symbol).unwrap(), &do_not_throw, by_tables)?;
        }
    }
    let mut summaries: Vec<_> = module
        .functions
        .iter()
        .map(|f| super::calls::provisional_summary(f, !do_not_throw.contains(&f.symbol)))
        .collect();
    // Callees de outra imagem preparada (`Module::arc_external_callees`).
    if externals {
        summaries.extend(super::calls::external_summaries(&module.functions, &symbols)?);
    }
    let index = super::calls::DartFunctionIndex::new(&summaries)?;
    let mut total = (0, 0);
    for f in &mut module.functions {
        let new = new_plans.get_mut(&f.symbol).unwrap();
        super::exits::split_shared_exits(f, new)?;
        super::exits::split_guards(f, new)?;
        super::tokens::normalize_exits_throws(f, &new.tables)?;
        total.0 += super::phis::copy_borrows(f, &index, new)?;
        super::calls::produce_dart_calls_and_instructions(
            f,
            &index,
            &mut new.classes,
            &mut new.tokens,
        ).map_err(|error| format!("contratos ARC em {}: {error}", f.symbol))?;
        if prepare_calls {
            total.0 += super::borrows::protect(f, new)?;
            let (copies, drops) = super::edges::prepare(f, new)?;
            total.0 += copies;
            total.1 += drops;
        }
        let (copies, drops) = insert_arc_dart_exits(
            f,
            &mut new.classes,
            &mut new.tokens,
            &new.tables,
            &new.scopes,
        )?;
        verify_dart_function_contract(
            f,
            &new.classes,
            &new.tokens,
            &new.tables,
            &new.scopes,
        )?;
        new.owners_at_landing_pad =
            tokens_at_landing_pad_entry(f, &new.classes, &new.tables, &new.tokens)?;
        new.tables.foreign_cleanup = ForeignCleanup::new(f, new)?;
        total.0 += copies;
        total.1 += drops;
    }
    // A convenção foi comprovada conjuntamente; os resumos provisórios não
    // escapam desta transação. Tabelas/limites incluem as saídas separadas.
    functions.clone_from_slice(&module.functions);
    *plans = new_plans;
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_publishes_final_arc_cfg_tables_without_enabling_modes() {
        for case in 0..6 {
            let (functions, mut plans) = set();
            plans.get_mut("folha").unwrap().tables.checks_stack = true;
            let mut m = Module::new();
            m.functions = functions;
            m.arc_memory = case != 2;
            m.excecoes_por_tabelas = case != 3;
            // Inventário anterior incompleto: não pode sobreviver ao novo CFG.
            m.tables.push(FunctionTables::default());
            if case == 1 {
                plans.remove("folha");
            } else if case == 4 {
                // Falha depois da preparação privada dos callers: não publicar
                // nem seus CFGs novos nem as tabelas parcialmente recalculadas.
                m.functions[2].blocks[0].instructions.push((
                    ValueId(2),
                    Instruction::ArcDrop {
                        value: Operand::Val(ValueId(0)),
                    },
                    Type::Void,
                ));
            } else if case == 5 {
                m.functions.reverse();
            }
            let before = format!("{m:?}/{plans:?}");
            let result = prepare_arc_dart_module(&mut m, &mut plans);
            if case == 1 || case == 4 {
                assert!(result.is_err());
                assert_eq!(format!("{m:?}/{plans:?}"), before);
            } else if case == 2 {
                assert_eq!(result.unwrap(), (0, 0));
                assert_eq!(format!("{m:?}/{plans:?}"), before);
            } else {
                result.unwrap();
                if case == 3 {
                    assert!(!m.excecoes_por_tabelas);
                    assert_eq!(m.tables.len(), 1);
                    continue;
                }
                assert_eq!(m.tables.len(), m.functions.len());
                for (f, table) in m.functions.iter().zip(&m.tables) {
                    assert_eq!(
                        format!("{table:?}"),
                        format!("{:?}", plans[&f.symbol].tables)
                    );
                }
                let caller = m
                    .functions
                    .iter()
                    .position(|f| f.symbol == "caller")
                    .unwrap();
                assert!(!m.tables[caller].invocations.is_empty());
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(ir.contains("invoke i64 @folha("), "{ir}");
                let verified = format!("{m:?}/{plans:?}");
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    (0, 0)
                );
                assert_eq!(format!("{m:?}/{plans:?}"), verified);
            }
        }
    }

    #[test]
    fn keepalive_phi_prepares_all_origins_before_reclassifying() {
        for case in 0..7 {
            let fallible = case == 1;
            let mut m = Module::new();
            m.arc_memory = case != 3;
            let mut branches = Vec::new();
            for (block, result, pending, cmp) in [(1, 2, 6, 7), (2, 3, 8, 9)] {
                let mut instructions = vec![(
                    ValueId(result),
                    if case == 2 && block == 2 {
                        Instruction::CellGet { cell: Operand::Val(ValueId(0)) }
                    } else if case == 6 && block == 2 {
                        Instruction::LoadIndexed { base: Operand::Val(ValueId(0)), index: Operand::Constant(Constant::Int(0)) }
                    } else if case == 5 && block == 2 {
                        Instruction::EnvGet {
                            env: Operand::Val(ValueId(0)),
                            index: 0,
                        }
                    } else {
                        Instruction::CallRuntime {
                            name: if fallible {
                                "df.teste.campo_falivel"
                            } else {
                                "dartforge_arc_ler_campo_ref_v1"
                            }
                            .into(),
                            args: vec![
                                (Operand::Val(ValueId(0)), Type::Ref),
                                (Operand::Constant(Constant::Int(0)), Type::I64),
                            ],
                            ret_ty: Type::Ref,
                        }
                    },
                    Type::Ref,
                )];
                let terminator = if fallible {
                    instructions.extend([
                        (
                            ValueId(pending),
                            Instruction::CallRuntime {
                                name: "dartforge_exception_pending".into(),
                                args: vec![],
                                ret_ty: Type::I8,
                            },
                            Type::I8,
                        ),
                        (
                            ValueId(cmp),
                            Instruction::ICmp(
                                ICmpOp::Ne,
                                Operand::Val(ValueId(pending)),
                                Operand::Constant(Constant::Int(0)),
                            ),
                            Type::I1,
                        ),
                    ]);
                    Terminator::CondBranch {
                        cond: Operand::Val(ValueId(cmp)),
                        then_block: BlockId(4),
                        else_block: BlockId(3),
                    }
                } else {
                    Terminator::Branch(BlockId(3))
                };
                branches.push(BasicBlock {
                    id: BlockId(block),
                    instructions,
                    terminator,
                });
            }
            let mut blocks = vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::CondBranch {
                    cond: Operand::Val(ValueId(1)),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                },
            }];
            blocks.extend(branches);
            blocks.push(BasicBlock {
                id: BlockId(3),
                instructions: vec![
                    (
                        ValueId(4),
                        Instruction::Phi {
                            incoming: vec![
                                (BlockId(1), Operand::Val(ValueId(2))),
                                (BlockId(2), Operand::Val(ValueId(3))),
                            ],
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(5),
                        Instruction::CallRuntime {
                            name: "dartforge_arc_gravar_campo_ref_v1".into(),
                            args: vec![
                                (Operand::Val(ValueId(0)), Type::Ref),
                                (Operand::Constant(Constant::Int(0)), Type::I64),
                                (Operand::Constant(Constant::Null), Type::Ref),
                            ],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(4)))),
            });
            if fallible {
                blocks.push(BasicBlock {
                    id: BlockId(4),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
                });
            }
            m.functions.push(Function {
                symbol: "phi_multiplos_borrows".into(),
                name: "phi_multiplos_borrows".into(),
                debug_info: None,
                params: vec![
                    (ValueId(0), "objeto".into(), Type::Ref),
                    (ValueId(1), "cond".into(), Type::I1),
                ],
                return_ty: Type::Ref,
                blocks,
            });
            if case == 4 {
                let f = &mut m.functions[0];
                f.params.push((ValueId(10), "repetir".into(), Type::I1));
                let Instruction::Phi { incoming, .. } = &mut f.blocks[3].instructions[0].1 else {
                    panic!("Phi ausente")
                };
                incoming.push((BlockId(5), Operand::Val(ValueId(4))));
                f.blocks[3].terminator = Terminator::CondBranch {
                    cond: Operand::Val(ValueId(10)),
                    then_block: BlockId(5),
                    else_block: BlockId(4),
                };
                f.blocks.extend([
                    BasicBlock {
                        id: BlockId(4),
                        instructions: vec![],
                        terminator: Terminator::Return(Some(Operand::Val(ValueId(4)))),
                    },
                    BasicBlock {
                        id: BlockId(5),
                        instructions: vec![],
                        terminator: Terminator::Branch(BlockId(3)),
                    },
                ]);
            }
            let mut plan = DartFunctionPlan::default();
            plan.tokens.ret = TokenReturn::Owned;
            if case == 6 {
                // Proveniência explícita do alias; sem ABI que possa produzir
                // keepalive na origem. Falha após a primeira promoção privada.
                plan.classes.insert(
                    ValueId(3),
                    Ownership::Borrowed {
                        owner: OwnerOrigin::Value(ValueId(0)),
                        scope: 0,
                    },
                );
                plan
                    .tokens
                    .instructions
                    .insert(ValueId(3), TokenEffect::default());
            }
            let mut plans = HashMap::from([("phi_multiplos_borrows".into(), plan)]);
            let before = format!("{m:?}/{plans:?}");
            if case == 6 {
                assert!(
                    prepare_arc_dart_module(&mut m, &mut plans)
                        .unwrap_err()
                        .contains("empréstimo invalidado")
                );
                assert_eq!(format!("{m:?}/{plans:?}"), before);
            } else if case == 3 {
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    (0, 0)
                );
                assert_eq!(format!("{m:?}/{plans:?}"), before);
            } else {
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    (2, 0)
                );
                for v in [2, 3, 4] {
                    assert_eq!(
                        plans["phi_multiplos_borrows"].classes[&ValueId(v)],
                        Ownership::Owned
                    );
                }
                if fallible {
                    let Instruction::Phi { incoming, .. } =
                        &m.functions[0].blocks[3].instructions[0].1
                    else {
                        panic!("Phi ausente")
                    };
                    // As entradas chegam pelos blocos novos das continuações
                    // (a numeração depende de quantos blocos a preparação cria).
                    let origins: Vec<_> = incoming.iter().map(|(b, _)| b.0).collect();
                    assert_eq!(origins.len(), 2);
                    assert!(origins.iter().all(|b| *b > 4) && origins[0] != origins[1], "{origins:?}");
                    assert!(m.functions[0].blocks[4].instructions.is_empty());
                    assert_eq!(plans["phi_multiplos_borrows"].tokens.pendings.len(), 2);
                }
                let prepared = format!("{m:?}/{plans:?}");
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    (0, 0)
                );
                assert_eq!(format!("{m:?}/{plans:?}"), prepared);
                crate::llvm::LlvmEmitter::new(&m).emit_all();
            }
        }
    }

    #[test]
    fn fallible_keepalive_retains_only_on_success_and_carries_phi_and_limits() {
        for case in 0..6 {
            let mut m = Module::new();
            m.arc_memory = case != 4;
            let error = if case == 3 {
                BlockId(u32::MAX)
            } else {
                BlockId(1)
            };
            m.functions.push(Function {
                symbol: "borrow_pending".into(),
                name: "borrow_pending".into(),
                debug_info: None,
                params: vec![(ValueId(0), "record".into(), Type::Ref)],
                return_ty: Type::Ref,
                blocks: vec![
                    BasicBlock {
                        id: BlockId(0),
                        instructions: vec![
                            (
                                ValueId(1),
                                Instruction::CallRuntime {
                                    name: "df.teste.campo_falivel".into(),
                                    args: vec![
                                        (Operand::Val(ValueId(0)), Type::Ref),
                                        (Operand::Constant(Constant::Int(0)), Type::I64),
                                    ],
                                    ret_ty: Type::Ref,
                                },
                                Type::Ref,
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
                                    Operand::Constant(Constant::Int(if case == 2 { 1 } else { 0 })),
                                ),
                                Type::I1,
                            ),
                        ],
                        terminator: Terminator::CondBranch {
                            cond: Operand::Val(ValueId(3)),
                            then_block: error,
                            else_block: BlockId(2),
                        },
                    },
                    BasicBlock {
                        id: error,
                        instructions: vec![],
                        terminator: Terminator::Return(Some(if case == 1 {
                            Operand::Val(ValueId(1))
                        } else {
                            Operand::Constant(Constant::Null)
                        })),
                    },
                    BasicBlock {
                        id: BlockId(2),
                        instructions: vec![
                            (
                                ValueId(4),
                                Instruction::Phi {
                                    incoming: vec![(BlockId(0), Operand::Val(ValueId(1)))],
                                    ty: Type::Ref,
                                },
                                Type::Ref,
                            ),
                            (
                                ValueId(5),
                                Instruction::CallRuntime {
                                    name: "dartforge_arc_gravar_campo_ref_v1".into(),
                                    args: vec![
                                        (Operand::Val(ValueId(0)), Type::Ref),
                                        (Operand::Constant(Constant::Int(0)), Type::I64),
                                        (Operand::Constant(Constant::Null), Type::Ref),
                                    ],
                                    ret_ty: Type::Void,
                                },
                                Type::Void,
                            ),
                        ],
                        terminator: Terminator::Return(Some(Operand::Val(ValueId(4)))),
                    },
                ],
            });
            if case == 5 {
                let f = &mut m.functions[0];
                f.return_ty = Type::Void;
                f.blocks[1].terminator = Terminator::Return(None);
                f.blocks[2].instructions.extend([
                    (
                        ValueId(6),
                        Instruction::ArcCopy {
                            value: Operand::Val(ValueId(4)),
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(7),
                        Instruction::ArcDrop {
                            value: Operand::Val(ValueId(6)),
                        },
                        Type::Void,
                    ),
                ]);
                f.blocks[2].terminator = Terminator::Return(None);
            }
            let mut plan = DartFunctionPlan::default();
            if case != 5 {
                plan.tokens.ret = TokenReturn::Owned;
            }
            plan
                .scopes
                .before
                .insert(ValueId(1), vec![ScopeChange::Open(1)]);
            for dest in [error, BlockId(2)] {
                plan
                    .scopes
                    .edges
                    .insert((BlockId(0), dest), vec![ScopeChange::Close(1)]);
            }
            let mut plans = HashMap::from([("borrow_pending".into(), plan)]);
            let before = format!("{m:?}/{plans:?}");
            if case == 0 || case == 5 {
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    (1, usize::from(case == 5))
                );
                let f = &m.functions[0];
                let p = &plans["borrow_pending"];
                let copy_inst = f.blocks.last().unwrap();
                assert_eq!(copy_inst.id, BlockId(3));
                assert!(matches!(
                    copy_inst.instructions[0],
                    (ValueId(1), Instruction::ArcCopy { .. }, Type::Ref)
                ));
                assert!(
                    f.blocks[0]
                        .instructions
                        .iter()
                        .all(|(_, i, _)| !matches!(i, Instruction::ArcCopy { .. }))
                );
                assert!(f.blocks[1].instructions.iter().all(|(_, i, _)| !matches!(
                    i,
                    Instruction::ArcCopy { .. } | Instruction::ArcDrop { .. }
                )));
                assert!(matches!(&f.blocks[2].instructions[0].1,
                    Instruction::Phi { incoming, .. } if incoming[0].0 == copy_inst.id));
                assert_eq!(p.classes[&ValueId(1)], Ownership::Owned);
                assert_eq!(p.classes[&ValueId(4)], Ownership::Owned);
                assert_eq!(p.tokens.pendings.len(), 1);
                assert!(!p.tokens.pendings.contains_key(&ValueId(1)));
                assert!(p.scopes.edges.contains_key(&(copy_inst.id, BlockId(2))));
                assert!(!p.scopes.edges.contains_key(&(BlockId(0), BlockId(2))));
                assert!(p.scopes.edges.contains_key(&(BlockId(0), error)));
                let prepared = format!("{m:?}/{plans:?}");
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    (0, 0)
                );
                assert_eq!(format!("{m:?}/{plans:?}"), prepared);
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(ir.contains("call void @dartforge_arc_retain"));
            } else if case == 4 {
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    (0, 0)
                );
                assert_eq!(format!("{m:?}/{plans:?}"), before);
            } else {
                assert!(prepare_arc_dart_module(&mut m, &mut plans).is_err());
                assert_eq!(format!("{m:?}/{plans:?}"), before);
            }
        }
    }

    #[test]
    fn dart_call_keepalive_is_atomic_idempotent_and_arc_only() {
        for case in 0..4 {
            let mut m = Module::new();
            m.arc_memory = case != 2;
            m.functions.push(Function {
                symbol: "caller_borrow".into(),
                name: "caller_borrow".into(),
                debug_info: None,
                params: vec![(ValueId(0), "objeto".into(), Type::Ref)],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (
                            ValueId(1),
                            Instruction::CallRuntime {
                                name: "dartforge_arc_ler_campo_ref_v1".into(),
                                args: vec![
                                    (Operand::Val(ValueId(0)), Type::Ref),
                                    (Operand::Constant(Constant::Int(0)), Type::I64),
                                ],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(2),
                            Instruction::CallStatic {
                                symbol: "sink".into(),
                                args: vec![Operand::Val(ValueId(1))],
                                ret_ty: Type::Void,
                            },
                            Type::Void,
                        ),
                    ],
                    terminator: Terminator::Return(None),
                }],
            });
            let mut sink = Function {
                symbol: "sink".into(),
                name: "sink".into(),
                debug_info: None,
                params: vec![(ValueId(0), "valor".into(), Type::Ref)],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                }],
            };
            if case == 1 {
                sink.blocks[0].instructions.push((
                    ValueId(1),
                    Instruction::CallRuntime {
                        name: "runtime_sem_contrato".into(),
                        args: vec![],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ));
            }
            if case == 3 {
                m.functions[0].blocks[0].instructions.push((
                    ValueId(u32::MAX),
                    Instruction::Const(Constant::Int(0)),
                    Type::I64,
                ));
            }
            m.functions.push(sink);
            let mut plans = HashMap::from([
                ("caller_borrow".into(), DartFunctionPlan::default()),
                ("sink".into(), DartFunctionPlan::default()),
            ]);
            let before = format!("{m:?}/{plans:?}");
            match case {
                0 => {
                    assert_eq!(
                        prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                        (1, 1)
                    );
                    assert_eq!(
                        plans["caller_borrow"].classes[&ValueId(1)],
                        Ownership::Owned
                    );
                    let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                    assert!(
                        ir.find("call void @dartforge_arc_retain").unwrap()
                            < ir.find("@sink(i64 %v1)").unwrap()
                    );
                    let prepared = format!("{m:?}/{plans:?}");
                    assert_eq!(
                        prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                        (0, 0)
                    );
                    assert_eq!(format!("{m:?}/{plans:?}"), prepared);
                }
                2 => {
                    assert_eq!(
                        prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                        (0, 0)
                    );
                    assert_eq!(format!("{m:?}/{plans:?}"), before);
                }
                _ => {
                    assert!(prepare_arc_dart_module(&mut m, &mut plans).is_err());
                    assert_eq!(format!("{m:?}/{plans:?}"), before);
                }
            }
        }
    }

    #[test]
    fn keepalive_before_diamond_rebuilds_phi_with_owned_transfer() {
        let mut m = Module::new();
        m.arc_memory = true;
        m.functions.push(Function {
            symbol: "diamante_borrow".into(),
            name: "diamante_borrow".into(),
            debug_info: None,
            params: vec![
                (ValueId(0), "objeto".into(), Type::Ref),
                (ValueId(5), "cond".into(), Type::I1),
            ],
            return_ty: Type::Ref,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::CallRuntime {
                            name: "dartforge_arc_ler_campo_ref_v1".into(),
                            args: vec![
                                (Operand::Val(ValueId(0)), Type::Ref),
                                (Operand::Constant(Constant::Int(0)), Type::I64),
                            ],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(5)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(2),
                        Instruction::CallRuntime {
                            name: "dartforge_arc_gravar_campo_ref_v1".into(),
                            args: vec![
                                (Operand::Val(ValueId(0)), Type::Ref),
                                (Operand::Constant(Constant::Int(0)), Type::I64),
                                (Operand::Constant(Constant::Null), Type::Ref),
                            ],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    )],
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(3),
                    instructions: vec![(
                        ValueId(4),
                        Instruction::Phi {
                            incoming: vec![
                                (BlockId(1), Operand::Val(ValueId(1))),
                                (BlockId(2), Operand::Val(ValueId(1))),
                            ],
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(4)))),
                },
            ],
        });
        let mut p = DartFunctionPlan::default();
        p.tokens.ret = TokenReturn::Owned;
        let mut plans = HashMap::from([("diamante_borrow".into(), p)]);
        assert_eq!(
            prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
            (1, 0)
        );
        assert_eq!(
            plans["diamante_borrow"].classes[&ValueId(1)],
            Ownership::Owned
        );
        assert_eq!(
            plans["diamante_borrow"].classes[&ValueId(4)],
            Ownership::Owned
        );
        let prepared = format!("{m:?}/{plans:?}");
        assert_eq!(
            prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{m:?}/{plans:?}"), prepared);
    }

    #[test]
    fn borrowed_field_requires_copy_before_replacing_edge() {
        for copy_before in [false, true] {
            let read = (
                ValueId(1),
                Instruction::CallRuntime {
                    name: "dartforge_arc_ler_campo_ref_v1".into(),
                    args: vec![
                        (Operand::Val(ValueId(0)), Type::Ref),
                        (Operand::Constant(Constant::Int(39)), Type::I64),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
            let swap = (
                ValueId(2),
                Instruction::CallRuntime {
                    name: "dartforge_arc_gravar_campo_ref_v1".into(),
                    args: vec![
                        (Operand::Val(ValueId(0)), Type::Ref),
                        (Operand::Constant(Constant::Int(39)), Type::I64),
                        (Operand::Constant(Constant::Null), Type::Ref),
                    ],
                    ret_ty: Type::Void,
                },
                Type::Void,
            );
            let copy_inst = (
                ValueId(3),
                Instruction::ArcCopy {
                    value: Operand::Val(ValueId(1)),
                },
                Type::Ref,
            );
            let mut instructions = vec![read];
            if copy_before {
                instructions.extend([copy_inst, swap]);
            } else {
                instructions.extend([swap, copy_inst]);
            }
            instructions.push((
                ValueId(4),
                Instruction::ArcDrop {
                    value: Operand::Val(ValueId(3)),
                },
                Type::Void,
            ));
            let f = Function {
                symbol: "borrow_campo".into(),
                name: "borrow_campo".into(),
                debug_info: None,
                params: vec![(ValueId(0), "objeto".into(), Type::Ref)],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: instructions,
                    terminator: Terminator::Return(None),
                }],
            };
            let mut classes = HashMap::from([(
                ValueId(0),
                Ownership::Borrowed {
                    owner: OwnerOrigin::Caller,
                    scope: 0,
                },
            )]);
            let mut plan = TokenPlan::default();
            produce_arc_contracts(&f, &mut classes, &mut plan).unwrap();
            let result = verify_tokens(&f, &classes, &FunctionTables::default(), &plan);
            if copy_before {
                result.unwrap();
            } else {
                assert!(result.unwrap_err().contains("empréstimo invalidado"));
            }
            let mut m = Module::new();
            m.arc_memory = true;
            m.functions.push(f);
            let mut plans = HashMap::from([("borrow_campo".into(), DartFunctionPlan::default())]);
            if copy_before {
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    (0, 0)
                );
            } else {
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    (1, 1)
                );
                assert_eq!(
                    plans["borrow_campo"].classes[&ValueId(1)],
                    Ownership::Owned
                );
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(
                    ir.find("call void @dartforge_arc_retain").unwrap()
                        < ir.find("call void @dartforge_arc_gravar_campo_ref_v1")
                            .unwrap()
                );
            }
            let before = format!("{m:?}/{plans:?}");
            assert_eq!(
                prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                (0, 0)
            );
            assert_eq!(format!("{m:?}/{plans:?}"), before);
        }
    }

    #[test]
    fn field_ref_read_borrows_receiver_and_copies_owned_return() {
        for case in 0..4 {
            let mut m = Module::new();
            m.arc_memory = true;
            let ret_ty = if case == 1 { Type::I64 } else { Type::Ref };
            m.functions.push(Function {
                symbol: "leitura_ref".into(),
                name: "leitura_ref".into(),
                debug_info: None,
                params: vec![(ValueId(0), "objeto".into(), Type::Ref)],
                return_ty: Type::Ref,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::CallRuntime {
                            name: if case == 2 {
                                "dartforge_object_get"
                            } else {
                                "dartforge_arc_ler_campo_ref_v1"
                            }
                            .into(),
                            args: vec![
                                (Operand::Val(ValueId(0)), Type::Ref),
                                (Operand::Constant(Constant::Int(39)), Type::I64),
                            ],
                            ret_ty,
                        },
                        ret_ty,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                }],
            });
            let mut p = DartFunctionPlan::default();
            p.tokens.ret = TokenReturn::Owned;
            if case == 3 {
                p.classes.insert(ValueId(1), Ownership::Owned);
            }
            let mut plans = HashMap::from([("leitura_ref".into(), p)]);
            let before = format!("{m:?}/{plans:?}");
            // O acesso físico cru (caso 2) vira a mesma leitura auditada.
            if case == 1 || case == 3 {
                assert!(prepare_arc_dart_module(&mut m, &mut plans).is_err());
                assert_eq!(format!("{m:?}/{plans:?}"), before);
            } else {
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    (1, 0)
                );
                assert_eq!(
                    plans["leitura_ref"].classes[&ValueId(1)],
                    Ownership::Borrowed {
                        owner: OwnerOrigin::Value(ValueId(0)),
                        scope: 0
                    }
                );
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(ir.contains("call i64 @dartforge_arc_ler_campo_ref_v1(i64 %v0, i64 39)"));
                assert!(ir.contains("call void @dartforge_arc_retain(i64 %v1)"));
                let prepared = format!("{m:?}/{plans:?}");
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    (0, 0)
                );
                assert_eq!(format!("{m:?}/{plans:?}"), prepared);
            }
            m.arc_memory = false;
            let before = format!("{m:?}/{plans:?}");
            assert_eq!(
                prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                (0, 0)
            );
            assert_eq!(format!("{m:?}/{plans:?}"), before);
        }
    }

    #[test]
    fn field_write_preserves_owners_and_rejects_forged_signature() {
        let integer = |n| (Operand::Constant(Constant::Int(n)), Type::I64);
        let reference = |n| (Operand::Val(ValueId(n)), Type::Ref);
        let call = |id, name: &str, args, ret_ty| {
            (
                ValueId(id),
                Instruction::CallRuntime {
                    name: name.into(),
                    args,
                    ret_ty,
                },
                ret_ty,
            )
        };
        for case in 0..4 {
            let mut m = Module::new();
            m.arc_memory = true;
            m.functions.push(Function {
                symbol: "campos".into(),
                name: "campos".into(),
                debug_info: None,
                params: vec![],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        call(
                            0,
                            "dartforge_arc_objeto_owned_v1",
                            vec![integer(123), integer(40)],
                            Type::Ref,
                        ),
                        call(
                            1,
                            "dartforge_arc_box_int_owned_v1",
                            vec![integer(i64::MAX)],
                            Type::Ref,
                        ),
                        call(
                            2,
                            if case == 3 {
                                "dartforge_object_set"
                            } else {
                                "dartforge_arc_gravar_campo_ref_v1"
                            },
                            vec![
                                reference(0),
                                integer(37),
                                if case == 1 {
                                    (Operand::Val(ValueId(1)), Type::I64)
                                } else {
                                    reference(1)
                                },
                            ],
                            Type::Void,
                        ),
                        call(
                            3,
                            "dartforge_arc_gravar_campo_escalar_v1",
                            vec![
                                reference(0),
                                integer(39),
                                if case == 2 {
                                    reference(1)
                                } else {
                                    integer(i64::MAX)
                                },
                            ],
                            Type::Void,
                        ),
                        call(
                            4,
                            "dartforge_arc_ler_campo_escalar_v1",
                            vec![reference(0), integer(39)],
                            Type::I64,
                        ),
                    ],
                    terminator: Terminator::Return(None),
                }],
            });
            let mut plans = HashMap::from([("campos".into(), DartFunctionPlan::default())]);
            let before = format!("{m:?}/{plans:?}");
            if case != 0 {
                assert!(prepare_arc_dart_module(&mut m, &mut plans).is_err());
                assert_eq!(format!("{m:?}/{plans:?}"), before);
            } else {
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    (0, 2)
                );
                let p = &plans["campos"];
                for id in [0, 1] {
                    assert_eq!(p.classes[&ValueId(id)], Ownership::Owned);
                }
                for id in [2, 3, 4] {
                    assert_eq!(p.classes[&ValueId(id)], Ownership::Trivial);
                }
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(ir.contains(
                    "call void @dartforge_arc_gravar_campo_ref_v1(i64 %v0, i64 37, i64 %v1)"
                ));
                assert!(ir.contains("call void @dartforge_arc_gravar_campo_escalar_v1(i64 %v0, i64 39, i64 9223372036854775807)"));
                assert!(ir.contains("call void @dartforge_arc_release(i64 %v0)"));
                assert!(ir.contains("call void @dartforge_arc_release(i64 %v1)"));
                let prepared = format!("{m:?}/{plans:?}");
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    (0, 0)
                );
                assert_eq!(format!("{m:?}/{plans:?}"), prepared);
            }
            m.arc_memory = false;
            let before = format!("{m:?}/{plans:?}");
            assert_eq!(
                prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                (0, 0)
            );
            assert_eq!(format!("{m:?}/{plans:?}"), before);
        }
    }

    #[test]
    fn instance_factory_converts_zeroed_without_classifying_legacy_allocator_directly() {
        for case in 0..4 {
            let mut m = Module::new();
            m.arc_memory = true;
            let ret_ty = if case == 1 { Type::I64 } else { Type::Ref };
            let name_str = if case == 3 {
                "dartforge_object_new"
            } else {
                "dartforge_arc_objeto_owned_v1"
            };
            m.functions.push(Function {
                symbol: "instancia".into(),
                name: "instancia".into(),
                debug_info: None,
                params: vec![],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (
                            ValueId(0),
                            Instruction::CallRuntime {
                                name: name_str.into(),
                                args: vec![
                                    (Operand::Constant(Constant::Int(123)), Type::I64),
                                    (
                                        Operand::Constant(Constant::Int(40)),
                                        if case == 2 { Type::Ref } else { Type::I64 },
                                    ),
                                ],
                                ret_ty,
                            },
                            ret_ty,
                        ),
                        (
                            ValueId(1),
                            Instruction::CallRuntime {
                                name: "dartforge_arc_ler_campo_escalar_v1".into(),
                                args: vec![
                                    (Operand::Val(ValueId(0)), Type::Ref),
                                    (Operand::Constant(Constant::Int(39)), Type::I64),
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
                    terminator: Terminator::Return(None),
                }],
            });
            let mut plans = HashMap::from([("instancia".into(), DartFunctionPlan::default())]);
            if case == 3 {
                // A ABI original não publica token. Somente a substituição
                // pela fábrica Owned na transação do módulo muda o contrato.
                let mut classes = HashMap::new();
                let mut tokens = TokenPlan::default();
                let before = format!("{classes:?}/{tokens:?}");
                assert!(produce_runtime_contracts(&m.functions[0], &mut classes, &mut tokens).is_err());
                assert_eq!(format!("{classes:?}/{tokens:?}"), before);
            }
            let before = format!("{m:?}/{plans:?}");
            if case == 1 || case == 2 {
                assert!(prepare_arc_dart_module(&mut m, &mut plans).is_err());
                assert_eq!(format!("{m:?}/{plans:?}"), before);
            } else {
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    (0, 1)
                );
                let p = &plans["instancia"];
                assert_eq!(p.classes[&ValueId(0)], Ownership::Owned);
                assert_eq!(p.classes[&ValueId(1)], Ownership::Trivial);
                assert_eq!(p.classes[&ValueId(2)], Ownership::Trivial);
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(
                    ir.contains("declare i64 @dartforge_arc_objeto_owned_v1(i64, i64) nounwind")
                );
                assert!(ir.contains("call i64 @dartforge_arc_objeto_owned_v1(i64 123, i64 40)"));
                assert!(ir.contains("call void @dartforge_arc_release(i64 %v0)"));
                let prepared = format!("{m:?}/{plans:?}");
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    (0, 0)
                );
                assert_eq!(format!("{m:?}/{plans:?}"), prepared);
            }
            m.arc_memory = false;
            let before = format!("{m:?}/{plans:?}");
            assert_eq!(
                prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                (0, 0)
            );
            assert_eq!(format!("{m:?}/{plans:?}"), before);
        }
    }

    #[test]
    fn module_publishes_facts_and_cleanup_together_and_tracing_does_not_prepare() {
        for failure in 0..3 {
            let (mut fs, mut ps) = boxing(Operand::Val(ValueId(3)), Type::I64);
            fs[0].params = vec![
                (ValueId(3), "numero".into(), Type::I64),
                (ValueId(4), "$tipos".into(), Type::I64),
            ];
            let mut m = Module::new();
            m.arc_memory = true;
            m.functions = fs;
            m.dart_scalar_parameters
                .insert("boxing".into(), HashSet::from([ValueId(3)]));
            m.dart_rti_parameters
                .insert("boxing".into(), HashSet::from([ValueId(4)]));
            if failure == 1 {
                m.dart_rti_parameters
                    .get_mut("boxing")
                    .unwrap()
                    .insert(ValueId(99));
            }
            if failure == 2 {
                m.functions[0].blocks[0].instructions.push((
                    ValueId(9),
                    Instruction::CallRuntime {
                        name: "sem_contrato".into(),
                        args: vec![],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ));
            }
            let before = format!("{m:?}/{ps:?}");
            if failure == 0 {
                assert_eq!(prepare_arc_dart_module(&mut m, &mut ps).unwrap(), (0, 1));
                assert_eq!(ps["boxing"].classes[&ValueId(3)], Ownership::Trivial);
                assert_eq!(ps["boxing"].classes[&ValueId(4)], Ownership::Trivial);
                assert!(matches!(
                    m.functions[0].blocks[0].instructions.last().unwrap().1,
                    Instruction::ArcDrop { .. }
                ));
                assert_eq!(prepare_arc_dart_module(&mut m, &mut ps).unwrap(), (0, 0));
            } else {
                assert!(prepare_arc_dart_module(&mut m, &mut ps).is_err());
                assert_eq!(format!("{m:?}/{ps:?}"), before);
            }
            m.arc_memory = false;
            let before = format!("{m:?}/{ps:?}");
            assert_eq!(prepare_arc_dart_module(&mut m, &mut ps).unwrap(), (0, 0));
            assert_eq!(format!("{m:?}/{ps:?}"), before);
        }
    }

    #[test]
    fn scalar_unboxing_preserves_borrow_and_requires_prepared_type_error() {
        for ty in [Type::I64, Type::F64, Type::I1] {
            let f = Function {
                symbol: "unboxing".into(),
                name: "unboxing".into(),
                debug_info: None,
                params: vec![(ValueId(0), "valor".into(), Type::Ref)],
                return_ty: ty,
                blocks: vec![
                    BasicBlock {
                        id: BlockId(0),
                        instructions: vec![
                            (
                                ValueId(1),
                                Instruction::Unbox {
                                    op: Operand::Val(ValueId(0)),
                                    to: ty,
                                },
                                ty,
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
                        terminator: Terminator::Return(None),
                    },
                    BasicBlock {
                        id: BlockId(2),
                        instructions: vec![],
                        terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                    },
                ],
            };
            let mut p = DartFunctionPlan::default();
            p.tables.exits.insert(BlockId(1), ExceptionExit::Throws);
            let mut fs = vec![f.clone()];
            let mut ps = HashMap::from([("unboxing".into(), p)]);
            assert_eq!(insert_arc_dart_functions(&mut fs, &mut ps).unwrap(), (0, 0));
            assert_eq!(ps["unboxing"].classes[&ValueId(1)], Ownership::Trivial);
            assert_eq!(ps["unboxing"].tokens.pendings[&ValueId(1)], BlockId(1));
            if ty == Type::I1 {
                assert!(matches!(
                    fs[0].blocks[0].instructions[0].1,
                    Instruction::Unbox { to: Type::I1, .. }
                ));
                let p = &ps["unboxing"];
                let mut tampered = p.tokens.clone();
                tampered
                    .instructions
                    .get_mut(&ValueId(1))
                    .unwrap()
                    .may_fail = false;
                tampered.pendings.remove(&ValueId(1));
                assert!(
                    verify_tokens(&fs[0], &p.classes, &p.tables, &tampered)
                        .unwrap_err()
                        .contains("ownership.tsv")
                );
                let mut m = Module::new();
                m.functions = fs.clone();
                m.excecoes_por_tabelas = true;
                m.tables.push(p.tables.clone());
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(ir.contains("%v1 = call i1 @df.desencaixa_bool(i64 %v0)"));
                assert!(ir.contains("%u = call i8 @dartforge_unbox_bool(i64 %r)"));
                assert!(ir.contains("%x = icmp ne i8 %u, 0"));
            } else {
                assert!(
                    // O unboxing pode rodar Dart (TypeError): num escopo nativo.
                    matches!(&fs[0].blocks[0].instructions[0].1, Instruction::CallRuntime { name, .. }
                        if name.strip_prefix(super::super::NATIVE_SCOPE_PREFIX)
                            == Some(if ty == Type::I64 { "dartforge_unbox_int" } else { "dartforge_unbox_double" }))
                );
            }
            let mut owned = f.clone();
            owned.params.clear();
            if let Instruction::Unbox { op, .. } = &mut owned.blocks[0].instructions[0].1 {
                *op = Operand::Val(ValueId(4));
            }
            owned.blocks[0].instructions.insert(
                0,
                (
                    ValueId(4),
                    Instruction::Box {
                        op: Operand::Constant(Constant::Int(i64::MAX)),
                        from: Type::I64,
                    },
                    Type::Ref,
                ),
            );
            let mut owned_plan = DartFunctionPlan::default();
            owned_plan
                .tables
                .exits
                .insert(BlockId(1), ExceptionExit::Throws);
            let mut owned = vec![owned];
            let mut owned_plans = HashMap::from([("unboxing".into(), owned_plan)]);
            assert_eq!(
                insert_arc_dart_functions(&mut owned, &mut owned_plans).unwrap(),
                (0, 2)
            );
            for b in &owned[0].blocks[1..] {
                assert_eq!(
                    b.instructions
                        .iter()
                        .filter(|(_, i, _)| matches!(
                            i,
                            Instruction::ArcDrop {
                                value: Operand::Val(ValueId(4))
                            }
                        ))
                        .count(),
                    1
                );
            }
            let mut fs = vec![f];
            fs[0].blocks.truncate(1);
            fs[0].blocks[0].instructions.truncate(1);
            fs[0].blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
            let mut ps = HashMap::from([("unboxing".into(), DartFunctionPlan::default())]);
            let before = format!("{fs:?}/{ps:?}");
            assert!(insert_arc_dart_functions(&mut fs, &mut ps).is_err());
            assert_eq!(format!("{fs:?}/{ps:?}"), before);
        }
    }

    #[test]
    fn double_and_bool_parameters_prepare_boxing_without_manual_contract() {
        for ty in [Type::F64, Type::I1, Type::I8] {
            let (mut fs, mut ps) = boxing(Operand::Val(ValueId(3)), ty);
            fs[0].params.push((ValueId(3), "valor".into(), ty));
            assert_eq!(
                insert_arc_dart_functions(&mut fs, &mut ps).unwrap(),
                (0, usize::from(ty == Type::F64))
            );
            assert_eq!(ps["boxing"].classes[&ValueId(3)], Ownership::Trivial);
            let (mut fs, mut ps) = boxing(Operand::Val(ValueId(3)), ty);
            fs[0].params.push((ValueId(3), "valor".into(), ty));
            ps.get_mut("boxing")
                .unwrap()
                .classes
                .insert(ValueId(3), Ownership::Owned);
            let before = format!("{fs:?}/{ps:?}");
            assert!(
                insert_arc_dart_functions(&mut fs, &mut ps)
                    .unwrap_err()
                    .contains("exige Trivial")
            );
            assert_eq!(format!("{fs:?}/{ps:?}"), before);
        }
        // A mesma preparação não autoriza presumir o significado de I64.
        let (mut fs, mut ps) = boxing(Operand::Val(ValueId(3)), Type::I64);
        fs[0].params.push((ValueId(3), "valor".into(), Type::I64));
        let before = format!("{fs:?}/{ps:?}");
        assert!(insert_arc_dart_functions(&mut fs, &mut ps).is_err());
        assert_eq!(format!("{fs:?}/{ps:?}"), before);
        ps.get_mut("boxing")
            .unwrap()
            .classes
            .insert(ValueId(3), Ownership::Trivial);
        assert_eq!(insert_arc_dart_functions(&mut fs, &mut ps).unwrap(), (0, 1));
    }

    fn boxing(op: Operand, from: Type) -> (Vec<Function>, HashMap<String, DartFunctionPlan>) {
        let f = Function {
            symbol: "boxing".into(),
            name: "boxing".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(ValueId(0), Instruction::Box { op, from }, Type::Ref)],
                terminator: Terminator::Return(None),
            }],
        };
        (
            vec![f],
            HashMap::from([("boxing".into(), DartFunctionPlan::default())]),
        )
    }

    #[test]
    fn owned_boxing_releases_mortals_and_preserves_static_bool() {
        for (constant, from, factory) in [
            (
                Constant::Int(i64::MAX),
                Type::I64,
                Some("dartforge_arc_box_int_owned_v1"),
            ),
            (
                Constant::Double(-0.0),
                Type::F64,
                Some("dartforge_arc_box_double_owned_v1"),
            ),
            (
                Constant::Double(f64::from_bits(0x7ff80000deadbeef)),
                Type::F64,
                Some("dartforge_arc_box_double_owned_v1"),
            ),
            (Constant::Bool(false), Type::I1, None),
        ] {
            let (mut fs, mut ps) = boxing(Operand::Constant(constant), from);
            assert_eq!(
                insert_arc_dart_functions(&mut fs, &mut ps).unwrap(),
                (0, usize::from(factory.is_some()))
            );
            assert_eq!(
                ps["boxing"].classes[&ValueId(0)],
                if factory.is_some() {
                    Ownership::Owned
                } else {
                    Ownership::Trivial
                }
            );
            if let Some(name_str) = factory {
                assert!(
                    matches!(&fs[0].blocks[0].instructions[0].1, Instruction::CallRuntime { name, .. } if name == name_str)
                );
                assert!(matches!(
                    &fs[0].blocks[0].instructions[1].1,
                    Instruction::ArcDrop {
                        value: Operand::Val(ValueId(0))
                    }
                ));
            } else {
                assert!(matches!(
                    fs[0].blocks[0].instructions[0].1,
                    Instruction::Box { .. }
                ));
            }
        }
    }

    #[test]
    fn owned_boxing_transfers_return_without_retain_and_rejects_invalid_shapes_atomically() {
        let (mut fs, mut ps) = boxing(Operand::Constant(Constant::Double(3.25)), Type::F64);
        fs[0].return_ty = Type::Ref;
        fs[0].blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(0))));
        ps.get_mut("boxing").unwrap().tokens.ret = TokenReturn::Owned;
        assert_eq!(insert_arc_dart_functions(&mut fs, &mut ps).unwrap(), (0, 0));
        assert_eq!(fs[0].blocks[0].instructions.len(), 1);
        for (op, from) in [
            (Operand::Constant(Constant::Int(1)), Type::F64),
            (Operand::Constant(Constant::Double(1.0)), Type::I64),
            (Operand::Constant(Constant::Null), Type::I1),
            (Operand::Constant(Constant::Int(256)), Type::I8),
            (Operand::Constant(Constant::Null), Type::Ref),
        ] {
            let (mut fs, mut ps) = boxing(op, from);
            let before = format!("{fs:?}/{ps:?}");
            assert!(insert_arc_dart_functions(&mut fs, &mut ps).is_err());
            assert_eq!(format!("{fs:?}/{ps:?}"), before);
        }
    }

    #[test]
    fn control_and_nsm_preserve_owner_on_success_and_release_on_failure() {
        for name_str in ["dartforge_ponto_seguro", "dartforge_nsm_chamada"] {
        for case in 0..5 {
            let mut m = Module::new();
            m.arc_memory = case != 4;
            m.functions.push(Function {
                symbol: "controle".into(), name: "controle".into(), debug_info: None,
                params: vec![(ValueId(0), "valor".into(), Type::Ref)], return_ty: Type::Ref,
                blocks: vec![
                    BasicBlock { id: BlockId(0), instructions: vec![
                        (ValueId(1), Instruction::ArcCopy { value: Operand::Val(ValueId(0)) }, Type::Ref),
                        (ValueId(2), Instruction::CallRuntime {
                            name: name_str.into(), args: vec![],
                            ret_ty: if case == 3 { Type::Ref } else { Type::Void },
                        }, if case == 3 { Type::Ref } else { Type::Void }),
                        (ValueId(3), Instruction::CallRuntime {
                            name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8,
                        }, Type::I8),
                        (ValueId(4), Instruction::ICmp(
                            if case == 2 { ICmpOp::Eq } else { ICmpOp::Ne },
                            Operand::Val(ValueId(3)), Operand::Constant(Constant::Int(0)),
                        ), Type::I1),
                    ], terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(4)), then_block: BlockId(2), else_block: BlockId(1),
                    } },
                    BasicBlock { id: BlockId(1), instructions: vec![],
                        terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) },
                    BasicBlock { id: BlockId(2), instructions: vec![],
                        terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
                ],
            });
            let mut plans = HashMap::from([("controle".into(), DartFunctionPlan::default())]);
            plans.get_mut("controle").unwrap().tokens.ret = TokenReturn::Owned;
            if case == 1 {
                m.functions[0].blocks[0].instructions.truncate(2);
                m.functions[0].blocks[0].terminator = Terminator::Branch(BlockId(1));
            }
            let before = format!("{m:?}/{plans:?}");
            let result = prepare_arc_dart_module(&mut m, &mut plans);
            if (1..4).contains(&case) {
                assert!(result.is_err(), "caso {case}");
                assert_eq!(format!("{m:?}/{plans:?}"), before);
            } else if case == 4 {
                assert_eq!(result.unwrap(), (0, 0));
                assert_eq!(format!("{m:?}/{plans:?}"), before);
            } else {
                result.unwrap();
                assert_eq!(plans["controle"].tokens.pendings[&ValueId(2)], BlockId(2));
                let f = &m.functions[0];
                let error = f.blocks.iter().find(|b| b.id == BlockId(2)).unwrap();
                assert!(error.instructions.iter().any(|(_, i, _)| matches!(i,
                    Instruction::ArcDrop { value: Operand::Val(ValueId(1)) })));
                let success = f.blocks.iter().find(|b| b.id == BlockId(1)).unwrap();
                assert!(!success.instructions.iter().any(|(_, i, _)| matches!(i,
                    Instruction::ArcDrop { value: Operand::Val(ValueId(1)) })));
                let ready = format!("{m:?}");
                let plan = plans["controle"].clone();
                assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
                assert_eq!(format!("{m:?}"), ready);
                let after = &plans["controle"];
                assert_eq!(after.classes, plan.classes);
                assert_eq!(after.tokens.instructions, plan.tokens.instructions);
                assert_eq!(after.tokens.pendings, plan.tokens.pendings);
                assert_eq!(after.tokens.ret, plan.tokens.ret);
                assert_eq!(after.tables, plan.tables);
                assert_eq!(format!("{:?}", after.scopes), format!("{:?}", plan.scopes));
                assert_eq!(after.owners_at_landing_pad, plan.owners_at_landing_pad);
            }
        }
        }
    }

    #[test]
    fn implicit_body_context_propagates_prologue_unwind() {
        for (name, ret_ty) in [
            ("dartforge_exception_pending", Type::I8),
            ("dartforge_exception_clear", Type::Void),
        ] {
            let (mut functions, mut plans) = set();
            functions[2].blocks[0].instructions.push((
                ValueId(2),
                Instruction::CallRuntime {
                    name: name.into(),
                    args: vec![],
                    ret_ty,
                },
                ret_ty,
            ));
            let before = format!("{functions:?}");
            let plans_before = format!("{plans:?}");
            let error = insert_arc_dart_functions(&mut functions, &mut plans).unwrap_err();
            assert!(
                error.contains("caller") && error.contains("saídas excepcionais"),
                "{error}"
            );
            assert_eq!(format!("{functions:?}"), before);
            assert_eq!(format!("{plans:?}"), plans_before);
            // A folha isolada prepara seu retorno; o resumo deve continuar
            // falível mesmo sem a marca confere_pilha ou saída Lanca.
            functions.drain(..2);
            plans.retain(|s, _| s == "folha");
            assert_eq!(
                insert_arc_dart_functions(&mut functions, &mut plans).unwrap(),
                (1, 0)
            );
            let p = &plans["folha"];
            let resumo = verify_dart_function_contract(
                &functions[0],
                &p.classes,
                &p.tokens,
                &p.tables,
                &p.scopes,
            )
            .unwrap();
            let (caller, _) = set();
            let caller = caller[1].clone();
            let mut classes = HashMap::new();
            let mut tokens = TokenPlan::default();
            produce_dart_calls(&caller, &[resumo], &mut classes, &mut tokens).unwrap();
            assert!(tokens.instructions[&ValueId(1)].may_fail);
            // O emissor também materializa a conferência na folha.
            let mut module = Module::new();
            module.functions = functions;
            module.excecoes_por_tabelas = true;
            module.tables.push(p.tables.clone());
            let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
            let body = ir
                .split("define i64 @folha(")
                .nth(1)
                .unwrap()
                .split("\n}")
                .next()
                .unwrap();
            let overflow = body
                .split("pilha.estouro:")
                .nth(1)
                .unwrap()
                .split("pilha.ok:")
                .next()
                .unwrap();
            assert!(overflow.contains("call void @df.lancar()"));
        }
    }

    #[test]
    fn stack_check_propagates_failure_and_requires_edge_in_caller() {
        let (mut functions, mut plans) = set();
        plans.get_mut("folha").unwrap().tables.checks_stack = true;
        let before = format!("{functions:?}");
        let plans_before = format!("{plans:?}");
        let error = insert_arc_dart_functions(&mut functions, &mut plans).unwrap_err();
        assert!(
            error.contains("caller") && error.contains("saídas excepcionais"),
            "{error}"
        );
        assert_eq!(format!("{functions:?}"), before);
        assert_eq!(format!("{plans:?}"), plans_before);
        // Com a saída preparada, o resultado só é transferido no sucesso.
        let (mut functions, mut plans) = set();
        functions.remove(1);
        plans.remove("intermediaria");
        let caller = &mut functions[0];
        if let Instruction::CallStatic { symbol, .. } = &mut caller.blocks[0].instructions[0].1 {
            *symbol = "folha".into();
        }
        caller.blocks[0].terminator = Terminator::CondBranch {
            cond: Operand::Constant(Constant::Bool(false)),
            then_block: BlockId(1),
            else_block: BlockId(2),
        };
        caller.blocks.push(BasicBlock {
            id: BlockId(1),
            instructions: vec![],
            terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
        });
        caller.blocks.push(BasicBlock {
            id: BlockId(2),
            instructions: vec![],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
        });
        let tables = &mut plans.get_mut("caller").unwrap().tables;
        tables.invocations.insert(ValueId(1), BlockId(1));
        tables.landing_pads.insert(BlockId(1));
        plans.get_mut("folha").unwrap().tables.checks_stack = true;
        assert_eq!(
            insert_arc_dart_functions(&mut functions, &mut plans).unwrap(),
            (1, 0)
        );
        assert!(plans["caller"].tokens.instructions[&ValueId(1)].may_fail);
        let leaf = &plans["folha"];
        let resumo = verify_dart_function_contract(
            &functions[1],
            &leaf.classes,
            &leaf.tokens,
            &leaf.tables,
            &leaf.scopes,
        )
        .unwrap();
        let mut classes = HashMap::new();
        let mut tokens = TokenPlan::default();
        produce_dart_calls(&functions[0], &[resumo], &mut classes, &mut tokens).unwrap();
        assert!(tokens.instructions[&ValueId(1)].may_fail);
    }

    #[test]
    fn scalar_arguments_with_phi_and_arithmetic_are_produced_in_the_set() {
        let create = Function {
            symbol: "criar".into(),
            name: "criar".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::I64,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Constant(Constant::Int(41)))),
            }],
        };
        let mut identity = create.clone();
        identity.symbol = "identidade_escalar".into();
        identity.params.push((ValueId(0), "x".into(), Type::I64));
        identity.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(0))));
        let caller = Function {
            symbol: "caller".into(),
            name: "caller".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::I64,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Constant(Constant::Bool(true)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(3),
                    instructions: vec![
                        (
                            ValueId(3),
                            Instruction::Phi {
                                ty: Type::I64,
                                incoming: vec![
                                    (BlockId(1), Operand::Val(ValueId(1))),
                                    (BlockId(2), Operand::Val(ValueId(2))),
                                ],
                            },
                            Type::I64,
                        ),
                        (
                            ValueId(4),
                            Instruction::Add(
                                Operand::Val(ValueId(3)),
                                Operand::Constant(Constant::Int(1)),
                            ),
                            Type::I64,
                        ),
                        (
                            ValueId(5),
                            Instruction::CallStatic {
                                symbol: identity.symbol.clone(),
                                args: vec![Operand::Val(ValueId(4))],
                                ret_ty: Type::I64,
                            },
                            Type::I64,
                        ),
                    ],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(5)))),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![(
                        ValueId(2),
                        Instruction::Const(Constant::Int(41)),
                        Type::I64,
                    )],
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::CallStatic {
                            symbol: create.symbol.clone(),
                            args: vec![],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    )],
                    terminator: Terminator::Branch(BlockId(3)),
                },
            ],
        };
        let mut functions = vec![caller, identity, create];
        let mut plans: HashMap<_, _> = functions
            .iter()
            .map(|f| (f.symbol.clone(), DartFunctionPlan::default()))
            .collect();
        // O parâmetro escalar do callee tem contrato semântico explícito.
        // Nenhuma classe/efeito é fornecido para o caller.
        plans
            .get_mut("identidade_escalar")
            .unwrap()
            .classes
            .insert(ValueId(0), Ownership::Trivial);
        plans
            .get_mut("caller")
            .unwrap()
            .owners_at_landing_pad
            .insert(BlockId(99), vec![ValueId(99)]);
        assert_eq!(
            insert_arc_dart_functions(&mut functions, &mut plans).unwrap(),
            (0, 0)
        );
        assert!(plans["caller"].owners_at_landing_pad.is_empty());
        for v in 1..=5 {
            assert_eq!(plans["caller"].classes[&ValueId(v)], Ownership::Trivial);
        }
        let (mut bad_set, mut bad_plans) = (functions, plans);
        bad_set[0].blocks[2].instructions[0] =
            (ValueId(2), Instruction::Const(Constant::Null), Type::Ref);
        let before = format!("{bad_set:?}");
        let plans_before = format!("{bad_plans:?}");
        assert!(
            insert_arc_dart_functions(&mut bad_set, &mut bad_plans)
                .unwrap_err()
                .contains("Phi")
        );
        assert_eq!(format!("{bad_set:?}"), before);
        assert_eq!(format!("{bad_plans:?}"), plans_before);
    }

    fn set() -> (Vec<Function>, HashMap<String, DartFunctionPlan>) {
        let leaf = Function {
            symbol: "folha".into(),
            name: "folha".into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        };
        let call = |sym_name: &str, dest: &str| {
            let mut f = leaf.clone();
            f.symbol = sym_name.into();
            f.blocks[0].instructions.push((
                ValueId(1),
                Instruction::CallStatic {
                    symbol: dest.into(),
                    args: vec![Operand::Val(ValueId(0))],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ));
            f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
            f
        };
        let functions = vec![
            call("caller", "intermediaria"),
            call("intermediaria", "folha"),
            leaf,
        ];
        let plans = functions
            .iter()
            .map(|f| {
                let mut p = DartFunctionPlan::default();
                p.tokens.ret = TokenReturn::Owned;
                (f.symbol.clone(), p)
            })
            .collect();
        (functions, plans)
    }

    #[test]
    fn dart_chain_prepares_later_callees_and_is_idempotent() {
        let (mut functions, mut plans) = set();
        assert_eq!(
            insert_arc_dart_functions(&mut functions, &mut plans).unwrap(),
            (1, 0)
        );
        for f in &functions {
            let p = &plans[&f.symbol];
            assert_eq!(p.classes[&ValueId(1)], Ownership::Owned);
            produce_and_verify_dart_tokens(
                f,
                &mut p.classes.clone(),
                &mut p.tokens.clone(),
                &p.tables,
                &p.scopes,
            )
            .unwrap();
        }
        assert!(matches!(
            functions[2].blocks[0].instructions[0].1,
            Instruction::ArcCopy {
                value: Operand::Val(ValueId(0))
            }
        ));
        let before = format!("{functions:?}");
        assert_eq!(
            insert_arc_dart_functions(&mut functions, &mut plans).unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{functions:?}"), before);
    }

    #[test]
    fn later_invalid_body_or_exitless_recursion_do_not_publish_set() {
        let (mut functions, mut plans) = set();
        functions[2].blocks[0].instructions.push((
            ValueId(1),
            Instruction::ArcDrop {
                value: Operand::Val(ValueId(0)),
            },
            Type::Void,
        ));
        let before = format!("{functions:?}");
        let plans_before = format!("{plans:?}");
        assert!(
            insert_arc_dart_functions(&mut functions, &mut plans)
                .unwrap_err()
                .contains("Owned")
        );
        assert_eq!(format!("{functions:?}"), before);
        assert_eq!(format!("{plans:?}"), plans_before);
        let (mut functions, mut plans) = set();
        if let Instruction::CallStatic { symbol, .. } = &mut functions[1].blocks[0].instructions[0].1
        {
            *symbol = "caller".into();
        }
        let before = format!("{functions:?}");
        let plans_before = format!("{plans:?}");
        assert!(
            insert_arc_dart_functions(&mut functions, &mut plans)
                .unwrap_err()
                .contains("saídas excepcionais")
        );
        assert_eq!(format!("{functions:?}"), before);
        assert_eq!(format!("{plans:?}"), plans_before);
    }
}

#[cfg(test)]
mod publication_tests {
    use super::*;

    fn val(n: u32) -> Operand {
        Operand::Val(ValueId(n))
    }

    // `call` em b0, a guarda de pendência e as duas saídas.
    fn saved(symbol: &str, params: Vec<(ValueId, String, Type)>, call: Instruction, ty: Type) -> Function {
        Function {
            symbol: symbol.into(), name: symbol.into(), debug_info: None, params, return_ty: ty,
            blocks: vec![
                BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(1), call, ty),
                    (ValueId(2), Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                    (ValueId(3), Instruction::ICmp(ICmpOp::Ne, val(2), Operand::Constant(Constant::Int(0))), Type::I1),
                ], terminator: Terminator::CondBranch { cond: val(3), then_block: BlockId(1), else_block: BlockId(2) } },
                BasicBlock { id: BlockId(1), instructions: vec![],
                    terminator: Terminator::Return(Some(super::super::super::operands::default_constant(ty))) },
                BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(val(1))) },
            ],
        }
    }

    #[test]
    fn runtime_function_running_dart_goes_through_scope_thunk() {
        let mut m = Module::new();
        m.arc_memory = true;
        m.functions.push(saved("unidade", vec![(ValueId(0), "s".into(), Type::Ref)],
            Instruction::CallRuntime {
                name: "dartforge_nativo_DartForge_string_codeUnitAt".into(),
                args: vec![(val(0), Type::Ref), (Operand::Constant(Constant::Int(0)), Type::I64)],
                ret_ty: Type::I64,
            }, Type::I64));
        let mut plans = HashMap::from([("unidade".into(), DartFunctionPlan::default())]);
        prepare_arc_dart_module(&mut m, &mut plans).unwrap();
        let name_str = super::super::NATIVE_SCOPE_PREFIX.to_string() + "dartforge_nativo_DartForge_string_codeUnitAt";
        assert!(matches!(&m.functions[0].blocks[0].instructions[0].1, Instruction::CallRuntime { name, .. } if *name == name_str));
        let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
        assert!(ir.contains(&format!("define internal i64 @{name_str}(ptr")) || ir.contains(&format!("define internal i64 @{name_str}(i64")), "{ir}");
        assert!(ir.contains("call i64 @dartforge_arc_escopo_nativo_abrir_v1()"));
        assert!(ir.contains("call void @dartforge_arc_escopo_nativo_fechar_v1(i64 %e)"));
        // Idempotente: o thunk não ganha outro prefixo.
        let before = format!("{m:?}");
        assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
        assert_eq!(format!("{m:?}"), before);
    }

    #[test]
    fn check_gets_pending_guard_and_tables_get_invoke() {
        for by_tables in [false, true] {
            let mut m = Module::new();
            m.arc_memory = true;
            // Recursiva: a análise a vê como falível.
            let mut fallible = Function {
                symbol: "falivel".into(), name: "falivel".into(), debug_info: None, params: vec![],
                return_ty: Type::Void, blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
                    terminator: Terminator::Return(None) }],
            };
            fallible.blocks[0].instructions.push((ValueId(0), Instruction::CallStatic {
                symbol: "falivel".into(), args: vec![], ret_ty: Type::Void }, Type::Void));
            let caller = Function {
                symbol: "chamador".into(), name: "chamador".into(), debug_info: None, params: vec![],
                return_ty: Type::Void, blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(0), Instruction::CallStatic { symbol: "falivel".into(), args: vec![], ret_ty: Type::Void }, Type::Void),
                ], terminator: Terminator::Return(None) }],
            };
            m.functions = vec![caller, fallible];
            if by_tables {
                crate::optimize::excecoes_por_tabelas(&mut m);
            }
            let mut plans: HashMap<_, _> = ["chamador", "falivel"].map(|s| (s.to_string(), DartFunctionPlan::default())).into();
            prepare_arc_dart_module(&mut m, &mut plans).unwrap();
            let p = &plans["chamador"];
            if by_tables {
                assert_eq!(p.tables.invocations.len(), 1);
                assert!(p.tokens.pendings.is_empty());
            } else {
                assert!(p.tables.invocations.is_empty());
                assert!(p.tokens.pendings.contains_key(&ValueId(0)));
                let b0 = &m.functions[0].blocks[0];
                assert!(matches!(&b0.instructions[1].1, Instruction::CallRuntime { name, .. } if name == "dartforge_exception_pending"));
                assert!(matches!(b0.terminator, Terminator::CondBranch { .. }));
            }
        }
    }

    #[test]
    fn callee_from_other_image_only_with_option_and_owned_result() {
        for externals in [false, true] {
            let mut m = Module::new();
            m.arc_memory = true;
            m.arc_external_callees = externals;
            m.functions.push(saved("programa", vec![(ValueId(0), "x".into(), Type::Ref)],
                Instruction::CallStatic { symbol: "df.sdk.externo".into(), args: vec![val(0)], ret_ty: Type::Ref },
                Type::Ref));
            m.dart_ref_returns.insert("programa".into());
            let mut plans = HashMap::from([("programa".into(), DartFunctionPlan::default())]);
            let result = prepare_arc_dart_module(&mut m, &mut plans);
            if !externals {
                assert!(result.unwrap_err().contains("callee Dart sem resumo"));
                continue;
            }
            result.unwrap();
            assert_eq!(plans["programa"].classes[&ValueId(1)], Ownership::Owned);
            assert!(plans["programa"].tokens.pendings.contains_key(&ValueId(1)));
        }
    }
}
