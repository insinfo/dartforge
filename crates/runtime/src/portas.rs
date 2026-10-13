// Runtime nativo: portas (`RawReceivePort`/`SendPort`) e a fila de mensagens
// do isolado — o `PortMap`/`MessageHandler` da VM (`runtime/vm/port.cc`,
// `message_handler.cc`).
//
// * Uma porta é um id global (nunca 0). O registro global diz a quem ela
//   pertence: a fila de um isolado (uma thread) ou um serviço do runtime
//   (as portas nativas, como a do IOService), que recebe a mensagem na
//   thread de quem enviou.
// * Enviar copia a mensagem para um grafo portátil (`Graph`), que o destino
//   materializa no próprio heap. É a semântica da VM
//   (`object_graph_copy.cc`): o que é mutável é copiado; o que é
//   profundamente imutável e permanente passa pela identidade — os objetos
//   estáticos da imagem (as caixas de `bool`, os literais de string do AOT)
//   em qualquer isolado do processo; constantes, enums, tear-offs de topo e
//   strings do heap quando a mensagem fica no mesmo isolado.
// * O grafo copia blocos do espaço de objetos pelo formato do corpo
//   (docs/NATIVO-ESPACO-UNIFICADO.md §2.12): `INSTANCIA` campo a campo com o
//   bit de referência; `REFS` pela palavra 0 e as referências; `BRUTO` cru,
//   menos o que aponta para dentro de si ou para fora do heap (lista tipada,
//   visão, anexo), que o destino refaz.
// * A fila do isolado guarda (porta, grafo, chegada). O laço de eventos
//   (`events.rs`) atende timers e mensagens pela ordem de chegada — na VM o
//   timer também é uma mensagem —, e esvazia as microtarefas depois de
//   cada uma.
// * O despacho chama o `_RawReceivePort._despachar` da sobreposição (o
//   `_handleMessage` da VM), que lê a porta e a mensagem correntes.
// * O isolado vive enquanto tiver porta aberta com `keepIsolateAlive`.

/// Uma palavra dentro de um grafo portátil (uma referência ou os bits de um
/// campo escalar, no destino).
#[derive(Clone, Copy, Debug)]
enum GValue {
    /// Uma palavra que não aponta para o heap: `null`, um `Smi` numa posição
    /// `Ref`, ou os bits de um campo escalar.
    Word(i64),
    /// O nó `i` do grafo.
    No(usize),
    /// Um objeto estático da imagem (`PERMANENTE`): o mesmo endereço em todo
    /// isolado do processo, nunca coletado nem gravado.
    Static(i64),
    /// Um objeto permanente do heap de origem, passado pela identidade (só
    /// quando origem e destino são o mesmo isolado).
    Same(i64),
    /// Uma constante canônica, pelo getter gerado que a produz: o destino
    /// recebe a instância canônica dele (o grupo de isolados da VM
    /// compartilha as constantes; aqui cada isolado tem a sua, e `identical`
    /// continua valendo entre eles).
    Constant(usize),
    /// O tear-off canônico de uma função de topo, pelo código, com o corpo
    /// tipado, a ABI e o metadado RTI (o tipo da função) da closure de
    /// origem: o destino que ainda não tem o tear-off o cria igual — antes
    /// nascia sem tipo (`runtimeType` `Function`) e o `as Stream
    /// Function(Stream, dynamic)` do `StreamIsolate` do new_sali/backend
    /// falhava no isolado novo (docs/NATIVO-PROJETOS-REAIS.md, C11). Os ids
    /// de tipo são do processo (`types.rs`).
    TearOff { code: i64, typed_one: i64, abi: i64, metadata: u32 },
}

/// Um nó do grafo: um bloco do espaço de objetos copiado pelo formato, com as
/// referências trocadas por `GValue`.
#[derive(Clone, Debug)]
enum GNode {
    /// Um objeto `INSTANCIA` (do programa, ou do runtime: `_GrowableList`,
    /// `_Closure`, `_Contexto`, `_Celula`, `_SendPort`…): a classe e os campos,
    /// com o bit de referência de cada um.
    Instance { cid: i32, fields: Vec<(GValue, bool)> },
    /// Um corpo `REFS` (`_List`/`_ImmutableList` geral, `_Record`): a palavra 0
    /// (o comprimento ou a forma) e as referências.
    Refs { cid: i32, word0: i64, refs: Vec<GValue> },
    /// Uma `_List`/`_ImmutableList` compacta (`BRUTO` com `ELEMENTO`): os
    /// elementos crus (`i64`, bits de `f64` ou 0/1).
    Compact { cid: i32, shape: crate::listas::Element, elements: Vec<i64> },
    /// Um corpo `BRUTO` sem ponteiro para dentro de si nem para fora do heap
    /// (`_Mint`, `_Double`, SIMD): as palavras e os `flags`, copiados crus.
    Bruto { cid: i32, flags: u8, words: Box<[i64]> },
    /// Uma string (a forma canônica é refeita no destino).
    Texto(Texto),
    /// O acumulador de um `StringBuffer` (`_AcumuladorDeTexto`, `ATTACHMENT`): as
    /// unidades, que moram fora do heap.
    Accumulator(Vec<u16>),
    /// O programa de um `RegExp` (`_ProgramaDeRegExp`, `ATTACHMENT`): o padrão e as
    /// opções, recompilado no destino (`regexp.rs`).
    ReProgram { source: Vec<u16>, options: [bool; 4] },
    /// Uma lista tipada interna ou externa (`EXTERNAL`): os bytes, copiados
    /// numa interna no destino (a VM copia a externa para memória nova,
    /// `object_graph_copy.cc`).
    Typed { tipo: u8, bytes: Vec<u8> },
    /// Uma visão: a base (copiada) e o deslocamento; o endereço dos dados é
    /// recalculado no destino.
    View { cid: i32, base: GValue, offset: usize, len: usize },
    /// Um objeto do runtime de um campo inteiro vindo de fora do heap
    /// (`SendPort`, `Capability` do C): a classe é a da posição `pos` da
    /// cid fixo (`layout::cid`, `_SendPort`/`_Capability`).
    FromRuntime { cid: i32, id: i64 },
}

/// Uma mensagem copiada: os nós (com o metadado RTI de cada um) e a raiz.
#[derive(Clone)]
pub struct Graph {
    nodes: Vec<(GNode, i64)>,
    root: GValue,
    /// O isolado de origem (id da fila): `GValue::Same` só vale nele.
    origin: u64,
    /// Numa mensagem para outro isolado, os tipos (RTI) dos nós: o
    /// metadado de cada nó (`1 + tipo`) é então `1 + índice` nesta tabela
    /// (0 = sem tipo), porque os ids de tipo são por isolado.
    types: Option<MessageTypes>,
    /// Numa mensagem para outro isolado, a tabela de métodos de cada classe
    /// dos objetos (registrada no isolado na criação do primeiro objeto da
    /// classe; um objeto que chega numa mensagem não passa por lá).
    tables: Vec<(i64, MethodTable)>,
}

/// Por que uma mensagem não pode ser enviada.
pub struct IllegalMessage(pub String);

/// A palavra `h` (numa posição `Ref`) no grafo: imediata, estática, pela
/// identidade, canônica, ou o nó do objeto (reservado agora e preenchido
/// quando `h` sai da pilha).
fn ref_de_handle(heap: &Heap, h: i64, map: &mut crate::hash::HashMap<i64, usize>, stack: &mut Vec<i64>, nodes: &mut Vec<(GNode, i64)>, share: bool) -> GValue {
    if !crate::layout::is_object(h) {
        return GValue::Word(h);
    }
    if heap.is_static(h) {
        return GValue::Static(h);
    }
    if share {
        if heap.is_permanent(h) || heap.is_string(h) {
            return GValue::Same(h);
        }
    } else if let Some(g) = heap.constant_getter(h) {
        return GValue::Constant(g);
    } else if let Some(c) = heap.tearoff_code(h) {
        let (typed_one, abi) = heap.closure(h).map_or((0, 0), |f| (f.typed_one, f.abi));
        return GValue::TearOff { code: c, typed_one, abi, metadata: heap.header(h).metadata };
    }
    let c = *heap.header(h);
    // Um anexo nativo que o grafo não sabe copiar (só o acumulador do
    // `StringBuffer` e o programa do `RegExp` têm cópia) chega como `null`.
    if c.flags & crate::layout::flags::ATTACHMENT != 0
        && c.class_id != crate::layout::cid::STRING_ACCUMULATOR
        && c.class_id != crate::layout::cid::REGEXP_PROGRAM
    {
        return GValue::Word(0);
    }
    if let Some(&i) = map.get(&h) {
        return GValue::No(i);
    }
    let i = nodes.len();
    map.insert(h, i);
    // Reserva o nó; o conteúdo é preenchido quando `h` sai da pilha.
    nodes.push((GNode::Accumulator(Vec::new()), 0));
    stack.push(h);
    GValue::No(i)
}

/// Classes cujo primeiro campo é MOVIDO numa mensagem (o
/// `TransferableTypedData`): o objeto de origem fica com `null`. Os ids de
/// classe são do programa, então o registro vale para todos os isolados.
fn transferable_classes() -> std::sync::RwLockReadGuard<'static, crate::hash::HashSet<i64>> {
    transferable().read().unwrap_or_else(|e| e.into_inner())
}

fn transferable() -> &'static std::sync::RwLock<crate::hash::HashSet<i64>> {
    static T: std::sync::OnceLock<std::sync::RwLock<crate::hash::HashSet<i64>>> = std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

