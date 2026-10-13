// O rastro no formato da VM (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
// §13.14): a tabela endereço → (função, linha, coluna) que cada imagem
// compilada com `--trace=simbolico` registra na partida, os endereços de
// retorno guardados no `throw` e a simbolização, feita só quando o texto é
// pedido (`toString`, a exceção não capturada).
//
// A entrada da tabela (`emit_native/src/llvm/trace.rs`) tem 12 bytes: o
// rótulo e o registro da função, cada um como deslocamento a partir do
// próprio campo, e uma palavra. Os dois bits baixos do campo do registro (o
// registro é alinhado em 4) dizem a espécie: 0, um ponto de chamada (a
// palavra é `line << 12 | column`; as entradas com o mesmo rótulo são os
// quadros embutidos, de dentro para fora); 1, a identidade de uma função
// pela entrada uniforme das closures dela (a palavra é o token); 2, o
// `await` de cada estado de um corpo `async`, na ordem; 3, o elo de uma
// closure com quem espera (`posição << 2 | direct << 1 | célula`). O
// registro da função é o deslocamento até a url, um byte de marcas e o
// nome, os textos terminados em zero.
//
// **A cadeia de quem espera** (o `AsyncAwareStackUnwinder` da VM,
// `runtime/vm/stack_trace.cc`). O corpo `async` retomado e o
// `_FutureListener.handleValue` empurram o quadro ou o ouvinte na pilha do
// rastro (`dartforge_rastro_entrar`); no `throw`, o percurso dos quadros de
// máquina casa cada quadro marcado com a entrada dele e, achado o primeiro
// corpo `async` já suspenso (ou o ouvinte), segue a cadeia pelo heap:
// quadro → `Completer` → `_Future` → `_FutureListener` → a closure que
// espera, pelos elos das capturas marcadas `@pragma('vm:awaiter-link')`, até
// o quadro do próximo corpo suspenso. Cada um sai como
// `<asynchronous suspension>` seguido do quadro dele, na posição do `await`.
// Os ramos de stream também (`UnwindFrameToStreamController`): o corpo
// `async*` vai ao controlador dele (`_AsyncStarStreamController.controller`),
// e o controlador inscrito, ao `onData` da inscrição; o `await for` segue o
// `_Future` do `moveNext` pelo `_StreamIterator` do tear-off `_onData`, e o
// `yield*`, o `asyncStarBody` do controlador de destino pelo `addStreamFuture`.
//
// Fragmento do runtime: sem `use` (os fragmentos são um programa só).
// Escrito sem compilar nem executar (2026-10-05).

/// As seções do rastro registradas, `(começo, end)`.
static TRACE_SECTIONS: std::sync::Mutex<Vec<(usize, usize)>> = std::sync::Mutex::new(Vec::new());
/// Alguma imagem registrou a tabela: o `throw` guarda os endereços.
static HAS_TRACE_TABLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// O que as seções dizem, e a geração das seções com que foi montado:
/// montado no primeiro pedido, refeito quando uma seção entra ou sai.
static TRACE_INDEX: std::sync::RwLock<(usize, TraceIndex)> =
    std::sync::RwLock::new((0, TraceIndex { points: Vec::new(), functions: Vec::new() }));
/// A geração das seções: cresce a cada registro e a cada retirada (o JIT
/// retira a seção do objeto que solta). Começa em 1: o índice vazio é a
/// geração 0.
static TRACE_GENERATION: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
/// Os campos do `dart:async` que a cadeia de quem espera lê:
/// `(id da class, "Class.field", posição)`.
static TRACE_FIELDS: std::sync::Mutex<Vec<(i64, String, usize)>> = std::sync::Mutex::new(Vec::new());

/// Quantos quadros o rastro guarda (os pares ficam em campos de um objeto,
/// até 2¹⁶ por objeto).
const FRAMES_IN_TRACE: usize = 1024;

/// O texto de hoje, sem tabela ou sem quadro Dart.
const TRACE_WITHOUT_TABLE: &str = "#0      main (dart:native)\n";

/// As marcas do registro de uma função (`emit_native/src/hir.rs`,
/// `marcas_do_rastro`).
const ASYNC_BODY_MARK: u8 = 1;
const HIDDEN_MARK: u8 = 2;
const LISTEN_MARK: u8 = 4;
/// O corpo é de um gerador `async*` (com [`ASYNC_BODY_MARK`]): o quadro
/// dele na pilha já foi suspenso (`WasPreviouslySuspended` do gerador).
const ASYNC_GENERATOR_MARK: u8 = 8;

/// Os estados de `_StreamController` e de `_BufferingStreamSubscription`
/// (`stream_controller.dart`, `stream_impl.dart`).
const CONTROLLER_SUBSCRIBED: i64 = 1;
const SUBSCRIPTION_MASK: i64 = 3;
const CONTROLLER_ADD_STREAM: i64 = 8;
const SUBSCRIPTION_WITH_ERROR: i64 = 1 << 5;

/// As espécies de tear-off que o percurso reconhece (o `Owner` da função
/// implícita na VM): o `_onData` de um `_StreamIterator` e o `_add` de um
/// `_StreamController`.
const ITERATOR_ON_DATA_TEAROFF: i64 = 1;
const CONTROLLER_ADD_TEAROFF: i64 = 2;

/// As entradas dos tear-offs reconhecidos: `(input, espécie)`.
static TRACE_TEAROFFS: std::sync::Mutex<Vec<(usize, i64)>> = std::sync::Mutex::new(Vec::new());

/// O id de classe do quadro de uma função `async` (`lower/async_sm.rs`,
/// `ASYNC_FRAME_ID`): `[estado, completer, body, …]`.
const ASYNC_FRAME_ID: i64 = 0x3FFF_FF00;

/// Os estados de `_FutureListener` (`future_impl.dart`).
const LISTENER_THEN: i64 = 1;
const LISTENER_CATCH_ERROR: i64 = 2;
const LISTENER_WHEN_COMPLETE: i64 = 8;
const LISTENER_AWAIT: i64 = 16;

/// A distância máxima entre o endereço gravado por
/// `dartforge_rastro_entrar` (um local dela) e o topo da pilha do quadro que
/// a chamou.
const STACK_WINDOW: usize = 16 * 1024;

/// Os pares guardados de um quadro da cadeia de quem espera (os de um quadro
/// síncrono são `(ret, começo)`, com os bits altos limpos).
const PAIR_WAIT: u64 = 1 << 62;
const PAIR_LISTENER: u64 = 1 << 61;

/// Um ponto de chamada, já com os endereços absolutos.
#[derive(Clone, Copy)]
struct TracePoint {
    label_name: usize,
    rec: usize,
    line_column: u32,
}

/// Uma função conhecida pela entrada uniforme das closures dela.
#[derive(Clone, Default)]
struct TraceFunction {
    rec: usize,
    token: u32,
    /// A posição do `await` de cada estado (o índice `k − 1`).
    waits: Vec<u32>,
    /// O elo com quem espera: `(posição, direct, célula)`.
    link: Option<(usize, bool, bool)>,
}

