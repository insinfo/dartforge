// Native runtime: ports (`RawReceivePort`/`SendPort`) and the isolate's
// message queue — the VM's `PortMap`/`MessageHandler` (`runtime/vm/port.cc`,
// `message_handler.cc`).
//
// * A port is a global id (never 0). The global registry says who it
//   belongs to: an isolate's queue (a thread) or a runtime service
//   (the native ports, like the IOService one), which receives the message on
//   the sender's thread.
// * Sending copies the message into a portable graph (`Graph`), which the
//   destination materializes in its own heap. This is the VM semantics
//   (`object_graph_copy.cc`): what is mutable is copied; what is
//   deeply immutable and permanent goes by identity — the image's static
//   objects (the `bool` boxes, the AOT string literals)
//   in any isolate of the process; constants, enums, top-level tear-offs and
//   heap strings when the message stays in the same isolate.
// * The graph copies blocks of the object space by body format
//   (docs/NATIVO-ESPACO-UNIFICADO.md §2.12): `INSTANCIA` field by field with the
//   reference bit; `REFS` by word 0 and the references; `BRUTO` raw,
//   except what points into itself or outside the heap (typed list,
//   view, attachment), which the destination rebuilds.
// * The isolate's queue keeps (port, graph, arrival). The event loop
//   (`events.rs`) serves timers and messages in arrival order — in the VM the
//   timer is also a message —, and drains the microtasks after
//   each one.
// * Dispatch calls the overlay's `_RawReceivePort._despachar` (the
//   VM's `_handleMessage`), which reads the current port and message.
// * The isolate lives while it has an open port with `keepIsolateAlive`.

/// A word inside a portable graph (a reference or the bits of a
/// scalar field, at the destination).
#[derive(Clone, Copy, Debug)]
enum GValue {
    /// A word that does not point to the heap: `null`, a `Smi` in a `Ref`
    /// position, or the bits of a scalar field.
    Word(i64),
    /// Node `i` of the graph.
    No(usize),
    /// A static object of the image (`PERMANENTE`): the same address in every
    /// isolate of the process, never collected nor written.
    Static(i64),
    /// A permanent object of the source heap, passed by identity (only
    /// when source and destination are the same isolate).
    Same(i64),
    /// A canonical constant, by the generated getter that produces it: the destination
    /// receives its own canonical instance (the VM's isolate group
    /// shares constants; here each isolate has its own, and `identical`
    /// still holds between them).
    Constant(usize),
    /// The canonical tear-off of a top-level function, by code, with the typed
    /// body, the ABI and the RTI metadata (the function type) of the source
    /// closure: a destination that does not have the tear-off yet creates it identically — before,
    /// it was born without a type (`runtimeType` `Function`) and the `as Stream
    /// Function(Stream, dynamic)` of `StreamIsolate` in new_sali/backend
    /// failed in the new isolate (docs/NATIVO-PROJETOS-REAIS.md, C11). Type
    /// ids belong to the process (`types.rs`).
    TearOff { code: i64, typed_one: i64, abi: i64, metadata: u32 },
}

/// A graph node: a block of the object space copied by format, with the
/// references replaced by `GValue`.
#[derive(Clone, Debug)]
enum GNode {
    /// An `INSTANCIA` object (of the program, or of the runtime: `_GrowableList`,
    /// `_Closure`, `_Contexto`, `_Celula`, `_SendPort`…): the class and the fields,
    /// with the reference bit of each.
    Instance { cid: i32, fields: Vec<(GValue, bool)> },
    /// A `REFS` body (general `_List`/`_ImmutableList`, `_Record`): word 0
    /// (the length or the shape) and the references.
    Refs { cid: i32, word0: i64, refs: Vec<GValue> },
    /// A compact `_List`/`_ImmutableList` (`BRUTO` with `ELEMENTO`): the
    /// raw elements (`i64`, `f64` bits or 0/1).
    Compact { cid: i32, shape: crate::lists::Element, elements: Vec<i64> },
    /// A `BRUTO` body with no pointer into itself nor outside the heap
    /// (`_Mint`, `_Double`, SIMD): the words and the `flags`, copied raw.
    Bruto { cid: i32, flags: u8, words: Box<[i64]> },
    /// A string (the canonical form is rebuilt at the destination).
    Texto(Texto),
    /// The accumulator of a `StringBuffer` (`_AcumuladorDeTexto`, `ATTACHMENT`): the
    /// units, which live outside the heap.
    Accumulator(Vec<u16>),
    /// The program of a `RegExp` (`_ProgramaDeRegExp`, `ATTACHMENT`): the pattern and the
    /// options, recompiled at the destination (`regexp.rs`).
    ReProgram { source: Vec<u16>, options: [bool; 4] },
    /// An internal or external (`EXTERNAL`) typed list: the bytes, copied
    /// into an internal one at the destination (the VM copies the external one to new memory,
    /// `object_graph_copy.cc`).
    Typed { tipo: u8, bytes: Vec<u8> },
    /// A view: the base (copied) and the offset; the data address is
    /// recomputed at the destination.
    View { cid: i32, base: GValue, offset: usize, len: usize },
    /// A runtime object with one integer field coming from outside the heap
    /// (`SendPort`, `Capability` from C): the class is the one at position `pos` of the
    /// fixed cids (`layout::cid`, `_SendPort`/`_Capability`).
    FromRuntime { cid: i32, id: i64 },
}

