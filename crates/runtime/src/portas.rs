// Runtime nativo: portas (`RawReceivePort`/`SendPort`) e a fila de mensagens
// do isolado — o `PortMap`/`MessageHandler` da VM (`runtime/vm/port.cc`,
// `message_handler.cc`).
//
// * Uma porta é um id global (nunca 0). O registro global diz a quem ela
//   pertence: a fila de um isolado (uma thread) ou um serviço do runtime
//   (as portas nativas, como a do IOService), que recebe a mensagem na
//   thread de quem enviou.
// * Enviar copia a mensagem para um grafo portátil (`Grafo`), que o destino
//   materializa no próprio heap. É a semântica da VM: o que é mutável é
//   copiado; o que é profundamente imutável e permanente (constantes,
//   enums, tear-offs de topo, literais, strings) passa pela identidade
//   quando a mensagem fica no mesmo isolado.
// * A fila do isolado guarda (porta, grafo, chegada). O laço de eventos
//   (`eventos.rs`) atende timers e mensagens pela ordem de chegada — na VM o
//   timer também é uma mensagem —, e esvazia as microtarefas depois de
//   cada uma.
// * O despacho chama o `_RawReceivePort._despachar` da sobreposição (o
//   `_handleMessage` da VM), que lê a porta e a mensagem correntes.
// * O isolado vive enquanto tiver porta aberta com `keepIsolateAlive`.

/// Um valor dentro de um grafo portátil.
#[derive(Clone, Copy, Debug)]
enum ValG {
    /// Escalar (int, double, bool) com a tag, ou `Smi`/null numa posição Ref.
    Bits(i64, bool, ValueTag),
    /// O nó `i` do grafo.
    No(usize),
    /// Um objeto permanente do heap de origem, passado pela identidade (só
    /// quando origem e destino são o mesmo isolado).
    Mesmo(i64),
}

/// Um nó do grafo: a forma de um `Value` do heap, com as referências
/// trocadas por `ValG`, e as marcas das tabelas laterais do objeto.
#[derive(Clone, Debug)]
enum NoG {
    String(Texto),
    StringBuffer(Vec<u16>),
    RegExp(Texto),
    Match(Texto),
    Object { class_id: i64, fields: Vec<ValG> },
    Cell(ValG),
    Environment(Vec<ValG>),
    Closure { code_id: i64, environment: ValG },
    List { itens: Vec<ValG>, fixa: bool, imutavel: bool, pendente: Option<usize> },
    Map(Vec<(ValG, ValG)>, bool),
    Set(Vec<ValG>, bool),
    Record(Vec<ValG>),
    BoxedInt(i64),
    BoxedDouble(f64),
    BoxedBool(bool),
}

/// Uma mensagem copiada: os nós (com o metadado RTI de cada um) e a raiz.
pub struct Grafo {
    nos: Vec<(NoG, i64)>,
    raiz: ValG,
    /// O isolado de origem (id da fila): `ValG::Mesmo` só vale nele.
    origem: u64,
}

/// Por que uma mensagem não pode ser enviada.
pub struct MensagemIlegal(pub String);

fn val_de_tagged(v: TaggedValue, mapa: &mut std::collections::HashMap<i64, usize>, pilha: &mut Vec<i64>, nos: &mut Vec<(NoG, i64)>, compartilhar: bool) -> ValG {
    if !v.is_ref || !smi::e_handle(v.bits) {
        return ValG::Bits(v.bits, v.is_ref, v.tag);
    }
    ref_de_handle(v.bits, mapa, pilha, nos, compartilhar)
}

fn ref_de_handle(h: i64, mapa: &mut std::collections::HashMap<i64, usize>, pilha: &mut Vec<i64>, nos: &mut Vec<(NoG, i64)>, compartilhar: bool) -> ValG {
    if !smi::e_handle(h) {
        return ValG::Bits(h, true, ValueTag::Ref);
    }
    if compartilhar {
        let permanente = HEAP.with(|heap| {
            let heap = heap.borrow();
            heap.e_permanente(h) || matches!(heap.try_get(h), Some(Value::String(_)))
        });
        if permanente {
            return ValG::Mesmo(h);
        }
    }
    if let Some(&i) = mapa.get(&h) {
        return ValG::No(i);
    }
    let i = nos.len();
    mapa.insert(h, i);
    // Reserva o nó; o conteúdo é preenchido quando `h` sai da pilha.
    nos.push((NoG::BoxedBool(false), 0));
    pilha.push(h);
    ValG::No(i)
}

