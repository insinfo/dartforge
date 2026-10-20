use super::*;
use std::collections::BTreeSet;

fn reachable_without(successors: &[Vec<usize>], removed: Option<usize>) -> BTreeSet<usize> {
    let mut seen = BTreeSet::new();
    let mut pending = vec![0];
    while let Some(block) = pending.pop() {
        if removed != Some(block) && seen.insert(block) {
            pending.extend(&successors[block]);
        }
    }
    seen
}

fn predecessors(successors: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut result = vec![Vec::new(); successors.len()];
    for (block, edges) in successors.iter().enumerate() {
        for &target in edges {
            result[target].push(block);
        }
    }
    result
}

fn verify_by_removing_blocks(successors: &[Vec<usize>]) {
    let reachable = reachable_without(successors, None);
    let actual = Dominators::new(&predecessors(successors), &reverse_postorder(successors));
    // A reachable block is dominated by a definition exactly when removing
    // that definition makes the block unreachable from the entry.
    for definition in 0..successors.len() {
        let bypass = reachable_without(successors, Some(definition));
        for &block in &reachable {
            assert_eq!(
                actual.dominates(definition, block),
                !bypass.contains(&block),
                "definition {definition}, block {block}, graph {successors:?}"
            );
        }
    }
}

#[test]
fn all_four_block_graphs_match_independent_path_removal() {
    // Every directed graph without an incoming edge to the entry, including
    // self loops, irreducible cycles, disconnected nodes and multiple joins.
    for count in 1..=4 {
        let edges: Vec<_> = (0..count)
            .flat_map(|source| (1..count).map(move |target| (source, target)))
            .collect();
        for mask in 0..(1usize << edges.len()) {
            let mut successors = vec![Vec::new(); count];
            for (index, &(source, target)) in edges.iter().enumerate() {
                if mask & (1 << index) != 0 {
                    successors[source].push(target);
                }
            }
            verify_by_removing_blocks(&successors);
        }
    }
}

#[test]
fn word_boundaries_duplicate_edges_and_unreachable_parents_match_paths() {
    let mut successors = vec![Vec::new(); MAX_BLOCKS];
    // Reverse block order forces several fixed-point passes. All bitmap words
    // are exercised; the highest block points back to a lower-numbered loop.
    successors[0] = vec![MAX_BLOCKS - 1, MAX_BLOCKS - 1];
    for (block, edges) in successors.iter_mut().enumerate().skip(2) {
        edges.push(block - 1);
    }
    successors[1] = vec![63];
    verify_by_removing_blocks(&successors);
    // Unreachable predecessors must not erase valid dominance facts.
    successors[0] = vec![64];
    successors[64] = vec![1, 1];
    successors[1].clear();
    verify_by_removing_blocks(&successors);
}

#[test]
fn structural_witness_keeps_the_existing_block_budget() {
    use crate::hir::{BasicBlock, BlockId, Constant, Function, Operand, Terminator, Type};
    let mut function = Function {
        symbol: "budget".into(),
        name: "budget".into(),
        params: Vec::new(),
        return_ty: Type::Ref,
        blocks: (0..MAX_BLOCKS)
            .map(|index| BasicBlock {
                id: BlockId(index as u32),
                instructions: Vec::new(),
                terminator: if index + 1 == MAX_BLOCKS {
                    Terminator::Return(Some(Operand::Constant(Constant::Null)))
                } else {
                    Terminator::Branch(BlockId((index + 1) as u32))
                },
            })
            .collect(),
        debug_info: None,
    };
    assert!(super::super::Shape::new(&function).is_ok());
    function.blocks.push(BasicBlock {
        id: BlockId(MAX_BLOCKS as u32),
        instructions: Vec::new(),
        terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))),
    });
    assert_eq!(
        super::super::Shape::new(&function).err().as_deref(),
        Some("empty body or structural proof budget exceeded")
    );
}

fn legacy_sets(predecessors: &[Vec<usize>], reachable: &BTreeSet<usize>) -> Vec<BTreeSet<usize>> {
    let mut sets = vec![reachable.clone(); predecessors.len()];
    sets[0] = BTreeSet::from([0]);
    loop {
        let mut changed = false;
        for &block in reachable.iter().filter(|&&block| block != 0) {
            let mut next = reachable.clone();
            for &parent in &predecessors[block] {
                if reachable.contains(&parent) {
                    next = next.intersection(&sets[parent]).copied().collect();
                }
            }
            next.insert(block);
            if next != sets[block] {
                sets[block] = next;
                changed = true;
            }
        }
        if !changed {
            return sets;
        }
    }
}

