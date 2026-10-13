// Runtime nativo: tabelas de métodos por classe e a busca por seletor, para o
// SDK compilado da fonte (P5c, δ; `crates/emit_native/src/llvm/seletores.rs`).
//
// Cada classe registra na partida a sua tabela: pares (hash do seletor,
// entrada uniforme), ordenados pelo hash. Uma chamada por seletor passa pelo
// cache do ponto de chamada (id de classe + 1, entrada); na falha, busca na
// tabela da classe dinâmica do receptor. O seletor que nenhuma classe tem dá
// a entrada de `NoSuchMethodError`.

thread_local! {
    /// Tabela de métodos por id de classe: (ponteiro para os pares, quantos).
    /// As tabelas de métodos por classe: `n` pares `[hash, input]`
    /// ordenados pelo hash, **copiados** da constante do módulo — a memória
    /// de uma geração do JIT pode ser liberada depois de uma recarga (J02), e
    /// um `Arc` porque as mensagens entre isolados levam a tabela junto.
    static METHODS: RefCell<HashMap<i64, MethodTable>> = RefCell::new(HashMap::default());
    /// As classes com tabela já registrada, por id (o teste de toda alocação
    /// em `dartforge_object_new_t`, sem o hash de `METHODS`). Só cresce, como
    /// `METHODS`.
    static REGISTERED: RefCell<Vec<bool>> = const { RefCell::new(Vec::new()) };
    /// O seletor da última busca que falhou (o texto do `NoSuchMethodError`).
    static MISSING_SELECTOR: RefCell<String> = const { RefCell::new(String::new()) };
}

/// As posições da tabela de 32 classes do runtime que o emissor grava
/// (`sdk_modulo::cids_do_runtime`, `@df.cids`). Com os cids fixos
/// (docs/NATIVO-ESPACO-UNIFICADO.md §2.4) a tabela é só a conferência de ABI
/// de [`dartforge_registrar_cids`]; as posições ficam para quem ainda pede o
/// cid por elas.
const CID_NULL: usize = 0;
const CID_SMI: usize = 1;
const CID_MINT: usize = 2;
const CID_DOUBLE: usize = 3;
const CID_BOOL: usize = 4;
const CID_ONE_BYTE_STRING: usize = 5;
const CID_TWO_BYTE_STRING: usize = 6;
const CID_GROWABLE_LIST: usize = 7;
const CID_LIST: usize = 8;
const CID_IMMUTABLE_LIST: usize = 9;
const CID_CLOSURE: usize = 10;
const CID_RECORD: usize = 11;
const CID_UINT8_LIST: usize = 12;
const CID_UINT8_VIEW: usize = 13;
const CID_INT64_LIST: usize = 14;
// 15–17: os valores SIMD (`simd.rs`).
const CID_INT8_LIST: usize = 18;
const CID_UINT8_CLAMPED_LIST: usize = 19;
const CID_INT16_LIST: usize = 20;
const CID_UINT16_LIST: usize = 21;
const CID_INT32_LIST: usize = 22;
const CID_UINT32_LIST: usize = 23;
const CID_UINT64_LIST: usize = 24;
const CID_FLOAT32_LIST: usize = 25;
const CID_FLOAT64_LIST: usize = 26;
const CID_INT32X4_LIST: usize = 27;
const CID_FLOAT32X4_LIST: usize = 28;
const CID_FLOAT64X2_LIST: usize = 29;
const CID_SEND_PORT: usize = 30;
const CID_CAPABILITY: usize = 31;