/// A copied message: the nodes (with the RTI metadata of each) and the root.
#[derive(Clone)]
pub struct Graph {
    nodes: Vec<(GNode, i64)>,
    root: GValue,
    /// The source isolate (queue id): `GValue::Same` is only valid there.
    origin: u64,
    /// In a message to another isolate, the types (RTI) of the nodes: the
    /// metadata of each node (`1 + type`) is then `1 + index` in this table
    /// (0 = no type), because type ids are per isolate.
    types: Option<MessageTypes>,
    /// In a message to another isolate, the method table of each class
    /// of the objects (registered in the isolate when the first object of the
    /// class is created; an object that arrives in a message does not go through there).
    tables: Vec<(i64, MethodTable)>,
}

/// Why a message cannot be sent.
pub struct IllegalMessage(pub String);

/// The word `h` (in a `Ref` position) in the graph: immediate, static, by
/// identity, canonical, or the object's node (reserved now and filled
/// when `h` leaves the stack).
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
    // A native attachment the graph does not know how to copy (only the
    // `StringBuffer` accumulator and the `RegExp` program have a copy) arrives as `null`.
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
    // Reserves the node; the content is filled when `h` leaves the stack.
    nodes.push((GNode::Accumulator(Vec::new()), 0));
    stack.push(h);
    GValue::No(i)
}

/// Classes whose first field is MOVED in a message (the
/// `TransferableTypedData`): the source object keeps `null`. Class
/// ids belong to the program, so the registry holds for all isolates.
fn transferable_classes() -> std::sync::RwLockReadGuard<'static, crate::hash::HashSet<i64>> {
    transferable().read().unwrap_or_else(|e| e.into_inner())
}

fn transferable() -> &'static std::sync::RwLock<crate::hash::HashSet<i64>> {
    static T: std::sync::OnceLock<std::sync::RwLock<crate::hash::HashSet<i64>>> = std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

thread_local! {
    /// Classes whose instances cannot go in a message (the receive
    /// ports; `DartForge_porta_abrir` registers the class).
    /// Class → the description the refusal carries (the VM text).
    static NOT_SENDABLE: RefCell<crate::hash::HashMap<i64, String>> = RefCell::new(crate::hash::HashMap::default());
    /// The description of the last refused message.
    static LAST_REJECTION: RefCell<String> = const { RefCell::new(String::new()) };
}

/// The units of the text accumulator `h` (`_AcumuladorDeTexto`): the attachment at
/// `b+16` is a `Box<Vec<u16>>` (docs/NATIVO-ESPACO-UNIFICADO.md §2.5).
fn accumulator_units(heap: &Heap, h: i64) -> Vec<u16> {
    let p = heap.attachment(h) as *const Vec<u16>;
    if p.is_null() {
        return Vec::new();
    }
    // SAFETY: the attachment of a live `_AcumuladorDeTexto` is its `Vec<u16>`,
    // freed only when the block dies; the heap borrow keeps it.
    #[allow(unsafe_code)]
    unsafe {
        (*p).clone()
    }
}

/// The elements of a `_List`/`_ImmutableList` as a graph node, by shape.
fn fixed_list_node(heap: &Heap, h: i64, class: i32, v: &mut dyn FnMut(i64) -> GValue) -> GNode {
    use crate::lists::{Element, RefElements};
    match heap.list_elements(h) {
        RefElements::General(e) => GNode::Refs { cid: class, word0: e.len() as i64, refs: e.iter().map(|&x| v(x)).collect() },
        RefElements::Int(e) => GNode::Compact { cid: class, shape: Element::Int, elements: e.to_vec() },
        RefElements::Double(e) => GNode::Compact { cid: class, shape: Element::Double, elements: e.iter().map(|d| d.to_bits() as i64).collect() },
        RefElements::Bool(e) => GNode::Compact { cid: class, shape: Element::Bool, elements: e.to_vec() },
    }
}

