// Exceções por tabelas (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13): as
// portas Rust → Dart e a personalidade do desenrolamento.
//
// No modo `--excecoes=tabelas` uma função Dart que lança sem tratador local
// não retorna com a exceção pendente: ela desenrola a pilha
// (`@df.lancar`, gerado em IR) até o `landingpad` de quem a pega. Duas regras
// seguram o runtime de pé:
//
// * **nenhum quadro Rust fica no caminho de um desenrolamento**: toda
//   chamada do runtime a código Dart passa por uma *porta* gerada
//   (`@df.porta.<r|v><n>`), que pega qualquer desenrolamento, restaura o topo
//   da pilha-sombra e volta com a exceção **pendente** — o protocolo que o
//   runtime já espera de uma função Dart que lançou;
// * **a exceção em si continua onde sempre esteve**: a pendência do runtime
//   (`dartforge_exception_throw`). O desenrolamento só transfere o controle.
//
// No modo padrão (`checagem`) nenhuma porta é registrada e as chamadas vão
// direto, como antes; a personalidade não é referenciada por ninguém.

/// As portas registradas pelo módulo do programa (`@df.portas`), pelo índice
/// `2 × aridade + (1 se devolve valor)`. Do processo inteiro: são endereços
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
/// com `--excecoes=tabelas` entrega as portas antes de qualquer código Dart
/// rodar.
///
/// # Safety
/// `tabela` aponta para `n` endereços de função (uma constante do módulo).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_portas(tabela: *const usize, n: i64) {
    MODO_TABELAS.store(true, std::sync::atomic::Ordering::Release);
    // A sabotagem `sem_porta` (D9, docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
    // §7.3): o runtime chama Dart direto, e uma exceção Dart atravessa os
    // quadros Rust.
    if crate::heap::sabotagem("sem_porta") {
        return;
    }
    let n = usize::try_from(n).unwrap_or(0).min(PORTAS_DART.len());
    for (i, porta) in PORTAS_DART.iter().enumerate().take(n) {
        // SAFETY: garantido por quem chama (vetor constante com `n` endereços).
        let endereco = unsafe { *tabela.add(i) };
        porta.store(endereco, std::sync::atomic::Ordering::Release);
    }
}

/// O programa foi compilado com `--excecoes=tabelas` (a entrada registrou as
/// portas, ou tentou: a sabotagem `sem_porta` volta antes de registrar).
static MODO_TABELAS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

thread_local! {
    /// As chamadas diretas Rust → Dart em curso nesta thread no modo
    /// `tabelas` (só a sabotagem `sem_porta` as produz): o endereço de um
    /// local do quadro Rust de cada uma.
    static CHAMADAS_DIRETAS: std::cell::RefCell<Vec<usize>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Uma chamada direta a código Dart, sem porta. No modo `tabelas` ela fica
/// registrada enquanto roda: um desenrolamento que pouse acima dela teria
/// atravessado o quadro Rust (D9), e a personalidade encerra o processo em
/// vez de deixá-lo seguir com o estado do runtime pela metade.
#[inline(never)]
fn direta<T>(chamar: impl FnOnce() -> T) -> T {
    if !MODO_TABELAS.load(std::sync::atomic::Ordering::Acquire) {
        return chamar();
    }
    let marca = 0u8;
    let endereco = std::ptr::addr_of!(marca) as usize;
    CHAMADAS_DIRETAS.with(|c| c.borrow_mut().push(endereco));
    let r = chamar();
    CHAMADAS_DIRETAS.with(|c| c.borrow_mut().pop());
    std::hint::black_box(&marca);
    r
}

/// O pouso no quadro `quadro` (o `EstablisherFrame`) passa por cima de uma
/// chamada direta Rust → Dart em curso: o quadro Rust dela está entre o
/// lançamento (mais fundo) e o pouso.
fn pouso_atravessa_rust(quadro: usize) -> bool {
    CHAMADAS_DIRETAS.with(|c| c.borrow().iter().any(|&m| m < quadro))
}

/// A porta de índice `i`, ou 0 no modo `checagem`.
#[inline]
fn porta_dart(i: usize) -> usize {
    PORTAS_DART[i].load(std::sync::atomic::Ordering::Acquire)
}

// As chamadas do runtime a código Dart, por assinatura: `dart_r<n>` devolve o
// valor (`i64`), `dart_v<n>` não devolve nada. `f` é o endereço da função
// Dart, com `n` parâmetros de uma palavra (inteiros, handles e ponteiros têm
// a mesma passagem). Depois da chamada, quem chama confere
// `dartforge_exception_pending`, como sempre.

fn dart_r0(f: usize) -> i64 {
    let p = porta_dart(1);
    if p != 0 {
        // SAFETY: `@df.porta.r0` tem a assinatura (ptr) -> i64.
        let g: extern "C" fn(usize) -> i64 = unsafe { std::mem::transmute(p) };
        return g(f);
    }
    // SAFETY: `f` é uma função gerada sem parâmetros que devolve uma palavra.
    let g: extern "C" fn() -> i64 = unsafe { std::mem::transmute(f) };
    direta(|| g())
}

fn dart_r1(f: usize, a: i64) -> i64 {
    let p = porta_dart(3);
    if p != 0 {
        // SAFETY: `@df.porta.r1` tem a assinatura (ptr, i64) -> i64.
        let g: extern "C" fn(usize, i64) -> i64 = unsafe { std::mem::transmute(p) };
        return g(f, a);
    }
    // SAFETY: `f` é uma função gerada com um parâmetro de uma palavra.
    let g: extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute(f) };
    direta(|| g(a))
}