/// O cid fixo de cada posição da tabela das classes do runtime (a ordem de
/// `CID_*`; `layout::cid`).
const FIXED_CIDS_BY_POSITION: [i32; 32] = {
    use crate::layout::cid;
    [
        cid::NULL,
        cid::SMI,
        cid::MINT,
        cid::DOUBLE,
        cid::BOOL,
        cid::ONE_BYTE_STRING,
        cid::TWO_BYTE_STRING,
        cid::GROWABLE_LIST,
        cid::LIST,
        cid::IMMUTABLE_LIST,
        cid::CLOSURE,
        cid::RECORD,
        cid::PRIMEIRA_TIPADA + 1,  // _Uint8List
        cid::PRIMEIRA_VISAO + 1,   // _Uint8ArrayView
        cid::PRIMEIRA_TIPADA + 7,  // _Int64List
        cid::FLOAT32X4,
        cid::INT32X4,
        cid::FLOAT64X2,
        cid::PRIMEIRA_TIPADA,      // _Int8List
        cid::PRIMEIRA_TIPADA + 2,  // _Uint8ClampedList
        cid::PRIMEIRA_TIPADA + 3,  // _Int16List
        cid::PRIMEIRA_TIPADA + 4,  // _Uint16List
        cid::PRIMEIRA_TIPADA + 5,  // _Int32List
        cid::PRIMEIRA_TIPADA + 6,  // _Uint32List
        cid::PRIMEIRA_TIPADA + 8,  // _Uint64List
        cid::PRIMEIRA_TIPADA + 9,  // _Float32List
        cid::PRIMEIRA_TIPADA + 10, // _Float64List
        cid::PRIMEIRA_TIPADA + 12, // _Int32x4List
        cid::PRIMEIRA_TIPADA + 11, // _Float32x4List
        cid::PRIMEIRA_TIPADA + 13, // _Float64x2List
        cid::SEND_PORT,
        cid::CAPABILITY,
    ]
};

const _: () = {
    // As posições nomeadas casam com a tabela.
    assert!(FIXED_CIDS_BY_POSITION[CID_NULL] == crate::layout::cid::NULL);
    assert!(FIXED_CIDS_BY_POSITION[CID_SMI] == crate::layout::cid::SMI);
    assert!(FIXED_CIDS_BY_POSITION[CID_UINT8_LIST] == crate::layout::cid::tipada(1));
    assert!(FIXED_CIDS_BY_POSITION[CID_UINT8_VIEW] == crate::layout::cid::visao(1, false));
    assert!(FIXED_CIDS_BY_POSITION[CID_INT64_LIST] == crate::layout::cid::tipada(7));
    assert!(FIXED_CIDS_BY_POSITION[CID_INT8_LIST] == crate::layout::cid::tipada(0));
    assert!(FIXED_CIDS_BY_POSITION[CID_FLOAT64X2_LIST] == crate::layout::cid::tipada(13));
    assert!(FIXED_CIDS_BY_POSITION[CID_SEND_PORT] == crate::layout::cid::SEND_PORT);
    assert!(FIXED_CIDS_BY_POSITION[CID_CAPABILITY] == crate::layout::cid::CAPABILITY);
    assert!(CID_MINT + CID_DOUBLE + CID_BOOL + CID_ONE_BYTE_STRING + CID_TWO_BYTE_STRING > 0);
    assert!(CID_GROWABLE_LIST + CID_LIST + CID_IMMUTABLE_LIST + CID_CLOSURE + CID_RECORD > 0);
    assert!(CID_UINT8_CLAMPED_LIST + CID_INT16_LIST + CID_UINT16_LIST + CID_INT32_LIST + CID_UINT32_LIST > 0);
    assert!(CID_UINT64_LIST + CID_FLOAT32_LIST + CID_FLOAT64_LIST + CID_INT32X4_LIST + CID_FLOAT32X4_LIST > 0);
};

/// Os pares `[hash, input]` de uma classe, em memória do runtime.
pub type MethodTable = std::sync::Arc<[[i64; 2]]>;

/// Copia `n` pares de uma constante do módulo.
///
/// # Safety
/// `pair_list` aponta para `2 * n` palavras legíveis.
unsafe fn copy_table(pair_list: *const i64, n: usize) -> MethodTable {
    // SAFETY: garantido por quem chama.
    unsafe { std::slice::from_raw_parts(pair_list as *const [i64; 2], n) }.into()
}

/// Registra a tabela de métodos da classe `cid`: `n` pares `[hash, input]`
/// ordenados pelo hash, numa constante do módulo, que é copiada.
///
/// # Safety
/// `pair_list` aponta para `2 * n` palavras legíveis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_metodos(cid: i64, pair_list: *const i64, n: i64) {
    // SAFETY: garantido por quem chama.
    let t = unsafe { copy_table(pair_list, n as usize) };
    METHODS.with(|m| m.borrow_mut().insert(cid, t));
    mark_registered(cid);
}

