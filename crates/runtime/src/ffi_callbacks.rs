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

/// Modos de um callback (o `mode` de `DartForge_ffi_callback_novo`).
const MODE_PERSISTENT: i64 = 0;
const MODE_LOCAL: i64 = 1;
const MODE_LISTENER: i64 = 2;

/// O estado de um callback; o endereço é o valor `nest` do trampolim.
pub struct CallbackContext {
    mode: i64,
    /// O isolado dono (id da fila).
    isolate: u64,
    /// A closure (modos local e persistente), raiz do heap do dono.
    closure: i64,
    /// A porta do `NativeCallable.listener`.
    port: i64,
    /// A assinatura nativa (RTI), para os `Pointer<X>` dos argumentos.
    fn_signature: i64,
    /// A chave da assinatura (`i_pp`): letras do retorno e dos parâmetros.
    key: String,
    /// O retorno excepcional, nos bits do tipo C do retorno.
    exceptional: i64,
    /// Chamadas em curso (a closure pode fechar o callback de dentro).
    active_list: u32,
    closed_flag: bool,
    /// O ponteiro de função C (o trampolim).
    trampoline: usize,
}

/// Chave da assinatura → a entrada C do callback.
fn callback_entries() -> &'static std::sync::RwLock<crate::hash::HashMap<String, usize>> {
    static T: std::sync::OnceLock<std::sync::RwLock<crate::hash::HashMap<String, usize>>> = std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

/// Ponteiro de função → contexto, dos callbacks vivos.
fn live_callbacks() -> &'static std::sync::Mutex<crate::hash::HashMap<usize, usize>> {
    static T: std::sync::OnceLock<std::sync::Mutex<crate::hash::HashMap<usize, usize>>> = std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

/// Reconhece trampolins síncronos que exigem a thread do isolado dono.
/// Ouvintes só postam mensagens e não entram em Dart nesta thread.
fn callback_requires_isolate_thread(address: usize) -> bool {
    let live_set = live_callbacks().lock().unwrap_or_else(|e| e.into_inner());
    let Some(&ctx) = live_set.get(&address) else { return false };
    // SAFETY: o registro mantém o contexto vivo; close precisa deste mutex
    // para removê-lo antes de liberar. O modo é imutável após a instalação.
    let mode = unsafe { (*(ctx as *const CallbackContext)).mode };
    matches!(mode, MODE_PERSISTENT | MODE_LOCAL)
}

thread_local! {
    /// Os `fromFunction` deste isolado: (chave, sítio, código da função,
    /// excepcional) → ponteiro de função.
    static PERSISTENTES: RefCell<crate::hash::HashMap<(String, i64, i64, i64), usize>> = RefCell::new(crate::hash::HashMap::default());
}

/// # Safety
/// `key` aponta para `n` bytes ASCII; `input` é a entrada C gerada da
/// assinatura (o contexto no parâmetro `nest`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_registrar_callback(key: *const u8, n: i64, input: usize) {
    // SAFETY: garantido por quem chama (constante do módulo).
    let b = unsafe { std::slice::from_raw_parts(key, n as usize) };
    let key = String::from_utf8_lossy(b).into_owned();
    callback_entries().write().unwrap_or_else(|e| e.into_inner()).insert(key, input);
}

