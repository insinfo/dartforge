// Native runtime: the VM's `dart:io` processes (`runtime/bin/process.cc`,
// `process_linux.cc`, `process_macos.cc`): `Process.start`/`run`/`runSync`,
// `Process.killPid`, signals (`ProcessSignal.watch`) and memory usage
// (`ProcessInfo`).
//
// Like the VM: the child is born from `fork` + `execvp`, with pipes for the input,
// the output and the error (which Dart reads as a `_NativeSocket`, through the
// event handler) and a control pipe that returns the `errno` of a failed `exec`; a
// thread (`ExitCodeHandler`) waits for the children with `wait` and writes the exit
// code (`[code, negative]`, two `int32`s) into each process's exit pipe. The
// detached modes go through a second `fork` and `setsid`, and the grandchild
// reports its own pid.
//
// Between `fork` and `exec`, the child only calls async-signal-safe functions
// (`read`, `dup2`, `chdir`, `execvp`, `write`, `_exit`…); everything it uses
// (arguments, environment) is built before the `fork`. The child reports only the
// `errno`; the message comes from the same `strerror` in the parent.
//
// On Windows (`io_windows_processes.rs`) the child is born from `CreateProcessW`,
// with overlapped named pipes on the parent's side, and the exit code arrives
// through the system thread pool (`RegisterWaitForSingleObject`), with the same
// functions (`start_process`, `wait_process`, `kill_process`,
// `subscribe_signal`…).

/// `ProcessStartMode`.
const MODE_NORMAL: i64 = 0;
const MODE_INHERIT_STDIO: i64 = 1;
const MODE_DETACHED: i64 = 2;
const MODE_DETACHED_WITH_STDIO: i64 = 3;

fn attached_mode(mode: i64) -> bool {
    mode == MODE_NORMAL || mode == MODE_INHERIT_STDIO
}

fn mode_with_stdio(mode: i64) -> bool {
    mode == MODE_NORMAL || mode == MODE_DETACHED_WITH_STDIO
}

/// The strings of a Dart `List<String>` in UTF-8; `None` if some element is not a
/// `String`.
fn list_strings(list: i64) -> Option<Vec<Vec<u8>>> {
    HEAP.with(|h| {
        let h = h.borrow();
        if !h.is_list(list) {
            return None;
        }
        let n = h.list_len(list);
        let mut output = Vec::with_capacity(n);
        for i in 0..n {
            match h.list_get(list, i) {
                crate::heap::Value::Ref(r) => output.push(h.text(r)?.to_vm_utf8()),
                _ => return None,
            }
        }
        Some(output)
    })
}

/// Writes the start error into the `_ProcessStartStatus` (the VM's
/// `Dart_SetField`), through the overlay's function.
fn start_failure(status: i64, code: i64, message: &str) {
    let Some(f) = helper("_dartforgeFalhaAoIniciar") else {
        panic!("compiler bug: dart:io without a registered `_dartforgeFalhaAoIniciar`");
    };
    // SAFETY: registered by the `dart:io` overlay with the signature
    // (`_ProcessStartStatus`, `int`, `String`) → `void`.
    let g = { let dart_target: usize = f; move |a0: i64, a1: i64, a2: i64| { dart_v3(dart_target, a0, a1, a2) } };
    let m = allocate_str(message);
    with_roots(&[m], || g(status, code, m));
}

/// A started process: the pid and the parent-side descriptors (-1 when the mode
/// does not have them).
struct StartedProcess {
    pid: i64,
    input: i64,
    output: i64,
    error: i64,
    end: i64,
}

// ---------------------------------------------------------------------------
// Unix.

#[cfg(unix)]
unsafe extern "C" {
    fn fork() -> i32;
    fn execvp(file: *const std::ffi::c_char, argv: *const *const std::ffi::c_char) -> i32;
    fn dup2(of: i32, to: i32) -> i32;
    fn chdir(file_path: *const std::ffi::c_char) -> i32;
    fn setsid() -> i32;
    fn getpid() -> i32;
    fn _exit(code: i32) -> !;
    fn close(fd: i32) -> i32;
    fn open(file_path: *const std::ffi::c_char, flags: i32, ...) -> i32;
    fn kill(pid: i32, sig_num: i32) -> i32;
    fn wait(status: *mut i32) -> i32;
    fn poll(fds: *mut PollFd, n: u64, wait_entry: i32) -> i32;
    fn sysconf(name: i32) -> std::ffi::c_long;
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn __errno_location() -> *mut i32;
    static mut environ: *const *const std::ffi::c_char;
}

#[cfg(all(unix, not(target_os = "linux")))]
unsafe extern "C" {
    fn __error() -> *mut i32;
    fn _NSGetEnviron() -> *mut *const *const std::ffi::c_char;
}

