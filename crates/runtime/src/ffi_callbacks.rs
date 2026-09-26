// Runtime nativo: callbacks do `dart:ffi` — `Pointer.fromFunction`,
// `NativeCallable.isolateLocal` e `NativeCallable.listener` (o
// `runtime/vm/ffi_callback_metadata.cc` e o `ffi_callback_trampolines` da
// VM).
//
// * O compilador gera, por assinatura nativa do programa (a mesma chave dos
//   trampolins de chamada, `lower/ffi.rs`), uma ENTRADA C: uma função com a
//   ABI C da assinatura e um parâmetro `nest` — o contexto do callback. A
//   entrada pergunta ao runtime o modo (`dartforge_ffi_callback_entrar`),
//   converte os argumentos C para Dart, chama a closure pela convenção
//   uniforme (por um corpo HIR comum), converte o retorno e, se a closure
//   lançou, devolve o retorno excepcional (`dartforge_ffi_callback_sair`).
// * Cada callback é um ponteiro de função C distinto: um trampolim de poucas
//   instruções que carrega o contexto no registrador `nest` da entrada (r10
//   no x86-64, x15 no AArch64 — o mesmo código que `llvm.init.trampoline`
//   escreveria) e salta para ela, escrito por este arquivo numa fatia de
//   memória executável. Escrever aqui, e não pelo intrínseco no módulo
//   gerado, mantém a pilha não executável em todo objeto (com o intrínseco
//   o LLVM omite a nota `.note.GNU-stack`, e as partições do LTO não a
//   recuperam).
// * Modos: `isolateLocal` e `fromFunction` chamam a closure na hora, e só
//   na thread do isolado dono (outra thread é erro fatal, como na VM);
//   `fromFunction` é persistente e único por (função, assinatura, retorno
//   excepcional). `listener` pode ser chamado de qualquer thread: a entrada
//   copia os argumentos para uma mensagem à porta do `NativeCallable`, e o
//   isolado dono chama a closure no laço de eventos.
// * A closure de um callback local é raiz do coletor (um global de raiz do
//   heap do isolado, pelo endereço do contexto) até o `close()`. Fechar
//   dentro do próprio callback é permitido: o contexto só é solto quando a
//   última chamada em curso sai.

/// Modos de um callback (o `modo` de `DartForge_ffi_callback_novo`).
const MODO_PERSISTENTE: i64 = 0;
const MODO_LOCAL: i64 = 1;
const MODO_OUVINTE: i64 = 2;

/// O estado de um callback; o endereço é o valor `nest` do trampolim.
pub struct ContextoCallback {
    modo: i64,
    /// O isolado dono (id da fila).
    isolado: u64,
    /// A closure (modos local e persistente), raiz do heap do dono.
    closure: i64,
    /// A porta do `NativeCallable.listener`.
    porta: i64,
    /// A assinatura nativa (RTI), para os `Pointer<X>` dos argumentos.
    assinatura: i64,
    /// A chave da assinatura (`i_pp`): letras do retorno e dos parâmetros.
    chave: String,
    /// O retorno excepcional, nos bits do tipo C do retorno.
    excepcional: i64,
    /// Chamadas em curso (a closure pode fechar o callback de dentro).
    ativas: u32,
    fechado: bool,
    /// O ponteiro de função C (o trampolim).
    trampolim: usize,
}

/// Chave da assinatura → a entrada C do callback.
fn entradas_de_callback() -> &'static std::sync::RwLock<std::collections::HashMap<String, usize>> {
    static T: std::sync::OnceLock<std::sync::RwLock<std::collections::HashMap<String, usize>>> = std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

/// Ponteiro de função → contexto, dos callbacks vivos.
fn callbacks_vivos() -> &'static std::sync::Mutex<std::collections::HashMap<usize, usize>> {
    static T: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<usize, usize>>> = std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

thread_local! {
    /// Os `fromFunction` deste isolado: (chave, sítio, código da função,
    /// excepcional) → ponteiro de função.
    static PERSISTENTES: RefCell<std::collections::HashMap<(String, i64, i64, i64), usize>> = RefCell::new(std::collections::HashMap::new());
}

/// # Safety
/// `chave` aponta para `n` bytes ASCII; `entrada` é a entrada C gerada da
/// assinatura (o contexto no parâmetro `nest`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_registrar_callback(chave: *const u8, n: i64, entrada: usize) {
    // SAFETY: garantido por quem chama (constante do módulo).
    let b = unsafe { std::slice::from_raw_parts(chave, n as usize) };
    let chave = String::from_utf8_lossy(b).into_owned();
    entradas_de_callback().write().unwrap_or_else(|e| e.into_inner()).insert(chave, entrada);
}

