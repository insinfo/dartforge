// Native runtime: the VM's `dart:io` — files (`runtime/bin/file.cc`,
// `file_linux.cc`, `file_macos.cc`, `file_win.cc`), the native field of
// `NativeFieldWrapperClass1` and what the other `io_*` fragments use: system
// errors, paths and the Dart values that the natives return.
//
// The natives follow the VM's contract to the letter: what the VM returns in
// `Dart_SetReturnValue` (an `int`, `bool`, `String`, `Uint8List` or an `OSError`)
// is what the Dart code of the VM's patches (`file_patch.dart`,
// `directory_patch.dart`…) expects, without an overlay. The parameters arrive in
// the HIR representation (`int` → `i64`, `bool` → `u8`, the rest → `Ref`), and the
// `_Namespace` is ignored: native code only has the default namespace (the
// process's current directory), which is the only one that the `dart` embedder
// creates outside Fuchsia.
//
// An open file is a reference-counted [`NativeFile`], like the VM's `File`
// (`ReferenceCounted<File>`): the Dart object keeps the pointer in its native field
// and releases its reference when it is closed or collected (the finalizer of
// `Heap::finalizable`); `getPointer` retains a reference for the IOService, which
// releases it at the end of the request.

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering as AtomicOrdering};

// ---------------------------------------------------------------------------
// System errors.

/// The error codes that the natives assign on their own (the `SetErrno` of
/// `file_linux.cc`): errno on Unix, `GetLastError` on Windows.
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

/// The system message for code `c`, as the VM builds it.
///
/// On Windows it is the VM's `FormatMessageIntoBuffer` (`bin/utils_win.cc`): the
/// text of `FormatMessageW` as it comes, with the trailing CRLF (`std` removes it),
/// and `OS Error N` when the system has no message.
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
    // SAFETY: the buffer has the reported size; no inserts.
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

/// On Unix, `strerror` (`std`'s message without the " (os error N)").
#[cfg(not(windows))]
fn system_message(c: i32) -> String {
    let text = std::io::Error::from_raw_os_error(c).to_string();
    let suffix = format!(" (os error {c})");
    text.strip_suffix(&suffix).map(str::to_string).unwrap_or(text)
}

/// An operating system error, like the VM's `OSError` (`bin/utils.h`): the code and
/// the system message (`strerror_r`; `FormatMessage` on Windows).
#[derive(Clone, Debug)]
struct OsError {
    code: i64,
    message: String,
}

impl OsError {
    /// The error of system code `c`.
    fn from_code(c: i32) -> OsError {
        OsError { code: i64::from(c), message: system_message(c) }
    }

    /// The error of a `std` operation (which carries the system code).
    fn de(e: &std::io::Error) -> OsError {
        match e.raw_os_error() {
            Some(c) => OsError::from_code(c),
            None => OsError { code: -1, message: e.to_string() },
        }
    }

    /// The `OSError(-1, "Invalid argument", kUnknown)` of the VM's natives.
    fn invalid_argument() -> OsError {
        OsError { code: -1, message: "Invalid argument".to_string() }
    }

    /// Dart's `OSError` (`DartUtils::NewDartOSError`), through the `dart:io`
    /// overlay's function.
    fn to_dart(&self) -> i64 {
        let Some(f) = ajudante("_dartforgeOSError") else {
            panic!("compiler bug: dart:io without a registered `_dartforgeOSError`");
        };
        // SAFETY: registered by the overlay's `dart:io` library with the signature
        // (`String`, `int`) → `OSError`.
        let g = { let dart_target: usize = f; move |a0: i64, a1: i64| -> i64 { dart_r2(dart_target, a0, a1) } };
        let message = allocate_str(&self.message);
        with_roots(&[message], || g(message, self.code))
    }