/// Replaces the process environment (in the child, before `execvp`): libc's
/// `environ`; on macOS, through `_NSGetEnviron` (the symbol is not visible to a
/// dynamic library).
#[cfg(unix)]
unsafe fn swap_env(new: *const *const std::ffi::c_char) {
    #[cfg(target_os = "linux")]
    // SAFETY: only the child, on one thread, before `exec`.
    unsafe {
        environ = new;
    }
    #[cfg(not(target_os = "linux"))]
    // SAFETY: likewise.
    unsafe {
        *_NSGetEnviron() = new;
    }
}

/// The current `errno` (safe in the child between `fork` and `exec`).
#[cfg(unix)]
fn current_errno() -> i32 {
    // SAFETY: the address of the thread's `errno`.
    #[cfg(target_os = "linux")]
    unsafe {
        *__errno_location()
    }
    #[cfg(not(target_os = "linux"))]
    unsafe {
        *__error()
    }
}

#[cfg(unix)]
#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

/// A pipe with both sides closed on `exec` (`pipe2(O_CLOEXEC)`).
#[cfg(unix)]
fn cloexec_pipe() -> IoResult<[i32; 2]> {
    let mut fds = [-1i32; 2];
    // SAFETY: `fds` has two integers.
    if unsafe { pipe(fds.as_mut_ptr()) } != 0 {
        return Err(OsError::of(&std::io::Error::last_os_error()));
    }
    close_on_exec(fds[0]);
    close_on_exec(fds[1]);
    Ok(fds)
}

/// Writes everything, retrying on interruption (`FDUtils::WriteToBlocking`).
#[cfg(unix)]
fn write_blocking(fd: i32, data: &[u8]) -> isize {
    let mut done = 0usize;
    while done < data.len() {
        // SAFETY: the data is readable.
        let n = unsafe { write(fd, data[done..].as_ptr().cast(), data.len() - done) };
        if n < 0 {
            if current_errno() == 4 {
                continue;
            }
            return -1;
        }
        if n == 0 {
            break;
        }
        done += n as usize;
    }
    done as isize
}

/// Reads until full or the end (`FDUtils::ReadFromBlocking`).
#[cfg(unix)]
fn read_blocking(fd: i32, dest: &mut [u8]) -> isize {
    let mut done = 0usize;
    while done < dest.len() {
        // SAFETY: the destination is writable.
        let n = unsafe { read(fd, dest[done..].as_mut_ptr().cast(), dest.len() - done) };
        if n < 0 {
            if current_errno() == 4 {
                continue;
            }
            return -1;
        }
        if n == 0 {
            break;
        }
        done += n as usize;
    }
    done as isize
}

/// The attached children and the exit pipe of each one (`ProcessInfoList`), and the
/// thread that waits for them (`ExitCodeHandler`).
#[cfg(unix)]
struct ChildWait {
    state: std::sync::Mutex<WaitState>,
    sig_num: std::sync::Condvar,
}

#[cfg(unix)]
#[derive(Default)]
struct WaitState {
    /// pid → write side of the exit pipe.
    active: crate::hash::HashMap<i32, i32>,
    running: bool,
}

#[cfg(unix)]
fn child_wait() -> &'static ChildWait {
    static E: std::sync::OnceLock<ChildWait> = std::sync::OnceLock::new();
    E.get_or_init(|| ChildWait { state: std::sync::Mutex::new(WaitState::default()), sig_num: std::sync::Condvar::new() })
}

#[cfg(unix)]
impl ChildWait {
    /// Registers a child and makes sure the waiting thread exists.
    fn register(&'static self, pid: i32, output_fd: i32) {
        let mut e = self.state.lock().unwrap_or_else(|x| x.into_inner());
        e.active.insert(pid, output_fd);
        self.sig_num.notify_all();
        if e.running {
            return;
        }
        e.running = true;
        std::thread::Builder::new()
            .name("dart:io Process.start".to_string())
            .spawn(move || self.wait_for())
            .expect("dart:io: failed to create the processes thread");
    }