#[test]
#[ignore = "controlled representation measurement; also checks every result against the previous implementation"]
fn dominance_representation_measurement() {
    let mut successors = vec![Vec::new(); 256];
    successors[0].push(255);
    for (block, edges) in successors.iter_mut().enumerate().skip(2) {
        edges.push(block - 1);
    }
    let parents = predecessors(&successors);
    let reachable = reachable_without(&successors, None);
    let start = std::time::Instant::now();
    let previous = std::hint::black_box(legacy_sets(&parents, &reachable));
    let previous_ns = start.elapsed().as_nanos();
    let start = std::time::Instant::now();
    let dense = std::hint::black_box(Dominators::new(&parents, &reverse_postorder(&successors)));
    let dense_ns = start.elapsed().as_nanos();
    for (block, definitions) in previous.iter().enumerate() {
        for definition in 0..successors.len() {
            assert_eq!(
                dense.dominates(definition, block),
                definitions.contains(&definition)
            );
        }
    }
    println!(
        "{}",
        serde_json::json!({"blocks":successors.len(),"previous_ns":previous_ns,"dense_ns":dense_ns,"every_dominance_pair_equal":true,"end_to_end_speedup_measured":false})
    );
}

#[test]
fn reverse_postorder_visits_reachable_blocks_once_without_recursive_frames() {
    let mut graph = vec![vec![]; MAX_BLOCKS];
    graph[0] = vec![MAX_BLOCKS - 1; 4096];
    for (block, edges) in graph.iter_mut().enumerate().skip(2) {
        edges.push(block - 1);
    }
    graph[1].push(63);
    let order = reverse_postorder(&graph);
    assert_eq!(
        order,
        std::iter::once(0)
            .chain((1..MAX_BLOCKS).rev())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        order.iter().copied().collect::<BTreeSet<_>>(),
        reachable_without(&graph, None)
    );
    let disconnected = vec![vec![2], vec![1], vec![]];
    assert_eq!(reverse_postorder(&disconnected), vec![0, 2]);
}

#[test]
fn shuffled_cyclic_graphs_preserve_the_independent_path_removal_oracle() {
    let mut random = 0x6b8b4567u64;
    for _ in 0..48 {
        let count = 32;
        let mut labels: Vec<_> = (0..count).collect();
        for i in (2..count).rev() {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            labels.swap(i, 1 + (random as usize % i));
        }
        let mut graph = vec![vec![]; count];
        for source in 0..count {
            for target in 1..count {
                random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                if random.is_multiple_of(11) {
                    graph[labels[source]].push(labels[target]);
                }
            }
        }
        verify_by_removing_blocks(&graph);
    }
}

#[test]
fn flow_order_reaches_the_same_fixed_point_with_fewer_sweeps() {
    let mut graph = vec![vec![]; MAX_BLOCKS];
    graph[0].push(MAX_BLOCKS - 1);
    for (block, edges) in graph.iter_mut().enumerate().skip(2) {
        edges.push(block - 1);
    }
    let parents = predecessors(&graph);
    let numeric: Vec<_> = reachable_without(&graph, None).into_iter().collect();
    let flow = reverse_postorder(&graph);
    let start = std::time::Instant::now();
    let previous = std::hint::black_box(Dominators::new(&parents, &numeric));
    let previous_ns = start.elapsed().as_nanos();
    let start = std::time::Instant::now();
    let updated = std::hint::black_box(Dominators::new(&parents, &flow));
    let updated_ns = start.elapsed().as_nanos();
    assert_eq!(previous.sets, updated.sets);
    // The deepest node already has its final all-block set at initialization.
    assert_eq!(previous.sweeps, MAX_BLOCKS - 1);
    assert_eq!(updated.sweeps, 2);
    println!(
        "{}",
        serde_json::json!({"blocks":MAX_BLOCKS,"numeric_sweeps":previous.sweeps,
        "flow_sweeps":updated.sweeps,"numeric_ns":previous_ns,"flow_ns":updated_ns,
        "same_sets":true,"order_construction_timed":false,"compiler_speedup_measured":false})
    );
}