/// As seções lidas.
struct TraceIndex {
    /// Os pontos de chamada pelo rótulo; os do mesmo rótulo, na ordem da
    /// seção, são a cadeia de quadros embutidos.
    points: Vec<TracePoint>,
    /// As funções pela entrada, em ordem.
    functions: Vec<(usize, TraceFunction)>,
}

impl TraceIndex {
    /// A cadeia do quadro de `ret` (o maior rótulo abaixo dele, dentro da
    /// função que começa em `start`), de dentro para fora; vazia sem ponto.
    fn chain(&self, ret: u64, start: u64) -> &[TracePoint] {
        let r = ret as usize;
        let k = self.points.partition_point(|p| p.label_name < r);
        let Some(last_one) = k.checked_sub(1) else { return &[] };
        let label_name = self.points[last_one].label_name;
        // A função do quadro: o rótulo tem de estar nela. Sem o começo (o
        // desenrolador não o deu), só um rótulo próximo.
        if (start != 0 && label_name < start as usize) || (start == 0 && r - label_name > 1 << 16) {
            return &[];
        }
        let mut first = last_one;
        while first > 0 && self.points[first - 1].label_name == label_name {
            first -= 1;
        }
        &self.points[first..=last_one]
    }

    fn function(&self, input: usize) -> Option<&TraceFunction> {
        self.functions.binary_search_by_key(&input, |(e, _)| *e).ok().map(|i| &self.functions[i].1)
    }
}

/// Um quadro do rastro guardado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TraceFrame {
    /// O endereço de retorno e o começo da função.
    Sync(u64, u64),
    /// `<asynchronous suspension>`.
    Gap,
    /// Quem espera num `await`: a entrada do corpo e o estado.
    Wait(u64, i64),
    /// A closure que escuta um `Future`: a entrada dela.
    Listener(u64),
}

fn frame_pair(q: TraceFrame) -> (u64, u64) {
    match q {
        TraceFrame::Sync(r, f) => (r, f),
        TraceFrame::Gap => (0, 0),
        TraceFrame::Wait(e, k) => (PAIR_WAIT | (k as u64 & (PAIR_LISTENER - 1)), e),
        TraceFrame::Listener(e) => (PAIR_LISTENER, e),
    }
}

fn pair_frame((a, b): (u64, u64)) -> TraceFrame {
    if a == 0 && b == 0 {
        TraceFrame::Gap
    } else if a & PAIR_WAIT != 0 {
        TraceFrame::Wait(b, (a & !PAIR_WAIT) as i64)
    } else if a == PAIR_LISTENER {
        TraceFrame::Listener(b)
    } else {
        TraceFrame::Sync(a, b)
    }
}

thread_local! {
    /// Os pares do último `throw` desta thread (veja [`frame_pair`]), de
    /// dentro para fora; vazio sem tabela.
    static THROW_RETURNS: std::cell::RefCell<Vec<(u64, u64)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// A pilha do rastro: `(tipo, value, endereço)` empurrado por
    /// `dartforge_rastro_entrar` — o quadro de um corpo `async` (tipo 0) ou
    /// o ouvinte de `handleValue` (tipo 1) e um endereço da pilha de máquina
    /// logo abaixo do quadro que chamou.
    static TRACE_STACK: std::cell::RefCell<Vec<(i64, i64, usize)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// O registro da tabela do rastro de uma imagem: o módulo compilado com
/// `--trace=simbolico` chama isto na partida com o começo e o fim da seção
/// (`dfpcl`; as sentinelas `.dfpcl$a`/`$z` no COFF). Idempotente por seção;
/// não aloca no heap do coletor nem lança.
///
/// # Safety
/// `[start, end)` é a seção do rastro de uma imagem carregada, que fica
/// carregada até o fim do processo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_rastro(start: *const u8, end: *const u8) {
    let (start, end) = (start as usize, end as usize);
    if start == 0 || end <= start {
        return;
    }
    let mut sections = TRACE_SECTIONS.lock().unwrap_or_else(|e| e.into_inner());
    if sections.iter().any(|&(a, _)| a == start) {
        return;
    }
    sections.push((start, end));
    TRACE_GENERATION.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    HAS_TRACE_TABLE.store(true, std::sync::atomic::Ordering::Release);
}

/// Retira a seção do rastro registrada com `start` (o objeto do JIT que a
/// sessão solta, numa recarga). Não aloca no heap do coletor nem lança.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_desregistrar_rastro(start: *const u8) {
    let start = start as usize;
    let mut sections = TRACE_SECTIONS.lock().unwrap_or_else(|e| e.into_inner());
    let before = sections.len();
    sections.retain(|&(a, _)| a != start);
    if sections.len() != before {
        TRACE_GENERATION.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    }
}

/// A posição de um campo do `dart:async` que a cadeia de quem espera lê
/// (`"Class.field"`, `n` bytes em `name`). Idempotente; não aloca no heap
/// do coletor nem lança.
///
/// # Safety
/// `name` aponta `n` bytes UTF-8 válidos.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_campo_do_rastro(class: i64, name: *const u8, n: i64, cur_position: i64) {
    let (Ok(n), Ok(cur_position)) = (usize::try_from(n), usize::try_from(cur_position)) else { return };
    if name.is_null() {
        return;
    }
    // SAFETY: o contrato da função.
    let bytes = unsafe { std::slice::from_raw_parts(name, n) };
    let Ok(name) = std::str::from_utf8(bytes) else { return };
    let mut fields = TRACE_FIELDS.lock().unwrap_or_else(|e| e.into_inner());
    if !fields.iter().any(|(c, x, _)| *c == class && x == name) {
        fields.push((class, name.to_string(), cur_position));
    }
}

/// A entrada do tear-off de espécie `kind` (o `_onData` de
/// `_StreamIterator`, o `_add` de `_StreamController`) que a imagem tem.
/// Idempotente; não aloca no heap do coletor nem lança.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_registrar_tearoff_do_rastro(input: *const u8, kind: i64) {
    let input = input as usize;
    if input == 0 {
        return;
    }
    let mut v = TRACE_TEAROFFS.lock().unwrap_or_else(|e| e.into_inner());
    if !v.iter().any(|&(e, _)| e == input) {
        v.push((input, kind));
    }
}

/// Empurra na pilha do rastro o quadro de um corpo `async` (`tipo` 0) ou o
/// ouvinte de `_FutureListener.handleValue` (`tipo` 1) na entrada da função;
/// devolve a profundidade, que a saída passa a [`dartforge_rastro_sair`].
#[unsafe(no_mangle)]
#[inline(never)]
pub extern "C" fn dartforge_rastro_entrar(tipo: i64, value: i64) -> i64 {
    let mark = std::hint::black_box(0u8);
    let address = std::hint::black_box(std::ptr::addr_of!(mark)) as usize;
    TRACE_STACK.with(|p| {
        let mut p = p.borrow_mut();
        p.push((tipo, value, address));
        p.len() as i64
    })
}