    /// `ExitCodeHandlerEntry`: waits for any child and reports the code.
    fn wait_for(&self) {
        loop {
            {
                let mut e = self.state.lock().unwrap_or_else(|x| x.into_inner());
                while e.active.is_empty() {
                    e = self.sig_num.wait(e).unwrap_or_else(|x| x.into_inner());
                }
            }
            let mut status = 0i32;
            // SAFETY: waits for any child.
            let pid = unsafe { wait(&mut status) };
            if pid < 0 {
                if current_errno() == 4 {
                    continue;
                }
                // No children (ECHILD): the registration is ahead of another
                // process's `fork`; try again.
                std::thread::sleep(std::time::Duration::from_millis(1));
                continue;
            }
            let (mut code, mut negative) = (0i32, 0i32);
            let termination = status & 0x7f;
            if termination == 0 {
                code = (status >> 8) & 0xff;
            } else if termination != 0x7f {
                code = termination;
                negative = 1;
            }
            let fd = self.state.lock().unwrap_or_else(|x| x.into_inner()).active.remove(&pid);
            if let Some(fd) = fd {
                let mut msg = [0u8; 8];
                msg[..4].copy_from_slice(&code.to_ne_bytes());
                msg[4..].copy_from_slice(&negative.to_ne_bytes());
                write_blocking(fd, &msg);
                // SAFETY: the write side belongs to this registration, closed once.
                unsafe { close(fd) };
            }
        }
    }
}