thread_local! {
    /// Classes cujas instâncias não podem ir numa mensagem (as portas de
    /// recepção; `DartForge_porta_abrir` registra a classe).
    /// Classe → a descrição que a recusa leva (o texto da VM).
    static NOT_SENDABLE: RefCell<crate::hash::HashMap<i64, String>> = RefCell::new(crate::hash::HashMap::default());
    /// A descrição da última mensagem recusada.
    static LAST_REJECTION: RefCell<String> = const { RefCell::new(String::new()) };
}

/// As unidades do acumulador de texto `h` (`_AcumuladorDeTexto`): o anexo em
/// `b+16` é um `Box<Vec<u16>>` (docs/NATIVO-ESPACO-UNIFICADO.md §2.5).
fn accumulator_units(heap: &Heap, h: i64) -> Vec<u16> {
    let p = heap.attachment(h) as *const Vec<u16>;
    if p.is_null() {
        return Vec::new();
    }
    // SAFETY: o anexo de um `_AcumuladorDeTexto` vivo é o `Vec<u16>` dele,
    // solto só quando o bloco morre; o empréstimo do heap o mantém.
    #[allow(unsafe_code)]
    unsafe {
        (*p).clone()
    }
}

/// Os elementos de uma `_List`/`_ImmutableList` como nó do grafo, pela forma.
fn fixed_list_node(heap: &Heap, h: i64, class: i32, v: &mut dyn FnMut(i64) -> GValue) -> GNode {
    use crate::listas::{Element, RefElements};
    match heap.list_elements(h) {
        RefElements::General(e) => GNode::Refs { cid: class, word0: e.len() as i64, refs: e.iter().map(|&x| v(x)).collect() },
        RefElements::Int(e) => GNode::Compact { cid: class, shape: Element::Int, elements: e.to_vec() },
        RefElements::Double(e) => GNode::Compact { cid: class, shape: Element::Double, elements: e.iter().map(|d| d.to_bits() as i64).collect() },
        RefElements::Bool(e) => GNode::Compact { cid: class, shape: Element::Bool, elements: e.to_vec() },
    }
}

/// O nó do bloco `h` (já conferido: nem imediato, nem estático, nem anexo de
/// programa). `v` traduz cada referência encontrada.
fn block_node(heap: &Heap, h: i64, v: &mut dyn FnMut(i64) -> GValue, transferred: &mut Vec<i64>) -> Result<GNode, IllegalMessage> {
    use crate::layout::{cid, flags};
    let c = *heap.header(h);
    let class = c.class_id;
    let shape = c.flags & flags::FORMA;
    if shape == flags::INSTANCIA {
        if cid::is_typed(class) {
            // Visão (`INSTANCIA`, cids 36–65): a base e o deslocamento.
            let t = heap.tipada(h).expect("visão viva");
            return Ok(GNode::View { cid: class, base: v(t.base.unwrap_or(0)), offset: t.offset, len: t.len });
        }
        let object = heap.object(h).expect("objeto vivo");
        let class_id = object.class_id;
        let fields = object.to_vec();
        if let Some(d) = NOT_SENDABLE.with(|n| n.borrow().get(&class_id).cloned()) {
            return Err(IllegalMessage(d));
        }
        if transferable_classes().contains(&class_id) {
            if fields.first().is_none_or(|f| f.0 == 0) {
                return Err(IllegalMessage("(TransferableTypedData has been transferred already)\n".to_string()));
            }
            transferred.push(h);
        }
        let fields = fields.iter().map(|&(bits, is_ref_kind)| (if is_ref_kind { v(bits) } else { GValue::Word(bits) }, is_ref_kind)).collect();
        return Ok(GNode::Instance { cid: class, fields });
    }
    if shape == flags::REFS {
        if cid::is_fixed_list(class) {
            return Ok(fixed_list_node(heap, h, class, v));
        }
        if class == cid::RECORD {
            let fields = heap.record(h).expect("record vivo");
            return Ok(GNode::Refs { cid: class, word0: fields.len() as i64, refs: fields.iter().map(|&x| v(x)).collect() });
        }
        // Outro `REFS`: a palavra 0 é o número de referências.
        let p = heap.words(h);
        let n = usize::try_from(p[0]).unwrap_or(0).min(p.len().saturating_sub(1));
        return Ok(GNode::Refs { cid: class, word0: p[0], refs: p[1..=n].iter().map(|&x| v(x)).collect() });
    }
    // `BRUTO`.
    if c.flags & flags::ATTACHMENT != 0 {
        if class == cid::REGEXP_PROGRAM {
            let (source, options) = re_program_pattern(heap, h);
            return Ok(GNode::ReProgram { source, options });
        }
        return Ok(GNode::Accumulator(accumulator_units(heap, h)));
    }
    if cid::is_string(class) {
        return Ok(GNode::Texto(heap.text(h).expect("string viva").to_string_fn()));
    }
    if cid::is_typed(class) {
        let t = heap.tipada(h).expect("lista tipada viva");
        let bytes = heap.typed_bytes(h).map(<[u8]>::to_vec).unwrap_or_default();
        return Ok(GNode::Typed { tipo: t.tipo, bytes });
    }
    if cid::is_fixed_list(class) {
        // Compacta: `list_elements` diz a forma.
        return Ok(fixed_list_node(heap, h, class, v));
    }
    Ok(GNode::Bruto { cid: class, flags: c.flags, words: heap.words(h).into() })
}

/// Copia o valor `root` (um valor numa posição `Ref`) para um grafo.
/// `share`: o destino é este mesmo isolado.
fn copy_to_graph(root: i64, share: bool) -> Result<Graph, IllegalMessage> {
    let mut map = crate::hash::HashMap::default();
    let mut stack = Vec::new();
    let mut nodes: Vec<(GNode, i64)> = Vec::new();
    let mut types = (!share).then(MessageTypes::default);
    let mut transferred: Vec<i64> = Vec::new();
    let r = HEAP.with(|heap| ref_de_handle(&heap.borrow(), root, &mut map, &mut stack, &mut nodes, share));
    while let Some(h) = stack.pop() {
        let i = map[&h];
        let r = HEAP.with(|heap| -> Result<(GNode, i64), IllegalMessage> {
            let heap = heap.borrow();
            let meta = i64::from(heap.header(h).metadata);
            let mut v = |x: i64| ref_de_handle(&heap, x, &mut map, &mut stack, &mut nodes, share);
            let no = block_node(&heap, h, &mut v, &mut transferred)?;
            Ok((no, meta))
        });
        let (mut no, meta) = r?;
        let meta = match types.as_mut() {
            // O metadado é `1 + tipo` (0 = sem tipo, `types.rs`).
            Some(t) if meta != 0 => 1 + t.export(meta - 1),
            _ => meta,
        };
        // Um objeto `Type` leva o tipo pela tabela da mensagem.
        if let (Some(t), GNode::Instance { cid, fields }) = (types.as_mut(), &mut no)
            && i64::from(*cid) == TYPE_CLASS
            && let Some((GValue::Word(id), _)) = fields.first().copied()
        {
            fields[0] = (GValue::Word(t.export(id)), false);
        }
        nodes[i] = (no, meta);
    }
    // Os `TransferableTypedData` enviados ficam vazios na origem.
    if !transferred.is_empty() {
        HEAP.with(|heap| {
            let mut heap = heap.borrow_mut();
            for h in transferred {
                if heap.object(h).is_some_and(|o| !o.is_empty()) {
                    heap.set_field(h, 0, 0, true);
                }
            }
        });
    }
    let tables = if share {
        Vec::new()
    } else {
        let classes: std::collections::BTreeSet<i64> = nodes
            .iter()
            .filter_map(|(no, _)| match no {
                GNode::Instance { cid, .. } => Some(i64::from(*cid)),
                _ => None,
            })
            .collect();
        METHODS.with(|m| {
            let m = m.borrow();
            classes.into_iter().filter_map(|c| m.get(&c).map(|t| (c, t.clone()))).collect()
        })
    };
    Ok(Graph { nodes, root: r, origin: isolate_id(), types, tables })
}

impl Graph {
    /// Chama `f` para a raiz e para cada aresta do grafo.
    fn visit_values(&self, mut f: impl FnMut(&GValue)) {
        f(&self.root);
        for (no, _) in &self.nodes {
            match no {
                GNode::Instance { fields, .. } => fields.iter().for_each(|(v, _)| f(v)),
                GNode::Refs { refs, .. } => refs.iter().for_each(&mut f),
                GNode::View { base, .. } => f(base),
                GNode::Compact { .. }
                | GNode::Bruto { .. }
                | GNode::Texto(_)
                | GNode::Accumulator(_)
                | GNode::ReProgram { .. }
                | GNode::Typed { .. }
                | GNode::FromRuntime { .. } => {}
            }
        }
    }

    /// Um grafo de um valor só, sem nó (as respostas do runtime): `null` ou
    /// um `Smi`.
    fn immediate(r: i64) -> Graph {
        Graph { nodes: Vec::new(), root: GValue::Word(r), origin: 0, types: None, tables: Vec::new() }
    }

    /// O grafo de `null`.
    pub fn null_val() -> Graph {
        Graph::immediate(0)
    }

    /// O grafo do `int` `v`.
    pub fn from_int(v: i64) -> Graph {
        Portable::Int(v).to_graph()
    }
}

