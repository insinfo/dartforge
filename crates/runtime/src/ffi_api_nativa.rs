// Runtime nativo: a API nativa de portas do Dart (`dart_native_api.h`) e os
// dados de `dart_api_dl.h` — o que `NativeApi.postCObject`,
// `NativeApi.newNativePort`, `NativeApi.closeNativePort` e
// `NativeApi.initializeApiDLData` entregam ao código C.
//
// * `Dart_PostCObject(port, object)`: lê a árvore `Dart_CObject` (em qualquer
//   thread, sem heap Dart), monta a mensagem portátil (`Portable`) e a posta
//   na porta, como `SendPort.send`. Dados externos (`kExternalTypedData`) são
//   copiados e o finalizador do C é chamado logo em seguida — a VM o chama
//   quando o objeto morre; aqui a mensagem já não depende da memória dele.
// * `Dart_NewNativePort(name, handler, concorrente)`: uma porta atendida
//   por uma thread própria, que converte cada mensagem de volta para
//   `Dart_CObject` e chama o handler (a VM usa o pool de threads; as
//   mensagens de uma porta chegam em ordem, uma de cada vez).
// * `initializeApiDLData`: a tabela `DartApi` da versão 2.5 da API DL com as
//   funções da API nativa; os nomes que dependem de `Dart_Handle` (a API
//   completa do embedder) não constam, e `Dart_InitializeApiDL` os deixa
//   nulos — como faria com um símbolo ausente.
//
// Layout do `Dart_CObject` (C, 64 bits): `type` (int32) em 0 e a união em 8,
// com 40 bytes; os campos de cada variante seguem o cabeçalho.

use std::ffi::{c_char, CStr, CString};

const KNULL: i32 = 0;
const KBOOL: i32 = 1;
const KINT32: i32 = 2;
const KINT64: i32 = 3;
const KDOUBLE: i32 = 4;
const KSTRING: i32 = 5;
const KARRAY: i32 = 6;
const KTYPED_DATA: i32 = 7;
const KEXTERNAL_TYPED_DATA: i32 = 8;
const KSEND_PORT: i32 = 9;
const KCAPABILITY: i32 = 10;
const KNATIVE_POINTER: i32 = 11;
const KUNMODIFIABLE_EXTERNAL_TYPED_DATA: i32 = 13;

/// Tamanho de um `Dart_CObject` (o `type` e a união de 40 bytes).
const COBJECT_SIZE: usize = 48;

/// `Dart_TypedData_Type` → o `TIPO_*` do runtime.
fn typed_data_type(t: i32) -> Option<u8> {
    Some(match t {
        0 => TYPE_BYTE_DATA,
        1 => TYPE_INT8,
        2 => TYPE_UINT8,
        3 => TYPE_UINT8_CLAMPED,
        4 => TYPE_INT16,
        5 => TYPE_UINT16,
        6 => TYPE_INT32,
        7 => TYPE_UINT32,
        8 => TYPE_INT64,
        9 => TYPE_UINT64,
        10 => TYPE_FLOAT32,
        11 => TYPE_FLOAT64,
        12 => TYPE_INT32X4,
        13 => TYPE_FLOAT32X4,
        14 => TYPE_FLOAT64X2,
        _ => return None,
    })
}

/// O inverso de [`typed_data_type`].
fn typed_data_of_type(t: u8) -> i32 {
    match t {
        TYPE_BYTE_DATA => 0,
        TYPE_INT8 => 1,
        TYPE_UINT8 => 2,
        TYPE_UINT8_CLAMPED => 3,
        TYPE_INT16 => 4,
        TYPE_UINT16 => 5,
        TYPE_INT32 => 6,
        TYPE_UINT32 => 7,
        TYPE_INT64 => 8,
        TYPE_UINT64 => 9,
        TYPE_FLOAT32 => 10,
        TYPE_FLOAT64 => 11,
        TYPE_INT32X4 => 12,
        TYPE_FLOAT32X4 => 13,
        TYPE_FLOAT64X2 => 14,
        _ => 2,
    }
}

type HandleFinalizer = extern "C" fn(usize, usize);