/// The VM's `ProcessStarter`.
#[cfg(unix)]
fn start_process(
    file_path: &[u8],
    arguments: &[Vec<u8>],
    directory: Option<&[u8]>,
    env: Option<&[Vec<u8>]>,
    mode: i64,
) -> Result<StartedProcess, OsError> {
    let cstr = |b: &[u8]| std::ffi::CString::new(b.to_vec()).map_err(|_| OsError::from_code(os_code::INVALID));
    let path_c = cstr(file_path)?;
    let mut args_c = vec![path_c.clone()];
    for a in arguments {
        args_c.push(cstr(a)?);
    }
    let mut argv: Vec<*const std::ffi::c_char> = args_c.iter().map(|c| c.as_ptr()).collect();
    argv.push(std::ptr::null());
    let c_env = match env {
        Some(v) => Some(v.iter().map(|e| cstr(e)).collect::<Result<Vec<_>, _>>()?),
        None => None,
    };
    let envp: Option<Vec<*const std::ffi::c_char>> = c_env.as_ref().map(|v| {
        let mut p: Vec<_> = v.iter().map(|c| c.as_ptr()).collect();
        p.push(std::ptr::null());
        p
    });
    let c_directory = match directory {
        Some(d) => Some(cstr(d)?),
        None => None,
    };
    let c_null = c"/dev/null";

    let mut pipes: Vec<i32> = Vec::new();
    let close_all = |pipes: &[i32]| {
        for &fd in pipes {
            if fd >= 0 {
                // SAFETY: descriptors of this start.
                unsafe { close(fd) };
            }
        }
    };
    let control = cloexec_pipe()?;
    pipes.extend(control);
    let read_input = match cloexec_pipe() {
        Ok(p) => p,
        Err(e) => {
            close_all(&pipes);
            return Err(e);
        }
    };
    pipes.extend(read_input);
    let (read_error, write_output) = if mode_with_stdio(mode) {
        let a = cloexec_pipe().inspect_err(|_| close_all(&pipes))?;
        pipes.extend(a);
        let b = cloexec_pipe().inspect_err(|_| close_all(&pipes))?;
        pipes.extend(b);
        (a, b)
    } else {
        ([-1, -1], [-1, -1])
    };
    // SAFETY: `sysconf(_SC_OPEN_MAX)`.
    let max_fds = {
        #[cfg(target_os = "linux")]
        const SC_OPEN_MAX: i32 = 4;
        #[cfg(not(target_os = "linux"))]
        const SC_OPEN_MAX: i32 = 5;
        let n = unsafe { sysconf(SC_OPEN_MAX) };
        if n < 0 { 256 } else { n as i32 }
    };

    // SAFETY: the child only uses async-signal-safe functions and memory that is
    // already built.
    let pid = unsafe { fork() };
    if pid < 0 {
        let e = OsError::of(&std::io::Error::last_os_error());
        close_all(&pipes);
        return Err(e);
    }
    if pid == 0 {
        // --- The child. ---
        // SAFETY: everything here is safe after `fork`.
        unsafe {
            let report = |fd: i32| -> ! {
                let e = current_errno();
                write_blocking(fd, &e.to_ne_bytes());
                close(fd);
                _exit(1)
            };
            let mut msg = [0u8; 1];
            if read_blocking(read_input[0], &mut msg) != 1 {
                _exit(1);
            }
            if attached_mode(mode) {
                if mode == MODE_NORMAL {
                    if dup2(write_output[0], 0) == -1 || dup2(read_input[1], 1) == -1 || dup2(read_error[1], 2) == -1 {
                        report(control[1]);
                    }
                }
                if let Some(d) = &c_directory {
                    if chdir(d.as_ptr()) != 0 {
                        report(control[1]);
                    }
                }
                if let Some(e) = &envp {
                    swap_env(e.as_ptr());
                }
                execvp(path_c.as_ptr(), argv.as_ptr());
                report(control[1]);
            }
            // Detached: a second `fork` after `setsid`, and the grandchild
            // reports its own pid through the control pipe.
            if mode == MODE_DETACHED {
                close(read_input[0]);
                close(read_input[1]);
            }
            let p = fork();
            if p < 0 {
                report(control[1]);
            }
            if p > 0 {
                _exit(0);
            }
            if setsid() == -1 {
                report(control[1]);
            }
            let p = fork();
            if p < 0 {
                report(control[1]);
            }
            if p > 0 {
                _exit(0);
            }
            if mode == MODE_DETACHED {
                for fd in 0..max_fds {
                    if fd != control[1] {
                        close(fd);
                    }
                }
                if open(c_null.as_ptr(), 2 /* O_RDWR */) != 0 || dup2(0, 1) != 1 || dup2(0, 2) != 2 {
                    report(control[1]);
                }
            } else {
                for fd in 0..max_fds {
                    if fd != control[1] && fd != write_output[0] && fd != read_input[1] && fd != read_error[1] {
                        close(fd);
                    }
                }
                if dup2(write_output[0], 0) == -1 || dup2(read_input[1], 1) == -1 || dup2(read_error[1], 2) == -1 {
                    report(control[1]);
                }
                close(write_output[0]);
                close(read_input[1]);
                close(read_error[1]);
            }
            if let Some(d) = &c_directory {
                if chdir(d.as_ptr()) != 0 {
                    report(control[1]);
                }
            }
            if let Some(e) = &envp {
                swap_env(e.as_ptr());
            }
            write_blocking(control[1], &getpid().to_ne_bytes());
            execvp(path_c.as_ptr(), argv.as_ptr());
            report(control[1]);
        }
    }

    // --- The parent. ---
    let mut end = -1i64;
    if attached_mode(mode) {
        match cloexec_pipe() {
            Ok(p) => {
                set_nonblocking(p[0]);
                child_wait().register(pid, p[1]);
                end = i64::from(p[0]);
            }
            Err(e) => {
                close_all(&pipes);
                return Err(e);
            }
        }
    }
    let shutdown_end = |end: i64| {
        if end >= 0 {
            // SAFETY: the read side of this start's exit pipe.
            unsafe { close(end as i32) };
        }
    };
    if write_blocking(read_input[1], b"1") != 1 {
        let e = OsError::of(&std::io::Error::last_os_error());
        shutdown_end(end);
        close_all(&pipes);
        return Err(e);
    }
    // SAFETY: the write side of the control pipe now belongs only to the child.
    unsafe { close(control[1]) };
    let mut final_pid = pid;
    let child_error = if attached_mode(mode) {
        let mut b = [0u8; 4];
        match read_blocking(control[0], &mut b) {
            4 => Some(i32::from_ne_bytes(b)),
            -1 => Some(current_errno()),
            _ => None,
        }
    } else {
        let mut b = [0u8; 8];
        match read_blocking(control[0], &mut b) {
            4 => {
                final_pid = i32::from_ne_bytes(b[..4].try_into().expect("4 bytes"));
                None
            }
            8 => {
                final_pid = i32::from_ne_bytes(b[..4].try_into().expect("4 bytes"));
                Some(i32::from_ne_bytes(b[4..].try_into().expect("4 bytes")))
            }
            -1 => Some(current_errno()),
            _ => None,
        }
    };
    // SAFETY: the read side of the control pipe, closed once.
    unsafe { close(control[0]) };
    if let Some(e) = child_error {
        shutdown_end(end);
        close_all(&[read_input[0], read_input[1], read_error[0], read_error[1], write_output[0], write_output[1]]);
        return Err(OsError::from_code(e));
    }
    let mut r = StartedProcess { pid: i64::from(final_pid), input: -1, output: -1, error: -1, end };
    if mode_with_stdio(mode) {
        set_nonblocking(read_input[0]);
        set_nonblocking(write_output[1]);
        set_nonblocking(read_error[0]);
        // The child's output is what the parent reads (`_stdout`); the input, what
        // the parent writes (`_stdin`).
        r.output = i64::from(read_input[0]);
        r.input = i64::from(write_output[1]);
        r.error = i64::from(read_error[0]);
        // SAFETY: the child's sides, closed once.
        unsafe {
            close(read_input[1]);
            close(write_output[0]);
            close(read_error[1]);
        }
    } else {
        // SAFETY: the synchronization pipe, closed once.
        unsafe {
            close(read_input[0]);
            close(read_input[1]);
        }
    }
    Ok(r)
}

