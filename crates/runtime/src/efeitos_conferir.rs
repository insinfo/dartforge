// A conferência da tabela de efeitos das externs (`crates/runtime/efeitos.tsv`,
// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.8).
//
// Com `DARTFORGE_EFEITOS=conferir` na compilação, o código gerado chama
// `dartforge_efeitos_antes(nome, marcas)` antes e `dartforge_efeitos_depois()`
// depois de cada extern que a tabela marca `coleta = 0` (bit 0 das marcas) ou
// `lanca = 0` (bit 1). Aqui a marca é posta à prova:
//
// * `coleta = 0`: uma coleta enquanto a extern roda encerra o processo
//   (`heap::proibir_coleta`), dizendo o nome dela;
// * `lanca = 0`: se a extern entrou sem exceção pendente e saiu com uma, o
//   processo é encerrado, dizendo o nome dela.
//
// A marca otimista errada é o defeito mais perigoso do emissor (uma raiz que
// falta, uma exceção que ninguém confere): só passa de 1 para 0 depois de o
// corpus rodar assim, de preferência com `--gc-stress`. Sem a variável na
// compilação, nada daqui é chamado.

thread_local! {
    /// As externs conferidas em execução nesta thread: (nome, marcas, a
    /// pendência na entrada).
    static EXTERNS_EM_CONFERENCIA: std::cell::RefCell<Vec<(String, i64, u8)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// A entrada de uma extern conferida.
///
/// # Safety
/// `nome` aponta para `len` bytes (o nome da extern, uma constante do módulo).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_efeitos_antes(nome: *const u8, len: i64, marcas: i64) {
    // SAFETY: garantido por quem chama.
    let bytes = unsafe { std::slice::from_raw_parts(nome, usize::try_from(len).unwrap_or(0)) };
    let nome = String::from_utf8_lossy(bytes).into_owned();
    let pendente = dartforge_exception_pending();
    if marcas & 1 != 0 {
        crate::heap::proibir_coleta(nome.clone());
    }
    EXTERNS_EM_CONFERENCIA.with(|p| p.borrow_mut().push((nome, marcas, pendente)));
}

/// A saída da extern conferida mais recente.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_efeitos_depois() {
    let Some((nome, marcas, pendente_antes)) = EXTERNS_EM_CONFERENCIA.with(|p| p.borrow_mut().pop()) else {
        return;
    };
    if marcas & 1 != 0 {
        crate::heap::permitir_coleta();
    }
    if marcas & 2 != 0 && pendente_antes == 0 && dartforge_exception_pending() != 0 {
        eprintln!("dartforge: a extern {nome} está marcada lanca = 0 em efeitos.tsv e deixou uma exceção pendente");
        std::process::abort();
    }
}