/// Lê a árvore `Dart_CObject` em `p`. `None`: tipo não suportado (a VM
/// recusa o `Dart_PostCObject`). Os finalizadores de dados externos lidos
/// vão para `external`, chamados só se a mensagem for postada.
///
/// # Safety
/// `p` aponta para um `Dart_CObject` válido do chamador.
unsafe fn read_cobject(p: *const u8, external: &mut Vec<(usize, usize)>, depth: usize) -> Option<Portable> {
    if p.is_null() || depth > 10_000 {
        return None;
    }
    // SAFETY: garantido por quem chama; cada leitura está dentro dos 48
    // bytes do objeto, nas posições do cabeçalho.
    unsafe {
        let tipo = *(p as *const i32);
        let v = p.add(8);
        Some(match tipo {
            KNULL => Portable::Null,
            KBOOL => Portable::Bool(*v != 0),
            KINT32 => Portable::Int(i64::from(*(v as *const i32))),
            KINT64 => Portable::Int(*(v as *const i64)),
            KDOUBLE => Portable::Double(*(v as *const f64)),
            KSTRING => {
                let s = *(v as *const *const c_char);
                if s.is_null() {
                    return None;
                }
                Portable::Str(CStr::from_ptr(s).to_str().ok()?.to_owned())
            }
            KARRAY => {
                let n = *(v as *const isize);
                let value_list = *(v.add(8) as *const *const *const u8);
                if n < 0 || (n > 0 && value_list.is_null()) {
                    return None;
                }
                let mut items = Vec::with_capacity(n as usize);
                for i in 0..n as usize {
                    items.push(read_cobject(*value_list.add(i), external, depth + 1)?);
                }
                Portable::List(items)
            }
            KTYPED_DATA | KEXTERNAL_TYPED_DATA | KUNMODIFIABLE_EXTERNAL_TYPED_DATA => {
                let t = typed_data_type(*(v as *const i32))?;
                let n = *(v.add(8) as *const isize);
                let data = *(v.add(16) as *const *const u8);
                if n < 0 || (n > 0 && data.is_null()) {
                    return None;
                }
                let bytes = n as usize * tamanho_do_elemento(t);
                let copy = if bytes == 0 { Vec::new() } else { std::slice::from_raw_parts(data, bytes).to_vec() };
                if tipo != KTYPED_DATA {
                    let pair = *(v.add(24) as *const usize);
                    let finalizer = *(v.add(32) as *const usize);
                    if finalizer != 0 {
                        external.push((finalizer, pair));
                    }
                }
                Portable::Typed(t, copy)
            }
            KSEND_PORT => Portable::FromRuntime { cid: crate::layout::cid::SEND_PORT, id: *(v as *const i64) },
            KCAPABILITY => Portable::FromRuntime { cid: crate::layout::cid::CAPABILITY, id: *(v as *const i64) },
            // O endereço chega como `int`; o finalizador fica com o C (a VM
            // só o chama se a mensagem não for entregue).
            KNATIVE_POINTER => Portable::Int(*(v as *const i64)),
            _ => return None,
        })
    }
}

/// Se `port` existe (aberta em algum isolado ou serviço).
fn port_exists(port: i64) -> bool {
    rec().lock().unwrap_or_else(|e| e.into_inner()).contains_key(&port)
}

/// `Dart_PostCObject(port_id, message)`.
///
/// # Safety
/// `message` aponta para um `Dart_CObject` válido.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_api_post_cobject(port: i64, message: *const u8) -> bool {
    let mut external = Vec::new();
    // SAFETY: garantido por quem chama.
    let Some(value) = (unsafe { read_cobject(message, &mut external, 0) }) else {
        return false;
    };
    if !port_exists(port) {
        return false;
    }
    post(port, value.to_graph());
    for (f, pair) in external {
        // SAFETY: o `Dart_HandleFinalizer` do chamador, `(isolate_data, peer)`.
        let f: HandleFinalizer = unsafe { std::mem::transmute(f) };
        f(0, pair);
    }
    true
}

