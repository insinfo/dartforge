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
// Um arquivo aberto é um [`NativeFile`] com contagem de referências,
// como o `File` da VM (`ReferenceCounted<File>`): o objeto Dart guarda o
// ponteiro no campo nativo e solta a referência dele quando é fechado ou
// coletado (o finalizador de `Heap::finalizable`); `getPointer` retém uma
// referência para o IOService, que a solta ao fim do pedido.

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering as AtomicOrdering};

// ---------------------------------------------------------------------------
// Erros do sistema.

/// Os códigos de erro que os natives atribuem por conta própria (o
/// `SetErrno` de `file_linux.cc`): errno no Unix, `GetLastError` no Windows.
#[cfg(unix)]
mod os_code {
    pub const NOT_FOUND: i32 = 2; // ENOENT
    pub const IS_DIRECTORY: i32 = 21; // EISDIR
    pub const INVALID: i32 = 22; // EINVAL
    pub const NOT_A_DIRECTORY: i32 = 20; // ENOTDIR
    pub const ALREADY_EXISTS: i32 = 17; // EEXIST
}
#[cfg(windows)]
mod os_code {
    pub const NOT_FOUND: i32 = 2; // ERROR_FILE_NOT_FOUND
    pub const IS_DIRECTORY: i32 = 5; // ERROR_ACCESS_DENIED
    pub const INVALID: i32 = 87; // ERROR_INVALID_PARAMETER
    pub const NOT_A_DIRECTORY: i32 = 267; // ERROR_DIRECTORY
    pub const ALREADY_EXISTS: i32 = 183; // ERROR_ALREADY_EXISTS
}

/// A mensagem do sistema para o código `c`, como a VM a monta.
///
/// No Windows é o `FormatMessageIntoBuffer` da VM (`bin/utils_win.cc`): o
/// texto do `FormatMessageW` como vem, com o CRLF final (o `std` o tira), e
/// `OS Error N` quando o sistema não tem mensagem.
#[cfg(windows)]
fn system_message(c: i32) -> String {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn FormatMessageW(
            flags: u32,
            origin: *const std::ffi::c_void,
            id: u32,
            locale: u32,
            buffer: *mut u16,
            size: u32,
            arguments: *const std::ffi::c_void,
        ) -> u32;
    }
    const FORMAT_MESSAGE_IGNORE_INSERTS: u32 = 0x0000_0200;
    const FORMAT_MESSAGE_FROM_SYSTEM: u32 = 0x0000_1000;
    // MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT).
    const LOCALE: u32 = 0x0400;
    let mut buffer = [0u16; 1024];
    // SAFETY: o buffer tem o tamanho informado; sem inserções.
    let n = unsafe {
        FormatMessageW(
            FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
            std::ptr::null(),
            c as u32,
            LOCALE,
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
fn system_message(c: i32) -> String {
    let text = std::io::Error::from_raw_os_error(c).to_string();
    let suffix = format!(" (os error {c})");
    text.strip_suffix(&suffix).map(str::to_string).unwrap_or(text)
}

/// Um erro do sistema operacional, como o `OSError` da VM (`bin/utils.h`):
/// o código e a mensagem do sistema (`strerror_r`; `FormatMessage` no
/// Windows).
#[derive(Clone, Debug)]
struct OsError {
    code: i64,
    message: String,
}

impl OsError {
    /// O erro do código `c` do sistema.
    fn from_code(c: i32) -> OsError {
        OsError { code: i64::from(c), message: system_message(c) }
    }

    /// O erro de uma operação do `std` (que carrega o código do sistema).
    fn de(e: &std::io::Error) -> OsError {
        match e.raw_os_error() {
            Some(c) => OsError::from_code(c),
            None => OsError { code: -1, message: e.to_string() },
        }
    }

    /// O `OSError(-1, "Invalid argument", kUnknown)` dos natives da VM.
    fn invalid_argument() -> OsError {
        OsError { code: -1, message: "Invalid argument".to_string() }
    }

    /// O `OSError` do Dart (`DartUtils::NewDartOSError`), pela função da
    /// sobreposição de `dart:io`.
    fn to_dart(&self) -> i64 {
        let Some(f) = ajudante("_dartforgeOSError") else {
            panic!("bug do compilador: dart:io sem `_dartforgeOSError` registrado");
        };
        // SAFETY: registrada pela biblioteca `dart:io` da sobreposição com a
        // assinatura (`String`, `int`) → `OSError`.
        let g = { let dart_target: usize = f; move |a0: i64, a1: i64| -> i64 { dart_r2(dart_target, a0, a1) } };
        let message = allocate_str(&self.message);
        with_roots(&[message], || g(message, self.code))
    }

    /// A resposta de erro do IOService (`CObject::NewOSError`):
    /// `[kOSError, código, message]`.
    fn to_response(&self) -> Portable {
        Portable::List(vec![
            Portable::Int(RESPONSE_OS_ERROR),
            Portable::Int(self.code),
            Portable::Str(self.message.clone()),
        ])
    }
}

impl From<std::io::Error> for OsError {
    fn from(e: std::io::Error) -> OsError {
        OsError::de(&e)
    }
}

type IoResult<T> = Result<T, OsError>;

// ---------------------------------------------------------------------------
// Valores Dart devolvidos pelos natives.

fn dart_bool(b: bool) -> i64 {
    Heap::box_bool(b)
}

fn dart_int(i: i64) -> i64 {
    HEAP.with(|h| h.borrow_mut().box_int(i))
}

/// Uma `String` Dart a partir de bytes do sistema: UTF-8, com a sequência
/// inválida trocada por U+FFFD.
fn dart_string_from_bytes(b: &[u8]) -> i64 {
    allocate_str(&String::from_utf8_lossy(b))
}

/// Uma lista tipada interna do `tipo` com os bytes (o comprimento em
/// elementos é `bytes.len() / tamanho`).
fn dart_typed_from_bytes(tipo: u8, bytes: &[u8]) -> i64 {
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        let l = h.new_typed(tipo, bytes.len() / tamanho_do_elemento(tipo));
        if let Some(b) = h.typed_bytes_mut(l) {
            b.copy_from_slice(bytes);
        }
        l
    })
}

/// Uma `Uint8List` (`_Uint8List`) com os bytes.
fn dart_bytes(bytes: Vec<u8>) -> i64 {
    dart_typed_from_bytes(TYPE_UINT8, &bytes)
}

/// Uma `Int64List` (`_Int64List`) com os valores.
fn dart_int64s(value_list: &[i64]) -> i64 {
    let bytes: Vec<u8> = value_list.iter().flat_map(|v| v.to_ne_bytes()).collect();
    dart_typed_from_bytes(TYPE_INT64, &bytes)
}