/// Um valor que o runtime lê ou monta fora de um heap (as mensagens dos
/// serviços nativos — o `Dart_CObject` da VM): `null`, `bool`, `int`,
/// `double`, `String`, lista, `Uint8List` e, só na leitura, os campos de um
/// objeto (a `SendPort` de resposta).
#[derive(Clone, Debug)]
pub enum Portable {
    Null,
    Bool(bool),
    Int(i64),
    Double(f64),
    Str(String),
    List(Vec<Portable>),
    /// Os bytes de uma lista tipada (de qualquer tipo de elemento, como o
    /// `CObjectTypedData` da VM).
    Bytes(Vec<u8>),
    /// Uma lista tipada de elementos `tipo` (os `TIPO_*` do runtime).
    Typed(u8, Vec<u8>),
    /// Um objeto do runtime de um campo inteiro (ver [`GNode::FromRuntime`]).
    FromRuntime { cid: i32, id: i64 },
    Objeto(Vec<Portable>),
}

impl Portable {
    pub fn int(&self) -> Option<i64> {
        match self {
            Portable::Int(x) => Some(*x),
            _ => None,
        }
    }
    pub fn bool(&self) -> Option<bool> {
        match self {
            Portable::Bool(b) => Some(*b),
            _ => None,
        }
    }
    pub fn str(&self) -> Option<&str> {
        match self {
            Portable::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn bytes(&self) -> Option<&[u8]> {
        match self {
            Portable::Bytes(b) => Some(b),
            _ => None,
        }
    }
    pub fn list(&self) -> Option<&[Portable]> {
        match self {
            Portable::List(l) => Some(l),
            _ => None,
        }
    }
}

/// Um objeto estático da imagem (`GValue::Static`) lido sem heap: as caixas
/// de `bool` e os literais de string (o bloco é constante e vive o processo
/// inteiro; `layout` dá os deslocamentos).
fn portable_from_static(h: i64) -> Portable {
    use crate::layout::{DESLOCAMENTO_DO_HANDLE, cid, desl};
    let b = (h - DESLOCAMENTO_DO_HANDLE) as *const u8;
    // SAFETY: `h` é o handle de um objeto estático (conferido na cópia), um
    // bloco constante da seção da imagem com o cabeçalho e o corpo inteiros.
    #[allow(unsafe_code)]
    unsafe {
        let c = *(b as *const crate::layout::Header);
        let word = |d: usize| *(b.add(d) as *const i64);
        match c.class_id {
            cid::BOOL => Portable::Bool(word(desl::VALOR) != 0),
            cid::ONE_BYTE_STRING => {
                let n = word(desl::COMPRIMENTO) as usize;
                let u = std::slice::from_raw_parts(b.add(desl::UNIDADES), n);
                Portable::Str(u.iter().map(|&x| char::from(x)).collect())
            }
            cid::TWO_BYTE_STRING => {
                let n = word(desl::COMPRIMENTO) as usize;
                let u = std::slice::from_raw_parts(b.add(desl::UNIDADES) as *const u16, n);
                Portable::Str(String::from_utf16_lossy(u))
            }
            _ => Portable::Null,
        }
    }
}

impl Graph {
    /// O grafo lido como [`Portable`] (numa thread sem heap Dart). Um ciclo,
    /// que as mensagens dos serviços nunca têm, vira `Null` na volta.
    pub fn to_portable(&self) -> Portable {
        /// A palavra `v`; `is_ref_kind` diz se ela está numa posição `Ref` (senão,
        /// os bits de um campo escalar, lidos como `int`).
        fn val(g: &Graph, v: &GValue, is_ref_kind: bool, visiting: &mut Vec<bool>) -> Portable {
            match *v {
                GValue::Word(bits) if !is_ref_kind => Portable::Int(bits),
                GValue::Word(0) => Portable::Null,
                GValue::Word(bits) if smi::is_smi(bits) => Portable::Int(smi::value(bits)),
                GValue::Word(_) => Portable::Null,
                GValue::Static(h) => portable_from_static(h),
                GValue::Same(_) | GValue::Constant(_) | GValue::TearOff { .. } => Portable::Null,
                GValue::No(i) => {
                    if visiting[i] {
                        return Portable::Null;
                    }
                    visiting[i] = true;
                    let r = no(g, &g.nodes[i].0, visiting);
                    visiting[i] = false;
                    r
                }
            }
        }
        fn no(g: &Graph, n: &GNode, visiting: &mut Vec<bool>) -> Portable {
            use crate::layout::cid;
            use crate::listas::Element;
            match n {
                GNode::Texto(t) => Portable::Str(t.para_string()),
                GNode::Bruto { cid: cid::MINT, words, .. } => Portable::Int(words.first().copied().unwrap_or(0)),
                GNode::Bruto { cid: cid::DOUBLE, words, .. } => Portable::Double(f64::from_bits(words.first().copied().unwrap_or(0) as u64)),
                GNode::Refs { cid: c, refs, .. } if cid::is_fixed_list(*c) => Portable::List(refs.iter().map(|x| val(g, x, true, visiting)).collect()),
                GNode::Compact { shape, elements, .. } => Portable::List(
                    elements
                        .iter()
                        .map(|&x| match shape {
                            Element::Double => Portable::Double(f64::from_bits(x as u64)),
                            Element::Bool => Portable::Bool(x != 0),
                            _ => Portable::Int(x),
                        })
                        .collect(),
                ),
                GNode::Instance { cid: cid::GROWABLE_LIST, fields } => {
                    // O comprimento (campo 0, bruto) e o armazenamento (campo 1).
                    let n = match fields.first() {
                        Some((GValue::Word(n), false)) => usize::try_from(*n).unwrap_or(0),
                        _ => 0,
                    };
                    match fields.get(1).map(|(d, _)| val(g, d, true, visiting)) {
                        Some(Portable::List(mut items)) => {
                            items.truncate(n);
                            Portable::List(items)
                        }
                        _ => Portable::List(Vec::new()),
                    }
                }
                GNode::Instance { fields, .. } => Portable::Objeto(fields.iter().map(|(x, is_ref_kind)| val(g, x, *is_ref_kind, visiting)).collect()),
                GNode::Typed { bytes, .. } => Portable::Bytes(bytes.clone()),
                GNode::FromRuntime { cid, id } => Portable::FromRuntime { cid: *cid, id: *id },
                GNode::View { cid: c, base, offset, len } => {
                    let tipo = if *c >= cid::BYTE_DATA_VIEW { TYPE_UINT8 } else { ((*c - cid::PRIMEIRA_VISAO) % cid::ELEMENT_TYPES) as u8 };
                    let n = len * tamanho_do_elemento(tipo);
                    match val(g, base, true, visiting) {
                        Portable::Bytes(b) => Portable::Bytes(b[(*offset).min(b.len())..(offset + n).min(b.len())].to_vec()),
                        _ => Portable::Null,
                    }
                }
                _ => Portable::Null,
            }
        }
        let mut visiting = vec![false; self.nodes.len()];
        val(self, &self.root, true, &mut visiting)
    }
}

impl Portable {
    /// O grafo deste valor.
    pub fn to_graph(&self) -> Graph {
        fn no(n: GNode, nodes: &mut Vec<(GNode, i64)>) -> GValue {
            nodes.push((n, 0));
            GValue::No(nodes.len() - 1)
        }
        fn val(p: &Portable, nodes: &mut Vec<(GNode, i64)>) -> GValue {
            use crate::layout::{cid, flags};
            match p {
                Portable::Null | Portable::Objeto(_) => GValue::Word(0),
                Portable::Bool(b) => GValue::Static(Heap::box_bool(*b)),
                Portable::Int(x) => match smi::de(*x) {
                    Some(r) => GValue::Word(r),
                    None => no(GNode::Bruto { cid: cid::MINT, flags: flags::BRUTO, words: Box::new([*x]) }, nodes),
                },
                Portable::Double(x) => no(GNode::Bruto { cid: cid::DOUBLE, flags: flags::BRUTO, words: Box::new([x.to_bits() as i64]) }, nodes),
                Portable::Str(s) => no(GNode::Texto(Texto::de_str(s)), nodes),
                Portable::List(items) => {
                    let i = nodes.len();
                    nodes.push((GNode::Accumulator(Vec::new()), 0));
                    let refs: Vec<GValue> = items.iter().map(|x| val(x, nodes)).collect();
                    nodes[i] = (GNode::Refs { cid: cid::LIST, word0: refs.len() as i64, refs }, 0);
                    GValue::No(i)
                }
                Portable::Bytes(b) => no(GNode::Typed { tipo: TYPE_UINT8, bytes: b.clone() }, nodes),
                Portable::Typed(tipo, b) => no(GNode::Typed { tipo: *tipo, bytes: b.clone() }, nodes),
                Portable::FromRuntime { cid, id } => no(GNode::FromRuntime { cid: *cid, id: *id }, nodes),
            }
        }
        let mut nodes = Vec::new();
        let root = val(self, &mut nodes);
        Graph { nodes, root, origin: 0, types: None, tables: Vec::new() }
    }
}

/// Aloca, no heap deste isolado, o bloco vazio do nó `no` (as referências
/// ficam para depois: o grafo pode ter ciclos). `None` para a visão, que
/// precisa da base já alocada, e para os anexos (acumulador, programa de
/// `RegExp`), que o runtime cria fora do empréstimo do heap
/// ([`allocate_attachment`]).
fn allocate_node(heap: &mut Heap, no: &GNode) -> Option<i64> {
    use crate::layout::{cid, flags};
    Some(match no {
        GNode::Instance { cid: c, fields } => {
            let empties: Vec<crate::heap::Field> = fields.iter().map(|&(_, is_ref_kind)| (0, is_ref_kind)).collect();
            heap.new_object(i64::from(*c), &empties)
        }
        GNode::Refs { cid: c, word0, refs } => {
            if cid::is_fixed_list(*c) {
                heap.new_list(*c, refs.len(), crate::listas::Element::General)
            } else if *c == cid::RECORD {
                heap.new_record(&vec![0; refs.len()])
            } else {
                let h = heap.allocate(*c, 1 + refs.len(), flags::REFS);
                heap.words_mut(h)[0] = *word0;
                h
            }
        }
        GNode::Compact { cid: c, shape, elements } => {
            let h = heap.new_list(*c, elements.len(), *shape);
            // Antes de publicar: sem barreira (e são escalares).
            heap.words_mut(h)[1..=elements.len()].copy_from_slice(elements);
            h
        }
        GNode::Bruto { cid: c, flags: f, words } => {
            let h = heap.allocate(*c, words.len(), *f);
            heap.words_mut(h).copy_from_slice(words);
            h
        }
        GNode::Texto(t) => heap.allocate_string(t.vista()),
        GNode::Typed { tipo, bytes } => {
            let tipo = if *tipo == TYPE_BYTE_DATA { TYPE_UINT8 } else { *tipo };
            let h = heap.new_typed(tipo, bytes.len() / tamanho_do_elemento(tipo));
            if let Some(b) = heap.typed_bytes_mut(h) {
                let n = b.len().min(bytes.len());
                b[..n].copy_from_slice(&bytes[..n]);
            }
            h
        }
        GNode::FromRuntime { cid, id } => heap.new_object(i64::from(*cid), &[(*id, false)]),
        GNode::View { .. } | GNode::Accumulator(_) | GNode::ReProgram { .. } => return None,
    })
}

/// O bloco de um nó com anexo nativo (o acumulador do `StringBuffer`, o
/// programa do `RegExp`), criado pelo dono do anexo (`nativos_strings.rs`,
/// `regexp.rs`), que empresta o heap ele mesmo. `None` para os outros nós.
fn allocate_attachment(no: &GNode) -> Option<i64> {
    match no {
        GNode::Accumulator(u) => Some(new_accumulator_with(u.clone())),
        GNode::ReProgram { source, options } => Some(re_program_from_pattern(source, *options)),
        _ => None,
    }
}

/// Recria o grafo no heap deste isolado e devolve o valor (numa posição
/// `Ref`). Todos os nós são alocados primeiro, enraizados, e as arestas
/// ligadas depois — o grafo pode ter ciclos.
fn materialize(g: &Graph) -> i64 {
    let same = g.origin == isolate_id();
    // As tabelas de métodos das classes que este isolado ainda não viu.
    if !g.tables.is_empty() {
        METHODS.with(|m| {
            let mut m = m.borrow_mut();
            for (c, t) in &g.tables {
                m.entry(*c).or_insert_with(|| t.clone());
            }
        });
    }
    // Os tipos da mensagem no universo deste isolado, e os objetos `Type`
    // canônicos dela (fora do empréstimo do heap).
    let ids_de_tipo = g.types.as_ref().map(MessageTypes::import);
    let mut type_objects: crate::hash::HashMap<usize, i64> = crate::hash::HashMap::default();
    if let Some(ids) = &ids_de_tipo {
        for (i, (no, _)) in g.nodes.iter().enumerate() {
            if let GNode::Instance { cid, fields } = no
                && i64::from(*cid) == TYPE_CLASS
                && let Some((GValue::Word(k), _)) = fields.first()
            {
                type_objects.insert(i, dartforge_rti_objeto_tipo(ids[*k as usize]));
            }
        }
    }
    // As constantes canônicas deste isolado, pelos getters (fora do
    // empréstimo do heap: o getter é código gerado, que aloca na primeira
    // vez). São permanentes: não precisam de raiz durante a montagem.
    let mut canonical: crate::hash::HashMap<usize, i64> = crate::hash::HashMap::default();
    let mut tearoffs: crate::hash::HashMap<i64, i64> = crate::hash::HashMap::default();
    g.visit_values(|v| match *v {
        GValue::Constant(getter) => {
            canonical.entry(getter).or_insert_with(|| {
                // SAFETY: `getter` é o endereço de um getter gerado sem
                // argumentos (registrado por `dartforge_marcar_constante`),
                // o mesmo em todos os isolados do processo.
                let f = { let dart_target: usize = getter; move || -> i64 { dart_r0(dart_target) } };
                f()
            });
        }
        GValue::TearOff { code, typed_one, abi, metadata } => {
            tearoffs.entry(code).or_insert_with(|| {
                HEAP.with(|h| {
                    let mut heap = h.borrow_mut();
                    if let Some(t) = heap.registered_tearoff(code) {
                        return t;
                    }
                    let t = heap.new_closure(code, (0, false), typed_one, abi);
                    if metadata != 0 {
                        heap.set_metadata(t, i64::from(metadata));
                    }
                    heap.register_tearoff(code, t);
                    t
                })
            });
        }
        _ => {}
    });
    // Cópias proprietárias dos nós, inclusive anexos criados fora do
    // empréstimo do heap, duram até ligar as arestas e refazer os índices.
    let frame = HEAP.with(|heap| heap.borrow_mut().push_owning_frame(g.nodes.len()));
    let mut handles = vec![0i64; g.nodes.len()];
    for (i, (no, _)) in g.nodes.iter().enumerate() {
        if let Some(h) = allocate_attachment(no) {
            HEAP.with(|heap| heap.borrow_mut().set_root(frame, i, h));
            handles[i] = h;
        }
    }
    let result = HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        for (i, (no, _)) in g.nodes.iter().enumerate() {
            if handles[i] != 0 {
                continue;
            }
            let h = match type_objects.get(&i) {
                Some(&h) => h,
                None => match allocate_node(&mut heap, no) {
                    Some(h) => h,
                    None => continue,
                },
            };
            heap.set_root(frame, i, h);
            handles[i] = h;
        }
        let word = |v: &GValue, handles: &[i64]| -> i64 {
            match *v {
                GValue::Word(bits) => bits,
                GValue::No(i) => handles[i],
                GValue::Static(h) => h,
                GValue::Same(h) if same => h,
                GValue::Constant(getter) => canonical.get(&getter).copied().unwrap_or(0),
                GValue::TearOff { code, .. } => tearoffs.get(&code).copied().unwrap_or(0),
                // Fora do isolado de origem não há o que compartilhar: o
                // emissor copia (`share` falso) para outro isolado.
                GValue::Same(_) => 0,
            }
        };
        // As visões, depois das bases (uma visão de visão espera a dela).
        loop {
            let mut pending_list = false;
            let mut progress = false;
            for (i, (no, _)) in g.nodes.iter().enumerate() {
                let GNode::View { cid, base, offset, len } = no else { continue };
                if handles[i] != 0 {
                    continue;
                }
                let b = word(base, &handles);
                if b == 0 && matches!(base, GValue::No(_)) {
                    pending_list = true;
                    continue;
                }
                let h = heap.new_view(*cid, b, *offset, *len);
                heap.set_root(frame, i, h);
                handles[i] = h;
                progress = true;
            }
            if !pending_list || !progress {
                break;
            }
        }
        for (i, (no, meta)) in g.nodes.iter().enumerate() {
            if type_objects.contains_key(&i) {
                continue;
            }
            let h = handles[i];
            if h == 0 {
                continue;
            }
            match no {
                GNode::Instance { fields, .. } => {
                    for (k, (v, is_ref_kind)) in fields.iter().enumerate() {
                        let bits = word(v, &handles);
                        heap.set_field(h, k, bits, *is_ref_kind);
                    }
                }
                GNode::Refs { refs, .. } if !refs.is_empty() => {
                    let new_ones: Vec<i64> = refs.iter().map(|v| word(v, &handles)).collect();
                    heap.write_refs(h, 1, &new_ones);
                }
                _ => {}
            }
            if *meta != 0 {
                let meta = match &ids_de_tipo {
                    Some(ids) => 1 + ids[(*meta - 1) as usize],
                    None => *meta,
                };
                heap.set_metadata(h, meta);
            }
        }
        word(&g.root, &handles)
    });
    refazer_indices_copiados(g, &handles);
    HEAP.with(|heap| heap.borrow_mut().pop_frame(frame));
    result
}