fn dart_r2(f: usize, a: i64, b: i64) -> i64 {
    let p = porta_dart(5);
    if p != 0 {
        // SAFETY: `@df.porta.r2` tem a assinatura (ptr, i64, i64) -> i64.
        let g: extern "C" fn(usize, i64, i64) -> i64 = unsafe { std::mem::transmute(p) };
        return g(f, a, b);
    }
    // SAFETY: `f` é uma função gerada com dois parâmetros de uma palavra.
    let g: extern "C" fn(i64, i64) -> i64 = unsafe { std::mem::transmute(f) };
    direta(|| g(a, b))
}

fn dart_r3(f: usize, a: i64, b: i64, c: i64) -> i64 {
    let p = porta_dart(7);
    if p != 0 {
        // SAFETY: `@df.porta.r3` tem a assinatura (ptr, i64, i64, i64) -> i64.
        let g: extern "C" fn(usize, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(p) };
        return g(f, a, b, c);
    }
    // SAFETY: `f` é uma função gerada com três parâmetros de uma palavra.
    let g: extern "C" fn(i64, i64, i64) -> i64 = unsafe { std::mem::transmute(f) };
    direta(|| g(a, b, c))
}

fn dart_r4(f: usize, a: i64, b: i64, c: i64, d: i64) -> i64 {
    let p = porta_dart(9);
    if p != 0 {
        // SAFETY: `@df.porta.r4` tem a assinatura (ptr, i64 × 4) -> i64.
        let g: extern "C" fn(usize, i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(p) };
        return g(f, a, b, c, d);
    }
    // SAFETY: `f` é uma função gerada com quatro parâmetros de uma palavra.
    let g: extern "C" fn(i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(f) };
    direta(|| g(a, b, c, d))
}

fn dart_r5(f: usize, a: i64, b: i64, c: i64, d: i64, e: i64) -> i64 {
    let p = porta_dart(11);
    if p != 0 {
        // SAFETY: `@df.porta.r5` tem a assinatura (ptr, i64 × 5) -> i64.
        let g: extern "C" fn(usize, i64, i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(p) };
        return g(f, a, b, c, d, e);
    }
    // SAFETY: `f` é uma função gerada com cinco parâmetros de uma palavra.
    let g: extern "C" fn(i64, i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(f) };
    direta(|| g(a, b, c, d, e))
}

#[allow(clippy::too_many_arguments)]
fn dart_r7(f: usize, a: i64, b: i64, c: i64, d: i64, e: i64, g6: i64, h: i64) -> i64 {
    let p = porta_dart(15);
    if p != 0 {
        // SAFETY: `@df.porta.r7` tem a assinatura (ptr, i64 × 7) -> i64.
        let g: extern "C" fn(usize, i64, i64, i64, i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(p) };
        return g(f, a, b, c, d, e, g6, h);
    }
    // SAFETY: `f` é uma função gerada com sete parâmetros de uma palavra.
    let g: extern "C" fn(i64, i64, i64, i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(f) };
    direta(|| g(a, b, c, d, e, g6, h))
}

fn dart_v0(f: usize) {
    let p = porta_dart(0);
    if p != 0 {
        // SAFETY: `@df.porta.v0` tem a assinatura (ptr) -> void.
        let g: extern "C" fn(usize) = unsafe { std::mem::transmute(p) };
        return g(f);
    }
    // SAFETY: `f` é uma função gerada `void` sem parâmetros.
    let g: extern "C" fn() = unsafe { std::mem::transmute(f) };
    direta(|| g())
}

fn dart_v1(f: usize, a: i64) {
    let p = porta_dart(2);
    if p != 0 {
        // SAFETY: `@df.porta.v1` tem a assinatura (ptr, i64) -> void.
        let g: extern "C" fn(usize, i64) = unsafe { std::mem::transmute(p) };
        return g(f, a);
    }
    // SAFETY: `f` é uma função gerada `void` com um parâmetro de uma palavra.
    let g: extern "C" fn(i64) = unsafe { std::mem::transmute(f) };
    direta(|| g(a))
}