/// A memória executável dos trampolins: regiões mapeadas uma vez e nunca
/// devolvidas, divididas em fatias de [`SLICE_SIZE`] bytes com lista livre.
mod executable_memory {
    /// Cabe o maior trampolim do LLVM (x86-64: 23 bytes; AArch64: 32).
    pub const SLICE_SIZE: usize = 64;
    const REGION_SIZE: usize = 64 * 1024;

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
        fn VirtualAlloc(addr: *mut u8, len: usize, tipo: u32, protection: u32) -> *mut u8;
        fn FlushInstructionCache(process: usize, addr: *const u8, len: usize) -> i32;
        fn GetCurrentProcess() -> usize;
    }

    #[cfg(unix)]
    fn map_pages() -> Option<usize> {
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
        let p = unsafe { mmap(std::ptr::null_mut(), REGION_SIZE, PROT_RWX, MAP_PRIVATE | MAP_ANON | MAP_JIT, -1, 0) };
        if p.is_null() || p as isize == -1 { None } else { Some(p as usize) }
    }

    #[cfg(windows)]
    fn map_pages() -> Option<usize> {
        const MEM_COMMIT_RESERVE: u32 = 0x1000 | 0x2000;
        const PAGE_EXECUTE_READWRITE: u32 = 0x40;
        // SAFETY: reserva e compromete páginas novas.
        let p = unsafe { VirtualAlloc(std::ptr::null_mut(), REGION_SIZE, MEM_COMMIT_RESERVE, PAGE_EXECUTE_READWRITE) };
        if p.is_null() { None } else { Some(p as usize) }
    }

    pub fn allocate() -> Option<usize> {
        let mut free_list = LIVRES.lock().unwrap_or_else(|e| e.into_inner());
        if free_list.is_empty() {
            let base = map_pages()?;
            free_list.extend((0..REGION_SIZE / SLICE_SIZE).rev().map(|i| base + i * SLICE_SIZE));
        }
        free_list.pop()
    }

    pub fn free(p: usize) {
        LIVRES.lock().unwrap_or_else(|e| e.into_inner()).push(p);
    }

    /// Escreve em `p` o trampolim que carrega `ctx` no registrador `nest` e
    /// salta para `input`; devolve o ponteiro de função (o próprio `p`).
    pub fn write_trampoline(p: usize, input: usize, ctx: usize) -> usize {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        // SAFETY: alterna a proteção MAP_JIT só nesta thread.
        unsafe {
            pthread_jit_write_protect_np(0)
        };
        // SAFETY: `p` é uma fatia de `SLICE_SIZE` bytes graváveis desta
        // região, de uso exclusivo deste callback.
        unsafe { write_code(p as *mut u8, input, ctx) };
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        // SAFETY: idem.
        unsafe {
            pthread_jit_write_protect_np(1)
        };
        sync_instructions(p);
        p
    }

    /// x86-64: `movabs r11, input; movabs r10, ctx; jmp r11`.
    #[cfg(target_arch = "x86_64")]
    unsafe fn write_code(p: *mut u8, input: usize, ctx: usize) {
        let mut c = Vec::with_capacity(23);
        c.extend_from_slice(&[0x49, 0xBB]);
        c.extend_from_slice(&(input as u64).to_le_bytes());
        c.extend_from_slice(&[0x49, 0xBA]);
        c.extend_from_slice(&(ctx as u64).to_le_bytes());
        c.extend_from_slice(&[0x41, 0xFF, 0xE3]);
        // SAFETY: garantido por quem chama (23 bytes de 64).
        unsafe { std::ptr::copy_nonoverlapping(c.as_ptr(), p, c.len()) };
    }

    /// AArch64: `ldr x15, [pc+16]; ldr x17, [pc+20]; br x17; nop`, com o
    /// contexto em +16 e a entrada em +24.
    #[cfg(target_arch = "aarch64")]
    unsafe fn write_code(p: *mut u8, input: usize, ctx: usize) {
        let instructions: [u32; 4] = [0x5800_008F, 0x5800_00B1, 0xD61F_0220, 0xD503_201F];
        let mut c = Vec::with_capacity(32);
        for i in instructions {
            c.extend_from_slice(&i.to_le_bytes());
        }
        c.extend_from_slice(&(ctx as u64).to_le_bytes());
        c.extend_from_slice(&(input as u64).to_le_bytes());
        // SAFETY: garantido por quem chama (32 bytes de 64).
        unsafe { std::ptr::copy_nonoverlapping(c.as_ptr(), p, c.len()) };
    }

    /// O cache de instruções vê o código recém escrito.
    fn sync_instructions(p: usize) {
        #[cfg(windows)]
        // SAFETY: a fatia acabou de ser escrita neste processo.
        unsafe {
            FlushInstructionCache(GetCurrentProcess(), p as *const u8, SLICE_SIZE)
        };
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        // SAFETY: invalida o cache de instruções da fatia escrita.
        unsafe {
            sys_icache_invalidate(p as *mut u8, SLICE_SIZE)
        };
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        // SAFETY: manutenção de cache de EL0 (`dc cvau`/`ic ivau`), como o
        // `__clear_cache` do compiler-rt, linha a linha da fatia.
        unsafe {
            const LINHA: usize = 64;
            let begin = p & !(LINHA - 1);
            let mut a = begin;
            while a < p + SLICE_SIZE {
                std::arch::asm!("dc cvau, {0}", in(reg) a);
                a += LINHA;
            }
            std::arch::asm!("dsb ish");
            a = begin;
            while a < p + SLICE_SIZE {
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
enum KeyPart {
    Letter(char),
    Compound(i64),
}

/// As partes da chave: o retorno e os parâmetros.
fn key_parts(key: &str) -> (KeyPart, Vec<KeyPart>) {
    let mut part_list = Vec::new();
    let mut rest = key;
    let mut ret = None;
    while let Some(c) = rest.chars().next() {
        if c == '_' && ret.is_none() {
            ret = part_list.pop();
            rest = &rest[1..];
            continue;
        }
        if c == 'S' {
            let end = rest.find('.').unwrap_or(rest.len());
            part_list.push(KeyPart::Compound(rest[1..end].parse().unwrap_or(0)));
            rest = &rest[(end + 1).min(rest.len())..];
        } else {
            part_list.push(KeyPart::Letter(c));
            rest = &rest[c.len_utf8()..];
        }
    }
    (ret.unwrap_or(KeyPart::Letter('v')), part_list)
}

/// O valor C de `v` (um objeto Dart) no tipo da letra `l`, para o retorno
/// excepcional.
fn exceptional_bits(v: i64, l: char) -> Result<i64, String> {
    if v == 0 {
        return Ok(0);
    }
    if l == 'p' {
        return address_of(v).ok_or_else(|| "exceptionalReturn must be a Pointer".to_string());
    }
    if l == 'H' {
        return Err("exceptionalReturn must not be given for a Handle return".to_string());
    }
    HEAP.with(|h| {
        let h = h.borrow();
        let integer = h.int_of(v);
        match (l, h.double_of(v), h.bool_of(v)) {
            ('f' | 'd', Some(d), _) => Ok(d.to_bits() as i64),
            ('f' | 'd', None, _) => integer.map(|i| (i as f64).to_bits() as i64).ok_or_else(|| "exceptionalReturn must be a double".to_string()),
            ('b', _, Some(b)) => Ok(i64::from(b)),
            _ => integer.ok_or_else(|| "exceptionalReturn must be an int".to_string()),
        }
    })
}

/// O código de uma closure (a identidade de uma função de topo ou
/// estática, para o cache de `fromFunction`).
fn closure_code(clo: i64) -> i64 {
    HEAP.with(|h| h.borrow().closure(clo).map_or(0, |c| c.code))
}

/// Cria o trampolim de `ctx` e o registra.
fn install_callback(ctx: Box<CallbackContext>) -> Result<usize, String> {
    let rec = callback_entries().read().unwrap_or_else(|e| e.into_inner()).get(&ctx.key).copied();
    let Some(input) = rec else {
        let text = RTI.with(|u| u.borrow().text(ctx.fn_signature));
        return Err(format!("the native signature `{text}` was not compiled into this program (it must be written as a constant type argument)"));
    };
    let Some(memory) = executable_memory::allocate() else {
        return Err("could not allocate executable memory for a native callback".to_string());
    };
    let ctx = Box::into_raw(ctx);
    let f = executable_memory::write_trampoline(memory, input, ctx as usize);
    // SAFETY: o contexto acabou de ser criado e só esta thread o conhece.
    unsafe { (*ctx).trampoline = memory };
    live_callbacks().lock().unwrap_or_else(|e| e.into_inner()).insert(f, ctx as usize);
    Ok(f)
}

/// `DartForge_ffi_callback_novo(assinatura, funcao, excepcional, modo,
/// porta)`: o pointer de função C de one callback new (`porta` é a do
/// ouvinte, ou o sítio de um `fromFunction`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_callback_novo(fn_signature: i64, function: i64, exceptional: i64, mode: i64, port: i64) -> i64 {
    let Some(ns) = object_type_type(fn_signature) else {
        throw_argument_error("native callback: signature type argument expected");
        return 0;
    };
    let key = match signature_key(ns) {
        Ok(c) => c,
        Err(m) => {
            throw_unsupported(&m);
            return 0;
        }
    };
    let (ret, params) = key_parts(&key);
    if mode == MODE_LISTENER && ret != KeyPart::Letter('v') {
        throw_argument_error("NativeCallable.listener callbacks must return void");
        return 0;
    }
    if mode == MODE_LISTENER && params.contains(&KeyPart::Letter('H')) {
        throw_argument_error("NativeCallable.listener callbacks cannot take a Handle");
        return 0;
    }
    let exceptional = match ret {
        // Struct devolvida: sem retorno excepcional (a VM o recusa); com
        // exceção, a struct volta zerada.
        KeyPart::Compound(_) if exceptional != 0 => {
            throw_argument_error("exceptionalReturn must not be given for a struct or union return");
            return 0;
        }
        KeyPart::Compound(_) => 0,
        KeyPart::Letter(l) => match exceptional_bits(exceptional, l) {
            Ok(b) => b,
            Err(m) => {
                throw_argument_error(&m);
                return 0;
            }
        },
    };
    // `fromFunction`: `port` é o sítio da chamada (um trampolim por sítio).
    let persistent = (mode == MODE_PERSISTENT).then(|| (key.clone(), port, closure_code(function), exceptional));
    if let Some(k) = &persistent
        && let Some(f) = PERSISTENTES.with(|p| p.borrow().get(k).copied())
    {
        return f as i64;
    }
    let ctx = Box::new(CallbackContext {
        mode,
        isolate: isolate_id(),
        closure: if mode == MODE_LISTENER { 0 } else { function },
        port: if mode == MODE_LISTENER { port } else { 0 },
        fn_signature: ns,
        key,
        exceptional,
        active_list: 0,
        closed_flag: false,
        trampoline: 0,
    });
    let root = &*ctx as *const CallbackContext as i64;
    match install_callback(ctx) {
        Ok(f) => {
            if mode != MODE_LISTENER {
                HEAP.with(|h| h.borrow_mut().set_global_root(root, function));
            }
            if let Some(k) = persistent {
                PERSISTENTES.with(|p| p.borrow_mut().insert(k, f));
            }
            f as i64
        }
        Err(m) => {
            throw_unsupported(&m);
            0
        }
    }
}

/// Solta o contexto e o trampolim (sem chamada em curso).
fn release_callback(ctx: *mut CallbackContext) {
    // SAFETY: o contexto saiu do registro; ninguém mais o alcança.
    let ctx = unsafe { Box::from_raw(ctx) };
    executable_memory::free(ctx.trampoline);
}

/// `DartForge_ffi_callback_apagar(address)`: o `close()` de um
/// `NativeCallable`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_callback_apagar(address: i64) {
    let Some(ctx) = live_callbacks().lock().unwrap_or_else(|e| e.into_inner()).remove(&(address as usize)) else {
        return;
    };
    let ctx = ctx as *mut CallbackContext;
    HEAP.with(|h| h.borrow_mut().release_global_root(ctx as i64));
    // SAFETY: o contexto é deste isolado (o `close()` roda nele).
    let in_use_flag = unsafe {
        (*ctx).closed_flag = true;
        (*ctx).closure = 0;
        (*ctx).active_list > 0
    };
    if !in_use_flag {
        release_callback(ctx);
    }
}

/// `DartForge_ffi_callback_manter(delta)`: o contador de `NativeCallable`s
/// locais que mantêm o isolado vivo (`keepIsolateAlive`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_callback_manter(delta: i64) {
    adjust_callbacks_keeping_alive(delta);
}

