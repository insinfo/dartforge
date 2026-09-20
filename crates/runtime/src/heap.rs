//! Heap preciso com tracing iterativo, raízes explícitas e contadores observáveis.

/// Contadores cumulativos de trabalho; não representam bytes físicos do alocador.
#[derive(Debug, Clone, Copy, Default)]
pub struct HeapStats {
    pub allocations: u64,
    pub collections: u64,
    pub reclaimed: u64,
    pub roots_scanned: u64,
    pub slots_scanned: u64,
    pub live_objects: usize,
    pub reserved_slots: usize,
}

/// Valor gerenciado; somente campos marcados como referência participam do tracing.
#[derive(Debug)]
pub enum Value {
    String(String),
    Object {
        class_id: i64,
        fields: Vec<(i64, bool)>,
    },
}

/// Heap preciso sem compactação; handles positivos indexam slots reutilizáveis.
#[derive(Debug)]
pub struct Heap {
    slots: Vec<Option<Value>>,
    free: Vec<usize>,
    frames: Vec<(i64, Vec<i64>)>,
    next_frame: i64,
    allocations: usize,
    threshold: usize,
    stress: bool,
    stats: HeapStats,
    marks: Vec<bool>,
    pending: Vec<i64>,
}
impl Heap {
    /// Inicializa heap; stress força coleta antes de cada alocação.
    pub fn new(stress: bool) -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            frames: Vec::new(),
            next_frame: 1,
            allocations: 0,
            threshold: 256,
            stress,
            stats: HeapStats::default(),
            marks: Vec::new(),
            pending: Vec::new(),
        }
    }
    /// Abre frame de raízes com identificador monotônico.
    pub fn push_frame(&mut self) -> i64 {
        let id = self.next_frame;
        self.next_frame = id.checked_add(1).expect("frames esgotados");
        self.frames.push((id, Vec::new()));
        id
    }
    /// Protege handle até o retorno da função; null não ocupa uma raiz.
    pub fn root(&mut self, frame: i64, handle: i64) {
        if handle == 0 {
            return;
        }
        self.get(handle);
        self.frames
            .iter_mut()
            .rev()
            .find(|(id, _)| *id == frame)
            .expect("frame inexistente")
            .1
            .push(handle);
    }
    /// Fecha exatamente o frame do topo, sem coletar entre retorno e raiz do chamador.
    pub fn pop_frame(&mut self, frame: i64) {
        assert_eq!(self.frames.last().map(|(id, _)| *id), Some(frame));
        self.frames.pop();
    }
    /// Aloca após coleta; o chamador deve proteger o resultado antes de outra alocação.
    pub fn allocate(&mut self, value: Value) -> i64 {
        if self.stress || self.allocations >= self.threshold {
            self.collect();
        }
        self.allocations += 1;
        self.stats.allocations += 1;
        let index = if let Some(index) = self.free.pop() {
            self.slots[index] = Some(value);
            index
        } else {
            self.slots.push(Some(value));
            self.slots.len() - 1
        };
        i64::try_from(index + 1).expect("handles esgotados")
    }
    /// Obtém valor vivo; o protocolo ABI não permite handles obsoletos.
    pub fn get(&self, handle: i64) -> &Value {
        let index = usize::try_from(handle.checked_sub(1).expect("handle inválido"))
            .expect("handle inválido");
        self.slots
            .get(index)
            .and_then(Option::as_ref)
            .expect("handle não vivo")
    }
    /// Atualiza campo com tag explícita; valores escalares jamais são raízes.
    pub fn set(&mut self, handle: i64, index: i64, bits: i64, is_ref: bool) {
        if is_ref && bits != 0 {
            self.get(bits);
        }
        let slot = usize::try_from(handle - 1).expect("handle inválido");
        let Value::Object { fields, .. } = self
            .slots
            .get_mut(slot)
            .and_then(Option::as_mut)
            .expect("handle não vivo")
        else {
            panic!("objeto esperado")
        };
        fields[usize::try_from(index).expect("índice inválido")] = (bits, is_ref);
    }
    /// Marca raízes e arestas tipadas iterativamente e libera inclusive ciclos inalcançáveis.
    pub fn collect(&mut self) {
        self.stats.collections += 1;
        self.stats.roots_scanned += self
            .frames
            .iter()
            .map(|(_, roots)| roots.len() as u64)
            .sum::<u64>();
        self.stats.slots_scanned += self.slots.len() as u64;
        self.marks.resize(self.slots.len(), false);
        self.marks.fill(false);
        self.pending.clear();
        self.pending.extend(
            self.frames
                .iter()
                .flat_map(|(_, roots)| roots.iter().copied()),
        );
        let mut live = 0_usize;
        while let Some(handle) = self.pending.pop() {
            if handle == 0 {
                continue;
            }
            let index = usize::try_from(handle - 1).expect("handle inválido");
            if self.marks[index] {
                continue;
            }
            self.marks[index] = true;
            live += 1;
            if let Value::Object { fields, .. } =
                self.slots[index].as_ref().expect("handle não vivo")
            {
                self.pending.extend(
                    fields
                        .iter()
                        .filter_map(|(bits, is_ref)| is_ref.then_some(*bits)),
                );
            }
        }
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if slot.is_some() && !self.marks[index] {
                *slot = None;
                self.free.push(index);
                self.stats.reclaimed += 1;
            }
        }
        self.allocations = 0;
        self.threshold = live.saturating_mul(2).max(256);
    }

    /// Obtém contadores sem percorrer os objetos ou suas raízes.
    pub fn stats(&self) -> HeapStats {
        HeapStats {
            live_objects: self.slots.len() - self.free.len(),
            reserved_slots: self.slots.len(),
            ..self.stats
        }
    }

    /// Compara conteúdo, preservando NUL e diferenças entre sequências Unicode.
    pub fn string_equal(&self, a: i64, b: i64) -> bool {
        if a == 0 || b == 0 {
            return a == b;
        }
        let Value::String(left) = self.get(a) else {
            panic!("string esperada")
        };
        let Value::String(right) = self.get(b) else {
            panic!("string esperada")
        };
        left == right
    }

    /// Concatena conteúdo antes da possível coleta; os operandos seguem o protocolo de raízes.
    pub fn string_concat(&mut self, a: i64, b: i64) -> i64 {
        let Value::String(left) = self.get(a) else {
            panic!("string esperada")
        };
        let Value::String(right) = self.get(b) else {
            panic!("string esperada")
        };
        let result = format!("{left}{right}");
        self.allocate(Value::String(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Um único objeto raiz mantém transitivamente ciclos e strings de seus campos.
    #[test]
    fn tagged_edges_keep_unrooted_children_alive() {
        let mut heap = Heap::new(true);
        let outer = heap.push_frame();
        let parent = heap.allocate(Value::Object {
            class_id: 1,
            fields: vec![(0, true)],
        });
        heap.root(outer, parent);
        let inner = heap.push_frame();
        let text = heap.allocate(Value::String("ação 🦀".into()));
        heap.root(inner, text);
        heap.set(parent, 0, text, true);
        heap.pop_frame(inner);
        heap.collect();
        assert!(matches!(heap.get(text), Value::String(s) if s == "ação 🦀"));
        heap.set(parent, 0, 0, true);
        heap.collect();
        assert!(heap.slots[(text - 1) as usize].is_none());
        heap.pop_frame(outer);
        heap.collect();
        assert_eq!(heap.slots.iter().flatten().count(), 0);
    }
    /// Ciclos sobrevivem por raiz, depois são coletados e seus slots reutilizados.
    #[test]
    fn cycles_and_slot_reuse() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame();
        let a = heap.allocate(Value::Object {
            class_id: 1,
            fields: vec![(0, true)],
        });
        heap.root(frame, a);
        let b = heap.allocate(Value::Object {
            class_id: 2,
            fields: vec![(a, true)],
        });
        heap.root(frame, b);
        heap.set(a, 0, b, true);
        heap.collect();
        assert_eq!(heap.slots.iter().flatten().count(), 2);
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(heap.slots.iter().flatten().count(), 0);
        let c = heap.allocate(Value::String("novo".into()));
        assert!(c == a || c == b);
        assert_eq!(heap.slots.len(), 2);
    }
    /// Bits escalares coincidentes com handles não retêm objetos.
    #[test]
    fn scalar_bits_do_not_trace() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame();
        let text = heap.allocate(Value::String("descartável".into()));
        let object = heap.allocate(Value::Object {
            class_id: 1,
            fields: vec![(text, false)],
        });
        heap.root(frame, object);
        heap.collect();
        assert!(heap.slots[(text - 1) as usize].is_none());
        assert!(matches!(heap.get(object), Value::Object { .. }));
    }
    /// Frames aninhados preservam argumentos; stress coleta durante milhares de alocações.
    #[test]
    fn nested_frames_stress() {
        let mut heap = Heap::new(true);
        let outer = heap.push_frame();
        let permanent = heap.allocate(Value::String("vivo".into()));
        heap.root(outer, permanent);
        for _ in 0..1000 {
            let inner = heap.push_frame();
            let temporary = heap.allocate(Value::String("temporário".into()));
            heap.root(inner, temporary);
            heap.collect();
            assert!(matches!(heap.get(permanent), Value::String(s) if s == "vivo"));
            heap.pop_frame(inner);
        }
        heap.collect();
        assert_eq!(heap.slots.iter().flatten().count(), 1);
        assert!(heap.slots.len() <= 2);
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;
    /// Igualdade preserva Unicode, NUL embutido e nulidade sem normalização implícita.
    #[test]
    fn unicode_nul_nullable_and_concat() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame();
        let a = heap.allocate(Value::String("á\0🦀".into()));
        heap.root(frame, a);
        let b = heap.allocate(Value::String("á\0🦀".into()));
        heap.root(frame, b);
        assert_ne!(a, b);
        assert!(heap.string_equal(a, b));
        assert!(heap.string_equal(0, 0));
        assert!(!heap.string_equal(0, a));
        assert!(!heap.string_equal(a, 0));
        let c = heap.allocate(Value::String("a\u{301}\0🦀".into()));
        heap.root(frame, c);
        assert!(!heap.string_equal(a, c));
        let joined = heap.string_concat(a, b);
        heap.root(frame, joined);
        assert!(matches!(heap.get(joined), Value::String(s) if s == "á\0🦀á\0🦀"));
    }
    /// Retorno transfere raiz sem coleta entre pop e registro no chamador.
    #[test]
    fn returned_handles_and_identity() {
        let mut heap = Heap::new(true);
        let caller = heap.push_frame();
        let callee = heap.push_frame();
        let a = heap.allocate(Value::Object {
            class_id: 7,
            fields: vec![],
        });
        heap.root(callee, a);
        heap.pop_frame(callee);
        heap.root(caller, a);
        let b = heap.allocate(Value::Object {
            class_id: 7,
            fields: vec![],
        });
        heap.root(caller, b);
        assert_ne!(a, b);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 2);
    }
    /// Tracing e destruição de um ciclo grande não dependem da pilha de chamadas.
    #[test]
    fn large_cycle_is_iterative() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame();
        let first = heap.allocate(Value::Object {
            class_id: 1,
            fields: vec![(0, true)],
        });
        heap.root(frame, first);
        let mut previous = first;
        for _ in 1..20_000 {
            let next = heap.allocate(Value::Object {
                class_id: 1,
                fields: vec![(first, true)],
            });
            heap.set(previous, 0, next, true);
            previous = next;
        }
        heap.collect();
        assert_eq!(heap.stats().live_objects, 20_000);
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
        assert_eq!(heap.stats().reclaimed, 20_000);
    }
    /// Microbenchmark reproduzível: tempos incluem raízes e coletas automáticas declaradas.
    #[test]
    #[ignore = "microbenchmark; execute em release com --ignored --nocapture"]
    fn gc_microbenchmark() {
        for (count, stress) in [(100_000, false), (2_000, true)] {
            let mut heap = Heap::new(stress);
            let frame = heap.push_frame();
            let started = std::time::Instant::now();
            for index in 0..count {
                let handle = heap.allocate(Value::Object {
                    class_id: 1,
                    fields: vec![(index, false)],
                });
                heap.root(frame, handle);
                std::hint::black_box(handle);
            }
            let allocation_ns = started.elapsed().as_nanos();
            let started = std::time::Instant::now();
            heap.collect();
            let collect_live_ns = started.elapsed().as_nanos();
            heap.pop_frame(frame);
            let started = std::time::Instant::now();
            heap.collect();
            let collect_dead_ns = started.elapsed().as_nanos();
            let stats = heap.stats();
            println!(
                "count={count} stress={stress} alloc_root_auto_gc_ns={allocation_ns} collect_live_ns={collect_live_ns} collect_dead_ns={collect_dead_ns} stats={stats:?}"
            );
            assert_eq!(stats.live_objects, 0);
            assert_eq!(stats.reclaimed, count as u64);
        }
    }
}