fn dart_v2(f: usize, a: i64, b: i64) {
    let p = porta_dart(4);
    if p != 0 {
        // SAFETY: `@df.porta.v2` tem a assinatura (ptr, i64, i64) -> void.
        let g: extern "C" fn(usize, i64, i64) = unsafe { std::mem::transmute(p) };
        return g(f, a, b);
    }
    // SAFETY: `f` é uma função gerada `void` com dois parâmetros de uma palavra.
    let g: extern "C" fn(i64, i64) = unsafe { std::mem::transmute(f) };
    direta(|| g(a, b))
}

fn dart_v3(f: usize, a: i64, b: i64, c: i64) {
    let p = porta_dart(6);
    if p != 0 {
        // SAFETY: `@df.porta.v3` tem a assinatura (ptr, i64, i64, i64) -> void.
        let g: extern "C" fn(usize, i64, i64, i64) = unsafe { std::mem::transmute(p) };
        return g(f, a, b, c);
    }
    // SAFETY: `f` é uma função gerada `void` com três parâmetros de uma palavra.
    let g: extern "C" fn(i64, i64, i64) = unsafe { std::mem::transmute(f) };
    direta(|| g(a, b, c))
}

// ---------------------------------------------------------------------------
// A personalidade.

/// O código da exceção estruturada com que o código gerado desenrola
/// (`RaiseException` em `@df.lancar`): um código de cliente, próprio.
#[cfg(all(windows, target_arch = "x86_64"))]
const CODIGO_DE_DESENROLAMENTO_DART: u32 = 0xE044_4652;

/// Encerra o processo com uma mensagem: um estado que só um defeito do
/// compilador ou do runtime produz, num ponto de onde não se pode lançar.
fn abortar_desenrolamento(mensagem: &str) -> ! {
    eprintln!("dartforge: defeito no desenrolamento de exceção: {mensagem}");
    std::process::abort()
}

/// Lê um inteiro `uleb128` e avança o cursor.
///
/// # Safety
/// `*p` aponta para um `uleb128` bem formado da LSDA.
#[cfg(any(all(windows, target_arch = "x86_64"), unix))]
unsafe fn ler_uleb128(p: &mut *const u8) -> u64 {
    let mut resultado: u64 = 0;
    let mut deslocamento: u32 = 0;
    loop {
        // SAFETY: garantido por quem chama.
        let byte = unsafe { **p };
        // SAFETY: o byte seguinte ainda é da tabela (o `uleb128` termina num
        // byte sem o bit alto).
        *p = unsafe { p.add(1) };
        if deslocamento < 64 {
            resultado |= u64::from(byte & 0x7f) << deslocamento;
        }
        deslocamento += 7;
        if byte & 0x80 == 0 {
            return resultado;
        }
    }
}

/// O deslocamento do pouso e o índice de ação que cobrem `ip_relativo`.
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
unsafe fn sitio_e_tabela_de_acoes(lsda: *const u8, ip_relativo: u64) -> (u64, u64, *const u8) {
    if lsda.is_null() {
        return (0, 0, std::ptr::null());
    }
    let mut p = lsda;
    // SAFETY: a LSDA tem pelo menos o cabeçalho de três bytes.
    unsafe {
        if *p != 0xff {
            abortar_desenrolamento("LSDA com LPStart explícito");
        }
        p = p.add(1);
        let codificacao_dos_tipos = *p;
        p = p.add(1);
        if codificacao_dos_tipos != 0xff {
            let _ = ler_uleb128(&mut p);
        }
        // A codificação dos call sites: `uleb128` (0x01, COFF e ELF) ou
        // `udata4` (0x03, que o LLVM pode usar no Mach-O; não conferido).
        let codificacao = *p;
        if codificacao != 0x01 && codificacao != 0x03 {
            abortar_desenrolamento("LSDA com call sites em codificação inesperada");
        }
        p = p.add(1);
        let ler_campo = |p: &mut *const u8| -> u64 {
            if codificacao == 0x01 {
                ler_uleb128(p)
            } else {
                let valor = u32::from_le_bytes([**p, *p.add(1), *p.add(2), *p.add(3)]);
                *p = p.add(4);
                u64::from(valor)
            }
        };
        let tamanho = ler_uleb128(&mut p);
        let fim = p.add(tamanho as usize);
        while p < fim {
            let inicio = ler_campo(&mut p);
            let comprimento = ler_campo(&mut p);
            let pouso = ler_campo(&mut p);
            let acao = ler_uleb128(&mut p);
            if ip_relativo < inicio {
                break;
            }
            if ip_relativo - inicio < comprimento {
                return (pouso, acao, fim);
            }
        }
    }
    (0, 0, std::ptr::null())
}