/// `Process_Start(namespace, path, arguments, workingDirectory, environment, mode,
/// stdin, stdout, stderr, exitHandler, status)`.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn dartforge_native_Process_Start(
    process: i64,
    _ns: i64,
    file_path: i64,
    arguments: i64,
    directory: i64,
    env: i64,
    mode: i64,
    input: i64,
    output: i64,
    error: i64,
    end: i64,
    status: i64,
) -> u8 {
    let path_b = utf8_of_string(file_path);
    let Some(args) = list_strings(arguments) else {
        start_failure(status, 0, "Arguments must be builtin strings");
        return 0;
    };
    let dir = (directory != 0).then(|| utf8_of_string(directory));
    let env_vars = if env == 0 {
        None
    } else {
        match list_strings(env) {
            Some(v) => Some(v),
            None => {
                start_failure(status, 0, "Environment values must be builtin strings");
                return 0;
            }
        }
    };
    match start_process(&path_b, &args, dir.as_deref(), env_vars.as_deref(), mode.clamp(0, 3)) {
        Ok(r) => {
            if mode_with_stdio(mode) {
                set_socket_on_object(input, r.input, SocketFinalizer::Normal);
                set_socket_on_object(output, r.output, SocketFinalizer::Normal);
                set_socket_on_object(error, r.error, SocketFinalizer::Normal);
            }
            if attached_mode(mode) {
                set_socket_on_object(end, r.end, SocketFinalizer::Normal);
            }
            write_native_field(process, r.pid);
            1
        }
        Err(e) => {
            start_failure(status, e.code, &e.message);
            0
        }
    }
}

/// `Process::Wait` (`runSync`): closes the input, reads the output and the error to
/// the end and the code from the exit pipe; `(code, output, error)`.
#[cfg(unix)]
fn wait_process(input: i64, output: i64, error: i64, end: i64) -> IoResult<(i64, Vec<u8>, Vec<u8>)> {
    close_socket_descriptor(input);
    let mut output_data = Vec::new();
    let mut error_data = Vec::new();
    let mut code = [0u8; 8];
    let mut live_set = vec![output, error, end];
    const POLLIN: i16 = 1;
    const POLLERR: i16 = 8;
    const POLLHUP: i16 = 0x10;
    const POLLNVAL: i16 = 0x20;
    let close_live = |live_set: &[i64]| {
        for &fd in live_set {
            close_socket_descriptor(fd);
        }
    };
    while !live_set.is_empty() {
        let mut fds: Vec<PollFd> = live_set.iter().map(|&fd| PollFd { fd: fd as i32, events: POLLIN, revents: 0 }).collect();
        // SAFETY: `fds` has `len` elements.
        let r = unsafe { poll(fds.as_mut_ptr(), fds.len() as u64, -1) };
        if r <= 0 {
            if r < 0 && current_errno() == 4 {
                continue;
            }
            let e = OsError::of(&std::io::Error::last_os_error());
            close_live(&live_set);
            return Err(e);
        }
        let mut closed_set = Vec::new();
        for f in &fds {
            let fd = i64::from(f.fd);
            if f.revents & (POLLNVAL | POLLERR) != 0 {
                let e = OsError::of(&std::io::Error::last_os_error());
                close_live(&live_set);
                return Err(e);
            }
            if f.revents & POLLIN != 0 {
                let available = available_bytes(fd).max(0) as usize;
                if fd == end {
                    if available == 8 && read_blocking(f.fd, &mut code) != 8 {
                        let e = OsError::of(&std::io::Error::last_os_error());
                        close_live(&live_set);
                        return Err(e);
                    }
                } else {
                    let mut b = vec![0u8; available];
                    let n = read_from_socket(fd, &mut b)?;
                    b.truncate(n);
                    if fd == output { output_data.extend(b) } else { error_data.extend(b) }
                }
            }
            if f.revents & POLLHUP != 0 {
                close_socket_descriptor(fd);
                closed_set.push(fd);
            }
        }
        live_set.retain(|fd| !closed_set.contains(fd));
    }
    let value = i64::from(i32::from_ne_bytes(code[..4].try_into().expect("4 bytes")));
    let negative = i32::from_ne_bytes(code[4..].try_into().expect("4 bytes"));
    Ok((if negative != 0 { -value } else { value }, output_data, error_data))
}

