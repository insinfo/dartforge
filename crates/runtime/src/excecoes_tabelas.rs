// Exceções por tabelas (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13): as
// portas Rust → Dart e a personalidade do desenrolamento.
//
// No modo `--excecoes=tables` uma função Dart que lança sem tratador local
// não retorna com a exceção pendente: ela desenrola a pilha
// (`@df.throw`, gerado em IR) até o `landingpad` de quem a pega. Duas regras
// seguram o runtime de pé:
//
// * **nenhum quadro Rust fica no caminho de um desenrolamento**: toda
//   chamada do runtime a código Dart passa por uma *porta* gerada
//   (`@df.port.<r|v><n>`), que pega qualquer desenrolamento, restaura o topo
//   da pilha-sombra e volta com a exceção **pendente** — o protocolo que o
//   runtime já espera de uma função Dart que lançou;
// * **a exceção em si continua onde sempre esteve**: a pendência do runtime
//   (`dartforge_exception_throw`). O desenrolamento só transfere o controle.
//
// No modo padrão (`checagem`) nenhuma porta é registrada e as chamadas vão
// direto, como antes; a personalidade não é referenciada por ninguém.

/// As portas registradas pelo módulo do programa (`@df.ports`), pelo índice
/// `2 × arity + (1 se devolve value)`. Do processo inteiro: são endereços
/// de código, os mesmos em todos os isolados. 0 = sem porta (modo `checagem`).
static PORTAS_DART: [std::sync::atomic::AtomicUsize; 16] = [
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
];

/// `dartforge_registrar_portas(tabela, n)`: a entrada do programa compilado
/// com `--excecoes=tables` entrega as portas antes de qualquer código Dart
/// rodar.
///
/// # Safety
/// `tabela` aponta para `n` endereços de função (uma constante do módulo).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_portas(tabela: *const usize, n: i64) {
    TABLES_MODE.store(true, std::sync::atomic::Ordering::Release);
    // A sabotagem `sem_porta` (D9, docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
    // §7.3): o runtime chama Dart direto, e uma exceção Dart atravessa os
    // quadros Rust.
    if crate::heap::sabotage("sem_porta") {
        return;
    }
    let n = usize::try_from(n).unwrap_or(0).min(PORTAS_DART.len());
    for (i, port) in PORTAS_DART.iter().enumerate().take(n) {
        // SAFETY: garantido por quem chama (vetor constante com `n` endereços).
        let address = unsafe { *tabela.add(i) };
        port.store(address, std::sync::atomic::Ordering::Release);
    }
}

/// O programa foi compilado com `--excecoes=tables` (a entrada registrou as
/// portas, ou tentou: a sabotagem `sem_porta` volta antes de registrar).
static TABLES_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

thread_local! {
    /// As chamadas diretas Rust → Dart em curso nesta thread no modo
    /// `tables` (só a sabotagem `sem_porta` as produz): o endereço de um
    /// local do quadro Rust de cada uma.
    static DIRECT_CALLS: std::cell::RefCell<Vec<usize>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Uma chamada direta a código Dart, sem porta. No modo `tables` ela fica
/// registrada enquanto roda: um desenrolamento que pouse acima dela teria
/// atravessado o quadro Rust (D9), e a personalidade encerra o processo em
/// vez de deixá-lo seguir com o estado do runtime pela metade.
#[inline(never)]
fn direct<T>(call: impl FnOnce() -> T) -> T {
    if !TABLES_MODE.load(std::sync::atomic::Ordering::Acquire) {
        return call();
    }
    let mark = 0u8;
    let address = std::ptr::addr_of!(mark) as usize;
    DIRECT_CALLS.with(|c| c.borrow_mut().push(address));
    let r = call();
    DIRECT_CALLS.with(|c| c.borrow_mut().pop());
    std::hint::black_box(&mark);
    r
}

/// O pouso no quadro `frame` (o `EstablisherFrame`) passa por cima de uma
/// chamada direta Rust → Dart em curso: o quadro Rust dela está entre o
/// lançamento (mais fundo) e o pouso.
fn landing_crosses_rust(frame: usize) -> bool {
    DIRECT_CALLS.with(|c| c.borrow().iter().any(|&m| m < frame))
}

/// A porta de índice `i`, ou 0 no modo `checagem`.
#[inline]
fn dart_port(i: usize) -> usize {
    PORTAS_DART[i].load(std::sync::atomic::Ordering::Acquire)
}

// As chamadas do runtime a código Dart, por assinatura: `dart_r<n>` devolve o
// valor (`i64`), `dart_v<n>` não devolve nada. `f` é o endereço da função
// Dart, com `n` parâmetros de uma palavra (inteiros, handles e ponteiros têm
// a mesma passagem). Depois da chamada, quem chama confere
// `dartforge_exception_pending`, como sempre.

fn dart_r0(f: usize) -> i64 {
    let arc_guard = arc_prepare_port(f, 0).expect("fronteira de callback ARC inválida");
    arc_finish_port(arc_guard, dart_r0_invoke(f))
}

fn dart_r0_invoke(f: usize) -> i64 {
    let p = dart_port(1);
    if p != 0 {
        // SAFETY: a porta r0 recebe o alvo e 0 palavras, devolvendo uma palavra.
        let g: extern "C" fn(usize) -> i64 = unsafe { std::mem::transmute(p) };
        return g(f);
    }
    // SAFETY: a ABI declarada do alvo tem 0 parâmetros de palavra e retorno i64.
    let g: extern "C" fn() -> i64 = unsafe { std::mem::transmute(f) };
    direct(|| g())
}

fn dart_r1(f: usize, a: i64) -> i64 {
    let arc_guard = arc_prepare_port(f, 1).expect("fronteira de callback ARC inválida");
    arc_finish_port(arc_guard, dart_r1_invoke(f, a))
}

fn dart_r1_invoke(f: usize, a: i64) -> i64 {
    let p = dart_port(3);
    if p != 0 {
        // SAFETY: a porta r1 recebe o alvo e 1 palavras, devolvendo uma palavra.
        let g: extern "C" fn(usize, i64) -> i64 = unsafe { std::mem::transmute(p) };
        return g(f, a);
    }
    // SAFETY: a ABI declarada do alvo tem 1 parâmetros de palavra e retorno i64.
    let g: extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute(f) };
    direct(|| g(a))
}