/// A entrada C começou: 0 = chamar a closure aqui; 1 = postar ao ouvinte.
/// Um callback local chamado fora da thread do isolado dono é fatal.
///
/// # Safety
/// `ctx` é o contexto do trampolim (vivo enquanto o callback não fecha).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_callback_entrar(ctx: *mut CallbackContext) -> i64 {
    // SAFETY: garantido por quem chama.
    let c = unsafe { &mut *ctx };
    if c.mode == MODE_LISTENER {
        return 1;
    }
    if c.closed_flag || isolate_id() != c.isolate {
        eprintln!("Cannot invoke native callback outside an isolate.");
        std::process::abort();
    }
    c.active_list += 1;
    0
}

/// A closure do callback (a entrada chama por ela).
///
/// # Safety
/// Como [`dartforge_ffi_callback_entrar`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_callback_closure(ctx: i64) -> i64 {
    // SAFETY: garantido por quem chama.
    unsafe { (*(ctx as *const CallbackContext)).closure }
}

/// O `Pointer<X>` Dart do argumento `i` (endereço `address`).
///
/// # Safety
/// Como [`dartforge_ffi_callback_entrar`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_callback_ponteiro(ctx: i64, i: i64, address: i64) -> i64 {
    // SAFETY: garantido por quem chama.
    let ns = unsafe { (*(ctx as *const CallbackContext)).fn_signature };
    let tipo = RTI.with(|u| match u.borrow().tipo(ns) {
        Type::Function { pos, .. } => pos.get(i as usize).copied(),
        _ => None,
    });
    new_pointer(address, tipo)
}