    /// The IOService's error response (`CObject::NewOSError`):
    /// `[kOSError, code, message]`.
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
// Dart values returned by the natives.

fn dart_bool(b: bool) -> i64 {
    Heap::box_bool(b)
}

fn dart_int(i: i64) -> i64 {
    HEAP.with(|h| h.borrow_mut().box_int(i))
}

/// A Dart `String` from system bytes: UTF-8, with an invalid sequence replaced by
/// U+FFFD.
fn dart_string_from_bytes(b: &[u8]) -> i64 {
    allocate_str(&String::from_utf8_lossy(b))
}

/// An internal typed list of `tipo` with the bytes (the length in elements is
/// `bytes.len() / size`).
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

/// A `Uint8List` (`_Uint8List`) with the bytes.
fn dart_bytes(bytes: Vec<u8>) -> i64 {
    dart_typed_from_bytes(TYPE_UINT8, &bytes)
}

/// An `Int64List` (`_Int64List`) with the values.
fn dart_int64s(value_list: &[i64]) -> i64 {
    let bytes: Vec<u8> = value_list.iter().flat_map(|v| v.to_ne_bytes()).collect();
    dart_typed_from_bytes(TYPE_INT64, &bytes)
}

/// A fixed-length `_List` (cid 8, general) with the values (already in the `Ref`
/// position), rooted during the allocation.
fn dart_fixed_list(value_list: &[i64]) -> i64 {
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        let frame = h.push_owning_frame(value_list.len().max(1));
        for (i, &v) in value_list.iter().enumerate() {
            h.set_root(frame, i, v);
        }
        let l = h.new_list(crate::layout::cid::LIST, value_list.len(), crate::lists::Element::General);
        // Word 0 of the body is the length; the elements, the following ones.
        if !value_list.is_empty() {
            h.write_refs(l, 1, value_list);
        }
        // The list's edges replace the temporary owning copies.
        h.pop_frame(frame);
        l
    })
}

/// The Dart value of a result: the `Ok` through `f`, the error as an `OSError`.
fn dart_or_error<T>(r: IoResult<T>, f: impl FnOnce(T) -> i64) -> i64 {
    match r {
        Ok(v) => f(v),
        Err(e) => e.to_dart(),
    }
}

/// `true` or the `OSError` — the shape of almost every path native.
fn dart_true_or_error(r: IoResult<()>) -> i64 {
    dart_or_error(r, |()| dart_bool(true))
}

/// `null` or the `OSError`.
fn dart_null_or_error(r: IoResult<()>) -> i64 {
    dart_or_error(r, |()| 0)
}

/// Throws `StateError(message)` (the `_InternalError` of the VM's natives).
fn throw_internal_error(message: &str) {
    let e = allocate_state_error(message);
    with_roots(&[e], || dartforge_exception_throw(e, 3));
}

// ---------------------------------------------------------------------------
// Paths.

/// The bytes of a typed list (internal list or view).
fn typed_list_bytes(h: i64) -> Option<Vec<u8>> {
    HEAP.with(|heap| heap.borrow().typed_bytes(h).map(<[u8]>::to_vec))
}

/// The bytes of a core list of integers, each truncated to 8 bits (the VM's
/// `Dart_ListGetAsBytes`); empty if `h` is not a core list.
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

/// The path of a `rawPath` (a NUL-terminated UTF-8 `Uint8List`, the VM's
/// `DartUtils::GetNativeTypedDataArgument`): the bytes up to the NUL.
fn path_bytes(raw: i64) -> Vec<u8> {
    let mut b = typed_list_bytes(raw).unwrap_or_default();
    if let Some(end) = b.iter().position(|&c| c == 0) {
        b.truncate(end);
    }
    b
}

/// The system path for the bytes (Unix: the bytes; Windows: the UTF-8 converted to
/// UTF-16, like the VM's `Utf8ToWideChar`).
#[cfg(unix)]
fn path_from_bytes(b: &[u8]) -> std::path::PathBuf {
    use std::os::unix::ffi::OsStrExt;
    std::path::PathBuf::from(std::ffi::OsStr::from_bytes(b))
}
#[cfg(windows)]
fn path_from_bytes(b: &[u8]) -> std::path::PathBuf {
    std::path::PathBuf::from(String::from_utf8_lossy(b).into_owned())
}

/// The bytes of a system path (the inverse of [`path_from_bytes`]).
#[cfg(unix)]
fn path_bytes_of(p: &std::path::Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    p.as_os_str().as_bytes().to_vec()
}
#[cfg(windows)]
fn path_bytes_of(p: &std::path::Path) -> Vec<u8> {
    p.to_string_lossy().into_owned().into_bytes()
}

/// The path of a Dart `rawPath`.
fn raw_path(raw: i64) -> std::path::PathBuf {
    path_from_bytes(&path_bytes(raw))
}

