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
    /// As tabelas de métodos por classe: `n` pares `[hash, entrada]`
    /// ordenados pelo hash, **copiados** da constante do módulo — a memória
    /// de uma geração do JIT pode ser liberada depois de uma recarga (J02), e
    /// um `Arc` porque as mensagens entre isolados levam a tabela junto.
    static METODOS: RefCell<HashMap<i64, TabelaDeMetodos>> = RefCell::new(HashMap::default());
    /// As classes com tabela já registrada, por id (o teste de toda alocação
    /// em `dartforge_object_new_t`, sem o hash de `METODOS`). Só cresce, como
    /// `METODOS`.
    static REGISTRADAS: RefCell<Vec<bool>> = const { RefCell::new(Vec::new()) };
    /// O seletor da última busca que falhou (o texto do `NoSuchMethodError`).
    static SELETOR_AUSENTE: RefCell<String> = const { RefCell::new(String::new()) };
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
const CIDS_FIXOS_POR_POSICAO: [i32; 32] = {
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
    assert!(CIDS_FIXOS_POR_POSICAO[CID_NULL] == crate::layout::cid::NULL);
    assert!(CIDS_FIXOS_POR_POSICAO[CID_SMI] == crate::layout::cid::SMI);
    assert!(CIDS_FIXOS_POR_POSICAO[CID_UINT8_LIST] == crate::layout::cid::tipada(1));
    assert!(CIDS_FIXOS_POR_POSICAO[CID_UINT8_VIEW] == crate::layout::cid::visao(1, false));
    assert!(CIDS_FIXOS_POR_POSICAO[CID_INT64_LIST] == crate::layout::cid::tipada(7));
    assert!(CIDS_FIXOS_POR_POSICAO[CID_INT8_LIST] == crate::layout::cid::tipada(0));
    assert!(CIDS_FIXOS_POR_POSICAO[CID_FLOAT64X2_LIST] == crate::layout::cid::tipada(13));
    assert!(CIDS_FIXOS_POR_POSICAO[CID_SEND_PORT] == crate::layout::cid::SEND_PORT);
    assert!(CIDS_FIXOS_POR_POSICAO[CID_CAPABILITY] == crate::layout::cid::CAPABILITY);
    assert!(CID_MINT + CID_DOUBLE + CID_BOOL + CID_ONE_BYTE_STRING + CID_TWO_BYTE_STRING > 0);
    assert!(CID_GROWABLE_LIST + CID_LIST + CID_IMMUTABLE_LIST + CID_CLOSURE + CID_RECORD > 0);
    assert!(CID_UINT8_CLAMPED_LIST + CID_INT16_LIST + CID_UINT16_LIST + CID_INT32_LIST + CID_UINT32_LIST > 0);
    assert!(CID_UINT64_LIST + CID_FLOAT32_LIST + CID_FLOAT64_LIST + CID_INT32X4_LIST + CID_FLOAT32X4_LIST > 0);
};

/// Os pares `[hash, entrada]` de uma classe, em memória do runtime.
pub type TabelaDeMetodos = std::sync::Arc<[[i64; 2]]>;

/// Copia `n` pares de uma constante do módulo.
///
/// # Safety
/// `pares` aponta para `2 * n` palavras legíveis.
unsafe fn copiar_tabela(pares: *const i64, n: usize) -> TabelaDeMetodos {
    // SAFETY: garantido por quem chama.
    unsafe { std::slice::from_raw_parts(pares as *const [i64; 2], n) }.into()
}

/// Registra a tabela de métodos da classe `cid`: `n` pares `[hash, entrada]`
/// ordenados pelo hash, numa constante do módulo, que é copiada.
///
/// # Safety
/// `pares` aponta para `2 * n` palavras legíveis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_metodos(cid: i64, pares: *const i64, n: i64) {
    // SAFETY: garantido por quem chama.
    let t = unsafe { copiar_tabela(pares, n as usize) };
    METODOS.with(|m| m.borrow_mut().insert(cid, t));
    marcar_registrada(cid);
}