/// Os `_Map`/`_Set` copiados chegam com o `_index` da origem, calculado com
/// os hashes de identidade de lá (aqui o hash é o endereço, `heap.rs`
/// `identity_hash`): como a VM (`runtime/vm/object_graph_copy.cc`,
/// `CopyLinkedHashBase` + `_rehashObjects`), o índice é refeito no destino
/// quando alguma chave pode ter outro hash. A decisão e o refazer são do
/// Dart (`_dartforgeRefazerIndiceCopiado`, `native_sdk/collection/
/// compact_hash.dart`): -1 to object que não é `_Map`/`_Set` (a classe
/// inteira fica de fora daí em diante). Os nós continuam enraizados no
/// quadro de [`materialize`].
fn refazer_indices_copiados(g: &Graph, handles: &[i64]) {
    let Some(f) = ajudante("_dartforgeRefazerIndiceCopiado") else { return };
    // SAFETY: registrado pela `dart:_compact_hash` com a assinatura
    // `(Object) -> int`.
    let redo = { let dart_target: usize = f; move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
    let mut others: crate::hash::HashSet<i32> = crate::hash::HashSet::default();
    for (i, (no, _)) in g.nodes.iter().enumerate() {
        let GNode::Instance { cid, .. } = no else { continue };
        if handles[i] == 0 || others.contains(cid) {
            continue;
        }
        if redo(handles[i]) < 0 {
            others.insert(*cid);
        }
        if dartforge_exception_pending() != 0 {
            return;
        }
    }
}

// ---------------------------------------------------------------------------
// O registro das portas e a fila do isolado.

#[cfg(test)]
mod tests_owners_materialization {
    use super::*;

    #[test]
    fn closed_queue_discards_graph_without_querying_dead_handle() {
        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
        let queue = QUEUE.with(|f| f.clone());
        let (frame, text) = HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.enable_arc();
            let frame = h.push_owning_frame(1);
            let text = h.allocate_str("não publicar");
            h.set_root(frame, 0, text);
            (frame, text)
        });
        let graph = copy_to_graph(text, true).ok().unwrap();
        close_isolate_ports();
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.pop_frame(frame);
            h.collect();
            assert!(!h.is_live_object(text));
        });
        post_to_queue(&queue, 123, graph);
        assert!(queue.messages.lock().unwrap().normal.is_empty());
        HEAP.with(|h| { h.replace(previous); });
    }

    #[test]
    fn sender_with_old_clone_does_not_publish_after_shutdown() {
        let queue = QUEUE.with(|f| f.clone());
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let free = barrier.clone();
        let dest = queue.clone();
        let sender = std::thread::spawn(move || {
            free.wait();
            post_to_queue(&dest, 123, Graph {
                nodes: Vec::new(), root: GValue::Word(0), origin: 0,
                types: None, tables: Vec::new(),
            });
        });
        close_isolate_ports();
        barrier.wait();
        sender.join().unwrap();
        let m = queue.messages.lock().unwrap();
        assert!(m.closed && m.normal.is_empty());
    }

    #[test]
    fn isolate_shutdown_releases_pending_message_owners() {
        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
        let (frame, text) = HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.enable_arc();
            let frame = h.push_owning_frame(1);
            let text = h.allocate_str("mensagem não atendida");
            h.set_root(frame, 0, text);
            (frame, text)
        });
        let port = dartforge_nativo_DartForge_porta_abrir(0);
        assert_eq!(dartforge_nativo_DartForge_porta_enviar(port, text), 0);
        assert_eq!(dartforge_nativo_DartForge_porta_enviar(port, text), 0);
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.pop_frame(frame);
            h.collect();
            assert!(h.is_live_object(text));
        });
        close_isolate_ports();
        QUEUE.with(|f| assert!(f.messages.lock().unwrap().normal.is_empty()));
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            assert!(!h.is_live_object(text));
        });
        HEAP.with(|h| { h.replace(previous); });
    }

    extern "C" fn verify_message(_: i64) -> i64 {
        let value = dartforge_nativo_DartForge_mensagem_atual();
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            let text = h.words(value)[1];
            assert_eq!(h.words(value)[2], text);
            assert_eq!(h.text(text).unwrap().para_string(), "aliases na fila");
        });
        0
    }

    #[test]
    fn shared_aliases_pass_from_queue_to_dispatch_without_residual_owner() {
        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
        let (frame, text, list, dispatcher) = HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.enable_arc();
            let frame = h.push_owning_frame(1);
            let text = h.allocate_str("aliases na fila");
            h.set_root(frame, 0, text);
            let list = h.new_list(crate::layout::cid::LIST, 2, crate::listas::Element::General);
            h.write_refs(list, 1, &[text, text]);
            h.set_root(frame, 0, list);
            let dispatcher = h.new_closure(123, (0, false), 0, 0);
            (frame, text, list, dispatcher)
        });
        dartforge_nativo_DartForge_portas_despachante(dispatcher);
        let port = dartforge_nativo_DartForge_porta_abrir(0);
        assert_eq!(dartforge_nativo_DartForge_porta_enviar(port, list), 0);
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.pop_frame(frame);
            h.collect();
            assert!(!h.is_live_object(list) && h.is_live_object(text));
        });
        assert!(dispatch_next(verify_message));
        dartforge_nativo_DartForge_porta_fechar(port);
        dartforge_nativo_DartForge_portas_despachante(0);
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            assert!(!h.is_live_object(text));
            assert!(!h.is_live_object(dispatcher));
        });
        HEAP.with(|h| { h.replace(previous); });
    }

    #[test]
    fn shared_text_in_queue_survives_without_sender_owner() {
        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
        let (frame, text) = HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.enable_arc();
            let frame = h.push_owning_frame(1);
            let text = h.allocate_str("mensagem compartilhada");
            h.set_root(frame, 0, text);
            (frame, text)
        });
        let port = dartforge_nativo_DartForge_porta_abrir(0);
        assert_eq!(dartforge_nativo_DartForge_porta_enviar(port, text), 0);
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.pop_frame(frame);
            h.collect();
            assert!(h.is_live_object(text), "a fila deve manter o owner compartilhado");
        });
        dartforge_nativo_DartForge_porta_fechar(port);
        extern "C" fn ignore(_: i64) -> i64 { 0 }
        assert!(dispatch_next(ignore));
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            assert!(!h.is_live_object(text), "descarte da mensagem deve soltar seu owner");
        });
        HEAP.with(|h| { h.replace(previous); });
    }

    #[test]
    fn cyclic_graph_with_aliases_preserves_nodes_until_edges_are_linked() {
        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
        let frame = HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.enable_arc();
            h.push_owning_frame(1)
        });
        let graph = Graph {
            nodes: vec![
                (GNode::Refs { cid: crate::layout::cid::LIST, word0: 2,
                    refs: vec![GValue::No(1), GValue::No(1)] }, 0),
                (GNode::Refs { cid: crate::layout::cid::LIST, word0: 1,
                    refs: vec![GValue::No(0)] }, 0),
            ],
            root: GValue::No(0),
            origin: isolate_id(),
            types: None,
            tables: Vec::new(),
        };
        let root = materialize(&graph);
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.set_root(frame, 0, root);
            h.collect();
            let child = h.words(root)[1];
            assert_eq!(h.words(root)[2], child);
            assert_eq!(h.words(child)[1], root);
            h.pop_frame(frame);
            h.collect();
            assert!(!h.is_live_object(root) && !h.is_live_object(child));
        });
        HEAP.with(|h| { h.replace(previous); });
    }
}