thread_local! {
    /// Classes cujas instâncias não podem ir numa mensagem (as portas de
    /// recepção; `DartForge_porta_abrir` registra a classe).
    /// Classe → a descrição que a recusa leva (o texto da VM).
    static NAO_ENVIAVEIS: RefCell<std::collections::HashMap<i64, String>> = RefCell::new(std::collections::HashMap::new());
    /// A descrição da última mensagem recusada.
    static ULTIMA_RECUSA: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Copia o valor `raiz` (um valor numa posição `Ref`) para um grafo.
/// `compartilhar`: o destino é este mesmo isolado.
fn copiar_para_grafo(raiz: i64, compartilhar: bool) -> Result<Grafo, MensagemIlegal> {
    let mut mapa = std::collections::HashMap::new();
    let mut pilha = Vec::new();
    let mut nos: Vec<(NoG, i64)> = Vec::new();
    let r = ref_de_handle(raiz, &mut mapa, &mut pilha, &mut nos, compartilhar);
    while let Some(h) = pilha.pop() {
        let i = mapa[&h];
        let r = HEAP.with(|heap| -> Result<(NoG, i64), MensagemIlegal> {
            let heap = heap.borrow();
            let meta = heap.metadado(h);
            let fixa = heap.fixas.contains(&h);
            let imutavel = heap.imutaveis.contains(&h);
            let pendente = heap.pendentes.get(&h).copied();
            let mut v = |t: &TaggedValue| val_de_tagged(*t, &mut mapa, &mut pilha, &mut nos, compartilhar);
            let no = match heap.get(h) {
                Value::String(t) => NoG::String(t.clone()),
                Value::StringBuffer(u) => NoG::StringBuffer(u.clone()),
                Value::RegExp(t) => NoG::RegExp(t.clone()),
                Value::Match(t) => NoG::Match(t.clone()),
                Value::Object { class_id, fields } => {
                    if let Some(d) = NAO_ENVIAVEIS.with(|n| n.borrow().get(class_id).cloned()) {
                        return Err(MensagemIlegal(d));
                    }
                    let fields = fields
                        .iter()
                        .map(|&(bits, is_ref)| {
                            if is_ref {
                                v(&TaggedValue::reference(bits))
                            } else {
                                ValG::Bits(bits, false, ValueTag::Int)
                            }
                        })
                        .collect();
                    NoG::Object { class_id: *class_id, fields }
                }
                Value::Cell(t) => NoG::Cell(v(t)),
                Value::Environment(vs) => NoG::Environment(vs.iter().map(&mut v).collect()),
                Value::Closure { code_id, environment } => {
                    NoG::Closure { code_id: *code_id, environment: v(&TaggedValue::reference(*environment)) }
                }
                Value::List(vs) => NoG::List { itens: vs.iter().map(&mut v).collect(), fixa, imutavel, pendente },
                Value::Map(es) => NoG::Map(es.iter().map(|(a, b)| (v(a), v(b))).collect(), imutavel),
                Value::Set(vs) => NoG::Set(vs.iter().map(&mut v).collect(), imutavel),
                Value::Record(vs) => NoG::Record(vs.iter().map(&mut v).collect()),
                Value::BoxedInt(x) => NoG::BoxedInt(*x),
                Value::BoxedDouble(x) => NoG::BoxedDouble(*x),
                Value::BoxedBool(x) => NoG::BoxedBool(*x),
            };
            Ok((no, meta))
        });
        let (no, meta) = r?;
        nos[i] = (no, meta);
    }
    Ok(Grafo { nos, raiz: r, origem: id_do_isolado() })
}

impl Grafo {
    /// Um grafo de um valor só, sem referências (as respostas do runtime).
    pub fn escalar(bits: i64, tag: ValueTag) -> Grafo {
        Grafo { nos: Vec::new(), raiz: ValG::Bits(bits, false, tag), origem: 0 }
    }
}

/// Um valor que o runtime monta fora de um heap (as respostas dos serviços
/// nativos) e que vira um grafo: `null`, `bool`, `int`, `double`, `String`,
/// lista e `Uint8List` (os bytes; materializada como a lista de inteiros de
/// tamanho fixo que o `typed_data` da sobreposição envolve).
#[derive(Clone, Debug)]
pub enum Portavel {
    Nulo,
    Bool(bool),
    Int(i64),
    Double(f64),
    Str(String),
    Lista(Vec<Portavel>),
    Bytes(Vec<u8>),
}

impl Portavel {
    /// O grafo deste valor.
    pub fn para_grafo(&self) -> Grafo {
        fn val(p: &Portavel, nos: &mut Vec<(NoG, i64)>) -> ValG {
            match p {
                Portavel::Nulo => ValG::Bits(0, true, ValueTag::Ref),
                Portavel::Bool(b) => {
                    let i = nos.len();
                    nos.push((NoG::BoxedBool(*b), 0));
                    ValG::No(i)
                }
                Portavel::Int(x) => match smi::de(*x) {
                    Some(r) => ValG::Bits(r, true, ValueTag::Ref),
                    None => {
                        let i = nos.len();
                        nos.push((NoG::BoxedInt(*x), 0));
                        ValG::No(i)
                    }
                },
                Portavel::Double(x) => {
                    let i = nos.len();
                    nos.push((NoG::BoxedDouble(*x), 0));
                    ValG::No(i)
                }
                Portavel::Str(s) => {
                    let i = nos.len();
                    nos.push((NoG::String(Texto::de_str(s)), 0));
                    ValG::No(i)
                }
                Portavel::Lista(itens) => {
                    let i = nos.len();
                    nos.push((NoG::BoxedBool(false), 0));
                    let vs: Vec<ValG> = itens.iter().map(|x| val(x, nos)).collect();
                    nos[i] = (NoG::List { itens: vs, fixa: true, imutavel: false, pendente: None }, 0);
                    ValG::No(i)
                }
                Portavel::Bytes(b) => {
                    let i = nos.len();
                    let vs = b.iter().map(|&x| ValG::Bits(i64::from(x), false, ValueTag::Int)).collect();
                    nos.push((NoG::List { itens: vs, fixa: true, imutavel: false, pendente: None }, 0));
                    ValG::No(i)
                }
            }
        }
        let mut nos = Vec::new();
        let raiz = val(self, &mut nos);
        Grafo { nos, raiz, origem: 0 }
    }
}

/// Recria o grafo no heap deste isolado e devolve o valor (numa posição
/// `Ref`). Todos os nós são alocados primeiro, enraizados, e as arestas
/// ligadas depois — o grafo pode ter ciclos.
fn materializar(g: &Grafo) -> i64 {
    let mesmo = g.origem == id_do_isolado();
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let frame = heap.push_frame_with_slots(g.nos.len());
        let mut handles = Vec::with_capacity(g.nos.len());
        for (i, (no, _)) in g.nos.iter().enumerate() {
            let vazio = match no {
                NoG::String(t) => Value::String(t.clone()),
                NoG::StringBuffer(u) => Value::StringBuffer(u.clone()),
                NoG::RegExp(t) => Value::RegExp(t.clone()),
                NoG::Match(t) => Value::Match(t.clone()),
                NoG::Object { class_id, fields } => Value::Object { class_id: *class_id, fields: vec![(0, false); fields.len()] },
                NoG::Cell(_) => Value::Cell(TaggedValue::scalar(0)),
                NoG::Environment(v) => Value::Environment(vec![TaggedValue::scalar(0); v.len()]),
                NoG::Closure { code_id, .. } => Value::Closure { code_id: *code_id, environment: 0 },
                NoG::List { itens, .. } => Value::List(vec![TaggedValue::scalar(0); itens.len()]),
                NoG::Map(es, _) => Value::Map(Vec::with_capacity(es.len())),
                NoG::Set(v, _) => Value::Set(Vec::with_capacity(v.len())),
                NoG::Record(v) => Value::Record(vec![TaggedValue::scalar(0); v.len()]),
                NoG::BoxedInt(x) => Value::BoxedInt(*x),
                NoG::BoxedDouble(x) => Value::BoxedDouble(*x),
                NoG::BoxedBool(b) => {
                    let h = heap.caixa_bool(*b);
                    heap.set_root(frame, i, h);
                    handles.push(h);
                    continue;
                }
            };
            let h = heap.allocate(vazio);
            heap.set_root(frame, i, h);
            handles.push(h);
        }
        let t = |v: &ValG| -> TaggedValue {
            match *v {
                ValG::Bits(bits, is_ref, tag) => TaggedValue { bits, is_ref, tag },
                ValG::No(i) => TaggedValue::reference(handles[i]),
                ValG::Mesmo(h) if mesmo => TaggedValue::reference(h),
                // Fora do isolado de origem não há o que compartilhar: o
                // emissor copia (`compartilhar` falso) para outro isolado.
                ValG::Mesmo(_) => TaggedValue::reference(0),
            }
        };
        for (i, (no, meta)) in g.nos.iter().enumerate() {
            let h = handles[i];
            match no {
                NoG::Object { fields, .. } => {
                    let novos: Vec<(i64, bool)> = fields.iter().map(|f| { let x = t(f); (x.bits, x.is_ref) }).collect();
                    if let Value::Object { fields: fs, .. } = heap.get_mut(h) {
                        *fs = novos;
                    }
                }
                NoG::Cell(v) => {
                    let x = t(v);
                    *heap.get_mut(h) = Value::Cell(x);
                }
                NoG::Environment(vs) => {
                    let x: Vec<TaggedValue> = vs.iter().map(t).collect();
                    *heap.get_mut(h) = Value::Environment(x);
                }
                NoG::Closure { code_id, environment } => {
                    let e = t(environment).bits;
                    *heap.get_mut(h) = Value::Closure { code_id: *code_id, environment: e };
                }
                NoG::List { itens, fixa, imutavel, pendente } => {
                    let x: Vec<TaggedValue> = itens.iter().map(t).collect();
                    *heap.get_mut(h) = Value::List(x);
                    if *fixa {
                        heap.fixas.insert(h);
                    }
                    if *imutavel {
                        heap.imutaveis.insert(h);
                    }
                    if let Some(n) = pendente {
                        heap.pendentes.insert(h, *n);
                    }
                }
                NoG::Map(es, imutavel) => {
                    let x: Vec<(TaggedValue, TaggedValue)> = es.iter().map(|(a, b)| (t(a), t(b))).collect();
                    *heap.get_mut(h) = Value::Map(x);
                    if *imutavel {
                        heap.imutaveis.insert(h);
                    }
                }
                NoG::Set(vs, imutavel) => {
                    let x: Vec<TaggedValue> = vs.iter().map(t).collect();
                    *heap.get_mut(h) = Value::Set(x);
                    if *imutavel {
                        heap.imutaveis.insert(h);
                    }
                }
                NoG::Record(vs) => {
                    let x: Vec<TaggedValue> = vs.iter().map(t).collect();
                    *heap.get_mut(h) = Value::Record(x);
                }
                _ => {}
            }
            if *meta != 0 && !matches!(no, NoG::BoxedBool(_)) {
                heap.set_metadado(h, *meta);
            }
        }
        let r = t(&g.raiz);
        let resultado = if r.is_ref { r.bits } else { heap.como_ref(r) };
        heap.pop_frame(frame);
        resultado
    })
}

