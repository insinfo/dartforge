// Runtime nativo: o `dart:io` da VM — arquivos (`runtime/bin/file.cc`,
// `file_linux.cc`, `file_macos.cc`, `file_win.cc`), o campo nativo das
// `NativeFieldWrapperClass1` e o que os outros fragmentos `io_*` usam:
// erros do sistema, caminhos e os valores Dart que os natives devolvem.
//
// Os natives seguem o contrato da VM ao pé da letra: o que a VM devolve em
// `Dart_SetReturnValue` (um `int`, `bool`, `String`, `Uint8List` ou um
// `OSError`) é o que o Dart dos patches da VM (`file_patch.dart`,
// `directory_patch.dart`…) espera, sem sobreposição. Os parâmetros chegam
// na representação da HIR (`int` → `i64`, `bool` → `u8`, o resto → `Ref`),
// e o `_Namespace` é ignorado: o nativo só tem o namespace padrão (o
// diretório corrente do processo), que é o único que o embedder `dart`
// cria fora do Fuchsia.
//
// Um arquivo aberto é um [`ArquivoNativo`] com contagem de referências,
// como o `File` da VM (`ReferenceCounted<File>`): o objeto Dart guarda o
// ponteiro no campo nativo e solta a referência dele quando é fechado ou
// coletado (o finalizador de `Heap::finalizaveis`); `getPointer` retém uma
// referência para o IOService, que a solta ao fim do pedido.

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering as OrdemAtomica};

// ---------------------------------------------------------------------------
// Erros do sistema.

/// Os códigos de erro que os natives atribuem por conta própria (o
/// `SetErrno` de `file_linux.cc`): errno no Unix, `GetLastError` no Windows.
#[cfg(unix)]
mod codigo_do_so {
    pub const NAO_EXISTE: i32 = 2; // ENOENT
    pub const E_DIRETORIO: i32 = 21; // EISDIR
    pub const INVALIDO: i32 = 22; // EINVAL
    pub const NAO_E_DIRETORIO: i32 = 20; // ENOTDIR
    pub const JA_EXISTE: i32 = 17; // EEXIST
}
#[cfg(windows)]
mod codigo_do_so {
    pub const NAO_EXISTE: i32 = 2; // ERROR_FILE_NOT_FOUND
    pub const E_DIRETORIO: i32 = 5; // ERROR_ACCESS_DENIED
    pub const INVALIDO: i32 = 87; // ERROR_INVALID_PARAMETER
    pub const NAO_E_DIRETORIO: i32 = 267; // ERROR_DIRECTORY
    pub const JA_EXISTE: i32 = 183; // ERROR_ALREADY_EXISTS
}

/// A mensagem do sistema para o código `c`, como a VM a monta.
///
/// No Windows é o `FormatMessageIntoBuffer` da VM (`bin/utils_win.cc`): o
/// texto do `FormatMessageW` como vem, com o CRLF final (o `std` o tira), e
/// `OS Error N` quando o sistema não tem mensagem.
#[cfg(windows)]
fn mensagem_do_sistema(c: i32) -> String {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn FormatMessageW(
            flags: u32,
            origem: *const std::ffi::c_void,
            id: u32,
            idioma: u32,
            buffer: *mut u16,
            tamanho: u32,
            argumentos: *const std::ffi::c_void,
        ) -> u32;
    }
    const FORMAT_MESSAGE_IGNORE_INSERTS: u32 = 0x0000_0200;
    const FORMAT_MESSAGE_FROM_SYSTEM: u32 = 0x0000_1000;
    // MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT).
    const IDIOMA: u32 = 0x0400;
    let mut buffer = [0u16; 1024];
    // SAFETY: o buffer tem o tamanho informado; sem inserções.
    let n = unsafe {
        FormatMessageW(
            FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
            std::ptr::null(),
            c as u32,
            IDIOMA,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            std::ptr::null(),
        )
    };
    if n == 0 {
        return format!("OS Error {c}");
    }
    String::from_utf16_lossy(&buffer[..n as usize])
}

/// No Unix, o `strerror` (a mensagem do `std` sem o " (os error N)").
#[cfg(not(windows))]
fn mensagem_do_sistema(c: i32) -> String {
    let texto = std::io::Error::from_raw_os_error(c).to_string();
    let sufixo = format!(" (os error {c})");
    texto.strip_suffix(&sufixo).map(str::to_string).unwrap_or(texto)
}

/// Um erro do sistema operacional, como o `OSError` da VM (`bin/utils.h`):
/// o código e a mensagem do sistema (`strerror_r`; `FormatMessage` no
/// Windows).
#[derive(Clone, Debug)]
struct ErroDoSo {
    codigo: i64,
    mensagem: String,
}

impl ErroDoSo {
    /// O erro do código `c` do sistema.
    fn do_codigo(c: i32) -> ErroDoSo {
        ErroDoSo { codigo: i64::from(c), mensagem: mensagem_do_sistema(c) }
    }

    /// O erro de uma operação do `std` (que carrega o código do sistema).
    fn de(e: &std::io::Error) -> ErroDoSo {
        match e.raw_os_error() {
            Some(c) => ErroDoSo::do_codigo(c),
            None => ErroDoSo { codigo: -1, mensagem: e.to_string() },
        }
    }

    /// O `OSError(-1, "Invalid argument", kUnknown)` dos natives da VM.
    fn argumento_invalido() -> ErroDoSo {
        ErroDoSo { codigo: -1, mensagem: "Invalid argument".to_string() }
    }

    /// O `OSError` do Dart (`DartUtils::NewDartOSError`), pela função da
    /// sobreposição de `dart:io`.
    fn para_dart(&self) -> i64 {
        let Some(f) = ajudante("_dartforgeOSError") else {
            panic!("bug do compilador: dart:io sem `_dartforgeOSError` registrado");
        };
        // SAFETY: registrada pela biblioteca `dart:io` da sobreposição com a
        // assinatura (`String`, `int`) → `OSError`.
        let g: extern "C" fn(i64, i64) -> i64 = unsafe { std::mem::transmute(f) };
        let mensagem = alocar_str(&self.mensagem);
        com_raizes(&[mensagem], || g(mensagem, self.codigo))
    }

    /// A resposta de erro do IOService (`CObject::NewOSError`):
    /// `[kOSError, código, mensagem]`.
    fn para_resposta(&self) -> Portavel {
        Portavel::Lista(vec![
            Portavel::Int(RESPOSTA_ERRO_DO_SO),
            Portavel::Int(self.codigo),
            Portavel::Str(self.mensagem.clone()),
        ])
    }
}

impl From<std::io::Error> for ErroDoSo {
    fn from(e: std::io::Error) -> ErroDoSo {
        ErroDoSo::de(&e)
    }
}

type ResultadoIo<T> = Result<T, ErroDoSo>;

// ---------------------------------------------------------------------------
// Valores Dart devolvidos pelos natives.

fn dart_bool(b: bool) -> i64 {
    HEAP.with(|h| h.borrow_mut().caixa_bool(b))
}

fn dart_int(i: i64) -> i64 {
    HEAP.with(|h| h.borrow_mut().caixa_int(i))
}

/// Uma `String` Dart a partir de bytes do sistema: UTF-8, com a sequência
/// inválida trocada por U+FFFD.
fn dart_texto_de_bytes(b: &[u8]) -> i64 {
    alocar_str(&String::from_utf8_lossy(b))
}