/// Uma `_List` de tamanho fixo (cid 8, geral) com os valores (já na posição
/// `Ref`), enraizados durante a alocação.
fn dart_fixed_list(value_list: &[i64]) -> i64 {
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        let frame = h.push_owning_frame(value_list.len().max(1));
        for (i, &v) in value_list.iter().enumerate() {
            h.set_root(frame, i, v);
        }
        let l = h.new_list(crate::layout::cid::LIST, value_list.len(), crate::lists::Element::General);
        // A palavra 0 do corpo é o comprimento; os elementos, as seguintes.
        if !value_list.is_empty() {
            h.write_refs(l, 1, value_list);
        }
        // As arestas da lista substituem as cópias proprietárias temporárias.
        h.pop_frame(frame);
        l
    })
}

/// O valor Dart de um resultado: o `Ok` por `f`, o erro como `OSError`.
fn dart_or_error<T>(r: IoResult<T>, f: impl FnOnce(T) -> i64) -> i64 {
    match r {
        Ok(v) => f(v),
        Err(e) => e.to_dart(),
    }
}

/// `true` ou o `OSError` — a forma de quase todos os natives de caminho.
fn dart_true_or_error(r: IoResult<()>) -> i64 {
    dart_or_error(r, |()| dart_bool(true))
}

/// `null` ou o `OSError`.
fn dart_null_or_error(r: IoResult<()>) -> i64 {
    dart_or_error(r, |()| 0)
}

/// Lança `StateError(message)` (o `_InternalError` dos natives da VM).
fn throw_internal_error(message: &str) {
    let e = allocate_state_error(message);
    with_roots(&[e], || dartforge_exception_throw(e, 3));
}

// ---------------------------------------------------------------------------
// Caminhos.

/// Os bytes de uma lista tipada (lista interna ou visão).
fn typed_list_bytes(h: i64) -> Option<Vec<u8>> {
    HEAP.with(|heap| heap.borrow().typed_bytes(h).map(<[u8]>::to_vec))
}

/// Os bytes de uma lista do núcleo de inteiros, cada um truncado a 8 bits
/// (o `Dart_ListGetAsBytes` da VM); vazio se `h` não é lista do núcleo.
fn bytes_from_value_list(heap: &Heap, h: i64) -> Vec<u8> {
    if !heap.e_lista(h) {
        return Vec::new();
    }
    (0..heap.list_len(h))
        .map(|i| match heap.list_get(h, i) {
            crate::heap::Value::Int(v) => v as u8,
            crate::heap::Value::Ref(r) => heap.int_of(r).unwrap_or(0) as u8,
            crate::heap::Value::Bool(b) => u8::from(b),
            crate::heap::Value::Double(_) => 0,
        })
        .collect()
}

/// O caminho de um `rawPath` (`Uint8List` UTF-8 terminado em NUL, o
/// `DartUtils::GetNativeTypedDataArgument` da VM): os bytes até o NUL.
fn path_bytes(raw: i64) -> Vec<u8> {
    let mut b = typed_list_bytes(raw).unwrap_or_default();
    if let Some(end) = b.iter().position(|&c| c == 0) {
        b.truncate(end);
    }
    b
}

/// O caminho do sistema para os bytes (Unix: os bytes; Windows: o UTF-8
/// convertido para UTF-16, como o `Utf8ToWideChar` da VM).
#[cfg(unix)]
fn path_from_bytes(b: &[u8]) -> std::path::PathBuf {
    use std::os::unix::ffi::OsStrExt;
    std::path::PathBuf::from(std::ffi::OsStr::from_bytes(b))
}
#[cfg(windows)]
fn path_from_bytes(b: &[u8]) -> std::path::PathBuf {
    std::path::PathBuf::from(String::from_utf8_lossy(b).into_owned())
}

/// Os bytes de um caminho do sistema (o inverso de [`path_from_bytes`]).
#[cfg(unix)]
fn path_bytes_of(p: &std::path::Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    p.as_os_str().as_bytes().to_vec()
}
#[cfg(windows)]
fn path_bytes_of(p: &std::path::Path) -> Vec<u8> {
    p.to_string_lossy().into_owned().into_bytes()
}

/// O caminho de um `rawPath` Dart.
fn raw_path(raw: i64) -> std::path::PathBuf {
    path_from_bytes(&path_bytes(raw))
}

/// O caminho de uma `String` Dart (`DartUtils::GetNativeStringArgument`:
/// UTF-8).
fn path_text(s: i64) -> std::path::PathBuf {
    path_from_bytes(&utf8_of_string(s))
}

/// O UTF-8 de uma `String` Dart (o substituto solto vira U+FFFD, como no
/// `Dart_StringToUTF8` da VM).
fn utf8_of_string(s: i64) -> Vec<u8> {
    if s == 0 {
        return Vec::new();
    }
    HEAP.with(|h| h.borrow().text(s).map(|t| t.to_vm_utf8()).unwrap_or_default())
}

// ---------------------------------------------------------------------------
// O campo nativo de `NativeFieldWrapperClass1` (a sobreposição de
// `dart:nativewrappers` o declara como o primeiro campo do layout).

fn native_field(obj: i64) -> i64 {
    HEAP.with(|h| match h.borrow().object(obj) {
        Some(o) => o.first().map_or(0, |f| f.0),
        None => panic!("bug do compilador: campo nativo de algo que não é objeto"),
    })
}

fn write_native_field(obj: i64, value: i64) {
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        if h.object(obj).is_none_or(|o| o.is_empty()) {
            panic!("bug do compilador: campo nativo de algo que não é objeto");
        }
        h.set_field(obj, 0, value, false);
    });
}

/// Liga `pair` ao objeto: quando o objeto for coletado, `finalizer(pair)`
/// (o `Dart_NewFinalizableHandle` da VM).
fn attach_finalizer(obj: i64, finalizer: fn(usize), pair: usize) {
    HEAP.with(|h| h.borrow_mut().finalizable.insert(obj, (finalizer, pair)));
}

fn remove_finalizer(obj: i64) {
    HEAP.with(|h| h.borrow_mut().finalizable.remove(&obj));
}

// ---------------------------------------------------------------------------
// O arquivo aberto.

/// O descritor de um arquivo fechado (`File::kClosedFd`).
const CLOSED_DESCRIPTOR: i64 = -1;