// ---------------------------------------------------------------------------
// O registro das portas e a fila do isolado.

/// Uma mensagem na fila de um isolado.
struct Mensagem {
    porta: i64,
    grafo: Grafo,
    chegada: std::time::Instant,
}

/// A fila de mensagens de um isolado; qualquer thread posta nela.
pub struct FilaDoIsolado {
    id: u64,
    mensagens: std::sync::Mutex<std::collections::VecDeque<Mensagem>>,
    sinal: std::sync::Condvar,
}

/// Um serviço nativo: recebe (porta, grafo) na thread de quem enviou.
pub type ServicoNativo = std::sync::Arc<dyn Fn(i64, Grafo) + Send + Sync>;

enum Dono {
    Isolado(std::sync::Arc<FilaDoIsolado>),
    Nativo(ServicoNativo),
}

fn registro() -> &'static std::sync::Mutex<std::collections::HashMap<i64, Dono>> {
    static R: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<i64, Dono>>> = std::sync::OnceLock::new();
    R.get_or_init(Default::default)
}

fn proximo_id_de_porta() -> i64 {
    static PROX: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);
    PROX.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

thread_local! {
    static FILA: std::sync::Arc<FilaDoIsolado> = {
        static PROX: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        std::sync::Arc::new(FilaDoIsolado {
            id: PROX.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            mensagens: std::sync::Mutex::new(std::collections::VecDeque::new()),
            sinal: std::sync::Condvar::new(),
        })
    };
    /// Portas abertas deste isolado → se mantêm o isolado vivo.
    static PORTAS_ABERTAS: RefCell<std::collections::HashMap<i64, bool>> = RefCell::new(std::collections::HashMap::new());
    /// A mensagem em despacho: (porta, valor), o valor enraizado.
    static ATUAL: RefCell<(i64, i64)> = const { RefCell::new((0, 0)) };
    /// O despachante Dart (`_RawReceivePort._despachar`), enraizado.
    static DESPACHANTE: RefCell<i64> = const { RefCell::new(0) };
}