/// The node of block `h` (already checked: neither immediate, nor static, nor a program
/// attachment). `v` translates each reference found.
fn block_node(heap: &Heap, h: i64, v: &mut dyn FnMut(i64) -> GValue, transferred: &mut Vec<i64>) -> Result<GNode, IllegalMessage> {
    use crate::layout::{cid, flags};
    let c = *heap.header(h);
    let class = c.class_id;
    let shape = c.flags & flags::FORMA;
    if shape == flags::INSTANCIA {
        if cid::is_typed(class) {
            // View (`INSTANCIA`, cids 36–65): the base and the offset.
            let t = heap.tipada(h).expect("live view");
            return Ok(GNode::View { cid: class, base: v(t.base.unwrap_or(0)), offset: t.offset, len: t.len });
        }
        let object = heap.object(h).expect("live object");
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
            let fields = heap.record(h).expect("live record");
            return Ok(GNode::Refs { cid: class, word0: fields.len() as i64, refs: fields.iter().map(|&x| v(x)).collect() });
        }
        // Another `REFS`: word 0 is the number of references.
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
        let t = heap.tipada(h).expect("live typed list");
        let bytes = heap.typed_bytes(h).map(<[u8]>::to_vec).unwrap_or_default();
        return Ok(GNode::Typed { tipo: t.tipo, bytes });
    }
    if cid::is_fixed_list(class) {
        // Compact: `list_elements` tells the shape.
        return Ok(fixed_list_node(heap, h, class, v));
    }
    Ok(GNode::Bruto { cid: class, flags: c.flags, words: heap.words(h).into() })
}

/// Copies the value `root` (a value in a `Ref` position) into a graph.
/// `share`: the destination is this same isolate.
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
            // The metadata is `1 + type` (0 = no type, `types.rs`).
            Some(t) if meta != 0 => 1 + t.export(meta - 1),
            _ => meta,
        };
        // A `Type` object carries the type through the message table.
        if let (Some(t), GNode::Instance { cid, fields }) = (types.as_mut(), &mut no)
            && i64::from(*cid) == TYPE_CLASS
            && let Some((GValue::Word(id), _)) = fields.first().copied()
        {
            fields[0] = (GValue::Word(t.export(id)), false);
        }
        nodes[i] = (no, meta);
    }
    // Sent `TransferableTypedData` become empty at the source.
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
    /// Calls `f` for the root and for each edge of the graph.
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

    /// A single-value graph, without nodes (runtime answers): `null` or
    /// a `Smi`.
    fn immediate(r: i64) -> Graph {
        Graph { nodes: Vec::new(), root: GValue::Word(r), origin: 0, types: None, tables: Vec::new() }
    }

    /// The graph of `null`.
    pub fn null_val() -> Graph {
        Graph::immediate(0)
    }

    /// The graph of the `int` `v`.
    pub fn from_int(v: i64) -> Graph {
        Portable::Int(v).to_graph()
    }
}