/// The path of a Dart `String` (`DartUtils::GetNativeStringArgument`: UTF-8).
fn path_text(s: i64) -> std::path::PathBuf {
    path_from_bytes(&utf8_of_string(s))
}

/// The UTF-8 of a Dart `String` (a lone surrogate becomes U+FFFD, as in the VM's
/// `Dart_StringToUTF8`).
fn utf8_of_string(s: i64) -> Vec<u8> {
    if s == 0 {
        return Vec::new();
    }
    HEAP.with(|h| h.borrow().text(s).map(|t| t.to_vm_utf8()).unwrap_or_default())
}

// ---------------------------------------------------------------------------
// The native field of `NativeFieldWrapperClass1` (the `dart:nativewrappers`
// overlay declares it as the first field of the layout).

fn native_field(obj: i64) -> i64 {
    HEAP.with(|h| match h.borrow().object(obj) {
        Some(o) => o.first().map_or(0, |f| f.0),
        None => panic!("compiler bug: native field of something that is not an object"),
    })
}

fn write_native_field(obj: i64, value: i64) {
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        if h.object(obj).is_none_or(|o| o.is_empty()) {
            panic!("compiler bug: native field of something that is not an object");
        }
        h.set_field(obj, 0, value, false);
    });
}

/// Binds `pair` to the object: when the object is collected, `finalizer(pair)` (the
/// VM's `Dart_NewFinalizableHandle`).
fn attach_finalizer(obj: i64, finalizer: fn(usize), pair: usize) {
    HEAP.with(|h| h.borrow_mut().finalizable.insert(obj, (finalizer, pair)));
}

fn remove_finalizer(obj: i64) {
    HEAP.with(|h| h.borrow_mut().finalizable.remove(&obj));
}

// ---------------------------------------------------------------------------
// The open file.

/// The descriptor of a closed file (`File::kClosedFd`).
const CLOSED_DESCRIPTOR: i64 = -1;

/// An open file (the `File` of `bin/file.h`): the system descriptor (Unix) or the
/// `HANDLE` (Windows). The isolate and the IOService threads share it; the Dart
/// code of the patches serializes the operations on the same file
/// (`_RandomAccessFile._checkAvailable`).
struct NativeFile {
    descriptor: AtomicI64,
}

impl NativeFile {
    /// Creates the file with one reference and returns its pointer.
    fn new(descriptor: i64) -> i64 {
        Arc::into_raw(Arc::new(NativeFile { descriptor: AtomicI64::new(descriptor) })) as i64
    }

    /// The file of pointer `p`, borrowed from whoever has the reference.
    ///
    /// # Safety
    /// `p` came from [`NativeFile::new`] and the reference still exists.
    unsafe fn de<'a>(p: i64) -> &'a NativeFile {
        // SAFETY: guaranteed by the caller.
        unsafe { &*(p as *const NativeFile) }
    }

    /// One more reference (`Retain`).
    fn retain_ref(p: i64) {
        // SAFETY: `p` is a live pointer from `new`.
        unsafe { Arc::increment_strong_count(p as *const NativeFile) };
    }

    /// Releases a reference (`Release`); the last one closes the file.
    fn free(p: i64) {
        // SAFETY: `p` is a live pointer from `new`, with this reference.
        unsafe { Arc::decrement_strong_count(p as *const NativeFile) };
    }

    /// Takes the reference of `p` (the VM's `RefCntReleaseScope`: released when
    /// leaving the scope).
    ///
    /// # Safety
    /// `p` is a pointer from `new` whose reference passes to the caller.
    unsafe fn take(p: i64) -> Arc<NativeFile> {
        // SAFETY: guaranteed by the caller.
        unsafe { Arc::from_raw(p as *const NativeFile) }
    }

    fn closed_flag(&self) -> bool {
        self.descriptor.load(AtomicOrdering::Acquire) == CLOSED_DESCRIPTOR
    }

    fn descriptor(&self) -> i64 {
        self.descriptor.load(AtomicOrdering::Acquire)
    }

    /// Operates on the file through `std::fs::File`, without transferring the descriptor.
    fn com<R>(&self, f: impl FnOnce(&mut std::fs::File) -> std::io::Result<R>) -> IoResult<R> {
        let d = self.descriptor();
        if d == CLOSED_DESCRIPTOR {
            return Err(OsError::from_code(os_code::INVALID));
        }
        let mut file = std::mem::ManuallyDrop::new(file_from_descriptor(d));
        f(&mut file).map_err(OsError::from)
    }