/// Consulta legada: preserva o índice sem interpretar a cadeia.
#[cfg(any(all(windows, target_arch = "x86_64"), unix))]
unsafe fn sitio_da_lsda(lsda: *const u8, ip_relativo: u64) -> (u64, u64) {
    // SAFETY: mesma LSDA bem formada do chamador.
    let (pouso, acao, _) = unsafe { sitio_e_tabela_de_acoes(lsda, ip_relativo) };
    (pouso, acao)
}

/// Lê SLEB128, incluindo deslocamentos negativos entre ações.
#[cfg(any(unix, test))]
unsafe fn ler_sleb128(p: &mut *const u8) -> i64 {
    let mut bits = 0_u64;
    let mut deslocamento = 0_u32;
    loop {
        // SAFETY: o chamador fornece uma codificação válida e completa.
        let byte = unsafe { **p };
        *p = unsafe { p.add(1) };
        if deslocamento < 64 { bits |= u64::from(byte & 0x7f) << deslocamento; }
        deslocamento += 7;
        if byte & 0x80 == 0 {
            if deslocamento < 64 && byte & 0x40 != 0 { bits |= u64::MAX << deslocamento; }
            return bits as i64;
        }
    }
}

/// Descobre cleanup e o primeiro seletor catch-all numa cadeia válida.
/// O índice do sítio é deslocamento em bytes, não o seletor do catch.
#[cfg(any(unix, test))]
unsafe fn acoes_itanium(tabela: *const u8, indice: u64) -> (bool, i32) {
    if indice == 0 { return (true, 0); }
    let mut limpeza = false;
    let mut seletor = 0;
    // SAFETY: índice e elos pertencem à LSDA válida fornecida pelo sistema.
    unsafe {
        let mut p = tabela.add((indice - 1) as usize);
        loop {
            let filtro = ler_sleb128(&mut p);
            if filtro == 0 { limpeza = true; }
            else if filtro > 0 && seletor == 0 {
                seletor = i32::try_from(filtro).unwrap_or_else(|_| abortar_desenrolamento("seletor Itanium excede i32"));
            } else if filtro < 0 {
                abortar_desenrolamento("filtros negativos não suportados no protocolo cleanup");
            }
            // O elo é relativo ao início do campo de deslocamento, e pode
            // apontar para trás; não é relativo ao fim do SLEB128.
            let origem = p;
            let proximo = ler_sleb128(&mut p);
            if proximo == 0 { return (limpeza, seletor); }
            p = origem.offset(proximo as isize);
        }
    }
}

/// `EXCEPTION_RECORD` (x86-64).
#[cfg(all(windows, target_arch = "x86_64"))]
#[allow(dead_code)]
#[repr(C)]
struct RegistroDeExcecaoSeh {
    codigo: u32,
    bandeiras: u32,
    registro: *mut RegistroDeExcecaoSeh,
    endereco: *mut u8,
    n_parametros: u32,
    parametros: [usize; 15],
}

/// `RUNTIME_FUNCTION` (x86-64).
#[cfg(all(windows, target_arch = "x86_64"))]
#[allow(dead_code)]
#[repr(C)]
struct FuncaoDeRuntimeSeh {
    inicio: u32,
    fim: u32,
    desenrolamento: u32,
}