fn dart_r2(f: usize, a: i64, b: i64) -> i64 {
    let arc_guard = arc_prepare_port(f, 2).expect("fronteira de callback ARC inválida");
    arc_finish_port(arc_guard, dart_r2_invoke(f, a, b))
}

fn dart_r2_invoke(f: usize, a: i64, b: i64) -> i64 {
    let p = dart_port(5);
    if p != 0 {
        // SAFETY: a porta r2 recebe o alvo e 2 palavras, devolvendo uma palavra.
        let g: extern "C" fn(usize, i64, i64) -> i64 = unsafe { std::mem::transmute(p) };
        return g(f, a, b);
    }
    // SAFETY: a ABI declarada do alvo tem 2 parâmetros de palavra e retorno i64.
    let g: extern "C" fn(i64, i64) -> i64 = unsafe { std::mem::transmute(f) };
    direct(|| g(a, b))
}

fn dart_r3(f: usize, a: i64, b: i64, c: i64) -> i64 {
    let arc_guard = arc_prepare_port(f, 3).expect("fronteira de callback ARC inválida");
    arc_finish_port(arc_guard, dart_r3_invoke(f, a, b, c))
}

fn dart_r3_invoke(f: usize, a: i64, b: i64, c: i64) -> i64 {
    let p = dart_port(7);
    if p != 0 {
        // SAFETY: a porta r3 recebe o alvo e 3 palavras, devolvendo uma palavra.
        let g: extern "C" fn(usize, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(p) };
        return g(f, a, b, c);
    }
    // SAFETY: a ABI declarada do alvo tem 3 parâmetros de palavra e retorno i64.
    let g: extern "C" fn(i64, i64, i64) -> i64 = unsafe { std::mem::transmute(f) };
    direct(|| g(a, b, c))
}

fn dart_r4(f: usize, a: i64, b: i64, c: i64, d: i64) -> i64 {
    let arc_guard = arc_prepare_port(f, 4).expect("fronteira de callback ARC inválida");
    arc_finish_port(arc_guard, dart_r4_invoke(f, a, b, c, d))
}

fn dart_r4_invoke(f: usize, a: i64, b: i64, c: i64, d: i64) -> i64 {
    let p = dart_port(9);
    if p != 0 {
        // SAFETY: a porta r4 recebe o alvo e 4 palavras, devolvendo uma palavra.
        let g: extern "C" fn(usize, i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(p) };
        return g(f, a, b, c, d);
    }
    // SAFETY: a ABI declarada do alvo tem 4 parâmetros de palavra e retorno i64.
    let g: extern "C" fn(i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(f) };
    direct(|| g(a, b, c, d))
}

fn dart_r5(f: usize, a: i64, b: i64, c: i64, d: i64, e: i64) -> i64 {
    let arc_guard = arc_prepare_port(f, 5).expect("fronteira de callback ARC inválida");
    arc_finish_port(arc_guard, dart_r5_invoke(f, a, b, c, d, e))
}

fn dart_r5_invoke(f: usize, a: i64, b: i64, c: i64, d: i64, e: i64) -> i64 {
    let p = dart_port(11);
    if p != 0 {
        // SAFETY: a porta r5 recebe o alvo e 5 palavras, devolvendo uma palavra.
        let g: extern "C" fn(usize, i64, i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(p) };
        return g(f, a, b, c, d, e);
    }
    // SAFETY: a ABI declarada do alvo tem 5 parâmetros de palavra e retorno i64.
    let g: extern "C" fn(i64, i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(f) };
    direct(|| g(a, b, c, d, e))
}

#[allow(clippy::too_many_arguments)]

fn dart_r7(f: usize, a: i64, b: i64, c: i64, d: i64, e: i64, g6: i64, h: i64) -> i64 {
    let arc_guard = arc_prepare_port(f, 7).expect("fronteira de callback ARC inválida");
    arc_finish_port(arc_guard, dart_r7_invoke(f, a, b, c, d, e, g6, h))
}

#[allow(clippy::too_many_arguments)]
fn dart_r7_invoke(f: usize, a: i64, b: i64, c: i64, d: i64, e: i64, g6: i64, h: i64) -> i64 {
    let p = dart_port(15);
    if p != 0 {
        // SAFETY: a porta r7 recebe o alvo e 7 palavras, devolvendo uma palavra.
        let g: extern "C" fn(usize, i64, i64, i64, i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(p) };
        return g(f, a, b, c, d, e, g6, h);
    }
    // SAFETY: a ABI declarada do alvo tem 7 parâmetros de palavra e retorno i64.
    let g: extern "C" fn(i64, i64, i64, i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(f) };
    direct(|| g(a, b, c, d, e, g6, h))
}