/// Uma `Uint8List` (`_Uint8List`) com os bytes.
fn dart_bytes(bytes: Vec<u8>) -> i64 {
    let class_id = cid_registrado(CID_UINT8_LIST).expect("bug do compilador: dart:io sem o SDK da fonte");
    HEAP.with(|h| h.borrow_mut().allocate(Value::TypedData { class_id, tipo: TIPO_UINT8, bytes }))
}

/// Uma `Int64List` (`_Int64List`) com os valores.
fn dart_int64s(valores: &[i64]) -> i64 {
    let class_id = cid_registrado(CID_INT64_LIST).expect("bug do compilador: dart:io sem o SDK da fonte");
    let bytes = valores.iter().flat_map(|v| v.to_ne_bytes()).collect();
    HEAP.with(|h| h.borrow_mut().allocate(Value::TypedData { class_id, tipo: TIPO_INT64, bytes }))
}

/// Uma `_List` de tamanho fixo com os valores (já na posição `Ref`).
fn dart_lista_fixa(valores: &[i64]) -> i64 {
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        let l = h.allocate(Value::List(valores.iter().map(|&v| TaggedValue::reference(v)).collect()));
        h.fixas.insert(l);
        l
    })
}

/// O valor Dart de um resultado: o `Ok` por `f`, o erro como `OSError`.
fn dart_ou_erro<T>(r: ResultadoIo<T>, f: impl FnOnce(T) -> i64) -> i64 {
    match r {
        Ok(v) => f(v),
        Err(e) => e.para_dart(),
    }
}

/// `true` ou o `OSError` — a forma de quase todos os natives de caminho.
fn dart_verdadeiro_ou_erro(r: ResultadoIo<()>) -> i64 {
    dart_ou_erro(r, |()| dart_bool(true))
}

/// `null` ou o `OSError`.
fn dart_nulo_ou_erro(r: ResultadoIo<()>) -> i64 {
    dart_ou_erro(r, |()| 0)
}

/// Lança `StateError(mensagem)` (o `_InternalError` dos natives da VM).
fn lancar_erro_interno(mensagem: &str) {
    let e = allocate_state_error(mensagem);
    com_raizes(&[e], || dartforge_exception_throw(e, 3));
}

// ---------------------------------------------------------------------------
// Caminhos.

/// Os bytes de uma lista tipada (lista interna ou visão).
fn bytes_da_lista_tipada(h: i64) -> Option<Vec<u8>> {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let (interna, desloc, tipo, n) = resolver(&heap, h)?;
        let b = bytes_de(&heap, interna);
        let fim = (desloc + n * tamanho_do_elemento(tipo)).min(b.len());
        Some(b[desloc.min(fim)..fim].to_vec())
    })
}

/// O caminho de um `rawPath` (`Uint8List` UTF-8 terminado em NUL, o
/// `DartUtils::GetNativeTypedDataArgument` da VM): os bytes até o NUL.
fn bytes_do_caminho(raw: i64) -> Vec<u8> {
    let mut b = bytes_da_lista_tipada(raw).unwrap_or_default();
    if let Some(fim) = b.iter().position(|&c| c == 0) {
        b.truncate(fim);
    }
    b
}

/// O caminho do sistema para os bytes (Unix: os bytes; Windows: o UTF-8
/// convertido para UTF-16, como o `Utf8ToWideChar` da VM).
#[cfg(unix)]
fn caminho_de_bytes(b: &[u8]) -> std::path::PathBuf {
    use std::os::unix::ffi::OsStrExt;
    std::path::PathBuf::from(std::ffi::OsStr::from_bytes(b))
}
#[cfg(windows)]
fn caminho_de_bytes(b: &[u8]) -> std::path::PathBuf {
    std::path::PathBuf::from(String::from_utf8_lossy(b).into_owned())
}

/// Os bytes de um caminho do sistema (o inverso de [`caminho_de_bytes`]).
#[cfg(unix)]
fn bytes_de_caminho(p: &std::path::Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    p.as_os_str().as_bytes().to_vec()
}
#[cfg(windows)]
fn bytes_de_caminho(p: &std::path::Path) -> Vec<u8> {
    p.to_string_lossy().into_owned().into_bytes()
}

/// O caminho de um `rawPath` Dart.
fn caminho_raw(raw: i64) -> std::path::PathBuf {
    caminho_de_bytes(&bytes_do_caminho(raw))
}

/// O caminho de uma `String` Dart (`DartUtils::GetNativeStringArgument`:
/// UTF-8).
fn caminho_texto(s: i64) -> std::path::PathBuf {
    caminho_de_bytes(&utf8_de_texto(s))
}

/// O UTF-8 de uma `String` Dart (o substituto solto vira U+FFFD, como no
/// `Dart_StringToUTF8` da VM).
fn utf8_de_texto(s: i64) -> Vec<u8> {
    if s == 0 {
        return Vec::new();
    }
    HEAP.with(|h| h.borrow().texto(s).para_utf8_da_vm())
}

// ---------------------------------------------------------------------------
// O campo nativo de `NativeFieldWrapperClass1` (a sobreposição de
// `dart:nativewrappers` o declara como o primeiro campo do layout).

fn campo_nativo(obj: i64) -> i64 {
    HEAP.with(|h| match h.borrow().get(obj) {
        Value::Object { fields, .. } => fields.first().map_or(0, |f| f.0),
        _ => panic!("bug do compilador: campo nativo de algo que não é objeto"),
    })
}

fn gravar_campo_nativo(obj: i64, valor: i64) {
    HEAP.with(|h| match h.borrow_mut().get_mut(obj) {
        Value::Object { fields, .. } if !fields.is_empty() => fields[0] = (valor, false),
        _ => panic!("bug do compilador: campo nativo de algo que não é objeto"),
    });
}

/// Liga `par` ao objeto: quando o objeto for coletado, `finalizador(par)`
/// (o `Dart_NewFinalizableHandle` da VM).
fn anexar_finalizador(obj: i64, finalizador: fn(usize), par: usize) {
    HEAP.with(|h| h.borrow_mut().finalizaveis.insert(obj, (finalizador, par)));
}

fn remover_finalizador(obj: i64) {
    HEAP.with(|h| h.borrow_mut().finalizaveis.remove(&obj));
}

// ---------------------------------------------------------------------------
// O arquivo aberto.

/// O descritor de um arquivo fechado (`File::kClosedFd`).
const DESCRITOR_FECHADO: i64 = -1;

/// Um arquivo aberto (o `File` de `bin/file.h`): o descritor do sistema
/// (Unix) ou o `HANDLE` (Windows). O isolado e as threads do IOService o
/// compartilham; o Dart dos patches serializa as operações de um mesmo
/// arquivo (`_RandomAccessFile._checkAvailable`).
struct ArquivoNativo {
    descritor: AtomicI64,
}

impl ArquivoNativo {
    /// Cria o arquivo com uma referência e devolve o ponteiro dela.
    fn novo(descritor: i64) -> i64 {
        Arc::into_raw(Arc::new(ArquivoNativo { descritor: AtomicI64::new(descritor) })) as i64
    }

    /// O arquivo do ponteiro `p`, emprestado de quem tem a referência.
    ///
    /// # Safety
    /// `p` veio de [`ArquivoNativo::novo`] e a referência ainda existe.
    unsafe fn de<'a>(p: i64) -> &'a ArquivoNativo {
        // SAFETY: garantido por quem chama.
        unsafe { &*(p as *const ArquivoNativo) }
    }