/// `DISPATCHER_CONTEXT` (x86-64).
#[cfg(all(windows, target_arch = "x86_64"))]
#[allow(dead_code)]
#[repr(C)]
struct ContextoDeDespachoSeh {
    control_pc: u64,
    image_base: u64,
    function_entry: *const FuncaoDeRuntimeSeh,
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
        exception_record: *mut RegistroDeExcecaoSeh,
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
unsafe fn personalidade_seh(registro: *mut u8, quadro: *mut u8, despacho: *mut u8) -> i32 {
    /// `ExceptionContinueSearch`.
    const CONTINUAR_A_BUSCA: i32 = 1;
    let registro = registro.cast::<RegistroDeExcecaoSeh>();
    let despacho = despacho.cast::<ContextoDeDespachoSeh>();
    // SAFETY: os ponteiros são os do despachante; a LSDA é a da função cujo
    // quadro está sendo examinado.
    unsafe {
        if (*registro).codigo != CODIGO_DE_DESENROLAMENTO_DART {
            return CONTINUAR_A_BUSCA;
        }
        if (*registro).bandeiras & 0x6 != 0 {
            return CONTINUAR_A_BUSCA;
        }
        if (*despacho).function_entry.is_null() {
            return CONTINUAR_A_BUSCA;
        }
        let inicio = (*despacho).image_base + u64::from((*(*despacho).function_entry).inicio);
        // O endereço de retorno é o da instrução seguinte à chamada, que pode
        // já ser de outra faixa: o ponto da chamada é um byte antes.
        let ip_relativo = (*despacho).control_pc.wrapping_sub(1).wrapping_sub(inicio);
        let (pouso, _acao) = sitio_da_lsda((*despacho).handler_data, ip_relativo);
        if pouso == 0 {
            return CONTINUAR_A_BUSCA;
        }
        if pouso_atravessa_rust(quadro as usize) {
            abortar_desenrolamento("uma exceção Dart atravessou quadros Rust (chamada do runtime a código Dart sem porta)");
        }
        RtlUnwindEx(
            quadro,
            (inicio + pouso) as *mut u8,
            registro,
            std::ptr::null_mut(),
            (*despacho).context_record,
            (*despacho).history_table,
        );
    }
    abortar_desenrolamento("RtlUnwindEx voltou")
}

/// A personalidade das funções geradas com `--excecoes=tabelas`
/// (`personality ptr @dartforge_personalidade` no IR). Só o Windows x86-64
/// tem o modo; nos outros alvos o emissor o recusa e ninguém referencia este
/// símbolo.
///
/// # Safety
/// Só o despachante de exceções do sistema a chama.
#[cfg(not(unix))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_personalidade(registro: *mut u8, quadro: *mut u8, contexto: *mut u8, despacho: *mut u8) -> i32 {
    let _ = contexto;
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        // SAFETY: os argumentos são os do despachante do sistema.
        unsafe { personalidade_seh(registro, quadro, despacho) }
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        let _ = (registro, quadro, despacho);
        abortar_desenrolamento("exceções por tabelas não existem neste alvo")
    }
}

// ---------------------------------------------------------------------------
// Os alvos Itanium (Linux, macOS; docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
// §13.11 e Etapa 4). Escrito sem compilar nem executar (2026-10-05).

/// As funções do desenrolador do sistema (`libgcc_s` ou `libunwind`, que a
/// biblioteca padrão do Rust já liga).
#[cfg(unix)]
unsafe extern "C" {
    fn _Unwind_RaiseException(objeto: *mut ObjetoDeDesenrolamento) -> i32;
    fn _Unwind_GetIPInfo(contexto: *mut u8, antes_da_instrucao: *mut i32) -> usize;
    fn _Unwind_GetRegionStart(contexto: *mut u8) -> usize;
    fn _Unwind_GetLanguageSpecificData(contexto: *mut u8) -> *const u8;
    fn _Unwind_SetGR(contexto: *mut u8, registrador: i32, valor: usize);
    fn _Unwind_SetIP(contexto: *mut u8, valor: usize);
}

/// A classe das exceções deste runtime (`exception_class`): os oito
/// caracteres `DARTFRGE`.
#[cfg(unix)]
const CLASSE_DE_DESENROLAMENTO_DART: u64 = u64::from_be_bytes(*b"DARTFRGE");

/// `_Unwind_Exception`: a classe, a limpeza e a área privada do
/// desenrolador (duas palavras no x86-64 e no aarch64; sobra espaço de
/// propósito). O alinhamento é o máximo do alvo, como no cabeçalho C.
#[cfg(unix)]
#[repr(C, align(16))]
struct ObjetoDeDesenrolamento {
    classe: u64,
    limpeza: Option<unsafe extern "C" fn(i32, *mut ObjetoDeDesenrolamento)>,
    privado: [u64; 6],
}

#[cfg(unix)]
thread_local! {
    /// O objeto de desenrolamento da thread. Um só basta: a exceção Dart é
    /// a pendência do runtime, e todo pouso do código gerado pega tudo, de
    /// modo que nunca há dois desenrolamentos em curso na mesma thread.
    static OBJETO_DE_DESENROLAMENTO: std::cell::UnsafeCell<ObjetoDeDesenrolamento> =
        const { std::cell::UnsafeCell::new(ObjetoDeDesenrolamento { classe: CLASSE_DE_DESENROLAMENTO_DART, limpeza: None, privado: [0; 6] }) };
}

/// O objeto de exceção da thread, pronto para o desenrolamento das
/// exceções por tabelas nos alvos Itanium: `@df.lancar` (gerado em IR) o
/// entrega ele mesmo ao `_Unwind_RaiseException`, sem quadro Rust no
/// caminho até o pouso (a guarda de abortar de uma função `extern "C"` na
/// variante `panic=unwind` do runtime pararia a busca). Não aloca nem lança.
///
/// # Safety
/// Só o código gerado a chama, com a exceção Dart já pendente.
#[cfg(unix)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_objeto_de_desenrolamento() -> *mut u8 {
    let objeto = OBJETO_DE_DESENROLAMENTO.with(|o| o.get());
    // SAFETY: o objeto é da thread e não há outro desenrolamento em curso;
    // o desenrolador só usa a área privada dele.
    unsafe {
        (*objeto).classe = CLASSE_DE_DESENROLAMENTO_DART;
        (*objeto).limpeza = None;
        (*objeto).privado = [0; 6];
    }
    objeto.cast()
}

