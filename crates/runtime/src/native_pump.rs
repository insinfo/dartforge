// Opt-in macOS main-thread CFRunLoop integration. This is a wait/poll adapter,
// not AppKit NSEvent dispatch and not a nested Dart event-loop API.

// Only the owner calls clone/take/publish. Other threads may execute a
// notification while borrowing the installed value under this same mutex.
struct NativePumpSlot<T> {
    state: std::sync::Mutex<Option<std::sync::Arc<T>>>,
}

impl<T> NativePumpSlot<T> {
    fn new() -> Self {
        Self {
            state: std::sync::Mutex::new(None),
        }
    }

    fn notify(&self, signal: impl FnOnce(&T)) {
        let installed = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(pump) = installed.as_ref() {
            signal(pump);
        }
    }

    fn owner_clone(&self) -> Option<std::sync::Arc<T>> {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn take(&self) -> Option<std::sync::Arc<T>> {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).take()
    }

    fn publish(&self, pump: std::sync::Arc<T>) {
        let previous = self
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .replace(pump);
        // Destruction happens after releasing the installation lock.
        drop(previous);
    }
}

#[derive(Default)]
struct NativePumpNotification(std::sync::atomic::AtomicBool);

impl NativePumpNotification {
    fn signal(&self) {
        self.0.store(true, std::sync::atomic::Ordering::Release);
    }
    fn take(&self) -> bool {
        self.0.swap(false, std::sync::atomic::Ordering::AcqRel)
    }
}

struct NativePump {
    #[cfg(target_os = "macos")]
    run_loop: usize,
    #[cfg(target_os = "macos")]
    source: usize,
    pending: NativePumpNotification,
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct NativePumpSourceContext {
    version: isize,
    info: *mut std::ffi::c_void,
    retain: usize,
    release: usize,
    copy_description: usize,
    equal: usize,
    hash: usize,
    schedule: usize,
    cancel: usize,
    perform: extern "C" fn(*mut std::ffi::c_void),
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    #[link_name = "CFRunLoopGetMain"]
    fn pump_get_main() -> *const std::ffi::c_void;
    #[link_name = "CFRetain"]
    fn pump_retain(value: *const std::ffi::c_void) -> *const std::ffi::c_void;
    #[link_name = "CFRelease"]
    fn pump_release(value: *const std::ffi::c_void);
    #[link_name = "CFRunLoopSourceCreate"]
    fn pump_source_create(
        allocator: *const std::ffi::c_void,
        order: isize,
        context: *mut NativePumpSourceContext,
    ) -> *const std::ffi::c_void;
    #[link_name = "CFRunLoopAddSource"]
    fn pump_add_source(
        run_loop: *const std::ffi::c_void,
        source: *const std::ffi::c_void,
        mode: *const std::ffi::c_void,
    );
    #[link_name = "CFRunLoopRemoveSource"]
    fn pump_remove_source(
        run_loop: *const std::ffi::c_void,
        source: *const std::ffi::c_void,
        mode: *const std::ffi::c_void,
    );
    #[link_name = "CFRunLoopSourceInvalidate"]
    fn pump_invalidate_source(source: *const std::ffi::c_void);
    #[link_name = "CFRunLoopSourceSignal"]
    fn pump_signal_source(source: *const std::ffi::c_void);
    #[link_name = "CFRunLoopWakeUp"]
    fn pump_wake(run_loop: *const std::ffi::c_void);
    #[link_name = "CFRunLoopRunInMode"]
    fn pump_run(mode: *const std::ffi::c_void, seconds: f64, return_after_source: u8) -> i32;
    #[link_name = "kCFRunLoopDefaultMode"]
    static PUMP_DEFAULT_MODE: *const std::ffi::c_void;
    #[link_name = "kCFRunLoopCommonModes"]
    static PUMP_COMMON_MODES: *const std::ffi::c_void;
    #[link_name = "pthread_main_np"]
    fn pump_is_main_thread() -> i32;
}

#[cfg(target_os = "macos")]
extern "C" fn native_pump_perform(_info: *mut std::ffi::c_void) {
    // CF consumes its own latched source signal. Dart work stays in the outer
    // runtime loop, so this callback cannot overwrite active port dispatch state.
}

impl NativePump {
    fn notify(&self) {
        self.pending.signal();
        #[cfg(target_os = "macos")]
        // SAFETY: caller holds the queue's installation lock or an owner-thread
        // Arc. Both CF objects remain retained, even during a concurrent teardown.
        unsafe {
            pump_signal_source(self.source as *const _);
            pump_wake(self.run_loop as *const _);
        }
    }

    fn take_notification(&self) -> bool {
        self.pending.take()
    }