/// Uma mensagem na fila de um isolado.
struct Message {
    port: i64,
    graph: Graph,
    /// Tokens do heap destinatário para cada ocorrência `Same`.
    owners: Vec<u64>,
    arrival: std::time::Instant,
}

/// As mensagens de um isolado: as das portas do Dart e as da porta de
/// controle (as OOB da VM: `pause`, `kill`, `ping`…), que o runtime atende
/// antes das outras.
#[derive(Default)]
struct Queues {
    /// Protegido pelo mesmo mutex da publicação: clones antigos não reabrem a fila.
    closed: bool,
    normal: std::collections::VecDeque<Message>,
    control: std::collections::VecDeque<Graph>,
    /// Os pedidos para rodar no ponto seguro (a publicação de uma recarga
    /// do JIT), atendidos com o controle, entre um evento e outro.
    safepoints: std::collections::VecDeque<SafepointRequest>,
}

/// Um pedido para rodar no ponto seguro do isolado: a função recebe o dado
/// e `1` quando roda no ponto seguro, ou `0` quando o isolado terminou sem
/// atendê-lo (o pedido é descartado, e quem pediu fica sabendo).
pub type SafepointRequest = (extern "C" fn(usize, i32), usize);

/// A fila de mensagens de um isolado; qualquer thread posta nela.
pub struct IsolateQueue {
    id: u64,
    messages: std::sync::Mutex<Queues>,
    sig_num: std::sync::Condvar,
    /// A espera com prazo no macOS (ver [`Waker`]).
    waker: Waker,
    /// O endereço do pedido de interrupção da thread do isolado
    /// (`Context::interrupt`), ou 0 depois que ela terminou. Uma mensagem
    /// de controle o liga: o código gerado o lê no ponto seguro de cada volta
    /// de laço e chama [`dartforge_ponto_seguro`], que atende o controle sem
    /// esperar o próximo evento — um `kill` imediato ou um `ping` alcançam
    /// um isolado preso num laço sem eventos, como a verificação de pilha da
    /// VM. O mutex garante que ninguém grava depois que a thread o zerou.
    interrupt: std::sync::Mutex<usize>,
}

impl IsolateQueue {
    /// Acorda a thread do isolado: o `Condvar` e, no macOS, o despertador.
    fn notify(&self) {
        self.sig_num.notify_one();
        self.waker.wake();
    }
}

/// A espera com prazo do laço de eventos no macOS: um `kqueue` com um
/// `EVFILT_USER` que o aviso aciona e o prazo no `timeout` do `kevent`, como
/// o manipulador de eventos da VM (`eventhandler_macos.cc`, `Poll`). O
/// `pthread_cond_timedwait` do `Condvar` passa do prazo pela folga de
/// coalescência do kernel: 200 timers de 5 ms levavam 5,9 s contra 1,5 s na
/// VM (a sonda `tools/sondas/precisao_de_timers.dart`), e o
/// `corpus/native/33_mensagens_de_controle` estourava o tempo. Fora do macOS
/// o `Condvar` já é preciso e o despertador não faz nada.
struct Waker {
    #[cfg(target_vendor = "apple")]
    kq: i32,
}

