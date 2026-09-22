//! Heap preciso com tracing iterativo, raízes explícitas e contadores observáveis.
//! Células, ambientes, closures e listas são infraestrutura: ainda não implicam
//! lowering Dart para LLVM. O contrato detalhado está em CONTRACT.md nesta crate.

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
    /// Valores enum canônicos mantidos vivos até encerrar o runtime.
    pub permanent_roots: usize,
    pub live_roots: usize,
    pub peak_roots: usize,
    pub root_slots: usize,
    pub peak_root_slots: usize,
    /// Cabeçalhos vivos e capacidades dos payloads, sem metadados auxiliares/RSS.
    pub estimated_bytes: usize,
    pub peak_estimated_bytes: usize,
}

/// Marca escalar precisa; bits coincidentes entre int e bool não são iguais.
///
/// A distinção existe porque chaves de `Map` seguem `==` de Dart: `0` e `false`
/// são chaves diferentes, e zero como handle null só vale para referências.
/// A invariante é `is_ref ⟺ tag == Ref`; os construtores abaixo a garantem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueTag {
    Int,
    Bool,
    Double,
    Ref,
}

/// Payload com tag precisa; bits escalares jamais são interpretados como handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaggedValue {
    pub bits: i64,
    pub is_ref: bool,
    pub tag: ValueTag,
}
impl TaggedValue {
    /// Representa inteiro; booleanos usam [`TaggedValue::boolean`].
    pub fn scalar(bits: i64) -> Self {
        Self {
            bits,
            is_ref: false,
            tag: ValueTag::Int,
        }
    }
    /// Representa booleano sem confundir `false` (bits 0) com inteiro zero.
    pub fn boolean(value: bool) -> Self {
        Self {
            bits: i64::from(value),
            is_ref: false,
            tag: ValueTag::Bool,
        }
    }
    /// Representa ponto flutuante de 64 bits.
    pub fn double(val: f64) -> Self {
        Self {
            bits: val.to_bits() as i64,
            is_ref: false,
            tag: ValueTag::Double,
        }
    }
    /// Representa handle gerenciado; zero representa referência null.
    pub fn reference(handle: i64) -> Self {
        Self {
            bits: handle,
            is_ref: true,
            tag: ValueTag::Ref,
        }
    }
}