/// `Dart_PostInteger(port_id, message)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_api_post_integer(port: i64, value: i64) -> bool {
    if !port_exists(port) {
        return false;
    }
    post(port, Portable::Int(value).to_graph());
    true
}

/// Os objetos C montados para o handler de uma porta nativa; vivos até ele
/// voltar.
#[derive(Default)]
struct CObjectArena {
    objects: Vec<Box<[u64; COBJECT_SIZE / 8]>>,
    textos: Vec<CString>,
    lists: Vec<Vec<*mut u8>>,
    bytes: Vec<Vec<u8>>,
}

impl CObjectArena {
    fn new(&mut self, tipo: i32) -> *mut u8 {
        let mut o = Box::new([0u64; COBJECT_SIZE / 8]);
        o[0] = tipo as u32 as u64;
        let p = o.as_mut_ptr() as *mut u8;
        self.objects.push(o);
        p
    }

    /// O `Dart_CObject` de `v`.
    fn build(&mut self, v: &Portable) -> *mut u8 {
        // SAFETY: cada escrita fica dentro dos 48 bytes do objeto recém
        // criado, nas posições do cabeçalho.
        unsafe {
            match v {
                Portable::Null | Portable::Objeto(_) => self.new(KNULL),
                Portable::Bool(b) => {
                    let p = self.new(KBOOL);
                    *p.add(8) = u8::from(*b);
                    p
                }
                Portable::Int(x) => {
                    let p = self.new(KINT64);
                    *(p.add(8) as *mut i64) = *x;
                    p
                }
                Portable::Double(x) => {
                    let p = self.new(KDOUBLE);
                    *(p.add(8) as *mut f64) = *x;
                    p
                }
                Portable::Str(s) => {
                    let p = self.new(KSTRING);
                    let c = CString::new(s.replace('\0', "")).unwrap_or_default();
                    *(p.add(8) as *mut *const c_char) = c.as_ptr();
                    self.textos.push(c);
                    p
                }
                Portable::List(items) => {
                    let children: Vec<*mut u8> = items.iter().map(|x| self.build(x)).collect();
                    let p = self.new(KARRAY);
                    *(p.add(8) as *mut isize) = children.len() as isize;
                    *(p.add(16) as *mut *const *mut u8) = children.as_ptr();
                    self.lists.push(children);
                    p
                }
                Portable::Bytes(b) | Portable::Typed(_, b) => {
                    let t = if let Portable::Typed(t, _) = v { *t } else { TYPE_UINT8 };
                    let p = self.new(KTYPED_DATA);
                    *(p.add(8) as *mut i32) = typed_data_of_type(t);
                    *(p.add(16) as *mut isize) = (b.len() / tamanho_do_elemento(t).max(1)) as isize;
                    self.bytes.push(b.clone());
                    let last_one = self.bytes.last().expect("recém inserido");
                    *(p.add(24) as *mut *const u8) = last_one.as_ptr();
                    p
                }
                Portable::FromRuntime { cid, id } => {
                    let p = self.new(if *cid == crate::layout::cid::CAPABILITY { KCAPABILITY } else { KSEND_PORT });
                    *(p.add(8) as *mut i64) = *id;
                    p
                }
            }
        }
    }
}

type NativePortHandler = extern "C" fn(i64, *mut u8);

/// As portas nativas abertas: id → fila da thread que as atende.
fn native_ports() -> &'static std::sync::Mutex<crate::hash::HashMap<i64, std::sync::mpsc::Sender<(i64, Graph)>>> {
    static T: std::sync::OnceLock<std::sync::Mutex<crate::hash::HashMap<i64, std::sync::mpsc::Sender<(i64, Graph)>>>> =
        std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

