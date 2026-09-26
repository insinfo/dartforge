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
    static METODOS: RefCell<HashMap<i64, (usize, usize)>> = RefCell::new(HashMap::new());
    /// Ids de classe (do SDK da fonte) dos valores que o runtime representa
    /// por conta própria, na ordem de `CID_*`; vazio sem o SDK da fonte.
    static CIDS_DO_RUNTIME: RefCell<Vec<i64>> = const { RefCell::new(Vec::new()) };
    /// O seletor da última busca que falhou (o texto do `NoSuchMethodError`).
    static SELETOR_AUSENTE: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Posições em `CIDS_DO_RUNTIME` (a ordem que o emissor grava).
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

/// A posição em `CIDS_DO_RUNTIME` da lista tipada interna de elementos
/// `tipo` (os `TIPO_*` de `typed_data.rs`); `ByteData` vira `Uint8List`.
fn cid_da_lista_tipada(tipo: u8) -> Option<usize> {
    Some(match tipo {
        TIPO_INT8 => CID_INT8_LIST,
        TIPO_UINT8 | TIPO_BYTE_DATA => CID_UINT8_LIST,
        TIPO_UINT8_CLAMPED => CID_UINT8_CLAMPED_LIST,
        TIPO_INT16 => CID_INT16_LIST,
        TIPO_UINT16 => CID_UINT16_LIST,
        TIPO_INT32 => CID_INT32_LIST,
        TIPO_UINT32 => CID_UINT32_LIST,
        TIPO_INT64 => CID_INT64_LIST,
        TIPO_UINT64 => CID_UINT64_LIST,
        TIPO_FLOAT32 => CID_FLOAT32_LIST,
        TIPO_FLOAT64 => CID_FLOAT64_LIST,
        TIPO_INT32X4 => CID_INT32X4_LIST,
        TIPO_FLOAT32X4 => CID_FLOAT32X4_LIST,
        TIPO_FLOAT64X2 => CID_FLOAT64X2_LIST,
        _ => return None,
    })
}

/// O id de classe (do SDK da fonte) na posição `pos` de `CIDS_DO_RUNTIME`;
/// `None` sem o SDK da fonte.
fn cid_registrado(pos: usize) -> Option<i64> {
    CIDS_DO_RUNTIME.with(|c| c.borrow().get(pos).copied().filter(|&c| c >= 0))
}

/// Registra a tabela de métodos da classe `cid`: `n` pares `[hash, entrada]`
/// ordenados pelo hash, numa constante do módulo (vive o processo inteiro).
///
/// # Safety
/// `pares` aponta para `2 * n` palavras legíveis durante todo o processo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_metodos(cid: i64, pares: *const i64, n: i64) {
    METODOS.with(|m| m.borrow_mut().insert(cid, (pares as usize, n as usize)));
}

/// Registra a tabela de métodos da classe `cid` pela função que a devolve
/// (`df.mt.<biblioteca>.<Classe>`, `{cid, n, [hash, entrada]…}`), se ainda
/// não registrada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_registrar_tabela(cid: i64, f: extern "C" fn() -> *const i64) {
    if !REPUBLICANDO.with(|r| r.get()) && METODOS.with(|m| m.borrow().contains_key(&cid)) {
        return;
    }
    let t = f();
    // SAFETY: a tabela é uma constante do módulo: cid, n e os n pares.
    let n = unsafe { *t.add(1) } as usize;
    // SAFETY: os pares começam na terceira palavra.
    let pares = unsafe { t.add(2) };
    METODOS.with(|m| m.borrow_mut().insert(cid, (pares as usize, n)));
}

thread_local! {
    /// Os registros da geração nova de uma recarga do JIT estão sendo
    /// refeitos: as tabelas de métodos substituem as já registradas.
    static REPUBLICANDO: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// A publicação de uma geração nova do programa (a recarga do JIT), no ponto
/// seguro do isolado: estende a área de globais ao layout da geração
/// (`area`, o `df.preparar_area` dela — agora, sem quadro nenhum na pilha,
/// para que um crescimento que precise de outra alocação não deixe um
/// `%area` antigo apontando para a aposentada); refaz os registros do programa (`registrar`, o
/// `df.registrar.programa` da geração: nomes, arestas de subtipo e tabelas
/// de métodos, que agora SUBSTITUEM as da geração anterior — uma classe pode
/// ter ganhado métodos) e as regras de supertipo da RTI (`rti`). Os
/// registros que acrescentam (arestas, regras) não duplicam.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_publicar_geracao(
    area: Option<extern "C" fn()>,
    registrar: Option<extern "C" fn()>,
    rti: Option<extern "C" fn()>,
) {
    if let Some(f) = area {
        f();
    }
    REPUBLICANDO.with(|r| r.set(true));
    if let Some(f) = registrar {
        f();
    }
    REPUBLICANDO.with(|r| r.set(false));
    if let Some(f) = rti {
        f();
    }
}