#[cfg(target_vendor = "apple")]
#[repr(C)]
struct EventoDoDespertador {
    ident: usize,
    filter_obj: i16,
    flag_bits: u16,
    fd_flags: u32,
    data: isize,
    udata: *mut std::ffi::c_void,
}

#[cfg(target_vendor = "apple")]
#[repr(C)]
struct PrazoDoDespertador {
    seconds: i64,
    nanos: i64,
}

#[cfg(target_vendor = "apple")]
unsafe extern "C" {
    #[link_name = "kqueue"]
    fn kqueue_do_despertador() -> i32;
    #[link_name = "kevent"]
    fn kevent_do_despertador(
        kq: i32,
        changes: *const EventoDoDespertador,
        nm: i32,
        events: *mut EventoDoDespertador,
        ne: i32,
        wait_entry: *const PrazoDoDespertador,
    ) -> i32;
    #[link_name = "close"]
    fn close_do_despertador(fd: i32) -> i32;
}

#[cfg(target_vendor = "apple")]
impl Waker {
    const ACTIVE: bool = true;
    const EVFILT_USER: i16 = -10;
    const EV_ADD: u16 = 0x1;
    const EV_CLEAR: u16 = 0x20;
    const NOTE_TRIGGER: u32 = 0x0100_0000;

    fn event(flag_bits: u16, fd_flags: u32) -> EventoDoDespertador {
        EventoDoDespertador { ident: 1, filter_obj: Self::EVFILT_USER, flag_bits, fd_flags, data: 0, udata: std::ptr::null_mut() }
    }

    fn new() -> Waker {
        // SAFETY: cria o kqueue; o evento vive durante a chamada.
        let kq = unsafe { kqueue_do_despertador() };
        if kq < 0 {
            panic!("falha ao criar o kqueue do laço de eventos");
        }
        let ev = Self::event(Self::EV_ADD | Self::EV_CLEAR, 0);
        // SAFETY: a mudança vive durante a chamada.
        if unsafe { kevent_do_despertador(kq, &ev, 1, std::ptr::null_mut(), 0, std::ptr::null()) } == -1 {
            panic!("falha ao registrar o despertador do laço de eventos");
        }
        Waker { kq }
    }

    fn wake(&self) {
        let ev = Self::event(0, Self::NOTE_TRIGGER);
        // SAFETY: a mudança vive durante a chamada; o kqueue é deste isolado.
        unsafe { kevent_do_despertador(self.kq, &ev, 1, std::ptr::null_mut(), 0, std::ptr::null()) };
    }

    fn wait_for(&self, deadline: std::time::Duration) {
        let ts = PrazoDoDespertador { seconds: deadline.as_secs() as i64, nanos: i64::from(deadline.subsec_nanos()) };
        let mut ev = Self::event(0, 0);
        // SAFETY: um evento de saída; o prazo vive durante a chamada.
        unsafe { kevent_do_despertador(self.kq, std::ptr::null(), 0, &mut ev, 1, &ts) };
    }
}

#[cfg(target_vendor = "apple")]
impl Drop for Waker {
    fn drop(&mut self) {
        // SAFETY: o kqueue é deste despertador.
        unsafe { close_do_despertador(self.kq) };
    }
}

#[cfg(not(target_vendor = "apple"))]
impl Waker {
    const ACTIVE: bool = false;
    fn new() -> Waker {
        Waker {}
    }
    fn wake(&self) {}
    fn wait_for(&self, _deadline: std::time::Duration) {}
}

/// Zera o endereço do pedido de interrupção quando a thread termina (antes
/// de a memória dos `thread_local` dela ser liberada).
struct InterruptGuard(std::sync::Arc<IsolateQueue>);

impl Drop for InterruptGuard {
    fn drop(&mut self) {
        *self.0.interrupt.lock().unwrap_or_else(|e| e.into_inner()) = 0;
    }
}

/// Um serviço nativo: recebe (porta, grafo) na thread de quem enviou.
pub type NativeService = std::sync::Arc<dyn Fn(i64, Graph) + Send + Sync>;

enum Owner {
    Isolate(std::sync::Arc<IsolateQueue>),
    /// A porta de controle de um isolado (`Isolate.controlPort`).
    Control(std::sync::Arc<IsolateQueue>),
    Native(NativeService),
}

fn rec() -> &'static std::sync::Mutex<crate::hash::HashMap<i64, Owner>> {
    static R: std::sync::OnceLock<std::sync::Mutex<crate::hash::HashMap<i64, Owner>>> = std::sync::OnceLock::new();
    R.get_or_init(Default::default)
}

fn next_port_id() -> i64 {
    static NEXT_ID: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);
    NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

thread_local! {
    static QUEUE: std::sync::Arc<IsolateQueue> = {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let target = crate::heap::CONTEXTO.with(|c| &c.interrupt as *const std::sync::atomic::AtomicU8 as usize);
        let queue = std::sync::Arc::new(IsolateQueue {
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            messages: std::sync::Mutex::new(Queues::default()),
            sig_num: std::sync::Condvar::new(),
            waker: Waker::new(),
            interrupt: std::sync::Mutex::new(target),
        });
        let guard = InterruptGuard(queue.clone());
        INTERRUPT_GUARD.with(|g| *g.borrow_mut() = Some(guard));
        queue
    };
    /// Zera o endereço do pedido de interrupção no fim da thread.
    static INTERRUPT_GUARD: RefCell<Option<InterruptGuard>> = const { RefCell::new(None) };
    /// Portas abertas deste isolado → se mantêm o isolado vivo.
    static OPEN_PORTS: RefCell<crate::hash::HashMap<i64, bool>> = RefCell::new(crate::hash::HashMap::default());
    /// A mensagem em despacho: (porta, valor), o valor enraizado.
    static CURRENT: RefCell<(i64, i64)> = const { RefCell::new((0, 0)) };
    /// O despachante Dart (`_RawReceivePort._despachar`), enraizado.
    static DISPATCHER: RefCell<i64> = const { RefCell::new(0) };
}

/// Raízes globais do runtime para as portas (ids negativos fora da faixa
/// dos timers de `events.rs`).
const DISPATCHER_ROOT: i64 = -0x7000_0000_0000;
const MESSAGE_ROOT: i64 = -0x7000_0000_0001;

fn isolate_id() -> u64 {
    QUEUE.with(|f| f.id)
}

/// Posta `graph` para a porta `port`: na fila do isolado dono, ou no
/// serviço nativo. Porta fechada ou inexistente: a mensagem é descartada,
/// como na VM.
pub fn post(port: i64, graph: Graph) {
    let owner = {
        let r = rec().lock().unwrap_or_else(|e| e.into_inner());
        match r.get(&port) {
            Some(Owner::Isolate(f)) => Some(Owner::Isolate(f.clone())),
            Some(Owner::Control(f)) => Some(Owner::Control(f.clone())),
            Some(Owner::Native(s)) => Some(Owner::Native(s.clone())),
            None => None,
        }
    };
    match owner {
        Some(Owner::Isolate(f)) => {
            post_to_queue(&f, port, graph);
        }
        Some(Owner::Control(f)) => {
            let mut m = f.messages.lock().unwrap_or_else(|e| e.into_inner());
            if m.closed { return; }
            m.control.push_back(graph);
            f.notify();
        }
        Some(Owner::Native(s)) => s(port, graph),
        None => {}
    }
}

/// Publica com a referência já obtida do registro; o mutex lineariza o fechamento.
fn post_to_queue(f: &IsolateQueue, port: i64, graph: Graph) {
    let mut m = f.messages.lock().unwrap_or_else(|e| e.into_inner());
    if m.closed { return; }
    // Retenção não coleta nem chama Dart; o fechamento usa este mesmo mutex.
    let mut owners = Vec::new();
    graph.visit_values(|v| {
        if let GValue::Same(h) = *v {
            assert_eq!(f.id, isolate_id(), "owner compartilhado fora do seu isolado");
            assert_eq!(graph.origin, f.id, "origem do owner compartilhado incorreta");
            owners.push(HEAP.with(|heap| heap.borrow_mut().retain_message_owner(h)));
        }
    });
    m.normal.push_back(Message { port, graph, owners, arrival: std::time::Instant::now() });
    f.notify();
}

/// Posta `graph` na fila de controle do isolado dono de `port` (as
/// mensagens OOB: `Isolate_sendOOB` vale para qualquer porta do isolado).
fn post_control(port: i64, graph: Graph) {
    let queue = {
        let r = rec().lock().unwrap_or_else(|e| e.into_inner());
        match r.get(&port) {
            Some(Owner::Isolate(f) | Owner::Control(f)) => Some(f.clone()),
            _ => None,
        }
    };
    if let Some(f) = queue {
        {
            let mut m = f.messages.lock().unwrap_or_else(|e| e.into_inner());
            if m.closed { return; }
            m.control.push_back(graph);
        }
        let target = f.interrupt.lock().unwrap_or_else(|e| e.into_inner());
        if *target != 0 {
            // SAFETY: o contexto da thread do isolado, vivo enquanto o
            // endereço não foi zerado (com este mutex) pela guarda dela.
            unsafe { (*(*target as *const std::sync::atomic::AtomicU8)).store(1, std::sync::atomic::Ordering::Release) };
        }
        drop(target);
        f.notify();
    }
}

