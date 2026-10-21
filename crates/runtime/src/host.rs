//! The host ABI v1 of WebAssembly modules (`dartforge_host_v1`,
//! docs/WASM-HOST-ABI-V1.md): the only outside world of the `wasm32-linear`
//! runtime. The browser adapter (`runtime/wasm/dartforge_host_v1.mjs`) and the
//! independent executor (`crates/wasm_host`) implement it. Empty on the native
//! targets, which talk to their operating system instead.
#![cfg(target_arch = "wasm32")]
#![allow(unsafe_code)]

#[link(wasm_import_module = "dartforge_host_v1")]
unsafe extern "C" {
    #[link_name = "write"]
    fn host_write(channel: i32, offset: i32, length: i32) -> i32;
    #[link_name = "clock_realtime_us"]
    fn host_clock_realtime_us() -> f64;
    #[link_name = "clock_monotonic_us"]
    fn host_clock_monotonic_us() -> f64;
    #[link_name = "random_fill"]
    fn host_random_fill(offset: i32, length: i32) -> i32;
    #[link_name = "time_zone"]
    fn host_time_zone(
        epoch_seconds: f64,
        offset_out: i32,
        name_offset: i32,
        name_capacity: i32,
    ) -> i32;
    #[link_name = "exit"]
    fn host_exit(status: i32);
    #[link_name = "option"]
    fn host_option(name_offset: i32, name_length: i32, out_offset: i32, out_capacity: i32) -> i32;
}

/// Standard output and standard error, the channels of `write`.
pub const STDOUT: i32 = 1;
pub const STDERR: i32 = 2;

/// Copies `bytes` to `channel`; the host copies them before returning.
pub fn write(channel: i32, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    // SAFETY: the range is a live slice of linear memory, read during the call.
    let status = unsafe { host_write(channel, bytes.as_ptr() as i32, bytes.len() as i32) };
    debug_assert_eq!(status, 0, "host write");
}

/// Microseconds since the Unix epoch (UTC), the wall clock.
pub fn realtime_micros() -> f64 {
    // SAFETY: a host import without arguments.
    unsafe { host_clock_realtime_us() }
}

/// Microseconds since an origin of the host, never decreasing.
pub fn monotonic_micros() -> f64 {
    // SAFETY: a host import without arguments.
    unsafe { host_clock_monotonic_us() }
}

/// Fills `buffer` from the host's cryptographic generator. A host without one
/// is not a valid host: a runtime failure, never a weak fallback.
pub fn random_fill(buffer: &mut [u8]) {
    // SAFETY: the range is a live, writable slice of linear memory.
    let status = unsafe { host_random_fill(buffer.as_mut_ptr() as i32, buffer.len() as i32) };
    assert_eq!(status, 0, "the host has no cryptographic random source");
}

/// The local time zone at `epoch_seconds`: offset in seconds east of UTC and the
/// abbreviated name.
pub fn time_zone(epoch_seconds: i64) -> (i64, String) {
    let mut offset: i32 = 0;
    let mut name = [0u8; 64];
    // SAFETY: the offset and the name buffer are live locals of linear memory.
    let length = unsafe {
        host_time_zone(
            epoch_seconds as f64,
            (&raw mut offset) as i32,
            name.as_mut_ptr() as i32,
            name.len() as i32,
        )
    };
    let length = (length.max(0) as usize).min(name.len());
    (
        i64::from(offset),
        String::from_utf8_lossy(&name[..length]).into_owned(),
    )
}

/// Ends the run with `status`: the host stops executing the module (it unwinds
/// the module's frames itself, like WASI's `proc_exit`) and reports the status.
pub fn exit(status: i32) -> ! {
    // SAFETY: a host import; it does not return to the module.
    unsafe { host_exit(status) };
    core::arch::wasm32::unreachable()
}

/// The runtime option `name` the host passes (`DARTFORGE_*`), or `None`.
pub fn option(name: &str) -> Option<String> {
    let mut value = vec![0u8; 256];
    loop {
        // SAFETY: the name and the buffer are live slices of linear memory.
        let length = unsafe {
            host_option(
                name.as_ptr() as i32,
                name.len() as i32,
                value.as_mut_ptr() as i32,
                value.len() as i32,
            )
        };
        if length < 0 {
            return None;
        }
        let length = length as usize;
        if length <= value.len() {
            value.truncate(length);
            return Some(String::from_utf8_lossy(&value).into_owned());
        }
        value.resize(length, 0);
    }
}