/// A value the runtime reads or builds outside a heap (the messages of the
/// native services — the VM's `Dart_CObject`): `null`, `bool`, `int`,
/// `double`, `String`, list, `Uint8List` and, only when reading, the fields of an
/// object (the reply `SendPort`).
#[derive(Clone, Debug)]
pub enum Portable {
    Null,
    Bool(bool),
    Int(i64),
    Double(f64),
    Str(String),
    List(Vec<Portable>),
    /// The bytes of a typed list (of any element type, like the VM's
    /// `CObjectTypedData`).
    Bytes(Vec<u8>),
    /// A typed list of `tipo` elements (the runtime's `TIPO_*`).
    Typed(u8, Vec<u8>),
    /// A runtime object with one integer field (see [`GNode::FromRuntime`]).
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

/// A static object of the image (`GValue::Static`) read without a heap: the `bool`
/// boxes and the string literals (the block is constant and lives for the whole
/// process; `layout` gives the offsets).
fn portable_from_static(h: i64) -> Portable {
    use crate::layout::{DESLOCAMENTO_DO_HANDLE, cid, desl};
    let b = (h - DESLOCAMENTO_DO_HANDLE) as *const u8;
    // SAFETY: `h` is the handle of a static object (checked when copying), a
    // constant block of the image section with the whole header and body.
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
    /// The graph read as [`Portable`] (on a thread without a Dart heap). A cycle,
    /// which service messages never have, becomes `Null` on the way back.
    pub fn to_portable(&self) -> Portable {
        /// The word `v`; `is_ref_kind` says whether it is in a `Ref` position (otherwise,
        /// the bits of a scalar field, read as `int`).
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
            use crate::lists::Element;
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
                    // The length (field 0, raw) and the storage (field 1).
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
    /// The graph of this value.
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

/// Allocates, in this isolate's heap, the empty block of node `no` (the references
/// are left for later: the graph may have cycles). `None` for the view, which
/// needs the base already allocated, and for attachments (accumulator, `RegExp`
/// program), which the runtime creates outside the heap borrow
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
                heap.new_list(*c, refs.len(), crate::lists::Element::General)
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
            // Before publishing: no barrier (and they are scalars).
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

/// The block of a node with a native attachment (the `StringBuffer` accumulator, the
/// `RegExp` program), created by the attachment owner (`natives_strings.rs`,
/// `regexp.rs`), which borrows the heap itself. `None` for the other nodes.
fn allocate_attachment(no: &GNode) -> Option<i64> {
    match no {
        GNode::Accumulator(u) => Some(new_accumulator_with(u.clone())),
        GNode::ReProgram { source, options } => Some(re_program_from_pattern(source, *options)),
        _ => None,
    }
}

/// Recreates the graph in this isolate's heap and returns the value (in a
/// `Ref` position). All nodes are allocated first, rooted, and the edges
/// linked afterwards — the graph may have cycles.
fn materialize(g: &Graph) -> i64 {
    let same = g.origin == isolate_id();
    // The method tables of the classes this isolate has not seen yet.
    if !g.tables.is_empty() {
        METHODS.with(|m| {
            let mut m = m.borrow_mut();
            for (c, t) in &g.tables {
                m.entry(*c).or_insert_with(|| t.clone());
            }
        });
    }
    // The message types in this isolate's universe, and its canonical `Type`
    // objects (outside the heap borrow).
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
    // This isolate's canonical constants, through the getters (outside the
    // heap borrow: the getter is generated code, which allocates the first
    // time). They are permanent: they need no root during assembly.
    let mut canonical: crate::hash::HashMap<usize, i64> = crate::hash::HashMap::default();
    let mut tearoffs: crate::hash::HashMap<i64, i64> = crate::hash::HashMap::default();
    g.visit_values(|v| match *v {
        GValue::Constant(getter) => {
            canonical.entry(getter).or_insert_with(|| {
                // SAFETY: `getter` is the address of a generated getter without
                // arguments (registered by `dartforge_marcar_constante`),
                // the same in all isolates of the process.
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
    // Owning copies of the nodes, including attachments created outside the
    // heap borrow, last until the edges are linked and the indices rebuilt.
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
                // Outside the source isolate there is nothing to share: the
                // sender copies (`share` false) for another isolate.
                GValue::Same(_) => 0,
            }
        };
        // The views, after the bases (a view of a view waits for its own).
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

/// Copied `_Map`/`_Set` arrive with the source's `_index`, computed with
/// the identity hashes there (here the hash is the address, `heap.rs`
/// `identity_hash`): like the VM (`runtime/vm/object_graph_copy.cc`,
/// `CopyLinkedHashBase` + `_rehashObjects`), the index is rebuilt at the destination
/// when some key may have another hash. The decision and the rebuild belong to
/// Dart (`_dartforgeRefazerIndiceCopiado`, `native_sdk/collection/
/// compact_hash.dart`): -1 for an object that is not a `_Map`/`_Set` (the whole class
/// is left out from then on). The nodes stay rooted in the
/// frame of [`materialize`].
fn refazer_indices_copiados(g: &Graph, handles: &[i64]) {
    let Some(f) = ajudante("_dartforgeRefazerIndiceCopiado") else { return };
    // SAFETY: registered by `dart:_compact_hash` with the signature
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
// The port registry and the isolate queue.

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
            let text = h.allocate_str("do not publish");
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
            let text = h.allocate_str("unserved message");
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
            assert_eq!(h.text(text).unwrap().para_string(), "aliases in the queue");
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
            let text = h.allocate_str("aliases in the queue");
            h.set_root(frame, 0, text);
            let list = h.new_list(crate::layout::cid::LIST, 2, crate::lists::Element::General);
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
            let text = h.allocate_str("shared message");
            h.set_root(frame, 0, text);
            (frame, text)
        });
        let port = dartforge_nativo_DartForge_porta_abrir(0);
        assert_eq!(dartforge_nativo_DartForge_porta_enviar(port, text), 0);
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.pop_frame(frame);
            h.collect();
            assert!(h.is_live_object(text), "the queue must keep the shared owner");
        });
        dartforge_nativo_DartForge_porta_fechar(port);
        extern "C" fn ignore(_: i64) -> i64 { 0 }
        assert!(dispatch_next(ignore));
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            assert!(!h.is_live_object(text), "discarding the message must release its owner");
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

/// A message in an isolate's queue.
struct Message {
    port: i64,
    graph: Graph,
    /// Tokens of the receiving heap for each `Same` occurrence.
    owners: Vec<u64>,
    arrival: std::time::Instant,
}

/// The messages of an isolate: those of the Dart ports and those of the control
/// port (the VM's OOB ones: `pause`, `kill`, `ping`…), which the runtime serves
/// before the others.
#[derive(Default)]
struct Queues {
    /// Protected by the same mutex as publication: old clones do not reopen the queue.
    closed: bool,
    normal: std::collections::VecDeque<Message>,
    control: std::collections::VecDeque<Graph>,
    /// Requests to run at the safepoint (the publication of a JIT
    /// reload), served with the control messages, between one event and the next.
    safepoints: std::collections::VecDeque<SafepointRequest>,
}

/// A request to run at the isolate's safepoint: the function receives the datum
/// and `1` when it runs at the safepoint, or `0` when the isolate ended without
/// serving it (the request is discarded, and the requester gets to know).
pub type SafepointRequest = (extern "C" fn(usize, i32), usize);

/// The message queue of an isolate; any thread posts to it.
pub struct IsolateQueue {
    id: u64,
    messages: std::sync::Mutex<Queues>,
    sig_num: std::sync::Condvar,
    /// The timed wait on macOS (see [`Waker`]).
    waker: Waker,
    /// The address of the interrupt request of the isolate's thread
    /// (`Context::interrupt`), or 0 after it ended. A control message
    /// sets it: the generated code reads it at the safepoint of each loop
    /// iteration and calls [`dartforge_ponto_seguro`], which serves the control without
    /// waiting for the next event — an immediate `kill` or a `ping` reach
    /// an isolate stuck in a loop without events, like the VM's stack
    /// check. The mutex guarantees nobody writes after the thread zeroed it.
    interrupt: std::sync::Mutex<usize>,
}

impl IsolateQueue {
    /// Wakes the isolate's thread: the `Condvar` and, on macOS, the waker.
    fn notify(&self) {
        self.sig_num.notify_one();
        self.waker.wake();
    }
}

/// The event loop's timed wait on macOS: a `kqueue` with an
/// `EVFILT_USER` that the notification triggers and the deadline in the `kevent` `timeout`, like
/// the VM's event handler (`eventhandler_macos.cc`, `Poll`). The
/// `Condvar`'s `pthread_cond_timedwait` overshoots the deadline by the kernel's
/// coalescing slack: 200 timers of 5 ms took 5.9 s versus 1.5 s in the
/// VM (the probe `tools/sondas/precisao_de_timers.dart`), and
/// `corpus/native/33_mensagens_de_controle` timed out. Outside macOS
/// the `Condvar` is already precise and the waker does nothing.
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
        // SAFETY: creates the kqueue; the event lives during the call.
        let kq = unsafe { kqueue_do_despertador() };
        if kq < 0 {
            panic!("failed to create the event loop kqueue");
        }
        let ev = Self::event(Self::EV_ADD | Self::EV_CLEAR, 0);
        // SAFETY: the change lives during the call.
        if unsafe { kevent_do_despertador(kq, &ev, 1, std::ptr::null_mut(), 0, std::ptr::null()) } == -1 {
            panic!("failed to register the event loop waker");
        }
        Waker { kq }
    }

    fn wake(&self) {
        let ev = Self::event(0, Self::NOTE_TRIGGER);
        // SAFETY: the change lives during the call; the kqueue belongs to this isolate.
        unsafe { kevent_do_despertador(self.kq, &ev, 1, std::ptr::null_mut(), 0, std::ptr::null()) };
    }

    fn wait_for(&self, deadline: std::time::Duration) {
        let ts = PrazoDoDespertador { seconds: deadline.as_secs() as i64, nanos: i64::from(deadline.subsec_nanos()) };
        let mut ev = Self::event(0, 0);
        // SAFETY: an output event; the deadline lives during the call.
        unsafe { kevent_do_despertador(self.kq, std::ptr::null(), 0, &mut ev, 1, &ts) };
    }
}

#[cfg(target_vendor = "apple")]
impl Drop for Waker {
    fn drop(&mut self) {
        // SAFETY: the kqueue belongs to this waker.
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

/// Zeroes the interrupt request address when the thread ends (before
/// the memory of its `thread_local`s is freed).
struct InterruptGuard(std::sync::Arc<IsolateQueue>);

impl Drop for InterruptGuard {
    fn drop(&mut self) {
        *self.0.interrupt.lock().unwrap_or_else(|e| e.into_inner()) = 0;
    }
}

/// A native service: receives (port, graph) on the sender's thread.
pub type NativeService = std::sync::Arc<dyn Fn(i64, Graph) + Send + Sync>;

enum Owner {
    Isolate(std::sync::Arc<IsolateQueue>),
    /// The control port of an isolate (`Isolate.controlPort`).
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
    /// Zeroes the interrupt request address at the end of the thread.
    static INTERRUPT_GUARD: RefCell<Option<InterruptGuard>> = const { RefCell::new(None) };
    /// Open ports of this isolate → whether they keep the isolate alive.
    static OPEN_PORTS: RefCell<crate::hash::HashMap<i64, bool>> = RefCell::new(crate::hash::HashMap::default());
    /// The message being dispatched: (port, value), the value rooted.
    static CURRENT: RefCell<(i64, i64)> = const { RefCell::new((0, 0)) };
    /// The Dart dispatcher (`_RawReceivePort._despachar`), rooted.
    static DISPATCHER: RefCell<i64> = const { RefCell::new(0) };
}

/// Global runtime roots for the ports (negative ids outside the range
/// of the timers of `events.rs`).
const DISPATCHER_ROOT: i64 = -0x7000_0000_0000;
const MESSAGE_ROOT: i64 = -0x7000_0000_0001;

fn isolate_id() -> u64 {
    QUEUE.with(|f| f.id)
}

/// Posts `graph` to the port `port`: in the owner isolate's queue, or in the
/// native service. Closed or nonexistent port: the message is discarded,
/// as in the VM.
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

/// Publishes with the reference already obtained from the registry; the mutex linearizes closing.
fn post_to_queue(f: &IsolateQueue, port: i64, graph: Graph) {
    let mut m = f.messages.lock().unwrap_or_else(|e| e.into_inner());
    if m.closed { return; }
    // Retaining neither collects nor calls Dart; closing uses this same mutex.
    let mut owners = Vec::new();
    graph.visit_values(|v| {
        if let GValue::Same(h) = *v {
            assert_eq!(f.id, isolate_id(), "shared owner outside its isolate");
            assert_eq!(graph.origin, f.id, "wrong origin of the shared owner");
            owners.push(HEAP.with(|heap| heap.borrow_mut().retain_message_owner(h)));
        }
    });
    m.normal.push_back(Message { port, graph, owners, arrival: std::time::Instant::now() });
    f.notify();
}

/// Posts `graph` to the control queue of the isolate that owns `port` (the
/// OOB messages: `Isolate_sendOOB` holds for any port of the isolate).
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
            // SAFETY: the context of the isolate's thread, alive while the
            // address has not been zeroed (with this mutex) by its guard.
            unsafe { (*(*target as *const std::sync::atomic::AtomicU8)).store(1, std::sync::atomic::Ordering::Release) };
        }
        drop(target);
        f.notify();
    }
}

/// Clears this isolate's interrupt request (whoever serves it will drain the
/// control queue next; a message arriving later sets it again).
fn clear_interrupt_request() {
    crate::heap::CONTEXTO.with(|c| c.interrupt.store(0, std::sync::atomic::Ordering::Relaxed));
}

// ---------------------------------------------------------------------------
// The main isolate's safepoint (the JIT reload).
//
// Publishing a new generation (swapping the cells of the stable
// entries, registering the tables) can only happen when no Dart frame
// of the isolate is on the stack: between two loop events — the point at which
// the VM also commits a reload (`isolate_reload.cc`). The watcher of
// `dartforge reload` compiles on its own thread and hands over the publication as
// a request in the main isolate's queue; the loop serves it before the next
// event, and an idle isolate (a server waiting for a connection) wakes up for it.

fn main_isolate() -> &'static std::sync::Mutex<Option<std::sync::Arc<IsolateQueue>>> {
    static P: std::sync::OnceLock<std::sync::Mutex<Option<std::sync::Arc<IsolateQueue>>>> = std::sync::OnceLock::new();
    P.get_or_init(Default::default)
}