/// `dartforge_object_new` que registra a tabela de métodos da classe na
/// primeira alocação (SDK da fonte).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_object_new_t(cid: i64, campos: i64, f: extern "C" fn() -> *const i64) -> i64 {
    dartforge_registrar_tabela(cid, f);
    dartforge_object_new(cid, campos)
}

/// Registra os ids de classe dos valores do runtime (SDK da fonte).
///
/// # Safety
/// `ids` aponta para `n` palavras legíveis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_cids(ids: *const i64, n: i64) {
    // SAFETY: o emissor passa uma constante do módulo com `n` palavras.
    let v = unsafe { std::slice::from_raw_parts(ids, n as usize) }.to_vec();
    CIDS_DO_RUNTIME.with(|c| *c.borrow_mut() = v);
}

/// O id de classe (do SDK da fonte) de um valor que o runtime representa por
/// conta própria; `None` sem o SDK da fonte ou para um objeto comum.
fn cid_do_runtime(handle: i64) -> Option<i64> {
    CIDS_DO_RUNTIME.with(|c| {
        let c = c.borrow();
        if c.is_empty() {
            return None;
        }
        let pos = if handle == 0 {
            CID_NULL
        } else if crate::heap::smi::e_smi(handle) {
            CID_SMI
        } else {
            let pos = HEAP.with(|heap| {
                let heap = heap.borrow();
                Some(match heap.get(handle) {
                    Value::Object { .. } => return None,
                    Value::String(t) => {
                        if t.e_um_byte() {
                            CID_ONE_BYTE_STRING
                        } else {
                            CID_TWO_BYTE_STRING
                        }
                    }
                    Value::List(_) => {
                        if heap.imutaveis.contains(&handle) {
                            CID_IMMUTABLE_LIST
                        } else if heap.fixas.contains(&handle) {
                            CID_LIST
                        } else {
                            CID_GROWABLE_LIST
                        }
                    }
                    Value::Closure { .. } => CID_CLOSURE,
                    Value::BoxedInt(_) => CID_MINT,
                    Value::BoxedDouble(_) => CID_DOUBLE,
                    Value::BoxedBool(_) => CID_BOOL,
                    Value::Record(_) => CID_RECORD,
                    _ => return None,
                })
            });
            pos?
        };
        c.get(pos).copied()
    })
}

/// A entrada uniforme do seletor `hash` na tabela da classe `cid`, se ela tem.
fn metodo_da_classe(cid: i64, hash: i64) -> Option<usize> {
    METODOS.with(|m| {
        let m = m.borrow();
        let &(p, n) = m.get(&cid)?;
        // SAFETY: registrado por `dartforge_registrar_metodos` com `n` pares.
        let pares = unsafe { std::slice::from_raw_parts(p as *const [i64; 2], n) };
        let i = pares.binary_search_by(|par| par[0].cmp(&hash)).ok()?;
        Some(pares[i][1] as usize)
    })
}

/// Os nomes dos argumentos nomeados, pelo hash do descritor (o mesmo em
/// todo módulo e em todo isolado).
fn nomes_de_argumento() -> &'static std::sync::RwLock<HashMap<i64, String>> {
    static N: std::sync::OnceLock<std::sync::RwLock<HashMap<i64, String>>> = std::sync::OnceLock::new();
    N.get_or_init(Default::default)
}

/// Registra o nome de um argumento nomeado que algum descritor do módulo usa.
///
/// # Safety
/// `nome` aponta para `len` bytes UTF-8 de uma constante do módulo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_nome_de_argumento(nome: *const u8, len: i64) {
    // SAFETY: constante do módulo com `len` bytes.
    let bytes = unsafe { std::slice::from_raw_parts(nome, len as usize) };
    let nome = String::from_utf8_lossy(bytes).into_owned();
    let h = hash_do_nome(&nome);
    nomes_de_argumento().write().unwrap_or_else(|e| e.into_inner()).entry(h).or_insert(nome);
}

/// FNV-1a de 64 bits (`lower/closures.rs::hash_nome`).
fn hash_do_nome(nome: &str) -> i64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in nome.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h as i64
}