fn dart_v0(f: usize) {
    if let Some(guard) = arc_prepare_port(f, 0).expect("fronteira de callback ARC inválida") {
        // O consumidor ignora o valor, mas a ABI do corpo continua Ref Owned.
        // Usa a porta de retorno e consome esse token antes de prosseguir.
        arc_discard_port(guard, dart_r0_invoke(f));
        return;
    }
    let p = dart_port(0);
    if p != 0 {
        // SAFETY: `@df.port.v0` tem a assinatura (ptr) -> void.
        let g: extern "C" fn(usize) = unsafe { std::mem::transmute(p) };
        return g(f);
    }
    // SAFETY: `f` é uma função gerada `void` sem parâmetros.
    let g: extern "C" fn() = unsafe { std::mem::transmute(f) };
    direct(|| g())
}

fn dart_v1(f: usize, a: i64) {
    if let Some(guard) = arc_prepare_port(f, 1).expect("fronteira de callback ARC inválida") {
        // O consumidor ignora o valor, mas a ABI do corpo continua Ref Owned.
        // Usa a porta de retorno e consome esse token antes de prosseguir.
        arc_discard_port(guard, dart_r1_invoke(f, a));
        return;
    }
    let p = dart_port(2);
    if p != 0 {
        // SAFETY: `@df.port.v1` tem a assinatura (ptr, i64) -> void.
        let g: extern "C" fn(usize, i64) = unsafe { std::mem::transmute(p) };
        return g(f, a);
    }
    // SAFETY: `f` é uma função gerada `void` com um parâmetro de uma palavra.
    let g: extern "C" fn(i64) = unsafe { std::mem::transmute(f) };
    direct(|| g(a))
}

fn dart_v2(f: usize, a: i64, b: i64) {
    if let Some(guard) = arc_prepare_port(f, 2).expect("fronteira de callback ARC inválida") {
        // O consumidor ignora o valor, mas a ABI do corpo continua Ref Owned.
        // Usa a porta de retorno e consome esse token antes de prosseguir.
        arc_discard_port(guard, dart_r2_invoke(f, a, b));
        return;
    }
    let p = dart_port(4);
    if p != 0 {
        // SAFETY: `@df.port.v2` tem a assinatura (ptr, i64, i64) -> void.
        let g: extern "C" fn(usize, i64, i64) = unsafe { std::mem::transmute(p) };
        return g(f, a, b);
    }
    // SAFETY: `f` é uma função gerada `void` com dois parâmetros de uma palavra.
    let g: extern "C" fn(i64, i64) = unsafe { std::mem::transmute(f) };
    direct(|| g(a, b))
}

fn dart_v3(f: usize, a: i64, b: i64, c: i64) {
    if let Some(guard) = arc_prepare_port(f, 3).expect("fronteira de callback ARC inválida") {
        // O consumidor ignora o valor, mas a ABI do corpo continua Ref Owned.
        // Usa a porta de retorno e consome esse token antes de prosseguir.
        arc_discard_port(guard, dart_r3_invoke(f, a, b, c));
        return;
    }
    let p = dart_port(6);
    if p != 0 {
        // SAFETY: `@df.port.v3` tem a assinatura (ptr, i64, i64, i64) -> void.
        let g: extern "C" fn(usize, i64, i64, i64) = unsafe { std::mem::transmute(p) };
        return g(f, a, b, c);
    }
    // SAFETY: `f` é uma função gerada `void` com três parâmetros de uma palavra.
    let g: extern "C" fn(i64, i64, i64) = unsafe { std::mem::transmute(f) };
    direct(|| g(a, b, c))
}

// ---------------------------------------------------------------------------
// A personalidade.

/// O código da exceção estruturada com que o código gerado desenrola
/// (`RaiseException` em `@df.throw`): um código de cliente, próprio.
#[cfg(all(windows, target_arch = "x86_64"))]
const DART_UNWIND_CODE: u32 = 0xE044_4652;

/// Encerra o processo com uma mensagem: um estado que só um defeito do
/// compilador ou do runtime produz, num ponto de onde não se pode lançar.
fn abort_unwinding(message: &str) -> ! {
    eprintln!("dartforge: defeito no desenrolamento de exceção: {message}");
    std::process::abort()
}

/// Lê um inteiro `uleb128` e avança o cursor.
///
/// # Safety
/// `*p` aponta para um `uleb128` bem formado da LSDA.
#[cfg(any(all(windows, target_arch = "x86_64"), unix))]
unsafe fn read_uleb128(p: &mut *const u8) -> u64 {
    let mut result: u64 = 0;
    let mut offset: u32 = 0;
    loop {
        // SAFETY: garantido por quem chama.
        let byte = unsafe { **p };
        // SAFETY: o byte seguinte ainda é da tabela (o `uleb128` termina num
        // byte sem o bit alto).
        *p = unsafe { p.add(1) };
        if offset < 64 {
            result |= u64::from(byte & 0x7f) << offset;
        }
        offset += 7;
        if byte & 0x80 == 0 {
            return result;
        }
    }
}