/// `Dart_NewNativePort(name, handler, handle_concurrently)`: o id da porta
/// (0 = `ILLEGAL_PORT`).
///
/// # Safety
/// `name` é nulo ou um texto C; `handler` é `void(Dart_Port, Dart_CObject*)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_api_new_native_port(name: *const c_char, handler: usize, _concurrent: bool) -> i64 {
    if handler == 0 {
        return 0;
    }
    let name = if name.is_null() {
        "native port".to_owned()
    } else {
        // SAFETY: garantido por quem chama.
        unsafe { CStr::from_ptr(name) }.to_string_lossy().into_owned()
    };
    // SAFETY: a assinatura de `Dart_NativeMessageHandler`.
    let handler: NativePortHandler = unsafe { std::mem::transmute(handler) };
    let (tx, rx) = std::sync::mpsc::channel::<(i64, Graph)>();
    let queue = std::sync::Mutex::new(tx.clone());
    let port = open_native_port(std::sync::Arc::new(move |port, graph| {
        let _ = queue.lock().unwrap_or_else(|e| e.into_inner()).send((port, graph));
    }));
    let created = std::thread::Builder::new().name(name).spawn(move || {
        for (port, graph) in rx {
            let mut arena = CObjectArena::default();
            let object = arena.build(&graph.to_portable());
            handler(port, object);
        }
    });
    if created.is_err() {
        close_native_port(port);
        return 0;
    }
    native_ports().lock().unwrap_or_else(|e| e.into_inner()).insert(port, tx);
    port
}

/// `Dart_CloseNativePort(native_port_id)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_api_close_native_port(port: i64) -> bool {
    let Some(tx) = native_ports().lock().unwrap_or_else(|e| e.into_inner()).remove(&port) else {
        return false;
    };
    close_native_port(port);
    // Sem o registro nem esta ponta, a fila da thread fecha e ela termina
    // depois das mensagens já entregues.
    drop(tx);
    true
}

/// As funções da API nativa, pelo nome de `dart_native_api.h`.
fn native_api_functions() -> [(&'static str, usize); 4] {
    [
        ("Dart_PostCObject", dartforge_api_post_cobject as usize),
        ("Dart_PostInteger", dartforge_api_post_integer as usize),
        ("Dart_NewNativePort", dartforge_api_new_native_port as usize),
        ("Dart_CloseNativePort", dartforge_api_close_native_port as usize),
    ]
}

/// `DartNativeApiFunctionPointer(name)`: o endereço da função da API nativa.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartNativeApiFunctionPointer(name: i64) -> i64 {
    let name = HEAP.with(|h| h.borrow().text(name).map(|t| t.para_string()).unwrap_or_default());
    match native_api_functions().iter().find(|(n, _)| *n == name) {
        Some((_, f)) => *f as i64,
        None => {
            throw_argument_error(&format!("Unknown native API function: {name}"));
            0
        }
    }
}

/// A versão da API DL (`DART_API_DL_MAJOR_VERSION`/`MINOR`) da VM de
/// referência.
const DL_API_VERSION: (i32, i32) = (2, 5);

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartApiDLMajorVersion() -> i64 {
    i64::from(DL_API_VERSION.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartApiDLMinorVersion() -> i64 {
    i64::from(DL_API_VERSION.1)
}

/// `DartApiEntry { const char* name; void (*function)(void); }`.
#[repr(C)]
struct ApiEntry {
    name: *const c_char,
    function: usize,
}

/// `DartApi { const int major; const int minor; const DartApiEntry* const functions; }`.
#[repr(C)]
struct ApiData {
    larger: i32,
    minor: i32,
    functions: *const ApiEntry,
}

/// O endereço dos dados, montados uma vez e vivos o processo inteiro.
fn api_data() -> usize {
    static DADOS: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *DADOS.get_or_init(|| {
        let mut entries: Vec<ApiEntry> = native_api_functions()
            .iter()
            .map(|(n, f)| ApiEntry { name: CString::new(*n).expect("nome C").into_raw(), function: *f })
            .collect();
        entries.push(ApiEntry { name: std::ptr::null(), function: 0 });
        let functions = Box::leak(entries.into_boxed_slice()).as_ptr();
        let data = Box::leak(Box::new(ApiData { larger: DL_API_VERSION.0, minor: DL_API_VERSION.1, functions }));
        data as *const ApiData as usize
    })
}

/// `DartApiDLInitializeData()`: o ponteiro que o C passa a
/// `Dart_InitializeApiDL`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartApiDLInitializeData() -> i64 {
    api_data() as i64
}