    /// Mais uma referência (`Retain`).
    fn reter(p: i64) {
        // SAFETY: `p` é um ponteiro vivo de `novo`.
        unsafe { Arc::increment_strong_count(p as *const ArquivoNativo) };
    }

    /// Solta uma referência (`Release`); a última fecha o arquivo.
    fn liberar(p: i64) {
        // SAFETY: `p` é um ponteiro vivo de `novo`, com esta referência.
        unsafe { Arc::decrement_strong_count(p as *const ArquivoNativo) };
    }

    /// Toma a referência de `p` (o `RefCntReleaseScope` da VM: solta ao
    /// sair do escopo).
    ///
    /// # Safety
    /// `p` é um ponteiro de `novo` cuja referência passa a quem chama.
    unsafe fn tomar(p: i64) -> Arc<ArquivoNativo> {
        // SAFETY: garantido por quem chama.
        unsafe { Arc::from_raw(p as *const ArquivoNativo) }
    }

    fn fechado(&self) -> bool {
        self.descritor.load(OrdemAtomica::Acquire) == DESCRITOR_FECHADO
    }

    fn descritor(&self) -> i64 {
        self.descritor.load(OrdemAtomica::Acquire)
    }

    /// Opera no arquivo pelo `std::fs::File`, sem transferir o descritor.
    fn com<R>(&self, f: impl FnOnce(&mut std::fs::File) -> std::io::Result<R>) -> ResultadoIo<R> {
        let d = self.descritor();
        if d == DESCRITOR_FECHADO {
            return Err(ErroDoSo::do_codigo(codigo_do_so::INVALIDO));
        }
        let mut arquivo = std::mem::ManuallyDrop::new(arquivo_do_descritor(d));
        f(&mut arquivo).map_err(ErroDoSo::from)
    }

    /// `File::Read`: uma leitura (repetida só na interrupção por sinal).
    fn ler(&self, destino: &mut [u8]) -> ResultadoIo<usize> {
        use std::io::Read;
        self.com(|f| loop {
            match f.read(destino) {
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                r => return r,
            }
        })
    }

    /// `File::WriteFully`.
    fn escrever_tudo(&self, dados: &[u8]) -> ResultadoIo<()> {
        use std::io::Write;
        if matches!(self.descritor(), 1 | 2) {
            // A saída do `print` passa pelo `stdout` do `std`: o que ele
            // tiver guardado sai antes, para a ordem ser a do programa.
            let _ = std::io::stdout().flush();
        }
        self.com(|f| f.write_all(dados))
    }

    fn posicao(&self) -> ResultadoIo<i64> {
        use std::io::Seek;
        self.com(|f| f.stream_position()).map(|p| p as i64)
    }

    fn mudar_posicao(&self, p: i64) -> ResultadoIo<()> {
        use std::io::Seek;
        self.com(|f| f.seek(std::io::SeekFrom::Start(p as u64))).map(|_| ())
    }

    fn truncar(&self, n: i64) -> ResultadoIo<()> {
        if n < 0 {
            return Err(ErroDoSo::do_codigo(codigo_do_so::INVALIDO));
        }
        self.com(|f| f.set_len(n as u64))
    }

    fn tamanho(&self) -> ResultadoIo<i64> {
        self.com(|f| f.metadata()).map(|m| m.len() as i64)
    }

    /// `File::Flush`: `fsync` (Unix; o `sync_all` do `std` no macOS seria o
    /// `F_FULLFSYNC`, que a VM não usa) ou `FlushFileBuffers` (Windows).
    fn sincronizar(&self) -> ResultadoIo<()> {
        #[cfg(unix)]
        {
            unsafe extern "C" {
                fn fsync(fd: i32) -> i32;
            }
            let d = self.descritor();
            // SAFETY: `d` é um descritor aberto deste arquivo.
            if unsafe { fsync(d as i32) } == -1 {
                return Err(ErroDoSo::de(&std::io::Error::last_os_error()));
            }
            Ok(())
        }
        #[cfg(windows)]
        {
            self.com(|f| f.sync_all())
        }
    }

    /// `File::Lock`: `fcntl(F_SETLK[W])` no Unix, `LockFileEx` no Windows.
    fn travar(&self, tipo: i64, inicio: i64, fim: i64) -> ResultadoIo<()> {
        travar_descritor(self.descritor(), tipo, inicio, fim)
    }

    /// `File::Close`: fecha o descritor. A saída padrão não é fechada: vira
    /// `/dev/null` (a VM faz o mesmo, para o processo não escrever num
    /// descritor que outro `open` reaproveitaria).
    fn fechar(&self) {
        let d = self.descritor.swap(DESCRITOR_FECHADO, OrdemAtomica::AcqRel);
        if d == DESCRITOR_FECHADO {
            return;
        }
        fechar_descritor(d);
    }
}

impl Drop for ArquivoNativo {
    /// O destrutor do `File` da VM: fecha o que ficou aberto, menos a saída
    /// e o erro padrão.
    fn drop(&mut self) {
        let d = self.descritor();
        if d != DESCRITOR_FECHADO && !e_saida_padrao(d) {
            self.fechar();
        }
    }
}

/// O finalizador do objeto Dart dono de um arquivo.
fn liberar_arquivo_do_objeto(par: usize) {
    ArquivoNativo::liberar(par as i64);
}

#[cfg(unix)]
fn arquivo_do_descritor(d: i64) -> std::fs::File {
    use std::os::unix::io::FromRawFd;
    // SAFETY: `d` é um descritor aberto; quem chama não o fecha pelo `File`.
    unsafe { std::fs::File::from_raw_fd(d as i32) }
}
#[cfg(windows)]
fn arquivo_do_descritor(d: i64) -> std::fs::File {
    use std::os::windows::io::FromRawHandle;
    // SAFETY: `d` é um HANDLE aberto; quem chama não o fecha pelo `File`.
    unsafe { std::fs::File::from_raw_handle(d as usize as *mut std::ffi::c_void) }
}

/// O descritor de um `std::fs::File` (que deixa de ser dono dele).
#[cfg(unix)]
fn descritor_de_arquivo(f: std::fs::File) -> i64 {
    use std::os::unix::io::IntoRawFd;
    i64::from(f.into_raw_fd())
}
#[cfg(windows)]
fn descritor_de_arquivo(f: std::fs::File) -> i64 {
    use std::os::windows::io::IntoRawHandle;
    f.into_raw_handle() as usize as i64
}

#[cfg(unix)]
fn e_saida_padrao(d: i64) -> bool {
    d == 1 || d == 2
}
#[cfg(windows)]
fn e_saida_padrao(d: i64) -> bool {
    d == descritor_padrao(1) || d == descritor_padrao(2)
}

#[cfg(unix)]
fn fechar_descritor(d: i64) {
    unsafe extern "C" {
        fn open(caminho: *const std::ffi::c_char, flags: i32, ...) -> i32;
        fn dup2(de: i32, para: i32) -> i32;
        fn close(fd: i32) -> i32;
    }
    if d == 1 {
        use std::io::Write;
        let _ = std::io::stdout().flush();
        // SAFETY: caminho C constante; os descritores são válidos.
        unsafe {
            let nulo = open(c"/dev/null".as_ptr(), 1 /* O_WRONLY */);
            if nulo >= 0 {
                dup2(nulo, 1);
                close(nulo);
            }
        }
        return;
    }
    // SAFETY: `d` é um descritor aberto deste arquivo, fechado uma vez.
    unsafe { close(d as i32) };
}
#[cfg(windows)]
fn fechar_descritor(d: i64) {
    if e_saida_padrao(d) || d == descritor_padrao(0) {
        return;
    }
    drop(arquivo_do_descritor(d));
}