/// Um arquivo aberto (o `File` de `bin/file.h`): o descritor do sistema
/// (Unix) ou o `HANDLE` (Windows). O isolado e as threads do IOService o
/// compartilham; o Dart dos patches serializa as operações de um mesmo
/// arquivo (`_RandomAccessFile._checkAvailable`).
struct NativeFile {
    descriptor: AtomicI64,
}

impl NativeFile {
    /// Cria o arquivo com uma referência e devolve o ponteiro dela.
    fn new(descriptor: i64) -> i64 {
        Arc::into_raw(Arc::new(NativeFile { descriptor: AtomicI64::new(descriptor) })) as i64
    }

    /// O arquivo do ponteiro `p`, emprestado de quem tem a referência.
    ///
    /// # Safety
    /// `p` veio de [`NativeFile::new`] e a referência ainda existe.
    unsafe fn de<'a>(p: i64) -> &'a NativeFile {
        // SAFETY: garantido por quem chama.
        unsafe { &*(p as *const NativeFile) }
    }

    /// Mais uma referência (`Retain`).
    fn retain_ref(p: i64) {
        // SAFETY: `p` é um ponteiro vivo de `new`.
        unsafe { Arc::increment_strong_count(p as *const NativeFile) };
    }

    /// Solta uma referência (`Release`); a última fecha o arquivo.
    fn free(p: i64) {
        // SAFETY: `p` é um ponteiro vivo de `new`, com esta referência.
        unsafe { Arc::decrement_strong_count(p as *const NativeFile) };
    }

    /// Toma a referência de `p` (o `RefCntReleaseScope` da VM: solta ao
    /// sair do escopo).
    ///
    /// # Safety
    /// `p` é um ponteiro de `new` cuja referência passa a quem chama.
    unsafe fn take(p: i64) -> Arc<NativeFile> {
        // SAFETY: garantido por quem chama.
        unsafe { Arc::from_raw(p as *const NativeFile) }
    }

    fn closed_flag(&self) -> bool {
        self.descriptor.load(AtomicOrdering::Acquire) == CLOSED_DESCRIPTOR
    }

    fn descriptor(&self) -> i64 {
        self.descriptor.load(AtomicOrdering::Acquire)
    }

    /// Opera no arquivo pelo `std::fs::File`, sem transferir o descritor.
    fn com<R>(&self, f: impl FnOnce(&mut std::fs::File) -> std::io::Result<R>) -> IoResult<R> {
        let d = self.descriptor();
        if d == CLOSED_DESCRIPTOR {
            return Err(OsError::from_code(os_code::INVALID));
        }
        let mut file = std::mem::ManuallyDrop::new(file_from_descriptor(d));
        f(&mut file).map_err(OsError::from)
    }

    /// `File::Read`: uma leitura (repetida só na interrupção por sinal).
    fn do_read(&self, dest: &mut [u8]) -> IoResult<usize> {
        use std::io::Read;
        self.com(|f| loop {
            match f.read(dest) {
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                r => return r,
            }
        })
    }

    /// `File::WriteFully`.
    fn write_all_bytes(&self, data: &[u8]) -> IoResult<()> {
        use std::io::Write;
        if matches!(self.descriptor(), 1 | 2) {
            // A saída do `print` passa pelo `stdout` do `std`: o que ele
            // tiver guardado sai antes, para a ordem ser a do programa.
            let _ = std::io::stdout().flush();
        }
        self.com(|f| f.write_all(data))
    }

    fn cur_position(&self) -> IoResult<i64> {
        use std::io::Seek;
        self.com(|f| f.stream_position()).map(|p| p as i64)
    }

    fn set_position(&self, p: i64) -> IoResult<()> {
        use std::io::Seek;
        self.com(|f| f.seek(std::io::SeekFrom::Start(p as u64))).map(|_| ())
    }

    fn truncate_file(&self, n: i64) -> IoResult<()> {
        if n < 0 {
            return Err(OsError::from_code(os_code::INVALID));
        }
        self.com(|f| f.set_len(n as u64))
    }

    fn size(&self) -> IoResult<i64> {
        self.com(|f| f.metadata()).map(|m| m.len() as i64)
    }

    /// `File::Flush`: `fsync` (Unix; o `sync_all` do `std` no macOS seria o
    /// `F_FULLFSYNC`, que a VM não usa) ou `FlushFileBuffers` (Windows).
    fn sync_file(&self) -> IoResult<()> {
        #[cfg(unix)]
        {
            unsafe extern "C" {
                fn fsync(fd: i32) -> i32;
            }
            let d = self.descriptor();
            // SAFETY: `d` é um descritor aberto deste arquivo.
            if unsafe { fsync(d as i32) } == -1 {
                return Err(OsError::de(&std::io::Error::last_os_error()));
            }
            Ok(())
        }
        #[cfg(windows)]
        {
            self.com(|f| f.sync_all())
        }
    }

    /// `File::Lock`: `fcntl(F_SETLK[W])` no Unix, `LockFileEx` no Windows.
    fn lock_file(&self, tipo: i64, start: i64, end: i64) -> IoResult<()> {
        lock_descriptor(self.descriptor(), tipo, start, end)
    }

    /// `File::Close`: fecha o descritor. A saída padrão não é fechada: vira
    /// `/dev/null` (a VM faz o mesmo, para o processo não escrever num
    /// descritor que outro `open` reaproveitaria).
    fn do_close(&self) {
        let d = self.descriptor.swap(CLOSED_DESCRIPTOR, AtomicOrdering::AcqRel);
        if d == CLOSED_DESCRIPTOR {
            return;
        }
        close_descriptor(d);
    }
}

impl Drop for NativeFile {
    /// O destrutor do `File` da VM: fecha o que ficou aberto, menos a saída
    /// e o erro padrão.
    fn drop(&mut self) {
        let d = self.descriptor();
        if d != CLOSED_DESCRIPTOR && !is_stdout(d) {
            self.do_close();
        }
    }
}

/// O finalizador do objeto Dart dono de um arquivo.
fn free_object_file(pair: usize) {
    NativeFile::free(pair as i64);
}

#[cfg(unix)]
fn file_from_descriptor(d: i64) -> std::fs::File {
    use std::os::unix::io::FromRawFd;
    // SAFETY: `d` é um descritor aberto; quem chama não o fecha pelo `File`.
    unsafe { std::fs::File::from_raw_fd(d as i32) }
}
#[cfg(windows)]
fn file_from_descriptor(d: i64) -> std::fs::File {
    use std::os::windows::io::FromRawHandle;
    // SAFETY: `d` é um HANDLE aberto; quem chama não o fecha pelo `File`.
    unsafe { std::fs::File::from_raw_handle(d as usize as *mut std::ffi::c_void) }
}

