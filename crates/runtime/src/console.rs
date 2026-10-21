//! Standard output, standard error and the end of the process for the runtime,
//! on every target. The native targets use `std`; on WebAssembly `std`'s streams
//! discard their bytes and `process::exit` only traps, so both go through the
//! host ABI (`crate::host`, docs/WASM-HOST-ABI-V1.md). The crate's `print!`
//! family (lib.rs) writes here on WebAssembly.

/// Standard output, locked.
#[cfg(not(target_arch = "wasm32"))]
pub fn stdout() -> std::io::StdoutLock<'static> {
    std::io::stdout().lock()
}

/// Standard error, locked.
#[cfg(not(target_arch = "wasm32"))]
pub fn stderr() -> std::io::StderrLock<'static> {
    std::io::stderr().lock()
}

/// Ends the program with `code`.
#[cfg(not(target_arch = "wasm32"))]
pub fn exit(code: i32) -> ! {
    std::process::exit(code)
}

/// A host channel of `write` (unbuffered: the host copies each write).
#[cfg(target_arch = "wasm32")]
pub struct HostStream(i32);

#[cfg(target_arch = "wasm32")]
impl std::io::Write for HostStream {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        crate::host::write(self.0, bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Standard output (`write` channel 1).
#[cfg(target_arch = "wasm32")]
pub fn stdout() -> HostStream {
    HostStream(crate::host::STDOUT)
}

/// Standard error (`write` channel 2).
#[cfg(target_arch = "wasm32")]
pub fn stderr() -> HostStream {
    HostStream(crate::host::STDERR)
}

/// Ends the program with `code`: the host stops the module and reports it.
#[cfg(target_arch = "wasm32")]
pub fn exit(code: i32) -> ! {
    crate::host::exit(code)
}

/// A runtime option (`DARTFORGE_*`, not Dart's `Platform.environment`): the
/// process environment natively; on WebAssembly, what the host passes
/// (`option`, docs/WASM-HOST-ABI-V1.md). The same result type as
/// `std::env::var`.
#[cfg(not(target_arch = "wasm32"))]
pub fn env_var(name: &str) -> Result<String, std::env::VarError> {
    std::env::var(name)
}

/// [`env_var`] as `std::env::var_os` returns it.
#[cfg(not(target_arch = "wasm32"))]
pub fn env_var_os(name: &str) -> Option<std::ffi::OsString> {
    std::env::var_os(name)
}

/// A runtime option (`DARTFORGE_*`) the host passes (`option`).
#[cfg(target_arch = "wasm32")]
pub fn env_var(name: &str) -> Result<String, std::env::VarError> {
    crate::host::option(name).ok_or(std::env::VarError::NotPresent)
}

/// [`env_var`] as `std::env::var_os` returns it.
#[cfg(target_arch = "wasm32")]
pub fn env_var_os(name: &str) -> Option<std::ffi::OsString> {
    crate::host::option(name).map(std::ffi::OsString::from)
}
