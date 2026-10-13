//! The walker of the constant dependency graph: a literal port of the
//! analyzer's `DependencyWalker` (`dependency_walker.dart:7-152`, used by
//! `computeConstants`, `compute.dart:31-96`). It is Tarjan's algorithm with
//! two particularities that parity requires reproducing
//! (docs/ANALYZER-ESPECIFICACAO.md §D, D.R4-1 §1.4, and §G, T8):
//!
//! * the index and the lowest reachable index of each node **persist** across
//!   walks, and the index counter **restarts at 1** on each walk;
//! * a node only leaves consideration when the graph considers it evaluated
//!   ([`Grafo::evaluated`]). A constructor in a cycle never becomes evaluated:
//!   in a later walk it is "already seen" with its old index, and a
//!   dependent may be popped together with the ancestor as a multi-node
//!   component, which is the false `recursive_compile_time_constant` that
//!   depends on declaration order (case `c27` of the specification).
//!
//! Each strongly connected component is delivered once: a single node without
//! an edge to itself goes to [`Grafo::avaliar`]; any other component
//! (several nodes, or a node that depends on itself) goes to
//! [`Grafo::evaluate_component`], which marks the cycle.

/// The graph the walker traverses. Nodes are dense indices.
pub trait Graph {
    /// The direct dependencies of `no`, in the order the analyzer reports them
    /// (`computeDependencies`). Called at most once per node.
    fn dependencies(&mut self, no: usize) -> Vec<usize>;
    /// The node already has a result (`isConstantEvaluated`): a variable with a
    /// stored value, an already computed `const` constructor outside a cycle, an
    /// annotation with a value; a non-`const` variable and a parameter without a
    /// default, always.
    fn evaluated(&self, no: usize) -> bool;
    /// A single-node component without self-reference: computes the value
    /// (`computeConstantValue`). All dependencies are already evaluated or
    /// marked as a cycle.
    fn evaluate(&mut self, no: usize);
    /// A component with a cycle, from the top of the stack down (the node that
    /// closes the component is the last one): each variable gets the cycle error
    /// and each constructor stops being cycle-free (`generateCycleError`).
    fn evaluate_component(&mut self, nodes: &[usize]);
}

/// The walk state that persists across calls to
/// [`Walker::percorrer`].
#[derive(Debug, Default)]
pub struct Walker {
    /// `_index` of each node (0 = never seen).
    index: Vec<u32>,
    /// `_lowLink` of each node.
    less: Vec<u32>,
    /// The dependencies of each node, requested from the graph once.
    dependencies: Vec<Option<Vec<usize>>>,
}

/// A `strongConnect` frame (the original's recursion, made explicit: a chain
/// of constants may be deeper than the thread's stack).
struct Frame {
    no: usize,
    /// The next dependency to examine.
    next: usize,
    /// The node depends on itself (`hasTrivialCycle`).
    trivial: bool,
}

impl Walker {
    pub fn new() -> Self {
        Self::default()
    }

    fn ensure(&mut self, no: usize) {
        if no >= self.index.len() {
            self.index.resize(no + 1, 0);
            self.less.resize(no + 1, 0);
            self.dependencies.resize_with(no + 1, || None);
        }
    }