/// O descritor de um `std::fs::File` (que deixa de ser dono dele).
#[cfg(unix)]
fn file_descriptor(f: std::fs::File) -> i64 {
    use std::os::unix::io::IntoRawFd;
    i64::from(f.into_raw_fd())
}
#[cfg(windows)]
fn file_descriptor(f: std::fs::File) -> i64 {
    use std::os::windows::io::IntoRawHandle;
    f.into_raw_handle() as usize as i64
}

#[cfg(unix)]
fn is_stdout(d: i64) -> bool {
    d == 1 || d == 2
}
#[cfg(windows)]
fn is_stdout(d: i64) -> bool {
    d == default_descriptor(1) || d == default_descriptor(2)
}

#[cfg(unix)]
fn close_descriptor(d: i64) {
    unsafe extern "C" {
        fn open(file_path: *const std::ffi::c_char, flags: i32, ...) -> i32;
        fn dup2(de: i32, to: i32) -> i32;
        fn close(fd: i32) -> i32;
    }
    if d == 1 {
        use std::io::Write;
        let _ = std::io::stdout().flush();
        // SAFETY: caminho C constante; os descritores são válidos.
        unsafe {
            let null_val = open(c"/dev/null".as_ptr(), 1 /* O_WRONLY */);
            if null_val >= 0 {
                dup2(null_val, 1);
                close(null_val);
            }
        }
        return;
    }
    // SAFETY: `d` é um descritor aberto deste arquivo, fechado uma vez.
    unsafe { close(d as i32) };
}
#[cfg(windows)]
fn close_descriptor(d: i64) {
    if is_stdout(d) || d == default_descriptor(0) {
        return;
    }
    drop(file_from_descriptor(d));
}

/// O descritor da entrada/saída padrão `fd` (0, 1, 2).
#[cfg(unix)]
fn default_descriptor(fd: i64) -> i64 {
    fd
}
#[cfg(windows)]
fn default_descriptor(fd: i64) -> i64 {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(which: u32) -> *mut std::ffi::c_void;
    }
    // STD_INPUT_HANDLE = -10, STD_OUTPUT_HANDLE = -11, STD_ERROR_HANDLE = -12.
    let which = (-10 - fd) as i32 as u32;
    // SAFETY: só lê o handle do processo.
    unsafe { GetStdHandle(which) as usize as i64 }
}

#[cfg(all(unix, target_os = "linux"))]
fn lock_descriptor(d: i64, tipo: i64, start: i64, end: i64) -> IoResult<()> {
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
        LOCK_UNLOCK => F_UNLCK,
        LOCK_SHARED | LOCK_SHARED_BLOCKING => F_RDLCK,
        LOCK_EXCLUSIVE | LOCK_EXCLUSIVE_BLOCKING => F_WRLCK,
        _ => return Err(OsError::invalid_argument()),
    };
    let fl = Flock { l_type, l_whence: 0, l_start: start, l_len: if end == -1 { 0 } else { end - start }, l_pid: 0 };
    let cmd = if matches!(tipo, LOCK_SHARED_BLOCKING | LOCK_EXCLUSIVE_BLOCKING) { F_SETLKW } else { F_SETLK };
    fcntl_lock(d, cmd, &fl as *const Flock as *const std::ffi::c_void)
}
#[cfg(all(unix, not(target_os = "linux")))]
fn lock_descriptor(d: i64, tipo: i64, start: i64, end: i64) -> IoResult<()> {
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
        LOCK_UNLOCK => F_UNLCK,
        LOCK_SHARED | LOCK_SHARED_BLOCKING => F_RDLCK,
        LOCK_EXCLUSIVE | LOCK_EXCLUSIVE_BLOCKING => F_WRLCK,
        _ => return Err(OsError::invalid_argument()),
    };
    let fl = Flock { l_start: start, l_len: if end == -1 { 0 } else { end - start }, l_pid: 0, l_type, l_whence: 0 };
    let cmd = if matches!(tipo, LOCK_SHARED_BLOCKING | LOCK_EXCLUSIVE_BLOCKING) { F_SETLKW } else { F_SETLK };
    fcntl_lock(d, cmd, &fl as *const Flock as *const std::ffi::c_void)
}
#[cfg(unix)]
fn fcntl_lock(d: i64, cmd: i32, fl: *const std::ffi::c_void) -> IoResult<()> {
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
            return Err(OsError::de(&e));
        }
    }
}
#[cfg(windows)]
fn lock_descriptor(d: i64, tipo: i64, start: i64, end: i64) -> IoResult<()> {
    #[repr(C)]
    struct Overlapped {
        internal: usize,
        internal_high: usize,
        offset: u32,
        offset_high: u32,
        event: *mut std::ffi::c_void,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LockFileEx(h: *mut std::ffi::c_void, flags: u32, r: u32, low: u32, high: u32, o: *mut Overlapped) -> i32;
        fn UnlockFileEx(h: *mut std::ffi::c_void, r: u32, low: u32, high: u32, o: *mut Overlapped) -> i32;
    }
    const LOCKFILE_FAIL_IMMEDIATELY: u32 = 1;
    const LOCKFILE_EXCLUSIVE_LOCK: u32 = 2;
    let length: u64 = if end == -1 { u64::MAX } else { (end - start) as u64 };
    let mut o = Overlapped {
        internal: 0,
        internal_high: 0,
        offset: start as u64 as u32,
        offset_high: ((start as u64) >> 32) as u32,
        event: std::ptr::null_mut(),
    };
    let h = d as usize as *mut std::ffi::c_void;
    let (low, high) = (length as u32, (length >> 32) as u32);
    // SAFETY: `h` é o HANDLE aberto do arquivo; `o` vive durante a chamada.
    let ok = unsafe {
        match tipo {
            LOCK_UNLOCK => UnlockFileEx(h, 0, low, high, &mut o),
            LOCK_SHARED => LockFileEx(h, LOCKFILE_FAIL_IMMEDIATELY, 0, low, high, &mut o),
            LOCK_SHARED_BLOCKING => LockFileEx(h, 0, 0, low, high, &mut o),
            LOCK_EXCLUSIVE => LockFileEx(h, LOCKFILE_FAIL_IMMEDIATELY | LOCKFILE_EXCLUSIVE_LOCK, 0, low, high, &mut o),
            LOCK_EXCLUSIVE_BLOCKING => LockFileEx(h, LOCKFILE_EXCLUSIVE_LOCK, 0, low, high, &mut o),
            _ => return Err(OsError::invalid_argument()),
        }
    };
    if ok == 0 {
        return Err(OsError::de(&std::io::Error::last_os_error()));
    }
    Ok(())
}