/// A closure do callback sob ARC, com um owner do código.
///
/// A closure continua raiz do contexto até o `close()`; o owner é do código
/// gerado, que o transfere ou chama release exatamente uma vez. Não aloca no
/// heap, não executa Dart nem deixa exceção pendente.
///
/// # Safety
/// Como [`dartforge_ffi_callback_entrar`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_ffi_callback_closure_owned_v1(ctx: i64) -> i64 {
    // SAFETY: o contrato acima.
    let closure = unsafe { dartforge_ffi_callback_closure(ctx) };
    dartforge_arc_retain(closure);
    closure
}

/// O `Pointer<X>` do argumento `i` sob ARC, com um owner do código.
///
/// O ajudante Dart `_dartforgePonteiro` roda dentro de um escopo nativo: o
/// token do retorno da porta Owned fica nele e é copiado para o owner do
/// chamador antes de fechar. Uma exceção do ajudante fica pendente e o
/// resultado é null, sem owner. Pode alocar, coletar e executar Dart.
///
/// # Safety
/// Como [`dartforge_ffi_callback_entrar`].
///
/// # Panics
/// Ajudante ausente, registro Owned incompatível ou escopo reentrante não
/// fechado; aborta no limite C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_ffi_callback_ponteiro_owned_v1(ctx: i64, i: i64, address: i64) -> i64 {
    // SAFETY: o contrato acima.
    owner_in_native_scope(|| unsafe { dartforge_ffi_callback_ponteiro(ctx, i, address) })
}