/// Registra a tabela de métodos da classe `cid` pela função que a devolve
/// (`df.mt.<library>.<Class>`, `{cid, n, [hash, input]…}`), se ainda
/// não registrada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_registrar_tabela(cid: i64, f: extern "C" fn() -> *const i64) {
    if already_registered(cid) && !REPUBLISHING.with(|r| r.get()) {
        return;
    }
    let t = f();
    // SAFETY: a tabela é uma constante do módulo: cid, n e os n pares
    // (a partir da terceira palavra), copiados aqui.
    let tabela = unsafe { copy_table(t.add(2), *t.add(1) as usize) };
    METHODS.with(|m| m.borrow_mut().insert(cid, tabela));
    mark_registered(cid);
}

/// Até que id de classe o vetor [`REGISTERED`] vale: os ids do programa e
/// do SDK são densos e pequenos; os internos do runtime (a classe `Type`,
/// 0x3FFF_FF01…) ficam com o mapa.
const REGISTERED_UP_TO: usize = 1 << 16;

/// Publica [`REGISTERED`] no contexto da thread (a alocação em linha de
/// `dartforge_object_new_t` confere o registro por ali, `llvm/mod.rs`).
fn publish_registered(r: &[bool]) {
    crate::heap::CONTEXTO.with(|c| {
        c.registered.set(r.as_ptr().cast());
        c.n_registered.set(r.len());
    });
}

/// A classe `cid` já tem tabela registrada?
fn already_registered(cid: i64) -> bool {
    match usize::try_from(cid) {
        Ok(i) if i < REGISTERED_UP_TO => REGISTERED.with(|r| r.borrow().get(i).copied().unwrap_or(false)),
        _ => METHODS.with(|m| m.borrow().contains_key(&cid)),
    }
}

fn mark_registered(cid: i64) {
    if let Ok(i) = usize::try_from(cid)
        && i < REGISTERED_UP_TO
    {
        REGISTERED.with(|r| {
            let mut r = r.borrow_mut();
            if r.len() <= i {
                r.resize(i + 1, false);
            }
            r[i] = true;
            publish_registered(&r);
        });
    }
}

thread_local! {
    /// Os registros da geração nova de uma recarga do JIT estão sendo
    /// refeitos: as tabelas de métodos substituem as já registradas.
    static REPUBLISHING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// A publicação de uma geração nova do programa (a recarga do JIT), no ponto
/// seguro do isolado: estende a área de globais ao layout da geração
/// (`area`, o `df.preparar_area` dela — agora, sem quadro nenhum na pilha,
/// para que um crescimento que precise de outra alocação não deixe um
/// `%area` antigo apontando para a aposentada); refaz os registros do programa (`register`, o
/// `df.register.program` da geração: nomes, arestas de subtipo e tabelas
/// de métodos, que agora SUBSTITUEM as da geração anterior — uma classe pode
/// ter ganhado métodos) e as regras de supertipo da RTI (`rti`). Os
/// registros que acrescentam (arestas, regras) não duplicam.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_publicar_geracao(
    area: Option<extern "C" fn()>,
    register: Option<extern "C" fn()>,
    rti: Option<extern "C" fn()>,
) {
    // Os objetos vivos deste isolado passam ao layout da geração nova (J03).
    apply_pending_migration();
    if let Some(f) = area {
        f();
    }
    REPUBLISHING.with(|r| r.set(true));
    // Enquanto os registros se refazem, toda alocação passa pelo runtime,
    // que registra de novo (`dartforge_object_new_t`).
    crate::heap::CONTEXTO.with(|c| c.n_registered.set(0));
    if let Some(f) = register {
        f();
    }
    REPUBLISHING.with(|r| r.set(false));
    REGISTERED.with(|r| publish_registered(&r.borrow()));
    if let Some(f) = rti {
        f();
    }
    // Nada deste isolado aponta mais para a memória das gerações anteriores.
    if area.is_some() {
        forget_previous_area_generations();
    }
    // As tabelas de métodos mudaram: os caches dos pontos de chamada (os do
    // código novo e os do que continua de gerações anteriores, J04) buscam
    // de novo.
    empty_area_caches();
}