/// `File::LockType`.
const LOCK_UNLOCK: i64 = 0;
const LOCK_SHARED: i64 = 1;
const LOCK_EXCLUSIVE: i64 = 2;
const LOCK_SHARED_BLOCKING: i64 = 3;
const LOCK_EXCLUSIVE_BLOCKING: i64 = 4;

// ---------------------------------------------------------------------------
// As operações sobre caminhos (`File::*` de `file_linux.cc`), partilhadas
// pelos natives síncronos e pelo IOService.

/// `File::Type`.
const TYPE_FILE: i64 = 0;
const TYPE_DIRECTORY: i64 = 1;
const TYPE_LINK: i64 = 2;
const TYPE_SOCKET: i64 = 3;
const TYPE_PIPE: i64 = 4;
const TYPE_NOT_FOUND: i64 = 5;

/// `File::DartFileOpenMode` → (leitura/escrita, só escrita, truncar).
fn open_mode(dart_mode: i64) -> Option<(bool, bool, bool)> {
    match dart_mode {
        0 => Some((false, false, false)), // read → kRead
        1 => Some((true, false, true)),   // write → kWriteTruncate
        2 => Some((true, false, false)),  // append → kWrite
        3 => Some((false, true, true)),   // writeOnly → kWriteOnlyTruncate
        4 => Some((false, true, false)),  // writeOnlyAppend → kWriteOnly
        _ => None,
    }
}

/// O tipo de uma entrada do sistema de arquivos (`File::Type`).
fn metadata_kind(m: &std::fs::Metadata) -> i64 {
    let t = m.file_type();
    if t.is_dir() {
        return TYPE_DIRECTORY;
    }
    if t.is_file() {
        return TYPE_FILE;
    }
    if t.is_symlink() {
        return TYPE_LINK;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        if t.is_socket() {
            return TYPE_SOCKET;
        }
        if t.is_fifo() {
            return TYPE_PIPE;
        }
    }
    TYPE_NOT_FOUND
}

/// `File::GetType`.
fn path_type(p: &std::path::Path, follow_links_flag: bool) -> i64 {
    let m = if follow_links_flag { std::fs::metadata(p) } else { std::fs::symlink_metadata(p) };
    m.map_or(TYPE_NOT_FOUND, |m| metadata_kind(&m))
}

/// O `SetErrno` de `file_linux.cc`: o erro de um tipo inesperado.
fn type_error(tipo: i64) -> OsError {
    OsError::from_code(match tipo {
        TYPE_DIRECTORY => os_code::IS_DIRECTORY,
        TYPE_NOT_FOUND => os_code::NOT_FOUND,
        _ => os_code::INVALID,
    })
}

/// Arquivo comum, soquete ou pipe: o que `Delete`/`Rename`/`Copy` aceitam.
fn is_file_type(tipo: i64) -> bool {
    matches!(tipo, TYPE_FILE | TYPE_SOCKET | TYPE_PIPE)
}

/// `File::Open`: só arquivos comuns, dispositivos de caractere e pipes; o
/// modo de acréscimo posiciona no fim.
fn open_file(p: &std::path::Path, dart_mode: i64) -> IoResult<i64> {
    let Some((write_side, write_only, truncate_file)) = open_mode(dart_mode) else {
        return Err(OsError::invalid_argument());
    };
    if let Ok(m) = std::fs::metadata(p) {
        let t = m.file_type();
        #[cfg(unix)]
        let accepted = {
            use std::os::unix::fs::FileTypeExt;
            t.is_file() || t.is_char_device() || t.is_fifo()
        };
        #[cfg(windows)]
        let accepted = !t.is_dir();
        if !accepted {
            return Err(OsError::from_code(if t.is_dir() { os_code::IS_DIRECTORY } else { os_code::NOT_FOUND }));
        }
    }
    let mut o = std::fs::OpenOptions::new();
    if write_side {
        o.read(true).write(true).create(true);
    } else if write_only {
        o.write(true).create(true);
    } else {
        o.read(true);
    }
    if truncate_file {
        o.truncate(true);
    }
    let mut f = o.open(p)?;
    if (write_side || write_only) && !truncate_file {
        use std::io::Seek;
        f.seek(std::io::SeekFrom::End(0))?;
    }
    Ok(file_descriptor(f))
}

/// `File::Exists`: tudo que não é diretório nem link é arquivo para o Dart.
fn file_exists(p: &std::path::Path) -> bool {
    std::fs::metadata(p).is_ok_and(|m| !m.is_dir() && !m.file_type().is_symlink())
}

/// `File::Create`: cria (ou, sem `exclusive`, aceita) o arquivo; um
/// diretório ou link no caminho é erro.
fn create_file(p: &std::path::Path, exclusive: bool) -> IoResult<()> {
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
        o.read(true).custom_flags(O_CREAT | if exclusive { O_EXCL } else { 0 });
    }
    #[cfg(windows)]
    {
        o.read(true).write(true);
        if exclusive {
            o.create_new(true);
        } else {
            o.create(true);
        }
    }
    let f = o.open(p)?;
    let m = f.metadata()?;
    if m.is_dir() {
        return Err(OsError::from_code(os_code::IS_DIRECTORY));
    }
    if m.file_type().is_symlink() {
        return Err(OsError::from_code(os_code::NOT_FOUND));
    }
    Ok(())
}

/// `File::CreateLink`.
fn create_link(name: &std::path::Path, target: &std::path::Path) -> IoResult<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, name)?;
    }
    #[cfg(windows)]
    {
        // Como a VM (`file_win.cc`): link de diretório se o alvo, resolvido
        // a partir da pasta do link, é um diretório.
        let base = name.parent().map(|d| d.join(target)).unwrap_or_else(|| target.to_path_buf());
        if std::fs::metadata(&base).is_ok_and(|m| m.is_dir()) {
            std::os::windows::fs::symlink_dir(target, name)?;
        } else {
            std::os::windows::fs::symlink_file(target, name)?;
        }
    }
    Ok(())
}

/// `File::Delete`.
fn delete_file(p: &std::path::Path) -> IoResult<()> {
    let tipo = path_type(p, true);
    if !is_file_type(tipo) {
        return Err(type_error(tipo));
    }
    Ok(std::fs::remove_file(p)?)
}

/// `File::DeleteLink`.
fn delete_link(p: &std::path::Path) -> IoResult<()> {
    let tipo = path_type(p, false);
    if tipo != TYPE_LINK {
        return Err(type_error(tipo));
    }
    #[cfg(windows)]
    if std::fs::metadata(p).is_ok_and(|m| m.is_dir()) {
        return Ok(std::fs::remove_dir(p)?);
    }
    Ok(std::fs::remove_file(p)?)
}