/// A entrada C terminou a chamada: devolve 1 (e o retorno excepcional em
/// `out`) se a closure lançou — a exceção é descartada, como na VM.
/// Solta o contexto se ele foi fechado durante a chamada.
///
/// # Safety
/// Como [`dartforge_ffi_callback_entrar`]; `out` é gravável.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_callback_sair(ctx: *mut CallbackContext, out: *mut i64) -> u8 {
    // SAFETY: garantido por quem chama.
    let c = unsafe { &mut *ctx };
    c.active_list -= 1;
    let threw = dartforge_exception_pending() != 0;
    if threw {
        dartforge_exception_clear();
        // SAFETY: garantido por quem chama.
        unsafe { *out = c.exceptional };
    }
    if c.closed_flag && c.active_list == 0 {
        release_callback(ctx);
    }
    u8::from(threw)
}

/// A entrada C de um `listener`: os `n` argumentos (bits de cada tipo C)
/// viram a mensagem `[contexto, a0, a1, …]` à porta do `NativeCallable`.
///
/// # Safety
/// Como [`dartforge_ffi_callback_entrar`]; `args` tem `n` valores.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_callback_postar(ctx: *const CallbackContext, args: *const i64, n: i64) {
    // SAFETY: garantido por quem chama.
    let c = unsafe { &*ctx };
    // SAFETY: idem.
    let bits = unsafe { std::slice::from_raw_parts(args, n.max(0) as usize) };
    let mut items = Vec::with_capacity(bits.len() + 1);
    items.push(Portable::Int(ctx as i64));
    let (_, params) = key_parts(&c.key);
    for (b, p) in bits.iter().zip(params) {
        items.push(match p {
            KeyPart::Letter('f' | 'd') => Portable::Double(f64::from_bits(*b as u64)),
            KeyPart::Letter('b') => Portable::Bool(*b != 0),
            // Uma struct: os bytes (o endereço só vale durante a chamada).
            KeyPart::Compound(rti) => {
                let n = compound_from_rti(rti).map_or(0, |c| c.size as usize);
                // SAFETY: a entrada C passa o endereço dos `n` bytes do
                // argumento, vivos durante a chamada.
                Portable::Bytes(unsafe { std::slice::from_raw_parts(*b as usize as *const u8, n) }.to_vec())
            }
            KeyPart::Letter(_) => Portable::Int(*b),
        });
    }
    post(c.port, Portable::List(items).to_graph());
}