/// This isolate is the main one: it starts accepting requests at the safepoint.
pub fn mark_main_isolate() {
    let queue = QUEUE.with(|f| f.clone());
    *main_isolate().lock().unwrap_or_else(|e| e.into_inner()) = Some(queue);
}

/// The main isolate ended: pending requests are discarded (the
/// requester receives `0`) and new ones refused.
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

/// Asks for `f(datum, 1)` to run at the next safepoint of the main isolate,
/// on its thread. Returns `0` (and does not call `f`) if there is no main isolate
/// running the event loop; `1` if the request was accepted — then `f` is
/// called exactly once, with `1` at the safepoint or with `0` if the
/// isolate ends first.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_pedir_no_ponto_seguro(f: extern "C" fn(usize, i32), datum: usize) -> i32 {
    // The registry stays locked during the insertion: the isolate's end-of-life
    // discard does not lose a request arriving at the same time.
    let main_queue = main_isolate().lock().unwrap_or_else(|e| e.into_inner());
    let Some(queue) = main_queue.as_ref() else {
        return 0;
    };
    queue.messages.lock().unwrap_or_else(|e| e.into_inner()).safepoints.push_back((f, datum));
    queue.notify();
    1
}

// Stopping all isolates (the publication of a reload): the new code
// can only come in with no Dart frame on the stack of ANY isolate,
// because the cells of the stable entries belong to the process. The main
// isolate, at its safepoint, asks the others to stop at their next
// safepoint (`dartforge_parar_isolados`), swaps the code, and
// releases them (`dartforge_liberar_isolados`); each one redoes its own registrations
// of the new generation (the runtime state is per isolate) before continuing.