/// O descritor da entrada/saída padrão `fd` (0, 1, 2).
#[cfg(unix)]
fn descritor_padrao(fd: i64) -> i64 {
    fd
}
#[cfg(windows)]
fn descritor_padrao(fd: i64) -> i64 {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(qual: u32) -> *mut std::ffi::c_void;
    }
    // STD_INPUT_HANDLE = -10, STD_OUTPUT_HANDLE = -11, STD_ERROR_HANDLE = -12.
    let qual = (-10 - fd) as i32 as u32;
    // SAFETY: só lê o handle do processo.
    unsafe { GetStdHandle(qual) as usize as i64 }
}

#[cfg(all(unix, target_os = "linux"))]
fn travar_descritor(d: i64, tipo: i64, inicio: i64, fim: i64) -> ResultadoIo<()> {
    #[repr(C)]
    struct Flock {
        l_type: i16,
        l_whence: i16,
        l_start: i64,
        l_len: i64,
        l_pid: i32,
    }
    const F_RDLCK: i16 = 0;
    const F_WRLCK: i16 = 1;
    const F_UNLCK: i16 = 2;
    const F_SETLK: i32 = 6;
    const F_SETLKW: i32 = 7;
    let l_type = match tipo {
        TRAVA_SOLTAR => F_UNLCK,
        TRAVA_COMPARTILHADA | TRAVA_COMPARTILHADA_BLOQUEANTE => F_RDLCK,
        TRAVA_EXCLUSIVA | TRAVA_EXCLUSIVA_BLOQUEANTE => F_WRLCK,
        _ => return Err(ErroDoSo::argumento_invalido()),
    };
    let fl = Flock { l_type, l_whence: 0, l_start: inicio, l_len: if fim == -1 { 0 } else { fim - inicio }, l_pid: 0 };
    let cmd = if matches!(tipo, TRAVA_COMPARTILHADA_BLOQUEANTE | TRAVA_EXCLUSIVA_BLOQUEANTE) { F_SETLKW } else { F_SETLK };
    fcntl_trava(d, cmd, &fl as *const Flock as *const std::ffi::c_void)
}
#[cfg(all(unix, not(target_os = "linux")))]
fn travar_descritor(d: i64, tipo: i64, inicio: i64, fim: i64) -> ResultadoIo<()> {
    #[repr(C)]
    struct Flock {
        l_start: i64,
        l_len: i64,
        l_pid: i32,
        l_type: i16,
        l_whence: i16,
    }
    const F_RDLCK: i16 = 1;
    const F_UNLCK: i16 = 2;
    const F_WRLCK: i16 = 3;
    const F_SETLK: i32 = 8;
    const F_SETLKW: i32 = 9;
    let l_type = match tipo {
        TRAVA_SOLTAR => F_UNLCK,
        TRAVA_COMPARTILHADA | TRAVA_COMPARTILHADA_BLOQUEANTE => F_RDLCK,
        TRAVA_EXCLUSIVA | TRAVA_EXCLUSIVA_BLOQUEANTE => F_WRLCK,
        _ => return Err(ErroDoSo::argumento_invalido()),
    };
    let fl = Flock { l_start: inicio, l_len: if fim == -1 { 0 } else { fim - inicio }, l_pid: 0, l_type, l_whence: 0 };
    let cmd = if matches!(tipo, TRAVA_COMPARTILHADA_BLOQUEANTE | TRAVA_EXCLUSIVA_BLOQUEANTE) { F_SETLKW } else { F_SETLK };
    fcntl_trava(d, cmd, &fl as *const Flock as *const std::ffi::c_void)
}
#[cfg(unix)]
fn fcntl_trava(d: i64, cmd: i32, fl: *const std::ffi::c_void) -> ResultadoIo<()> {
    unsafe extern "C" {
        fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    }
    loop {
        // SAFETY: `fl` aponta para uma `struct flock` válida da plataforma.
        if unsafe { fcntl(d as i32, cmd, fl) } != -1 {
            return Ok(());
        }
        let e = std::io::Error::last_os_error();
        if e.kind() != std::io::ErrorKind::Interrupted {
            return Err(ErroDoSo::de(&e));
        }
    }
}
#[cfg(windows)]
fn travar_descritor(d: i64, tipo: i64, inicio: i64, fim: i64) -> ResultadoIo<()> {
    #[repr(C)]
    struct Overlapped {
        internal: usize,
        internal_high: usize,
        offset: u32,
        offset_high: u32,
        evento: *mut std::ffi::c_void,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LockFileEx(h: *mut std::ffi::c_void, flags: u32, r: u32, baixo: u32, alto: u32, o: *mut Overlapped) -> i32;
        fn UnlockFileEx(h: *mut std::ffi::c_void, r: u32, baixo: u32, alto: u32, o: *mut Overlapped) -> i32;
    }
    const LOCKFILE_FAIL_IMMEDIATELY: u32 = 1;
    const LOCKFILE_EXCLUSIVE_LOCK: u32 = 2;
    let comprimento: u64 = if fim == -1 { u64::MAX } else { (fim - inicio) as u64 };
    let mut o = Overlapped {
        internal: 0,
        internal_high: 0,
        offset: inicio as u64 as u32,
        offset_high: ((inicio as u64) >> 32) as u32,
        evento: std::ptr::null_mut(),
    };
    let h = d as usize as *mut std::ffi::c_void;
    let (baixo, alto) = (comprimento as u32, (comprimento >> 32) as u32);
    // SAFETY: `h` é o HANDLE aberto do arquivo; `o` vive durante a chamada.
    let ok = unsafe {
        match tipo {
            TRAVA_SOLTAR => UnlockFileEx(h, 0, baixo, alto, &mut o),
            TRAVA_COMPARTILHADA => LockFileEx(h, LOCKFILE_FAIL_IMMEDIATELY, 0, baixo, alto, &mut o),
            TRAVA_COMPARTILHADA_BLOQUEANTE => LockFileEx(h, 0, 0, baixo, alto, &mut o),
            TRAVA_EXCLUSIVA => LockFileEx(h, LOCKFILE_FAIL_IMMEDIATELY | LOCKFILE_EXCLUSIVE_LOCK, 0, baixo, alto, &mut o),
            TRAVA_EXCLUSIVA_BLOQUEANTE => LockFileEx(h, LOCKFILE_EXCLUSIVE_LOCK, 0, baixo, alto, &mut o),
            _ => return Err(ErroDoSo::argumento_invalido()),
        }
    };
    if ok == 0 {
        return Err(ErroDoSo::de(&std::io::Error::last_os_error()));
    }
    Ok(())
}

/// `File::LockType`.
const TRAVA_SOLTAR: i64 = 0;
const TRAVA_COMPARTILHADA: i64 = 1;
const TRAVA_EXCLUSIVA: i64 = 2;
const TRAVA_COMPARTILHADA_BLOQUEANTE: i64 = 3;
const TRAVA_EXCLUSIVA_BLOQUEANTE: i64 = 4;

// ---------------------------------------------------------------------------
// As operações sobre caminhos (`File::*` de `file_linux.cc`), partilhadas
// pelos natives síncronos e pelo IOService.