/// `dartforge_object_new` que registra a tabela de métodos da classe na
/// primeira alocação (SDK da fonte).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_object_new_t(cid: i64, fields: i64, f: extern "C" fn() -> *const i64) -> i64 {
    dartforge_registrar_tabela(cid, f);
    dartforge_object_new(cid, fields)
}

/// A conferência de ABI do módulo (docs/NATIVO-ESPACO-UNIFICADO.md §2.4,
/// §3.6): as classes do runtime têm cids fixos, e um módulo compilado com
/// outra numeração que a deste runtime não pode rodar sobre ele. `ids` é a
/// tabela do módulo: os cids de `layout::cid::DO_SDK` em ordem, ou as 32
/// posições de [`FIXED_CIDS_BY_POSITION`] (uma classe que o SDK carregado
/// não tem vem como -1). Diferente: o processo aborta com a mensagem.
///
/// # Safety
/// `ids` aponta para `n` palavras legíveis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_cids(ids: *const i64, n: i64) {
    // SAFETY: o emissor passa uma constante do módulo com `n` palavras.
    let v = unsafe { std::slice::from_raw_parts(ids, usize::try_from(n).unwrap_or(0)) };
    let expected: Vec<i64> = if v.len() == crate::layout::cid::DO_SDK.len() {
        crate::layout::cid::DO_SDK.iter().map(|&(c, _, _)| i64::from(c)).collect()
    } else {
        FIXED_CIDS_BY_POSITION.iter().map(|&c| i64::from(c)).collect()
    };
    let checks = v.len() == expected.len() && v.iter().zip(&expected).all(|(&m, &r)| m == r || m == -1);
    if !checks {
        use std::io::Write;
        let _ = std::io::stdout().flush();
        let _ = writeln!(
            std::io::stderr().lock(),
            "erro: o módulo foi compilado com outra tabela de classes do runtime (ABI): módulo {v:?}, runtime {expected:?}"
        );
        std::process::abort();
    }
}

/// A entrada uniforme do seletor `hash` na tabela da classe `cid`, se ela tem.
fn class_method(cid: i64, hash: i64) -> Option<usize> {
    METHODS.with(|m| {
        let m = m.borrow();
        let pair_list = m.get(&cid)?;
        let i = pair_list.binary_search_by(|pair| pair[0].cmp(&hash)).ok()?;
        Some(pair_list[i][1] as usize)
    })
}

/// Os nomes dos argumentos nomeados, pelo hash do descritor (o mesmo em
/// todo módulo e em todo isolado).
fn argument_names() -> &'static std::sync::RwLock<HashMap<i64, String>> {
    static N: std::sync::OnceLock<std::sync::RwLock<HashMap<i64, String>>> = std::sync::OnceLock::new();
    N.get_or_init(Default::default)
}

/// Registra o nome de um argumento nomeado que algum descritor do módulo usa.
///
/// # Safety
/// `name` aponta para `len` bytes UTF-8 de uma constante do módulo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_nome_de_argumento(name: *const u8, len: i64) {
    // SAFETY: constante do módulo com `len` bytes.
    let bytes = unsafe { std::slice::from_raw_parts(name, len as usize) };
    let name = String::from_utf8_lossy(bytes).into_owned();
    let h = name_hash(&name);
    argument_names().write().unwrap_or_else(|e| e.into_inner()).entry(h).or_insert(name);
}

/// FNV-1a de 64 bits (`lower/closures.rs::hash_nome`).
fn name_hash(name: &str) -> i64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h as i64
}

