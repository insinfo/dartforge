//! O caminhante do grafo de dependências de constantes: o porte literal do
//! `DependencyWalker` do analyzer (`dependency_walker.dart:7-152`, usado por
//! `computeConstants`, `compute.dart:31-96`). É o algoritmo de Tarjan com duas
//! particularidades que a paridade exige reproduzir
//! (docs/ANALYZER-ESPECIFICACAO.md §D, D.R4-1 §1.4, e §G, T8):
//!
//! * o índice e o menor índice alcançável de cada nó **persistem** entre os
//!   percursos, e o contador de índices **recomeça em 1** a cada percurso;
//! * um nó só sai de consideração quando o grafo o dá por avaliado
//!   ([`Grafo::avaliado`]). O construtor de um ciclo nunca fica avaliado: num
//!   percurso posterior ele é "já visto" com o índice velho, e um dependente
//!   pode ser desempilhado junto com o ancestral como um componente de vários
//!   nós — o falso `recursive_compile_time_constant` que depende da ordem
//!   das declarações (o caso `c27` da especificação).
//!
//! Cada componente fortemente conexo é entregue uma vez: um nó sozinho e sem
//! aresta para si vai a [`Grafo::avaliar`]; qualquer outro componente (vários
//! nós, ou um nó que depende de si) vai a [`Grafo::avaliar_componente`], que
//! marca o ciclo.

/// O grafo que o caminhante percorre. Os nós são índices densos.
pub trait Grafo {
    /// As dependências diretas de `no`, na ordem em que o analyzer as relata
    /// (`computeDependencies`). Chamado no máximo uma vez por nó.
    fn dependencias(&mut self, no: usize) -> Vec<usize>;
    /// O nó já tem resultado (`isConstantEvaluated`): variável com valor
    /// guardado, construtor `const` fora de ciclo já computado, anotação com
    /// valor; variável não `const` e parâmetro sem padrão, sempre.
    fn evaluated(&self, no: usize) -> bool;
    /// Um componente de um nó só, sem auto-referência: calcula o valor
    /// (`computeConstantValue`). Todas as dependências já estão avaliadas ou
    /// marcadas como ciclo.
    fn avaliar(&mut self, no: usize);
    /// Um componente com ciclo, do topo da pilha para baixo (o nó que fecha
    /// o componente é o último): cada variável recebe o erro de ciclo e cada
    /// construtor deixa de ser livre de ciclo (`generateCycleError`).
    fn evaluate_component(&mut self, nodes: &[usize]);
}

/// O estado do percurso que persiste entre as chamadas de
/// [`Caminhante::percorrer`].
#[derive(Debug, Default)]
pub struct Walker {
    /// `_index` de cada nó (0 = nunca visto).
    indice: Vec<u32>,
    /// `_lowLink` de cada nó.
    less: Vec<u32>,
    /// As dependências de cada nó, pedidas ao grafo uma vez.
    dependencias: Vec<Option<Vec<usize>>>,
}

/// Um quadro de `strongConnect` (a recursão do original, explícita: uma
/// cadeia de constantes pode ser mais funda que a pilha da thread).
struct Quadro {
    no: usize,
    /// A próxima dependência a examinar.
    proxima: usize,
    /// O nó depende de si mesmo (`hasTrivialCycle`).
    trivial: bool,
}

impl Walker {
    pub fn novo() -> Self {
        Self::default()
    }

    fn ensure(&mut self, no: usize) {
        if no >= self.indice.len() {
            self.indice.resize(no + 1, 0);
            self.less.resize(no + 1, 0);
            self.dependencias.resize_with(no + 1, || None);
        }
    }

    /// `walk(start)`: avalia `inicio` e tudo de que ele depende, na ordem das
    /// dependências, entregando ao grafo cada componente fortemente conexo.
    pub fn percorrer<G: Grafo>(&mut self, g: &mut G, start: usize) {
        if g.evaluated(start) {
            return;
        }
        // O contador recomeça a cada percurso; os índices dos nós, não.
        let mut counter: u32 = 1;
        let mut stack: Vec<usize> = Vec::new();
        let mut frames: Vec<Quadro> = Vec::new();
        self.abrir(start, &mut counter, &mut stack, &mut frames);
        while let Some(top) = frames.len().checked_sub(1) {
            let no = frames[top].no;
            if self.dependencias[no].is_none() {
                let deps = g.dependencias(no);
                for &d in &deps {
                    self.ensure(d);
                }
                self.dependencias[no] = Some(deps);
            }
            let next_ = frames[top].proxima;
            let dep = self.dependencias[no].as_ref().and_then(|d| d.get(next_).copied());
            match dep {
                Some(dep) => {
                    frames[top].proxima += 1;
                    if g.evaluated(dep) {
                        continue;
                    }
                    if dep == no {
                        frames[top].trivial = true;
                    } else if self.indice[dep] == 0 {
                        // Desce; o menor índice do filho sobe na volta
                        // (abaixo, quando o quadro dele fecha).
                        self.abrir(dep, &mut counter, &mut stack, &mut frames);
                    } else if self.indice[dep] < self.less[no] {
                        // Já visto (neste percurso ou num anterior, sem
                        // ter sido avaliado): o índice dele, como está.
                        self.less[no] = self.indice[dep];
                    }
                }
                None => {
                    // Todas as dependências vistas: fecha o nó.
                    let trivial = frames[top].trivial;
                    frames.pop();
                    if self.less[no] == self.indice[no] {
                        if stack.last() == Some(&no) {
                            stack.pop();
                            if trivial {
                                g.evaluate_component(&[no]);
                            } else {
                                g.avaliar(no);
                            }
                        } else {
                            let mut component = Vec::new();
                            while let Some(x) = stack.pop() {
                                component.push(x);
                                if x == no {
                                    break;
                                }
                            }
                            g.evaluate_component(&component);
                        }
                    }
                    // A volta de `strongConnect(dep)` no pai.
                    if let Some(parent) = frames.last() {
                        let parent = parent.no;
                        if self.less[no] < self.less[parent] {
                            self.less[parent] = self.less[no];
                        }
                    }
                }
            }
        }
    }