    /// `File::Read`: one read (repeated only on interruption by a signal).
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
            // `print`'s output goes through `std`'s `stdout`: whatever it has
            // buffered goes out first, so that the order is the program's.
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

    /// `File::Flush`: `fsync` (Unix; `std`'s `sync_all` on macOS would be
    /// `F_FULLFSYNC`, which the VM does not use) or `FlushFileBuffers` (Windows).
    fn sync_file(&self) -> IoResult<()> {
        #[cfg(unix)]
        {
            unsafe extern "C" {
                fn fsync(fd: i32) -> i32;
            }
            let d = self.descriptor();
            // SAFETY: `d` is an open descriptor of this file.
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

    /// `File::Lock`: `fcntl(F_SETLK[W])` on Unix, `LockFileEx` on Windows.
    fn lock_file(&self, tipo: i64, start: i64, end: i64) -> IoResult<()> {
        lock_descriptor(self.descriptor(), tipo, start, end)
    }

    /// `File::Close`: closes the descriptor. Standard output is not closed: it
    /// becomes `/dev/null` (the VM does the same, so that the process does not write
    /// to a descriptor that another `open` would reuse).
    fn do_close(&self) {
        let d = self.descriptor.swap(CLOSED_DESCRIPTOR, AtomicOrdering::AcqRel);
        if d == CLOSED_DESCRIPTOR {
            return;
        }
        close_descriptor(d);
    }
}

impl Drop for NativeFile {
    /// The destructor of the VM's `File`: closes whatever was left open, except
    /// standard output and standard error.
    fn drop(&mut self) {
        let d = self.descriptor();
        if d != CLOSED_DESCRIPTOR && !is_stdout(d) {
            self.do_close();
        }
    }
}

/// The finalizer of the Dart object that owns a file.
fn free_object_file(pair: usize) {
    NativeFile::free(pair as i64);
}

#[cfg(unix)]
fn file_from_descriptor(d: i64) -> std::fs::File {
    use std::os::unix::io::FromRawFd;
    // SAFETY: `d` is an open descriptor; the caller does not close it through `File`.
    unsafe { std::fs::File::from_raw_fd(d as i32) }
}
#[cfg(windows)]
fn file_from_descriptor(d: i64) -> std::fs::File {
    use std::os::windows::io::FromRawHandle;
    // SAFETY: `d` is an open HANDLE; the caller does not close it through `File`.
    unsafe { std::fs::File::from_raw_handle(d as usize as *mut std::ffi::c_void) }
}

/// The descriptor of a `std::fs::File` (which stops owning it).
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
        // SAFETY: a constant C path; the descriptors are valid.
        unsafe {
            let null_val = open(c"/dev/null".as_ptr(), 1 /* O_WRONLY */);
            if null_val >= 0 {
                dup2(null_val, 1);
                close(null_val);
            }
        }
        return;
    }
    // SAFETY: `d` is an open descriptor of this file, closed once.
    unsafe { close(d as i32) };
}
#[cfg(windows)]
fn close_descriptor(d: i64) {
    if is_stdout(d) || d == default_descriptor(0) {
        return;
    }
    drop(file_from_descriptor(d));
}

/// The descriptor of standard input/output `fd` (0, 1, 2).
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
    // SAFETY: only reads the process's handle.
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
        // SAFETY: `fl` points to a valid platform `struct flock`.
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
    // SAFETY: `h` is the file's open HANDLE; `o` lives during the call.
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
// The operations on paths (the `File::*` of `file_linux.cc`), shared by the
// synchronous natives and the IOService.

/// `File::Type`.
const TYPE_FILE: i64 = 0;
const TYPE_DIRECTORY: i64 = 1;
const TYPE_LINK: i64 = 2;
const TYPE_SOCKET: i64 = 3;
const TYPE_PIPE: i64 = 4;
const TYPE_NOT_FOUND: i64 = 5;

/// `File::DartFileOpenMode` → (read/write, write only, truncate).
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

/// The type of a file system entry (`File::Type`).
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