/// O deslocamento do pouso e o índice de ação que cobrem `relative_ip`.
/// Os endereços são relativos à função; (0, 0) indica faixa ausente.
/// Preserva a ação: zero identifica cleanup sem ação tipada; valores
/// positivos são índices da tabela de ações, não seletores de tipo.
///
/// A LSDA é a que o LLVM emite para uma personalidade que ele não conhece
/// (formato Itanium, `GCC_except_table`): `LPStart` omitido, a codificação da
/// tabela de tipos (com o deslocamento até o fim dela, quando presente), a
/// codificação `uleb128` dos *call sites* e a tabela deles — início,
/// comprimento, pouso e ação de cada faixa, em ordem. As ações e os tipos
/// não são interpretados aqui. O protocolo atual ainda trata os pousos
/// como handlers; a distinção nas fases será necessária para resume nativo.
///
/// # Safety
/// `lsda` aponta para a LSDA de uma função gerada.
#[cfg(any(all(windows, target_arch = "x86_64"), unix))]
unsafe fn site_and_action_table(lsda: *const u8, relative_ip: u64) -> (u64, u64, *const u8) {
    if lsda.is_null() {
        return (0, 0, std::ptr::null());
    }
    let mut p = lsda;
    // SAFETY: a LSDA tem pelo menos o cabeçalho de três bytes.
    unsafe {
        if *p != 0xff {
            abort_unwinding("LSDA com LPStart explícito");
        }
        p = p.add(1);
        let type_encoding = *p;
        p = p.add(1);
        if type_encoding != 0xff {
            let _ = read_uleb128(&mut p);
        }
        // A codificação dos call sites: `uleb128` (0x01, COFF e ELF) ou
        // `udata4` (0x03, que o LLVM pode usar no Mach-O; não conferido).
        let encoding = *p;
        if encoding != 0x01 && encoding != 0x03 {
            abort_unwinding("LSDA com call sites em codificação inesperada");
        }
        p = p.add(1);
        let read_field = |p: &mut *const u8| -> u64 {
            if encoding == 0x01 {
                read_uleb128(p)
            } else {
                let value = u32::from_le_bytes([**p, *p.add(1), *p.add(2), *p.add(3)]);
                *p = p.add(4);
                u64::from(value)
            }
        };
        let size = read_uleb128(&mut p);
        let end = p.add(size as usize);
        while p < end {
            let start = read_field(&mut p);
            let length = read_field(&mut p);
            let landing = read_field(&mut p);
            let action = read_uleb128(&mut p);
            if relative_ip < start {
                break;
            }
            if relative_ip - start < length {
                return (landing, action, end);
            }
        }
    }
    (0, 0, std::ptr::null())
}

/// Consulta legada: preserva o índice sem interpretar a cadeia.
#[cfg(any(all(windows, target_arch = "x86_64"), unix))]
unsafe fn lsda_site(lsda: *const u8, relative_ip: u64) -> (u64, u64) {
    // SAFETY: mesma LSDA bem formada do chamador.
    let (landing, action, _) = unsafe { site_and_action_table(lsda, relative_ip) };
    (landing, action)
}

/// Lê SLEB128, incluindo deslocamentos negativos entre ações.
#[cfg(any(unix, test))]
unsafe fn read_sleb128(p: &mut *const u8) -> i64 {
    let mut bits = 0_u64;
    let mut offset = 0_u32;
    loop {
        // SAFETY: o chamador fornece uma codificação válida e completa.
        let byte = unsafe { **p };
        *p = unsafe { p.add(1) };
        if offset < 64 { bits |= u64::from(byte & 0x7f) << offset; }
        offset += 7;
        if byte & 0x80 == 0 {
            if offset < 64 && byte & 0x40 != 0 { bits |= u64::MAX << offset; }
            return bits as i64;
        }
    }
}

/// Descobre cleanup e o primeiro seletor catch-all numa cadeia válida.
/// O índice do sítio é deslocamento em bytes, não o seletor do catch.
#[cfg(any(unix, test))]
unsafe fn itanium_actions(tabela: *const u8, ix: u64) -> (bool, i32) {
    if ix == 0 { return (true, 0); }
    let mut cleanup_pad = false;
    let mut selector = 0;
    // SAFETY: índice e elos pertencem à LSDA válida fornecida pelo sistema.
    unsafe {
        let mut p = tabela.add((ix - 1) as usize);
        loop {
            let filter_obj = read_sleb128(&mut p);
            if filter_obj == 0 { cleanup_pad = true; }
            else if filter_obj > 0 && selector == 0 {
                selector = i32::try_from(filter_obj).unwrap_or_else(|_| abort_unwinding("seletor Itanium excede i32"));
            } else if filter_obj < 0 {
                abort_unwinding("filtros negativos não suportados no protocolo cleanup");
            }
            // O elo é relativo ao início do campo de deslocamento, e pode
            // apontar para trás; não é relativo ao fim do SLEB128.
            let origin = p;
            let next_offset = read_sleb128(&mut p);
            if next_offset == 0 { return (cleanup_pad, selector); }
            p = origin.offset(next_offset as isize);
        }
    }
}

/// `EXCEPTION_RECORD` (x86-64).
#[cfg(all(windows, target_arch = "x86_64"))]
#[allow(dead_code)]
#[repr(C)]
struct SehExceptionRecord {
    code: u32,
    flag_bits: u32,
    rec: *mut SehExceptionRecord,
    address: *mut u8,
    n_parameters: u32,
    parameters: [usize; 15],
}

/// `RUNTIME_FUNCTION` (x86-64).
#[cfg(all(windows, target_arch = "x86_64"))]
#[allow(dead_code)]
#[repr(C)]
struct SehRuntimeFunction {
    start: u32,
    end: u32,
    unwinding: u32,
}

/// `DISPATCHER_CONTEXT` (x86-64).
#[cfg(all(windows, target_arch = "x86_64"))]
#[allow(dead_code)]
#[repr(C)]
struct ContextoDeDespachoSeh {
    control_pc: u64,
    image_base: u64,
    function_entry: *const SehRuntimeFunction,
    establisher_frame: u64,
    target_ip: u64,
    context_record: *mut u8,
    language_handler: *mut u8,
    handler_data: *const u8,
    history_table: *mut u8,
    scope_index: u32,
    fill0: u32,
}