/// Entrada de um seletor que a classe do receptor não tem: como na VM, o
/// `noSuchMethod` do receptor recebe o `Invocation` (o de `Object` lança
/// `NoSuchMethodError`; uma classe pode sobrescrevê-lo e devolver um valor).
extern "C" fn dartforge_nsm_seletor(recv: i64, args: *const i64, desc: *const i64) -> i64 {
    let text = MISSING_SELECTOR.with(|s| s.borrow().clone());
    let (tipo, name) = text.split_once(':').unwrap_or(("c", text.as_str()));
    let name = name.split_once('@').map_or(name, |(n, _)| n);
    let code = match tipo {
        "g" => 1,
        "s" => 2,
        _ => 0,
    };
    // SAFETY: o descritor e o vetor de argumentos são os da chamada; a
    // chamada por seletor sempre leva a tupla de tipos depois dos argumentos
    // (0 sem argumentos de tipo).
    let (value_list, hashes, tuple) = unsafe {
        let (npos, nnom) = (*desc as usize, *desc.add(1) as usize);
        (
            std::slice::from_raw_parts(args, npos + nnom).to_vec(),
            std::slice::from_raw_parts(desc.add(2), nnom).to_vec(),
            *args.add(npos + nnom),
        )
    };
    let npos = value_list.len() - hashes.len();
    let names: Vec<String> = {
        let tabela = argument_names().read().unwrap_or_else(|e| e.into_inner());
        hashes.iter().map(|h| tabela.get(h).cloned().unwrap_or_default()).collect()
    };
    let n = with_roots(&[recv], || HEAP.with(|h| h.borrow_mut().allocate_str(name)));
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    if let Some(r) = with_roots(&[n], || invocar_no_such_method(recv, code, n, &value_list, npos, &names, tuple)) {
        return r;
    }
    let error = with_roots(&[n], || dartforge_no_such_method_error_new(n));
    with_roots(&[error], || dartforge_exception_throw(error, 3));
    0
}

/// Os argumentos de tipo de uma tupla RTI (`L<…>`), ou nenhum.
fn tuple_arguments(tuple: i64) -> Vec<i64> {
    if tuple <= 0 {
        return Vec::new();
    }
    RTI.with(|u| match u.borrow().get_value(tuple) {
        Some(Type::Tuple(v)) => v.clone(),
        _ => Vec::new(),
    })
}

/// `receiver_obj.noSuchMethod(Invocation)` pelo `_dartforgeNoSuchMethod`
/// (`code`: 0 método, 1 getter, 2 setter): `value_list` são os posicionais
/// e depois os nomeados, na ordem de `names`. `None` sem o SDK da fonte.
fn invocar_no_such_method(recv: i64, code: i64, name: i64, value_list: &[i64], npos: usize, names: &[&str], tuple: i64) -> Option<i64> {
    let f = ajudante("_dartforgeNoSuchMethod")?;
    let types = tuple_arguments(tuple);
    // Tudo o que é alocado aqui fica num frame de raízes até a chamada
    // (cada alocação pode coletar).
    let frame = HEAP.with(|h| h.borrow_mut().push_frame_with_slots(value_list.len() + names.len() + types.len() + 7));
    let mut next_offset = 0;
    let mut root_it = |x: i64| {
        HEAP.with(|h| h.borrow_mut().set_root(frame, next_offset, x));
        next_offset += 1;
        x
    };
    root_it(recv);
    root_it(name);
    for &v in value_list {
        root_it(v);
    }
    let pos = root_it(dart_fixed_list(&value_list[..npos]));
    let textos: Vec<i64> = names.iter().map(|t| root_it(HEAP.with(|h| h.borrow_mut().allocate_str(t)))).collect();
    let names = root_it(dart_fixed_list(&textos));
    let vals = root_it(dart_fixed_list(&value_list[npos..]));
    let objects: Vec<i64> = types.iter().map(|&t| root_it(dartforge_rti_objeto_tipo(t))).collect();
    let types = root_it(dart_fixed_list(&objects));
    // SAFETY: registrado pelo `dart:core` com a assinatura
    // (Object?, int, String, List, List, List, List) -> Object?.
    let g = { let dart_target: usize = f; move |a0: i64, a1: i64, a2: i64, a3: i64, a4: i64, a5: i64, a6: i64| -> i64 { dart_r7(dart_target, a0, a1, a2, a3, a4, a5, a6) } };
    let r = g(recv, code, name, pos, names, vals, types);
    HEAP.with(|h| h.borrow_mut().pop_frame(frame));
    Some(r)
}