/// A saída da função que entrou com `depth`: corta a pilha do rastro
/// ali (o que funções desenroladas por exceção deixaram em cima cai junto).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rastro_sair(depth: i64) {
    let n = usize::try_from(depth.saturating_sub(1)).unwrap_or(0);
    TRACE_STACK.with(|p| p.borrow_mut().truncate(n));
}

/// Alguma imagem registrou a tabela do rastro.
fn has_trace_table() -> bool {
    HAS_TRACE_TABLE.load(std::sync::atomic::Ordering::Acquire)
}

/// Os pares da pilha daqui para fora (os quadros síncronos e a cadeia de
/// quem espera), com tabela; vazio sem.
fn capture_returns() -> Vec<(u64, u64)> {
    if !has_trace_table() {
        return Vec::new();
    }
    let frames = crate::heap::return_addresses(FRAMES_IN_TRACE);
    let stack = TRACE_STACK.with(|p| p.borrow().clone());
    // Sem nada na pilha do rastro, nenhum quadro casa: só os síncronos (sem
    // montar o índice no `throw`).
    if stack.is_empty() {
        return frames.into_iter().map(|(r, f, _)| (r, f)).collect();
    }
    let frames = with_trace_index(|ix| heap_without_borrow(|heap| unwind(ix, heap, &frames, &stack)));
    frames.into_iter().map(frame_pair).collect()
}

/// O `throw`: guarda os pares desta pilha.
fn store_throw_returns() {
    if !has_trace_table() {
        return;
    }
    let returns = capture_returns();
    THROW_RETURNS.with(|r| *r.borrow_mut() = returns);
}

/// Os pares do último `throw` desta thread (vazio sem tabela).
fn throw_returns() -> Vec<(u64, u64)> {
    THROW_RETURNS.with(|r| r.borrow().clone())
}

/// Esquece os pares do último `throw` (a exceção foi tratada).
fn forget_throw_returns() {
    THROW_RETURNS.with(|r| r.borrow_mut().clear());
}

// --- a cadeia de quem espera (`AsyncAwareStackUnwinder`) ---------------------

/// O quadro de quem espera em construção (o `AwaiterFrame` da VM): a closure
/// e o próximo objeto da cadeia (um quadro de corpo `async`, um `Completer`,
/// um `_Future`).
#[derive(Default)]
struct Waiter {
    closure: Option<i64>,
    next_offset: Option<i64>,
}

/// O nome da classe de `h`, se é objeto.
fn class_of(heap: &crate::heap::Heap, h: i64) -> Option<String> {
    let cid = heap.object_class(h)?;
    CLASS_NAMES.with(|m| m.borrow().get(&cid).cloned())
}

/// O campo `field` do objeto `h` (a posição registrada para a classe dele).
fn trace_field(heap: &crate::heap::Heap, h: i64, field: &str) -> Option<crate::heap::Field> {
    let cid = heap.object_class(h)?;
    let cur_position = {
        let fields = TRACE_FIELDS.lock().unwrap_or_else(|e| e.into_inner());
        fields.iter().find(|(c, name, _)| *c == cid && name.rsplit('.').next() == Some(field)).map(|(_, _, p)| *p)?
    };
    heap.object(h)?.get(cur_position)
}

/// O campo de referência (não nulo).
fn trace_ref(heap: &crate::heap::Heap, h: i64, field: &str) -> Option<i64> {
    match trace_field(heap, h, field)? {
        (r, true) if r != 0 => Some(r),
        _ => None,
    }
}

/// Um `int` guardado num campo (cru ou em caixa).
fn field_int(heap: &crate::heap::Heap, c: crate::heap::Field) -> Option<i64> {
    match c {
        (b, false) => Some(b),
        (r, true) => heap.int_of(r),
    }
}

fn is_async_frame(heap: &crate::heap::Heap, h: i64) -> bool {
    heap.object_class(h) == Some(ASYNC_FRAME_ID)
}

/// O estado do quadro de um corpo `async` (0 antes do primeiro `await`).
fn frame_state(heap: &crate::heap::Heap, q: i64) -> i64 {
    heap.object(q).and_then(|o| o.get(0)).and_then(|c| field_int(heap, c)).unwrap_or(0)
}

/// As marcas do registro de função `r`.
#[allow(unsafe_code)]
fn record_marks(r: usize) -> u8 {
    // SAFETY: o registro de uma entrada de uma seção registrada (o byte das
    // marcas segue o deslocamento até a url).
    unsafe { ((r + 4) as *const u8).read() }
}

/// O elo da closure `c` com quem espera (`TryGetAwaiterLink`): `None` se a
/// função dela não tem elo; `Some(None)` se o elo é nulo.
fn closure_link(ix: &TraceIndex, heap: &crate::heap::Heap, c: i64) -> Option<Option<i64>> {
    let clo = heap.closure(c)?;
    let f = ix.function(clo.code as usize)?;
    let value = if record_marks(f.rec) & ASYNC_BODY_MARK != 0 {
        // A closure do corpo: o quadro na posição 0 do ambiente.
        heap.object(clo.contexto.0)?.get(0)?
    } else {
        let (cur_position, direct, cell) = f.link?;
        let v = if direct { clo.contexto } else { heap.object(clo.contexto.0)?.get(cur_position)? };
        if cell && v.1 { heap.object(v.0).and_then(|o| o.get(0)).unwrap_or((0, false)) } else { v }
    };
    Some((value.1 && value.0 != 0).then_some(value.0))
}

/// `ComputeNextFrameFromAwaiterLink` (com `FollowAwaiterLinks`): segue os
/// elos das closures enquanto o elo é outra closure; o último elo que não é
/// closure passa a ser o próximo.
fn follow_links(ix: &TraceIndex, heap: &crate::heap::Heap, q: &mut Waiter) {
    let mut link: Option<i64> = None;
    let mut steps = 0;
    while let Some(c) = q.closure {
        link = None;
        let Some(l) = closure_link(ix, heap, c) else { break };
        link = l;
        match l {
            Some(x) if heap.closure(x).is_some() && steps < 64 => {
                q.closure = Some(x);
                steps += 1;
            }
            _ => break,
        }
    }
    if let Some(x) = link
        && heap.closure(x).is_none()
    {
        q.next_offset = Some(x);
    }
}

