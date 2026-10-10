//! Componentes fortemente conexos de um grafo dirigido, sem recursão Rust.
//! O domínio dos nós pertence ao chamador: chamadas, sítios, tipos ou unidades.

/// Decompõe o grafo por Tarjan iterativo em O(V + E), com espaço O(V).
/// Cada nó aparece em exatamente um componente; componentes são emitidos
/// depois dos sucessores externos, permitindo resolver dependências primeiro.
/// Não interpreta uma SCC como prova de ciclos de instâncias ou de ownership.
///
/// # Panics
/// Uma aresta cujo destino não existe no vetor do grafo viola a precondição.
pub(crate) fn componentes(arestas: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let n = arestas.len();
    assert!(
        arestas.iter().flatten().all(|&v| v < n),
        "aresta fora do grafo"
    );
    let mut componentes = Vec::new();
    let mut ordem = vec![usize::MAX; n];
    let mut baixo = vec![0usize; n];
    let mut na_pilha = vec![false; n];
    let mut pilha: Vec<usize> = Vec::new();
    let mut proximo = 0usize;
    for raiz in 0..n {
        if ordem[raiz] != usize::MAX {
            continue;
        }
        // (nó, próxima aresta a visitar)
        let mut chamadas: Vec<(usize, usize)> = vec![(raiz, 0)];
        ordem[raiz] = proximo;
        baixo[raiz] = proximo;
        proximo += 1;
        pilha.push(raiz);
        na_pilha[raiz] = true;
        while let Some(topo) = chamadas.last_mut() {
            let v = topo.0;
            if topo.1 < arestas[v].len() {
                let w = arestas[v][topo.1];
                topo.1 += 1;
                if ordem[w] == usize::MAX {
                    ordem[w] = proximo;
                    baixo[w] = proximo;
                    proximo += 1;
                    pilha.push(w);
                    na_pilha[w] = true;
                    chamadas.push((w, 0));
                } else if na_pilha[w] {
                    baixo[v] = baixo[v].min(ordem[w]);
                }
                continue;
            }
            chamadas.pop();
            if let Some(&(pai, _)) = chamadas.last() {
                baixo[pai] = baixo[pai].min(baixo[v]);
            }
            if baixo[v] == ordem[v] {
                let mut componente = Vec::new();
                loop {
                    let w = pilha.pop().expect("componente na pilha");
                    na_pilha[w] = false;
                    componente.push(w);
                    if w == v {
                        break;
                    }
                }
                componentes.push(componente);
            }
        }
    }
    componentes
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn componentes_conferem_com_alcance_independente_em_todos_os_grafos_de_tres_nos() {
        for mascara in 0..(1 << 9) {
            let grafo: Vec<Vec<_>> = (0..3)
                .map(|v| {
                    (0..3)
                        .filter(|&w| mascara & (1 << (3 * v + w)) != 0)
                        .collect()
                })
                .collect();
            let mut alcance = [[false; 3]; 3];
            for v in 0..3 {
                alcance[v][v] = true;
                for &w in &grafo[v] {
                    alcance[v][w] = true;
                }
            }
            // Fecho transitivo de Floyd-Warshall, independente de Tarjan.
            for k in 0..3 {
                for v in 0..3 {
                    for w in 0..3 {
                        alcance[v][w] |= alcance[v][k] && alcance[k][w];
                    }
                }
            }
            let cs = componentes(&grafo);
            let mut donos = [usize::MAX; 3];
            for (id, c) in cs.iter().enumerate() {
                assert!(!c.is_empty());
                for &v in c {
                    assert_eq!(donos[v], usize::MAX);
                    donos[v] = id;
                }
            }
            assert!(donos.iter().all(|&id| id != usize::MAX));
            for v in 0..3 {
                for w in 0..3 {
                    assert_eq!(
                        donos[v] == donos[w],
                        alcance[v][w] && alcance[w][v],
                        "grafo {mascara}"
                    );
                }
            }
            for v in 0..3 {
                for &w in &grafo[v] {
                    assert!(donos[v] >= donos[w], "ordem de dependências {mascara}");
                }
            }
        }
    }

    #[test]
    fn cadeia_e_ciclo_profundo_nao_usam_a_pilha_rust() {
        let n = 100_000;
        let mut grafo: Vec<Vec<_>> = (0..n)
            .map(|v| if v + 1 < n { vec![v + 1] } else { vec![] })
            .collect();
        let cs = componentes(&grafo);
        assert_eq!(cs.len(), n);
        assert_eq!(cs.first().unwrap(), &[n - 1]);
        assert_eq!(cs.last().unwrap(), &[0]);
        grafo[n - 1].push(0);
        let cs = componentes(&grafo);
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].len(), n);
    }

    #[test]
    fn vazio_isolados_e_arestas_repetidas_preservam_a_particao() {
        assert!(componentes(&[]).is_empty());
        let cs = componentes(&[vec![1, 1], vec![0, 0], vec![], vec![3]]);
        assert_eq!(cs, vec![vec![1, 0], vec![2], vec![3]]);
    }

    #[test]
    #[should_panic(expected = "aresta fora do grafo")]
    fn destino_inexistente_e_recusado() {
        componentes(&[vec![1]]);
    }
}
