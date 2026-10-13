//! Publicações locais e seu alcance transitivo (§28.2 e §30.1).
//! Não calcula regiões/observadores ou efeitos interprocedurais de chamadas.

use super::{hir::*, model::*};
use crate::hir::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Destino conservador de publicação; operação opaca pode reter ou reentrar.
/// Isso não significa que toda publicação possível ocorra numa execução.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analise::escape::CausaEscape;
/// assert_ne!(CausaEscape::Retorno, CausaEscape::Excecao);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum EscapeCause {
    /// Valor devolvido ao chamador e seus descendentes possíveis.
    Return,
    /// Valor lançado, incluindo os objetos alcançáveis do payload.
    Exception,
    /// Publicação numa raiz global nominal.
    Global(String),
    /// ID local de uma operação sem resumo de heap/retencão válido.
    OpaqueOperation(u32),
    /// Prólogo/saída inserido na emissão, sem resumo de heap validado.
    ImplicitEmission,
}

/// Alcance possível publicado, agrupado por causa. Topo impede prova negativa.
/// Ausência de causa aqui não prova confinamento: observadores/externs/SDK,
/// callbacks, vidas e saídas excepcionais ainda exigem cobertura completa.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analise::escape::ResultadoEscape;
/// assert!(ResultadoEscape::default().publicacoes.is_empty());
/// ```
#[derive(Debug, Default)]
pub struct EscapeResult {
    /// Conjuntos transitivos, ou topo quando qualquer fronteira é desconhecida.
    pub publications: BTreeMap<EscapeCause, PointSet>,
}

/// Calcula publicações sobre a solução da mesma versão local da função.
/// Percorre todos os campos do esquema de cada nó e conserva ciclos/aliases;
/// esquema/campo ausente, campo opaco e budget excedido propagam topo.
/// Operação opaca recebe alcance desconhecido, inclusive sem argumentos,
/// pois pode acessar globais/callbacks não resumidos. Não infere noescape
/// do contrato Borrowed da ABI. Não transforma HIR nem seleciona política.
///
/// # Erros
/// O corpo mudou depois da solução points-to, ou há dependências de callees
/// que precisam ser conferidas por [`compute_in_module`]. Recalcular quando necessário.
/// O hash é uma guarda local, não dependência versionada SDK/JIT.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::analise::{hir::*, escape::*}};
/// let f = Function { symbol: "id".into(), name: "id".into(), depuracao: None,
/// params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
/// blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
/// terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] };
/// let a = analisar(&f, &FatosHir::default(), 8)?;
/// assert!(calcular(&f, &a)?.publicacoes[&CausaEscape::Retorno].nos().is_none());
/// # Ok::<(), String>(())
/// ```
pub fn compute(f: &Function, a: &HirAnalysis) -> Result<EscapeResult, String> {
    if !a.body_dependencies.is_empty()
        || !a.table_dependencies.is_empty()
        || a.module_premises.is_some()
    {
        return Err("escape ARC: validar dependências pelo módulo".into());
    }
    compute_validated(f, a)
}

/// Confere corpos e tabelas dos callees usados para aliases/campos, antes do escape.
/// Confere modo de memória, sítios do chamador e layouts locais quando a
/// solução veio do módulo. Não valida pins, gerações JIT ou resumos SDK.
/// Caminhos implícitos registrados pelo módulo publicam topo sob
/// [`EscapeCause::ImplicitEmission`], sem fabricar instrução HIR.
///
/// # Erros
/// Corpo/callee ausente, duplicado, alterado, tabela de emissão modificada
/// ou premissas locais de modo/sítios/layouts diferentes.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::analise::{hir::*, escape::*}};
/// let f = Function { symbol: "f".into(), name: "f".into(), depuracao: None,
/// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
/// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
/// let a = analisar(&f, &FatosHir::default(), 8)?;
/// let mut m = Module::new(); m.functions.push(f);
/// assert!(calcular_no_modulo(&m, "f", &a)?.publicacoes.is_empty());
/// # Ok::<(), String>(())
/// ```
pub fn compute_in_module(
    m: &Module,
    symbol: &str,
    a: &HirAnalysis,
) -> Result<EscapeResult, String> {
    compute_validated(check_analysis_in_module(m, symbol, a)?, a)
}