/// `InitializeAwaiterFrameFromFutureListener`.
fn start_from_listener(ix: &TraceIndex, heap: &crate::heap::Heap, q: &mut Waiter, listener: i64) {
    if class_of(heap, listener).as_deref() != Some("_FutureListener") {
        *q = Waiter::default();
        return;
    }
    let estado = trace_field(heap, listener, "state").and_then(|c| field_int(heap, c)).unwrap_or(-1);
    let result = if [
        LISTENER_THEN,
        LISTENER_CATCH_ERROR,
        LISTENER_WHEN_COMPLETE,
        LISTENER_THEN | LISTENER_CATCH_ERROR,
        LISTENER_THEN | LISTENER_CATCH_ERROR | LISTENER_AWAIT,
    ]
    .contains(&estado)
    {
        trace_ref(heap, listener, "result")
    } else {
        None
    };
    q.closure = trace_ref(heap, listener, "callback").filter(|&c| heap.closure(c).is_some());
    q.next_offset = result;
    follow_links(ix, heap, q);
}

/// `UnwindFrameToFutureListener`: o primeiro ouvinte do `_Future` do próximo.
fn up_to_listener(ix: &TraceIndex, heap: &crate::heap::Heap, q: &mut Waiter) {
    let listener = q.next_offset.and_then(|f| trace_ref(heap, f, "_resultOrListeners"));
    match listener {
        Some(l) if class_of(heap, l).as_deref() == Some("_FutureListener") => start_from_listener(ix, heap, q, l),
        _ => *q = Waiter::default(),
    }
}

/// Um `bool` guardado num campo (cru ou em caixa).
fn field_bool(heap: &crate::heap::Heap, c: crate::heap::Field) -> Option<bool> {
    match c {
        (b, false) => Some(b != 0),
        (r, true) => heap.bool_of(r),
    }
}

/// O campo inteiro `field` de `h`.
fn trace_int(heap: &crate::heap::Heap, h: i64, field: &str) -> Option<i64> {
    trace_field(heap, h, field).and_then(|c| field_int(heap, c))
}

/// A espécie e o receptor de um tear-off reconhecido.
fn tearoff_of(heap: &crate::heap::Heap, c: i64) -> Option<(i64, Option<i64>)> {
    let clo = heap.closure(c)?;
    let kind = {
        let v = TRACE_TEAROFFS.lock().unwrap_or_else(|e| e.into_inner());
        v.iter().find(|&&(e, _)| e as i64 == clo.code).map(|&(_, k)| k)?
    };
    // O ambiente do tear-off guarda o receptor na posição 0.
    let receiver_obj = if clo.contexto.1 {
        heap.object(clo.contexto.0).and_then(|o| o.get(0)).and_then(|v| (v.1 && v.0 != 0).then_some(v.0))
    } else {
        None
    };
    Some((kind, receiver_obj))
}

/// `FollowAwaiterLinks`: segue os elos de `closure` enquanto o elo é outra
/// closure; devolve o último elo (que não é closure), ou nada.
fn follow_links_from(ix: &TraceIndex, heap: &crate::heap::Heap, closure: &mut Option<i64>) -> Option<i64> {
    let mut link: Option<i64> = None;
    let mut steps = 0;
    while let Some(c) = *closure {
        link = None;
        let Some(l) = closure_link(ix, heap, c) else { break };
        link = l;
        match l {
            Some(x) if heap.closure(x).is_some() && steps < 64 => {
                *closure = Some(x);
                steps += 1;
            }
            _ => break,
        }
    }
    link.filter(|&x| heap.closure(x).is_none())
}

/// A função da closure `c` tem elo com quem espera (`HasAwaiterLink`).
fn has_link(ix: &TraceIndex, heap: &crate::heap::Heap, c: Option<i64>) -> bool {
    c.is_some_and(|c| closure_link(ix, heap, c).is_some())
}

/// `UnwindFrameToStreamController`: o controlador em `q.next_offset`.
fn up_to_controller(ix: &TraceIndex, heap: &crate::heap::Heap, q: &mut Waiter) {
    let Some(controller) = q.next_offset else {
        *q = Waiter::default();
        return;
    };
    *q = Waiter::default();
    let estado = trace_int(heap, controller, "_state").unwrap_or(0);
    if estado & SUBSCRIPTION_MASK != CONTROLLER_SUBSCRIBED {
        return;
    }
    let mut subscription = trace_ref(heap, controller, "_varData");
    if estado & CONTROLLER_ADD_STREAM != 0 {
        subscription = subscription.and_then(|s| trace_ref(heap, s, "_varData"));
    }
    let Some(subscription) = subscription else { return };
    let mut closure = trace_ref(heap, subscription, "_onData").filter(|&c| heap.closure(c).is_some());
    let subscription_state = trace_int(heap, subscription, "_state").unwrap_or(0);
    // O tear-off do `_StreamIterator._onData` (o `await for`) e o do
    // `_StreamController._add` (o `yield*`).
    if let Some((kind, receiver_obj)) = closure.and_then(|c| tearoff_of(heap, c)) {
        if kind == ITERATOR_ON_DATA_TEAROFF {
            let Some(iterator) = receiver_obj.filter(|&r| class_of(heap, r).as_deref() == Some("_StreamIterator")) else { return };
            // Com `_hasValue`, o `_stateData` é o valor: não segue.
            if trace_field(heap, iterator, "_hasValue").and_then(|c| field_bool(heap, c)) == Some(true) {
                return;
            }
            // O `_Future` do `await iterator.moveNext()`.
            if let Some(f) = trace_ref(heap, iterator, "_stateData")
                && class_of(heap, f).as_deref() == Some("_Future")
            {
                q.next_offset = Some(f);
            }
            return;
        }
        if kind == CONTROLLER_ADD_TEAROFF
            && let Some(dest) = receiver_obj
        {
            let target_state = trace_int(heap, dest, "_state").unwrap_or(0);
            if target_state & CONTROLLER_ADD_STREAM != 0 {
                let listener = trace_ref(heap, dest, "_varData")
                    .and_then(|s| trace_ref(heap, s, "addStreamFuture"))
                    .and_then(|f| trace_ref(heap, f, "_resultOrListeners"));
                if let Some(o) = listener
                    && class_of(heap, o).as_deref() == Some("_FutureListener")
                    && trace_int(heap, o, "state") == Some(LISTENER_THEN)
                {
                    let mut handler = trace_ref(heap, o, "callback").filter(|&c| heap.closure(c).is_some());
                    let link = follow_links_from(ix, heap, &mut handler);
                    if let Some(c) = link
                        && class_of(heap, c).as_deref() == Some("_AsyncStarStreamController")
                    {
                        q.closure = trace_ref(heap, c, "asyncStarBody").filter(|&x| heap.closure(x).is_some());
                        return;
                    }
                }
            }
        }
    }
    q.closure = closure;
    let mut sibling_with_link = false;
    let mut with_link = has_link(ix, heap, closure);
    if !with_link {
        // O `onError` ou o `onDone` com elo.
        closure = trace_ref(heap, subscription, "_onError").filter(|&c| heap.closure(c).is_some());
        with_link = has_link(ix, heap, closure);
        sibling_with_link = with_link;
    }
    if !with_link {
        closure = trace_ref(heap, subscription, "_onDone").filter(|&c| heap.closure(c).is_some());
        with_link = has_link(ix, heap, closure);
        sibling_with_link = with_link;
    }
    let has_error_handler = subscription_state & SUBSCRIPTION_WITH_ERROR != 0;
    let mut object = None;
    if has_error_handler || sibling_with_link {
        object = follow_links_from(ix, heap, &mut closure);
    }
    if sibling_with_link {
        let class = object.and_then(|o| class_of(heap, o));
        if matches!(class.as_deref(), Some("_AsyncStarStreamController" | "_SyncStreamController")) {
            q.closure = closure;
        } else {
            q.next_offset = object;
        }
    }
}