/// `Process_Wait(stdin, stdout, stderr, exitHandler)`: `[pid, code, output,
/// error]`; on an error, kills the child and throws the `OSError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Process_Wait(process: i64, input: i64, output: i64, error: i64, end: i64) -> i64 {
    let (Some(e), Some(s), Some(r), Some(f)) = (object_socket(input), object_socket(output), object_socket(error), object_socket(end)) else {
        return 0;
    };
    let pid = native_field(process);
    #[cfg(unix)]
    let result = {
        let fds = (e.descriptor(), s.descriptor(), r.descriptor(), f.descriptor());
        // The descriptors pass to this `wait`, which closes them.
        for x in [e, s, r, f] {
            x.descriptor.store(-1, std::sync::atomic::Ordering::Release);
        }
        wait_process(fds.0, fds.1, fds.2, fds.3)
    };
    #[cfg(windows)]
    let result = {
        let r0 = wait_process(e.descriptor(), s.descriptor(), r.descriptor(), f.descriptor());
        // `Process::Wait` closed the handles; the objects release their
        // references (`CloseFd`).
        for x in [e, s, r, f] {
            x.release_descriptor();
        }
        r0
    };
    match result {
        Ok((code, out, err)) => {
            let o = dart_bytes(out);
            let x = with_roots(&[o], || dart_bytes(err));
            with_roots(&[o, x], || dart_fixed_list(&[dart_int(pid), dart_int(code), o, x]))
        }
        Err(error) => {
            kill_process(pid, 9);
            throw_os_error(&error);
            0
        }
    }
}