    fn run(&self, deadline: Option<std::time::Instant>, poll: bool) {
        // Consume the Rust latch only for a blocking wait. CF also retains its
        // source signal, covering notification between this check and entering CF.
        if !poll && self.take_notification() {
            return;
        }
        #[cfg(target_os = "macos")]
        {
            let seconds = if poll {
                0.0
            } else {
                deadline.map_or(1.0e10, |d| {
                    d.saturating_duration_since(std::time::Instant::now())
                        .as_secs_f64()
                })
            };
            // SAFETY: only the process main thread owns/runs the pump. No queue,
            // heap or EVENTS borrow is retained by the caller across this call.
            // CF may invoke ordinary native callbacks, including local Dart FFI.
            unsafe {
                pump_run(PUMP_DEFAULT_MODE, seconds, 1);
            }
            NATIVE_PUMP_DISPATCH_READY.with(|ready| ready.set(true));
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for NativePump {
    fn drop(&mut self) {
        // Only owner-thread wait/poll operations clone Arc. Senders signal while
        // borrowing under the installation lock, so destruction stays on owner.
        debug_assert_eq!(unsafe { pump_is_main_thread() }, 1);
        // SAFETY: no queue notifier can access this removed installation; CF's
        // callback has no Rust context, and these objects own retained references.
        unsafe {
            pump_remove_source(
                self.run_loop as *const _,
                self.source as *const _,
                PUMP_COMMON_MODES,
            );
            pump_invalidate_source(self.source as *const _);
            pump_release(self.source as *const _);
            pump_release(self.run_loop as *const _);
        }
    }
}

fn native_pump_owner_status() -> i64 {
    #[cfg(target_os = "macos")]
    {
        if unsafe { pump_is_main_thread() } == 1 {
            1
        } else {
            -1
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        0
    }
}

thread_local! {
    static NATIVE_PUMP_ENABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static NATIVE_PUMP_DISPATCH_READY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn native_pump_signal_current() {
    if NATIVE_PUMP_ENABLED.with(|enabled| enabled.get()) {
        QUEUE.with(|q| q.native_pump.notify(|pump| pump.notify()));
    }
}

fn native_pump_current() -> Option<std::sync::Arc<NativePump>> {
    if !NATIVE_PUMP_ENABLED.with(|enabled| enabled.get()) {
        return None;
    }
    QUEUE.with(|q| q.native_pump.owner_clone())
}

fn native_pump_detach(queue: &IsolateQueue) {
    NATIVE_PUMP_ENABLED.with(|enabled| enabled.set(false));
    NATIVE_PUMP_DISPATCH_READY.with(|ready| ready.set(false));
    replace_native_event_dispatcher(None);
    let pump = queue.native_pump.take();
    if let Some(pump) = pump {
        // A disable invoked by a native callback wakes the outer CF invocation.
        // Its owner-thread Arc defers release until CF has returned.
        pump.notify();
        drop(pump);
    }
}

fn native_pump_poll() {
    if let Some(pump) = native_pump_current() {
        pump.run(None, true);
    }
}

/// Whether this isolate can install the macOS process-main-thread pump.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_native_pump_supported() -> u8 {
    u8::from(native_pump_owner_status() == 1)
}

/// Install the native pump and keep the isolate alive until explicit disable.
/// Status: 1 success/already enabled, 0 unsupported OS, -1 wrong thread,
/// -2 CoreFoundation setup failure. No AppKit or nested Dart dispatch is started.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_native_pump_enable() -> i64 {
    let status = native_pump_owner_status();
    if status != 1 {
        return status;
    }
    if native_pump_current().is_some() {
        return 1;
    }
    #[cfg(target_os = "macos")]
    {
        let mut context = NativePumpSourceContext {
            version: 0,
            info: std::ptr::null_mut(),
            retain: 0,
            release: 0,
            copy_description: 0,
            equal: 0,
            hash: 0,
            schedule: 0,
            cancel: 0,
            perform: native_pump_perform,
        };
        // SAFETY: CF copies the valid version-zero context. It carries no Rust
        // pointer; the source and main run loop are retained until detach.
        let source = unsafe { pump_source_create(std::ptr::null(), 0, &mut context) };
        if source.is_null() {
            return -2;
        }
        let run_loop = unsafe { pump_get_main() };
        if run_loop.is_null() {
            unsafe {
                pump_release(source);
            }
            return -2;
        }
        unsafe {
            pump_retain(run_loop);
            pump_add_source(run_loop, source, PUMP_COMMON_MODES);
        }
        let pump = std::sync::Arc::new(NativePump {
            run_loop: run_loop as usize,
            source: source as usize,
            pending: NativePumpNotification::default(),
        });
        // Latch an initial signal before publishing under the notifier lock.
        // Work posted before publication therefore cannot be stranded.
        QUEUE.with(|q| {
            pump.notify();
            q.native_pump.publish(pump);
        });
        NATIVE_PUMP_ENABLED.with(|enabled| enabled.set(true));
        1
    }
    #[cfg(not(target_os = "macos"))]
    {
        0
    }
}

/// Remove the current installation. Status: 1 success/already disabled,
/// 0 unsupported OS, -1 wrong thread. Ordinary ports/timers retain their lifetimes.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_native_pump_disable() -> i64 {
    let status = native_pump_owner_status();
    if status != 1 {
        return status;
    }
    QUEUE.with(|q| native_pump_detach(q));
    1
}

/// Whether the owner supports registering outer-turn native event work.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_native_pump_dispatcher_supported() -> u8 {
    u8::from(native_pump_owner_status() == 1)
}

/// Root the application dispatcher. Status: 1 success, 0 unsupported OS,
/// -1 wrong thread, -3 native pump not enabled. Never invokes the callback here.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_native_pump_set_dispatcher(closure: i64) -> i64 {
    let status = native_pump_owner_status();
    if status != 1 {
        return status;
    }
    if !NATIVE_PUMP_ENABLED.with(|enabled| enabled.get()) {
        return -3;
    }
    replace_native_event_dispatcher(Some(closure));
    NATIVE_PUMP_DISPATCH_READY.with(|ready| ready.set(true));
    native_pump_signal_current();
    1
}

/// Clear the application dispatcher. Status: 1 success/already clear,
/// 0 unsupported OS, -1 wrong thread, -3 native pump not enabled.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_native_pump_clear_dispatcher() -> i64 {
    let status = native_pump_owner_status();
    if status != 1 {
        return status;
    }
    if !NATIVE_PUMP_ENABLED.with(|enabled| enabled.get()) {
        return -3;
    }
    replace_native_event_dispatcher(None);
    NATIVE_PUMP_DISPATCH_READY.with(|ready| ready.set(false));
    1
}