/// O `_Unwind_RaiseException` de `@df.lancar` voltou: nenhum pouso pegou (a
/// entrada do programa e as portas têm pouso, então é defeito). Encerra.
#[cfg(unix)]
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_desenrolamento_falhou(motivo: i32) -> ! {
    abortar_desenrolamento(&format!("_Unwind_RaiseException voltou com {motivo} (nenhum quadro pegou)"))
}

/// A personalidade Itanium das funções com pouso: a mesma LSDA e a mesma
/// regra da SEH (todo pouso pega tudo), nas duas fases do desenrolador.
///
/// # Safety
/// Só o desenrolador do sistema a chama.
#[cfg(unix)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_personalidade(versao: i32, acoes: i32, classe: u64, _objeto: *mut u8, contexto: *mut u8) -> i32 {
    // SAFETY: contrato do despachante, preservando o protocolo legado.
    unsafe { personalidade_itanium(versao, acoes, classe, _objeto, contexto, false) }
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
    versao: i32, acoes: i32, classe: u64, objeto: *mut u8, contexto: *mut u8,
) -> i32 {
    // SAFETY: contrato do despachante para sítios do protocolo de cleanup.
    unsafe { personalidade_itanium(versao, acoes, classe, objeto, contexto, true) }
}

/// Decisão das fases, independente das APIs e registradores do alvo.
#[cfg(any(unix, test))]
fn decisao_itanium(versao: i32, acoes: i32, nossa: bool, pouso: u64, limpeza: bool, seletor: i32, cleanup: bool) -> i32 {
    const FATAL_NA_FASE_1: i32 = 3;
    const TRATADOR_ACHADO: i32 = 6;
    const INSTALAR_CONTEXTO: i32 = 7;
    const CONTINUAR: i32 = 8;
    if versao != 1 { return FATAL_NA_FASE_1; }
    if pouso == 0 { return CONTINUAR; }
    let busca = acoes & 1 != 0;
    let unwind = acoes & 2 != 0;
    let quadro_tratador = acoes & 4 != 0;
    let forcado = acoes & 8 != 0;
    if !cleanup {
        if !nossa { return CONTINUAR; }
        if busca { return TRATADOR_ACHADO; }
        return if forcado { CONTINUAR } else { INSTALAR_CONTEXTO };
    }
    if busca {
        return if nossa && !forcado && seletor > 0 { TRATADOR_ACHADO } else { CONTINUAR };
    }
    if unwind && (limpeza || (nossa && !forcado && quadro_tratador && seletor > 0)) {
        INSTALAR_CONTEXTO
    } else {
        CONTINUAR
    }
}

#[cfg(unix)]
unsafe fn personalidade_itanium(
    versao: i32, acoes: i32, classe: u64, objeto: *mut u8, contexto: *mut u8, cleanup: bool,
) -> i32 {
    // Valida versão/classe antes de acessar o contexto, como no protocolo
    // legado. Cleanup também precisa acompanhar exceções estrangeiras.
    if versao != 1 { return 3; }
    let nossa = classe == CLASSE_DE_DESENROLAMENTO_DART;
    if !cleanup && !nossa { return 8; }
    // SAFETY: contexto e LSDA entregues pelo desenrolador do sistema.
    unsafe {
        let mut antes: i32 = 0;
        let ip = _Unwind_GetIPInfo(contexto, &mut antes) as u64;
        let ip = if antes == 0 { ip.wrapping_sub(1) } else { ip };
        let inicio = _Unwind_GetRegionStart(contexto) as u64;
        let (pouso, acao, tabela) = sitio_e_tabela_de_acoes(_Unwind_GetLanguageSpecificData(contexto), ip.wrapping_sub(inicio));
        let (limpeza, seletor) = if cleanup && pouso != 0 {
            acoes_itanium(tabela, acao)
        } else { (acao == 0, if acao != 0 { 1 } else { 0 }) };
        let decisao = decisao_itanium(versao, acoes, nossa, pouso, limpeza, seletor, cleanup);
        if decisao != 7 { return decisao; }
        // Cleanup intermediário ou estrangeiro usa seletor zero. Um pouso
        // misto escolhido como handler também executa cleanup, com o seletor
        // do catch, antes de desviar para o tratador ou retomar a exceção.
        let trata = nossa && acoes & 4 != 0 && acoes & 8 == 0 && seletor > 0;
        _Unwind_SetGR(contexto, 0, if cleanup { objeto as usize } else { 0 });
        _Unwind_SetGR(contexto, 1, if cleanup && trata { seletor as usize } else { 0 });
        _Unwind_SetIP(contexto, (inicio + pouso) as usize);
        decisao
    }
}