/// `UnwindAwaiterFrame`: do quadro suspenso ao `Future` dele (pelo
/// `Completer`), e do `Future` ao ouvinte.
fn unwind_waiter(ix: &TraceIndex, heap: &crate::heap::Heap, q: &mut Waiter) {
    let mut next_offset = q.next_offset;
    if let Some(p) = next_offset
        && is_async_frame(heap, p)
    {
        // O `completer` do quadro (posição 1); num gerador `async*`, o
        // controlador (o `function_data` do `SuspendState` da VM).
        next_offset = heap.object(p).and_then(|o| o.get(1)).and_then(|c| (c.1 && c.0 != 0).then_some(c.0));
    } else if let Some(p) = next_offset {
        match class_of(heap, p).as_deref() {
            Some("_SyncCompleter" | "_AsyncCompleter") => next_offset = trace_ref(heap, p, "future"),
            _ => {}
        }
    }
    if let Some(p) = next_offset
        && class_of(heap, p).as_deref() == Some("_AsyncAwaitCompleter")
    {
        next_offset = trace_ref(heap, p, "_future");
    }
    if let Some(p) = next_offset
        && class_of(heap, p).as_deref() == Some("_AsyncStarStreamController")
    {
        next_offset = trace_ref(heap, p, "controller");
    }
    match next_offset.and_then(|p| class_of(heap, p)).as_deref() {
        Some("_Future") => {
            q.next_offset = next_offset;
            up_to_listener(ix, heap, q);
        }
        Some("_SyncStreamController") => {
            q.next_offset = next_offset;
            up_to_controller(ix, heap, q);
            follow_links(ix, heap, q);
        }
        _ => *q = Waiter::default(),
    }
}

/// `UnwindToAwaiter`.
fn up_to_waiter(ix: &TraceIndex, heap: &crate::heap::Heap, q: &mut Waiter) {
    let mut steps = 0;
    loop {
        unwind_waiter(ix, heap, q);
        steps += 1;
        if q.closure.is_some() || q.next_offset.is_none() || steps > 1024 {
            break;
        }
    }
}

/// A entrada da pilha do rastro do quadro de máquina cujo topo é `sp`: a
/// mais alta ainda livre, do `tipo`, gravada logo abaixo de `sp`. As de cima
/// dela ficam para trás (de quadros mais fundos já casados, ou deixadas por
/// quadros desenrolados).
fn match_at(stack: &[(i64, i64, usize)], top: &mut usize, tipo: i64, sp: u64) -> Option<i64> {
    let sp = sp as usize;
    if sp == 0 {
        return None;
    }
    for i in (0..*top).rev() {
        let (t, v, address) = stack[i];
        if t == tipo && address < sp && sp - address < STACK_WINDOW {
            *top = i;
            return Some(v);
        }
    }
    None
}

/// O percurso do `AsyncAwareStackUnwinder::Unwind`: os quadros síncronos até
/// o primeiro com quem o espere, e depois a cadeia de quem espera.
fn unwind(ix: &TraceIndex, heap: &crate::heap::Heap, frames: &[(u64, u64, u64)], stack: &[(i64, i64, usize)]) -> Vec<TraceFrame> {
    let mut out = Vec::with_capacity(frames.len() + 8);
    let mut top = stack.len();
    let mut q = Waiter::default();
    'sincronos: for &(ret, start, sp) in frames {
        out.push(TraceFrame::Sync(ret, start));
        for p in ix.chain(ret, start) {
            let marks = record_marks(p.rec);
            if marks & ASYNC_BODY_MARK != 0 {
                // `InitializeAwaiterFrameFromSuspendState`: o corpo retomado
                // (já suspenso uma vez; o de um gerador `async*` sempre)
                // segue pela cadeia.
                if let Some(frame) = match_at(stack, &mut top, 0, sp)
                    && (marks & ASYNC_GENERATOR_MARK != 0 || frame_state(heap, frame) != 0)
                {
                    q = Waiter { closure: None, next_offset: Some(frame) };
                    up_to_waiter(ix, heap, &mut q);
                }
            } else if marks & LISTEN_MARK != 0
                && let Some(listener) = match_at(stack, &mut top, 1, sp)
            {
                start_from_listener(ix, heap, &mut q, listener);
                up_to_waiter(ix, heap, &mut q);
            }
            if q.closure.is_some() {
                break 'sincronos;
            }
        }
    }
    let mut some_one = false;
    let mut steps = 0;
    while let Some(c) = q.closure {
        steps += 1;
        if steps > FRAMES_IN_TRACE {
            break;
        }
        let input = heap.closure(c).map_or(0, |clo| clo.code as u64);
        some_one = true;
        match q.next_offset {
            Some(p) if is_async_frame(heap, p) => {
                let k = frame_state(heap, p);
                // `pc == 0`: o corpo já foi retomado (está na pilha).
                if k > 0 {
                    out.push(TraceFrame::Gap);
                    out.push(TraceFrame::Wait(input, k));
                }
            }
            _ => {
                out.push(TraceFrame::Gap);
                out.push(TraceFrame::Listener(input));
            }
        }
        up_to_waiter(ix, heap, &mut q);
    }
    if some_one {
        out.push(TraceFrame::Gap);
    }
    out
}

// --- as seções e o texto ----------------------------------------------------