/// Raízes globais do runtime para as portas (ids negativos fora da faixa
/// dos timers de `eventos.rs`).
const RAIZ_DESPACHANTE: i64 = -0x7000_0000_0000;
const RAIZ_MENSAGEM: i64 = -0x7000_0000_0001;

fn id_do_isolado() -> u64 {
    FILA.with(|f| f.id)
}

/// Posta `grafo` para a porta `porta`: na fila do isolado dono, ou no
/// serviço nativo. Porta fechada ou inexistente: a mensagem é descartada,
/// como na VM.
pub fn postar(porta: i64, grafo: Grafo) {
    let dono = {
        let r = registro().lock().unwrap_or_else(|e| e.into_inner());
        match r.get(&porta) {
            Some(Dono::Isolado(f)) => Some(Dono::Isolado(f.clone())),
            Some(Dono::Nativo(s)) => Some(Dono::Nativo(s.clone())),
            None => None,
        }
    };
    match dono {
        Some(Dono::Isolado(f)) => {
            let mut m = f.mensagens.lock().unwrap_or_else(|e| e.into_inner());
            m.push_back(Mensagem { porta, grafo, chegada: std::time::Instant::now() });
            f.sinal.notify_one();
        }
        Some(Dono::Nativo(s)) => s(porta, grafo),
        None => {}
    }
}