    /// `walk(start)`: evaluates `start` and everything it depends on, in
    /// dependency order, handing each strongly connected component to the graph.
    pub fn traverse<G: Graph>(&mut self, g: &mut G, start: usize) {
        if g.evaluated(start) {
            return;
        }
        // The counter restarts on each walk; the node indices do not.
        let mut counter: u32 = 1;
        let mut stack: Vec<usize> = Vec::new();
        let mut frames: Vec<Frame> = Vec::new();
        self.open(start, &mut counter, &mut stack, &mut frames);
        while let Some(top) = frames.len().checked_sub(1) {
            let no = frames[top].no;
            if self.dependencies[no].is_none() {
                let deps = g.dependencies(no);
                for &d in &deps {
                    self.ensure(d);
                }
                self.dependencies[no] = Some(deps);
            }
            let next_ = frames[top].next;
            let dep = self.dependencies[no].as_ref().and_then(|d| d.get(next_).copied());
            match dep {
                Some(dep) => {
                    frames[top].next += 1;
                    if g.evaluated(dep) {
                        continue;
                    }
                    if dep == no {
                        frames[top].trivial = true;
                    } else if self.index[dep] == 0 {
                        // Goes down; the child's lowest index comes back up on return
                        // (below, when its frame closes).
                        self.open(dep, &mut counter, &mut stack, &mut frames);
                    } else if self.index[dep] < self.less[no] {
                        // Already seen (in this walk or an earlier one, without having
                        // been evaluated): its index, as it is.
                        self.less[no] = self.index[dep];
                    }
                }
                None => {
                    // All dependencies seen: closes the node.
                    let trivial = frames[top].trivial;
                    frames.pop();
                    if self.less[no] == self.index[no] {
                        if stack.last() == Some(&no) {
                            stack.pop();
                            if trivial {
                                g.evaluate_component(&[no]);
                            } else {
                                g.evaluate(no);
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
                    // The return of `strongConnect(dep)` in the parent.
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

    /// The start of `strongConnect(no)`: numbers the node and pushes it.
    fn open(&mut self, no: usize, counter: &mut u32, stack: &mut Vec<usize>, frames: &mut Vec<Frame>) {
        self.ensure(no);
        self.index[no] = *counter;
        self.less[no] = *counter;
        *counter += 1;
        stack.push(no);
        frames.push(Frame { no, next: 0, trivial: false });
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// A test graph: the edges, which nodes are constructors (never evaluated
    /// after a cycle, and evaluated after `avaliar`), and what happened.
    struct Test {
        edges: Vec<Vec<usize>>,
        evaluated: Vec<bool>,
        constructor: Vec<bool>,
        simple: Vec<usize>,
        cycles: Vec<Vec<usize>>,
    }

    impl Test {
        fn novo(edges: Vec<Vec<usize>>, constructors: &[usize]) -> Self {
            let n = edges.len();
            let mut constructor = vec![false; n];
            for &c in constructors {
                constructor[c] = true;
            }
            Test { edges, evaluated: vec![false; n], constructor, simple: Vec::new(), cycles: Vec::new() }
        }

        fn run(&mut self, order: &[usize]) {
            let mut c = Walker::new();
            for &no in order {
                c.traverse(self, no);
            }
        }
    }

    impl Graph for Test {
        fn dependencies(&mut self, no: usize) -> Vec<usize> {
            self.edges[no].clone()
        }
        fn evaluated(&self, no: usize) -> bool {
            self.evaluated[no]
        }
        fn evaluate(&mut self, no: usize) {
            self.evaluated[no] = true;
            self.simple.push(no);
        }
        fn evaluate_component(&mut self, nodes: &[usize]) {
            // A variable in a cycle keeps the stored invalid value (evaluated);
            // a constructor in a cycle never becomes evaluated.
            for &n in nodes {
                if !self.constructor[n] {
                    self.evaluated[n] = true;
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
        assert_eq!(g.simple, vec![2, 1, 0]);
        assert!(g.cycles.is_empty());
    }

    #[test]
    fn three_cycle_with_clean_dependent() {
        // `c02`: a → b → c → a; d → a. Only the three in the cycle get the error.
        let mut g = Test::novo(vec![vec![1], vec![2], vec![0], vec![0]], &[]);
        g.run(&[0, 1, 2, 3]);
        assert_eq!(g.cycles, vec![vec![0, 1, 2]]);
        assert_eq!(g.simple, vec![3]);
    }

    #[test]
    fn self_reference_and_trivial_cycle() {
        // `const a = a;`
        let mut g = Test::novo(vec![vec![0]], &[]);
        g.run(&[0]);
        assert_eq!(g.cycles, vec![vec![0]]);
        assert!(g.simple.is_empty());
    }

    #[test]
    fn cycle_constructor_seen_before_gives_false_cycle() {
        // `c27`: class C { const C() : this.a(); const C.a() : this(); }
        //        const y = z;  const z = const C();
        // Nodes: 0 = C(), 1 = C.a(), 2 = y, 3 = z. Target order: source order.
        let mut g = Test::novo(vec![vec![1], vec![0], vec![3], vec![0]], &[0, 1]);
        g.run(&[0, 1, 2, 3]);
        // The two constructors form the real cycle; `y` and `z` come out as
        // a second component, which does not exist in the graph.
        assert_eq!(g.cycles, vec![vec![0, 1], vec![2, 3]]);
        // The walk of `C.a`, alone, closes a single-node component (the old
        // index of `C()` is not lower than its own): it is "computed".
        assert_eq!(g.simple, vec![1]);
    }

    #[test]
    fn same_thing_with_class_after_gives_no_false_cycle() {
        // `s28`: const w2 = w;  const w = const R();  class R { both constructors }
        // Nodes: 0 = w2, 1 = w, 2 = R(), 3 = R.a(). The walk of `w2` is the one
        // that visits `R` for the first time.
        let mut g = Test::novo(vec![vec![1], vec![2], vec![3], vec![2]], &[2, 3]);
        g.run(&[0, 1, 2, 3]);
        assert_eq!(g.cycles, vec![vec![2, 3]]);
        // `w` and `w2` come out clean; the following walks of the two
        // constructors, each alone, consider them computed.
        assert_eq!(g.simple, vec![1, 0, 2, 3]);
    }
}