/// `File::Type`.
const TIPO_ARQUIVO: i64 = 0;
const TIPO_DIRETORIO: i64 = 1;
const TIPO_LINK: i64 = 2;
const TIPO_SOQUETE: i64 = 3;
const TIPO_PIPE: i64 = 4;
const TIPO_NAO_EXISTE: i64 = 5;

/// `File::DartFileOpenMode` → (leitura/escrita, só escrita, truncar).
fn modo_de_abertura(modo_dart: i64) -> Option<(bool, bool, bool)> {
    match modo_dart {
        0 => Some((false, false, false)), // read → kRead
        1 => Some((true, false, true)),   // write → kWriteTruncate
        2 => Some((true, false, false)),  // append → kWrite
        3 => Some((false, true, true)),   // writeOnly → kWriteOnlyTruncate
        4 => Some((false, true, false)),  // writeOnlyAppend → kWriteOnly
        _ => None,
    }
}

/// O tipo de uma entrada do sistema de arquivos (`File::Type`).
fn tipo_de_metadados(m: &std::fs::Metadata) -> i64 {
    let t = m.file_type();
    if t.is_dir() {
        return TIPO_DIRETORIO;
    }
    if t.is_file() {
        return TIPO_ARQUIVO;
    }
    if t.is_symlink() {
        return TIPO_LINK;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        if t.is_socket() {
            return TIPO_SOQUETE;
        }
        if t.is_fifo() {
            return TIPO_PIPE;
        }
    }
    TIPO_NAO_EXISTE
}

/// `File::GetType`.
fn tipo_do_caminho(p: &std::path::Path, seguir_links: bool) -> i64 {
    let m = if seguir_links { std::fs::metadata(p) } else { std::fs::symlink_metadata(p) };
    m.map_or(TIPO_NAO_EXISTE, |m| tipo_de_metadados(&m))
}

/// O `SetErrno` de `file_linux.cc`: o erro de um tipo inesperado.
fn erro_do_tipo(tipo: i64) -> ErroDoSo {
    ErroDoSo::do_codigo(match tipo {
        TIPO_DIRETORIO => codigo_do_so::E_DIRETORIO,
        TIPO_NAO_EXISTE => codigo_do_so::NAO_EXISTE,
        _ => codigo_do_so::INVALIDO,
    })
}

/// Arquivo comum, soquete ou pipe: o que `Delete`/`Rename`/`Copy` aceitam.
fn e_tipo_de_arquivo(tipo: i64) -> bool {
    matches!(tipo, TIPO_ARQUIVO | TIPO_SOQUETE | TIPO_PIPE)
}

/// `File::Open`: só arquivos comuns, dispositivos de caractere e pipes; o
/// modo de acréscimo posiciona no fim.
fn abrir_arquivo(p: &std::path::Path, modo_dart: i64) -> ResultadoIo<i64> {
    let Some((escrita, so_escrita, truncar)) = modo_de_abertura(modo_dart) else {
        return Err(ErroDoSo::argumento_invalido());
    };
    if let Ok(m) = std::fs::metadata(p) {
        let t = m.file_type();
        #[cfg(unix)]
        let aceito = {
            use std::os::unix::fs::FileTypeExt;
            t.is_file() || t.is_char_device() || t.is_fifo()
        };
        #[cfg(windows)]
        let aceito = !t.is_dir();
        if !aceito {
            return Err(ErroDoSo::do_codigo(if t.is_dir() { codigo_do_so::E_DIRETORIO } else { codigo_do_so::NAO_EXISTE }));
        }
    }
    let mut o = std::fs::OpenOptions::new();
    if escrita {
        o.read(true).write(true).create(true);
    } else if so_escrita {
        o.write(true).create(true);
    } else {
        o.read(true);
    }
    if truncar {
        o.truncate(true);
    }
    let mut f = o.open(p)?;
    if (escrita || so_escrita) && !truncar {
        use std::io::Seek;
        f.seek(std::io::SeekFrom::End(0))?;
    }
    Ok(descritor_de_arquivo(f))
}

/// `File::Exists`: tudo que não é diretório nem link é arquivo para o Dart.
fn arquivo_existe(p: &std::path::Path) -> bool {
    std::fs::metadata(p).is_ok_and(|m| !m.is_dir() && !m.file_type().is_symlink())
}

/// `File::Create`: cria (ou, sem `exclusivo`, aceita) o arquivo; um
/// diretório ou link no caminho é erro.
fn criar_arquivo(p: &std::path::Path, exclusivo: bool) -> ResultadoIo<()> {
    let mut o = std::fs::OpenOptions::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // `O_RDONLY | O_CREAT [| O_EXCL]`, como a VM: o `std` só aceita
        // `create` com escrita.
        #[cfg(target_os = "linux")]
        const O_CREAT: i32 = 0o100;
        #[cfg(target_os = "linux")]
        const O_EXCL: i32 = 0o200;
        #[cfg(not(target_os = "linux"))]
        const O_CREAT: i32 = 0x200;
        #[cfg(not(target_os = "linux"))]
        const O_EXCL: i32 = 0x800;
        o.read(true).custom_flags(O_CREAT | if exclusivo { O_EXCL } else { 0 });
    }
    #[cfg(windows)]
    {
        o.read(true).write(true);
        if exclusivo {
            o.create_new(true);
        } else {
            o.create(true);
        }
    }
    let f = o.open(p)?;
    let m = f.metadata()?;
    if m.is_dir() {
        return Err(ErroDoSo::do_codigo(codigo_do_so::E_DIRETORIO));
    }
    if m.file_type().is_symlink() {
        return Err(ErroDoSo::do_codigo(codigo_do_so::NAO_EXISTE));
    }
    Ok(())
}

/// `File::CreateLink`.
fn criar_link(nome: &std::path::Path, alvo: &std::path::Path) -> ResultadoIo<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(alvo, nome)?;
    }
    #[cfg(windows)]
    {
        // Como a VM (`file_win.cc`): link de diretório se o alvo, resolvido
        // a partir da pasta do link, é um diretório.
        let base = nome.parent().map(|d| d.join(alvo)).unwrap_or_else(|| alvo.to_path_buf());
        if std::fs::metadata(&base).is_ok_and(|m| m.is_dir()) {
            std::os::windows::fs::symlink_dir(alvo, nome)?;
        } else {
            std::os::windows::fs::symlink_file(alvo, nome)?;
        }
    }
    Ok(())
}

/// `File::Delete`.
fn apagar_arquivo(p: &std::path::Path) -> ResultadoIo<()> {
    let tipo = tipo_do_caminho(p, true);
    if !e_tipo_de_arquivo(tipo) {
        return Err(erro_do_tipo(tipo));
    }
    Ok(std::fs::remove_file(p)?)
}

/// `File::DeleteLink`.
fn apagar_link(p: &std::path::Path) -> ResultadoIo<()> {
    let tipo = tipo_do_caminho(p, false);
    if tipo != TIPO_LINK {
        return Err(erro_do_tipo(tipo));
    }
    #[cfg(windows)]
    if std::fs::metadata(p).is_ok_and(|m| m.is_dir()) {
        return Ok(std::fs::remove_dir(p)?);
    }
    Ok(std::fs::remove_file(p)?)
}