/// Abre uma porta nativa atendida por `servico` e devolve o id.
pub fn abrir_porta_nativa(servico: ServicoNativo) -> i64 {
    let id = proximo_id_de_porta();
    registro().lock().unwrap_or_else(|e| e.into_inner()).insert(id, Dono::Nativo(servico));
    id
}

/// Se o isolado tem porta aberta que o mantém vivo.
fn tem_porta_viva() -> bool {
    PORTAS_ABERTAS.with(|p| p.borrow().values().any(|&v| v))
}

/// A chegada da mensagem mais antiga da fila, se houver.
fn chegada_da_proxima() -> Option<std::time::Instant> {
    FILA.with(|f| f.mensagens.lock().unwrap_or_else(|e| e.into_inner()).front().map(|m| m.chegada))
}

/// Espera uma mensagem até `prazo` (ou sem prazo). Devolve se chegou.
fn esperar_mensagem(prazo: Option<std::time::Instant>) -> bool {
    FILA.with(|f| {
        let mut m = f.mensagens.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if !m.is_empty() {
                return true;
            }
            match prazo {
                None => m = f.sinal.wait(m).unwrap_or_else(|e| e.into_inner()),
                Some(p) => {
                    let agora = std::time::Instant::now();
                    if agora >= p {
                        return false;
                    }
                    m = f.sinal.wait_timeout(m, p - agora).unwrap_or_else(|e| e.into_inner()).0;
                }
            }
        }
    })
}

/// Tira a próxima mensagem, materializa e despacha pelo `chamar` do código
/// gerado. Devolve se havia mensagem.
fn despachar_proxima(chamar: extern "C" fn(i64) -> i64) -> bool {
    let Some(m) = FILA.with(|f| f.mensagens.lock().unwrap_or_else(|e| e.into_inner()).pop_front()) else {
        return false;
    };
    // Mensagem para porta já fechada: descartada, como na VM.
    if !PORTAS_ABERTAS.with(|p| p.borrow().contains_key(&m.porta)) {
        return true;
    }
    let despachante = DESPACHANTE.with(|d| *d.borrow());
    if despachante == 0 {
        return true;
    }
    let valor = materializar(&m.grafo);
    HEAP.with(|h| h.borrow_mut().set_global_root(RAIZ_MENSAGEM, valor));
    ATUAL.with(|a| *a.borrow_mut() = (m.porta, valor));
    chamar(despachante);
    ATUAL.with(|a| *a.borrow_mut() = (0, 0));
    HEAP.with(|h| h.borrow_mut().set_global_root(RAIZ_MENSAGEM, 0));
    true
}

// ---------------------------------------------------------------------------
// Natives da sobreposição (`sdk_nativo/isolate/isolate_patch.dart`).