/// O encaminhador de `noSuchMethod` que o compilador gera na tabela de uma
/// classe com `noSuchMethod` próprio, para um membro de interface sem
/// implementação: `env` tem os `npos` posicionais, os `nnom`
/// nomeados (todos, com os padrões) e depois os nomes deles, na ordem da
/// declaração.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_encaminhar_nsm(recv: i64, code: i64, name: i64, env: i64, npos: i64, nnom: i64, tuple: i64) -> i64 {
    let (npos, nnom) = (npos as usize, nnom as usize);
    // O `_Contexto` do encaminhador (`lower/sdk_fonte.rs`, `gerar_encaminhador_nsm`) guarda todos os valores
    // como `Ref` (encaixotados pelo compilador) e depois os nomes. Sem
    // argumento nenhum o contexto é `null` (`AllocEnv` vazio não aloca).
    let fields: Vec<i64> = HEAP.with(|h| {
        let h = h.borrow();
        if !crate::layout::is_object(env) {
            return Vec::new();
        }
        let n = h.object(env).map_or(0, |o| o.len());
        (0..n).map(|i| h.capture(env, i).0).collect()
    });
    if fields.len() != npos + 2 * nnom {
        return 0;
    }
    let frame = HEAP.with(|h| h.borrow_mut().push_frame_with_slots(3));
    let root_it = |i: usize, x: i64| HEAP.with(|h| h.borrow_mut().set_root(frame, i, x));
    root_it(0, recv);
    root_it(1, name);
    root_it(2, env);
    let value_list: Vec<i64> = fields[..npos + nnom].to_vec();
    let names: Vec<String> = HEAP.with(|h| {
        let h = h.borrow();
        fields[npos + nnom..].iter().map(|&t| h.text(t).map(|t| t.para_string()).unwrap_or_default()).collect()
    });
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let r = invocar_no_such_method(recv, code, name, &value_list, npos, &names, tuple).unwrap_or(0);
    HEAP.with(|h| h.borrow_mut().pop_frame(frame));
    r
}

/// A entrada de `recv.<selector>`, pelo cache do ponto de chamada
/// (`cache[0]` = id de classe + 1, `cache[1]` = entrada).
///
/// # Safety
/// `cache` aponta para duas palavras graváveis do módulo; `name` para `len`
/// bytes legíveis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_seletor(cache: *mut i64, recv: i64, hash: i64, name: *const u8, len: i64) -> usize {
    let cid = dartforge_value_class(recv);
    // SAFETY: o cache é um `[2 x i64]` privado do ponto de chamada.
    unsafe {
        if *cache == cid.wrapping_add(1) {
            return *cache.add(1) as usize;
        }
    }
    let mut found_item = class_method(cid, hash);
    // O seletor da chamada tipada (`tc:m`, `ts:x`; `lower/entrada_tipada.rs`)
    // só está na tabela dos métodos que têm a entrada tipada: sem ela, a
    // entrada de sempre (`c:m`, `s:x`), que também serve — confere mais.
    // SAFETY: o nome é uma constante do módulo com `len` bytes.
    let bytes = unsafe { std::slice::from_raw_parts(name, len as usize) };
    let typed_one = bytes.first() == Some(&b't');
    if found_item.is_none() && typed_one {
        let sem_t = String::from_utf8_lossy(&bytes[1..]);
        found_item = class_method(cid, name_hash(&sem_t));
    }
    match found_item {
        Some(f) => {
            // SAFETY: ver acima.
            unsafe {
                *cache = cid.wrapping_add(1);
                *cache.add(1) = f as i64;
            }
            f
        }
        None => {
            let text = String::from_utf8_lossy(if typed_one { &bytes[1..] } else { bytes }).into_owned();
            if debug() {
                let class = CLASS_NAMES.with(|m| m.borrow().get(&cid).cloned()).unwrap_or_default();
                eprintln!("[depurar] seletor ausente: {text} na classe {cid} {class}");
            }
            MISSING_SELECTOR.with(|s| *s.borrow_mut() = text);
            dartforge_nsm_seletor as usize
        }
    }
}

/// Um membro do SDK da fonte que o backend recusou (P5c): o programa chegou
/// nele em tempo de execução. Nunca uma saída errada: a mensagem diz o
/// membro e o motivo, e o processo sai com 254 (o código de "erro de
/// compilação" do harness).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_membro_recusado(text: i64) {
    let t = HEAP.with(|heap| heap.borrow().text(text).map(|t| t.para_string()).unwrap_or_default());
    use std::io::Write;
    let _ = std::io::stdout().flush();
    let _ = writeln!(std::io::stderr().lock(), "erro: membro do SDK não suportado no backend nativo: {t}");
    std::process::exit(254);
}