/// `File::Rename`.
fn renomear_arquivo(de: &std::path::Path, para: &std::path::Path) -> ResultadoIo<()> {
    let tipo = tipo_do_caminho(de, true);
    if !e_tipo_de_arquivo(tipo) {
        return Err(erro_do_tipo(tipo));
    }
    Ok(std::fs::rename(de, para)?)
}

/// `File::RenameLink`.
fn renomear_link(de: &std::path::Path, para: &std::path::Path) -> ResultadoIo<()> {
    let tipo = tipo_do_caminho(de, false);
    if tipo != TIPO_LINK {
        return Err(erro_do_tipo(tipo));
    }
    Ok(std::fs::rename(de, para)?)
}

/// `File::Copy`: o destino é criado com o modo da origem; se a cópia
/// falha, o destino parcial é apagado.
fn copiar_arquivo(de: &std::path::Path, para: &std::path::Path) -> ResultadoIo<()> {
    let tipo = tipo_do_caminho(de, true);
    if !e_tipo_de_arquivo(tipo) {
        return Err(erro_do_tipo(tipo));
    }
    let mut origem = std::fs::File::open(de)?;
    let mut o = std::fs::OpenOptions::new();
    o.write(true).truncate(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        o.mode(origem.metadata()?.permissions().mode());
    }
    let mut destino = o.open(para)?;
    // O `std::io::copy` entre dois `File` usa `copy_file_range`/`sendfile`
    // no Linux (o `sendfile64` da VM) e cai para leitura e escrita.
    if let Err(e) = std::io::copy(&mut origem, &mut destino) {
        drop(destino);
        let _ = std::fs::remove_file(para);
        return Err(ErroDoSo::de(&e));
    }
    Ok(())
}

/// O `StatHelper` de `file_linux.cc`: os metadados, com diretório como erro.
fn metadados_de_arquivo(p: &std::path::Path) -> ResultadoIo<std::fs::Metadata> {
    let m = std::fs::metadata(p)?;
    if m.is_dir() {
        return Err(ErroDoSo::do_codigo(codigo_do_so::E_DIRETORIO));
    }
    Ok(m)
}

/// Segundos inteiros de um instante (o `st_mtime` da VM).
fn segundos_de(t: std::time::SystemTime) -> i64 {
    match t.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => -(e.duration().as_secs_f64().ceil() as i64),
    }
}

/// Milissegundos de um instante (o `TimespecToMilliseconds` da VM).
fn milissegundos_de(t: std::time::SystemTime) -> i64 {
    match t.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_millis() as i64,
        Err(e) => -(e.duration().as_millis() as i64),
    }
}

fn instante_de_milissegundos(ms: i64) -> std::time::SystemTime {
    let d = std::time::Duration::from_millis(ms.unsigned_abs());
    if ms >= 0 { std::time::UNIX_EPOCH + d } else { std::time::UNIX_EPOCH - d }
}

/// `File::LastModified` / `LastAccessed`, em milissegundos (a VM multiplica
/// os segundos inteiros por 1000).
fn ultima_modificacao(p: &std::path::Path) -> ResultadoIo<i64> {
    Ok(segundos_de(metadados_de_arquivo(p)?.modified()?) * 1000)
}

fn ultimo_acesso(p: &std::path::Path) -> ResultadoIo<i64> {
    Ok(segundos_de(metadados_de_arquivo(p)?.accessed()?) * 1000)
}

/// `File::SetLastModified` / `SetLastAccessed`: troca um instante e mantém o
/// outro.
fn mudar_instantes(p: &std::path::Path, acesso: Option<i64>, modificacao: Option<i64>) -> ResultadoIo<()> {
    let m = metadados_de_arquivo(p)?;
    let tempos = std::fs::FileTimes::new()
        .set_accessed(acesso.map_or_else(|| m.accessed(), |ms| Ok(instante_de_milissegundos(ms)))?)
        .set_modified(modificacao.map_or_else(|| m.modified(), |ms| Ok(instante_de_milissegundos(ms)))?);
    let mut o = std::fs::OpenOptions::new();
    #[cfg(unix)]
    o.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        o.access_mode(0x100 /* FILE_WRITE_ATTRIBUTES */);
    }
    Ok(o.open(p)?.set_times(tempos)?)
}

/// `File::Stat`: `[tipo, criação, modificação, acesso, modo, tamanho]`.
fn estatisticas(p: &std::path::Path) -> ResultadoIo<[i64; 6]> {
    let m = std::fs::metadata(p)?;
    let tipo = tipo_de_metadados(&m);
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let ms = |s: i64, ns: i64| s * 1000 + ns / 1_000_000;
        Ok([
            tipo,
            ms(m.ctime(), m.ctime_nsec()),
            ms(m.mtime(), m.mtime_nsec()),
            ms(m.atime(), m.atime_nsec()),
            i64::from(m.mode()),
            m.size() as i64,
        ])
    }
    #[cfg(windows)]
    {
        // O `st_mode` do `_wstat64`: tipo, leitura e escrita repetidas para
        // grupo e outros, execução nos diretórios.
        let leitura_escrita = if m.permissions().readonly() { 0o444 } else { 0o666 };
        let modo = if m.is_dir() { 0o040000 | leitura_escrita | 0o111 } else { 0o100000 | leitura_escrita };
        let ms = |t: std::io::Result<std::time::SystemTime>| t.map_or(0, milissegundos_de);
        Ok([tipo, ms(m.created()), ms(m.modified()), ms(m.accessed()), modo, m.len() as i64])
    }
}

/// `File::LinkTarget`.
fn alvo_do_link(p: &std::path::Path) -> ResultadoIo<Vec<u8>> {
    let m = std::fs::symlink_metadata(p)?;
    if !m.file_type().is_symlink() {
        return Err(ErroDoSo::do_codigo(codigo_do_so::NAO_EXISTE));
    }
    Ok(bytes_de_caminho(&std::fs::read_link(p)?))
}

/// `File::GetCanonicalPath` (`realpath`).
fn caminho_canonico(p: &std::path::Path) -> ResultadoIo<Vec<u8>> {
    let c = std::fs::canonicalize(p)?;
    let b = bytes_de_caminho(&c);
    // No Windows, o `std` devolve o caminho estendido (`\\?\C:\…`); a VM
    // tira o prefixo.
    #[cfg(windows)]
    if let Some(sem) = b.strip_prefix(br"\\?\") {
        if !sem.starts_with(b"UNC\\") {
            return Ok(sem.to_vec());
        }
    }
    Ok(b)
}

/// `File::AreIdentical`: o mesmo dispositivo e o mesmo nó (sem seguir
/// links).
#[cfg(unix)]
fn identicos(a: &std::path::Path, b: &std::path::Path) -> ResultadoIo<bool> {
    use std::os::unix::fs::MetadataExt;
    let ma = std::fs::symlink_metadata(a)?;
    let mb = std::fs::symlink_metadata(b)?;
    Ok(ma.dev() == mb.dev() && ma.ino() == mb.ino())
}
#[cfg(windows)]
fn identicos(a: &std::path::Path, b: &std::path::Path) -> ResultadoIo<bool> {
    #[repr(C)]
    #[derive(Default)]
    struct Informacao {
        atributos: u32,
        tempos: [u32; 6],
        volume: u32,
        tamanho_alto: u32,
        tamanho_baixo: u32,
        links: u32,
        indice_alto: u32,
        indice_baixo: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileInformationByHandle(h: *mut std::ffi::c_void, i: *mut Informacao) -> i32;
    }
    fn identidade(p: &std::path::Path) -> ResultadoIo<(u32, u32, u32)> {
        use std::os::windows::fs::OpenOptionsExt;
        use std::os::windows::io::AsRawHandle;
        // FILE_FLAG_BACKUP_SEMANTICS abre diretórios; sem seguir o link.
        let f = std::fs::OpenOptions::new().access_mode(0).custom_flags(0x0200_0000 | 0x0020_0000).open(p)?;
        let mut i = Informacao::default();
        // SAFETY: o handle é válido enquanto `f` vive.
        if unsafe { GetFileInformationByHandle(f.as_raw_handle(), &mut i) } == 0 {
            return Err(ErroDoSo::de(&std::io::Error::last_os_error()));
        }
        Ok((i.volume, i.indice_alto, i.indice_baixo))
    }
    Ok(identidade(a)? == identidade(b)?)
}

