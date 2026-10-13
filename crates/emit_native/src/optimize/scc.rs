//! Componentes fortemente conexos de um grafo dirigido, sem recursão Rust.
//! O domínio dos nós pertence ao chamador: chamadas, sítios, tipos ou unidades.

/// Decompõe o grafo por Tarjan iterativo em O(V + E), com espaço O(V + E),
/// incluindo a conferência independente da partição antes de devolvê-la.
/// Cada nó aparece em exatamente um componente; componentes são emitidos
/// depois dos sucessores externos, permitindo resolver dependências primeiro.
/// Não interpreta uma SCC como prova de ciclos de instâncias ou de ownership.
///
/// # Panics
/// Uma aresta cujo destino não existe no vetor do grafo viola a precondição.
pub(crate) fn components(edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let n = edges.len();
    assert!(
        edges.iter().flatten().all(|&v| v < n),
        "aresta fora do grafo"
    );
    let mut components = Vec::new();
    let mut order = vec![usize::MAX; n];
    let mut low = vec![0usize; n];
    let mut on_stack = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut next_item = 0usize;
    for root in 0..n {
        if order[root] != usize::MAX {
            continue;
        }
        // (nó, próxima aresta a visitar)
        let mut calls: Vec<(usize, usize)> = vec![(root, 0)];
        order[root] = next_item;
        low[root] = next_item;
        next_item += 1;
        stack.push(root);
        on_stack[root] = true;
        while let Some(top) = calls.last_mut() {
            let v = top.0;
            if top.1 < edges[v].len() {
                let w = edges[v][top.1];
                top.1 += 1;
                if order[w] == usize::MAX {
                    order[w] = next_item;
                    low[w] = next_item;
                    next_item += 1;
                    stack.push(w);
                    on_stack[w] = true;
                    calls.push((w, 0));
                } else if on_stack[w] {
                    low[v] = low[v].min(order[w]);
                }
                continue;
            }
            calls.pop();
            if let Some(&(parent, _)) = calls.last() {
                low[parent] = low[parent].min(low[v]);
            }
            if low[v] == order[v] {
                let mut component = Vec::new();
                loop {
                    let w = stack.pop().expect("componente na pilha");
                    on_stack[w] = false;
                    component.push(w);
                    if w == v {
                        break;
                    }
                }
                components.push(component);
            }
        }
    }
    check(edges, &components).expect("partição SCC inválida");
    components
}