/// `DartForge_ffi_callback_args(message)`: os argumentos Dart de uma
/// mensagem de `listener` (os ponteiros viram `Pointer<X>` da assinatura).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_ffi_callback_args(message: i64) -> i64 {
    // Os elementos da mensagem (uma lista do núcleo), na posição `Ref`.
    let items: Vec<crate::heap::Value> = HEAP.with(|h| {
        let h = h.borrow();
        if !h.e_lista(message) {
            return Vec::new();
        }
        (0..h.list_len(message)).map(|i| h.list_get(message, i)).collect()
    });
    let integer = |v: &crate::heap::Value| argument_int(*v).unwrap_or(0);
    let Some(ctx) = items.first().map(integer) else { return message };
    let alive = live_callbacks().lock().unwrap_or_else(|e| e.into_inner()).values().any(|&c| c as i64 == ctx);
    if !alive {
        return dart_fixed_list(&[]);
    }
    // SAFETY: o contexto está registrado (vivo).
    let (key, ns) = unsafe {
        let c = &*(ctx as *const CallbackContext);
        (c.key.clone(), c.fn_signature)
    };
    let types: Vec<i64> = RTI.with(|u| match u.borrow().tipo(ns) {
        Type::Function { pos, .. } => pos.to_vec(),
        _ => Vec::new(),
    });
    // Cada argumento vira um `Ref` enraizado no quadro (a caixa de um
    // inteiro, o `Pointer`, a struct copiada), e a lista é montada no fim.
    let n = items.len().saturating_sub(1);
    let frame = HEAP.with(|h| h.borrow_mut().push_frame_with_slots(n.max(1)));
    let (_, params) = key_parts(&key);
    let mut out = Vec::with_capacity(n);
    for (i, (v, p)) in items.iter().skip(1).zip(params).enumerate() {
        let x = match p {
            KeyPart::Letter('p') => new_pointer(integer(v), types.get(i).copied()),
            // Os bytes da struct viram uma struct sobre memória Dart.
            KeyPart::Compound(rti) => {
                let bytes = match v {
                    crate::heap::Value::Ref(r) => HEAP.with(|h| h.borrow().typed_bytes(*r).map(<[u8]>::to_vec)).unwrap_or_default(),
                    _ => Vec::new(),
                };
                dartforge_ffi_composto_copia(rti, bytes.as_ptr() as i64)
            }
            KeyPart::Letter(_) => HEAP.with(|h| h.borrow_mut().to_ref(*v)),
        };
        HEAP.with(|h| h.borrow_mut().set_root(frame, i, x));
        out.push(x);
    }
    let list = dart_fixed_list(&out);
    HEAP.with(|h| h.borrow_mut().pop_frame(frame));
    list
}

