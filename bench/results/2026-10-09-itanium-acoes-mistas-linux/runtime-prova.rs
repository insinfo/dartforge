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

use std::sync::atomic::{AtomicPtr, AtomicUsize, AtomicBool, Ordering};
static ORIGINAL: AtomicPtr<u8> = AtomicPtr::new(std::ptr::null_mut());
static LIMPEZAS: AtomicUsize = AtomicUsize::new(0);
static INVALIDO: AtomicBool = AtomicBool::new(false);
thread_local! {
    static FIXTURE: std::cell::UnsafeCell<ObjetoDeDesenrolamento> = const {
        std::cell::UnsafeCell::new(ObjetoDeDesenrolamento {
            classe: CLASSE_DE_DESENROLAMENTO_DART, limpeza: None, privado: [0; 6]
        })
    };
}
#[unsafe(no_mangle)]
pub extern "C" fn fixture_objeto(estrangeiro: i32) -> *mut u8 {
    let objeto = FIXTURE.with(|o| o.get().cast::<u8>());
    // Objeto da fixture, ainda sem unwind em curso, com classe própria ou estrangeira.
    unsafe { (*(objeto.cast::<ObjetoDeDesenrolamento>())).classe =
        if estrangeiro == 0 { CLASSE_DE_DESENROLAMENTO_DART } else { 0x464f524549474e31 }; }
    ORIGINAL.store(objeto, Ordering::SeqCst);
    objeto
}
#[unsafe(no_mangle)]
pub extern "C" fn fixture_cleanup(objeto: *mut u8, seletor: i32, id: i32, esperado: i32) {
    if objeto != ORIGINAL.load(Ordering::SeqCst) || seletor != esperado {
        INVALIDO.store(true, Ordering::SeqCst);
    }
    let passo = LIMPEZAS.fetch_add(1, Ordering::SeqCst);
    if (passo == 0 && id != 1) || (passo == 1 && id != 2) || passo > 1 {
        INVALIDO.store(true, Ordering::SeqCst);
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn fixture_resultado(objeto: *mut u8, seletor: i32) -> i32 {
    if INVALIDO.load(Ordering::SeqCst) { return 10; }
    if objeto != ORIGINAL.load(Ordering::SeqCst) { return 11; }
    if seletor != 1 { return 12; }
    if LIMPEZAS.load(Ordering::SeqCst) != 2 { return 13; }
    0
}