/// Confere uma partição sem executar Tarjan: cobertura única dos nós,
/// alcance interno nos dois sentidos e ordem estrita das dependências externas.
/// Essa ordem impede ciclos entre componentes, portanto também confere que
/// uma SCC não foi dividida. Não interpreta os nós nem certifica o produtor
/// das arestas. Tempo/espaço O(V + E), sem recursão Rust.
///
/// # Erros
/// Aresta/nó fora do grafo, componente vazio, nó omitido/duplicado, componente
/// sem conexão forte ou dependência externa fora da ordem declarada.
pub(crate) fn check(edges: &[Vec<usize>], cs: &[Vec<usize>]) -> Result<(), String> {
    let n = edges.len();
    let mut owners = vec![usize::MAX; n];
    for (id, c) in cs.iter().enumerate() {
        if c.is_empty() {
            return Err("SCC: componente vazio".into());
        }
        for &v in c {
            let owner = owners.get_mut(v).ok_or("SCC: nó fora do grafo")?;
            if *owner != usize::MAX {
                return Err("SCC: nó duplicado".into());
            }
            *owner = id;
        }
    }
    if owners.contains(&usize::MAX) {
        return Err("SCC: nó omitido".into());
    }
    let mut inverses = vec![Vec::new(); n];
    for (v, dests) in edges.iter().enumerate() {
        for &w in dests {
            if w >= n {
                return Err("SCC: aresta fora do grafo".into());
            }
            if owners[v] != owners[w] && owners[v] <= owners[w] {
                return Err("SCC: dependência fora da ordem".into());
            }
            inverses[w].push(v);
        }
    }
    for adjacent in [edges, inverses.as_slice()] {
        // Cada nó é visitado no máximo uma vez por direção; componentes
        // são disjuntos. Não zerar um vetor de V marcas para cada SCC.
        let mut seen_set = vec![false; n];
        for (id, c) in cs.iter().enumerate() {
            let mut queue = vec![c[0]];
            seen_set[c[0]] = true;
            let mut quantity = 0;
            while let Some(v) = queue.pop() {
                quantity += 1;
                for &w in &adjacent[v] {
                    if owners[w] == id && !seen_set[w] {
                        seen_set[w] = true;
                        queue.push(w);
                    }
                }
            }
            if quantity != c.len() {
                return Err("SCC: componente sem conexão forte".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_rejects_tampered_partitions_and_reversed_order() {
        let g = vec![vec![1], vec![0, 2], vec![]];
        assert!(check(&g, &[vec![2], vec![0, 1]]).is_ok());
        assert!(check(&g, &[vec![2], vec![1, 0]]).is_ok());
        for cs in [
            vec![vec![2], vec![0]],       // Omissão.
            vec![vec![2], vec![0, 1, 1]], // Duplicação.
            vec![vec![], vec![2], vec![0, 1]],
            vec![vec![2], vec![0, 1, 3]],    // Nó inexistente.
            vec![vec![0, 1], vec![2]],       // Ordem invertida.
            vec![vec![2], vec![0], vec![1]], // Ciclo dividido.
            vec![vec![0, 1, 2]],             // Fusão de SCCs diferentes.
        ] {
            assert!(check(&g, &cs).is_err(), "{cs:?}");
        }
        // Alcance somente no sentido direto não prova conexão forte.
        assert!(check(&[vec![1], vec![]], &[vec![0, 1]]).is_err());
        assert!(check(&[vec![3]], &[vec![0]]).is_err());
        assert!(check(&[], &[]).is_ok());
    }

    #[test]
    fn components_match_independent_reach_in_all_three_node_graphs() {
        for mask in 0..(1 << 9) {
            let graph: Vec<Vec<_>> = (0..3)
                .map(|v| {
                    (0..3)
                        .filter(|&w| mask & (1 << (3 * v + w)) != 0)
                        .collect()
                })
                .collect();
            let mut reachability = [[false; 3]; 3];
            for v in 0..3 {
                reachability[v][v] = true;
                for &w in &graph[v] {
                    reachability[v][w] = true;
                }
            }
            // Fecho transitivo de Floyd-Warshall, independente de Tarjan.
            for k in 0..3 {
                for v in 0..3 {
                    for w in 0..3 {
                        reachability[v][w] |= reachability[v][k] && reachability[k][w];
                    }
                }
            }
            // Todas as atribuições de três nós a até três componentes:
            // Floyd decide equivalência; as arestas decidem a ordem. Isso
            // testa também candidatos incorretos, sem usar a saída de Tarjan.
            for assignment in 0..27 {
                let mut code = assignment;
                let mut candidate = vec![vec![]; 3];
                let mut owners = [0; 3];
                for (v, owner) in owners.iter_mut().enumerate() {
                    *owner = code % 3;
                    code /= 3;
                    candidate[*owner].push(v);
                }
                while candidate.last().is_some_and(Vec::is_empty) {
                    candidate.pop();
                }
                let expected = candidate.iter().all(|c| !c.is_empty())
                    && (0..3).all(|v| {
                        (0..3).all(|w| (owners[v] == owners[w]) == (reachability[v][w] && reachability[w][v]))
                            && graph[v].iter().all(|&w| owners[v] >= owners[w])
                    });
                assert_eq!(
                    check(&graph, &candidate).is_ok(),
                    expected,
                    "grafo {mask}, atribuição {assignment}"
                );
            }
            let cs = components(&graph);
            let mut owners = [usize::MAX; 3];
            for (id, c) in cs.iter().enumerate() {
                assert!(!c.is_empty());
                for &v in c {
                    assert_eq!(owners[v], usize::MAX);
                    owners[v] = id;
                }
            }
            assert!(owners.iter().all(|&id| id != usize::MAX));
            for v in 0..3 {
                for w in 0..3 {
                    assert_eq!(
                        owners[v] == owners[w],
                        reachability[v][w] && reachability[w][v],
                        "grafo {mask}"
                    );
                }
            }
            for v in 0..3 {
                for &w in &graph[v] {
                    assert!(owners[v] >= owners[w], "ordem de dependências {mask}");
                }
            }
        }
    }

    #[test]
    fn deep_chain_and_cycle_do_not_use_rust_stack() {
        let n = 100_000;
        let mut graph: Vec<Vec<_>> = (0..n)
            .map(|v| if v + 1 < n { vec![v + 1] } else { vec![] })
            .collect();
        let cs = components(&graph);
        assert_eq!(cs.len(), n);
        assert_eq!(cs.first().unwrap(), &[n - 1]);
        assert_eq!(cs.last().unwrap(), &[0]);
        graph[n - 1].push(0);
        let cs = components(&graph);
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].len(), n);
    }

    #[test]
    fn empty_isolated_and_repeated_edges_preserve_partition() {
        assert!(components(&[]).is_empty());
        let cs = components(&[vec![1, 1], vec![0, 0], vec![], vec![3]]);
        assert_eq!(cs, vec![vec![1, 0], vec![2], vec![3]]);
    }

    #[test]
    #[should_panic(expected = "aresta fora do grafo")]
    fn nonexistent_destination_is_rejected() {
        components(&[vec![1]]);
    }
}