#[cfg(test)]
mod tests_native_pump_lifecycle {
    use super::*;
    use std::sync::{
        Arc, Barrier,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    struct CheckedInstallation {
        owner: std::thread::ThreadId,
        notifying: AtomicBool,
        dropped: Arc<AtomicUsize>,
    }

    impl Drop for CheckedInstallation {
        fn drop(&mut self) {
            assert!(!self.notifying.load(Ordering::Acquire));
            assert_eq!(std::thread::current().id(), self.owner);
            self.dropped.fetch_add(1, Ordering::AcqRel);
        }
    }

    #[test]
    fn removal_serializes_with_sender_and_destroys_on_owner() {
        let dropped = Arc::new(AtomicUsize::new(0));
        let slot = Arc::new(NativePumpSlot::new());
        slot.publish(Arc::new(CheckedInstallation {
            owner: std::thread::current().id(),
            notifying: AtomicBool::new(false),
            dropped: dropped.clone(),
        }));
        let gate = Arc::new(Barrier::new(2));
        let (entered, receiver) = std::sync::mpsc::channel();
        let sender_slot = slot.clone();
        let sender_gate = gate.clone();
        let sender = std::thread::spawn(move || {
            sender_slot.notify(|pump| {
                pump.notifying.store(true, Ordering::Release);
                entered.send(()).unwrap();
                sender_gate.wait();
                pump.notifying.store(false, Ordering::Release);
            })
        });
        receiver.recv().unwrap();
        assert_eq!(dropped.load(Ordering::Acquire), 0);
        let release = std::thread::spawn(move || {
            gate.wait();
        });
        drop(slot.take());
        sender.join().unwrap();
        release.join().unwrap();
        assert_eq!(dropped.load(Ordering::Acquire), 1);
        slot.notify(|_| panic!("notification reached a removed installation"));
        assert!(slot.owner_clone().is_none());
    }

    #[test]
    fn active_owner_defers_release_until_wait_returns() {
        let dropped = Arc::new(AtomicUsize::new(0));
        let slot = NativePumpSlot::new();
        slot.publish(Arc::new(CheckedInstallation {
            owner: std::thread::current().id(),
            notifying: AtomicBool::new(false),
            dropped: dropped.clone(),
        }));
        let active_wait = slot.owner_clone().unwrap();
        drop(slot.take());
        assert_eq!(dropped.load(Ordering::Acquire), 0);
        drop(active_wait);
        assert_eq!(dropped.load(Ordering::Acquire), 1);
    }

    #[test]
    fn notifications_are_latched_and_coalesced() {
        // Exercise the production atomic operation without constructing CF
        // objects on the Rust test runner's helper thread.
        let pending = NativePumpNotification::default();
        pending.signal();
        pending.signal();
        assert!(pending.take());
        assert!(!pending.take());
        // A notification after the consume remains visible to the next wait.
        pending.signal();
        assert!(pending.take());
    }

    #[test]
    fn unsupported_owner_cannot_install_or_detach() {
        let (supported, enable, disable) = std::thread::spawn(|| {
            (
                dartforge_native_DartForge_native_pump_supported(),
                dartforge_native_DartForge_native_pump_enable(),
                dartforge_native_DartForge_native_pump_disable(),
            )
        })
        .join()
        .unwrap();
        assert_eq!(supported, 0);
        let expected = if cfg!(target_os = "macos") { -1 } else { 0 };
        assert_eq!(enable, expected);
        assert_eq!(disable, expected);
    }
}