/// `File::Rename`.
fn rename_file(de: &std::path::Path, to: &std::path::Path) -> IoResult<()> {
    let tipo = path_type(de, true);
    if !is_file_type(tipo) {
        return Err(type_error(tipo));
    }
    Ok(std::fs::rename(de, to)?)
}

/// `File::RenameLink`.
fn rename_link(de: &std::path::Path, to: &std::path::Path) -> IoResult<()> {
    let tipo = path_type(de, false);
    if tipo != TYPE_LINK {
        return Err(type_error(tipo));
    }
    Ok(std::fs::rename(de, to)?)
}

/// `File::Copy`: o destino é criado com o modo da origem; se a cópia
/// falha, o destino parcial é apagado.
fn copy_file(de: &std::path::Path, to: &std::path::Path) -> IoResult<()> {
    let tipo = path_type(de, true);
    if !is_file_type(tipo) {
        return Err(type_error(tipo));
    }
    let mut origin = std::fs::File::open(de)?;
    let mut o = std::fs::OpenOptions::new();
    o.write(true).truncate(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        o.mode(origin.metadata()?.permissions().mode());
    }
    let mut dest = o.open(to)?;
    // O `std::io::copy` entre dois `File` usa `copy_file_range`/`sendfile`
    // no Linux (o `sendfile64` da VM) e cai para leitura e escrita.
    if let Err(e) = std::io::copy(&mut origin, &mut dest) {
        drop(dest);
        let _ = std::fs::remove_file(to);
        return Err(OsError::de(&e));
    }
    Ok(())
}

/// O `StatHelper` de `file_linux.cc`: os metadados, com diretório como erro.
fn file_metadata(p: &std::path::Path) -> IoResult<std::fs::Metadata> {
    let m = std::fs::metadata(p)?;
    if m.is_dir() {
        return Err(OsError::from_code(os_code::IS_DIRECTORY));
    }
    Ok(m)
}

/// Segundos inteiros de um instante (o `st_mtime` da VM).
fn seconds_of(t: std::time::SystemTime) -> i64 {
    match t.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => -(e.duration().as_secs_f64().ceil() as i64),
    }
}

/// Milissegundos de um instante (o `TimespecToMilliseconds` da VM).
fn millis_of(t: std::time::SystemTime) -> i64 {
    match t.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_millis() as i64,
        Err(e) => -(e.duration().as_millis() as i64),
    }
}

fn instant_from_millis(ms: i64) -> std::time::SystemTime {
    let d = std::time::Duration::from_millis(ms.unsigned_abs());
    if ms >= 0 { std::time::UNIX_EPOCH + d } else { std::time::UNIX_EPOCH - d }
}

/// `File::LastModified` / `LastAccessed`, em milissegundos (a VM multiplica
/// os segundos inteiros por 1000).
fn last_modified(p: &std::path::Path) -> IoResult<i64> {
    Ok(seconds_of(file_metadata(p)?.modified()?) * 1000)
}

fn last_accessed(p: &std::path::Path) -> IoResult<i64> {
    Ok(seconds_of(file_metadata(p)?.accessed()?) * 1000)
}

/// `File::SetLastModified` / `SetLastAccessed`: troca um instante e mantém o
/// outro.
fn change_instants(p: &std::path::Path, access: Option<i64>, modification: Option<i64>) -> IoResult<()> {
    let m = file_metadata(p)?;
    let tempos = std::fs::FileTimes::new()
        .set_accessed(access.map_or_else(|| m.accessed(), |ms| Ok(instant_from_millis(ms)))?)
        .set_modified(modification.map_or_else(|| m.modified(), |ms| Ok(instant_from_millis(ms)))?);
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

/// `File::Stat`: `[tipo, criação, modificação, access, mode, size]`.
fn statistics(p: &std::path::Path) -> IoResult<[i64; 6]> {
    let m = std::fs::metadata(p)?;
    let tipo = metadata_kind(&m);
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
        let read_write = if m.permissions().readonly() { 0o444 } else { 0o666 };
        let mode = if m.is_dir() { 0o040000 | read_write | 0o111 } else { 0o100000 | read_write };
        let ms = |t: std::io::Result<std::time::SystemTime>| t.map_or(0, millis_of);
        Ok([tipo, ms(m.created()), ms(m.modified()), ms(m.accessed()), mode, m.len() as i64])
    }
}

/// `File::LinkTarget`.
fn link_target(p: &std::path::Path) -> IoResult<Vec<u8>> {
    let m = std::fs::symlink_metadata(p)?;
    if !m.file_type().is_symlink() {
        return Err(OsError::from_code(os_code::NOT_FOUND));
    }
    Ok(path_bytes_of(&std::fs::read_link(p)?))
}

/// `File::GetCanonicalPath` (`realpath`).
fn canonical_path(p: &std::path::Path) -> IoResult<Vec<u8>> {
    let c = std::fs::canonicalize(p)?;
    let b = path_bytes_of(&c);
    // No Windows, o `std` devolve o caminho estendido (`\\?\C:\…`); a VM
    // tira o prefixo.
    #[cfg(windows)]
    if let Some(without) = b.strip_prefix(br"\\?\") {
        if !without.starts_with(b"UNC\\") {
            return Ok(without.to_vec());
        }
    }
    Ok(b)
}

/// `File::AreIdentical`: o mesmo dispositivo e o mesmo nó (sem seguir
/// links).
#[cfg(unix)]
fn identical(a: &std::path::Path, b: &std::path::Path) -> IoResult<bool> {
    use std::os::unix::fs::MetadataExt;
    let ma = std::fs::symlink_metadata(a)?;
    let mb = std::fs::symlink_metadata(b)?;
    Ok(ma.dev() == mb.dev() && ma.ino() == mb.ino())
}
#[cfg(windows)]
fn identical(a: &std::path::Path, b: &std::path::Path) -> IoResult<bool> {
    #[repr(C)]
    #[derive(Default)]
    struct InfoKind {
        attributes: u32,
        tempos: [u32; 6],
        volume: u32,
        size_high: u32,
        size_low: u32,
        links: u32,
        index_high: u32,
        index_low: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileInformationByHandle(h: *mut std::ffi::c_void, i: *mut InfoKind) -> i32;
    }
    fn identidade(p: &std::path::Path) -> IoResult<(u32, u32, u32)> {
        use std::os::windows::fs::OpenOptionsExt;
        use std::os::windows::io::AsRawHandle;
        // FILE_FLAG_BACKUP_SEMANTICS abre diretórios; sem seguir o link.
        let f = std::fs::OpenOptions::new().access_mode(0).custom_flags(0x0200_0000 | 0x0020_0000).open(p)?;
        let mut i = InfoKind::default();
        // SAFETY: o handle é válido enquanto `f` vive.
        if unsafe { GetFileInformationByHandle(f.as_raw_handle(), &mut i) } == 0 {
            return Err(OsError::de(&std::io::Error::last_os_error()));
        }
        Ok((i.volume, i.index_high, i.index_low))
    }
    Ok(identidade(a)? == identidade(b)?)
}