/// The isolates alive in the event loop (those that serve requests).
fn live_isolates() -> &'static std::sync::Mutex<Vec<std::sync::Arc<IsolateQueue>>> {
    static V: std::sync::OnceLock<std::sync::Mutex<Vec<std::sync::Arc<IsolateQueue>>>> = std::sync::OnceLock::new();
    V.get_or_init(Default::default)
}

thread_local! {
    static LIVE_REGISTERED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// This isolate entered the loop: it is now stopped during publications.
fn register_live_isolate() {
    if LIVE_REGISTERED.with(|r| r.replace(true)) {
        return;
    }
    let queue = QUEUE.with(|f| f.clone());
    live_isolates().lock().unwrap_or_else(|e| e.into_inner()).push(queue);
}

/// This isolate ended: it leaves the registry, and its pending requests are
/// answered with `0` (under the same latch: no request arrives afterwards).
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

/// The barrier of a stop: how many stopped, and the registrations of the new generation
/// when the main isolate releases.
#[derive(Default)]
struct WriteBarrier {
    estado: std::sync::Mutex<(usize, Option<[usize; 3]>)>,
    sig_num: std::sync::Condvar,
}

/// The stop request, in an isolate that is not the main one.
extern "C" fn parar_neste_isolado(datum: usize, execute_fn: i32) {
    // SAFETY: `datum` is an `Arc<Barreira>` handed over by `dartforge_parar_isolados`
    // (one reference per request, returned here).
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
        // SAFETY: 0 or a `void ()` trampoline of the published generation.
        (p != 0).then(|| unsafe { std::mem::transmute::<usize, extern "C" fn()>(p) })
    };
    dartforge_publicar_geracao(as_fn(area), as_fn(register), as_fn(rti));
}