/// O `int` de um elemento de lista (escalar da forma compacta, `Smi` ou
/// `_Mint`).
fn argument_int(v: crate::heap::Value) -> Option<i64> {
    match v {
        crate::heap::Value::Int(i) => Some(i),
        crate::heap::Value::Bool(b) => Some(i64::from(b)),
        crate::heap::Value::Double(_) => None,
        crate::heap::Value::Ref(r) => HEAP.with(|h| h.borrow().int_of(r)),
    }
}

#[cfg(test)]
mod tests_owned_callback {
    use super::*;

    #[test]
    fn callback_closure_gains_an_owner_and_releases_on_release() {
        let closure = dartforge_arc_objeto_owned_v1(123, 1);
        let ctx = Box::new(CallbackContext {
            mode: MODE_LOCAL, isolate: 0, closure, port: 0, fn_signature: 0,
            key: String::new(), exceptional: 0, active_list: 0,
            closed_flag: false, trampoline: 0,
        });
        let address = (&*ctx as *const CallbackContext) as i64;
        // SAFETY: o contexto acima vive até o fim do teste.
        let owner = unsafe { dartforge_arc_ffi_callback_closure_owned_v1(address) };
        assert_eq!(owner, closure);
        assert_eq!(dartforge_exception_pending(), 0);
        dartforge_arc_release(closure);
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observar_heap_v1(closure), 3);
        dartforge_arc_release(owner);
        dartforge_arc_collect();
        assert_eq!(dartforge_arc_observar_heap_v1(closure), 0);
    }
}

#[cfg(test)]
mod finalizador_callback_tests {
    use super::*;

    #[test]
    fn finalizer_rejects_sync_trampolines_before_creating_attachment() {
        for mode in [MODE_PERSISTENT, MODE_LOCAL, MODE_LISTENER] {
            let ctx = Box::new(CallbackContext {
                mode, isolate: 0, closure: 0, port: 0, fn_signature: 0,
                key: String::new(), exceptional: 0, active_list: 0,
                closed_flag: false, trampoline: 0,
            });
            // Identidade exclusiva para este registro de teste; nunca é chamada.
            let address = (&*ctx as *const CallbackContext) as usize;
            live_callbacks().lock().unwrap().insert(address, address);
            let synchronous = mode != MODE_LISTENER;
            assert_eq!(callback_requires_isolate_thread(address), synchronous);
            if synchronous {
                dartforge_exception_clear();
                // Handles nulos denunciariam uma criação indevida de anexo.
                dartforge_nativo_DartForge_finalizador_anexar_nativo(0, 0, address as i64, 0, 0, 17);
                assert_eq!(dartforge_exception_pending(), 1);
                dartforge_exception_clear();
            }
            live_callbacks().lock().unwrap().remove(&address);
            assert!(!callback_requires_isolate_thread(address));
        }
    }
}