// Guarda comum aos consumidores de escape e de grafo, sem resolver points-to.
pub(super) fn check_analysis_in_module<'m>(
    m: &'m Module,
    symbol: &str,
    a: &HirAnalysis,
) -> Result<&'m Function, String> {
    if let Some(signature) = a.module_premises
        && signature != super::module::premises_signature(m, symbol)
    {
        return Err("escape ARC: modo, sítios ou layouts mudaram".into());
    }
    let mut index = std::collections::HashMap::new();
    let positions: std::collections::HashMap<_, _> = m
        .functions
        .iter()
        .enumerate()
        .map(|(i, f)| (f.symbol.as_str(), i))
        .collect();
    for f in &m.functions {
        if index.insert(&f.symbol, f).is_some() {
            return Err("escape ARC: corpo duplicado".into());
        }
    }
    for (s, signature) in &a.body_dependencies {
        let f = index.get(s).ok_or("escape ARC: callee ausente")?;
        if body_signature(f) != *signature {
            return Err(format!("escape ARC: callee mudou {s}"));
        }
    }
    for (s, signature) in &a.table_dependencies {
        let index = positions
            .get(s.as_str())
            .ok_or("escape ARC: callee ausente")?;
        if super::hir::table_state(m, *index) != *signature {
            return Err(format!("escape ARC: tabela mudou {s}"));
        }
    }
    let f = index
        .get(&symbol.to_string())
        .ok_or("escape ARC: corpo ausente")?;
    if a.body != body_signature(f) {
        return Err("escape ARC: corpo mudou após points-to".into());
    }
    Ok(f)
}

fn compute_validated(f: &Function, a: &HirAnalysis) -> Result<EscapeResult, String> {
    if a.body != body_signature(f) {
        return Err("escape ARC: corpo mudou após points-to".into());
    }
    let mut r = EscapeResult::default();
    let operand = |o: &Operand| match o {
        Operand::Val(v) => a
            .value(*v)
            .cloned()
            .unwrap_or_else(|| PointSet::unknown(a.limit)),
        Operand::Constant(Constant::Null) => PointSet::empty(a.limit),
        _ => PointSet::unknown(a.limit),
    };
    let mut publish = |cause, roots: PointSet| {
        let dests = reachability(a, &roots);
        r.publications
            .entry(cause)
            .or_insert_with(|| PointSet::empty(a.limit))
            .join_set(&dests);
    };
    for b in &f.blocks {
        match &b.terminator {
            Terminator::Return(Some(o)) => publish(EscapeCause::Return, operand(o)),
            Terminator::Throw(o) => publish(EscapeCause::Exception, operand(o)),
            _ => {}
        }
        for (_, i, _) in &b.instructions {
            match i {
                Instruction::StoreGlobal { symbol, val, .. }
                | Instruction::ArcStoreStrong {
                    slot: StrongSlot::Global { symbol },
                    value: val,
                    ..
                } => publish(EscapeCause::Global(symbol.clone()), operand(val)),
                _ => {}
            }
        }
    }
    // Inclusões não dispensam a publicação opaca da chamada nem da emissão.
    for (cause, variable) in &a.publication_inclusions {
        let points = a
            .points()
            .variable(*variable)
            .ok_or("escape ARC: variável de publicação inválida")?;
        publish(cause.clone(), points.clone());
    }
    for id in a.unknown_set() {
        publish(
            EscapeCause::OpaqueOperation(id.0),
            PointSet::unknown(a.limit),
        );
    }
    if a.opaque_implicit_emission {
        publish(
            EscapeCause::ImplicitEmission,
            PointSet::unknown(a.limit),
        );
    }
    Ok(r)
}