#[cfg(all(windows, target_arch = "x86_64"))]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn RtlUnwindEx(
        target_frame: *mut u8,
        target_ip: *mut u8,
        exception_record: *mut SehExceptionRecord,
        return_value: *mut u8,
        context_record: *mut u8,
        history_table: *mut u8,
    );
}

/// A personalidade no Windows x86-64: o *language handler* que o `.xdata` de
/// cada função gerada com `landingpad` nomeia.
///
/// * exceção que não é o nosso desenrolamento (violação de acesso, C++ de
///   uma biblioteca nativa): não é Dart, a busca continua;
/// * segunda passagem (`EXCEPTION_UNWINDING`/`EXCEPTION_EXIT_UNWIND`): não há
///   limpeza a fazer nos quadros do meio (os quadros de raízes pulados são
///   desempilhados de uma vez pelo pouso, que restaura o topo);
/// * busca: se o ponto da chamada tem pouso na LSDA, `RtlUnwindEx` desenrola
///   até este quadro e continua no pouso. Não volta.
///
/// # Safety
/// Chamada pelo despachante de exceções do sistema com os quatro argumentos
/// de um `EXCEPTION_ROUTINE`.
#[cfg(all(windows, target_arch = "x86_64"))]
unsafe fn seh_personality(rec: *mut u8, frame: *mut u8, dispatch: *mut u8) -> i32 {
    /// `ExceptionContinueSearch`.
    const CONTINUE_SEARCH: i32 = 1;
    let rec = rec.cast::<SehExceptionRecord>();
    let dispatch = dispatch.cast::<ContextoDeDespachoSeh>();
    // SAFETY: os ponteiros são os do despachante; a LSDA é a da função cujo
    // quadro está sendo examinado.
    unsafe {
        if (*rec).code != DART_UNWIND_CODE {
            return CONTINUE_SEARCH;
        }
        if (*rec).flag_bits & 0x6 != 0 {
            return CONTINUE_SEARCH;
        }
        if (*dispatch).function_entry.is_null() {
            return CONTINUE_SEARCH;
        }
        let start = (*dispatch).image_base + u64::from((*(*dispatch).function_entry).start);
        // O endereço de retorno é o da instrução seguinte à chamada, que pode
        // já ser de outra faixa: o ponto da chamada é um byte antes.
        let relative_ip = (*dispatch).control_pc.wrapping_sub(1).wrapping_sub(start);
        let (landing, _action) = lsda_site((*dispatch).handler_data, relative_ip);
        if landing == 0 {
            return CONTINUE_SEARCH;
        }
        if landing_crosses_rust(frame as usize) {
            abort_unwinding("uma exceção Dart atravessou quadros Rust (chamada do runtime a código Dart sem porta)");
        }
        RtlUnwindEx(
            frame,
            (start + landing) as *mut u8,
            rec,
            std::ptr::null_mut(),
            (*dispatch).context_record,
            (*dispatch).history_table,
        );
    }
    abort_unwinding("RtlUnwindEx voltou")
}

/// A personalidade das funções geradas com `--excecoes=tables`
/// (`personality ptr @dartforge_personalidade` no IR). Só o Windows x86-64
/// tem o modo; nos outros alvos o emissor o recusa e ninguém referencia este
/// símbolo.
///
/// # Safety
/// Só o despachante de exceções do sistema a chama.
#[cfg(not(unix))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_personalidade(rec: *mut u8, frame: *mut u8, contexto: *mut u8, dispatch: *mut u8) -> i32 {
    let _ = contexto;
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        // SAFETY: os argumentos são os do despachante do sistema.
        unsafe { seh_personality(rec, frame, dispatch) }
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        let _ = (rec, frame, dispatch);
        abort_unwinding("exceções por tabelas não existem neste alvo")
    }
}

// ---------------------------------------------------------------------------
// Os alvos Itanium (Linux, macOS; docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
// §13.11 e Etapa 4). Escrito sem compilar nem executar (2026-10-05).

/// As funções do desenrolador do sistema (`libgcc_s` ou `libunwind`, que a
/// biblioteca padrão do Rust já liga).
#[cfg(unix)]
unsafe extern "C" {
    fn _Unwind_RaiseException(object: *mut UnwindObject) -> i32;
    fn _Unwind_GetIPInfo(contexto: *mut u8, before_instruction: *mut i32) -> usize;
    fn _Unwind_GetRegionStart(contexto: *mut u8) -> usize;
    fn _Unwind_GetLanguageSpecificData(contexto: *mut u8) -> *const u8;
    fn _Unwind_SetGR(contexto: *mut u8, register: i32, value: usize);
    fn _Unwind_SetIP(contexto: *mut u8, value: usize);
}

/// A classe das exceções deste runtime (`exception_class`): os oito
/// caracteres `DARTFRGE`.
#[cfg(unix)]
const DART_UNWIND_CLASS: u64 = u64::from_be_bytes(*b"DARTFRGE");

/// `_Unwind_Exception`: a classe, a limpeza e a área privada do
/// desenrolador (duas palavras no x86-64 e no aarch64; sobra espaço de
/// propósito). O alinhamento é o máximo do alvo, como no cabeçalho C.
#[cfg(unix)]
#[repr(C, align(16))]
struct UnwindObject {
    class: u64,
    cleanup_pad: Option<unsafe extern "C" fn(i32, *mut UnwindObject)>,
    private: [u64; 6],
}