/// Registra a tabela de métodos da classe `cid` pela função que a devolve
/// (`df.mt.<biblioteca>.<Classe>`, `{cid, n, [hash, entrada]…}`), se ainda
/// não registrada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_registrar_tabela(cid: i64, f: extern "C" fn() -> *const i64) {
    if ja_registrada(cid) && !REPUBLICANDO.with(|r| r.get()) {
        return;
    }
    let t = f();
    // SAFETY: a tabela é uma constante do módulo: cid, n e os n pares
    // (a partir da terceira palavra), copiados aqui.
    let tabela = unsafe { copiar_tabela(t.add(2), *t.add(1) as usize) };
    METODOS.with(|m| m.borrow_mut().insert(cid, tabela));
    marcar_registrada(cid);
}

/// Até que id de classe o vetor [`REGISTRADAS`] vale: os ids do programa e
/// do SDK são densos e pequenos; os internos do runtime (a classe `Type`,
/// 0x3FFF_FF01…) ficam com o mapa.
const REGISTRADAS_ATE: usize = 1 << 16;

/// Publica [`REGISTRADAS`] no contexto da thread (a alocação em linha de
/// `dartforge_object_new_t` confere o registro por ali, `llvm/mod.rs`).
fn publicar_registradas(r: &[bool]) {
    crate::heap::CONTEXTO.with(|c| {
        c.registradas.set(r.as_ptr().cast());
        c.n_registradas.set(r.len());
    });
}

/// A classe `cid` já tem tabela registrada?
fn ja_registrada(cid: i64) -> bool {
    match usize::try_from(cid) {
        Ok(i) if i < REGISTRADAS_ATE => REGISTRADAS.with(|r| r.borrow().get(i).copied().unwrap_or(false)),
        _ => METODOS.with(|m| m.borrow().contains_key(&cid)),
    }
}

fn marcar_registrada(cid: i64) {
    if let Ok(i) = usize::try_from(cid)
        && i < REGISTRADAS_ATE
    {
        REGISTRADAS.with(|r| {
            let mut r = r.borrow_mut();
            if r.len() <= i {
                r.resize(i + 1, false);
            }
            r[i] = true;
            publicar_registradas(&r);
        });
    }
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
    // Os objetos vivos deste isolado passam ao layout da geração nova (J03).
    aplicar_migracao_pendente();
    if let Some(f) = area {
        f();
    }
    REPUBLICANDO.with(|r| r.set(true));
    // Enquanto os registros se refazem, toda alocação passa pelo runtime,
    // que registra de novo (`dartforge_object_new_t`).
    crate::heap::CONTEXTO.with(|c| c.n_registradas.set(0));
    if let Some(f) = registrar {
        f();
    }
    REPUBLICANDO.with(|r| r.set(false));
    REGISTRADAS.with(|r| publicar_registradas(&r.borrow()));
    if let Some(f) = rti {
        f();
    }
    // Nada deste isolado aponta mais para a memória das gerações anteriores.
    if area.is_some() {
        esquecer_geracoes_anteriores_das_areas();
    }
    // As tabelas de métodos mudaram: os caches dos pontos de chamada (os do
    // código novo e os do que continua de gerações anteriores, J04) buscam
    // de novo.
    esvaziar_caches_das_areas();
}

/// `dartforge_object_new` que registra a tabela de métodos da classe na
/// primeira alocação (SDK da fonte).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_object_new_t(cid: i64, campos: i64, f: extern "C" fn() -> *const i64) -> i64 {
    dartforge_registrar_tabela(cid, f);
    dartforge_object_new(cid, campos)
}