/// `File::GetStdioHandleType`: 0 terminal, 1 pipe, 2 arquivo, 3 soquete,
/// 4 outro.
#[cfg(unix)]
fn tipo_de_stdio(fd: i64) -> ResultadoIo<i64> {
    use std::os::unix::fs::FileTypeExt;
    let f = std::mem::ManuallyDrop::new(arquivo_do_descritor(fd));
    let t = f.metadata()?.file_type();
    Ok(if t.is_char_device() {
        0
    } else if t.is_fifo() {
        1
    } else if t.is_socket() {
        3
    } else if t.is_file() {
        2
    } else {
        4
    })
}
#[cfg(windows)]
fn tipo_de_stdio(fd: i64) -> ResultadoIo<i64> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileType(h: *mut std::ffi::c_void) -> u32;
    }
    let h = descritor_padrao(fd);
    // SAFETY: consulta o tipo de um handle do processo.
    let t = unsafe { GetFileType(h as usize as *mut std::ffi::c_void) };
    // FILE_TYPE_DISK = 1, FILE_TYPE_CHAR = 2, FILE_TYPE_PIPE = 3.
    Ok(match t {
        1 => 2,
        2 => 0,
        3 => 1,
        _ => 4,
    })
}

// ---------------------------------------------------------------------------
// Os natives de `_File` (estáticos, sobre caminhos).

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Exists(_ns: i64, raw: i64) -> i64 {
    dart_bool(arquivo_existe(&caminho_raw(raw)))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Create(_ns: i64, raw: i64, exclusivo: u8) -> i64 {
    dart_verdadeiro_ou_erro(criar_arquivo(&caminho_raw(raw), exclusivo != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_CreateLink(_ns: i64, raw: i64, alvo: i64) -> i64 {
    dart_nulo_ou_erro(criar_link(&caminho_raw(raw), &caminho_texto(alvo)))
}

/// `File_CreatePipe`: `[ponteiro de leitura, ponteiro de escrita]`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_CreatePipe(_ns: i64) -> i64 {
    match criar_pipe() {
        Ok((leitura, escrita)) => {
            let a = dart_int(ArquivoNativo::novo(leitura));
            let b = com_raizes(&[a], || dart_int(ArquivoNativo::novo(escrita)));
            com_raizes(&[a, b], || dart_lista_fixa(&[a, b]))
        }
        Err(e) => e.para_dart(),
    }
}

/// Um pipe anônimo: (leitura, escrita).
#[cfg(unix)]
fn criar_pipe() -> ResultadoIo<(i64, i64)> {
    let (r, w) = std::io::pipe()?;
    use std::os::unix::io::IntoRawFd;
    Ok((i64::from(r.into_raw_fd()), i64::from(w.into_raw_fd())))
}
#[cfg(windows)]
fn criar_pipe() -> ResultadoIo<(i64, i64)> {
    let (r, w) = std::io::pipe()?;
    use std::os::windows::io::IntoRawHandle;
    Ok((r.into_raw_handle() as usize as i64, w.into_raw_handle() as usize as i64))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_LinkTarget(_ns: i64, raw: i64) -> i64 {
    dart_ou_erro(alvo_do_link(&caminho_raw(raw)), |b| dart_texto_de_bytes(&b))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Delete(_ns: i64, raw: i64) -> i64 {
    dart_verdadeiro_ou_erro(apagar_arquivo(&caminho_raw(raw)))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_DeleteLink(_ns: i64, raw: i64) -> i64 {
    dart_verdadeiro_ou_erro(apagar_link(&caminho_raw(raw)))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Rename(_ns: i64, raw: i64, novo: i64) -> i64 {
    dart_verdadeiro_ou_erro(renomear_arquivo(&caminho_raw(raw), &caminho_texto(novo)))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_RenameLink(_ns: i64, raw: i64, novo: i64) -> i64 {
    dart_verdadeiro_ou_erro(renomear_link(&caminho_raw(raw), &caminho_texto(novo)))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Copy(_ns: i64, raw: i64, novo: i64) -> i64 {
    dart_verdadeiro_ou_erro(copiar_arquivo(&caminho_raw(raw), &caminho_texto(novo)))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_LengthFromPath(_ns: i64, raw: i64) -> i64 {
    dart_ou_erro(metadados_de_arquivo(&caminho_raw(raw)), |m| dart_int(m.len() as i64))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_LastModified(_ns: i64, raw: i64) -> i64 {
    dart_ou_erro(ultima_modificacao(&caminho_raw(raw)), dart_int)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_LastAccessed(_ns: i64, raw: i64) -> i64 {
    dart_ou_erro(ultimo_acesso(&caminho_raw(raw)), dart_int)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_SetLastModified(_ns: i64, raw: i64, ms: i64) -> i64 {
    dart_nulo_ou_erro(mudar_instantes(&caminho_raw(raw), None, Some(ms)))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_SetLastAccessed(_ns: i64, raw: i64, ms: i64) -> i64 {
    dart_nulo_ou_erro(mudar_instantes(&caminho_raw(raw), Some(ms), None))
}

/// `File_Open`: o ponteiro do arquivo aberto (um `int`) ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Open(_ns: i64, raw: i64, modo: i64) -> i64 {
    dart_ou_erro(abrir_arquivo(&caminho_raw(raw), modo), |d| dart_int(ArquivoNativo::novo(d)))
}

/// `File_OpenStdio`: o arquivo da entrada/saída padrão `fd`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_OpenStdio(fd: i64) -> i64 {
    ArquivoNativo::novo(descritor_padrao(fd))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_GetStdioHandleType(fd: i64) -> i64 {
    dart_ou_erro(tipo_de_stdio(fd), dart_int)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_GetType(_ns: i64, raw: i64, seguir: u8) -> i64 {
    dart_int(tipo_do_caminho(&caminho_raw(raw), seguir != 0))
}

/// `File_Stat`: a `Int64List` de [`estatisticas`], ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Stat(_ns: i64, caminho: i64) -> i64 {
    dart_ou_erro(estatisticas(&caminho_texto(caminho)), |d| dart_int64s(&d))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_AreIdentical(_ns: i64, a: i64, b: i64) -> i64 {
    dart_ou_erro(identicos(&caminho_texto(a), &caminho_texto(b)), dart_bool)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_ResolveSymbolicLinks(_ns: i64, raw: i64) -> i64 {
    dart_ou_erro(caminho_canonico(&caminho_raw(raw)), |b| dart_texto_de_bytes(&b))
}

// ---------------------------------------------------------------------------
// Os natives de `_RandomAccessFileOpsImpl` (instância, sobre o arquivo do
// campo nativo).

/// O arquivo do receptor, ou lança o erro interno da VM ("No native peer").
fn arquivo_do_receptor<'a>(this: i64) -> Option<&'a ArquivoNativo> {
    let p = campo_nativo(this);
    if p == 0 {
        lancar_erro_interno("No native peer");
        return None;
    }
    // SAFETY: o campo guarda a referência do objeto, viva até `close` ou a
    // coleta dele.
    Some(unsafe { ArquivoNativo::de(p) })
}

/// `File_SetPointer`: o objeto passa a ser dono da referência de `p`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_SetPointer(this: i64, p: i64) {
    anexar_finalizador(this, liberar_arquivo_do_objeto, p as usize);
    gravar_campo_nativo(this, p);
}

/// `File_GetPointer`: retém uma referência para o IOService (0 se fechado).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_GetPointer(this: i64) -> i64 {
    let p = campo_nativo(this);
    if p != 0 {
        ArquivoNativo::reter(p);
    }
    p
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_GetFD(this: i64) -> i64 {
    arquivo_do_receptor(this).map_or(0, |a| a.descritor())
}

/// `File_Close`: 0, ou -1 se já fechado.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Close(this: i64) -> i64 {
    let p = campo_nativo(this);
    if p == 0 {
        return -1;
    }
    // SAFETY: a referência do objeto está viva.
    unsafe { ArquivoNativo::de(p) }.fechar();
    remover_finalizador(this);
    ArquivoNativo::liberar(p);
    gravar_campo_nativo(this, 0);
    0
}

/// `File_ReadByte`: o byte, -1 no fim, ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_ReadByte(this: i64) -> i64 {
    let Some(a) = arquivo_do_receptor(this) else { return 0 };
    let mut b = [0u8; 1];
    dart_ou_erro(a.ler(&mut b), |n| dart_int(if n == 1 { i64::from(b[0]) } else { -1 }))
}

/// `File_WriteByte`: 1 ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_WriteByte(this: i64, valor: i64) -> i64 {
    let Some(a) = arquivo_do_receptor(this) else { return 0 };
    let Some(v) = HEAP.with(|h| h.borrow().int_de_ref(valor)) else {
        return ErroDoSo::argumento_invalido().para_dart();
    };
    dart_ou_erro(a.escrever_tudo(&[v as u8]), |()| dart_int(1))
}

/// `File_Read`: os bytes lidos (uma visão, como o `_makeUint8ListView` da
/// VM, se vieram menos que o pedido) ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Read(this: i64, n: i64) -> i64 {
    let Some(a) = arquivo_do_receptor(this) else { return 0 };
    if n < 0 {
        return ErroDoSo::argumento_invalido().para_dart();
    }
    let mut buf = vec![0u8; n as usize];
    match a.ler(&mut buf) {
        Ok(lidos) if lidos as i64 == n => dart_bytes(buf),
        Ok(lidos) => {
            buf.truncate(lidos);
            let base = dart_bytes(buf);
            let cid = cid_registrado(CID_UINT8_VIEW).expect("bug do compilador: dart:io sem o SDK da fonte");
            com_raizes(&[base], || dartforge_view_nova(cid, i64::from(TIPO_UINT8), base, 0, lidos as i64))
        }
        Err(e) => e.para_dart(),
    }
}

/// `File_ReadInto(buffer, start, end)`: os bytes lidos, gravados em
/// `buffer[start..]`, ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_ReadInto(this: i64, buffer: i64, inicio: i64, fim: i64) -> i64 {
    let Some(a) = arquivo_do_receptor(this) else { return 0 };
    let fim = HEAP.with(|h| h.borrow().int_de_ref(fim)).unwrap_or(inicio);
    let mut buf = vec![0u8; (fim - inicio).max(0) as usize];
    match a.ler(&mut buf) {
        Ok(lidos) => {
            gravar_bytes_na_lista(buffer, inicio as usize, &buf[..lidos]);
            dart_int(lidos as i64)
        }
        Err(e) => e.para_dart(),
    }
}

/// Grava `bytes` em `lista[inicio..]` (lista tipada ou `List<int>`, o
/// `Dart_ListSetAsBytes` da VM).
fn gravar_bytes_na_lista(lista: i64, inicio: usize, bytes: &[u8]) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        if let Some((interna, desloc, tipo, _)) = resolver(&heap, lista) {
            // Só listas de bytes chegam aqui (`_ensureFastAndSerializable…`
            // nos patches); outra largura grava um byte por elemento.
            let passo = tamanho_do_elemento(tipo);
            let destino = bytes_de_mut(&mut heap, interna);
            for (i, &b) in bytes.iter().enumerate() {
                let pos = desloc + (inicio + i) * passo;
                if pos < destino.len() {
                    destino[pos] = b;
                }
            }
            return;
        }
        for (i, &b) in bytes.iter().enumerate() {
            heap.list_set(lista, inicio + i, TaggedValue::scalar(i64::from(b)));
        }
    });
}

/// `File_WriteFrom(buffer, start, end)`: `null` ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_WriteFrom(this: i64, buffer: i64, inicio: i64, fim: i64) -> i64 {
    let Some(a) = arquivo_do_receptor(this) else { return 0 };
    let bytes = bytes_da_lista_tipada(buffer).unwrap_or_default();
    let fim = HEAP.with(|h| h.borrow().int_de_ref(fim)).unwrap_or(bytes.len() as i64) as usize;
    let inicio = (inicio.max(0) as usize).min(fim);
    dart_nulo_ou_erro(a.escrever_tudo(&bytes[inicio..fim.min(bytes.len())]))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Position(this: i64) -> i64 {
    let Some(a) = arquivo_do_receptor(this) else { return 0 };
    dart_ou_erro(a.posicao(), dart_int)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_SetPosition(this: i64, p: i64) -> i64 {
    let Some(a) = arquivo_do_receptor(this) else { return 0 };
    dart_verdadeiro_ou_erro(a.mudar_posicao(p))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Truncate(this: i64, n: i64) -> i64 {
    let Some(a) = arquivo_do_receptor(this) else { return 0 };
    dart_verdadeiro_ou_erro(a.truncar(n))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Length(this: i64) -> i64 {
    let Some(a) = arquivo_do_receptor(this) else { return 0 };
    dart_ou_erro(a.tamanho(), dart_int)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Flush(this: i64) -> i64 {
    let Some(a) = arquivo_do_receptor(this) else { return 0 };
    dart_verdadeiro_ou_erro(a.sincronizar())
}

/// `File_Lock(tipo, início, fim)`: `true` ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Lock(this: i64, tipo: i64, inicio: i64, fim: i64) -> i64 {
    let Some(a) = arquivo_do_receptor(this) else { return 0 };
    if !(TRAVA_SOLTAR..=TRAVA_EXCLUSIVA_BLOQUEANTE).contains(&tipo) || inicio < 0 || (fim != -1 && fim <= inicio) {
        return ErroDoSo::argumento_invalido().para_dart();
    }
    dart_verdadeiro_ou_erro(a.travar(tipo, inicio, fim))
}

// ---------------------------------------------------------------------------
// `_NamespaceImpl`: só o namespace padrão (o diretório corrente).

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Namespace_Create(ns: i64, _n: i64) -> i64 {
    ns
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Namespace_GetPointer(_ns: i64) -> i64 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Namespace_GetDefault() -> i64 {
    0
}