#[cfg(unix)]
thread_local! {
    /// O objeto de desenrolamento da thread. Um só basta: a exceção Dart é
    /// a pendência do runtime, e todo pouso do código gerado pega tudo, de
    /// modo que nunca há dois desenrolamentos em curso na mesma thread.
    static UNWIND_OBJECT: std::cell::UnsafeCell<UnwindObject> =
        const { std::cell::UnsafeCell::new(UnwindObject { class: DART_UNWIND_CLASS, cleanup_pad: None, private: [0; 6] }) };
}

/// O objeto de exceção da thread, pronto para o desenrolamento das
/// exceções por tabelas nos alvos Itanium: `@df.throw` (gerado em IR) o
/// entrega ele mesmo ao `_Unwind_RaiseException`, sem quadro Rust no
/// caminho até o pouso (a guarda de abortar de uma função `extern "C"` na
/// variante `panic=unwind` do runtime pararia a busca). Não aloca nem lança.
///
/// # Safety
/// Só o código gerado a chama, com a exceção Dart já pendente.
#[cfg(unix)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_objeto_de_desenrolamento() -> *mut u8 {
    let object = UNWIND_OBJECT.with(|o| o.get());
    // SAFETY: o objeto é da thread e não há outro desenrolamento em curso;
    // o desenrolador só usa a área privada dele.
    unsafe {
        (*object).class = DART_UNWIND_CLASS;
        (*object).cleanup_pad = None;
        (*object).private = [0; 6];
    }
    object.cast()
}

/// O `_Unwind_RaiseException` de `@df.throw` voltou: nenhum pouso pegou (a
/// entrada do programa e as portas têm pouso, então é defeito). Encerra.
#[cfg(unix)]
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_desenrolamento_falhou(reason: i32) -> ! {
    abort_unwinding(&format!("_Unwind_RaiseException voltou com {reason} (nenhum quadro pegou)"))
}

/// A personalidade Itanium das funções com pouso: a mesma LSDA e a mesma
/// regra da SEH (todo pouso pega tudo), nas duas fases do desenrolador.
///
/// # Safety
/// Só o desenrolador do sistema a chama.
#[cfg(unix)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_personalidade(version: i32, actions: i32, class: u64, _object: *mut u8, contexto: *mut u8) -> i32 {
    // SAFETY: contrato do despachante, preservando o protocolo legado.
    unsafe { itanium_personality(version, actions, class, _object, contexto, false) }
}

/// Personalidade Itanium para cleanup e catch-all, inclusive em pousos mistos.
///
/// Cleanup não termina a busca; executa na fase de unwind, inclusive para
/// exceção estrangeira/forçada. Entrega ao landingpad o objeto original e
/// seletor zero, permitindo resume sem reinicializar sua área privada.
/// Catch-all Dart usa o seletor da ação e só trata no quadro escolhido.
/// Pousos mistos executam cleanup para exceção estrangeira/forçada com seletor
/// zero. Não interpreta filtros tipados: as ações positivas são catch-all.
/// O emissor legado continua usando `dartforge_personalidade`.
///
/// # Erros
/// Devolve os códigos do protocolo Itanium; versão diferente de 1 devolve
/// erro fatal na fase de busca. Ausência de sítio permite continuar a busca.
///
/// # Safety
/// Só o desenrolador do sistema chama, com contexto/objeto e LSDA válidos.
/// A LSDA contém apenas cleanup e catch-all (tipo null); ações e elos são
/// válidos. O pouso precisa distinguir os seletores e preservar o par recebido.
///
/// ```
/// # #[cfg(unix)] {
/// let _: unsafe extern "C" fn(i32, i32, u64, *mut u8, *mut u8) -> i32 =
///     dartforge_runtime::abi::dartforge_personalidade_cleanup_itanium;
/// # }
/// ```
#[cfg(unix)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_personalidade_cleanup_itanium(
    version: i32, actions: i32, class: u64, object: *mut u8, contexto: *mut u8,
) -> i32 {
    // SAFETY: contrato do despachante para sítios do protocolo de cleanup.
    unsafe { itanium_personality(version, actions, class, object, contexto, true) }
}

/// Decisão das fases, independente das APIs e registradores do alvo.
#[cfg(any(unix, test))]
fn itanium_decision(version: i32, actions: i32, ours: bool, landing: u64, cleanup_pad: bool, selector: i32, cleanup: bool) -> i32 {
    const FATAL_IN_PHASE_1: i32 = 3;
    const HANDLER_FOUND: i32 = 6;
    const INSTALL_CONTEXT: i32 = 7;
    const CONTINUE_UNWIND: i32 = 8;
    if version != 1 { return FATAL_IN_PHASE_1; }
    if landing == 0 { return CONTINUE_UNWIND; }
    let search = actions & 1 != 0;
    let unwind = actions & 2 != 0;
    let handler_frame = actions & 4 != 0;
    let forced = actions & 8 != 0;
    if !cleanup {
        if !ours { return CONTINUE_UNWIND; }
        if search { return HANDLER_FOUND; }
        return if forced { CONTINUE_UNWIND } else { INSTALL_CONTEXT };
    }
    if search {
        return if ours && !forced && selector > 0 { HANDLER_FOUND } else { CONTINUE_UNWIND };
    }
    if unwind && (cleanup_pad || (ours && !forced && handler_frame && selector > 0)) {
        INSTALL_CONTEXT
    } else {
        CONTINUE_UNWIND
    }
}