#[cfg(all(test, any(all(windows, target_arch = "x86_64"), unix)))]
mod testes_excecoes_tabelas {
    use super::*;

    #[test]
    fn cadeia_de_acoes_separa_indice_seletor_e_cleanup() {
        // SAFETY: cadeias completas em arrays locais, com elos válidos.
        unsafe {
            assert_eq!(acoes_itanium(std::ptr::null(), 0), (true, 0));
            assert_eq!(acoes_itanium([0, 0].as_ptr(), 1), (true, 0));
            assert_eq!(acoes_itanium([2, 0].as_ptr(), 1), (false, 2));
            assert_eq!(acoes_itanium([0, 1, 1, 0].as_ptr(), 1), (true, 1));
            // O registro no índice 3 volta três bytes desde seu campo elo.
            assert_eq!(acoes_itanium([0, 0, 1, 0x7d].as_ptr(), 3), (true, 1));
        }
    }

    #[test]
    fn sleb128_preserva_sinal_e_extremos() {
        for (bytes, esperado) in [
            (vec![0x7f], -1), (vec![0x7d], -3), (vec![0x80, 0x01], 128),
            (vec![0x80, 0x7f], -128),
            (vec![0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00], i64::MAX),
            (vec![0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x7f], i64::MIN),
        ] {
            let mut p = bytes.as_ptr();
            // SAFETY: SLEB128 completo no vetor.
            assert_eq!(unsafe { ler_sleb128(&mut p) }, esperado);
            assert_eq!(p, unsafe { bytes.as_ptr().add(bytes.len()) });
        }
    }

    #[test]
    fn pouso_misto_limpa_estrangeira_sem_capturar_e_seleciona_catch_dart() {
        assert_eq!(decisao_itanium(1, 1, false, 7, true, 1, true), 8);
        assert_eq!(decisao_itanium(1, 2, false, 7, true, 1, true), 7);
        assert_eq!(decisao_itanium(1, 2 | 8, false, 7, true, 1, true), 7);
        assert_eq!(decisao_itanium(1, 1, true, 7, true, 1, true), 6);
        assert_eq!(decisao_itanium(1, 2 | 4, true, 7, true, 1, true), 7);
    }

    #[test]
    fn cleanup_nao_para_busca_e_instala_na_fase_de_unwind() {
        for nossa in [false, true] {
            for forcado in [0, 8] {
                assert_eq!(decisao_itanium(1, 1 | forcado, nossa, 7, true, 0, true), 8);
                assert_eq!(decisao_itanium(1, 2 | forcado, nossa, 7, true, 0, true), 7);
                assert_eq!(decisao_itanium(1, 0 | forcado, nossa, 7, true, 0, true), 8);
            }
        }
        assert_eq!(decisao_itanium(1, 2, true, 0, true, 0, true), 8);
        assert_eq!(decisao_itanium(0, 1, true, 7, true, 0, true), 3);
    }

    #[test]
    fn catch_all_so_instala_no_quadro_escolhido_para_excecao_dart() {
        assert_eq!(decisao_itanium(1, 1, true, 7, false, 1, true), 6);
        assert_eq!(decisao_itanium(1, 2, true, 7, false, 1, true), 8);
        assert_eq!(decisao_itanium(1, 2 | 4, true, 7, false, 1, true), 7);
        for acoes in [1, 2, 2 | 4, 2 | 8, 2 | 4 | 8] {
            assert_eq!(decisao_itanium(1, acoes, false, 7, false, 1, true), 8);
        }
        assert_eq!(decisao_itanium(1, 2 | 4 | 8, true, 7, false, 1, true), 8);
    }

    #[test]
    fn protocolo_legado_preserva_pousos_de_statepoint() {
        for acao in [0, 1] {
            assert_eq!(decisao_itanium(1, 1, true, 7, acao == 0, acao as i32, false), 6);
            assert_eq!(decisao_itanium(1, 2, true, 7, acao == 0, acao as i32, false), 7);
            assert_eq!(decisao_itanium(1, 2 | 8, true, 7, acao == 0, acao as i32, false), 8);
            assert_eq!(decisao_itanium(1, 2, false, 7, acao == 0, acao as i32, false), 8);
        }
    }

