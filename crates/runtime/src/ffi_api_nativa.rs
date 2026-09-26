// Runtime nativo: a API nativa de portas do Dart (`dart_native_api.h`) e os
// dados de `dart_api_dl.h` — o que `NativeApi.postCObject`,
// `NativeApi.newNativePort`, `NativeApi.closeNativePort` e
// `NativeApi.initializeApiDLData` entregam ao código C.
//
// * `Dart_PostCObject(porta, objeto)`: lê a árvore `Dart_CObject` (em qualquer
//   thread, sem heap Dart), monta a mensagem portátil (`Portavel`) e a posta
//   na porta, como `SendPort.send`. Dados externos (`kExternalTypedData`) são
//   copiados e o finalizador do C é chamado logo em seguida — a VM o chama
//   quando o objeto morre; aqui a mensagem já não depende da memória dele.
// * `Dart_NewNativePort(nome, handler, concorrente)`: uma porta atendida
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
const TAM_COBJECT: usize = 48;

/// `Dart_TypedData_Type` → o `TIPO_*` do runtime.
fn tipo_de_typed_data(t: i32) -> Option<u8> {
    Some(match t {
        0 => TIPO_BYTE_DATA,
        1 => TIPO_INT8,
        2 => TIPO_UINT8,
        3 => TIPO_UINT8_CLAMPED,
        4 => TIPO_INT16,
        5 => TIPO_UINT16,
        6 => TIPO_INT32,
        7 => TIPO_UINT32,
        8 => TIPO_INT64,
        9 => TIPO_UINT64,
        10 => TIPO_FLOAT32,
        11 => TIPO_FLOAT64,
        12 => TIPO_INT32X4,
        13 => TIPO_FLOAT32X4,
        14 => TIPO_FLOAT64X2,
        _ => return None,
    })
}

/// O inverso de [`tipo_de_typed_data`].
fn typed_data_de_tipo(t: u8) -> i32 {
    match t {
        TIPO_BYTE_DATA => 0,
        TIPO_INT8 => 1,
        TIPO_UINT8 => 2,
        TIPO_UINT8_CLAMPED => 3,
        TIPO_INT16 => 4,
        TIPO_UINT16 => 5,
        TIPO_INT32 => 6,
        TIPO_UINT32 => 7,
        TIPO_INT64 => 8,
        TIPO_UINT64 => 9,
        TIPO_FLOAT32 => 10,
        TIPO_FLOAT64 => 11,
        TIPO_INT32X4 => 12,
        TIPO_FLOAT32X4 => 13,
        TIPO_FLOAT64X2 => 14,
        _ => 2,
    }
}

type FinalizadorDeHandle = extern "C" fn(usize, usize);

/// Lê a árvore `Dart_CObject` em `p`. `None`: tipo não suportado (a VM
/// recusa o `Dart_PostCObject`). Os finalizadores de dados externos lidos
/// vão para `externos`, chamados só se a mensagem for postada.
///
/// # Safety
/// `p` aponta para um `Dart_CObject` válido do chamador.
unsafe fn ler_cobject(p: *const u8, externos: &mut Vec<(usize, usize)>, profundidade: usize) -> Option<Portavel> {
    if p.is_null() || profundidade > 10_000 {
        return None;
    }
    // SAFETY: garantido por quem chama; cada leitura está dentro dos 48
    // bytes do objeto, nas posições do cabeçalho.
    unsafe {
        let tipo = *(p as *const i32);
        let v = p.add(8);
        Some(match tipo {
            KNULL => Portavel::Nulo,
            KBOOL => Portavel::Bool(*v != 0),
            KINT32 => Portavel::Int(i64::from(*(v as *const i32))),
            KINT64 => Portavel::Int(*(v as *const i64)),
            KDOUBLE => Portavel::Double(*(v as *const f64)),
            KSTRING => {
                let s = *(v as *const *const c_char);
                if s.is_null() {
                    return None;
                }
                Portavel::Str(CStr::from_ptr(s).to_str().ok()?.to_owned())
            }
            KARRAY => {
                let n = *(v as *const isize);
                let valores = *(v.add(8) as *const *const *const u8);
                if n < 0 || (n > 0 && valores.is_null()) {
                    return None;
                }
                let mut itens = Vec::with_capacity(n as usize);
                for i in 0..n as usize {
                    itens.push(ler_cobject(*valores.add(i), externos, profundidade + 1)?);
                }
                Portavel::Lista(itens)
            }
            KTYPED_DATA | KEXTERNAL_TYPED_DATA | KUNMODIFIABLE_EXTERNAL_TYPED_DATA => {
                let t = tipo_de_typed_data(*(v as *const i32))?;
                let n = *(v.add(8) as *const isize);
                let dados = *(v.add(16) as *const *const u8);
                if n < 0 || (n > 0 && dados.is_null()) {
                    return None;
                }
                let bytes = n as usize * tamanho_do_elemento(t);
                let copia = if bytes == 0 { Vec::new() } else { std::slice::from_raw_parts(dados, bytes).to_vec() };
                if tipo != KTYPED_DATA {
                    let par = *(v.add(24) as *const usize);
                    let finalizador = *(v.add(32) as *const usize);
                    if finalizador != 0 {
                        externos.push((finalizador, par));
                    }
                }
                Portavel::Tipada(t, copia)
            }
            KSEND_PORT => Portavel::DoRuntime { pos: CID_SEND_PORT, id: *(v as *const i64) },
            KCAPABILITY => Portavel::DoRuntime { pos: CID_CAPABILITY, id: *(v as *const i64) },
            // O endereço chega como `int`; o finalizador fica com o C (a VM
            // só o chama se a mensagem não for entregue).
            KNATIVE_POINTER => Portavel::Int(*(v as *const i64)),
            _ => return None,
        })
    }
}