/// Entrada de um seletor que a classe do receptor não tem: como na VM, o
/// `noSuchMethod` do receptor recebe o `Invocation` (o de `Object` lança
/// `NoSuchMethodError`; uma classe pode sobrescrevê-lo e devolver um valor).
extern "C" fn dartforge_nsm_seletor(recv: i64, args: *const i64, desc: *const i64) -> i64 {
    let texto = SELETOR_AUSENTE.with(|s| s.borrow().clone());
    let (tipo, nome) = texto.split_once(':').unwrap_or(("c", texto.as_str()));
    let nome = nome.split_once('@').map_or(nome, |(n, _)| n);
    let codigo = match tipo {
        "g" => 1,
        "s" => 2,
        _ => 0,
    };
    // SAFETY: o descritor e o vetor de argumentos são os da chamada; a
    // chamada por seletor sempre leva a tupla de tipos depois dos argumentos
    // (0 sem argumentos de tipo).
    let (valores, hashes, tupla) = unsafe {
        let (npos, nnom) = (*desc as usize, *desc.add(1) as usize);
        (
            std::slice::from_raw_parts(args, npos + nnom).to_vec(),
            std::slice::from_raw_parts(desc.add(2), nnom).to_vec(),
            *args.add(npos + nnom),
        )
    };
    let npos = valores.len() - hashes.len();
    let nomes: Vec<String> = {
        let tabela = nomes_de_argumento().read().unwrap_or_else(|e| e.into_inner());
        hashes.iter().map(|h| tabela.get(h).cloned().unwrap_or_default()).collect()
    };
    let n = com_raizes(&[recv], || alocar_str(nome));
    let nomes: Vec<&str> = nomes.iter().map(String::as_str).collect();
    if let Some(r) = com_raizes(&[n], || invocar_no_such_method(recv, codigo, n, &valores, npos, &nomes, tupla)) {
        return r;
    }
    let erro = com_raizes(&[n], || dartforge_no_such_method_error_new(n));
    com_raizes(&[erro], || dartforge_exception_throw(erro, 3));
    0
}

/// Os argumentos de tipo de uma tupla RTI (`L<…>`), ou nenhum.
fn argumentos_da_tupla(tupla: i64) -> Vec<i64> {
    if tupla <= 0 {
        return Vec::new();
    }
    RTI.with(|u| match u.borrow().tipos.get(tupla as usize) {
        Some(Tipo::Tupla(v)) => v.clone(),
        _ => Vec::new(),
    })
}

/// `receptor.noSuchMethod(Invocation)` pelo `_dartforgeNoSuchMethod`
/// (`codigo`: 0 método, 1 getter, 2 setter): `valores` são os posicionais
/// e depois os nomeados, na ordem de `nomes`. `None` sem o SDK da fonte.
fn invocar_no_such_method(recv: i64, codigo: i64, nome: i64, valores: &[i64], npos: usize, nomes: &[&str], tupla: i64) -> Option<i64> {
    let f = ajudante("_dartforgeNoSuchMethod")?;
    let tipos = argumentos_da_tupla(tupla);
    // Tudo o que é alocado aqui fica num frame de raízes até a chamada
    // (cada alocação pode coletar).
    let frame = HEAP.with(|h| h.borrow_mut().push_frame_with_slots(valores.len() + nomes.len() + tipos.len() + 7));
    let mut proximo = 0;
    let mut enraizar = |x: i64| {
        HEAP.with(|h| h.borrow_mut().set_root(frame, proximo, x));
        proximo += 1;
        x
    };
    enraizar(recv);
    enraizar(nome);
    for &v in valores {
        enraizar(v);
    }
    let pos = enraizar(dart_lista_fixa(&valores[..npos]));
    let textos: Vec<i64> = nomes.iter().map(|t| enraizar(alocar_str(t))).collect();
    let nomes = enraizar(dart_lista_fixa(&textos));
    let vals = enraizar(dart_lista_fixa(&valores[npos..]));
    let objetos: Vec<i64> = tipos.iter().map(|&t| enraizar(dartforge_rti_objeto_tipo(t))).collect();
    let tipos = enraizar(dart_lista_fixa(&objetos));
    // SAFETY: registrado pelo `dart:core` com a assinatura
    // (Object?, int, String, List, List, List, List) -> Object?.
    let g: extern "C" fn(i64, i64, i64, i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(f) };
    let r = g(recv, codigo, nome, pos, nomes, vals, tipos);
    HEAP.with(|h| h.borrow_mut().pop_frame(frame));
    Some(r)
}