    /// A LSDA do §13.10 (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md): cabeçalho
    /// com tabela de tipos (`catch ptr null`), três faixas — sem pouso, com
    /// pouso em 0x45, sem pouso —, a tabela de ações e a de tipos.
    const COM_TIPOS: [u8; 28] = [
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
    fn pouso_pela_faixa_do_ponto_da_chamada() {
        let p = COM_TIPOS.as_ptr();
        // SAFETY: uma LSDA bem formada, inteira no vetor.
        unsafe {
            assert_eq!(sitio_da_lsda(p, 0x00), (0, 0));
            assert_eq!(sitio_da_lsda(p, 0x0f), (0, 0));
            assert_eq!(sitio_da_lsda(p, 0x10), (0x45, 1));
            assert_eq!(sitio_da_lsda(p, 0x2f), (0x45, 1));
            assert_eq!(sitio_da_lsda(p, 0x30), (0, 0));
            assert_eq!(sitio_da_lsda(p, 0x7f), (0, 0));
            // Depois da última faixa: sem pouso.
            assert_eq!(sitio_da_lsda(p, 0x80), (0, 0));
            assert_eq!(sitio_da_lsda(p, 0x1000), (0, 0));
        }
    }

    #[test]
    fn sem_tabela_de_tipos_e_com_uleb128_de_dois_bytes() {
        // `cleanup` puro: o segundo byte é 0xff e não há a distância. Uma
        // faixa com buraco antes ([0x80, 0x180), pouso 0x200), em uleb128
        // de dois bytes.
        let lsda: [u8; 10] = [0xff, 0xff, 0x01, 6, 0x80, 0x01, 0x80, 0x02, 0x80, 0x04];
        // A ação da faixa vem depois; o vetor termina nela.
        let mut completa = lsda.to_vec();
        completa.push(0x00);
        completa[3] = 7;
        let p = completa.as_ptr();
        // SAFETY: uma LSDA bem formada, inteira no vetor.
        unsafe {
            assert_eq!(sitio_da_lsda(p, 0x00), (0, 0));
            assert_eq!(sitio_da_lsda(p, 0x7f), (0, 0));
            assert_eq!(sitio_da_lsda(p, 0x80), (0x200, 0));
            assert_eq!(sitio_da_lsda(p, 0x17f), (0x200, 0));
            assert_eq!(sitio_da_lsda(p, 0x180), (0, 0));
        }
    }

    #[test]
    fn acao_uleb128_independe_da_codificacao_dos_enderecos() {
        // Endereços udata4 (Mach-O): a ação permanece uleb128 e pode ter
        // mais de um byte. É um índice cru, não um seletor de catch.
        let mut lsda = vec![0xff, 0xff, 0x03, 14];
        for campo in [0x10_u32, 0x20, 0x200] {
            lsda.extend(campo.to_le_bytes());
        }
        lsda.extend([0x81, 0x01]);
        // SAFETY: cabeçalho e faixa completos, inteiros no vetor.
        unsafe {
            assert_eq!(sitio_da_lsda(lsda.as_ptr(), 0x0f), (0, 0));
            assert_eq!(sitio_da_lsda(lsda.as_ptr(), 0x10), (0x200, 129));
            assert_eq!(sitio_da_lsda(lsda.as_ptr(), 0x2f), (0x200, 129));
            assert_eq!(sitio_da_lsda(lsda.as_ptr(), 0x30), (0, 0));
        }
    }

    #[test]
    fn faixa_no_limite_do_endereco_nao_soma_comprimento() {
        fn uleb(mut valor: u64, bytes: &mut Vec<u8>) {
            loop {
                let byte = (valor & 0x7f) as u8;
                valor >>= 7;
                bytes.push(byte | if valor == 0 { 0 } else { 0x80 });
                if valor == 0 { break; }
            }
        }
        let mut faixa = Vec::new();
        for campo in [u64::MAX - 1, 2, 7, 0] {
            uleb(campo, &mut faixa);
        }
        let mut lsda = vec![0xff, 0xff, 0x01, faixa.len() as u8];
        lsda.extend(faixa);
        // SAFETY: faixa completa em uleb128; o comprimento conceitual
        // termina após o maior endereço sem exigir soma em u64.
        unsafe {
            assert_eq!(sitio_da_lsda(lsda.as_ptr(), u64::MAX - 2), (0, 0));
            assert_eq!(sitio_da_lsda(lsda.as_ptr(), u64::MAX - 1), (7, 0));
            assert_eq!(sitio_da_lsda(lsda.as_ptr(), u64::MAX), (7, 0));
        }
    }

    #[test]
    fn lsda_ausente_nao_tem_pouso() {
        // SAFETY: o ponteiro nulo é tratado antes de qualquer leitura.
        assert_eq!(unsafe { sitio_da_lsda(std::ptr::null(), 0x10) }, (0, 0));
    }

    #[test]
    fn sem_porta_registrada_a_chamada_e_direta() {
        extern "C" fn soma(a: i64, b: i64) -> i64 {
            a + b
        }
        extern "C" fn sete() -> i64 {
            7
        }
        // Nenhum teste registra portas: o modo de sempre.
        assert_eq!(dart_r2(soma as usize, 40, 2), 42);
        assert_eq!(dart_r0(sete as usize), 7);
    }
}