/// Apaga o pedido de interrupção deste isolado (quem o atende vai esvaziar a
/// fila de controle em seguida; uma mensagem que chegar depois liga de novo).
fn clear_interrupt_request() {
    crate::heap::CONTEXTO.with(|c| c.interrupt.store(0, std::sync::atomic::Ordering::Relaxed));
}

// ---------------------------------------------------------------------------
// O ponto seguro do isolado principal (a recarga do JIT).
//
// A publicação de uma geração nova (a troca das células das entradas
// estáveis, o registro das tabelas) só pode acontecer quando nenhum quadro
// Dart do isolado está na pilha: entre dois eventos do laço — o ponto em que
// a VM também comita uma recarga (`isolate_reload.cc`). O observador do
// `dartforge reload` compila numa thread própria e entrega a publicação como
// um pedido na fila do isolado principal; o laço o atende antes do próximo
// evento, e um isolado ocioso (um servidor esperando conexão) acorda para ele.

fn main_isolate() -> &'static std::sync::Mutex<Option<std::sync::Arc<IsolateQueue>>> {
    static P: std::sync::OnceLock<std::sync::Mutex<Option<std::sync::Arc<IsolateQueue>>>> = std::sync::OnceLock::new();
    P.get_or_init(Default::default)
}

/// Este isolado é o principal: passa a aceitar pedidos no ponto seguro.
pub fn mark_main_isolate() {
    let queue = QUEUE.with(|f| f.clone());
    *main_isolate().lock().unwrap_or_else(|e| e.into_inner()) = Some(queue);
}

/// O isolado principal terminou: os pedidos pendentes são descartados (quem
/// pediu recebe `0`) e os novos, recusados.
pub fn unmark_main_isolate() {
    remove_live_isolate();
    let Some(queue) = main_isolate().lock().unwrap_or_else(|e| e.into_inner()).take() else {
        return;
    };
    let pending_list = std::mem::take(&mut queue.messages.lock().unwrap_or_else(|e| e.into_inner()).safepoints);
    for (f, datum) in pending_list {
        f(datum, 0);
    }
}

/// Pede que `f(datum, 1)` rode no próximo ponto seguro do isolado principal,
/// na thread dele. Devolve `0` (e não chama `f`) se não há isolado principal
/// rodando o laço de eventos; `1` se o pedido foi aceito — então `f` é
/// chamada exatamente uma vez, com `1` no ponto seguro ou com `0` se o
/// isolado terminar antes.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_pedir_no_ponto_seguro(f: extern "C" fn(usize, i32), datum: usize) -> i32 {
    // O registro fica travado durante a inserção: o descarte do fim do
    // isolado não perde um pedido que chegue ao mesmo tempo.
    let main_queue = main_isolate().lock().unwrap_or_else(|e| e.into_inner());
    let Some(queue) = main_queue.as_ref() else {
        return 0;
    };
    queue.messages.lock().unwrap_or_else(|e| e.into_inner()).safepoints.push_back((f, datum));
    queue.notify();
    1
}

// A parada de todos os isolados (a publicação de uma recarga): o código
// novo só pode entrar com nenhum quadro Dart na pilha de NENHUM isolado,
// porque as células das entradas estáveis são do processo. O isolado
// principal, no ponto seguro dele, pede aos demais que parem no próximo
// ponto seguro deles (`dartforge_parar_isolados`), troca o código, e os
// libera (`dartforge_liberar_isolados`); cada um refaz os próprios registros
// da geração nova (o estado do runtime é por isolado) antes de continuar.

/// Os isolados vivos no laço de eventos (os que atendem pedidos).
fn live_isolates() -> &'static std::sync::Mutex<Vec<std::sync::Arc<IsolateQueue>>> {
    static V: std::sync::OnceLock<std::sync::Mutex<Vec<std::sync::Arc<IsolateQueue>>>> = std::sync::OnceLock::new();
    V.get_or_init(Default::default)
}

thread_local! {
    static LIVE_REGISTERED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Este isolado entrou no laço: passa a ser parado nas publicações.
fn register_live_isolate() {
    if LIVE_REGISTERED.with(|r| r.replace(true)) {
        return;
    }
    let queue = QUEUE.with(|f| f.clone());
    live_isolates().lock().unwrap_or_else(|e| e.into_inner()).push(queue);
}

/// Este isolado terminou: sai do registro, e os pedidos pendentes dele são
/// respondidos com `0` (sob o mesmo trinco: nenhum pedido chega depois).
fn remove_live_isolate() {
    if !LIVE_REGISTERED.with(|r| r.replace(false)) {
        return;
    }
    let mine = isolate_id();
    let pending_list = {
        let mut live_set = live_isolates().lock().unwrap_or_else(|e| e.into_inner());
        live_set.retain(|f| f.id != mine);
        QUEUE.with(|f| std::mem::take(&mut f.messages.lock().unwrap_or_else(|e| e.into_inner()).safepoints))
    };
    for (f, datum) in pending_list {
        f(datum, 0);
    }
}

/// A barreira de uma parada: quantos pararam, e os registros da geração nova
/// quando o principal libera.
#[derive(Default)]
struct WriteBarrier {
    estado: std::sync::Mutex<(usize, Option<[usize; 3]>)>,
    sig_num: std::sync::Condvar,
}

/// O pedido de parada, num isolado que não é o principal.
extern "C" fn parar_neste_isolado(datum: usize, execute_fn: i32) {
    // SAFETY: `datum` é um `Arc<WriteBarrier>` entregue por `dartforge_parar_isolados`
    // (uma referência por pedido, devolvida aqui).
    let barrier = unsafe { std::sync::Arc::from_raw(datum as *const WriteBarrier) };
    let mut estado = barrier.estado.lock().unwrap_or_else(|e| e.into_inner());
    estado.0 += 1;
    barrier.sig_num.notify_all();
    if execute_fn == 0 {
        return;
    }
    while estado.1.is_none() {
        estado = barrier.sig_num.wait(estado).unwrap_or_else(|e| e.into_inner());
    }
    let [area, register, rti] = estado.1.expect("liberado");
    drop(estado);
    let as_fn = |p: usize| {
        // SAFETY: 0 ou um trampolim `void ()` da geração publicada.
        (p != 0).then(|| unsafe { std::mem::transmute::<usize, extern "C" fn()>(p) })
    };
    dartforge_publicar_geracao(as_fn(area), as_fn(register), as_fn(rti));
}

/// Para todos os isolados vivos além deste no ponto seguro deles e devolve a
/// parada, que [`dartforge_liberar_isolados`] encerra. Chamada no ponto
/// seguro do isolado principal.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_parar_isolados() -> usize {
    let mine = isolate_id();
    let barrier = std::sync::Arc::new(WriteBarrier::default());
    let mut n = 0;
    {
        let live_set = live_isolates().lock().unwrap_or_else(|e| e.into_inner());
        for f in live_set.iter().filter(|f| f.id != mine) {
            let datum = std::sync::Arc::into_raw(barrier.clone()) as usize;
            f.messages.lock().unwrap_or_else(|e| e.into_inner()).safepoints.push_back((parar_neste_isolado, datum));
            f.notify();
            n += 1;
        }
    }
    let mut estado = barrier.estado.lock().unwrap_or_else(|e| e.into_inner());
    while estado.0 < n {
        estado = barrier.sig_num.wait(estado).unwrap_or_else(|e| e.into_inner());
    }
    drop(estado);
    std::sync::Arc::into_raw(barrier) as usize
}

/// Libera os isolados da parada `stopped_flag`: cada um refaz os registros da
/// geração nova (`area`, `register`, `rti`, trampolins `void ()` ou 0) na
/// thread dele e continua.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_liberar_isolados(stopped_flag: usize, area: usize, register: usize, rti: usize) {
    // SAFETY: `stopped_flag` veio de `dartforge_parar_isolados`, uma vez.
    let barrier = unsafe { std::sync::Arc::from_raw(stopped_flag as *const WriteBarrier) };
    barrier.estado.lock().unwrap_or_else(|e| e.into_inner()).1 = Some([area, register, rti]);
    barrier.sig_num.notify_all();
}

/// Atende os pedidos no ponto seguro deste isolado (o laço de eventos).
fn serve_safepoints() {
    loop {
        let request = QUEUE.with(|f| f.messages.lock().unwrap_or_else(|e| e.into_inner()).safepoints.pop_front());
        let Some((f, datum)) = request else { return };
        f(datum, 1);
    }
}

/// A próxima mensagem da porta de controle deste isolado.
fn next_control() -> Option<Graph> {
    clear_interrupt_request();
    QUEUE.with(|f| f.messages.lock().unwrap_or_else(|e| e.into_inner()).control.pop_front())
}

/// Abre a porta de controle deste isolado e devolve o id.
fn open_control_port() -> i64 {
    let id = next_port_id();
    let queue = QUEUE.with(|f| f.clone());
    rec().lock().unwrap_or_else(|e| e.into_inner()).insert(id, Owner::Control(queue));
    id
}

/// Fecha todas as portas deste isolado (ele terminou): as mensagens que
/// chegarem depois são descartadas, como na VM.
fn close_isolate_ports() {
    let mine = isolate_id();
    rec().lock().unwrap_or_else(|e| e.into_inner()).retain(|_, d| match d {
        Owner::Isolate(f) | Owner::Control(f) => f.id != mine,
        Owner::Native(_) => true,
    });
    OPEN_PORTS.with(|p| p.borrow_mut().clear());
    let pending_list = QUEUE.with(|f| {
        let mut m = f.messages.lock().unwrap_or_else(|e| e.into_inner());
        m.closed = true;
        m.control.clear();
        std::mem::take(&mut m.normal)
    });
    for m in pending_list { release_message_owners(&m.owners); }
}