#[cfg(unix)]
unsafe fn itanium_personality(
    version: i32, actions: i32, class: u64, object: *mut u8, contexto: *mut u8, cleanup: bool,
) -> i32 {
    // Valida versão/classe antes de acessar o contexto, como no protocolo
    // legado. Cleanup também precisa acompanhar exceções estrangeiras.
    if version != 1 { return 3; }
    let ours = class == DART_UNWIND_CLASS;
    if !cleanup && !ours { return 8; }
    // SAFETY: contexto e LSDA entregues pelo desenrolador do sistema.
    unsafe {
        let mut before: i32 = 0;
        let ip = _Unwind_GetIPInfo(contexto, &mut before) as u64;
        let ip = if before == 0 { ip.wrapping_sub(1) } else { ip };
        let start = _Unwind_GetRegionStart(contexto) as u64;
        let (landing, action, tabela) = site_and_action_table(_Unwind_GetLanguageSpecificData(contexto), ip.wrapping_sub(start));
        let (cleanup_pad, selector) = if cleanup && landing != 0 {
            itanium_actions(tabela, action)
        } else { (action == 0, if action != 0 { 1 } else { 0 }) };
        let decision = itanium_decision(version, actions, ours, landing, cleanup_pad, selector, cleanup);
        if decision != 7 { return decision; }
        // Cleanup intermediário ou estrangeiro usa seletor zero. Um pouso
        // misto escolhido como handler também executa cleanup, com o seletor
        // do catch, antes de desviar para o tratador ou retomar a exceção.
        let handles = ours && actions & 4 != 0 && actions & 8 == 0 && selector > 0;
        _Unwind_SetGR(contexto, 0, if cleanup { object as usize } else { 0 });
        _Unwind_SetGR(contexto, 1, if cleanup && handles { selector as usize } else { 0 });
        _Unwind_SetIP(contexto, (start + landing) as usize);
        decision
    }
}

#[cfg(all(test, any(all(windows, target_arch = "x86_64"), unix)))]
mod tests_exceptions_tables {
    use super::*;

    #[test]
    fn action_chain_separates_index_selector_and_cleanup() {
        // SAFETY: cadeias completas em arrays locais, com elos válidos.
        unsafe {
            assert_eq!(itanium_actions(std::ptr::null(), 0), (true, 0));
            assert_eq!(itanium_actions([0, 0].as_ptr(), 1), (true, 0));
            assert_eq!(itanium_actions([2, 0].as_ptr(), 1), (false, 2));
            assert_eq!(itanium_actions([0, 1, 1, 0].as_ptr(), 1), (true, 1));
            // O registro no índice 3 volta três bytes desde seu campo elo.
            assert_eq!(itanium_actions([0, 0, 1, 0x7d].as_ptr(), 3), (true, 1));
        }
    }

    #[test]
    fn sleb128_preserves_sign_and_extremes() {
        for (bytes, expected) in [
            (vec![0x7f], -1), (vec![0x7d], -3), (vec![0x80, 0x01], 128),
            (vec![0x80, 0x7f], -128),
            (vec![0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00], i64::MAX),
            (vec![0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x7f], i64::MIN),
        ] {
            let mut p = bytes.as_ptr();
            // SAFETY: SLEB128 completo no vetor.
            assert_eq!(unsafe { read_sleb128(&mut p) }, expected);
            assert_eq!(p, unsafe { bytes.as_ptr().add(bytes.len()) });
        }
    }

    #[test]
    fn mixed_landing_cleans_foreign_without_catching_and_selects_dart_catch() {
        assert_eq!(itanium_decision(1, 1, false, 7, true, 1, true), 8);
        assert_eq!(itanium_decision(1, 2, false, 7, true, 1, true), 7);
        assert_eq!(itanium_decision(1, 2 | 8, false, 7, true, 1, true), 7);
        assert_eq!(itanium_decision(1, 1, true, 7, true, 1, true), 6);
        assert_eq!(itanium_decision(1, 2 | 4, true, 7, true, 1, true), 7);
    }

    #[test]
    fn cleanup_does_not_stop_search_and_installs_in_unwind_phase() {
        for ours in [false, true] {
            for forced in [0, 8] {
                assert_eq!(itanium_decision(1, 1 | forced, ours, 7, true, 0, true), 8);
                assert_eq!(itanium_decision(1, 2 | forced, ours, 7, true, 0, true), 7);
                assert_eq!(itanium_decision(1, 0 | forced, ours, 7, true, 0, true), 8);
            }
        }
        assert_eq!(itanium_decision(1, 2, true, 0, true, 0, true), 8);
        assert_eq!(itanium_decision(0, 1, true, 7, true, 0, true), 3);
    }

    #[test]
    fn catch_all_installs_only_in_frame_chosen_for_dart_exception() {
        assert_eq!(itanium_decision(1, 1, true, 7, false, 1, true), 6);
        assert_eq!(itanium_decision(1, 2, true, 7, false, 1, true), 8);
        assert_eq!(itanium_decision(1, 2 | 4, true, 7, false, 1, true), 7);
        for actions in [1, 2, 2 | 4, 2 | 8, 2 | 4 | 8] {
            assert_eq!(itanium_decision(1, actions, false, 7, false, 1, true), 8);
        }
        assert_eq!(itanium_decision(1, 2 | 4 | 8, true, 7, false, 1, true), 8);
    }

    #[test]
    fn legacy_protocol_preserves_statepoint_landings() {
        for action in [0, 1] {
            assert_eq!(itanium_decision(1, 1, true, 7, action == 0, action as i32, false), 6);
            assert_eq!(itanium_decision(1, 2, true, 7, action == 0, action as i32, false), 7);
            assert_eq!(itanium_decision(1, 2 | 8, true, 7, action == 0, action as i32, false), 8);
            assert_eq!(itanium_decision(1, 2, false, 7, action == 0, action as i32, false), 8);
        }
    }