/// A conferência de ABI do módulo (docs/NATIVO-ESPACO-UNIFICADO.md §2.4,
/// §3.6): as classes do runtime têm cids fixos, e um módulo compilado com
/// outra numeração que a deste runtime não pode rodar sobre ele. `ids` é a
/// tabela do módulo: os cids de `layout::cid::DO_SDK` em ordem, ou as 32
/// posições de [`CIDS_FIXOS_POR_POSICAO`] (uma classe que o SDK carregado
/// não tem vem como -1). Diferente: o processo aborta com a mensagem.
///
/// # Safety
/// `ids` aponta para `n` palavras legíveis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_cids(ids: *const i64, n: i64) {
    // SAFETY: o emissor passa uma constante do módulo com `n` palavras.
    let v = unsafe { std::slice::from_raw_parts(ids, usize::try_from(n).unwrap_or(0)) };
    let esperado: Vec<i64> = if v.len() == crate::layout::cid::DO_SDK.len() {
        crate::layout::cid::DO_SDK.iter().map(|&(c, _, _)| i64::from(c)).collect()
    } else {
        CIDS_FIXOS_POR_POSICAO.iter().map(|&c| i64::from(c)).collect()
    };
    let confere = v.len() == esperado.len() && v.iter().zip(&esperado).all(|(&m, &r)| m == r || m == -1);
    if !confere {
        use std::io::Write;
        let _ = std::io::stdout().flush();
        let _ = writeln!(
            std::io::stderr().lock(),
            "erro: o módulo foi compilado com outra tabela de classes do runtime (ABI): módulo {v:?}, runtime {esperado:?}"
        );
        std::process::abort();
    }
}