    /// O começo de `strongConnect(no)`: numera o nó e o empilha.
    fn abrir(&mut self, no: usize, counter: &mut u32, stack: &mut Vec<usize>, frames: &mut Vec<Quadro>) {
        self.ensure(no);
        self.indice[no] = *counter;
        self.less[no] = *counter;
        *counter += 1;
        stack.push(no);
        frames.push(Quadro { no, proxima: 0, trivial: false });
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Um grafo de teste: as arestas, quem é construtor (nunca fica avaliado
    /// depois de um ciclo, e fica depois de `avaliar`), e o que aconteceu.
    struct Test {
        arestas: Vec<Vec<usize>>,
        avaliados: Vec<bool>,
        construtor: Vec<bool>,
        simples: Vec<usize>,
        cycles: Vec<Vec<usize>>,
    }

    impl Test {
        fn novo(edges: Vec<Vec<usize>>, constructors: &[usize]) -> Self {
            let n = edges.len();
            let mut constructor = vec![false; n];
            for &c in constructors {
                constructor[c] = true;
            }
            Test { arestas: edges, avaliados: vec![false; n], construtor: constructor, simples: Vec::new(), cycles: Vec::new() }
        }

        fn run(&mut self, order: &[usize]) {
            let mut c = Walker::novo();
            for &no in order {
                c.percorrer(self, no);
            }
        }
    }

    impl Grafo for Test {
        fn dependencias(&mut self, no: usize) -> Vec<usize> {
            self.arestas[no].clone()
        }
        fn evaluated(&self, no: usize) -> bool {
            self.avaliados[no]
        }
        fn avaliar(&mut self, no: usize) {
            self.avaliados[no] = true;
            self.simples.push(no);
        }
        fn evaluate_component(&mut self, nodes: &[usize]) {
            // A variável de um ciclo fica com o inválido guardado (avaliada);
            // o construtor de um ciclo nunca fica avaliado.
            for &n in nodes {
                if !self.construtor[n] {
                    self.avaliados[n] = true;
                }
            }
            let mut v = nodes.to_vec();
            v.sort_unstable();
            self.cycles.push(v);
        }
    }

    #[test]
    fn acyclic_chain_evaluates_from_leaves_to_root() {
        // 0 → 1 → 2
        let mut g = Test::novo(vec![vec![1], vec![2], vec![]], &[]);
        g.run(&[0, 1, 2]);
        assert_eq!(g.simples, vec![2, 1, 0]);
        assert!(g.cycles.is_empty());
    }

    #[test]
    fn three_cycle_with_clean_dependent() {
        // `c02`: a → b → c → a; d → a. Só as três do ciclo recebem o erro.
        let mut g = Test::novo(vec![vec![1], vec![2], vec![0], vec![0]], &[]);
        g.run(&[0, 1, 2, 3]);
        assert_eq!(g.cycles, vec![vec![0, 1, 2]]);
        assert_eq!(g.simples, vec![3]);
    }

    #[test]
    fn self_reference_and_trivial_cycle() {
        // `const a = a;`
        let mut g = Test::novo(vec![vec![0]], &[]);
        g.run(&[0]);
        assert_eq!(g.cycles, vec![vec![0]]);
        assert!(g.simples.is_empty());
    }

    #[test]
    fn cycle_constructor_seen_before_gives_false_cycle() {
        // `c27`: class C { const C() : this.a(); const C.a() : this(); }
        //        const y = z;  const z = const C();
        // Nós: 0 = C(), 1 = C.a(), 2 = y, 3 = z. Ordem dos alvos: a da fonte.
        let mut g = Test::novo(vec![vec![1], vec![0], vec![3], vec![0]], &[0, 1]);
        g.run(&[0, 1, 2, 3]);
        // Os dois construtores formam o ciclo de verdade; `y` e `z` saem
        // como um segundo componente, que não existe no grafo.
        assert_eq!(g.cycles, vec![vec![0, 1], vec![2, 3]]);
        // O percurso de `C.a`, sozinho, fecha um componente de um nó (o
        // índice velho de `C()` não é menor que o dele): é "computado".
        assert_eq!(g.simples, vec![1]);
    }

    #[test]
    fn same_thing_with_class_after_gives_no_false_cycle() {
        // `s28`: const w2 = w;  const w = const R();  class R { os dois construtores }
        // Nós: 0 = w2, 1 = w, 2 = R(), 3 = R.a(). O percurso de `w2` é quem
        // visita `R` pela primeira vez.
        let mut g = Test::novo(vec![vec![1], vec![2], vec![3], vec![2]], &[2, 3]);
        g.run(&[0, 1, 2, 3]);
        assert_eq!(g.cycles, vec![vec![2, 3]]);
        // `w` e `w2` saem limpos; os percursos seguintes dos dois
        // construtores, cada um sozinho, os dão por computados.
        assert_eq!(g.simples, vec![1, 0, 2, 3]);
    }
}