/// A memória executável dos trampolins: regiões mapeadas uma vez e nunca
/// devolvidas, divididas em fatias de [`TAM_FATIA`] bytes com lista livre.
mod memoria_executavel {
    /// Cabe o maior trampolim do LLVM (x86-64: 23 bytes; AArch64: 32).
    pub const TAM_FATIA: usize = 64;
    const TAM_REGIAO: usize = 64 * 1024;

    static LIVRES: std::sync::Mutex<Vec<usize>> = std::sync::Mutex::new(Vec::new());

    #[cfg(unix)]
    unsafe extern "C" {
        fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, off: i64) -> *mut u8;
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    unsafe extern "C" {
        fn pthread_jit_write_protect_np(enabled: i32);
        fn sys_icache_invalidate(start: *mut u8, len: usize);
    }
    #[cfg(windows)]
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn VirtualAlloc(addr: *mut u8, len: usize, tipo: u32, protecao: u32) -> *mut u8;
        fn FlushInstructionCache(processo: usize, addr: *const u8, len: usize) -> i32;
        fn GetCurrentProcess() -> usize;
    }

    #[cfg(unix)]
    fn mapear() -> Option<usize> {
        const PROT_RWX: i32 = 1 | 2 | 4;
        const MAP_PRIVATE: i32 = 2;
        #[cfg(target_os = "linux")]
        const MAP_ANON: i32 = 0x20;
        #[cfg(not(target_os = "linux"))]
        const MAP_ANON: i32 = 0x1000;
        // No macOS a memória que vira código precisa de `MAP_JIT` (e a
        // escrita, no Apple Silicon, de `pthread_jit_write_protect_np`).
        #[cfg(target_os = "macos")]
        const MAP_JIT: i32 = 0x800;
        #[cfg(not(target_os = "macos"))]
        const MAP_JIT: i32 = 0;
        // SAFETY: mapeamento anônimo novo, sem arquivo.
        let p = unsafe { mmap(std::ptr::null_mut(), TAM_REGIAO, PROT_RWX, MAP_PRIVATE | MAP_ANON | MAP_JIT, -1, 0) };
        if p.is_null() || p as isize == -1 { None } else { Some(p as usize) }
    }

    #[cfg(windows)]
    fn mapear() -> Option<usize> {
        const MEM_COMMIT_RESERVE: u32 = 0x1000 | 0x2000;
        const PAGE_EXECUTE_READWRITE: u32 = 0x40;
        // SAFETY: reserva e compromete páginas novas.
        let p = unsafe { VirtualAlloc(std::ptr::null_mut(), TAM_REGIAO, MEM_COMMIT_RESERVE, PAGE_EXECUTE_READWRITE) };
        if p.is_null() { None } else { Some(p as usize) }
    }

    pub fn alocar() -> Option<usize> {
        let mut livres = LIVRES.lock().unwrap_or_else(|e| e.into_inner());
        if livres.is_empty() {
            let base = mapear()?;
            livres.extend((0..TAM_REGIAO / TAM_FATIA).rev().map(|i| base + i * TAM_FATIA));
        }
        livres.pop()
    }

    pub fn liberar(p: usize) {
        LIVRES.lock().unwrap_or_else(|e| e.into_inner()).push(p);
    }

    /// Escreve em `p` o trampolim que carrega `ctx` no registrador `nest` e
    /// salta para `entrada`; devolve o ponteiro de função (o próprio `p`).
    pub fn escrever_trampolim(p: usize, entrada: usize, ctx: usize) -> usize {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        // SAFETY: alterna a proteção MAP_JIT só nesta thread.
        unsafe {
            pthread_jit_write_protect_np(0)
        };
        // SAFETY: `p` é uma fatia de `TAM_FATIA` bytes graváveis desta
        // região, de uso exclusivo deste callback.
        unsafe { gravar_codigo(p as *mut u8, entrada, ctx) };
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        // SAFETY: idem.
        unsafe {
            pthread_jit_write_protect_np(1)
        };
        sincronizar_instrucoes(p);
        p
    }

    /// x86-64: `movabs r11, entrada; movabs r10, ctx; jmp r11`.
    #[cfg(target_arch = "x86_64")]
    unsafe fn gravar_codigo(p: *mut u8, entrada: usize, ctx: usize) {
        let mut c = Vec::with_capacity(23);
        c.extend_from_slice(&[0x49, 0xBB]);
        c.extend_from_slice(&(entrada as u64).to_le_bytes());
        c.extend_from_slice(&[0x49, 0xBA]);
        c.extend_from_slice(&(ctx as u64).to_le_bytes());
        c.extend_from_slice(&[0x41, 0xFF, 0xE3]);
        // SAFETY: garantido por quem chama (23 bytes de 64).
        unsafe { std::ptr::copy_nonoverlapping(c.as_ptr(), p, c.len()) };
    }

    /// AArch64: `ldr x15, [pc+16]; ldr x17, [pc+20]; br x17; nop`, com o
    /// contexto em +16 e a entrada em +24.
    #[cfg(target_arch = "aarch64")]
    unsafe fn gravar_codigo(p: *mut u8, entrada: usize, ctx: usize) {
        let instrucoes: [u32; 4] = [0x5800_008F, 0x5800_00B1, 0xD61F_0220, 0xD503_201F];
        let mut c = Vec::with_capacity(32);
        for i in instrucoes {
            c.extend_from_slice(&i.to_le_bytes());
        }
        c.extend_from_slice(&(ctx as u64).to_le_bytes());
        c.extend_from_slice(&(entrada as u64).to_le_bytes());
        // SAFETY: garantido por quem chama (32 bytes de 64).
        unsafe { std::ptr::copy_nonoverlapping(c.as_ptr(), p, c.len()) };
    }

    /// O cache de instruções vê o código recém escrito.
    fn sincronizar_instrucoes(p: usize) {
        #[cfg(windows)]
        // SAFETY: a fatia acabou de ser escrita neste processo.
        unsafe {
            FlushInstructionCache(GetCurrentProcess(), p as *const u8, TAM_FATIA)
        };
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        // SAFETY: invalida o cache de instruções da fatia escrita.
        unsafe {
            sys_icache_invalidate(p as *mut u8, TAM_FATIA)
        };
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        // SAFETY: manutenção de cache de EL0 (`dc cvau`/`ic ivau`), como o
        // `__clear_cache` do compiler-rt, linha a linha da fatia.
        unsafe {
            const LINHA: usize = 64;
            let ini = p & !(LINHA - 1);
            let mut a = ini;
            while a < p + TAM_FATIA {
                std::arch::asm!("dc cvau, {0}", in(reg) a);
                a += LINHA;
            }
            std::arch::asm!("dsb ish");
            a = ini;
            while a < p + TAM_FATIA {
                std::arch::asm!("ic ivau, {0}", in(reg) a);
                a += LINHA;
            }
            std::arch::asm!("dsb ish", "isb");
        };
        let _ = p;
    }
}

