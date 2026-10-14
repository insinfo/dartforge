// Native OS thread identity for the current isolate owner. IDs can be reused
// after a thread exits; this is not a process-wide isolate identifier.

#[cfg(target_os = "macos")]
unsafe extern "C" {
    #[link_name = "pthread_threadid_np"]
    fn identity_pthread_threadid(thread: *mut std::ffi::c_void, output: *mut u64) -> i32;
}

#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
unsafe extern "system" {
    #[link_name = "GetCurrentThreadId"]
    fn identity_windows_thread_id() -> u32;
}

#[cfg(all(
    target_os = "linux",
    any(
        target_arch = "x86_64",
        target_arch = "x86",
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "riscv64",
        target_arch = "loongarch64",
        target_arch = "powerpc",
        target_arch = "powerpc64",
        target_arch = "s390x"
    )
))]
unsafe extern "C" {
    #[link_name = "syscall"]
    fn identity_linux_syscall(number: std::ffi::c_long, ...) -> std::ffi::c_long;
}

fn current_os_thread_id() -> i64 {
    #[cfg(target_os = "macos")]
    {
        let mut id = 0;
        // A null pthread selects the calling thread, without consulting Rust TLS.
        if unsafe { identity_pthread_threadid(std::ptr::null_mut(), &mut id) } != 0 {
            return 0;
        }
        return i64::try_from(id).unwrap_or(0);
    }
    #[cfg(target_os = "windows")]
    {
        return i64::from(unsafe { identity_windows_thread_id() });
    }
    #[cfg(all(
        target_os = "linux",
        any(
            target_arch = "x86_64",
            target_arch = "x86",
            target_arch = "aarch64",
            target_arch = "arm",
            target_arch = "riscv64",
            target_arch = "loongarch64",
            target_arch = "powerpc",
            target_arch = "powerpc64",
            target_arch = "s390x"
        )
    ))]
    {
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
        const GETTID: std::ffi::c_long = 186;
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        const GETTID: std::ffi::c_long = 0x4000_0000 + 186;
        #[cfg(any(target_arch = "x86", target_arch = "arm"))]
        const GETTID: std::ffi::c_long = 224;
        #[cfg(any(
            target_arch = "aarch64",
            target_arch = "riscv64",
            target_arch = "loongarch64"
        ))]
        const GETTID: std::ffi::c_long = 178;
        #[cfg(any(target_arch = "powerpc", target_arch = "powerpc64"))]
        const GETTID: std::ffi::c_long = 207;
        #[cfg(target_arch = "s390x")]
        const GETTID: std::ffi::c_long = 236;
        let id = unsafe { identity_linux_syscall(GETTID) };
        return if id > 0 { id as i64 } else { 0 };
    }
    #[allow(unreachable_code)]
    0
}

/// Returns the current owner's numeric OS thread ID, or zero if unavailable.
/// macOS uses `pthread_threadid_np`, Linux uses `SYS_gettid`, and Windows uses
/// `GetCurrentThreadId`. This leaf function never constructs a Rust thread ID.
/// The owner remains on its dedicated OS thread between asynchronous turns.
///
/// ```
/// assert!(dartforge_runtime::abi::dartforge_native_DartForge_isolate_thread_id() >= 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_isolate_thread_id() -> i64 {
    current_os_thread_id()
}

#[cfg(test)]
mod tests_isolate_os_identity {
    use super::*;

    #[test]
    fn thread_identity_is_stable_and_differs_for_a_live_worker() {
        let owner = current_os_thread_id();
        let (send, receive) = std::sync::mpsc::channel();
        let (resume, wait) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let before = current_os_thread_id();
            send.send(before).unwrap();
            wait.recv().unwrap();
            std::thread::yield_now();
            assert_eq!(current_os_thread_id(), before);
        });
        let other = receive.recv().unwrap();
        if owner != 0 {
            assert!(owner > 0 && other > 0);
            assert_ne!(owner, other);
        } else {
            assert_eq!(other, 0);
        }
        std::thread::yield_now();
        assert_eq!(current_os_thread_id(), owner);
        resume.send(()).unwrap();
        worker.join().unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_identity_matches_the_actual_pthread() {
        unsafe extern "C" {
            #[link_name = "pthread_self"]
            fn identity_test_pthread_self() -> *mut std::ffi::c_void;
        }
        let mut actual = 0;
        assert_eq!(
            unsafe { identity_pthread_threadid(identity_test_pthread_self(), &mut actual) },
            0
        );
        assert!(actual > 0);
        assert_eq!(current_os_thread_id(), actual as i64);
    }
}