/// Valor gerenciado; somente campos marcados como referência participam do tracing.
#[derive(Debug)]
pub enum Value {
    String(String),
    RawString(Vec<u16>),
    StringBuffer(String),
    RegExp(String),
    Match(String),
    Object {
        class_id: i64,
        fields: Vec<(i64, bool)>,
    },
    /// Local capturado mutável compartilhado por ambientes distintos.
    Cell(TaggedValue),
    /// Capturas ordenadas imutáveis; mutabilidade compartilhada usa Cell.
    Environment(Vec<TaggedValue>),
    /// Identidade própria, código simbólico e ambiente; não executa código Rust/Dart.
    Closure {
        code_id: i64,
        environment: i64,
    },
    /// Lista expansível de payloads tipados para tracing, sem generics Dart ainda.
    List(Vec<TaggedValue>),
    /// Mapa de inserção ordenada, como o `LinkedHashMap` padrão de Dart.
    ///
    /// Chaves seguem `==` observável: escalares distinguem int de bool pelos
    /// bits e pela tag, e referências usam identidade, exceto strings, que
    /// comparam conteúdo. A busca é linear; adequada ao subconjunto, não a
    /// mapas grandes de produção.
    Map(Vec<(TaggedValue, TaggedValue)>),
    /// Conjunto de inserção ordenada, como o `LinkedHashSet` padrão de Dart.
    Set(Vec<TaggedValue>),
    /// Record do Dart: `(1, 'b')`
    Record(Vec<TaggedValue>),
}
impl Value {
    /// Estima armazenamento próprio usando capacidades efetivas, com overflow explícito.
    fn estimated_bytes(&self) -> usize {
        let payload = match self {
            Self::String(text) | Self::StringBuffer(text) | Self::RegExp(text) | Self::Match(text) => text.capacity(),
            Self::RawString(v) => v.capacity().checked_mul(2).expect("payload excede usize"),
            Self::Object { fields, .. } => fields
                .capacity()
                .checked_mul(std::mem::size_of::<(i64, bool)>())
                .expect("payload excede usize"),
            Self::Cell(_) | Self::Closure { .. } => 0,
            Self::Environment(values) | Self::List(values) | Self::Set(values) | Self::Record(values) => values
                .capacity()
                .checked_mul(std::mem::size_of::<TaggedValue>())
                .expect("payload excede usize"),
            Self::Map(entries) => entries
                .capacity()
                .checked_mul(std::mem::size_of::<(TaggedValue, TaggedValue)>())
                .expect("payload excede usize"),
        };
        std::mem::size_of::<Self>()
            .checked_add(payload)
            .expect("payload excede usize")
    }
    /// Igualdade de chaves de `Map`/`Set` segundo `==` observável de Dart.
    fn trace(&self, pending: &mut Vec<i64>) {
        match self {
            Self::String(_) | Self::RawString(_) | Self::StringBuffer(_) | Self::RegExp(_) | Self::Match(_) => {}
            Self::Object { fields, .. } => pending.extend(
                fields
                    .iter()
                    .filter_map(|(bits, is_ref)| is_ref.then_some(*bits)),
            ),
            Self::Cell(value) => {
                if value.is_ref {
                    pending.push(value.bits);
                }
            }
            Self::Environment(values) | Self::List(values) | Self::Set(values) | Self::Record(values) => pending.extend(
                values
                    .iter()
                    .filter_map(|value| value.is_ref.then_some(value.bits)),
            ),
            Self::Map(entries) => pending.extend(entries.iter().flat_map(|(key, value)| {
                [key, value]
                    .into_iter()
                    .filter_map(|part| part.is_ref.then_some(part.bits))
            })),
            Self::Closure { environment, .. } => pending.push(*environment),
        }
    }
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
    byte_threshold: usize,
    enum_values: std::collections::HashMap<(i64, i64), i64>,
    /// Tear-offs canônicos de funções top-level, por ID de código.
    ///
    /// O oráculo Dart 3.6.2 exige `identical(f, f)` verdadeiro para dois
    /// tear-offs da mesma função top-level; cada `code_id` tem um único handle,
    /// mantido vivo como raiz permanente, como os singletons de enum.
    tearoffs: std::collections::HashMap<i64, i64>,
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
            byte_threshold: 1024 * 1024,
            enum_values: std::collections::HashMap::new(),
            tearoffs: std::collections::HashMap::new(),
        }
    }
    /// Obtém o singleton de um valor enum, protegendo as alocações internas.
    pub fn enum_value(&mut self, class_id: i64, index: i64, name: &str) -> i64 {
        assert!(class_id >= 0 && index >= 0, "identidade enum inválida");
        if let Some(&handle) = self.enum_values.get(&(class_id, index)) {
            return handle;
        }
        let frame = self.push_frame_with_slots(1);
        let text = self.allocate(Value::String(name.to_owned()));
        self.set_root(frame, 0, text);
        let object = self.allocate(Value::Object {
            class_id,
            fields: vec![(index, false), (text, true)],
        });
        self.enum_values.insert((class_id, index), object);
        self.pop_frame(frame);
        object
    }
    /// Obtém o tear-off canônico de uma função top-level, criando-o uma vez.
    ///
    /// O ID de código é o índice da função no módulo; o ambiente é vazio e o
    /// handle devolvido é estável entre chamadas, preservando `identical`.
    /// Tear-offs de métodos (com receptor capturado) não passam por aqui:
    /// cada avaliação cria uma closure nova.
    pub fn tearoff(&mut self, code_id: i64) -> i64 {
        assert!(code_id >= 0, "ID de código inválido");
        if let Some(&handle) = self.tearoffs.get(&code_id) {
            return handle;
        }
        let frame = self.push_frame_with_slots(1);
        let env = self.create_environment(Vec::new());
        self.set_root(frame, 0, env);
        let closure = self.create_closure(code_id, env);
        self.tearoffs.insert(code_id, closure);
        self.pop_frame(frame);
        closure
    }
    /// Abre frame de raízes com identificador monotônico.
    pub fn push_frame(&mut self) -> i64 {
        self.push_frame_with_slots(0)
    }
    /// Reserva slots fixos inicialmente null, reutilizados por todas as iterações.
    pub fn push_frame_with_slots(&mut self, slots: usize) -> i64 {
        let id = self.next_frame;
        self.next_frame = id.checked_add(1).expect("frames esgotados");
        self.stats.root_slots = self
            .stats
            .root_slots
            .checked_add(slots)
            .expect("slots excedem usize");
        self.stats.peak_root_slots = self.stats.peak_root_slots.max(self.stats.root_slots);
        self.frames.push((id, vec![0; slots]));
        id
    }
    /// Substitui a raiz do slot; zero libera a referência anteriormente retida.
    pub fn set_root(&mut self, frame: i64, slot: usize, handle: i64) {
        if handle != 0 {
            self.get(handle);
        }
        let roots = &mut self
            .frames
            .iter_mut()
            .rev()
            .find(|(id, _)| *id == frame)
            .expect("frame inexistente")
            .1;
        let previous = roots.get_mut(slot).expect("slot de raiz inválido");
        self.stats.live_roots -= usize::from(*previous != 0);
        self.stats.live_roots += usize::from(handle != 0);
        *previous = handle;
        self.stats.peak_roots = self.stats.peak_roots.max(self.stats.live_roots);
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
        self.stats.live_roots += 1;
        self.stats.root_slots += 1;
        self.stats.peak_roots = self.stats.peak_roots.max(self.stats.live_roots);
        self.stats.peak_root_slots = self.stats.peak_root_slots.max(self.stats.root_slots);
    }
    /// Fecha exatamente o frame do topo, sem coletar entre retorno e raiz do chamador.
    pub fn pop_frame(&mut self, frame: i64) {
        assert_eq!(self.frames.last().map(|(id, _)| *id), Some(frame));
        let (_, roots) = self.frames.pop().unwrap();
        self.stats.root_slots -= roots.len();
        self.stats.live_roots -= roots.iter().filter(|handle| **handle != 0).count();
    }
    /// Aloca após coleta; o chamador deve proteger o resultado antes de outra alocação.
    pub fn allocate(&mut self, value: Value) -> i64 {
        let bytes = value.estimated_bytes();
        if self.stress
            || (!self.frames.is_empty() && (
                self.allocations >= self.threshold
                || self.stats.estimated_bytes.saturating_add(bytes) > self.byte_threshold
            ))
        {
            self.collect();
        }
        self.stats.estimated_bytes = self
            .stats
            .estimated_bytes
            .checked_add(bytes)
            .expect("heap excede usize");
        self.stats.peak_estimated_bytes = self
            .stats
            .peak_estimated_bytes
            .max(self.stats.estimated_bytes);
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
    /// Obtém valor vivo se o handle for válido, ou None se inválido/destruído.
    pub fn try_get(&self, handle: i64) -> Option<&Value> {
        let index = usize::try_from(handle.checked_sub(1)?).ok()?;
        self.slots.get(index).and_then(Option::as_ref)
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
    /// Aloca protegendo as referências do payload contra a coleta anterior à alocação.
    fn allocate_linked(&mut self, value: Value) -> i64 {
        let mut references = Vec::new();
        value.trace(&mut references);
        for &handle in &references {
            if handle != 0 {
                self.get(handle);
            }
        }
        let frame = self.push_frame_with_slots(references.len());
        for (slot, handle) in references.into_iter().enumerate() {
            self.set_root(frame, slot, handle);
        }
        let handle = self.allocate(value);
        self.pop_frame(frame);
        handle
    }
    /// Igualdade de chaves de `Map`/`Set` segundo `==` observável de Dart.
    ///
    /// Escalares comparam tag e bits (`0` int difere de `false`); referências
    /// null só igualam null; strings comparam conteúdo e demais referências,
    /// identidade de handle.
    fn key_equal(&self, left: &TaggedValue, right: &TaggedValue) -> bool {
        match (left.tag, right.tag) {
            (ValueTag::Int, ValueTag::Int)
            | (ValueTag::Bool, ValueTag::Bool)
            | (ValueTag::Double, ValueTag::Double) => {
                left.bits == right.bits
            }
            (ValueTag::Ref, ValueTag::Ref) => {
                if left.bits == 0 || right.bits == 0 {
                    return left.bits == right.bits;
                }
                if left.bits == right.bits {
                    return true;
                }
                self.string_equal(left.bits, right.bits)
            }
            _ => false,
        }
    }
    /// Cria célula compartilhável; proteja o resultado antes da próxima alocação.
    pub fn create_cell(&mut self, value: TaggedValue) -> i64 {
        self.allocate_linked(Value::Cell(value))
    }
    /// Lê captura mutável sem copiar o objeto apontado por uma referência.
    pub fn cell_get(&self, handle: i64) -> TaggedValue {
        match self.get(handle) {
            Value::Cell(value) => *value,
            _ => panic!("célula esperada"),
        }
    }
    /// Muda a captura observada por todos os ambientes que compartilham esta célula.
    pub fn cell_set(&mut self, handle: i64, value: TaggedValue) {
        self.validate_tag(value);
        let Value::Cell(current) = self.get_mut(handle) else {
            panic!("célula esperada")
        };
        *current = value;
    }
    /// Cria ambiente imutável; cada captura mutável deve apontar para uma célula.
    pub fn create_environment(&mut self, captures: Vec<TaggedValue>) -> i64 {
        self.allocate_linked(Value::Environment(captures))
    }
    /// Obtém captura por índice; índice inválido provoca panic, sem acesso inseguro.
    pub fn environment_get(&self, handle: i64, index: usize) -> TaggedValue {
        match self.get(handle) {
            Value::Environment(captures) => captures[index],
            _ => panic!("ambiente esperado"),
        }
    }
    /// Cria nova identidade de closure mesmo para o mesmo código e ambiente.
    /// O ID de código é simbólico: esta API não realiza despacho nem execução Dart.
    pub fn create_closure(&mut self, code_id: i64, environment: i64) -> i64 {
        assert!(code_id >= 0, "ID de código inválido");
        assert!(
            matches!(self.get(environment), Value::Environment(_)),
            "ambiente esperado"
        );
        self.allocate_linked(Value::Closure {
            code_id,
            environment,
        })
    }
    /// Retorna código simbólico e ambiente, preservando a identidade do handle.
    pub fn closure_parts(&self, handle: i64) -> (i64, i64) {
        match self.get(handle) {
            Value::Closure {
                code_id,
                environment,
            } => (*code_id, *environment),
            _ => panic!("closure esperada"),
        }
    }
    /// Cria lista expansível com tracing preciso de seus elementos gerenciados.
    pub fn create_list(&mut self, values: Vec<TaggedValue>) -> i64 {
        self.allocate_linked(Value::List(values))
    }
    /// Quantidade de elementos inicializados; capacidade interna não é comprimento.
    pub fn list_len(&self, handle: i64) -> usize {
        match self.get(handle) {
            Value::List(values) => values.len(),
            _ => panic!("lista esperada"),
        }
    }
    /// Lê o elemento sem alterar sua tag ou identidade.
    pub fn list_get(&self, handle: i64, index: usize) -> TaggedValue {
        match self.get(handle) {
            Value::List(values) => values[index],
            _ => panic!("lista esperada"),
        }
    }
    /// Substitui elemento existente; referências removidas deixam de ser rastreadas.
    pub fn list_set(&mut self, handle: i64, index: usize, value: TaggedValue) {
        self.validate_tag(value);
        let Value::List(values) = self.get_mut(handle) else {
            panic!("lista esperada")
        };
        values[index] = value;
    }
    /// Acrescenta elemento e contabiliza capacidade real do buffer na política de GC.
    /// A lista permanece protegida durante eventual coleta causada pelo crescimento.
    pub fn list_push(&mut self, handle: i64, value: TaggedValue) {
        self.validate_tag(value);
        let previous = self.get(handle).estimated_bytes();
        let Value::List(values) = self.get_mut(handle) else {
            panic!("lista esperada")
        };
        values.push(value);
        let added = self.get(handle).estimated_bytes() - previous;
        self.stats.estimated_bytes = self
            .stats
            .estimated_bytes
            .checked_add(added)
            .expect("heap excede usize");
        self.stats.peak_estimated_bytes = self
            .stats
            .peak_estimated_bytes
            .max(self.stats.estimated_bytes);
        if self.stress || self.stats.estimated_bytes > self.byte_threshold {
            let frame = self.push_frame_with_slots(1);
            self.set_root(frame, 0, handle);
            self.collect();
            self.pop_frame(frame);
        }
    }
    /// Cria mapa com ordem de inserção; chaves duplicadas conservam a última.
    ///
    /// A entrada duplicada mantém a posição da primeira ocorrência e o valor da
    /// última, como o literal de mapa do SDK 3.6.2. O chamador enraíza o
    /// resultado antes da próxima operação que possa coletar.
    pub fn create_map(&mut self, entries: Vec<(TaggedValue, TaggedValue)>) -> i64 {
        let mut unique: Vec<(TaggedValue, TaggedValue)> = Vec::with_capacity(entries.len());
        for (key, value) in entries {
            self.validate_tag(key);
            self.validate_tag(value);
            if let Some(slot) = unique.iter_mut().find(|(existing, _)| {
                // `find` não tem acesso a `self` sem conflito de empréstimo;
                // a comparação repete `key_equal` sobre chaves já validadas.
                self.key_equal(existing, &key)
            }) {
                slot.1 = value;
            } else {
                unique.push((key, value));
            }
        }
        self.allocate_linked(Value::Map(unique))
    }
    /// Quantidade de pares; capacidade interna não é comprimento.
    pub fn map_len(&self, handle: i64) -> usize {
        match self.get(handle) {
            Value::Map(entries) => entries.len(),
            _ => panic!("mapa esperado"),
        }
    }
    /// Diz se a chave existe, sem distinguir valor null de ausência pelo valor.
    pub fn map_contains(&self, handle: i64, key: TaggedValue) -> bool {
        match self.get(handle) {
            Value::Map(entries) => entries
                .iter()
                .any(|(existing, _)| self.key_equal(existing, &key)),
            _ => panic!("mapa esperado"),
        }
    }
    /// Obtém o valor da chave; ausência provoca panic, sem acesso inseguro.
    ///
    /// O protocolo LLVM consulta `map_contains` antes; esta API não devolve
    /// "null por ausência" para não confundir valor null armazenado com falta.
    pub fn map_get(&self, handle: i64, key: TaggedValue) -> TaggedValue {
        match self.get(handle) {
            Value::Map(entries) => entries
                .iter()
                .find(|(existing, _)| self.key_equal(existing, &key))
                .map(|(_, value)| *value)
                .expect("chave ausente"),
            _ => panic!("mapa esperado"),
        }
    }
    /// Insere ou substitui, preservando a posição da primeira ocorrência.
    pub fn map_set(&mut self, handle: i64, key: TaggedValue, value: TaggedValue) {
        self.validate_tag(key);
        self.validate_tag(value);
        let index = match self.get(handle) {
            Value::Map(entries) => entries
                .iter()
                .position(|(existing, _)| self.key_equal(existing, &key)),
            _ => panic!("mapa esperado"),
        };
        let Value::Map(entries) = self.get_mut(handle) else {
            panic!("mapa esperado")
        };
        if let Some(index) = index {
            entries[index].1 = value;
        } else {
            entries.push((key, value));
        }
    }
    /// Cria conjunto com ordem de inserção; duplicadas conservam a primeira.
    pub fn create_set(&mut self, values: Vec<TaggedValue>) -> i64 {
        let mut unique = Vec::with_capacity(values.len());
        for value in values {
            self.validate_tag(value);
            if !unique
                .iter()
                .any(|existing| self.key_equal(existing, &value))
            {
                unique.push(value);
            }
        }
        self.allocate_linked(Value::Set(unique))
    }
    /// Quantidade de elementos distintos.
    pub fn set_len(&self, handle: i64) -> usize {
        match self.get(handle) {
            Value::Set(values) => values.len(),
            _ => panic!("conjunto esperado"),
        }
    }
    /// Pertinência segundo `==` observável de chaves.
    pub fn set_contains(&self, handle: i64, value: TaggedValue) -> bool {
        match self.get(handle) {
            Value::Set(values) => values
                .iter()
                .any(|existing| self.key_equal(existing, &value)),
            _ => panic!("conjunto esperado"),
        }
    }
    /// Insere quando ausente; devolve se houve inserção (`Set.add` de Dart).
    pub fn set_add(&mut self, handle: i64, value: TaggedValue) -> bool {
        self.validate_tag(value);
        let present = match self.get(handle) {
            Value::Set(values) => values
                .iter()
                .any(|existing| self.key_equal(existing, &value)),
            _ => panic!("conjunto esperado"),
        };
        if present {
            return false;
        }
        let previous = self.get(handle).estimated_bytes();
        let Value::Set(values) = self.get_mut(handle) else {
            panic!("conjunto esperado")
        };
        values.push(value);
        let added = self.get(handle).estimated_bytes() - previous;
        self.stats.estimated_bytes = self
            .stats
            .estimated_bytes
            .checked_add(added)
            .expect("heap excede usize");
        self.stats.peak_estimated_bytes = self
            .stats
            .peak_estimated_bytes
            .max(self.stats.estimated_bytes);
        true
    }
    /// Valida referência antes de modificar o grafo; null não exige objeto vivo.
    fn validate_tag(&self, value: TaggedValue) {
        assert_eq!(
            value.is_ref,
            value.tag == ValueTag::Ref,
            "tag inconsistente com is_ref"
        );
        if value.is_ref && value.bits != 0 {
            self.get(value.bits);
        }
    }
    /// Obtém armazenamento mutável; não oferece acesso a slots já coletados.
    pub fn get_mut(&mut self, handle: i64) -> &mut Value {
        let slot = usize::try_from(handle.checked_sub(1).expect("handle inválido"))
            .expect("handle inválido");
        self.slots
            .get_mut(slot)
            .and_then(Option::as_mut)
            .expect("handle não vivo")
    }
    /// Marca raízes e arestas tipadas iterativamente e libera inclusive ciclos inalcançáveis.
    pub fn collect(&mut self) {
        self.stats.collections += 1;
        self.stats.roots_scanned += self.enum_values.len() as u64;
        self.stats.roots_scanned += self.tearoffs.len() as u64;
        self.stats.roots_scanned += self
            .frames
            .iter()
            .map(|(_, roots)| roots.len() as u64)
            .sum::<u64>();
        self.stats.slots_scanned += self.slots.len() as u64;
        self.marks.resize(self.slots.len(), false);
        self.marks.fill(false);
        self.pending.clear();
        self.pending.extend(self.enum_values.values().copied());
        self.pending.extend(self.tearoffs.values().copied());
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
            self.slots[index]
                .as_ref()
                .expect("handle não vivo")
                .trace(&mut self.pending);
        }
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if slot.is_some() && !self.marks[index] {
                self.stats.estimated_bytes -= slot.as_ref().unwrap().estimated_bytes();
                *slot = None;
                self.free.push(index);
                self.stats.reclaimed += 1;
            }
        }
        self.allocations = 0;
        self.threshold = live.saturating_mul(2).max(256);
        self.byte_threshold = self
            .stats
            .estimated_bytes
            .saturating_mul(2)
            .max(1024 * 1024);
    }

    /// Obtém contadores sem percorrer os objetos ou suas raízes.
    pub fn stats(&self) -> HeapStats {
        HeapStats {
            live_objects: self.slots.len() - self.free.len(),
            reserved_slots: self.slots.len(),
            permanent_roots: self.enum_values.len() + self.tearoffs.len(),
            ..self.stats
        }
    }

    /// Compara conteúdo, preservando NUL e diferenças entre sequências Unicode.
    pub fn string_equal(&self, a: i64, b: i64) -> bool {
        if a == 0 || b == 0 {
            return a == b;
        }
        match (self.get(a), self.get(b)) {
            (Value::String(left), Value::String(right)) => left == right,
            (Value::RawString(left), Value::RawString(right)) => left == right,
            (Value::String(left), Value::RawString(right)) => {
                left.encode_utf16().eq(right.iter().copied())
            }
            (Value::RawString(left), Value::String(right)) => {
                left.iter().copied().eq(right.encode_utf16())
            }
            _ => false,
        }
    }

    /// Concatena conteúdo antes da possível coleta; os operandos seguem o protocolo de raízes.
    pub fn string_concat(&mut self, a: i64, b: i64) -> i64 {
        match (self.get(a), self.get(b)) {
            (Value::String(left), Value::String(right)) => {
                let result = format!("{left}{right}");
                self.allocate(Value::String(result))
            }
            _ => {
                let mut units: Vec<u16> = match self.get(a) {
                    Value::String(s) => s.encode_utf16().collect(),
                    Value::RawString(r) => r.clone(),
                    _ => panic!("string esperada"),
                };
                match self.get(b) {
                    Value::String(s) => units.extend(s.encode_utf16()),
                    Value::RawString(r) => units.extend_from_slice(r),
                    _ => panic!("string esperada"),
                };
                match String::from_utf16(&units) {
                    Ok(valid_str) => self.allocate(Value::String(valid_str)),
                    Err(_) => self.allocate(Value::RawString(units)),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Enums preservam identidade e nomes sem frames externos, mesmo em stress.
    #[test]
    fn enum_singletons_survive_collection_and_keep_nominal_identity() {
        let mut heap = Heap::new(true);
        let first = heap.enum_value(1, 0, "red");
        let second = heap.enum_value(1, 1, "blue");
        let other_class = heap.enum_value(2, 0, "red");
        heap.collect();
        assert_eq!(heap.enum_value(1, 0, "red"), first);
        assert_ne!(first, second);
        assert_ne!(first, other_class);
        let Value::Object { class_id, fields } = heap.get(first) else {
            panic!("enum deve ser objeto")
        };
        assert_eq!(*class_id, 1);
        assert_eq!(fields[0], (0, false));
        assert!(matches!(heap.get(fields[1].0), Value::String(name) if name == "red"));
        assert_eq!(heap.stats().permanent_roots, 3);
        assert_eq!(heap.stats().live_objects, 6);
        assert_eq!(heap.stats().root_slots, 0);
    }
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

#[cfg(test)]
mod fixed_root_tests {
    use super::*;
    /// Slots substituídos não crescem com iterações; cópias locais têm raízes independentes.
    #[test]
    fn fixed_slots_preserve_copies_and_nested_returns() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame_with_slots(2);
        let kept = heap.allocate(Value::String("keep".into()));
        heap.set_root(frame, 0, kept);
        heap.set_root(frame, 1, kept);
        for _ in 0..1000 {
            let inner = heap.push_frame_with_slots(1);
            let temporary = heap.allocate(Value::String("next".into()));
            heap.set_root(inner, 0, temporary);
            heap.pop_frame(inner);
            heap.set_root(frame, 0, temporary);
            heap.collect();
            assert!(matches!(heap.get(kept), Value::String(s) if s == "keep"));
        }
        assert_eq!(heap.stats().peak_root_slots, 3);
        assert_eq!(heap.stats().live_roots, 2);
        heap.set_root(frame, 0, 0);
        heap.set_root(frame, 1, 0);
        heap.collect();
        assert_eq!(heap.stats().live_roots, 0);
        assert_eq!(heap.stats().estimated_bytes, 0);
        assert_eq!(heap.stats().live_objects, 0);
        assert!(heap.stats().reserved_slots <= 3);
        heap.pop_frame(frame);
        assert_eq!(heap.stats().root_slots, 0);
    }
    /// Payload grande dispara coleta antes do limiar por quantidade de objetos.
    #[test]
    fn byte_trigger_collects_large_payloads() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        for _ in 0..8 {
            let handle = heap.allocate(Value::String(String::with_capacity(2 * 1024 * 1024)));
            heap.set_root(frame, 0, handle);
        }
        let stats = heap.stats();
        assert!(stats.collections >= 3);
        assert!(stats.reclaimed >= 5);
        assert!(stats.peak_estimated_bytes < 7 * 1024 * 1024);
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(heap.stats().estimated_bytes, 0);
    }
    /// Mede alocações transientes com uma única raiz sobrescrita, sem alegar superioridade.
    #[test]
    #[ignore = "microbenchmark de slots fixos; execute release --ignored --nocapture"]
    fn fixed_slot_microbenchmark() {
        for stress in [false, true] {
            let mut heap = Heap::new(stress);
            let frame = heap.push_frame_with_slots(1);
            let start = std::time::Instant::now();
            for _ in 0..100_000 {
                let handle = heap.allocate(Value::String("temporary string".into()));
                heap.set_root(frame, 0, handle);
            }
            let allocation_ns = start.elapsed().as_nanos();
            let start = std::time::Instant::now();
            heap.collect();
            let collection_ns = start.elapsed().as_nanos();
            let stats = heap.stats();
            println!(
                "fixed count=100000 stress={stress} alloc_root_auto_gc_ns={allocation_ns} collect_ns={collection_ns} stats={stats:?}"
            );
            assert_eq!(stats.peak_root_slots, 1);
            assert_eq!(stats.live_objects, 1);
            assert_eq!(stats.reclaimed, 99_999);
            assert!(stats.reserved_slots <= 257);
        }
    }
}
#[cfg(test)]
mod maps_sets_tearoffs {
    use super::*;

    /// Chaves int e bool com os mesmos bits são entradas distintas, como no SDK.
    #[test]
    fn int_and_bool_keys_are_distinct_and_strings_compare_by_content() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame_with_slots(3);
        let first = heap.allocate(Value::String("chave".into()));
        heap.set_root(frame, 0, first);
        let second = heap.allocate(Value::String("chave".into()));
        heap.set_root(frame, 1, second);
        assert_ne!(first, second);
        let map = heap.create_map(vec![
            (TaggedValue::scalar(0), TaggedValue::scalar(1)),
            (TaggedValue::boolean(false), TaggedValue::scalar(2)),
            (TaggedValue::reference(first), TaggedValue::scalar(3)),
        ]);
        heap.set_root(frame, 2, map);
        assert_eq!(heap.map_len(map), 3);
        assert!(heap.map_contains(map, TaggedValue::scalar(0)));
        assert!(heap.map_contains(map, TaggedValue::boolean(false)));
        // Conteúdo igual, handle diferente: mesma chave.
        assert!(heap.map_contains(map, TaggedValue::reference(second)));
        assert_eq!(
            heap.map_get(map, TaggedValue::reference(second)),
            TaggedValue::scalar(3)
        );
        assert!(!heap.map_contains(map, TaggedValue::scalar(7)));
        heap.set_root(frame, 0, 0);
        heap.set_root(frame, 1, 0);
        heap.collect();
        // A chave original sobrevive pelo mapa; a duplicata é coletada.
        assert_eq!(heap.stats().live_objects, 2);
        assert_eq!(
            heap.map_get(map, TaggedValue::reference(first)),
            TaggedValue::scalar(3)
        );
        // Substituição preserva a posição; inserção acrescenta no fim.
        heap.map_set(map, TaggedValue::scalar(0), TaggedValue::scalar(10));
        assert_eq!(heap.map_len(map), 3);
        assert_eq!(
            heap.map_get(map, TaggedValue::scalar(0)),
            TaggedValue::scalar(10)
        );
    }

    /// Literais com chaves duplicadas conservam o último valor, como Dart.
    #[test]
    fn duplicate_literal_keys_keep_the_last_value() {
        let mut heap = Heap::new(false);
        let map = heap.create_map(vec![
            (TaggedValue::scalar(1), TaggedValue::scalar(100)),
            (TaggedValue::scalar(1), TaggedValue::scalar(200)),
        ]);
        assert_eq!(heap.map_len(map), 1);
        assert_eq!(
            heap.map_get(map, TaggedValue::scalar(1)),
            TaggedValue::scalar(200)
        );
    }

    /// Conjuntos removem duplicadas por conteúdo e `add` informa a inserção.
    #[test]
    fn sets_deduplicate_and_report_insertion() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame_with_slots(2);
        let text = heap.allocate(Value::String("dup".into()));
        heap.set_root(frame, 0, text);
        let set = heap.create_set(vec![
            TaggedValue::scalar(1),
            TaggedValue::scalar(1),
            TaggedValue::boolean(true),
            TaggedValue::reference(text),
            TaggedValue::reference(text),
        ]);
        heap.set_root(frame, 1, set);
        // `true` (bits 1) difere de `1` int pela tag.
        assert_eq!(heap.set_len(set), 3);
        assert!(heap.set_contains(set, TaggedValue::boolean(true)));
        assert!(!heap.set_add(set, TaggedValue::scalar(1)));
        assert!(heap.set_add(set, TaggedValue::scalar(9)));
        assert_eq!(heap.set_len(set), 4);
        heap.set_root(frame, 0, 0);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 2);
    }

    /// Tear-offs da mesma função são canônicos e sobrevivem sem frames externos.
    #[test]
    fn top_level_tearoffs_are_canonical_permanent_roots() {
        let mut heap = Heap::new(true);
        let first = heap.tearoff(4);
        let second = heap.tearoff(4);
        let other = heap.tearoff(9);
        assert_eq!(first, second);
        assert_ne!(first, other);
        heap.collect();
        assert_eq!(heap.tearoff(4), first);
        let (code, _) = heap.closure_parts(first);
        assert_eq!(code, 4);
        // Cada tear-off tem closure e ambiente próprios, ambos permanentes.
        assert_eq!(heap.stats().live_objects, 4);
        assert_eq!(heap.stats().permanent_roots, 2);
    }
}
#[cfg(test)]
mod captures_and_lists {
    use super::*;