/// Lê as entradas de `[start, end)`. A entrada toda zero é sentinela ou
/// enchimento.
///
/// # Safety
/// `[start, end)` é uma seção do rastro mapeada.
#[allow(unsafe_code)]
unsafe fn read_trace_section(start: usize, end: usize, points: &mut Vec<TracePoint>, functions: &mut Vec<(usize, TraceFunction)>) {
    let mut p = (start + 3) & !3;
    while p + 12 <= end {
        // A forma compacta de um objeto convertido (`rastro_compacto.rs`).
        // SAFETY: `p .. p + 4` está dentro da seção.
        if unsafe { std::slice::from_raw_parts(p as *const u8, 4) } == b"DFPC" {
            // SAFETY: o blob começa em `p` e fica dentro da seção.
            match unsafe { read_trace_blob(p, end, points, functions) } {
                Some(size) => {
                    p += (size + 3) & !3;
                    continue;
                }
                None => return,
            }
        }
        // SAFETY: `p .. p + 12` está dentro da seção.
        let (label_name, rec, word) = unsafe {
            (
                (p as *const i32).read_unaligned(),
                ((p + 4) as *const i32).read_unaligned(),
                ((p + 8) as *const u32).read_unaligned(),
            )
        };
        if label_name != 0 || rec != 0 || word != 0 {
            let rot = p.wrapping_add_signed(label_name as isize);
            let kind = (rec & 3) as u32;
            let reg = (p + 4).wrapping_add_signed((rec - kind as i32) as isize);
            match kind {
                0 => points.push(TracePoint { label_name: rot, rec: reg, line_column: word }),
                _ => {
                    if functions.last().is_none_or(|(e, _)| *e != rot) {
                        functions.push((rot, TraceFunction::default()));
                    }
                    let f = &mut functions.last_mut().expect("a função acabada de pôr").1;
                    match kind {
                        1 => {
                            f.rec = reg;
                            f.token = word;
                        }
                        2 => f.waits.push(word),
                        _ => f.link = Some(((word >> 2) as usize, word & 2 != 0, word & 1 != 0)),
                    }
                }
            }
        }
        p += 12;
    }
}

/// Um varint LEB128 em `*p`, sem passar de `end`.
///
/// # Safety
/// `[*p, end)` está mapeado.
#[allow(unsafe_code)]
unsafe fn trace_varint(p: &mut usize, end: usize) -> Option<u64> {
    let mut r = 0u64;
    let mut s = 0u32;
    loop {
        if *p >= end {
            return None;
        }
        // SAFETY: `*p < end`.
        let b = unsafe { (*p as *const u8).read() };
        *p += 1;
        if s < 64 {
            r |= u64::from(b & 0x7f) << s;
        }
        s += 7;
        if b & 0x80 == 0 {
            return Some(r);
        }
    }
}

/// Lê o blob DFPC em `start` (o formato de `rastro_compacto.rs`): o índice
/// das funções e os registros, relocados relativos ao próprio campo (no
/// COFF, ao byte seguinte), e o fluxo de cada função. Devolve o tamanho do
/// blob; `None` num blob malformado (o resto da seção é ignorado).
///
/// # Safety
/// `[start, end)` é uma seção do rastro mapeada.
#[allow(unsafe_code)]
unsafe fn read_trace_blob(start: usize, end: usize, points: &mut Vec<TracePoint>, functions: &mut Vec<(usize, TraceFunction)>) -> Option<usize> {
    // SAFETY: os campos lidos ficam dentro de `[start, end)` (conferido).
    unsafe {
        let u32_at = |q: usize| (q as *const u32).read_unaligned();
        if start + 20 > end || (start as *const u8).add(4).read() != 1 {
            return None;
        }
        let shape = (start as *const u8).add(5).read();
        let size = u32_at(start + 8) as usize;
        let nf = u32_at(start + 12) as usize;
        let nr = u32_at(start + 16) as usize;
        if size < 20 || start + size > end || 20 + 8 * nf + 4 * nr > size {
            return None;
        }
        let blob_end = start + size;
        let address = |field: usize| {
            let v = (field as *const i32).read_unaligned() as isize;
            if shape == 2 { (field + 4).wrapping_add_signed(v) } else { field.wrapping_add_signed(v) }
        };
        let records_base = start + 20 + 8 * nf;
        let records: Vec<usize> = (0..nr).map(|k| address(records_base + 4 * k)).collect();
        for k in 0..nf {
            let field = start + 20 + 8 * k;
            let function = address(field);
            let mut q = start + u32_at(field + 4) as usize;
            let n = trace_varint(&mut q, blob_end)?;
            let mut offset = 0usize;
            for _ in 0..n {
                offset += trace_varint(&mut q, blob_end)? as usize;
                let rk = trace_varint(&mut q, blob_end)?;
                let word = trace_varint(&mut q, blob_end)? as u32;
                let rot = function + offset;
                let kind = rk & 3;
                let reg = records.get((rk >> 2) as usize).copied().unwrap_or(0);
                match kind {
                    0 => points.push(TracePoint { label_name: rot, rec: reg, line_column: word }),
                    _ => {
                        if functions.last().is_none_or(|(e, _)| *e != rot) {
                            functions.push((rot, TraceFunction::default()));
                        }
                        let f = &mut functions.last_mut().expect("a função acabada de pôr").1;
                        match kind {
                            1 => {
                                f.rec = reg;
                                f.token = word;
                            }
                            2 => f.waits.push(word),
                            _ => f.link = Some(((word >> 2) as usize, word & 2 != 0, word & 1 != 0)),
                        }
                    }
                }
            }
        }
        Some(size)
    }
}

/// Chama `f` com o índice de todas as seções registradas.
#[allow(unsafe_code)]
fn with_trace_index<R>(f: impl FnOnce(&TraceIndex) -> R) -> R {
    let (sections, generation) = {
        let s = TRACE_SECTIONS.lock().unwrap_or_else(|e| e.into_inner());
        (s.clone(), TRACE_GENERATION.load(std::sync::atomic::Ordering::Acquire))
    };
    {
        let ix = TRACE_INDEX.read().unwrap_or_else(|e| e.into_inner());
        if ix.0 == generation {
            return f(&ix.1);
        }
    }
    let mut points = Vec::new();
    let mut functions = Vec::new();
    for &(start, end) in &sections {
        // SAFETY: seções registradas por `dartforge_registrar_rastro`.
        unsafe { read_trace_section(start, end, &mut points, &mut functions) };
    }
    // Estável: os do mesmo rótulo ficam na ordem da seção (de dentro para
    // fora).
    points.sort_by_key(|p| p.label_name);
    functions.sort_by_key(|(e, _)| *e);
    functions.dedup_by_key(|(e, _)| *e);
    let mut ix = TRACE_INDEX.write().unwrap_or_else(|e| e.into_inner());
    *ix = (generation, TraceIndex { points, functions });
    f(&ix.1)
}

/// O nome, a url e as marcas do registro de função em `r`.
///
/// # Safety
/// `r` é um registro de função de uma seção registrada.
#[allow(unsafe_code)]
unsafe fn trace_record(r: usize) -> (String, String, u8) {
    // SAFETY: o formato do registro (`llvm/trace.rs`): `i32` até a url, o
    // byte das marcas, o nome; os textos terminados em zero.
    unsafe {
        let until_url = (r as *const i32).read_unaligned();
        let url = std::ffi::CStr::from_ptr(r.wrapping_add_signed(until_url as isize) as *const std::ffi::c_char);
        let marks = ((r + 4) as *const u8).read();
        let name = std::ffi::CStr::from_ptr((r + 5) as *const std::ffi::c_char);
        (name.to_string_lossy().into_owned(), url.to_string_lossy().into_owned(), marks)
    }
}