/// A entrada uniforme do seletor `hash` na tabela da classe `cid`, se ela tem.
fn metodo_da_classe(cid: i64, hash: i64) -> Option<usize> {
    METODOS.with(|m| {
        let m = m.borrow();
        let pares = m.get(&cid)?;
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
    let n = com_raizes(&[recv], || HEAP.with(|h| h.borrow_mut().alocar_str(nome)));
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
    RTI.with(|u| match u.borrow().obter(tupla) {
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
    let textos: Vec<i64> = nomes.iter().map(|t| enraizar(HEAP.with(|h| h.borrow_mut().alocar_str(t)))).collect();
    let nomes = enraizar(dart_lista_fixa(&textos));
    let vals = enraizar(dart_lista_fixa(&valores[npos..]));
    let objetos: Vec<i64> = tipos.iter().map(|&t| enraizar(dartforge_rti_objeto_tipo(t))).collect();
    let tipos = enraizar(dart_lista_fixa(&objetos));
    // SAFETY: registrado pelo `dart:core` com a assinatura
    // (Object?, int, String, List, List, List, List) -> Object?.
    let g = { let alvo_dart: usize = f; move |a0: i64, a1: i64, a2: i64, a3: i64, a4: i64, a5: i64, a6: i64| -> i64 { dart_r7(alvo_dart, a0, a1, a2, a3, a4, a5, a6) } };
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
    // O `_Contexto` do encaminhador (`lower/sdk_fonte.rs`, `gerar_encaminhador_nsm`) guarda todos os valores
    // como `Ref` (encaixotados pelo compilador) e depois os nomes. Sem
    // argumento nenhum o contexto é `null` (`AllocEnv` vazio não aloca).
    let campos: Vec<i64> = HEAP.with(|h| {
        let h = h.borrow();
        if !crate::layout::e_objeto(ambiente) {
            return Vec::new();
        }
        let n = h.objeto(ambiente).map_or(0, |o| o.len());
        (0..n).map(|i| h.captura(ambiente, i).0).collect()
    });
    if campos.len() != npos + 2 * nnom {
        return 0;
    }
    let frame = HEAP.with(|h| h.borrow_mut().push_frame_with_slots(3));
    let enraizar = |i: usize, x: i64| HEAP.with(|h| h.borrow_mut().set_root(frame, i, x));
    enraizar(0, recv);
    enraizar(1, nome);
    enraizar(2, ambiente);
    let valores: Vec<i64> = campos[..npos + nnom].to_vec();
    let nomes: Vec<String> = HEAP.with(|h| {
        let h = h.borrow();
        campos[npos + nnom..].iter().map(|&t| h.texto(t).map(|t| t.para_string()).unwrap_or_default()).collect()
    });
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
    let mut achado = metodo_da_classe(cid, hash);
    // O seletor da chamada tipada (`tc:m`, `ts:x`; `lower/entrada_tipada.rs`)
    // só está na tabela dos métodos que têm a entrada tipada: sem ela, a
    // entrada de sempre (`c:m`, `s:x`), que também serve — confere mais.
    // SAFETY: o nome é uma constante do módulo com `len` bytes.
    let bytes = unsafe { std::slice::from_raw_parts(nome, len as usize) };
    let tipado = bytes.first() == Some(&b't');
    if achado.is_none() && tipado {
        let sem_t = String::from_utf8_lossy(&bytes[1..]);
        achado = metodo_da_classe(cid, hash_do_nome(&sem_t));
    }
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
            let texto = String::from_utf8_lossy(if tipado { &bytes[1..] } else { bytes }).into_owned();
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
    let t = HEAP.with(|heap| heap.borrow().texto(texto).map(|t| t.para_string()).unwrap_or_default());
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

/// A migração dos layouts de objeto da última recarga estrutural (J03), com a
/// época dela: por classe, a posição antiga de cada campo novo (`-1`: nenhuma).
fn migracao_pendente() -> &'static std::sync::Mutex<Option<(u64, std::sync::Arc<HashMap<i64, Vec<i64>>>)>> {
    static M: std::sync::OnceLock<std::sync::Mutex<Option<(u64, std::sync::Arc<HashMap<i64, Vec<i64>>>)>>> =
        std::sync::OnceLock::new();
    M.get_or_init(Default::default)
}

/// Define a migração da publicação em curso (J03): `dados` é
/// `[n, (classe, novo_len, origem…)…]`. Chamada pelo JIT no ponto seguro do
/// isolado principal, com os demais parados; cada isolado a aplica ao seu
/// heap em [`dartforge_publicar_geracao`].
///
/// # Safety
/// `dados` aponta para `n` palavras legíveis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_definir_migracao(dados: *const i64, n: i64) {
    // SAFETY: garantido por quem chama; copiado aqui.
    let v = unsafe { std::slice::from_raw_parts(dados, usize::try_from(n).unwrap_or(0)) };
    let mut plano: HashMap<i64, Vec<i64>> = HashMap::default();
    let mut i = 1;
    for _ in 0..v.first().copied().unwrap_or(0) {
        let (Some(&classe), Some(&len)) = (v.get(i), v.get(i + 1)) else { break };
        let len = usize::try_from(len).unwrap_or(0);
        let Some(origem) = v.get(i + 2..i + 2 + len) else { break };
        // As classes do runtime (cid < 128) têm layout fixo, igual em toda
        // geração: um plano que as mude é recusado (§2.13).
        if classe < crate::layout::PRIMEIRO_CID_LIVRE {
            use std::io::Write;
            let _ = writeln!(std::io::stderr().lock(), "erro: migração recusada para a classe do runtime {classe}");
        } else {
            plano.insert(classe, origem.to_vec());
        }
        i += 2 + len;
    }
    let epoca = crate::heap::EPOCA_DE_LAYOUT.fetch_add(1, std::sync::atomic::Ordering::AcqRel) + 1;
    *migracao_pendente().lock().unwrap_or_else(|e| e.into_inner()) = Some((epoca, std::sync::Arc::new(plano)));
}

/// Aplica ao heap deste isolado a migração pendente, se ele ainda não está na
/// época dela. Um heap criado depois da migração já nasce na época nova.
fn aplicar_migracao_pendente() {
    let atual = crate::heap::EPOCA_DE_LAYOUT.load(std::sync::atomic::Ordering::Acquire);
    let plano = migracao_pendente().lock().unwrap_or_else(|e| e.into_inner()).clone();
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        if h.epoca_de_layout >= atual {
            return;
        }
        if let Some((epoca, plano)) = plano
            && epoca == atual
        {
            h.migrar_instancias(&plano);
        }
        h.epoca_de_layout = atual;
    });
}