    /// A closure escapada conserva ambiente e celula depois de fechar o frame criador.
    #[test]
    fn escaping_closure_preserves_mutable_capture() {
        let mut heap = Heap::new(true);
        let outer = heap.push_frame_with_slots(1);
        let creator = heap.push_frame_with_slots(2);
        let cell = heap.create_cell(TaggedValue::scalar(10));
        heap.set_root(creator, 0, cell);
        let env = heap.create_environment(vec![TaggedValue::reference(cell)]);
        heap.set_root(creator, 1, env);
        let closure = heap.create_closure(7, env);
        heap.set_root(outer, 0, closure);
        heap.pop_frame(creator);
        heap.allocate(Value::String("coleta forcada".into()));
        heap.collect();
        let (code, escaped) = heap.closure_parts(closure);
        assert_eq!(code, 7);
        let captured = heap.environment_get(escaped, 0).bits;
        assert_eq!(heap.cell_get(captured), TaggedValue::scalar(10));
        heap.cell_set(captured, TaggedValue::scalar(11));
        assert_eq!(heap.cell_get(cell), TaggedValue::scalar(11));
        heap.pop_frame(outer);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
    }

    /// Ambientes diferentes compartilham celulas, mas closures conservam identidade propria.
    #[test]
    fn aliases_share_cells_and_closure_identity_is_not_code_identity() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame_with_slots(3);
        let cell = heap.create_cell(TaggedValue::scalar(1));
        heap.set_root(frame, 0, cell);
        let first_env = heap.create_environment(vec![TaggedValue::reference(cell)]);
        let first = heap.create_closure(42, first_env);
        heap.set_root(frame, 1, first);
        let second_env = heap.create_environment(vec![TaggedValue::reference(cell)]);
        let second = heap.create_closure(42, second_env);
        heap.set_root(frame, 2, second);
        assert_ne!(first, second);
        let (_, first_env) = heap.closure_parts(first);
        let (_, second_env) = heap.closure_parts(second);
        let alias = heap.environment_get(second_env, 0).bits;
        heap.cell_set(alias, TaggedValue::scalar(8));
        assert_eq!(
            heap.cell_get(heap.environment_get(first_env, 0).bits).bits,
            8
        );
        heap.collect();
        assert_eq!(heap.stats().live_objects, 5);
    }

    /// Tracing iterativo recupera o ciclo closure -> ambiente -> celula -> closure.
    #[test]
    fn closure_capture_cycle_is_collected() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame_with_slots(2);
        let cell = heap.create_cell(TaggedValue::reference(0));
        heap.set_root(frame, 0, cell);
        let env = heap.create_environment(vec![TaggedValue::reference(cell)]);
        heap.set_root(frame, 1, env);
        let closure = heap.create_closure(0, env);
        heap.cell_set(cell, TaggedValue::reference(closure));
        heap.collect();
        assert_eq!(heap.stats().live_objects, 3);
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
        assert_eq!(heap.stats().reclaimed, 3);
    }

    /// Lista distingue handles reais de inteiros coincidentes e libera referencias removidas.
    #[test]
    fn lists_trace_references_not_scalar_bits() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame_with_slots(3);
        let kept = heap.allocate(Value::String("mantido".into()));
        heap.set_root(frame, 0, kept);
        let scalar_bits = heap.allocate(Value::String("descartado".into()));
        heap.set_root(frame, 1, scalar_bits);
        let list = heap.create_list(vec![
            TaggedValue::reference(kept),
            TaggedValue::scalar(scalar_bits),
        ]);
        heap.set_root(frame, 2, list);
        heap.set_root(frame, 0, 0);
        heap.set_root(frame, 1, 0);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 2);
        assert_eq!(heap.list_get(list, 0), TaggedValue::reference(kept));
        heap.list_set(list, 0, TaggedValue::reference(0));
        heap.collect();
        assert_eq!(heap.stats().live_objects, 1);
        heap.list_push(list, TaggedValue::reference(list));
        assert_eq!(heap.list_len(list), 3);
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
    }

    /// Crescimento de capacidade entra nos contadores e nos limites de coleta.
    #[test]
    fn growable_list_accounts_capacity_and_keeps_new_reference() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame_with_slots(1);
        let list = heap.create_list(vec![]);
        heap.set_root(frame, 0, list);
        let before = heap.stats().estimated_bytes;
        for n in 0..128 {
            heap.list_push(list, TaggedValue::scalar(n));
        }
        let text = heap.allocate(Value::String("novo".into()));
        heap.list_push(list, TaggedValue::reference(text));
        assert!(matches!(heap.get(text),Value::String(s) if s=="novo"));
        assert_eq!(heap.list_len(list), 129);
        assert!(heap.stats().estimated_bytes >= before + 129 * std::mem::size_of::<TaggedValue>());
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(heap.stats().estimated_bytes, 0);
    }
}
