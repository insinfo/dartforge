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
    fn avaliado(&self, no: usize) -> bool;
    /// Um componente de um nó só, sem auto-referência: calcula o valor
    /// (`computeConstantValue`). Todas as dependências já estão avaliadas ou
    /// marcadas como ciclo.
    fn avaliar(&mut self, no: usize);
    /// Um componente com ciclo, do topo da pilha para baixo (o nó que fecha
    /// o componente é o último): cada variável recebe o erro de ciclo e cada
    /// construtor deixa de ser livre de ciclo (`generateCycleError`).
    fn avaliar_componente(&mut self, nos: &[usize]);
}

/// O estado do percurso que persiste entre as chamadas de
/// [`Caminhante::percorrer`].
#[derive(Debug, Default)]
pub struct Caminhante {
    /// `_index` de cada nó (0 = nunca visto).
    indice: Vec<u32>,
    /// `_lowLink` de cada nó.
    menor: Vec<u32>,
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

impl Caminhante {
    pub fn novo() -> Self {
        Self::default()
    }

    fn garantir(&mut self, no: usize) {
        if no >= self.indice.len() {
            self.indice.resize(no + 1, 0);
            self.menor.resize(no + 1, 0);
            self.dependencias.resize_with(no + 1, || None);
        }
    }

    /// `walk(start)`: avalia `inicio` e tudo de que ele depende, na ordem das
    /// dependências, entregando ao grafo cada componente fortemente conexo.
    pub fn percorrer<G: Grafo>(&mut self, g: &mut G, inicio: usize) {
        if g.avaliado(inicio) {
            return;
        }
        // O contador recomeça a cada percurso; os índices dos nós, não.
        let mut contador: u32 = 1;
        let mut pilha: Vec<usize> = Vec::new();
        let mut quadros: Vec<Quadro> = Vec::new();
        self.abrir(inicio, &mut contador, &mut pilha, &mut quadros);
        while let Some(topo) = quadros.len().checked_sub(1) {
            let no = quadros[topo].no;
            if self.dependencias[no].is_none() {
                let deps = g.dependencias(no);
                for &d in &deps {
                    self.garantir(d);
                }
                self.dependencias[no] = Some(deps);
            }
            let proxima = quadros[topo].proxima;
            let dep = self.dependencias[no].as_ref().and_then(|d| d.get(proxima).copied());
            match dep {
                Some(dep) => {
                    quadros[topo].proxima += 1;
                    if g.avaliado(dep) {
                        continue;
                    }
                    if dep == no {
                        quadros[topo].trivial = true;
                    } else if self.indice[dep] == 0 {
                        // Desce; o menor índice do filho sobe na volta
                        // (abaixo, quando o quadro dele fecha).
                        self.abrir(dep, &mut contador, &mut pilha, &mut quadros);
                    } else if self.indice[dep] < self.menor[no] {
                        // Já visto (neste percurso ou num anterior, sem
                        // ter sido avaliado): o índice dele, como está.
                        self.menor[no] = self.indice[dep];
                    }
                }
                None => {
                    // Todas as dependências vistas: fecha o nó.
                    let trivial = quadros[topo].trivial;
                    quadros.pop();
                    if self.menor[no] == self.indice[no] {
                        if pilha.last() == Some(&no) {
                            pilha.pop();
                            if trivial {
                                g.avaliar_componente(&[no]);
                            } else {
                                g.avaliar(no);
                            }
                        } else {
                            let mut componente = Vec::new();
                            while let Some(x) = pilha.pop() {
                                componente.push(x);
                                if x == no {
                                    break;
                                }
                            }
                            g.avaliar_componente(&componente);
                        }
                    }
                    // A volta de `strongConnect(dep)` no pai.
                    if let Some(pai) = quadros.last() {
                        let pai = pai.no;
                        if self.menor[no] < self.menor[pai] {
                            self.menor[pai] = self.menor[no];
                        }
                    }
                }
            }
        }
    }