/// Fecha uma porta nativa: as mensagens seguintes são descartadas.
pub fn close_native_port(id: i64) {
    rec().lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
}

/// Abre uma porta nativa atendida por `service` e devolve o id.
pub fn open_native_port(service: NativeService) -> i64 {
    let id = next_port_id();
    rec().lock().unwrap_or_else(|e| e.into_inner()).insert(id, Owner::Native(service));
    id
}

thread_local! {
    /// `NativeCallable.isolateLocal` abertos com `keepIsolateAlive`
    /// (`ffi_callbacks.rs`): cada um mantém o isolado vivo como uma porta.
    static CALLBACKS_KEEPING_ALIVE: std::cell::Cell<i64> = const { std::cell::Cell::new(0) };
}

/// Soma `delta` aos callbacks locais que mantêm o isolado vivo.
pub fn adjust_callbacks_keeping_alive(delta: i64) {
    CALLBACKS_KEEPING_ALIVE.with(|c| c.set((c.get() + delta).max(0)));
}

/// Se o isolado tem porta aberta (ou callback local) que o mantém vivo.
fn has_live_port() -> bool {
    CALLBACKS_KEEPING_ALIVE.with(|c| c.get() > 0) || OPEN_PORTS.with(|p| p.borrow().values().any(|&v| v))
}

/// A chegada da mensagem mais antiga da fila, se houver.
fn next_arrival() -> Option<std::time::Instant> {
    QUEUE.with(|f| f.messages.lock().unwrap_or_else(|e| e.into_inner()).normal.front().map(|m| m.arrival))
}

/// Espera uma mensagem até `deadline` (ou sem prazo). Devolve se chegou. Com
/// `control_only` (o isolado pausado), só a de controle acorda.
fn wait_message(deadline: Option<std::time::Instant>, control_only: bool) -> bool {
    QUEUE.with(|f| {
        let mut m = f.messages.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if !m.control.is_empty() || !m.safepoints.is_empty() || (!control_only && !m.normal.is_empty()) {
                return true;
            }
            match deadline {
                None => m = f.sig_num.wait(m).unwrap_or_else(|e| e.into_inner()),
                Some(p) => {
                    let current_time = std::time::Instant::now();
                    if current_time >= p {
                        return false;
                    }
                    if Waker::ACTIVE {
                        // O `kevent` com prazo (ver [`Waker`]): o aviso
                        // que chega entre soltar o mutex e esperar fica
                        // pendente no `EVFILT_USER` e acorda na hora.
                        drop(m);
                        f.waker.wait_for(p - current_time);
                        m = f.messages.lock().unwrap_or_else(|e| e.into_inner());
                    } else {
                        m = f.sig_num.wait_timeout(m, p - current_time).unwrap_or_else(|e| e.into_inner()).0;
                    }
                }
            }
        }
    })
}

/// Solta tokens exclusivamente no heap que os criou (o destinatário).
fn release_message_owners(owners: &[u64]) {
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        for &id in owners { h.release_message_owner(id); }
    });
}

/// Tira a próxima mensagem, materializa e despacha pelo `call` do código
/// gerado. Devolve se havia mensagem.
fn dispatch_next(call: extern "C" fn(i64) -> i64) -> bool {
    let Some(m) = QUEUE.with(|f| f.messages.lock().unwrap_or_else(|e| e.into_inner()).normal.pop_front()) else {
        return false;
    };
    // Mensagem para porta já fechada: descartada, como na VM.
    if !OPEN_PORTS.with(|p| p.borrow().contains_key(&m.port)) {
        release_message_owners(&m.owners);
        return true;
    }
    let dispatcher = DISPATCHER.with(|d| *d.borrow());
    if dispatcher == 0 {
        release_message_owners(&m.owners);
        return true;
    }
    let value = materialize(&m.graph);
    HEAP.with(|h| h.borrow_mut().set_global_root(MESSAGE_ROOT, value));
    release_message_owners(&m.owners);
    CURRENT.with(|a| *a.borrow_mut() = (m.port, value));
    dart_r1(call as usize, dispatcher);
    CURRENT.with(|a| *a.borrow_mut() = (0, 0));
    HEAP.with(|h| h.borrow_mut().set_global_root(MESSAGE_ROOT, 0));
    true
}

// ---------------------------------------------------------------------------
// Natives da sobreposição (`native_sdk/isolate/isolate_patch.dart`).

/// `DartForge_porta_abrir(port)`: abre uma porta deste isolado para o
/// objeto `port` (um `_RawReceivePort`, cuja classe passa a ser não
/// enviável) e devolve o id.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_porta_abrir(object: i64) -> i64 {
    let class = HEAP.with(|h| h.borrow().object(object).map(|o| o.class_id));
    if let Some(c) = class {
        NOT_SENDABLE.with(|n| {
            n.borrow_mut().entry(c).or_insert_with(|| "(object is a ReceivePort)\n".to_string());
        });
    }
    let id = next_port_id();
    let queue = QUEUE.with(|f| f.clone());
    rec().lock().unwrap_or_else(|e| e.into_inner()).insert(id, Owner::Isolate(queue));
    OPEN_PORTS.with(|p| p.borrow_mut().insert(id, true));
    id
}

/// `DartForge_porta_fechar(id)`: fecha a porta; mensagens pendentes para
/// ela são descartadas no despacho.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_porta_fechar(id: i64) {
    rec().lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
    OPEN_PORTS.with(|p| p.borrow_mut().remove(&id));
}

/// `DartForge_porta_manter_vivo(id, keep)`: `keepIsolateAlive`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_porta_manter_vivo(id: i64, keep: u8) {
    OPEN_PORTS.with(|p| {
        if let Some(v) = p.borrow_mut().get_mut(&id) {
            *v = keep != 0;
        }
    });
}

/// `DartForge_porta_mantem_vivo(id)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_porta_mantem_vivo(id: i64) -> u8 {
    OPEN_PORTS.with(|p| u8::from(p.borrow().get(&id).copied().unwrap_or(false)))
}

/// `DartForge_porta_enviar(id, message)`: copia e posta. Devolve 1 se a
/// mensagem tem objeto não enviável (a descrição fica em
/// `DartForge_porta_recusa`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_porta_enviar(id: i64, message: i64) -> u8 {
    let same = {
        let r = rec().lock().unwrap_or_else(|e| e.into_inner());
        let queue = QUEUE.with(|f| f.id);
        matches!(r.get(&id), Some(Owner::Isolate(f)) if f.id == queue)
    };
    match copy_to_graph(message, same) {
        Ok(g) => {
            post(id, g);
            0
        }
        Err(IllegalMessage(d)) => {
            LAST_REJECTION.with(|u| *u.borrow_mut() = d);
            1
        }
    }
}

/// `DartForge_porta_recusa()`: por que a última mensagem foi recusada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_porta_recusa() -> i64 {
    let d = LAST_REJECTION.with(|u| u.borrow().clone());
    HEAP.with(|h| h.borrow_mut().allocate_str(&d))
}

/// `DartForge_portas_despachante(f)`: a closure sem argumentos que o laço
/// chama para cada mensagem (`_RawReceivePort._despachar`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_portas_despachante(f: i64) {
    HEAP.with(|h| h.borrow_mut().set_global_root(DISPATCHER_ROOT, f));
    DISPATCHER.with(|d| *d.borrow_mut() = f);
}

/// `DartForge_porta_atual()`: a porta da mensagem em despacho.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_porta_atual() -> i64 {
    CURRENT.with(|a| a.borrow().0)
}

/// `DartForge_mensagem_atual()`: a mensagem em despacho.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_mensagem_atual() -> i64 {
    CURRENT.with(|a| a.borrow().1)
}

/// `DartForge_classe_nao_enviavel(object)`: a classe de `object` não vai
/// numa mensagem (`@pragma('vm:isolate-unsendable')` da VM). A descrição é a
/// da VM sem o sufixo de biblioteca privada e sem o caminho de retenção.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_classe_nao_enviavel(object: i64) {
    let class = HEAP.with(|h| h.borrow().object(object).map(|o| o.class_id));
    let Some(c) = class else { return };
    let name = class_name(c);
    NOT_SENDABLE.with(|n| {
        n.borrow_mut().entry(c).or_insert_with(|| {
            format!(
                "object is unsendable - Library:'dart:isolate' Class: {name} (see restrictions listed at `SendPort.send()` documentation for more information)\n"
            )
        });
    });
}

/// `DartForge_classe_transferivel(object)`: a classe de `object` tem o
/// primeiro campo movido numa mensagem (`TransferableTypedData`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_classe_transferivel(object: i64) {
    let class = HEAP.with(|h| h.borrow().object(object).map(|o| o.class_id));
    if let Some(c) = class
        && !transferable_classes().contains(&c)
    {
        transferable().write().unwrap_or_else(|e| e.into_inner()).insert(c);
    }
}

/// `DartForge_capacidade_nova()`: um id de capacidade único no processo.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_capacidade_nova() -> i64 {
    static NEXT_ID: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);
    NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// O nome registrado da classe `c` (vazio se não houver).
fn class_name(c: i64) -> String {
    CLASS_NAMES.with(|m| m.borrow().get(&c).cloned()).unwrap_or_default()
}