/// O encaminhador de `noSuchMethod` que o compilador gera na tabela de uma
/// classe com `noSuchMethod` próprio, para um membro de interface sem
/// implementação: `ambiente` tem os `npos` posicionais, os `nnom`
/// nomeados (todos, com os padrões) e depois os nomes deles, na ordem da
/// declaração.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_encaminhar_nsm(recv: i64, codigo: i64, nome: i64, ambiente: i64, npos: i64, nnom: i64, tupla: i64) -> i64 {
    let (npos, nnom) = (npos as usize, nnom as usize);
    let campos: Vec<TaggedValue> = HEAP.with(|h| match h.borrow().try_get(ambiente) {
        Some(Value::Environment(v)) => v.clone(),
        _ => Vec::new(),
    });
    if campos.len() != npos + 2 * nnom {
        return 0;
    }
    // O ambiente guarda os escalares sem caixa: cada valor volta à posição
    // `Ref` (pode alocar) e fica enraizado até a chamada.
    let frame = HEAP.with(|h| h.borrow_mut().push_frame_with_slots(npos + nnom + 3));
    let enraizar = |i: usize, x: i64| HEAP.with(|h| h.borrow_mut().set_root(frame, i, x));
    enraizar(0, recv);
    enraizar(1, nome);
    enraizar(2, ambiente);
    let mut valores = Vec::with_capacity(npos + nnom);
    for (i, t) in campos[..npos + nnom].iter().enumerate() {
        let r = HEAP.with(|h| h.borrow_mut().como_ref(*t));
        enraizar(3 + i, r);
        valores.push(r);
    }
    let nomes: Vec<String> =
        HEAP.with(|h| campos[npos + nnom..].iter().map(|t| h.borrow().texto(t.bits).para_string()).collect());
    let nomes: Vec<&str> = nomes.iter().map(String::as_str).collect();
    let r = invocar_no_such_method(recv, codigo, nome, &valores, npos, &nomes, tupla).unwrap_or(0);
    HEAP.with(|h| h.borrow_mut().pop_frame(frame));
    r
}

/// A entrada de `recv.<seletor>`, pelo cache do ponto de chamada
/// (`cache[0]` = id de classe + 1, `cache[1]` = entrada).
///
/// # Safety
/// `cache` aponta para duas palavras graváveis do módulo; `nome` para `len`
/// bytes legíveis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_seletor(cache: *mut i64, recv: i64, hash: i64, nome: *const u8, len: i64) -> usize {
    let cid = dartforge_value_class(recv);
    // SAFETY: o cache é um `[2 x i64]` privado do ponto de chamada.
    unsafe {
        if *cache == cid.wrapping_add(1) {
            return *cache.add(1) as usize;
        }
    }
    let achado = metodo_da_classe(cid, hash);
    match achado {
        Some(f) => {
            // SAFETY: ver acima.
            unsafe {
                *cache = cid.wrapping_add(1);
                *cache.add(1) = f as i64;
            }
            f
        }
        None => {
            // SAFETY: o nome é uma constante do módulo com `len` bytes.
            let bytes = unsafe { std::slice::from_raw_parts(nome, len as usize) };
            let texto = String::from_utf8_lossy(bytes).into_owned();
            if depurar() {
                let classe = CLASS_NAMES.with(|m| m.borrow().get(&cid).cloned()).unwrap_or_default();
                eprintln!("[depurar] seletor ausente: {texto} na classe {cid} {classe}");
            }
            SELETOR_AUSENTE.with(|s| *s.borrow_mut() = texto);
            dartforge_nsm_seletor as usize
        }
    }
}

/// Um membro do SDK da fonte que o backend recusou (P5c): o programa chegou
/// nele em tempo de execução. Nunca uma saída errada: a mensagem diz o
/// membro e o motivo, e o processo sai com 254 (o código de "erro de
/// compilação" do harness).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_membro_recusado(texto: i64) {
    let t = HEAP.with(|heap| heap.borrow().texto(texto).para_string());
    use std::io::Write;
    let _ = std::io::stdout().flush();
    let _ = writeln!(std::io::stderr().lock(), "erro: membro do SDK não suportado no backend nativo: {t}");
    std::process::exit(254);
}

/// `DARTFORGE_DEPURAR=1`: o runtime conta no stderr cada exceção lançada e
/// cada seletor que a classe do receptor não tem.
fn depurar() -> bool {
    thread_local! {
        static D: bool = std::env::var("DARTFORGE_DEPURAR").is_ok_and(|v| v == "1");
    }
    D.with(|d| *d)
}

thread_local! {
    /// A pilha de funções Dart (`DARTFORGE_RASTRO=1` na compilação).
    static RASTRO: RefCell<Vec<(usize, usize)>> = const { RefCell::new(Vec::new()) };
}

/// Entrada de uma função (rastro de depuração).
///
/// # Safety
/// `nome` aponta para `len` bytes de uma constante do módulo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_rastro_entrada(nome: *const u8, len: i64) {
    RASTRO.with(|r| r.borrow_mut().push((nome as usize, len as usize)));
}

/// Saída de uma função (rastro de depuração).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rastro_saida() {
    RASTRO.with(|r| {
        r.borrow_mut().pop();
    });
}

/// A pilha de funções Dart corrente, da mais interna para fora.
fn mostrar_rastro() {
    RASTRO.with(|r| {
        for &(p, n) in r.borrow().iter().rev().take(25) {
            // SAFETY: constantes do módulo registradas por `dartforge_rastro_entrada`.
            let b = unsafe { std::slice::from_raw_parts(p as *const u8, n) };
            eprintln!("    em {}", String::from_utf8_lossy(b));
        }
    });
}