/// Uma posição da chave de uma assinatura (`lower/ffi.rs`): a letra de um
/// tipo C ou `S<rti>.`, uma struct/union por valor.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ParteDaChave {
    Letra(char),
    Composto(i64),
}

/// As partes da chave: o retorno e os parâmetros.
fn partes_da_chave(chave: &str) -> (ParteDaChave, Vec<ParteDaChave>) {
    let mut partes = Vec::new();
    let mut resto = chave;
    let mut ret = None;
    while let Some(c) = resto.chars().next() {
        if c == '_' && ret.is_none() {
            ret = partes.pop();
            resto = &resto[1..];
            continue;
        }
        if c == 'S' {
            let fim = resto.find('.').unwrap_or(resto.len());
            partes.push(ParteDaChave::Composto(resto[1..fim].parse().unwrap_or(0)));
            resto = &resto[(fim + 1).min(resto.len())..];
        } else {
            partes.push(ParteDaChave::Letra(c));
            resto = &resto[c.len_utf8()..];
        }
    }
    (ret.unwrap_or(ParteDaChave::Letra('v')), partes)
}

/// O valor C de `v` (um objeto Dart) no tipo da letra `l`, para o retorno
/// excepcional.
fn bits_do_excepcional(v: i64, l: char) -> Result<i64, String> {
    if v == 0 {
        return Ok(0);
    }
    if l == 'p' {
        return endereco_de(v).ok_or_else(|| "exceptionalReturn must be a Pointer".to_string());
    }
    if l == 'H' {
        return Err("exceptionalReturn must not be given for a Handle return".to_string());
    }
    HEAP.with(|h| {
        let h = h.borrow();
        let inteiro = h.int_de_ref(v);
        match (l, h.try_get(v)) {
            ('f' | 'd', Some(Value::BoxedDouble(d))) => Ok(d.to_bits() as i64),
            ('f' | 'd', _) => inteiro.map(|i| (i as f64).to_bits() as i64).ok_or_else(|| "exceptionalReturn must be a double".to_string()),
            ('b', Some(Value::BoxedBool(b))) => Ok(i64::from(*b)),
            _ => inteiro.ok_or_else(|| "exceptionalReturn must be an int".to_string()),
        }
    })
}

