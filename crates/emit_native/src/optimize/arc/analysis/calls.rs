//! Solução local monotônica de aliases normais por SCC de chamadas (§28.3).
//! Compõe preservação/publicação de corpos positivos por chamadas diretas.
//! Demais efeitos permanecem opacos; não certifica políticas ARC.

use super::summaries::{ArcHeapSummary, ArcHeapReturn, extract_with_calls};
use crate::hir::*;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

/// Resume corpos alcançáveis pelas chamadas diretas das raízes fornecidas.
/// Alvos externos não são inventados. Bottom é interno: ciclos sem evidência
/// de retorno normal são elevados a topo antes de disponibilizar a SCC.
pub(super) fn resolver<'a>(
    m: &'a Module,
    roots: &HashSet<&str>,
    limit: usize,
) -> Result<HashMap<String, (&'a Function, ArcHeapSummary)>, String> {
    let mut index = HashMap::new();
    let mut positions = HashMap::new();
    for (position, f) in m.functions.iter().enumerate() {
        positions.insert(f.symbol.as_str(), position);
        if index.insert(f.symbol.as_str(), f).is_some() {
            return Err(format!("resumo: corpo duplicado {}", f.symbol));
        }
    }
    let mut seen_set = HashSet::new();
    let mut queue: VecDeque<_> = roots.iter().copied().collect();
    let mut bodies = Vec::new();
    while let Some(s) = queue.pop_front() {
        if !seen_set.insert(s) {
            continue;
        }
        let Some(&f) = index.get(s) else { continue };
        bodies.push(f);
        queue.extend(targets(f));
    }
    // Ordem nominal torna solução e diagnósticos independentes do HashSet.
    bodies.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    // Declaração é premissa de ABI, independente do orçamento de precisão.
    for f in &bodies {
        check_globals(m, f)?;
    }
    let facts: Vec<_> = bodies
        .iter()
        .map(|f| super::module::facts_in_module(m, f))
        .collect::<Result<_, _>>()?;
    let ids: HashMap<_, _> = bodies
        .iter()
        .enumerate()
        .map(|(i, f)| (f.symbol.as_str(), i))
        .collect();
    let edges: Vec<Vec<usize>> = bodies
        .iter()
        .map(|f| {
            let mut dests: Vec<_> = targets(f).filter_map(|s| ids.get(s).copied()).collect();
            dests.sort_unstable();
            dests.dedup();
            dests
        })
        .collect();
    let mut dependents = vec![Vec::new(); bodies.len()];
    for (v, dests) in edges.iter().enumerate() {
        for &w in dests {
            dependents[w].push(v);
        }
    }
    let empty = || ArcHeapReturn::Aliases {
        parameters: BTreeSet::new(),
        null: false,
    };
    let mut returns: Vec<_> = bodies
        .iter()
        .map(|f| {
            if f.return_ty == Type::Ref {
                empty()
            } else {
                ArcHeapReturn::UnknownValue
            }
        })
        .collect();
    for component in crate::optimize::scc::components(&edges) {
        let members: HashSet<_> = component.iter().copied().collect();
        let mut queue: VecDeque<_> = component.iter().copied().collect();
        let mut queued = members.clone();
        loop {
            while let Some(v) = queue.pop_front() {
                queued.remove(&v);
                let new = extract_with_calls(bodies[v], limit, true, |s| {
                    ids.get(s).map(|&w| (bodies[w], &returns[w]))
                })?;
                if join_set(&mut returns[v], new.ret(), limit) {
                    for &w in &dependents[v] {
                        if members.contains(&w) && queued.insert(w) {
                            queue.push_back(w);
                        }
                    }
                }
            }
            // Não expor bottom como ausência provada de aliases/null.
            let without_evidence: Vec<_> = component
                .iter()
                .copied()
                .filter(|&v| returns[v] == empty())
                .collect();
            if without_evidence.is_empty() {
                break;
            }
            for v in without_evidence {
                returns[v] = ArcHeapReturn::UnknownValue;
                for &w in &dependents[v] {
                    if members.contains(&w) && queued.insert(w) {
                        queue.push_back(w);
                    }
                }
            }
        }
    }
    // Reextração com dependências estabilizadas constrói o resumo guardado
    // pela assinatura do próprio corpo, sem publicar aproximações da worklist.
    let mut opaque_context = vec![false; bodies.len()];
    let mut summaries: Vec<_> = bodies
        .iter()
        .enumerate()
        .map(|(v, &f)| {
            let mut resumo = extract_with_calls(f, limit, false, |s| {
                ids.get(s).map(|&w| (bodies[w], &returns[w]))
            })?;
            let index = positions[f.symbol.as_str()];
            let table = if m.excecoes_por_tabelas {
                m.tables.get(index)
            } else {
                None
            };
            if (m.excecoes_por_tabelas && table.is_none())
                || crate::llvm::LlvmEmitter::requires_explicit_context(f, table)
                || crate::llvm::LlvmEmitter::may_require_context_for_roots(f)
                || table.is_some_and(|t| {
                    !t.invocations.is_empty()
                        || !t.landing_pads.is_empty()
                        || !t.exits.is_empty()
                        || t.foreign_cleanup.is_some()
                })
            {
                opaque_context[v] = true;
                resumo.invalidate_coverage();
            }
            debug_assert_eq!(resumo.ret(), &returns[v]);
            Ok(resumo)
        })
        .collect::<Result<_, String>>()?;
    // O estado inicial é desconhecido nas chamadas. Só promover quando
    // todas as dependências já têm cobertura positiva; nunca supor uma
    // SCC recursiva pura apenas porque nenhum store foi encontrado nela.
    let mut queue: VecDeque<_> = (0..bodies.len()).collect();
    let mut queued: HashSet<_> = (0..bodies.len()).collect();
    while let Some(v) = queue.pop_front() {
        queued.remove(&v);
        if opaque_context[v]
            || (summaries[v].fields() == super::summaries::ArcHeapFields::Preserved
                && summaries[v].publications() == super::summaries::ArcHeapPublications::ResultOnly)
        {
            continue;
        }
        if super::summaries::coverage_with_calls(bodies[v], |s| {
            ids.get(s).map(|&w| (bodies[w], &summaries[w]))
        }) {
            summaries[v].mark_coverage();
            for &w in &dependents[v] {
                if queued.insert(w) {
                    queue.push_back(w);
                }
            }
        }
    }
    // Inclusões de stores têm ponto fixo próprio, sem promover a cobertura
    // de campos/publicações nem ignorar prólogos/saídas opacos. Recursão
    // propaga inclusões positivas mesmo com opacidade; orçamento excedido
    // é absorvente e não autoriza reconstruir um conjunto truncado.
    let mut writes = vec![super::writes::WriteInclusions::empty(); bodies.len()];
    let mut globals = vec![super::writes::PublicationInclusions::empty(); bodies.len()];
    let mut paths: Vec<_> = bodies
        .iter()
        .map(|f| {
            if f.return_ty == Type::Ref {
                super::writes::ReturnPaths::empty()
            } else {
                super::writes::ReturnPaths::unknown()
            }
        })
        .collect();
    for component in crate::optimize::scc::components(&edges) {
        let members: HashSet<_> = component.iter().copied().collect();
        let mut queue: VecDeque<_> = component.iter().copied().collect();
        let mut queued = members.clone();
        loop {
            while let Some(v) = queue.pop_front() {
                queued.remove(&v);
                let (result, new, publications) = super::writes::extract_with_origins(
                    bodies[v],
                    limit,
                    true,
                    &facts[v].objects,
                    |s| {
                        ids.get(s).map(|&w| {
                            (
                                bodies[w],
                                paths[w].clone(),
                                &writes[w],
                                globals[w].clone(),
                            )
                        })
                    },
                )?;
                let write_changed = writes[v].join_set(&new, limit);
                let global_changed = globals[v].join_set(&publications, limit);
                if paths[v].join_set(&result, limit) || write_changed || global_changed {
                    for &w in &dependents[v] {
                        if members.contains(&w) && queued.insert(w) {
                            queue.push_back(w);
                        }
                    }
                }
            }
            let without_evidence: Vec<_> = component
                .iter()
                .copied()
                .filter(|&v| paths[v] == super::writes::ReturnPaths::empty())
                .collect();
            if without_evidence.is_empty() {
                break;
            }
            for v in without_evidence {
                paths[v] = super::writes::ReturnPaths::unknown();
                for &w in &dependents[v] {
                    if members.contains(&w) && queued.insert(w) {
                        queue.push_back(w);
                    }
                }
            }
        }
    }
    for (((resumo, inclusions), ret), publications) in
        summaries.iter_mut().zip(writes).zip(paths).zip(globals)
    {
        resumo.write_inclusions = inclusions;
        resumo.return_paths = ret;
        resumo.publication_inclusions = publications;
    }
    Ok(bodies
        .into_iter()
        .zip(summaries)
        .map(|(f, resumo)| (f.symbol.clone(), (f, resumo)))
        .collect())
}