/// The `SetErrno` of `file_linux.cc`: the error of an unexpected type.
fn type_error(tipo: i64) -> OsError {
    OsError::from_code(match tipo {
        TYPE_DIRECTORY => os_code::IS_DIRECTORY,
        TYPE_NOT_FOUND => os_code::NOT_FOUND,
        _ => os_code::INVALID,
    })
}

/// A regular file, socket or pipe: what `Delete`/`Rename`/`Copy` accept.
fn is_file_type(tipo: i64) -> bool {
    matches!(tipo, TYPE_FILE | TYPE_SOCKET | TYPE_PIPE)
}

/// `File::Open`: only regular files, character devices and pipes; append mode
/// positions at the end.
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

/// `File::Exists`: everything that is neither a directory nor a link is a file for Dart.
fn file_exists(p: &std::path::Path) -> bool {
    std::fs::metadata(p).is_ok_and(|m| !m.is_dir() && !m.file_type().is_symlink())
}

/// `File::Create`: creates (or, without `exclusive`, accepts) the file; a directory
/// or link at the path is an error.
fn create_file(p: &std::path::Path, exclusive: bool) -> IoResult<()> {
    let mut o = std::fs::OpenOptions::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // `O_RDONLY | O_CREAT [| O_EXCL]`, like the VM: `std` only accepts
        // `create` with write access.
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
        // Like the VM (`file_win.cc`): a directory link if the target, resolved from
        // the link's folder, is a directory.
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

/// `File::Copy`: the destination is created with the source's mode; if the copy
/// fails, the partial destination is deleted.
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
    // `std::io::copy` between two `File`s uses `copy_file_range`/`sendfile` on
    // Linux (the VM's `sendfile64`) and falls back to reading and writing.
    if let Err(e) = std::io::copy(&mut origin, &mut dest) {
        drop(dest);
        let _ = std::fs::remove_file(to);
        return Err(OsError::de(&e));
    }
    Ok(())
}

/// The `StatHelper` of `file_linux.cc`: the metadata, with a directory as an error.
fn file_metadata(p: &std::path::Path) -> IoResult<std::fs::Metadata> {
    let m = std::fs::metadata(p)?;
    if m.is_dir() {
        return Err(OsError::from_code(os_code::IS_DIRECTORY));
    }
    Ok(m)
}

/// Whole seconds of an instant (the VM's `st_mtime`).
fn seconds_of(t: std::time::SystemTime) -> i64 {
    match t.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => -(e.duration().as_secs_f64().ceil() as i64),
    }
}

/// Milliseconds of an instant (the VM's `TimespecToMilliseconds`).
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

/// `File::LastModified` / `LastAccessed`, in milliseconds (the VM multiplies the
/// whole seconds by 1000).
fn last_modified(p: &std::path::Path) -> IoResult<i64> {
    Ok(seconds_of(file_metadata(p)?.modified()?) * 1000)
}

fn last_accessed(p: &std::path::Path) -> IoResult<i64> {
    Ok(seconds_of(file_metadata(p)?.accessed()?) * 1000)
}

/// `File::SetLastModified` / `SetLastAccessed`: changes one instant and keeps the
/// other.
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

/// `File::Stat`: `[type, creation, modification, access, mode, size]`.
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
        // The `st_mode` of `_wstat64`: type, read and write repeated for group and
        // others, execution on directories.
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
    // On Windows, `std` returns the extended path (`\\?\C:\…`); the VM removes the
    // prefix.
    #[cfg(windows)]
    if let Some(without) = b.strip_prefix(br"\\?\") {
        if !without.starts_with(b"UNC\\") {
            return Ok(without.to_vec());
        }
    }
    Ok(b)
}

/// `File::AreIdentical`: the same device and the same node (without following
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
        // FILE_FLAG_BACKUP_SEMANTICS opens directories; without following the link.
        let f = std::fs::OpenOptions::new().access_mode(0).custom_flags(0x0200_0000 | 0x0020_0000).open(p)?;
        let mut i = InfoKind::default();
        // SAFETY: the handle is valid while `f` lives.
        if unsafe { GetFileInformationByHandle(f.as_raw_handle(), &mut i) } == 0 {
            return Err(OsError::de(&std::io::Error::last_os_error()));
        }
        Ok((i.volume, i.index_high, i.index_low))
    }
    Ok(identidade(a)? == identidade(b)?)
}