/// `DARTFORGE_DEPURAR=1`: o runtime conta no stderr cada exceção lançada e
/// cada seletor que a classe do receptor não tem.
fn debug() -> bool {
    thread_local! {
        static D: bool = std::env::var("DARTFORGE_DEPURAR").is_ok_and(|v| v == "1");
    }
    D.with(|d| *d)
}

thread_local! {
    /// A pilha de funções Dart (`DARTFORGE_RASTRO=1` na compilação).
    static TRACE: RefCell<Vec<(usize, usize)>> = const { RefCell::new(Vec::new()) };
}

/// Entrada de uma função (rastro de depuração).
///
/// # Safety
/// `name` aponta para `len` bytes de uma constante do módulo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_rastro_entrada(name: *const u8, len: i64) {
    TRACE.with(|r| r.borrow_mut().push((name as usize, len as usize)));
}

/// Saída de uma função (rastro de depuração).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rastro_saida() {
    TRACE.with(|r| {
        r.borrow_mut().pop();
    });
}

/// A pilha de funções Dart corrente, da mais interna para fora.
fn show_trace() {
    TRACE.with(|r| {
        for &(p, n) in r.borrow().iter().rev().take(25) {
            // SAFETY: constantes do módulo registradas por `dartforge_rastro_entrada`.
            let b = unsafe { std::slice::from_raw_parts(p as *const u8, n) };
            eprintln!("    em {}", String::from_utf8_lossy(b));
        }
    });
}

/// A migração dos layouts de objeto da última recarga estrutural (J03), com a
/// época dela: por classe, a posição antiga de cada campo novo (`-1`: nenhuma).
fn pending_migration() -> &'static std::sync::Mutex<Option<(u64, std::sync::Arc<HashMap<i64, Vec<i64>>>)>> {
    static M: std::sync::OnceLock<std::sync::Mutex<Option<(u64, std::sync::Arc<HashMap<i64, Vec<i64>>>)>>> =
        std::sync::OnceLock::new();
    M.get_or_init(Default::default)
}

/// Define a migração da publicação em curso (J03): `data` é
/// `[n, (class, novo_len, origin…)…]`. Chamada pelo JIT no ponto seguro do
/// isolado principal, com os demais parados; cada isolado a aplica ao seu
/// heap em [`dartforge_publicar_geracao`].
///
/// # Safety
/// `data` aponta para `n` palavras legíveis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_definir_migracao(data: *const i64, n: i64) {
    // SAFETY: garantido por quem chama; copiado aqui.
    let v = unsafe { std::slice::from_raw_parts(data, usize::try_from(n).unwrap_or(0)) };
    let mut plan: HashMap<i64, Vec<i64>> = HashMap::default();
    let mut i = 1;
    for _ in 0..v.first().copied().unwrap_or(0) {
        let (Some(&class), Some(&len)) = (v.get(i), v.get(i + 1)) else { break };
        let len = usize::try_from(len).unwrap_or(0);
        let Some(origin) = v.get(i + 2..i + 2 + len) else { break };
        // As classes do runtime (cid < 128) têm layout fixo, igual em toda
        // geração: um plano que as mude é recusado (§2.13).
        if class < crate::layout::PRIMEIRO_CID_LIVRE {
            use std::io::Write;
            let _ = writeln!(std::io::stderr().lock(), "erro: migração recusada para a classe do runtime {class}");
        } else {
            plan.insert(class, origin.to_vec());
        }
        i += 2 + len;
    }
    let epoch = crate::heap::LAYOUT_EPOCH.fetch_add(1, std::sync::atomic::Ordering::AcqRel) + 1;
    *pending_migration().lock().unwrap_or_else(|e| e.into_inner()) = Some((epoch, std::sync::Arc::new(plan)));
}

/// Aplica ao heap deste isolado a migração pendente, se ele ainda não está na
/// época dela. Um heap criado depois da migração já nasce na época nova.
fn apply_pending_migration() {
    let current = crate::heap::LAYOUT_EPOCH.load(std::sync::atomic::Ordering::Acquire);
    let plan = pending_migration().lock().unwrap_or_else(|e| e.into_inner()).clone();
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        if h.layout_epoch >= current {
            return;
        }
        if let Some((epoch, plan)) = plan
            && epoch == current
        {
            h.migrate_instances(&plan);
        }
        h.layout_epoch = current;
    });
}