#[cfg(unix)]
fn kill_process(pid: i64, sig_num: i64) -> bool {
    loop {
        // SAFETY: sends a signal to a pid.
        if unsafe { kill(pid as i32, sig_num as i32) } != -1 {
            return true;
        }
        if current_errno() != 4 {
            return false;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Process_KillPid(pid: i64, sig_num: i64) -> u8 {
    u8::from(kill_process(pid, system_signal(sig_num)))
}

/// The system's signal number from that of `ProcessSignal`, which Dart numbers like
/// Linux (`sigusr2` is 12): on macOS and the BSDs several numbers differ (there 12
/// is `SIGSYS`), and the VM translates (`SignalMap` in `process_macos.cc`) before
/// `kill` and subscription.
fn system_signal(sig_num: i64) -> i64 {
    if cfg!(any(target_os = "linux", target_os = "android", not(unix))) {
        return sig_num;
    }
    match sig_num {
        7 => 10,  // SIGBUS
        10 => 30, // SIGUSR1
        12 => 31, // SIGUSR2
        17 => 20, // SIGCHLD
        18 => 19, // SIGCONT
        19 => 17, // SIGSTOP
        20 => 18, // SIGTSTP
        23 => 16, // SIGURG
        29 => 23, // SIGPOLL (SIGIO)
        31 => 12, // SIGSYS
        s => s,
    }
}

// ---------------------------------------------------------------------------
// Signals (`ProcessSignal.watch`): each subscription is a pipe; the signal handler
// writes one byte into the signal's pipes, and Dart reads the other side as a
// socket.

/// The signals Dart may watch (`kSignals`), in the system's numbering.
#[cfg(unix)]
fn watchable_signal(s: i64) -> bool {
    #[cfg(target_os = "linux")]
    const SIGNALS: &[i64] = &[1, 2, 15, 10, 12, 28, 3];
    #[cfg(not(target_os = "linux"))]
    const SIGNALS: &[i64] = &[1, 2, 15, 30, 31, 28, 3];
    SIGNALS.contains(&s)
}

/// A subscription (`SignalInfo`): the signal, both sides of the pipe and how many
/// handlers are writing into it right now. The handler only uses atomics and
/// `write`.
#[cfg(unix)]
struct SignalSubscription {
    sig_num: std::sync::atomic::AtomicI32,
    write_side: std::sync::atomic::AtomicI32,
    read_side: std::sync::atomic::AtomicI32,
    in_use: std::sync::atomic::AtomicU32,
}

#[cfg(unix)]
const MAX_SUBSCRIPTIONS: usize = 64;
#[cfg(unix)]
static SUBSCRIPTIONS: [SignalSubscription; MAX_SUBSCRIPTIONS] = [const {
    SignalSubscription {
        sig_num: std::sync::atomic::AtomicI32::new(0),
        write_side: std::sync::atomic::AtomicI32::new(-1),
        read_side: std::sync::atomic::AtomicI32::new(-1),
        in_use: std::sync::atomic::AtomicU32::new(0),
    }
}; MAX_SUBSCRIPTIONS];

/// The handler: one byte into the pipe of each subscription of the signal.
///
/// The pipe is non-blocking: when full, `write` fails with `EAGAIN` and the
/// notification merges with those Dart has not read yet (as the system merges
/// pending signals), never stopping the interrupted thread. The interrupted
/// thread's `errno` is preserved. The `in_use` count prevents cancellation from
/// closing (and the system from reusing) the descriptor during the `write`.
#[cfg(unix)]
extern "C" fn handle_signal(sig_num: i32) {
    use std::sync::atomic::Ordering::SeqCst;
    let saved_errno = current_errno();
    for i in &SUBSCRIPTIONS {
        if i.sig_num.load(SeqCst) != sig_num {
            continue;
        }
        i.in_use.fetch_add(1, SeqCst);
        let fd = i.write_side.load(SeqCst);
        if fd >= 0 && i.sig_num.load(SeqCst) == sig_num {
            let b = [0u8];
            // SAFETY: `write` is safe in a signal handler; the descriptor is not
            // closed while `in_use` > 0, and it does not block.
            unsafe { write(fd, b.as_ptr().cast(), 1) };
        }
        i.in_use.fetch_sub(1, SeqCst);
    }
    set_errno(saved_errno);
}

#[cfg(target_os = "linux")]
fn set_errno(e: i32) {
    // SAFETY: the thread's `errno`.
    unsafe { *__errno_location() = e };
}
#[cfg(all(unix, not(target_os = "linux")))]
fn set_errno(e: i32) {
    // SAFETY: the thread's `errno`.
    unsafe { *__error() = e };
}

/// `signal(sig_num, handler)`: the previous handler.
#[cfg(unix)]
unsafe extern "C" {
    fn signal(sig_num: i32, handler: usize) -> usize;
}

#[cfg(unix)]
fn subscriptions_mutex() -> std::sync::MutexGuard<'static, crate::hash::HashMap<i32, usize>> {
    // Signal → the previous handler, restored when the last subscription leaves.
    static M: std::sync::OnceLock<std::sync::Mutex<crate::hash::HashMap<i32, usize>>> = std::sync::OnceLock::new();
    M.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// `Process::SetSignalHandler`: the read side of the subscription's pipe.
#[cfg(unix)]
fn subscribe_signal(sig_num: i64) -> IoResult<i64> {
    use std::sync::atomic::Ordering::SeqCst;
    let sig_num = system_signal(sig_num);
    if !watchable_signal(sig_num) {
        return Err(OsError::from_code(os_code::INVALID));
    }
    let fds = cloexec_pipe()?;
    if !(set_nonblocking(fds[0]) && set_nonblocking(fds[1])) {
        let e = OsError::of(&std::io::Error::last_os_error());
        // SAFETY: the descriptors were just created.
        unsafe {
            close(fds[0]);
            close(fds[1]);
        }
        return Err(e);
    }
    let mut previous_ones = subscriptions_mutex();
    // The fields only change under the lock; the handler sees the subscription
    // when the signal is written, last.
    let Some(free_ptr) = SUBSCRIPTIONS.iter().find(|i| i.sig_num.load(SeqCst) == 0) else {
        // SAFETY: the descriptors were just created.
        unsafe {
            close(fds[0]);
            close(fds[1]);
        }
        return Err(OsError::from_code(24 /* EMFILE */));
    };
    free_ptr.write_side.store(fds[1], SeqCst);
    free_ptr.read_side.store(fds[0], SeqCst);
    free_ptr.sig_num.store(sig_num as i32, SeqCst);
    if let std::collections::hash_map::Entry::Vacant(v) = previous_ones.entry(sig_num as i32) {
        // SAFETY: installs the signal handler.
        let old = unsafe { signal(sig_num as i32, handle_signal as usize) };
        v.insert(old);
    }
    Ok(i64::from(fds[0]))
}

/// Undoes the subscriptions that `filter_obj` picks (under the lock): removes them
/// from the handler, waits for the `write`s in progress and closes the write side;
/// the previous handler comes back when the signal has no subscriptions left.
#[cfg(unix)]
fn remove_subscriptions(previous_ones: &mut crate::hash::HashMap<i32, usize>, filter_obj: impl Fn(&SignalSubscription) -> bool) {
    use std::sync::atomic::Ordering::SeqCst;
    for i in &SUBSCRIPTIONS {
        let sig_num = i.sig_num.load(SeqCst);
        if sig_num == 0 || !filter_obj(i) {
            continue;
        }
        let f = i.write_side.swap(-1, SeqCst);
        i.read_side.store(-1, SeqCst);
        // A handler that has already read the descriptor finishes its `write`
        // before the `close`.
        while i.in_use.load(SeqCst) != 0 {
            std::hint::spin_loop();
        }
        if f >= 0 {
            // SAFETY: the subscription's write side, closed once.
            unsafe { close(f) };
        }
        i.sig_num.store(0, SeqCst);
        if !SUBSCRIPTIONS.iter().any(|x| x.sig_num.load(SeqCst) == sig_num) {
            if let Some(old) = previous_ones.remove(&sig_num) {
                // SAFETY: restores the previous handler.
                unsafe { signal(sig_num, old) };
            }
        }
    }
}

/// `Process::ClearSignalHandler`.
#[cfg(unix)]
fn cancel_signal(sig_num: i64) {
    let sig_num = system_signal(sig_num);
    let mut previous_ones = subscriptions_mutex();
    remove_subscriptions(&mut previous_ones, |i| i64::from(i.sig_num.load(std::sync::atomic::Ordering::SeqCst)) == sig_num);
}

/// `Process::ClearSignalHandlerByFd`: the signal's socket (the read side `fd`) was
/// closed.
#[cfg(unix)]
fn clear_signal_by_descriptor(fd: i64) {
    let mut previous_ones = subscriptions_mutex();
    remove_subscriptions(&mut previous_ones, |i| i64::from(i.read_side.load(std::sync::atomic::Ordering::SeqCst)) == fd);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Process_SetSignalHandler(sig_num: i64) -> i64 {
    dart_or_error(subscribe_signal(sig_num), dart_int)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Process_ClearSignalHandler(sig_num: i64) {
    cancel_signal(sig_num);
}

// ---------------------------------------------------------------------------
// `ProcessInfo`: resident memory.

/// `Process::CurrentRSS`.
#[cfg(target_os = "linux")]
fn resident_memory() -> IoResult<i64> {
    unsafe extern "C" {
        fn getpagesize() -> i32;
    }
    let statm = std::fs::read_to_string("/proc/self/statm")?;
    let pages: i64 = statm.split_whitespace().nth(1).and_then(|x| x.parse().ok()).ok_or_else(|| OsError::from_code(os_code::INVALID))?;
    // SAFETY: a query without effects.
    Ok(pages * i64::from(unsafe { getpagesize() }))
}
#[cfg(all(unix, not(target_os = "linux")))]
fn resident_memory() -> IoResult<i64> {
    #[repr(C)]
    #[derive(Default)]
    struct BasicInfo {
        virtual_size: u64,
        resident_size: u64,
        resident_size_max: u64,
        user_time: [i32; 2],
        system_time: [i32; 2],
        policy: i32,
        suspend_count: i32,
    }
    unsafe extern "C" {
        static mach_task_self_: u32;
        fn task_info(task: u32, flavor: i32, info: *mut BasicInfo, n: *mut u32) -> i32;
    }
    let mut i = BasicInfo::default();
    let mut n = (std::mem::size_of::<BasicInfo>() / 4) as u32;
    // SAFETY: MACH_TASK_BASIC_INFO = 20; `i` has the reported size.
    if unsafe { task_info(mach_task_self_, 20, &mut i, &mut n) } != 0 {
        return Err(OsError::from_code(os_code::INVALID));
    }
    Ok(i.resident_size as i64)
}

/// `Process::MaxRSS` (`getrusage`: KiB on Linux, bytes on macOS).
#[cfg(unix)]
fn max_resident_memory() -> IoResult<i64> {
    #[repr(C)]
    struct Usage {
        times: [i64; 4],
        maxrss: std::ffi::c_long,
        rest: [std::ffi::c_long; 13],
    }
    unsafe extern "C" {
        fn getrusage(who: i32, usage: *mut Usage) -> i32;
    }
    // SAFETY: the structure is plain data; the function fills it (RUSAGE_SELF = 0).
    let mut u: Usage = unsafe { std::mem::zeroed() };
    if unsafe { getrusage(0, &mut u) } < 0 {
        return Err(OsError::of(&std::io::Error::last_os_error()));
    }
    Ok(if cfg!(target_os = "linux") { u.maxrss as i64 * 1024 } else { u.maxrss as i64 })
}

#[cfg(windows)]
fn memory_counters() -> IoResult<(i64, i64)> {
    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        cb: u32,
        misses: u32,
        peak: usize,
        current: usize,
        rest: [usize; 6],
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn K32GetProcessMemoryInfo(p: *mut std::ffi::c_void, c: *mut Counters, n: u32) -> i32;
    }
    let mut c = Counters { cb: std::mem::size_of::<Counters>() as u32, ..Default::default() };
    // SAFETY: `c` has the reported size.
    if unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) } == 0 {
        return Err(OsError::of(&std::io::Error::last_os_error()));
    }
    Ok((c.current as i64, c.peak as i64))
}
#[cfg(windows)]
fn resident_memory() -> IoResult<i64> {
    memory_counters().map(|c| c.0)
}
#[cfg(windows)]
fn max_resident_memory() -> IoResult<i64> {
    memory_counters().map(|c| c.1)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_ProcessInfo_CurrentRSS() -> i64 {
    dart_or_error(resident_memory(), dart_int)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_ProcessInfo_MaxRSS() -> i64 {
    dart_or_error(max_resident_memory(), dart_int)
}
