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
    if METODOS.with(|m| m.borrow().contains_key(&cid)) {
        return;
    }
    let t = f();
    // SAFETY: a tabela é uma constante do módulo: cid, n e os n pares.
    let n = unsafe { *t.add(1) } as usize;
    // SAFETY: os pares começam na terceira palavra.
    let pares = unsafe { t.add(2) };
    METODOS.with(|m| m.borrow_mut().insert(cid, (pares as usize, n)));
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

/// Entrada de um seletor que a classe do receptor não tem.
extern "C" fn dartforge_nsm_seletor(_recv: i64, _args: *const i64, _desc: *const i64) -> i64 {
    let texto = SELETOR_AUSENTE.with(|s| s.borrow().clone());
    let nome = texto.split_once(':').map_or(texto.as_str(), |(_, n)| n);
    let nome = nome.split_once('@').map_or(nome, |(n, _)| n);
    let h = HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(Texto::de_str(nome))));
    let erro = com_raizes(&[h], || dartforge_no_such_method_error_new(h));
    com_raizes(&[erro], || dartforge_exception_throw(erro, 3));
    0
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
    let achado = METODOS.with(|m| {
        let m = m.borrow();
        let &(p, n) = m.get(&cid)?;
        // SAFETY: registrado por `dartforge_registrar_metodos` com `n` pares.
        let pares = unsafe { std::slice::from_raw_parts(p as *const [i64; 2], n) };
        let i = pares.binary_search_by(|par| par[0].cmp(&hash)).ok()?;
        Some(pares[i][1] as usize)
    });
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