/// `File::GetStdioHandleType`: 0 terminal, 1 pipe, 2 arquivo, 3 soquete,
/// 4 outro.
#[cfg(unix)]
fn stdio_kind(fd: i64) -> IoResult<i64> {
    use std::os::unix::fs::FileTypeExt;
    let f = std::mem::ManuallyDrop::new(file_from_descriptor(fd));
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
fn stdio_kind(fd: i64) -> IoResult<i64> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileType(h: *mut std::ffi::c_void) -> u32;
    }
    let h = default_descriptor(fd);
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
    dart_bool(file_exists(&raw_path(raw)))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Create(_ns: i64, raw: i64, exclusive: u8) -> i64 {
    dart_true_or_error(create_file(&raw_path(raw), exclusive != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_CreateLink(_ns: i64, raw: i64, target: i64) -> i64 {
    dart_null_or_error(create_link(&raw_path(raw), &path_text(target)))
}

/// `File_CreatePipe`: `[pointer de read_side, pointer de write_side]`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_CreatePipe(_ns: i64) -> i64 {
    match create_pipe() {
        Ok((read_side, write_side)) => {
            let a = dart_int(NativeFile::new(read_side));
            let b = with_roots(&[a], || dart_int(NativeFile::new(write_side)));
            with_roots(&[a, b], || dart_fixed_list(&[a, b]))
        }
        Err(e) => e.to_dart(),
    }
}

/// Um pipe anônimo: (leitura, escrita).
#[cfg(unix)]
fn create_pipe() -> IoResult<(i64, i64)> {
    let (r, w) = std::io::pipe()?;
    use std::os::unix::io::IntoRawFd;
    Ok((i64::from(r.into_raw_fd()), i64::from(w.into_raw_fd())))
}
#[cfg(windows)]
fn create_pipe() -> IoResult<(i64, i64)> {
    let (r, w) = std::io::pipe()?;
    use std::os::windows::io::IntoRawHandle;
    Ok((r.into_raw_handle() as usize as i64, w.into_raw_handle() as usize as i64))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_LinkTarget(_ns: i64, raw: i64) -> i64 {
    dart_or_error(link_target(&raw_path(raw)), |b| dart_string_from_bytes(&b))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Delete(_ns: i64, raw: i64) -> i64 {
    dart_true_or_error(delete_file(&raw_path(raw)))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_DeleteLink(_ns: i64, raw: i64) -> i64 {
    dart_true_or_error(delete_link(&raw_path(raw)))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Rename(_ns: i64, raw: i64, new: i64) -> i64 {
    dart_true_or_error(rename_file(&raw_path(raw), &path_text(new)))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_RenameLink(_ns: i64, raw: i64, new: i64) -> i64 {
    dart_true_or_error(rename_link(&raw_path(raw), &path_text(new)))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Copy(_ns: i64, raw: i64, new: i64) -> i64 {
    dart_true_or_error(copy_file(&raw_path(raw), &path_text(new)))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_LengthFromPath(_ns: i64, raw: i64) -> i64 {
    dart_or_error(file_metadata(&raw_path(raw)), |m| dart_int(m.len() as i64))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_LastModified(_ns: i64, raw: i64) -> i64 {
    dart_or_error(last_modified(&raw_path(raw)), dart_int)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_LastAccessed(_ns: i64, raw: i64) -> i64 {
    dart_or_error(last_accessed(&raw_path(raw)), dart_int)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_SetLastModified(_ns: i64, raw: i64, ms: i64) -> i64 {
    dart_null_or_error(change_instants(&raw_path(raw), None, Some(ms)))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_SetLastAccessed(_ns: i64, raw: i64, ms: i64) -> i64 {
    dart_null_or_error(change_instants(&raw_path(raw), Some(ms), None))
}

/// `File_Open`: o ponteiro do arquivo aberto (um `int`) ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Open(_ns: i64, raw: i64, mode: i64) -> i64 {
    dart_or_error(open_file(&raw_path(raw), mode), |d| dart_int(NativeFile::new(d)))
}

/// `File_OpenStdio`: o arquivo da entrada/saída padrão `fd`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_OpenStdio(fd: i64) -> i64 {
    NativeFile::new(default_descriptor(fd))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_GetStdioHandleType(fd: i64) -> i64 {
    dart_or_error(stdio_kind(fd), dart_int)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_GetType(_ns: i64, raw: i64, follow: u8) -> i64 {
    dart_int(path_type(&raw_path(raw), follow != 0))
}

/// `File_Stat`: a `Int64List` de [`statistics`], ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Stat(_ns: i64, file_path: i64) -> i64 {
    dart_or_error(statistics(&path_text(file_path)), |d| dart_int64s(&d))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_AreIdentical(_ns: i64, a: i64, b: i64) -> i64 {
    dart_or_error(identical(&path_text(a), &path_text(b)), dart_bool)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_ResolveSymbolicLinks(_ns: i64, raw: i64) -> i64 {
    dart_or_error(canonical_path(&raw_path(raw)), |b| dart_string_from_bytes(&b))
}

// ---------------------------------------------------------------------------
// Os natives de `_RandomAccessFileOpsImpl` (instância, sobre o arquivo do
// campo nativo).

/// O arquivo do receptor, ou lança o erro interno da VM ("No native peer").
fn receiver_file<'a>(this: i64) -> Option<&'a NativeFile> {
    let p = native_field(this);
    if p == 0 {
        throw_internal_error("No native peer");
        return None;
    }
    // SAFETY: o campo guarda a referência do objeto, viva até `close` ou a
    // coleta dele.
    Some(unsafe { NativeFile::de(p) })
}

/// `File_SetPointer`: o objeto passa a ser dono da referência de `p`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_SetPointer(this: i64, p: i64) {
    attach_finalizer(this, free_object_file, p as usize);
    write_native_field(this, p);
}

/// `File_GetPointer`: retém uma referência para o IOService (0 se fechado).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_GetPointer(this: i64) -> i64 {
    let p = native_field(this);
    if p != 0 {
        NativeFile::retain_ref(p);
    }
    p
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_GetFD(this: i64) -> i64 {
    receiver_file(this).map_or(0, |a| a.descriptor())
}

/// `File_Close`: 0, ou -1 se já fechado.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Close(this: i64) -> i64 {
    let p = native_field(this);
    if p == 0 {
        return -1;
    }
    // SAFETY: a referência do objeto está viva.
    unsafe { NativeFile::de(p) }.do_close();
    remove_finalizer(this);
    NativeFile::free(p);
    write_native_field(this, 0);
    0
}

/// `File_ReadByte`: o byte, -1 no fim, ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_ReadByte(this: i64) -> i64 {
    let Some(a) = receiver_file(this) else { return 0 };
    let mut b = [0u8; 1];
    dart_or_error(a.do_read(&mut b), |n| dart_int(if n == 1 { i64::from(b[0]) } else { -1 }))
}

/// `File_WriteByte`: 1 ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_WriteByte(this: i64, value: i64) -> i64 {
    let Some(a) = receiver_file(this) else { return 0 };
    let Some(v) = HEAP.with(|h| h.borrow().int_of(value)) else {
        return OsError::invalid_argument().to_dart();
    };
    dart_or_error(a.write_all_bytes(&[v as u8]), |()| dart_int(1))
}

/// `File_Read`: os bytes lidos (uma visão, como o `_makeUint8ListView` da
/// VM, se vieram menos que o pedido) ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Read(this: i64, n: i64) -> i64 {
    let Some(a) = receiver_file(this) else { return 0 };
    if n < 0 {
        return OsError::invalid_argument().to_dart();
    }
    let mut buf = vec![0u8; n as usize];
    match a.do_read(&mut buf) {
        Ok(read_list) if read_list as i64 == n => dart_bytes(buf),
        Ok(read_list) => {
            buf.truncate(read_list);
            let base = dart_bytes(buf);
            let cid = crate::layout::cid::visao(TYPE_UINT8, false);
            with_roots(&[base], || dartforge_view_nova(i64::from(cid), i64::from(TYPE_UINT8), base, 0, read_list as i64))
        }
        Err(e) => e.to_dart(),
    }
}

/// `File_ReadInto(buffer, start, end)`: os bytes lidos, gravados em
/// `buffer[start..]`, ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_ReadInto(this: i64, buffer: i64, start: i64, end: i64) -> i64 {
    let Some(a) = receiver_file(this) else { return 0 };
    let end = HEAP.with(|h| h.borrow().int_of(end)).unwrap_or(start);
    let mut buf = vec![0u8; (end - start).max(0) as usize];
    match a.do_read(&mut buf) {
        Ok(read_list) => {
            write_bytes_to_list(buffer, start as usize, &buf[..read_list]);
            dart_int(read_list as i64)
        }
        Err(e) => e.to_dart(),
    }
}

/// Grava `bytes` em `list[start..]` (lista tipada ou `List<int>`, o
/// `Dart_ListSetAsBytes` da VM).
fn write_bytes_to_list(list: i64, start: usize, bytes: &[u8]) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        if let Some(t) = resolver(&heap, list) {
            // Só listas de bytes chegam aqui (`_ensureFastAndSerializable…`
            // nos patches); outra largura grava um byte por elemento.
            let step = tamanho_do_elemento(t.tipo);
            if let Some(dest) = heap.typed_bytes_mut(list) {
                for (i, &b) in bytes.iter().enumerate() {
                    let pos = (start + i) * step;
                    if pos < dest.len() {
                        dest[pos] = b;
                    }
                }
            }
            return;
        }
        // `List<int>`: o byte como `int` (sem alocação: cabe no `Smi`).
        for (i, &b) in bytes.iter().enumerate() {
            heap.list_set(list, start + i, crate::heap::Value::Int(i64::from(b)));
        }
    });
}

/// `File_WriteFrom(buffer, start, end)`: `null` ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_WriteFrom(this: i64, buffer: i64, start: i64, end: i64) -> i64 {
    let Some(a) = receiver_file(this) else { return 0 };
    let bytes = typed_list_bytes(buffer).unwrap_or_default();
    let end = HEAP.with(|h| h.borrow().int_of(end)).unwrap_or(bytes.len() as i64) as usize;
    let start = (start.max(0) as usize).min(end);
    dart_null_or_error(a.write_all_bytes(&bytes[start..end.min(bytes.len())]))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Position(this: i64) -> i64 {
    let Some(a) = receiver_file(this) else { return 0 };
    dart_or_error(a.cur_position(), dart_int)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_SetPosition(this: i64, p: i64) -> i64 {
    let Some(a) = receiver_file(this) else { return 0 };
    dart_true_or_error(a.set_position(p))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Truncate(this: i64, n: i64) -> i64 {
    let Some(a) = receiver_file(this) else { return 0 };
    dart_true_or_error(a.truncate_file(n))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Length(this: i64) -> i64 {
    let Some(a) = receiver_file(this) else { return 0 };
    dart_or_error(a.size(), dart_int)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Flush(this: i64) -> i64 {
    let Some(a) = receiver_file(this) else { return 0 };
    dart_true_or_error(a.sync_file())
}

/// `File_Lock(tipo, início, end)`: `true` ou o `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Lock(this: i64, tipo: i64, start: i64, end: i64) -> i64 {
    let Some(a) = receiver_file(this) else { return 0 };
    if !(LOCK_UNLOCK..=LOCK_EXCLUSIVE_BLOCKING).contains(&tipo) || start < 0 || (end != -1 && end <= start) {
        return OsError::invalid_argument().to_dart();
    }
    dart_true_or_error(a.lock_file(tipo, start, end))
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

#[cfg(test)]
mod tests_io_list_owners {
    use super::*;

    #[test]
    fn io_result_with_copies_survives_collection_and_releases_chain() {
        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
        let (frame, text) = HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.enable_arc();
            let frame = h.push_owning_frame(1);
            let text = h.allocate_str("resultado de I/O");
            h.set_root(frame, 0, text);
            (frame, text)
        });
        let list = dart_fixed_list(&[text, text]);
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.set_root(frame, 0, list);
            h.collect();
            assert!(h.is_live_object(text));
            assert_eq!(&h.words(list)[1..3], &[text, text]);
            h.pop_frame(frame);
            h.collect();
            assert!(!h.is_live_object(list) && !h.is_live_object(text));
        });
        HEAP.with(|h| { h.replace(previous); });
    }
}