/// O código de uma closure (a identidade de uma função de topo ou
/// estática, para o cache de `fromFunction`).
fn codigo_da_closure(clo: i64) -> i64 {
    HEAP.with(|h| h.borrow().closure_parts(clo).0)
}

/// Cria o trampolim de `ctx` e o registra.
fn instalar_callback(ctx: Box<ContextoCallback>) -> Result<usize, String> {
    let registro = entradas_de_callback().read().unwrap_or_else(|e| e.into_inner()).get(&ctx.chave).copied();
    let Some(entrada) = registro else {
        let texto = RTI.with(|u| u.borrow().texto(ctx.assinatura));
        return Err(format!("the native signature `{texto}` was not compiled into this program (it must be written as a constant type argument)"));
    };
    let Some(memoria) = memoria_executavel::alocar() else {
        return Err("could not allocate executable memory for a native callback".to_string());
    };
    let ctx = Box::into_raw(ctx);
    let f = memoria_executavel::escrever_trampolim(memoria, entrada, ctx as usize);
    // SAFETY: o contexto acabou de ser criado e só esta thread o conhece.
    unsafe { (*ctx).trampolim = memoria };
    callbacks_vivos().lock().unwrap_or_else(|e| e.into_inner()).insert(f, ctx as usize);
    Ok(f)
}

/// `DartForge_ffi_callback_novo(assinatura, funcao, excepcional, modo,
/// porta)`: o ponteiro de função C de um callback novo (`porta` é a do
/// ouvinte, ou o sítio de um `fromFunction`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_callback_novo(assinatura: i64, funcao: i64, excepcional: i64, modo: i64, porta: i64) -> i64 {
    let Some(ns) = tipo_do_objeto_type(assinatura) else {
        lancar_erro_de_argumento("native callback: signature type argument expected");
        return 0;
    };
    let chave = match chave_da_assinatura(ns) {
        Ok(c) => c,
        Err(m) => {
            lancar_unsupported(&m);
            return 0;
        }
    };
    let (retorno, params) = partes_da_chave(&chave);
    if modo == MODO_OUVINTE && retorno != ParteDaChave::Letra('v') {
        lancar_erro_de_argumento("NativeCallable.listener callbacks must return void");
        return 0;
    }
    if modo == MODO_OUVINTE && params.contains(&ParteDaChave::Letra('H')) {
        lancar_erro_de_argumento("NativeCallable.listener callbacks cannot take a Handle");
        return 0;
    }
    let excepcional = match retorno {
        // Struct devolvida: sem retorno excepcional (a VM o recusa); com
        // exceção, a struct volta zerada.
        ParteDaChave::Composto(_) if excepcional != 0 => {
            lancar_erro_de_argumento("exceptionalReturn must not be given for a struct or union return");
            return 0;
        }
        ParteDaChave::Composto(_) => 0,
        ParteDaChave::Letra(l) => match bits_do_excepcional(excepcional, l) {
            Ok(b) => b,
            Err(m) => {
                lancar_erro_de_argumento(&m);
                return 0;
            }
        },
    };
    // `fromFunction`: `porta` é o sítio da chamada (um trampolim por sítio).
    let persistente = (modo == MODO_PERSISTENTE).then(|| (chave.clone(), porta, codigo_da_closure(funcao), excepcional));
    if let Some(k) = &persistente
        && let Some(f) = PERSISTENTES.with(|p| p.borrow().get(k).copied())
    {
        return f as i64;
    }
    let ctx = Box::new(ContextoCallback {
        modo,
        isolado: id_do_isolado(),
        closure: if modo == MODO_OUVINTE { 0 } else { funcao },
        porta: if modo == MODO_OUVINTE { porta } else { 0 },
        assinatura: ns,
        chave,
        excepcional,
        ativas: 0,
        fechado: false,
        trampolim: 0,
    });
    let raiz = &*ctx as *const ContextoCallback as i64;
    match instalar_callback(ctx) {
        Ok(f) => {
            if modo != MODO_OUVINTE {
                HEAP.with(|h| h.borrow_mut().set_global_root(raiz, funcao));
            }
            if let Some(k) = persistente {
                PERSISTENTES.with(|p| p.borrow_mut().insert(k, f));
            }
            f as i64
        }
        Err(m) => {
            lancar_unsupported(&m);
            0
        }
    }
}