/// O texto do rastro dos pares no formato da VM: `#<índice>` alinhado à
/// esquerda em 6 colunas, o nome e `(url:line:column)`, a linha e a coluna
/// só quando conhecidas; os quadros embutidos de um ponto, de dentro para
/// fora; os ocultos, fora; e `<asynchronous suspension>` entre os trechos
/// da cadeia de quem espera, uma vez por lacuna. Sem quadro nenhum, o texto
/// de hoje.
fn symbolize_returns(pair_list: &[(u64, u64)]) -> String {
    with_trace_index(|ix| symbolize_with(ix, pair_list))
}

fn escrever_quadro(text: &mut String, n: usize, name: &str, url: &str, line_column: u32) {
    use std::fmt::Write as _;
    let (line, column) = (line_column >> 12, line_column & 0xFFF);
    let _ = write!(text, "#{n:<6} {name} ({url}");
    if line > 0 {
        let _ = write!(text, ":{line}");
        if column > 0 {
            let _ = write!(text, ":{column}");
        }
    }
    text.push_str(")\n");
}

/// [`symbolize_returns`] com o índice dado.
#[allow(unsafe_code)]
fn symbolize_with(ix: &TraceIndex, pair_list: &[(u64, u64)]) -> String {
    let mut text = String::new();
    let mut n = 0usize;
    let mut in_gap = false;
    let mut some_frame = false;
    for &pair in pair_list {
        match pair_frame(pair) {
            TraceFrame::Sync(ret, start) => {
                for p in ix.chain(ret, start) {
                    // SAFETY: o registro de uma entrada da seção.
                    let (name, url, marks) = unsafe { trace_record(p.rec) };
                    if marks & HIDDEN_MARK != 0 {
                        continue;
                    }
                    escrever_quadro(&mut text, n, &name, &url, p.line_column);
                    n += 1;
                    in_gap = false;
                    some_frame = true;
                }
            }
            TraceFrame::Gap => {
                if !in_gap {
                    text.push_str("<asynchronous suspension>\n");
                }
                in_gap = true;
            }
            TraceFrame::Wait(input, k) => {
                let Some(f) = ix.function(input as usize) else { continue };
                let cur_position = usize::try_from(k).ok().and_then(|k| k.checked_sub(1)).and_then(|i| f.waits.get(i)).copied().unwrap_or(0);
                // SAFETY: o registro de uma entrada da seção.
                let (name, url, _) = unsafe { trace_record(f.rec) };
                escrever_quadro(&mut text, n, &name, &url, cur_position);
                n += 1;
                in_gap = false;
                some_frame = true;
            }
            TraceFrame::Listener(input) => {
                let Some(f) = ix.function(input as usize) else { continue };
                // SAFETY: o registro de uma entrada da seção.
                let (name, url, marks) = unsafe { trace_record(f.rec) };
                if marks & HIDDEN_MARK != 0 {
                    continue;
                }
                escrever_quadro(&mut text, n, &name, &url, f.token);
                n += 1;
                in_gap = false;
                some_frame = true;
            }
        }
    }
    if !some_frame {
        return TRACE_WITHOUT_TABLE.to_string();
    }
    text
}

/// Os pares guardados num `StackTrace` do rastro simbólico: o campo 0 nulo
/// (o texto, feito no primeiro pedido), o 1 o número de pares e os pares
/// depois, todos brutos. Vazio no `StackTrace` de texto.
fn object_returns(heap: &crate::heap::Heap, h: i64) -> Vec<(u64, u64)> {
    let Some(o) = heap.object(h) else { return Vec::new() };
    let Some((n, false)) = o.get(1) else { return Vec::new() };
    let n = usize::try_from(n).unwrap_or(0);
    let mut returns = Vec::with_capacity(n);
    for i in 0..n {
        match (o.get(2 + 2 * i), o.get(3 + 2 * i)) {
            (Some((r, false)), Some((f, false))) => returns.push((r as u64, f as u64)),
            _ => break,
        }
    }
    returns
}

/// Um `StackTrace` com os pares `returns` (veja [`object_returns`]);
/// uma alocação, sem referência a enraizar.
fn allocate_return_trace(returns: &[(u64, u64)]) -> i64 {
    let cid = stack_trace_class_id();
    let mut fields: Vec<crate::heap::Field> = Vec::with_capacity(2 + 2 * returns.len());
    fields.push((0, true));
    fields.push((returns.len() as i64, false));
    for &(r, f) in returns {
        fields.push((r as i64, false));
        fields.push((f as i64, false));
    }
    HEAP.with(|h| h.borrow_mut().new_object(cid, &fields))
}

/// O texto do `StackTrace` `h` como `String` (a exceção não capturada, a
/// descrição do runtime): o do campo 0, ou os pares simbolizados.
/// `None`: `h` não é objeto.
fn trace_text(h: i64) -> Option<String> {
    let (text, returns) = HEAP.with(|heap| {
        let heap = heap.borrow();
        let o = heap.object(h)?;
        Some(match o.first() {
            Some((t, true)) if t != 0 => (heap.text(t).map(|t| t.para_string()), Vec::new()),
            _ => (None, object_returns(&heap, h)),
        })
    })?;
    Some(text.unwrap_or_else(|| symbolize_returns(&returns)))
}

/// `_StackTrace.toString` (a VM o tem em C++): o texto do campo 0, ou os
/// pares simbolizados agora e guardados no campo 0 para o próximo pedido.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rastro_texto(h: i64) -> i64 {
    let (ready_flag, returns) = HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.object(h).and_then(|o| o.first()) {
            Some((t, true)) if t != 0 => (t, Vec::new()),
            _ => (0, object_returns(&heap, h)),
        }
    });
    if ready_flag != 0 {
        return ready_flag;
    }
    let text = if returns.is_empty() { TRACE_WITHOUT_TABLE.to_string() } else { symbolize_returns(&returns) };
    let t = with_roots(&[h], || error_text(&text));
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        if heap.object(h).is_some_and(|o| !o.is_empty()) {
            heap.set_field(h, 0, t, true);
        }
    });
    t
}

#[cfg(test)]
mod tests_trace {
    use super::*;

    /// Os registros, o código e a seção de um teste numa alocação só. Os
    /// campos da seção são deslocamentos `i32`, como entre as seções de um
    /// binário; alocações separadas do heap não têm essa garantia (no macOS
    /// arm64 o `malloc` põe blocos pequenos em regiões a centenas de GiB
    /// umas das outras), e o deslocamento truncado apontaria para fora.
    struct Area {
        rest: &'static mut [u8],
    }

    impl Area {
        fn new_area() -> Self {
            Area { rest: Box::leak(vec![0u8; 4096].into_boxed_slice()) }
        }