fn reachability(a: &HirAnalysis, roots: &PointSet) -> PointSet {
    let mut result = roots.clone();
    let Some(nodes) = roots.nodes() else {
        return result;
    };
    let mut queue: VecDeque<_> = nodes.iter().cloned().collect();
    let mut seen_set = BTreeSet::new();
    while let Some(node) = queue.pop_front() {
        if !seen_set.insert(node.clone()) {
            continue;
        }
        let Some(fields) = a.schemas.get(&node) else {
            return PointSet::unknown(a.limit);
        };
        // As inclusões podem representar referências laterais (anexos), sem
        // campo físico. Elas também publicam transitivamente seus destinos.
        for &(object, value) in &a.write_inclusions {
            let Some(dests) = a.points().variable(value) else {
                return PointSet::unknown(a.limit);
            };
            if dests.nodes().is_some_and(|ns| ns.is_empty()) {
                continue;
            }
            let Some(origins) = a.points().variable(object).and_then(|p| p.nodes()) else {
                return PointSet::unknown(a.limit);
            };
            if !origins.contains(&node) {
                continue;
            }
            let Some(nodes) = dests.nodes() else {
                return PointSet::unknown(a.limit);
            };
            queue.extend(nodes.iter().cloned());
            result.join_set(dests);
            if result.nodes().is_none() {
                return result;
            }
        }
        for field in fields {
            let Some(dests) = a.points().field(&node, field) else {
                return PointSet::unknown(a.limit);
            };
            let Some(nodes) = dests.nodes() else {
                return PointSet::unknown(a.limit);
            };
            for child in nodes {
                queue.push_back(child.clone());
            }
            result.join_set(dests);
            if result.nodes().is_none() {
                return result;
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::super::{hir::ObjectOrigin, points_to::ArcField};
    use super::*;
    use std::collections::HashMap;

    fn graph(mask: usize) -> (Function, HirFacts) {
        let field = ArcField {
            layout: "No".into(),
            position: 0,
        };
        let mut facts = HirFacts::default();
        let mut instructions = Vec::new();
        for v in 0..3 {
            instructions.push((
                ValueId(v),
                Instruction::AllocObject {
                    class_id: 1,
                    fields: vec![Operand::Constant(Constant::Null)],
                },
                Type::Ref,
            ));
            facts.objects.insert(
                ValueId(v),
                ObjectOrigin {
                    node: AbstractNode::Allocation {
                        site: ArcSite {
                            function: "f".into(),
                            origin: v as u64,
                            specialization: "".into(),
                        },
                        context: "".into(),
                    },
                    fields: vec![field.clone()],
                },
            );
        }
        for v in 0..3 {
            for w in 0..3 {
                if mask & (1 << (3 * v + w)) != 0 {
                    let id = ValueId(instructions.len() as u32);
                    instructions.push((
                        id,
                        Instruction::SetField {
                            object: Operand::Val(ValueId(v)),
                            index: 0,
                            value: Operand::Val(ValueId(w)),
                        },
                        Type::Void,
                    ));
                    facts.accesses.insert(id, field.clone());
                }
            }
        }
        (
            Function {
                symbol: "f".into(),
                name: "f".into(),
                debug_info: None,
                params: vec![],
                return_ty: Type::Ref,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions,
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
                }],
            },
            facts,
        )
    }

    #[test]
    fn return_publishes_descendants_in_all_three_node_graphs() {
        for mask in 0..512 {
            let (f, facts) = graph(mask);
            let a = analyze(&f, &facts, 3).unwrap();
            let r = compute(&f, &a).unwrap();
            // Oráculo por expansão de alcance, sem consultar campos do solver.
            let mut live_set = BTreeSet::from([0]);
            loop {
                let before = live_set.clone();
                for v in before.iter().copied() {
                    for w in 0..3 {
                        if mask & (1 << (3 * v + w)) != 0 {
                            live_set.insert(w);
                        }
                    }
                }
                if live_set == before {
                    break;
                }
            }
            let expected: BTreeSet<_> = live_set
                .iter()
                .map(|v| facts.objects[&ValueId(*v as u32)].node.clone())
                .collect();
            assert_eq!(
                r.publications[&EscapeCause::Return].nodes().unwrap(),
                &expected
            );
        }
    }

    #[test]
    fn exception_publishes_payload_and_stale_analysis_is_rejected() {
        let (mut f, facts) = graph(1 << 1);
        f.blocks[0].terminator = Terminator::Throw(Operand::Val(ValueId(0)));
        let a = analyze(&f, &facts, 3).unwrap();
        let r = compute(&f, &a).unwrap();
        assert_eq!(r.publications[&EscapeCause::Exception].nodes().unwrap().len(), 2);
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Constant(Constant::Null)));
        assert!(compute(&f, &a).is_err());
    }

    #[test]
    fn opacity_and_budget_never_prove_confinement() {
        let (mut f, facts) = graph(1 << 1);
        f.blocks[0].instructions.push((
            ValueId(9),
            Instruction::CallRuntime {
                name: "opaca".into(),
                args: vec![],
                ret_ty: Type::Void,
            },
            Type::Void,
        ));
        let a = analyze(&f, &facts, 3).unwrap();
        let r = compute(&f, &a).unwrap();
        assert!(r.publications[&EscapeCause::Return].nodes().is_none());
        assert!(
            r.publications[&EscapeCause::OpaqueOperation(9)]
                .nodes()
                .is_none()
        );
        f.blocks[0].instructions.pop();
        let a = analyze(&f, &facts, 1).unwrap();
        assert!(
            compute(&f, &a).unwrap().publications[&EscapeCause::Return]
                .nodes()
                .is_none()
        );
        let a = analyze(
            &f,
            &HirFacts {
                objects: HashMap::new(),
                accesses: HashMap::new(),
            },
            3,
        )
        .unwrap();
        assert!(
            compute(&f, &a).unwrap().publications[&EscapeCause::Return]
                .nodes()
                .is_none()
        );
    }

    #[test]
    fn global_publication_is_registered_even_without_object_return() {
        let (mut f, facts) = graph(1 << 1);
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Constant(Constant::Null)));
        f.blocks[0].instructions.push((
            ValueId(9),
            Instruction::StoreGlobal {
                symbol: "g".into(),
                val: Operand::Val(ValueId(0)),
                ty: Type::Ref,
                root: Some(0),
            },
            Type::Void,
        ));
        let a = analyze(&f, &facts, 3).unwrap();
        let r = compute(&f, &a).unwrap();
        // Store global ainda é opaco ao domínio de efeitos de heap: não
        // fabricar precisão transitiva antes de possuir esse contrato.
        assert!(
            r.publications[&EscapeCause::Global("g".into())]
                .nodes()
                .is_none()
        );
        assert!(
            r.publications[&EscapeCause::Return]
                .nodes()
                .unwrap()
                .is_empty()
        );
    }
}