/// Solta o contexto e o trampolim (sem chamada em curso).
fn soltar_callback(ctx: *mut ContextoCallback) {
    // SAFETY: o contexto saiu do registro; ninguém mais o alcança.
    let ctx = unsafe { Box::from_raw(ctx) };
    memoria_executavel::liberar(ctx.trampolim);
}

/// `DartForge_ffi_callback_apagar(endereco)`: o `close()` de um
/// `NativeCallable`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_callback_apagar(endereco: i64) {
    let Some(ctx) = callbacks_vivos().lock().unwrap_or_else(|e| e.into_inner()).remove(&(endereco as usize)) else {
        return;
    };
    let ctx = ctx as *mut ContextoCallback;
    HEAP.with(|h| h.borrow_mut().soltar_raiz_global(ctx as i64));
    // SAFETY: o contexto é deste isolado (o `close()` roda nele).
    let emuso = unsafe {
        (*ctx).fechado = true;
        (*ctx).closure = 0;
        (*ctx).ativas > 0
    };
    if !emuso {
        soltar_callback(ctx);
    }
}

/// `DartForge_ffi_callback_manter(delta)`: o contador de `NativeCallable`s
/// locais que mantêm o isolado vivo (`keepIsolateAlive`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_callback_manter(delta: i64) {
    ajustar_callbacks_que_mantem_vivo(delta);
}

/// A entrada C começou: 0 = chamar a closure aqui; 1 = postar ao ouvinte.
/// Um callback local chamado fora da thread do isolado dono é fatal.
///
/// # Safety
/// `ctx` é o contexto do trampolim (vivo enquanto o callback não fecha).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_callback_entrar(ctx: *mut ContextoCallback) -> i64 {
    // SAFETY: garantido por quem chama.
    let c = unsafe { &mut *ctx };
    if c.modo == MODO_OUVINTE {
        return 1;
    }
    if c.fechado || id_do_isolado() != c.isolado {
        eprintln!("Cannot invoke native callback outside an isolate.");
        std::process::abort();
    }
    c.ativas += 1;
    0
}

/// A closure do callback (a entrada chama por ela).
///
/// # Safety
/// Como [`dartforge_ffi_callback_entrar`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_callback_closure(ctx: i64) -> i64 {
    // SAFETY: garantido por quem chama.
    unsafe { (*(ctx as *const ContextoCallback)).closure }
}

/// O `Pointer<X>` Dart do argumento `i` (endereço `endereco`).
///
/// # Safety
/// Como [`dartforge_ffi_callback_entrar`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_callback_ponteiro(ctx: i64, i: i64, endereco: i64) -> i64 {
    // SAFETY: garantido por quem chama.
    let ns = unsafe { (*(ctx as *const ContextoCallback)).assinatura };
    let tipo = RTI.with(|u| match u.borrow().tipo(ns) {
        Tipo::Funcao { pos, .. } => pos.get(i as usize).copied(),
        _ => None,
    });
    novo_ponteiro(endereco, tipo)
}

/// A entrada C terminou a chamada: devolve 1 (e o retorno excepcional em
/// `saida`) se a closure lançou — a exceção é descartada, como na VM.
/// Solta o contexto se ele foi fechado durante a chamada.
///
/// # Safety
/// Como [`dartforge_ffi_callback_entrar`]; `saida` é gravável.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_callback_sair(ctx: *mut ContextoCallback, saida: *mut i64) -> u8 {
    // SAFETY: garantido por quem chama.
    let c = unsafe { &mut *ctx };
    c.ativas -= 1;
    let lancou = dartforge_exception_pending() != 0;
    if lancou {
        dartforge_exception_clear();
        // SAFETY: garantido por quem chama.
        unsafe { *saida = c.excepcional };
    }
    if c.fechado && c.ativas == 0 {
        soltar_callback(ctx);
    }
    u8::from(lancou)
}