/// Se `porta` existe (aberta em algum isolado ou serviço).
fn porta_existe(porta: i64) -> bool {
    registro().lock().unwrap_or_else(|e| e.into_inner()).contains_key(&porta)
}

/// `Dart_PostCObject(port_id, message)`.
///
/// # Safety
/// `mensagem` aponta para um `Dart_CObject` válido.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_api_post_cobject(porta: i64, mensagem: *const u8) -> bool {
    let mut externos = Vec::new();
    // SAFETY: garantido por quem chama.
    let Some(valor) = (unsafe { ler_cobject(mensagem, &mut externos, 0) }) else {
        return false;
    };
    if !porta_existe(porta) {
        return false;
    }
    postar(porta, valor.para_grafo());
    for (f, par) in externos {
        // SAFETY: o `Dart_HandleFinalizer` do chamador, `(isolate_data, peer)`.
        let f: FinalizadorDeHandle = unsafe { std::mem::transmute(f) };
        f(0, par);
    }
    true
}

/// `Dart_PostInteger(port_id, message)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_api_post_integer(porta: i64, valor: i64) -> bool {
    if !porta_existe(porta) {
        return false;
    }
    postar(porta, Portavel::Int(valor).para_grafo());
    true
}

/// Os objetos C montados para o handler de uma porta nativa; vivos até ele
/// voltar.
#[derive(Default)]
struct ArenaDeCObject {
    objetos: Vec<Box<[u64; TAM_COBJECT / 8]>>,
    textos: Vec<CString>,
    listas: Vec<Vec<*mut u8>>,
    bytes: Vec<Vec<u8>>,
}

impl ArenaDeCObject {
    fn novo(&mut self, tipo: i32) -> *mut u8 {
        let mut o = Box::new([0u64; TAM_COBJECT / 8]);
        o[0] = tipo as u32 as u64;
        let p = o.as_mut_ptr() as *mut u8;
        self.objetos.push(o);
        p
    }

    /// O `Dart_CObject` de `v`.
    fn montar(&mut self, v: &Portavel) -> *mut u8 {
        // SAFETY: cada escrita fica dentro dos 48 bytes do objeto recém
        // criado, nas posições do cabeçalho.
        unsafe {
            match v {
                Portavel::Nulo | Portavel::Objeto(_) => self.novo(KNULL),
                Portavel::Bool(b) => {
                    let p = self.novo(KBOOL);
                    *p.add(8) = u8::from(*b);
                    p
                }
                Portavel::Int(x) => {
                    let p = self.novo(KINT64);
                    *(p.add(8) as *mut i64) = *x;
                    p
                }
                Portavel::Double(x) => {
                    let p = self.novo(KDOUBLE);
                    *(p.add(8) as *mut f64) = *x;
                    p
                }
                Portavel::Str(s) => {
                    let p = self.novo(KSTRING);
                    let c = CString::new(s.replace('\0', "")).unwrap_or_default();
                    *(p.add(8) as *mut *const c_char) = c.as_ptr();
                    self.textos.push(c);
                    p
                }
                Portavel::Lista(itens) => {
                    let filhos: Vec<*mut u8> = itens.iter().map(|x| self.montar(x)).collect();
                    let p = self.novo(KARRAY);
                    *(p.add(8) as *mut isize) = filhos.len() as isize;
                    *(p.add(16) as *mut *const *mut u8) = filhos.as_ptr();
                    self.listas.push(filhos);
                    p
                }
                Portavel::Bytes(b) | Portavel::Tipada(_, b) => {
                    let t = if let Portavel::Tipada(t, _) = v { *t } else { TIPO_UINT8 };
                    let p = self.novo(KTYPED_DATA);
                    *(p.add(8) as *mut i32) = typed_data_de_tipo(t);
                    *(p.add(16) as *mut isize) = (b.len() / tamanho_do_elemento(t).max(1)) as isize;
                    self.bytes.push(b.clone());
                    let ultimo = self.bytes.last().expect("recém inserido");
                    *(p.add(24) as *mut *const u8) = ultimo.as_ptr();
                    p
                }
                Portavel::DoRuntime { pos, id } => {
                    let p = self.novo(if *pos == CID_CAPABILITY { KCAPABILITY } else { KSEND_PORT });
                    *(p.add(8) as *mut i64) = *id;
                    p
                }
            }
        }
    }
}

type HandlerDePortaNativa = extern "C" fn(i64, *mut u8);