    /// O começo de `strongConnect(no)`: numera o nó e o empilha.
    fn abrir(&mut self, no: usize, contador: &mut u32, pilha: &mut Vec<usize>, quadros: &mut Vec<Quadro>) {
        self.garantir(no);
        self.indice[no] = *contador;
        self.menor[no] = *contador;
        *contador += 1;
        pilha.push(no);
        quadros.push(Quadro { no, proxima: 0, trivial: false });
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Um grafo de teste: as arestas, quem é construtor (nunca fica avaliado
    /// depois de um ciclo, e fica depois de `avaliar`), e o que aconteceu.
    struct Teste {
        arestas: Vec<Vec<usize>>,
        avaliados: Vec<bool>,
        construtor: Vec<bool>,
        simples: Vec<usize>,
        ciclos: Vec<Vec<usize>>,
    }

    impl Teste {
        fn novo(arestas: Vec<Vec<usize>>, construtores: &[usize]) -> Self {
            let n = arestas.len();
            let mut construtor = vec![false; n];
            for &c in construtores {
                construtor[c] = true;
            }
            Teste { arestas, avaliados: vec![false; n], construtor, simples: Vec::new(), ciclos: Vec::new() }
        }

        fn rodar(&mut self, ordem: &[usize]) {
            let mut c = Caminhante::novo();
            for &no in ordem {
                c.percorrer(self, no);
            }
        }
    }

    impl Grafo for Teste {
        fn dependencias(&mut self, no: usize) -> Vec<usize> {
            self.arestas[no].clone()
        }
        fn avaliado(&self, no: usize) -> bool {
            self.avaliados[no]
        }
        fn avaliar(&mut self, no: usize) {
            self.avaliados[no] = true;
            self.simples.push(no);
        }
        fn avaliar_componente(&mut self, nos: &[usize]) {
            // A variável de um ciclo fica com o inválido guardado (avaliada);
            // o construtor de um ciclo nunca fica avaliado.
            for &n in nos {
                if !self.construtor[n] {
                    self.avaliados[n] = true;
                }
            }
            let mut v = nos.to_vec();
            v.sort_unstable();
            self.ciclos.push(v);
        }
    }

    #[test]
    fn cadeia_sem_ciclo_avalia_das_folhas_para_a_raiz() {
        // 0 → 1 → 2
        let mut g = Teste::novo(vec![vec![1], vec![2], vec![]], &[]);
        g.rodar(&[0, 1, 2]);
        assert_eq!(g.simples, vec![2, 1, 0]);
        assert!(g.ciclos.is_empty());
    }

    #[test]
    fn ciclo_de_tres_com_um_dependente_limpo() {
        // `c02`: a → b → c → a; d → a. Só as três do ciclo recebem o erro.
        let mut g = Teste::novo(vec![vec![1], vec![2], vec![0], vec![0]], &[]);
        g.rodar(&[0, 1, 2, 3]);
        assert_eq!(g.ciclos, vec![vec![0, 1, 2]]);
        assert_eq!(g.simples, vec![3]);
    }

    #[test]
    fn auto_referencia_e_ciclo_trivial() {
        // `const a = a;`
        let mut g = Teste::novo(vec![vec![0]], &[]);
        g.rodar(&[0]);
        assert_eq!(g.ciclos, vec![vec![0]]);
        assert!(g.simples.is_empty());
    }

    #[test]
    fn construtor_de_ciclo_visto_antes_da_o_falso_ciclo() {
        // `c27`: class C { const C() : this.a(); const C.a() : this(); }
        //        const y = z;  const z = const C();
        // Nós: 0 = C(), 1 = C.a(), 2 = y, 3 = z. Ordem dos alvos: a da fonte.
        let mut g = Teste::novo(vec![vec![1], vec![0], vec![3], vec![0]], &[0, 1]);
        g.rodar(&[0, 1, 2, 3]);
        // Os dois construtores formam o ciclo de verdade; `y` e `z` saem
        // como um segundo componente, que não existe no grafo.
        assert_eq!(g.ciclos, vec![vec![0, 1], vec![2, 3]]);
        // O percurso de `C.a`, sozinho, fecha um componente de um nó (o
        // índice velho de `C()` não é menor que o dele): é "computado".
        assert_eq!(g.simples, vec![1]);
    }

    #[test]
    fn a_mesma_coisa_com_a_classe_depois_nao_da_o_falso_ciclo() {
        // `s28`: const w2 = w;  const w = const R();  class R { os dois construtores }
        // Nós: 0 = w2, 1 = w, 2 = R(), 3 = R.a(). O percurso de `w2` é quem
        // visita `R` pela primeira vez.
        let mut g = Teste::novo(vec![vec![1], vec![2], vec![3], vec![2]], &[2, 3]);
        g.rodar(&[0, 1, 2, 3]);
        assert_eq!(g.ciclos, vec![vec![2, 3]]);
        // `w` e `w2` saem limpos; os percursos seguintes dos dois
        // construtores, cada um sozinho, os dão por computados.
        assert_eq!(g.simples, vec![1, 0, 2, 3]);
    }
}