pub(super) fn check_globals(m: &Module, f: &Function) -> Result<(), String> {
    for (_, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
        let symbol = match i {
            Instruction::StoreGlobal {
                symbol,
                ty: Type::Ref,
                ..
            }
            | Instruction::ArcStoreStrong {
                slot: StrongSlot::Global { symbol },
                ..
            }
            | Instruction::ArcLoadStrong {
                slot: StrongSlot::Global { symbol },
            } => Some(symbol),
            _ => None,
        };
        if let Some(symbol) = symbol {
            let declarations: Vec<_> = m.globals.iter().filter(|(_, _, s)| s == symbol).collect();
            if declarations.len() != 1 || declarations[0].1 != Type::Ref {
                return Err(format!(
                    "publicacoes ARC: global @{symbol} exige declaração Ref única"
                ));
            }
        }
    }
    Ok(())
}

fn targets(f: &Function) -> impl Iterator<Item = &str> {
    f.blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .filter_map(|(_, i, _)| {
            if let Instruction::CallStatic { symbol, .. } = i {
                Some(symbol.as_str())
            } else {
                None
            }
        })
}

fn join_set(dest: &mut ArcHeapReturn, origin: &ArcHeapReturn, limit: usize) -> bool {
    if *dest == ArcHeapReturn::UnknownValue {
        return false;
    }
    let previous = dest.clone();
    match origin {
        ArcHeapReturn::UnknownValue => *dest = ArcHeapReturn::UnknownValue,
        ArcHeapReturn::Aliases {
            parameters: ps,
            null: ns,
        } => {
            if let ArcHeapReturn::Aliases { parameters, null } = dest {
                parameters.extend(ps);
                *null |= ns;
                if parameters.len() > limit {
                    *dest = ArcHeapReturn::UnknownValue;
                }
            }
        }
    }
    *dest != previous
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar(name: &str, alvo: Option<&str>) -> Function {
        Function {
            symbol: name.into(),
            name: name.into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::I64)],
            return_ty: Type::I64,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: alvo
                    .into_iter()
                    .map(|symbol| {
                        (
                            ValueId(1),
                            Instruction::CallStatic {
                                symbol: symbol.into(),
                                args: vec![Operand::Val(ValueId(0))],
                                ret_ty: Type::I64,
                            },
                            Type::I64,
                        )
                    })
                    .collect(),
                terminator: Terminator::Return(Some(Operand::Val(ValueId(if alvo.is_some() {
                    1
                } else {
                    0
                })))),
            }],
        }
    }

    #[test]
    fn composite_coverage_deep_chain_and_body_order() {
        let mut m = Module::new();
        for v in 0..1000 {
            let alvo = (v < 999).then(|| format!("f{}", v + 1));
            m.functions.push(scalar(&format!("f{v}"), alvo.as_deref()));
        }
        for _ in 0..2 {
            let r = resolver(&m, &HashSet::from(["f0"]), 8).unwrap();
            assert_eq!(r.len(), 1000);
            for (_, resumo) in r.values() {
                assert_eq!(
                    resumo.fields(),
                    super::super::summaries::ArcHeapFields::Preserved
                );
                assert_eq!(
                    resumo.publications(),
                    super::super::summaries::ArcHeapPublications::ResultOnly
                );
            }
            m.functions.reverse();
        }
    }

    #[test]
    fn composite_coverage_rejects_opaque_dependency_recursion_and_abi() {
        use super::super::summaries::*;
        let base = vec![
            scalar("f", Some("g")),
            scalar("g", Some("h")),
            scalar("h", None),
        ];
        for case in 0..4 {
            let mut m = Module::new();
            m.functions = base.clone();
            match case {
                0 => m.functions[2] = scalar("h", Some("externo")),
                1 => m.functions[2] = scalar("h", Some("g")),
                2 => {
                    m.functions[2].blocks[0].terminator =
                        Terminator::Throw(Operand::Constant(Constant::Null))
                }
                3 => {
                    if let Instruction::CallStatic { args, .. } =
                        &mut m.functions[1].blocks[0].instructions[0].1
                    {
                        args[0] = Operand::Constant(Constant::Null);
                    }
                }
                _ => unreachable!(),
            }
            let r = resolver(&m, &HashSet::from(["f"]), 8).unwrap();
            assert_eq!(
                r["f"].1.fields(),
                ArcHeapFields::UnknownSet,
                "caso {case}"
            );
            assert_eq!(
                r["f"].1.publications(),
                ArcHeapPublications::UnknownValues,
                "caso {case}"
            );
        }
    }

    #[test]
    fn composite_coverage_does_not_reapprove_missing_context_or_table() {
        use super::super::summaries::*;
        let mut m = Module::new();
        m.functions = vec![scalar("f", Some("g")), scalar("g", None)];
        m.excecoes_por_tabelas = true;
        for tables in [
            vec![],
            vec![
                FunctionTables::default(),
                FunctionTables {
                    checks_stack: true,
                    ..Default::default()
                },
            ],
        ] {
            m.tables = tables;
            let r = resolver(&m, &HashSet::from(["f"]), 8).unwrap();
            assert_eq!(r["f"].1.fields(), ArcHeapFields::UnknownSet);
            assert_eq!(r["g"].1.publications(), ArcHeapPublications::UnknownValues);
        }
        m.tables = vec![FunctionTables::default(); 2];
        let r = resolver(&m, &HashSet::from(["f"]), 8).unwrap();
        assert_eq!(r["f"].1.fields(), ArcHeapFields::Preserved);
        // Alterar o próprio prólogo impede a propagação mesmo com g coberto.
        m.tables[0].checks_stack = true;
        let r = resolver(&m, &HashSet::from(["f"]), 8).unwrap();
        assert_eq!(r["f"].1.fields(), ArcHeapFields::UnknownSet);
        assert_eq!(r["g"].1.fields(), ArcHeapFields::Preserved);
    }

    #[test]
    fn composite_coverage_consumed_in_module_invalidates_transitive_dependency() {
        use super::super::{escape::*, module::analyze_in_module};
        let mut m = Module::new();
        m.functions = vec![
            scalar("f", Some("g")),
            scalar("g", Some("h")),
            scalar("h", None),
        ];
        assert!(analyze_in_module(&m, "f", 8).unwrap().is_none());
        m.arc_memory = true;
        let a = analyze_in_module(&m, "f", 8).unwrap().unwrap();
        assert!(a.unknown_set().is_empty());
        let e = compute_in_module(&m, "f", &a).unwrap();
        assert!(
            !e.publications
                .keys()
                .any(|c| matches!(c, EscapeCause::OpaqueOperation(_)))
        );
        // Efeito implícito de materialização em h, apesar do retorno escalar:
        // invalida a solução antiga de f e torna a nova chamada opaca.
        m.functions[2].blocks[0].instructions.push((
            ValueId(2),
            Instruction::Const(Constant::String("novo efeito".into())),
            Type::Ref,
        ));
        assert!(compute_in_module(&m, "f", &a).is_err());
        let new_one = analyze_in_module(&m, "f", 8).unwrap().unwrap();
        assert_eq!(new_one.unknown_set(), &[ValueId(1)]);
        assert!(
            compute_in_module(&m, "f", &new_one)
                .unwrap()
                .publications
                .contains_key(&EscapeCause::OpaqueOperation(1))
        );
    }

    #[test]
    fn chain_with_computation_preserves_fields_and_rejects_conversion_or_fallible_effect() {
        use super::super::{escape::*, module::analyze_in_module, summaries::*};
        let mut m = Module::new();
        m.arc_memory = true;
        m.arc_field_layouts.insert(1, vec![Type::Ref]);
        m.arc_field_layouts.insert(2, vec![]);
        m.functions.push(Function {
            symbol: "caller".into(),
            name: "caller".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(0),
                        Instruction::AllocObject {
                            class_id: 2,
                            fields: vec![],
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(1),
                        Instruction::AllocObject {
                            class_id: 1,
                            fields: vec![Operand::Val(ValueId(0))],
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(2),
                        Instruction::CallStatic {
                            symbol: "f0".into(),
                            args: vec![Operand::Constant(Constant::Int(4))],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    ),
                    (
                        ValueId(3),
                        Instruction::GetField {
                            object: Operand::Val(ValueId(1)),
                            index: 0,
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
            }],
        });
        super::super::origins::register(&mut m, 0, true);
        for v in 0..64 {
            let alvo = (v < 63).then(|| format!("f{}", v + 1));
            m.functions.push(scalar(&format!("f{v}"), alvo.as_deref()));
        }
        m.functions[64].blocks[0].instructions.push((
            ValueId(1),
            Instruction::Add(
                Operand::Val(ValueId(0)),
                Operand::Constant(Constant::Int(1)),
            ),
            Type::I64,
        ));
        m.functions[64].blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
        for _ in 0..2 {
            let r = resolver(&m, &HashSet::from(["f0"]), 8).unwrap();
            for (_, resumo) in r.values() {
                assert_eq!(resumo.fields(), ArcHeapFields::Preserved);
                assert_eq!(resumo.publications(), ArcHeapPublications::ResultOnly);
                assert_eq!(resumo.ret(), &ArcHeapReturn::UnknownValue);
            }
            let a = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
            assert_eq!(a.value(ValueId(0)), a.value(ValueId(3)));
            assert_eq!(a.value(ValueId(3)).unwrap().nos().unwrap().len(), 1);
            assert!(a.unknown_set().is_empty());
            // Resultado I64 não recebe prova de representação trivial/frescor.
            assert!(a.value(ValueId(2)).unwrap().nos().is_none());
            assert!(
                !compute_in_module(&m, "caller", &a)
                    .unwrap()
                    .publications
                    .contains_key(&EscapeCause::OpaqueOperation(2))
            );
            m.functions.reverse();
            compute_in_module(&m, "caller", &a).unwrap();
        }
        let previous = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
        for i in [
            Instruction::SDiv(
                Operand::Val(ValueId(0)),
                Operand::Constant(Constant::Int(0)),
            ),
            Instruction::SRem(
                Operand::Val(ValueId(0)),
                Operand::Constant(Constant::Int(0)),
            ),
        ] {
            m.functions[64].blocks[0].instructions[0].1 = i;
            assert!(compute_in_module(&m, "caller", &previous).is_err());
            let a = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
            assert!(a.value(ValueId(3)).unwrap().nos().is_none());
            assert_eq!(a.unknown_set(), &[ValueId(2)]);
            assert!(
                compute_in_module(&m, "caller", &a)
                    .unwrap()
                    .publications
                    .contains_key(&EscapeCause::OpaqueOperation(2))
            );
        }
        m.functions[64].blocks[0].instructions[0].1 = Instruction::Add(
            Operand::Val(ValueId(0)),
            Operand::Constant(Constant::Int(1)),
        );
        for arg in [
            Constant::Double(4.0),
            Constant::Null,
            Constant::String("4".into()),
        ] {
            if let Instruction::CallStatic { args, .. } =
                &mut m.functions[0].blocks[0].instructions[2].1
            {
                args[0] = Operand::Constant(arg);
            }
            let a = analyze_in_module(&m, "caller", 8).unwrap().unwrap();
            assert!(a.value(ValueId(3)).unwrap().nos().is_none());
            assert_eq!(a.unknown_set(), &[ValueId(2)]);
        }
        m.arc_memory = false;
        assert!(analyze_in_module(&m, "caller", 8).unwrap().is_none());
    }

    fn body(name: &str, base: Option<Operand>, targets: &[String]) -> Function {
        let mut options: Vec<_> = base.into_iter().map(|o| (vec![], o)).collect();
        for (k, s) in targets.iter().enumerate() {
            let v = ValueId(3 + k as u32);
            options.push((
                vec![(
                    v,
                    Instruction::CallStatic {
                        symbol: s.clone(),
                        args: vec![
                            Operand::Val(ValueId(0)),
                            Operand::Val(ValueId(1)),
                            Operand::Val(ValueId(2)),
                        ],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )],
                Operand::Val(v),
            ));
        }
        let mut blocks = Vec::new();
        let n = options.len();
        for (k, (instructions, ret)) in options.into_iter().enumerate() {
            let id = BlockId(2 * k as u32);
            let last_one = k + 1 == n;
            if !last_one {
                blocks.push(BasicBlock {
                    id,
                    instructions: vec![],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(2)),
                        then_block: BlockId(id.0 + 1),
                        else_block: BlockId(id.0 + 2),
                    },
                });
            }
            blocks.push(BasicBlock {
                id: if last_one { id } else { BlockId(id.0 + 1) },
                instructions,
                terminator: Terminator::Return(Some(ret)),
            });
        }
        if blocks.is_empty() {
            blocks.push(BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(0)),
            });
        }
        Function {
            symbol: name.into(),
            name: name.into(),
            debug_info: None,
            params: vec![
                (ValueId(0), "x".into(), Type::Ref),
                (ValueId(1), "y".into(), Type::Ref),
                (ValueId(2), "cond".into(), Type::I1),
            ],
            return_ty: Type::Ref,
            blocks,
        }
    }

    #[test]
    fn scc_matches_aliases_with_independent_reach_in_512_graphs() {
        for mask in 0..512 {
            let mut m = Module::new();
            let mut reachability = [[false; 3]; 3];
            for (v, line) in reachability.iter_mut().enumerate() {
                line[v] = true;
                let targets: Vec<_> = (0..3)
                    .filter(|&w| mask & (1 << (3 * v + w)) != 0)
                    .map(|w| {
                        line[w] = true;
                        format!("f{w}")
                    })
                    .collect();
                m.functions.push(body(
                    &format!("f{v}"),
                    Some(Operand::Val(ValueId((v % 2) as u32))),
                    &targets,
                ));
            }
            // Floyd-Warshall sobre o grafo original, sem usar a SCC/worklist.
            for k in 0..3 {
                for v in 0..3 {
                    for w in 0..3 {
                        reachability[v][w] |= reachability[v][k] && reachability[k][w];
                    }
                }
            }
            for limit in 0..=2 {
                let r = resolver(&m, &HashSet::from(["f0", "f1", "f2"]), limit).unwrap();
                for (v, line) in reachability.iter().enumerate() {
                    let parameters: BTreeSet<_> =
                        (0..3).filter(|&w| line[w]).map(|w| w % 2).collect();
                    let expected = if parameters.len() > limit {
                        ArcHeapReturn::UnknownValue
                    } else {
                        ArcHeapReturn::Aliases {
                            parameters,
                            null: false,
                        }
                    };
                    assert_eq!(
                        r[&format!("f{v}")].1.ret(),
                        &expected,
                        "grafo {mask}, limite {limit}, nó {v}"
                    );
                }
            }
        }
    }

    #[test]
    fn permuted_arguments_in_recursion_join_real_aliases() {
        let mut m = Module::new();
        m.functions.push(body("f", None, &["g".into()]));
        let mut g = body("g", Some(Operand::Val(ValueId(0))), &["f".into()]);
        let call = g
            .blocks
            .last_mut()
            .unwrap()
            .instructions
            .last_mut()
            .unwrap();
        if let Instruction::CallStatic { args, .. } = &mut call.1 {
            args.swap(0, 1);
        }
        m.functions.push(g);
        let r = resolver(&m, &HashSet::from(["f"]), 2).unwrap();
        assert!(r.values().all(|(_, r)| r.ret()
            == &ArcHeapReturn::Aliases {
                parameters: BTreeSet::from([0, 1]),
                null: false,
            }));
        let r = resolver(&m, &HashSet::from(["f"]), 1).unwrap();
        assert!(
            r.values()
                .all(|(_, r)| r.ret() == &ArcHeapReturn::UnknownValue)
        );
    }

    #[test]
    fn baseless_null_recursion_and_external_target_do_not_forge_empty_facts() {
        let mut m = Module::new();
        m.functions.push(body("f", None, &["g".into()]));
        m.functions.push(body(
            "g",
            Some(Operand::Constant(Constant::Null)),
            &["f".into()],
        ));
        let r = resolver(&m, &HashSet::from(["f"]), 8).unwrap();
        for (_, resumo) in r.values() {
            assert_eq!(
                resumo.ret(),
                &ArcHeapReturn::Aliases {
                    parameters: BTreeSet::new(),
                    null: true
                }
            );
        }
        m.functions[1] = body("g", None, &["f".into()]);
        let r = resolver(&m, &HashSet::from(["f"]), 8).unwrap();
        assert!(
            r.values()
                .all(|(_, r)| r.ret() == &ArcHeapReturn::UnknownValue)
        );
        m.functions[1] = body("g", Some(Operand::Val(ValueId(0))), &["externo".into()]);
        let r = resolver(&m, &HashSet::from(["f"]), 8).unwrap();
        assert!(
            r.values()
                .all(|(_, r)| r.ret() == &ArcHeapReturn::UnknownValue)
        );
        assert!(!r.contains_key("externo"));
    }

    #[test]
    fn deep_chain_independent_of_body_order_without_rust_recursion() {
        let mut m = Module::new();
        for v in 0..2000 {
            m.functions.push(if v == 1999 {
                body(&format!("f{v}"), Some(Operand::Val(ValueId(1))), &[])
            } else {
                body(&format!("f{v}"), None, &[format!("f{}", v + 1)])
            });
        }
        for _ in 0..2 {
            let r = resolver(&m, &HashSet::from(["f0"]), 8).unwrap();
            assert_eq!(r.len(), 2000);
            assert!(r.values().all(|(_, r)| r.ret()
                == &ArcHeapReturn::Aliases {
                    parameters: BTreeSet::from([1]),
                    null: false
                }));
            m.functions.reverse();
        }
    }
}