/// As portas nativas abertas: id → fila da thread que as atende.
fn portas_nativas() -> &'static std::sync::Mutex<std::collections::HashMap<i64, std::sync::mpsc::Sender<(i64, Grafo)>>> {
    static T: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<i64, std::sync::mpsc::Sender<(i64, Grafo)>>>> =
        std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

/// `Dart_NewNativePort(name, handler, handle_concurrently)`: o id da porta
/// (0 = `ILLEGAL_PORT`).
///
/// # Safety
/// `nome` é nulo ou um texto C; `handler` é `void(Dart_Port, Dart_CObject*)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_api_new_native_port(nome: *const c_char, handler: usize, _concorrente: bool) -> i64 {
    if handler == 0 {
        return 0;
    }
    let nome = if nome.is_null() {
        "native port".to_owned()
    } else {
        // SAFETY: garantido por quem chama.
        unsafe { CStr::from_ptr(nome) }.to_string_lossy().into_owned()
    };
    // SAFETY: a assinatura de `Dart_NativeMessageHandler`.
    let handler: HandlerDePortaNativa = unsafe { std::mem::transmute(handler) };
    let (tx, rx) = std::sync::mpsc::channel::<(i64, Grafo)>();
    let fila = std::sync::Mutex::new(tx.clone());
    let porta = abrir_porta_nativa(std::sync::Arc::new(move |porta, grafo| {
        let _ = fila.lock().unwrap_or_else(|e| e.into_inner()).send((porta, grafo));
    }));
    let criada = std::thread::Builder::new().name(nome).spawn(move || {
        for (porta, grafo) in rx {
            let mut arena = ArenaDeCObject::default();
            let objeto = arena.montar(&grafo.para_portavel());
            handler(porta, objeto);
        }
    });
    if criada.is_err() {
        fechar_porta_nativa(porta);
        return 0;
    }
    portas_nativas().lock().unwrap_or_else(|e| e.into_inner()).insert(porta, tx);
    porta
}

/// `Dart_CloseNativePort(native_port_id)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_api_close_native_port(porta: i64) -> bool {
    let Some(tx) = portas_nativas().lock().unwrap_or_else(|e| e.into_inner()).remove(&porta) else {
        return false;
    };
    fechar_porta_nativa(porta);
    // Sem o registro nem esta ponta, a fila da thread fecha e ela termina
    // depois das mensagens já entregues.
    drop(tx);
    true
}

/// As funções da API nativa, pelo nome de `dart_native_api.h`.
fn funcoes_da_api_nativa() -> [(&'static str, usize); 4] {
    [
        ("Dart_PostCObject", dartforge_api_post_cobject as usize),
        ("Dart_PostInteger", dartforge_api_post_integer as usize),
        ("Dart_NewNativePort", dartforge_api_new_native_port as usize),
        ("Dart_CloseNativePort", dartforge_api_close_native_port as usize),
    ]
}

/// `DartNativeApiFunctionPointer(nome)`: o endereço da função da API nativa.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartNativeApiFunctionPointer(nome: i64) -> i64 {
    let nome = HEAP.with(|h| h.borrow().texto(nome).para_string());
    match funcoes_da_api_nativa().iter().find(|(n, _)| *n == nome) {
        Some((_, f)) => *f as i64,
        None => {
            lancar_erro_de_argumento(&format!("Unknown native API function: {nome}"));
            0
        }
    }
}

/// A versão da API DL (`DART_API_DL_MAJOR_VERSION`/`MINOR`) da VM de
/// referência.
const VERSAO_API_DL: (i32, i32) = (2, 5);

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartApiDLMajorVersion() -> i64 {
    i64::from(VERSAO_API_DL.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartApiDLMinorVersion() -> i64 {
    i64::from(VERSAO_API_DL.1)
}

/// `DartApiEntry { const char* name; void (*function)(void); }`.
#[repr(C)]
struct EntradaDaApi {
    nome: *const c_char,
    funcao: usize,
}

/// `DartApi { const int major; const int minor; const DartApiEntry* const functions; }`.
#[repr(C)]
struct DadosDaApi {
    maior: i32,
    menor: i32,
    funcoes: *const EntradaDaApi,
}

/// O endereço dos dados, montados uma vez e vivos o processo inteiro.
fn dados_da_api() -> usize {
    static DADOS: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *DADOS.get_or_init(|| {
        let mut entradas: Vec<EntradaDaApi> = funcoes_da_api_nativa()
            .iter()
            .map(|(n, f)| EntradaDaApi { nome: CString::new(*n).expect("nome C").into_raw(), funcao: *f })
            .collect();
        entradas.push(EntradaDaApi { nome: std::ptr::null(), funcao: 0 });
        let funcoes = Box::leak(entradas.into_boxed_slice()).as_ptr();
        let dados = Box::leak(Box::new(DadosDaApi { maior: VERSAO_API_DL.0, menor: VERSAO_API_DL.1, funcoes }));
        dados as *const DadosDaApi as usize
    })
}

/// `DartApiDLInitializeData()`: o ponteiro que o C passa a
/// `Dart_InitializeApiDL`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartApiDLInitializeData() -> i64 {
    dados_da_api() as i64
}