    /// A LSDA do §13.10 (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md): cabeçalho
    /// com tabela de tipos (`catch ptr null`), três faixas — sem pouso, com
    /// pouso em 0x45, sem pouso —, a tabela de ações e a de tipos.
    const WITH_TYPES: [u8; 28] = [
        0xff, // LPStart omitido
        0x00, // tipos: absptr
        0x15, // distância até o fim da tabela de tipos
        0x01, // call sites em uleb128
        12,   // bytes da tabela de call sites
        0x00, 0x10, 0x00, 0x00, // [0x00, 0x10): sem pouso
        0x10, 0x20, 0x45, 0x01, // [0x10, 0x30): pouso 0x45
        0x30, 0x50, 0x00, 0x00, // [0x30, 0x80): sem pouso
        0x01, 0x00, // ações
        0x00, // alinhamento
        0, 0, 0, 0, 0, 0, 0, 0, // tipo 1 = null
    ];

    #[test]
    fn landing_by_call_site_range() {
        let p = WITH_TYPES.as_ptr();
        // SAFETY: uma LSDA bem formada, inteira no vetor.
        unsafe {
            assert_eq!(lsda_site(p, 0x00), (0, 0));
            assert_eq!(lsda_site(p, 0x0f), (0, 0));
            assert_eq!(lsda_site(p, 0x10), (0x45, 1));
            assert_eq!(lsda_site(p, 0x2f), (0x45, 1));
            assert_eq!(lsda_site(p, 0x30), (0, 0));
            assert_eq!(lsda_site(p, 0x7f), (0, 0));
            // Depois da última faixa: sem pouso.
            assert_eq!(lsda_site(p, 0x80), (0, 0));
            assert_eq!(lsda_site(p, 0x1000), (0, 0));
        }
    }

    #[test]
    fn without_type_table_and_with_two_byte_uleb128() {
        // `cleanup` puro: o segundo byte é 0xff e não há a distância. Uma
        // faixa com buraco antes ([0x80, 0x180), pouso 0x200), em uleb128
        // de dois bytes.
        let lsda: [u8; 10] = [0xff, 0xff, 0x01, 6, 0x80, 0x01, 0x80, 0x02, 0x80, 0x04];
        // A ação da faixa vem depois; o vetor termina nela.
        let mut full = lsda.to_vec();
        full.push(0x00);
        full[3] = 7;
        let p = full.as_ptr();
        // SAFETY: uma LSDA bem formada, inteira no vetor.
        unsafe {
            assert_eq!(lsda_site(p, 0x00), (0, 0));
            assert_eq!(lsda_site(p, 0x7f), (0, 0));
            assert_eq!(lsda_site(p, 0x80), (0x200, 0));
            assert_eq!(lsda_site(p, 0x17f), (0x200, 0));
            assert_eq!(lsda_site(p, 0x180), (0, 0));
        }
    }

    #[test]
    fn uleb128_action_is_independent_of_address_encoding() {
        // Endereços udata4 (Mach-O): a ação permanece uleb128 e pode ter
        // mais de um byte. É um índice cru, não um seletor de catch.
        let mut lsda = vec![0xff, 0xff, 0x03, 14];
        for field in [0x10_u32, 0x20, 0x200] {
            lsda.extend(field.to_le_bytes());
        }
        lsda.extend([0x81, 0x01]);
        // SAFETY: cabeçalho e faixa completos, inteiros no vetor.
        unsafe {
            assert_eq!(lsda_site(lsda.as_ptr(), 0x0f), (0, 0));
            assert_eq!(lsda_site(lsda.as_ptr(), 0x10), (0x200, 129));
            assert_eq!(lsda_site(lsda.as_ptr(), 0x2f), (0x200, 129));
            assert_eq!(lsda_site(lsda.as_ptr(), 0x30), (0, 0));
        }
    }

    #[test]
    fn range_at_address_limit_does_not_add_length() {
        fn uleb(mut value: u64, bytes: &mut Vec<u8>) {
            loop {
                let byte = (value & 0x7f) as u8;
                value >>= 7;
                bytes.push(byte | if value == 0 { 0 } else { 0x80 });
                if value == 0 { break; }
            }
        }
        let mut range = Vec::new();
        for field in [u64::MAX - 1, 2, 7, 0] {
            uleb(field, &mut range);
        }
        let mut lsda = vec![0xff, 0xff, 0x01, range.len() as u8];
        lsda.extend(range);
        // SAFETY: faixa completa em uleb128; o comprimento conceitual
        // termina após o maior endereço sem exigir soma em u64.
        unsafe {
            assert_eq!(lsda_site(lsda.as_ptr(), u64::MAX - 2), (0, 0));
            assert_eq!(lsda_site(lsda.as_ptr(), u64::MAX - 1), (7, 0));
            assert_eq!(lsda_site(lsda.as_ptr(), u64::MAX), (7, 0));
        }
    }

    #[test]
    fn missing_lsda_has_no_landing_pad() {
        // SAFETY: o ponteiro nulo é tratado antes de qualquer leitura.
        assert_eq!(unsafe { lsda_site(std::ptr::null(), 0x10) }, (0, 0));
    }

    #[test]
    fn without_registered_port_call_is_direct() {
        extern "C" fn total_sum(a: i64, b: i64) -> i64 {
            a + b
        }
        extern "C" fn seven() -> i64 {
            7
        }
        // Nenhum teste registra portas: o modo de sempre.
        assert_eq!(dart_r2(total_sum as usize, 40, 2), 42);
        assert_eq!(dart_r0(seven as usize), 7);
    }
}