/// Stops all live isolates other than this one at their safepoint and returns the
/// stop, which [`dartforge_liberar_isolados`] ends. Called at the
/// main isolate's safepoint.
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

/// Releases the isolates of the stop `stopped_flag`: each one redoes the registrations of the
/// new generation (`area`, `register`, `rti`, `void ()` trampolines or 0) on
/// its thread and continues.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_liberar_isolados(stopped_flag: usize, area: usize, register: usize, rti: usize) {
    // SAFETY: `stopped_flag` came from `dartforge_parar_isolados`, once.
    let barrier = unsafe { std::sync::Arc::from_raw(stopped_flag as *const WriteBarrier) };
    barrier.estado.lock().unwrap_or_else(|e| e.into_inner()).1 = Some([area, register, rti]);
    barrier.sig_num.notify_all();
}

/// Serves the requests at this isolate's safepoint (the event loop).
fn serve_safepoints() {
    loop {
        let request = QUEUE.with(|f| f.messages.lock().unwrap_or_else(|e| e.into_inner()).safepoints.pop_front());
        let Some((f, datum)) = request else { return };
        f(datum, 1);
    }
}

/// The next message of this isolate's control port.
fn next_control() -> Option<Graph> {
    clear_interrupt_request();
    QUEUE.with(|f| f.messages.lock().unwrap_or_else(|e| e.into_inner()).control.pop_front())
}

/// Opens this isolate's control port and returns the id.
fn open_control_port() -> i64 {
    let id = next_port_id();
    let queue = QUEUE.with(|f| f.clone());
    rec().lock().unwrap_or_else(|e| e.into_inner()).insert(id, Owner::Control(queue));
    id
}

/// Closes all ports of this isolate (it ended): messages that
/// arrive afterwards are discarded, as in the VM.
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

/// Closes a native port: the following messages are discarded.
pub fn close_native_port(id: i64) {
    rec().lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
}