/// `DartForge_porta_abrir(porta)`: abre uma porta deste isolado para o
/// objeto `porta` (um `_RawReceivePort`, cuja classe passa a ser não
/// enviável) e devolve o id.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_porta_abrir(objeto: i64) -> i64 {
    let classe = HEAP.with(|h| match h.borrow().try_get(objeto) {
        Some(Value::Object { class_id, .. }) => Some(*class_id),
        _ => None,
    });
    if let Some(c) = classe {
        NAO_ENVIAVEIS.with(|n| {
            n.borrow_mut().entry(c).or_insert_with(|| "(object is a ReceivePort)\n".to_string());
        });
    }
    let id = proximo_id_de_porta();
    let fila = FILA.with(|f| f.clone());
    registro().lock().unwrap_or_else(|e| e.into_inner()).insert(id, Dono::Isolado(fila));
    PORTAS_ABERTAS.with(|p| p.borrow_mut().insert(id, true));
    id
}

/// `DartForge_porta_fechar(id)`: fecha a porta; mensagens pendentes para
/// ela são descartadas no despacho.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_porta_fechar(id: i64) {
    registro().lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
    PORTAS_ABERTAS.with(|p| p.borrow_mut().remove(&id));
}

/// `DartForge_porta_manter_vivo(id, manter)`: `keepIsolateAlive`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_porta_manter_vivo(id: i64, manter: u8) {
    PORTAS_ABERTAS.with(|p| {
        if let Some(v) = p.borrow_mut().get_mut(&id) {
            *v = manter != 0;
        }
    });
}

/// `DartForge_porta_mantem_vivo(id)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_porta_mantem_vivo(id: i64) -> u8 {
    PORTAS_ABERTAS.with(|p| u8::from(p.borrow().get(&id).copied().unwrap_or(false)))
}

/// `DartForge_porta_enviar(id, mensagem)`: copia e posta. Devolve 1 se a
/// mensagem tem objeto não enviável (a descrição fica em
/// `DartForge_porta_recusa`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_porta_enviar(id: i64, mensagem: i64) -> u8 {
    let mesmo = {
        let r = registro().lock().unwrap_or_else(|e| e.into_inner());
        let fila = FILA.with(|f| f.id);
        matches!(r.get(&id), Some(Dono::Isolado(f)) if f.id == fila)
    };
    match copiar_para_grafo(mensagem, mesmo) {
        Ok(g) => {
            postar(id, g);
            0
        }
        Err(MensagemIlegal(d)) => {
            ULTIMA_RECUSA.with(|u| *u.borrow_mut() = d);
            1
        }
    }
}

/// `DartForge_porta_recusa()`: por que a última mensagem foi recusada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_porta_recusa() -> i64 {
    let d = ULTIMA_RECUSA.with(|u| u.borrow().clone());
    alocar_str(&d)
}

/// `DartForge_portas_despachante(f)`: a closure sem argumentos que o laço
/// chama para cada mensagem (`_RawReceivePort._despachar`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_portas_despachante(f: i64) {
    HEAP.with(|h| h.borrow_mut().set_global_root(RAIZ_DESPACHANTE, f));
    DESPACHANTE.with(|d| *d.borrow_mut() = f);
}

/// `DartForge_porta_atual()`: a porta da mensagem em despacho.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_porta_atual() -> i64 {
    ATUAL.with(|a| a.borrow().0)
}

/// `DartForge_mensagem_atual()`: a mensagem em despacho.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_mensagem_atual() -> i64 {
    ATUAL.with(|a| a.borrow().1)
}

/// `DartForge_classe_nao_enviavel(objeto)`: a classe de `objeto` não vai
/// numa mensagem (`@pragma('vm:isolate-unsendable')` da VM). A descrição é a
/// da VM sem o sufixo de biblioteca privada e sem o caminho de retenção.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_classe_nao_enviavel(objeto: i64) {
    let classe = HEAP.with(|h| match h.borrow().try_get(objeto) {
        Some(Value::Object { class_id, .. }) => Some(*class_id),
        _ => None,
    });
    let Some(c) = classe else { return };
    let nome = nome_da_classe(c);
    NAO_ENVIAVEIS.with(|n| {
        n.borrow_mut().entry(c).or_insert_with(|| {
            format!(
                "object is unsendable - Library:'dart:isolate' Class: {nome} (see restrictions listed at `SendPort.send()` documentation for more information)\n"
            )
        });
    });
}

/// `DartForge_capacidade_nova()`: um id de capacidade único no processo.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_capacidade_nova() -> i64 {
    static PROX: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);
    PROX.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// O nome registrado da classe `c` (vazio se não houver).
fn nome_da_classe(c: i64) -> String {
    CLASS_NAMES.with(|m| m.borrow().get(&c).cloned()).unwrap_or_default()
}