        /// `n` bytes zerados da área, alinhados a 16, vivos até o fim do
        /// processo.
        fn reserve_space(&mut self, n: usize) -> &'static mut [u8] {
            let rest = std::mem::take(&mut self.rest);
            let jump = rest.as_ptr().align_offset(16);
            let (block, rest) = rest[jump..].split_at_mut(n);
            self.rest = rest;
            block
        }
    }

    /// Os textos e os registros de um teste: `(base, rec de cada name)`.
    /// A url em 0; cada registro tem o deslocamento até ela, o byte das
    /// marcas e o nome.
    fn records(area: &mut Area, names: &[(&str, u8)]) -> Vec<usize> {
        let data = area.reserve_space(16 + 32 * names.len());
        data[..15].copy_from_slice(b"file:///a.dart\0");
        let base = data.as_ptr() as usize;
        let mut out = Vec::new();
        for (i, (name, marks)) in names.iter().enumerate() {
            let r = 16 + 32 * i;
            data[r..r + 4].copy_from_slice(&(-(r as i32)).to_le_bytes());
            data[r + 4] = *marks;
            data[r + 5..r + 5 + name.len()].copy_from_slice(name.as_bytes());
            out.push(base + r);
        }
        out
    }

    /// Uma seção montada à mão: `(espécie, rótulo, rec, word)`, com
    /// uma sentinela zerada no começo.
    fn section(area: &mut Area, entries: &[(u32, usize, usize, u32)]) -> (usize, usize) {
        let section = area.reserve_space(12 * (entries.len() + 1));
        let s = section.as_ptr() as usize;
        for (k, &(kind, label_name, rec, word)) in entries.iter().enumerate() {
            let k = k + 1;
            let e = s + 12 * k;
            section[12 * k..12 * k + 4].copy_from_slice(&((label_name as i64 - e as i64) as i32).to_le_bytes());
            let field = if kind == 3 { 3 } else { (rec as i64 - (e + 4) as i64) as i32 + kind as i32 };
            section[12 * k + 4..12 * k + 8].copy_from_slice(&field.to_le_bytes());
            section[12 * k + 8..12 * k + 12].copy_from_slice(&word.to_le_bytes());
        }
        (s, s + section.len())
    }

    fn ix(area: &mut Area, entries: &[(u32, usize, usize, u32)]) -> TraceIndex {
        let (s, f) = section(area, entries);
        let (mut points, mut functions) = (Vec::new(), Vec::new());
        // SAFETY: a seção acima, viva até o fim do processo.
        unsafe { read_trace_section(s, f, &mut points, &mut functions) };
        points.sort_by_key(|p| p.label_name);
        functions.sort_by_key(|(e, _)| *e);
        TraceIndex { points, functions }
    }

    /// Cada endereço de retorno acha o rótulo da função dele, e o que não
    /// acha fica de fora; os quadros embutidos saem de dentro para fora; os
    /// ocultos, não.
    #[test]
    fn symbolizes_in_vm_format() {
        let mut area = Area::new_area();
        let r = records(&mut area, &[("main", 0), ("f", 0), ("g", 0), ("stub", HIDDEN_MARK)]);
        let code = area.reserve_space(200);
        let c = code.as_ptr() as usize;
        let ind = ix(&mut area, &[
            (0, c + 50, r[1], 7 << 12),
            (0, c + 10, r[0], 3 << 12 | 5),
            // Um ponto com `g` copiada em `main` (de dentro para fora).
            (0, c + 30, r[2], 9 << 12 | 2),
            (0, c + 30, r[0], 4 << 12 | 1),
            (0, c + 120, r[3], 1 << 12),
        ]);
        assert_eq!(ind.points.len(), 5, "a sentinela não é entrada");
        let c = c as u64;
        let text = symbolize_with(&ind, &[(c + 5, c), (c + 20, c), (c + 35, c), (c + 60, c + 55), (c + 60, c + 40), (c + 125, c + 100)]);
        assert_eq!(
            text,
            "#0      main (file:///a.dart:3:5)\n#1      g (file:///a.dart:9:2)\n#2      main (file:///a.dart:4:1)\n#3      f (file:///a.dart:7)\n"
        );
        assert_eq!(symbolize_with(&ind, &[(c + 5, c)]), TRACE_WITHOUT_TABLE);
    }

    /// A cadeia de quem espera: a lacuna antes de cada quadro e no fim, uma
    /// vez só entre trechos; o quadro de quem espera na posição do `await`
    /// do estado; o ouvinte no token.
    #[test]
    fn writes_waiter_chain() {
        let mut area = Area::new_area();
        let r = records(&mut area, &[("f", ASYNC_BODY_MARK), ("main", ASYNC_BODY_MARK), ("main.<anonymous closure>", 0)]);
        let code = area.reserve_space(200);
        let c = code.as_ptr() as usize;
        let ind = ix(&mut area, &[
            (0, c + 10, r[0], 2 << 12 | 3),
            (1, c + 100, r[1], 1 << 12 | 6),
            (2, c + 100, r[1], 5 << 12 | 9),
            (2, c + 100, r[1], 6 << 12 | 9),
            (1, c + 150, r[2], 8 << 12 | 20),
            (3, c + 150, 0, 1 << 2 | 1),
        ]);
        assert_eq!(ind.function(c + 100).map(|f| f.waits.len()), Some(2));
        assert_eq!(ind.function(c + 150).and_then(|f| f.link), Some((1, false, true)));
        let c = c as u64;
        let pair_list: Vec<(u64, u64)> = [
            TraceFrame::Sync(c + 15, c),
            TraceFrame::Gap,
            TraceFrame::Wait(c + 100, 2),
            TraceFrame::Gap,
            TraceFrame::Gap,
            TraceFrame::Listener(c + 150),
            TraceFrame::Gap,
        ]
        .into_iter()
        .map(frame_pair)
        .collect();
        assert_eq!(
            symbolize_with(&ind, &pair_list),
            "#0      f (file:///a.dart:2:3)\n<asynchronous suspension>\n#1      main (file:///a.dart:6:9)\n<asynchronous suspension>\n#2      main.<anonymous closure> (file:///a.dart:8:20)\n<asynchronous suspension>\n"
        );
        for q in [TraceFrame::Gap, TraceFrame::Wait(77, 3), TraceFrame::Listener(78), TraceFrame::Sync(5, 1)] {
            assert_eq!(pair_frame(frame_pair(q)), q);
        }
    }

    /// A entrada da pilha do rastro casa com o quadro cujo topo está logo
    /// acima dela; a de um quadro desenrolado fica para trás.
    #[test]
    fn matches_trace_stack_by_frame_top() {
        let stack = [(0, 11, 9_000usize), (1, 22, 5_000), (0, 33, 4_000)];
        let mut top = stack.len();
        // O quadro mais fundo (topo em 5 100): a entrada de 4 000 é de um
        // quadro mais fundo que ele, já desenrolado, e não casa com o tipo.
        assert_eq!(match_at(&stack, &mut top, 1, 5_100), Some(22));
        assert_eq!(top, 1);
        assert_eq!(match_at(&stack, &mut top, 0, 9_050), Some(11));
        assert_eq!(match_at(&stack, &mut top, 0, 9_050), None);
        assert_eq!(match_at(&stack, &mut 3, 0, 0), None);
    }
}