/// Opens a native port served by `service` and returns the id.
pub fn open_native_port(service: NativeService) -> i64 {
    let id = next_port_id();
    rec().lock().unwrap_or_else(|e| e.into_inner()).insert(id, Owner::Native(service));
    id
}

thread_local! {
    /// `NativeCallable.isolateLocal` opened with `keepIsolateAlive`
    /// (`ffi_callbacks.rs`): each one keeps the isolate alive like a port.
    static CALLBACKS_KEEPING_ALIVE: std::cell::Cell<i64> = const { std::cell::Cell::new(0) };
}

/// Adds `delta` to the local callbacks that keep the isolate alive.
pub fn adjust_callbacks_keeping_alive(delta: i64) {
    CALLBACKS_KEEPING_ALIVE.with(|c| c.set((c.get() + delta).max(0)));
}

/// Whether the isolate has an open port (or local callback) that keeps it alive.
fn has_live_port() -> bool {
    CALLBACKS_KEEPING_ALIVE.with(|c| c.get() > 0) || OPEN_PORTS.with(|p| p.borrow().values().any(|&v| v))
}

/// The arrival of the oldest message in the queue, if any.
fn next_arrival() -> Option<std::time::Instant> {
    QUEUE.with(|f| f.messages.lock().unwrap_or_else(|e| e.into_inner()).normal.front().map(|m| m.arrival))
}

/// Waits for a message until `deadline` (or without deadline). Returns whether one arrived. With
/// `control_only` (the isolate paused), only a control message wakes it.
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
                        // The timed `kevent` (see [`Waker`]): a notification
                        // arriving between releasing the mutex and waiting stays
                        // pending in the `EVFILT_USER` and wakes immediately.
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

/// Releases tokens exclusively in the heap that created them (the receiver).
fn release_message_owners(owners: &[u64]) {
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        for &id in owners { h.release_message_owner(id); }
    });
}

/// Takes the next message, materializes it and dispatches it through the generated
/// code's `call`. Returns whether there was a message.
fn dispatch_next(call: extern "C" fn(i64) -> i64) -> bool {
    let Some(m) = QUEUE.with(|f| f.messages.lock().unwrap_or_else(|e| e.into_inner()).normal.pop_front()) else {
        return false;
    };
    // Message for an already closed port: discarded, as in the VM.
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
// Overlay natives (`native_sdk/isolate/isolate_patch.dart`).

/// `DartForge_porta_abrir(port)`: opens a port of this isolate for the
/// object `port` (a `_RawReceivePort`, whose class becomes
/// unsendable) and returns the id.
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

/// `DartForge_porta_fechar(id)`: closes the port; pending messages for
/// it are discarded at dispatch.
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

/// `DartForge_porta_enviar(id, message)`: copies and posts. Returns 1 if the
/// message has an unsendable object (the description stays in
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

/// `DartForge_porta_recusa()`: why the last message was refused.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_porta_recusa() -> i64 {
    let d = LAST_REJECTION.with(|u| u.borrow().clone());
    HEAP.with(|h| h.borrow_mut().allocate_str(&d))
}

/// `DartForge_portas_despachante(f)`: the argument-less closure the loop
/// calls for each message (`_RawReceivePort._despachar`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_portas_despachante(f: i64) {
    HEAP.with(|h| h.borrow_mut().set_global_root(DISPATCHER_ROOT, f));
    DISPATCHER.with(|d| *d.borrow_mut() = f);
}

/// `DartForge_porta_atual()`: the port of the message being dispatched.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_porta_atual() -> i64 {
    CURRENT.with(|a| a.borrow().0)
}

/// `DartForge_mensagem_atual()`: the message being dispatched.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_mensagem_atual() -> i64 {
    CURRENT.with(|a| a.borrow().1)
}

/// `DartForge_classe_nao_enviavel(object)`: the class of `object` does not go
/// in a message (the VM's `@pragma('vm:isolate-unsendable')`). The description is the
/// VM's without the private library suffix and without the retaining path.
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

/// `DartForge_classe_transferivel(object)`: the class of `object` has its
/// first field moved in a message (`TransferableTypedData`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_classe_transferivel(object: i64) {
    let class = HEAP.with(|h| h.borrow().object(object).map(|o| o.class_id));
    if let Some(c) = class
        && !transferable_classes().contains(&c)
    {
        transferable().write().unwrap_or_else(|e| e.into_inner()).insert(c);
    }
}

/// `DartForge_capacidade_nova()`: a capability id unique in the process.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_capacidade_nova() -> i64 {
    static NEXT_ID: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);
    NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// The registered name of class `c` (empty if none).
fn class_name(c: i64) -> String {
    CLASS_NAMES.with(|m| m.borrow().get(&c).cloned()).unwrap_or_default()
}