/// `File::GetStdioHandleType`: 0 terminal, 1 pipe, 2 file, 3 socket, 4 other.
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
    // SAFETY: queries the type of a process handle.
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
// The `_File` natives (static, on paths).

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

/// `File_CreatePipe`: `[pointer of read_side, pointer of write_side]`.
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

/// An anonymous pipe: (read, write).
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

/// `File_Open`: the pointer of the open file (an `int`) or the `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Open(_ns: i64, raw: i64, mode: i64) -> i64 {
    dart_or_error(open_file(&raw_path(raw), mode), |d| dart_int(NativeFile::new(d)))
}

/// `File_OpenStdio`: the file of standard input/output `fd`.
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

/// `File_Stat`: the `Int64List` of [`statistics`], or the `OSError`.
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
// The `_RandomAccessFileOpsImpl` natives (instance, on the file of the native
// field).

/// The receiver's file, or throws the VM's internal error ("No native peer").
fn receiver_file<'a>(this: i64) -> Option<&'a NativeFile> {
    let p = native_field(this);
    if p == 0 {
        throw_internal_error("No native peer");
        return None;
    }
    // SAFETY: the field holds the object's reference, alive until `close` or the
    // object's collection.
    Some(unsafe { NativeFile::de(p) })
}

/// `File_SetPointer`: the object becomes the owner of the reference of `p`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_SetPointer(this: i64, p: i64) {
    attach_finalizer(this, free_object_file, p as usize);
    write_native_field(this, p);
}

/// `File_GetPointer`: retains a reference for the IOService (0 if closed).
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

/// `File_Close`: 0, or -1 if already closed.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Close(this: i64) -> i64 {
    let p = native_field(this);
    if p == 0 {
        return -1;
    }
    // SAFETY: the object's reference is alive.
    unsafe { NativeFile::de(p) }.do_close();
    remove_finalizer(this);
    NativeFile::free(p);
    write_native_field(this, 0);
    0
}

/// `File_ReadByte`: the byte, -1 at the end, or the `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_ReadByte(this: i64) -> i64 {
    let Some(a) = receiver_file(this) else { return 0 };
    let mut b = [0u8; 1];
    dart_or_error(a.do_read(&mut b), |n| dart_int(if n == 1 { i64::from(b[0]) } else { -1 }))
}

/// `File_WriteByte`: 1 or the `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_WriteByte(this: i64, value: i64) -> i64 {
    let Some(a) = receiver_file(this) else { return 0 };
    let Some(v) = HEAP.with(|h| h.borrow().int_of(value)) else {
        return OsError::invalid_argument().to_dart();
    };
    dart_or_error(a.write_all_bytes(&[v as u8]), |()| dart_int(1))
}

/// `File_Read`: the bytes read (a view, like the VM's `_makeUint8ListView`, if fewer
/// than requested came) or the `OSError`.
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

/// `File_ReadInto(buffer, start, end)`: the bytes read, written into
/// `buffer[start..]`, or the `OSError`.
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

/// Writes `bytes` into `list[start..]` (a typed list or a `List<int>`, the VM's
/// `Dart_ListSetAsBytes`).
fn write_bytes_to_list(list: i64, start: usize, bytes: &[u8]) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        if let Some(t) = resolver(&heap, list) {
            // Only byte lists arrive here (`_ensureFastAndSerializable…` in the
            // patches); another width writes one byte per element.
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
        // `List<int>`: the byte as an `int` (no allocation: it fits in a `Smi`).
        for (i, &b) in bytes.iter().enumerate() {
            heap.list_set(list, start + i, crate::heap::Value::Int(i64::from(b)));
        }
    });
}

/// `File_WriteFrom(buffer, start, end)`: `null` or the `OSError`.
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

/// `File_Lock(type, start, end)`: `true` or the `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_File_Lock(this: i64, tipo: i64, start: i64, end: i64) -> i64 {
    let Some(a) = receiver_file(this) else { return 0 };
    if !(LOCK_UNLOCK..=LOCK_EXCLUSIVE_BLOCKING).contains(&tipo) || start < 0 || (end != -1 && end <= start) {
        return OsError::invalid_argument().to_dart();
    }
    dart_true_or_error(a.lock_file(tipo, start, end))
}

// ---------------------------------------------------------------------------
// `_NamespaceImpl`: only the default namespace (the current directory).

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
            let text = h.allocate_str("I/O result");
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