/// A entrada C de um `listener`: os `n` argumentos (bits de cada tipo C)
/// viram a mensagem `[contexto, a0, a1, …]` à porta do `NativeCallable`.
///
/// # Safety
/// Como [`dartforge_ffi_callback_entrar`]; `args` tem `n` valores.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_callback_postar(ctx: *const ContextoCallback, args: *const i64, n: i64) {
    // SAFETY: garantido por quem chama.
    let c = unsafe { &*ctx };
    // SAFETY: idem.
    let bits = unsafe { std::slice::from_raw_parts(args, n.max(0) as usize) };
    let mut itens = Vec::with_capacity(bits.len() + 1);
    itens.push(Portavel::Int(ctx as i64));
    let (_, params) = partes_da_chave(&c.chave);
    for (b, p) in bits.iter().zip(params) {
        itens.push(match p {
            ParteDaChave::Letra('f' | 'd') => Portavel::Double(f64::from_bits(*b as u64)),
            ParteDaChave::Letra('b') => Portavel::Bool(*b != 0),
            // Uma struct: os bytes (o endereço só vale durante a chamada).
            ParteDaChave::Composto(rti) => {
                let n = composto_de_rti(rti).map_or(0, |c| c.tamanho as usize);
                // SAFETY: a entrada C passa o endereço dos `n` bytes do
                // argumento, vivos durante a chamada.
                Portavel::Bytes(unsafe { std::slice::from_raw_parts(*b as usize as *const u8, n) }.to_vec())
            }
            ParteDaChave::Letra(_) => Portavel::Int(*b),
        });
    }
    postar(c.porta, Portavel::Lista(itens).para_grafo());
}

/// `DartForge_ffi_callback_args(mensagem)`: os argumentos Dart de uma
/// mensagem de `listener` (os ponteiros viram `Pointer<X>` da assinatura).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_callback_args(mensagem: i64) -> i64 {
    let itens: Vec<TaggedValue> = HEAP.with(|h| match h.borrow().try_get(mensagem) {
        Some(Value::List(v)) => v.clone(),
        _ => Vec::new(),
    });
    let inteiro = |v: &TaggedValue| if v.is_ref { HEAP.with(|h| h.borrow().int_de_ref(v.bits)).unwrap_or(0) } else { v.bits };
    let Some(ctx) = itens.first().map(inteiro) else { return mensagem };
    let vivo = callbacks_vivos().lock().unwrap_or_else(|e| e.into_inner()).values().any(|&c| c as i64 == ctx);
    if !vivo {
        return dartforge_list_new_empty();
    }
    // SAFETY: o contexto está registrado (vivo).
    let (chave, ns) = unsafe {
        let c = &*(ctx as *const ContextoCallback);
        (c.chave.clone(), c.assinatura)
    };
    let tipos: Vec<i64> = RTI.with(|u| match u.borrow().tipo(ns) {
        Tipo::Funcao { pos, .. } => pos.to_vec(),
        _ => Vec::new(),
    });
    let mut saida = Vec::with_capacity(itens.len().saturating_sub(1));
    let frame = HEAP.with(|h| h.borrow_mut().push_frame_with_slots(itens.len().max(1)));
    let (_, params) = partes_da_chave(&chave);
    for (i, (v, p)) in itens.iter().skip(1).zip(params).enumerate() {
        let x = match p {
            ParteDaChave::Letra('p') => {
                let p = novo_ponteiro(inteiro(v), tipos.get(i).copied());
                HEAP.with(|h| h.borrow_mut().set_root(frame, i, p));
                TaggedValue::reference(p)
            }
            // Os bytes da struct viram uma struct sobre memória Dart.
            ParteDaChave::Composto(rti) => {
                let bytes = HEAP.with(|h| match h.borrow().try_get(v.bits) {
                    Some(Value::TypedData { bytes, .. }) => bytes.to_vec(),
                    _ => Vec::new(),
                });
                let s = dartforge_ffi_composto_copia(rti, bytes.as_ptr() as i64);
                HEAP.with(|h| h.borrow_mut().set_root(frame, i, s));
                TaggedValue::reference(s)
            }
            ParteDaChave::Letra(_) => *v,
        };
        saida.push(x);
    }
    let lista = HEAP.with(|h| h.borrow_mut().create_list(saida));
    HEAP.with(|h| h.borrow_mut().pop_frame(frame));
    lista
}
