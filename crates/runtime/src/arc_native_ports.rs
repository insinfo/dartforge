// The boundary between Owned Dart callbacks and borrowing native consumers.
// Registration is explicit: addresses and words do not declare ownership.

#[derive(Clone, Copy)]
struct ArcReturnRecord {
    version: i64,
    calls: usize,
    uniform: bool,
}

struct ArcReturnScope {
    id: i64,
    calls: usize,
    owners: Vec<i64>,
}

#[derive(Default)]
struct ArcNativePorts {
    entries: HashMap<(usize, usize), ArcReturnRecord>,
    typed_lists: HashMap<(usize, i64), i64>,
}

// The base-5 ABI omits the initial Ref context; it encodes arguments and return.
// A Ref transfers an owner; i64/f64/bool are scalars without a managed token.
fn check_arc_typed_abi(mut abi: i64) -> Result<(), &'static str> {
    if abi <= 1 || !matches!(abi % 5, 1..=4) { return Err("invalid typed return"); }
    let mut n = 0;
    while abi > 1 {
        if !matches!(abi % 5, 1..=4) { return Err("invalid typed representation"); }
        abi /= 5; n += 1;
        if n > 13 { return Err("typed arity out of bounds"); }
    }
    if abi != 1 { return Err("invalid typed ABI prefix"); }
    Ok(())
}

fn arc_typed_target(
    heap: &Heap, ports: &ArcNativePorts, closure: i64, abi: i64,
) -> Result<i64, &'static str> {
    check_arc_typed_abi(abi)?;
    let object = heap.object(closure).ok_or("typed receiver is not a live object")?;
    if object.class_id != i64::from(crate::layout::cid::CLOSURE) || object.len() != 4 {
        return Err("typed receiver is not a four-field closure");
    }
    let (code, code_ref) = object.get(2).ok_or("missing typed body")?;
    let (declared, ref_abi) = object.get(3).ok_or("missing typed ABI")?;
    if code_ref || ref_abi || code == 0 || declared != abi {
        return Err("typed body/ABI incompatible with the physical map");
    }
    let code_usize = usize::try_from(code as u64).map_err(|_| "invalid typed address")?;
    if !ports.typed_lists.contains_key(&(code_usize, abi)) {
        return Err("typed body without an ARC registration of this ABI");
    }
    Ok(code)
}

impl ArcNativePorts {
    fn register_typed(&mut self, code: usize, abi: i64) -> Result<i64, &'static str> {
        check_arc_typed_abi(abi)?;
        if code == 0 { return Err("null typed body"); }
        Ok(*self.typed_lists.entry((code, abi)).or_insert_with(new_arc_port_id))
    }

    fn remove_typed(&mut self, code: usize, abi: i64, version: i64) -> Result<(), &'static str> {
        if self.typed_lists.get(&(code, abi)) != Some(&version) {
            return Err("missing typed registration or stale version");
        }
        self.typed_lists.remove(&(code, abi));
        Ok(())
    }
    fn register(&mut self, code: usize, arity: usize) -> Result<i64, &'static str> {
        self.register_contract(code, arity, false)
    }

    fn register_contract(
        &mut self,
        code: usize,
        arity: usize,
        uniform: bool,
    ) -> Result<i64, &'static str> {
        if code == 0 || !matches!(arity, 0..=5 | 7) || (uniform && arity != 3) {
            return Err("callback ABI not covered");
        }
        // The linker may merge the code of functions with different signatures.
        // The declaration belongs to the logical ABI (address, arity), not just the PC.
        if let Some(r) = self.entries.get(&(code, arity)) {
            if r.uniform != uniform {
                return Err("uniform contract conflicts with a word callback");
            }
            return Ok(r.version);
        }
        let version = new_arc_port_id();
        self.entries.insert(
            (code, arity),
            ArcReturnRecord {
                version,
                calls: 0,
                uniform,
            },
        );
        Ok(version)
    }

    fn deregister(&mut self, code: usize, version: i64) -> Result<(), &'static str> {
        let key = self
            .entries
            .iter()
            .find_map(|(k, r)| (k.0 == code && r.version == version).then_some(*k))
            .ok_or("callback not registered or stale version")?;
        if self.entries[&key].calls != 0 {
            return Err("active callback");
        }
        self.entries.remove(&key);
        Ok(())
    }
}

#[derive(Default)]
struct ArcPortScopes {
    scopes: Vec<ArcReturnScope>,
}

impl ArcPortScopes {
    fn do_close(&mut self, id: i64) -> Result<Vec<i64>, &'static str> {
        let e = self.scopes.last().ok_or("missing native scope")?;
        if e.id != id || e.calls != 0 {
            return Err("scope not at the top or active callback");
        }
        Ok(self.scopes.pop().unwrap().owners)
    }
}

fn new_arc_port_id() -> i64 {
    static NEXT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
    NEXT
        .fetch_update(
            std::sync::atomic::Ordering::Relaxed,
            std::sync::atomic::Ordering::Relaxed,
            |n| n.checked_add(1),
        )
        .expect("ARC port IDs exhausted")
        + 1
}

// Code belongs to the image, shared by the isolates. The global count prevents
// removing a version that is running on any thread.
fn arc_port_records() -> &'static std::sync::Mutex<ArcNativePorts> {
    static RECORDS: std::sync::OnceLock<std::sync::Mutex<ArcNativePorts>> =
        std::sync::OnceLock::new();
    RECORDS.get_or_init(|| std::sync::Mutex::new(ArcNativePorts::default()))
}

static HAS_ARC_RETURN_PORTS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

thread_local! {
    // The managed values belong to the heap of the thread that received the return.
    static ARC_NATIVE_PORTS: RefCell<ArcPortScopes> = RefCell::new(ArcPortScopes::default());
}

/// Declares an Owned Ref return of a word callback and returns its version.
///
/// A repeated registration with the same ABI is idempotent. Does not call code,
/// collect or run Dart. Neither selects the memory mode nor proves the Dart body.
///
/// # Safety
/// `code` must have the ABI of the port given by `arity`, with one-word parameters
/// and an Owned Ref result on the normal exit, null on the pending exit. Ref
/// parameters are borrowed; the callback does not consume runtime tokens. The
/// callback must restore the native scopes before returning. The producer must
/// verify the body and keep the image valid until it removes the registration and
/// all references/calls to the code. Does not admit an f64 ABI.
///
/// # Panics
/// Null code or an arity not covered; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn null_val() -> i64 { 0 }
/// // SAFETY: null is a Ref without a physical token; static zero-arity function.
/// let v = unsafe { dartforge_arc_register_return_owned_v1(null_val as usize as i64, 0) };
/// dartforge_arc_take_return_owned_v1(null_val as usize as i64, v);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_register_return_owned_v1(
    code: i64,
    arity: i64,
) -> i64 {
    let code = usize::try_from(code as u64).expect("invalid callback address");
    let arity = usize::try_from(arity).expect("negative arity");
    let mut p = arc_port_records()
        .lock()
        .expect("ARC port registry poisoned");
    let version = p
        .register(code, arity)
        .expect("ARC callback contract violated");
    HAS_ARC_RETURN_PORTS.store(true, std::sync::atomic::Ordering::Release);
    version
}

/// Declares an Owned uniform entry with read-only vectors.
///
/// The contract is distinct from the generic three-word callback. The registration
/// neither verifies the body nor selects the memory mode.
///
/// # Safety
/// The code has the ABI `(Ref, ptr args, ptr desc) -> Ref Owned`, null when pending.
/// The receiver and the Ref elements are borrowed. It neither changes, publishes nor
/// retains the vectors' addresses; it may copy their elements respecting
/// ownership. Every transitive call that receives those addresses respects the same
/// contract. The producer verifies the emitted body and restores the scopes on every
/// exit. The image lives until the registration and all the code's consumers are
/// removed.
///
/// # Panics
/// Null code or an incompatible previous contract; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn smi(_: i64, _: *const i64, _: *const i64) -> i64 { 3 }
/// // SAFETY: a static body that does not publish the vectors, returns a Smi without a token.
/// let v = unsafe { dartforge_arc_register_uniform_owned_v1(smi as *const () as usize as i64) };
/// dartforge_arc_take_return_owned_v1(smi as *const () as usize as i64, v);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_register_uniform_owned_v1(code: i64) -> i64 {
    let code = usize::try_from(code as u64).expect("invalid uniform address");
    let mut p = arc_port_records().lock().expect("ARC port registry poisoned");
    let version = p.register_contract(code, 3, true).expect("ARC uniform contract violated");
    HAS_ARC_RETURN_PORTS.store(true, std::sync::atomic::Ordering::Release);
    version
}

/// Registers a typed closure entry with an Owned or scalar return.
///
/// The base-5 ABI encodes the arguments (Ref=1, i64=2, f64=3, bool=4) and the
/// return, after the prefix 1; the initial Ref context is implicit. Does not call
/// code.
///
/// # Safety
/// The body was verified in this ABI: borrowed Refs, an Owned Ref or Trivial scalar
/// return; a canonical result when pending and restored scopes. A scalar i64 does
/// not contain a managed Dart word. The registration does not prove the body. Keep
/// the code/image valid until the version is removed and every consumer of the
/// address has finished. Does not establish JIT reload pins.
///
/// # Panics
/// Null code, an invalid ABI or more than 12 arguments.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn body(_: i64) -> i64 { 0 }
/// let pc = body as *const () as usize as i64;
/// // SAFETY: static function, borrowed context and a null Owned return.
/// let v = unsafe { dartforge_arc_register_typed_v1(pc, 6) };
/// // SAFETY: no consumer/call of the address is active.
/// unsafe { dartforge_arc_take_typed_v1(pc, 6, v); }
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_register_typed_v1(code: i64, abi: i64) -> i64 {
    arc_port_records().lock().expect("ARC port registry poisoned")
        .register_typed(usize::try_from(code as u64).expect("invalid typed address"), abi)
        .expect("invalid ARC typed registration")
}

/// Removes exactly one version and ABI of a typed entry.
///
/// # Safety
/// There must be no active call nor consumer of an address obtained through
/// `dartforge_arc_typed_target_v1`. The code must stay valid until here.
///
/// # Panics
/// Missing registration or stale version; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn body(_: i64) -> i64 { 0 }
/// let pc = body as *const () as usize as i64;
/// // SAFETY: static ABI and no active consumer.
/// let v = unsafe { dartforge_arc_register_typed_v1(pc, 6) };
/// unsafe { dartforge_arc_take_typed_v1(pc, 6, v); }
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_take_typed_v1(code: i64, abi: i64, version: i64) {
    arc_port_records().lock().expect("ARC port registry poisoned")
        .remove_typed(usize::try_from(code as u64).expect("invalid typed address"), abi, version)
        .expect("invalid ARC typed removal");
}

/// Checks the class, the map, the ABI and the ARC registration before returning
/// native code.
///
/// Neither retains, allocates, collects nor runs Dart. The result is a native
/// address, not a Ref. Its validity depends on the registration contract and on the
/// image's lifetime.
///
/// # Panics
/// Invalid receiver, fields, ABI or registration; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn body(_: i64) -> i64 { 0 }
/// let pc = body as *const () as usize as i64;
/// // SAFETY: static ABI with a Ref context and a null Owned Ref result.
/// let v = unsafe { dartforge_arc_register_typed_v1(pc, 6) };
/// let c = dartforge_arc_closure_owned_v1(0, 0, pc, 6);
/// assert_eq!(dartforge_arc_typed_target_v1(c, 6), pc);
/// dartforge_arc_release(c);
/// // SAFETY: no consumer of the address remains.
/// unsafe { dartforge_arc_take_typed_v1(pc, 6, v); }
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_typed_target_v1(closure: i64, abi: i64) -> i64 {
    HEAP.with(|heap| arc_typed_target(&heap.borrow(),
        &arc_port_records().lock().expect("ARC port registry poisoned"), closure, abi)
        .expect("invalid ARC typed target"))
}

struct ArcUniformScope(i64);

impl Drop for ArcUniformScope {
    fn drop(&mut self) {
        dartforge_arc_native_scope_close_v1(self.0);
    }
}

/// `f` inside a native scope, without changing the result: the tokens that Dart
/// ports return during `f` die at the closing, even on a panic.
pub(crate) fn in_native_scope<T>(f: impl FnOnce() -> T) -> T {
    let scope = ArcUniformScope(dartforge_arc_native_scope_open_v1());
    let r = f();
    drop(scope);
    r
}

/// `f` in a native scope only when some image registered ARC ports (an event of the
/// loop, the life of an isolate): without them, nothing changes.
pub(crate) fn in_native_scope_if_arc<T>(f: impl FnOnce() -> T) -> T {
    if HAS_ARC_RETURN_PORTS.load(std::sync::atomic::Ordering::Acquire) {
        in_native_scope(f)
    } else {
        f()
    }
}

/// Runs `f` (which may run Dart and returns a live object when it comes back) in a
/// native scope and hands over the result with a code owner. The tokens of the Dart
/// ports' Owned returns stay in the scope and are released when it closes, after
/// the copy. With a pending exception, null and no owner.
pub(crate) fn owner_in_native_scope(f: impl FnOnce() -> i64) -> i64 {
    let scope = ArcUniformScope(dartforge_arc_native_scope_open_v1());
    let r = f();
    let result = if dartforge_exception_pending() != 0 {
        0
    } else {
        dartforge_arc_retain(r);
        r
    };
    drop(scope);
    result
}

fn arc_invoke_uniform(
    code: usize,
    recv: i64,
    args: i64,
    desc: i64,
) -> Result<i64, &'static str> {
    let scope = ArcUniformScope(dartforge_arc_native_scope_open_v1());
    let result = if code == dartforge_nsm_selector as *const () as usize {
        // This native body reads/copies the vectors, without forwarding them to the SDK.
        // The Dart helper uses dart_r7: its Owned stays borrowed in this scope.
        let r = dartforge_nsm_selector(recv, args as *const i64, desc as *const i64);
        if dartforge_exception_pending() != 0 {
            assert_eq!(r, 0, "a pending noSuchMethod must return null");
            0
        } else {
            // The old convention returns Borrowed, even if the helper was Owned
            // captured by the boundary. Copies before releasing the scope. Under the
            // real ARC the fallback already returns Owned.
            if !HEAP.with(|h| h.borrow().prompt_death()) {
                dartforge_arc_retain(r);
            }
            r
        }
    } else {
        let guard = arc_prepare_contract_port(code, 3, true)?
            .ok_or("uniform entry not registered")?;
        let r = dart_r3_invoke(code, recv, args, desc);
        arc_check_port_scope(&guard);
        arc_validate_owned_return(r);
        // The direct return is already Owned: it neither transfers it to the scope
        // nor retains it. The guard must end before the closing of the scope it pins.
        drop(guard);
        r
    };
    drop(scope);
    Ok(result)
}

/// Invokes a certified uniform entry and returns exactly one Owned return.
///
/// Accepts the native noSuchMethod fallback, adapting its borrowed return. Closes
/// the callback scope also when pending, without clearing the exception. Unknown
/// code and a generic callback are not inferred as uniform.
///
/// # Safety
/// `code` and its image stay alive during the call. `args` and `desc` are valid
/// addresses of the uniform ABI: two non-negative counters, named hashes and a
/// vector with all the arguments plus the RTI tuple. The receiver and the Ref
/// elements stay alive for the whole reach of the call.
///
/// # Panics
/// An uncertified entry or an ownership/scope violation; aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn null_val(_: i64, _: *const i64, _: *const i64) -> i64 { 0 }
/// let pc = null_val as *const () as usize as i64;
/// // SAFETY: the body neither publishes vectors nor creates tokens, returns a null Ref.
/// let v = unsafe { dartforge_arc_register_uniform_owned_v1(pc) };
/// let args = [0_i64]; let desc = [0_i64, 0];
/// // SAFETY: valid vectors and a static registered body.
/// assert_eq!(unsafe { dartforge_arc_invoke_uniform_v1(pc, 0,
///     args.as_ptr() as i64, desc.as_ptr() as i64) }, 0);
/// dartforge_arc_take_return_owned_v1(pc, v);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_invoke_uniform_v1(
    code: i64,
    recv: i64,
    args: i64,
    desc: i64,
) -> i64 {
    let code = usize::try_from(code as u64).expect("invalid uniform address");
    arc_invoke_uniform(code, recv, args, desc).expect("invalid ARC uniform boundary")
}

fn arc_invoke_uniform_closure(closure: i64, args: i64, desc: i64) -> Result<i64, &'static str> {
    // The entry lookup may also run the Dart error helper. It pins its intermediate
    // returns until the exception is published, even if it fails.
    let scope = ArcUniformScope(dartforge_arc_native_scope_open_v1());
    let code = dartforge_closure_entry(closure);
    let result = if code == 0 {
        assert_ne!(dartforge_exception_pending(), 0, "invalid closure without an exception");
        0
    } else {
        arc_invoke_uniform(code as u64 as usize, closure, args, desc)?
    };
    drop(scope);
    Ok(result)
}

/// Invokes a closure or callable object with an Owned uniform return.
///
/// An invalid lookup preserves the exception and returns null, without invoking
/// code. Valid entries need the uniform registration; a generic callback is not
/// enough. Closes the lookup and call scopes, preserving the result's single owner.
///
/// # Safety
/// The closure/receiver and the Ref elements stay alive for the whole call. `args`
/// and `desc` point to valid read-only vectors of the uniform ABI. The code image
/// stays alive while the closure has consumers.
///
/// # Panics
/// A destination without a uniform contract or an ownership/scope violation; aborts
/// at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn null_val(_: i64, _: *const i64, _: *const i64) -> i64 { 0 }
/// let pc = null_val as *const () as usize as i64;
/// // SAFETY: a static read-only body with a null Ref return.
/// let v = unsafe { dartforge_arc_register_uniform_owned_v1(pc) };
/// let c = dartforge_arc_closure_owned_v1(pc, 0, 0, 0);
/// let args = [0_i64]; let desc = [0_i64, 0];
/// // SAFETY: live closure and vectors; registered static code.
/// assert_eq!(unsafe { dartforge_arc_invoke_uniform_closure_v1(c,
///     args.as_ptr() as i64, desc.as_ptr() as i64) }, 0);
/// dartforge_arc_release(c);
/// dartforge_arc_take_return_owned_v1(pc, v);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_invoke_uniform_closure_v1(closure: i64, args: i64, desc: i64) -> i64 {
    arc_invoke_uniform_closure(closure, args, desc).expect("invalid ARC uniform closure boundary")
}

/// Removes exactly the registered version, without invalidating a later version.
///
/// Must happen after removing the address's consumers. Rejects callbacks in
/// progress; the owners already returned stay in the native scopes. Neither
/// collects, runs Dart nor changes the pending exception.
///
/// # Panics
/// Missing registration, stale version or an active callback; aborts at the C
/// boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn null_val(_: i64) -> i64 { 0 }
/// // SAFETY: a static word ABI with a null Ref return.
/// let v = unsafe { dartforge_arc_register_return_owned_v1(null_val as usize as i64, 1) };
/// dartforge_arc_take_return_owned_v1(null_val as usize as i64, v);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_take_return_owned_v1(code: i64, version: i64) {
    let code = usize::try_from(code as u64).expect("invalid callback address");
    let mut p = arc_port_records()
        .lock()
        .expect("ARC port registry poisoned");
    p.deregister(code, version)
        .expect("invalid ARC callback removal");
    HAS_ARC_RETURN_PORTS.store(!p.entries.is_empty(), std::sync::atomic::Ordering::Release);
}

/// Opens a native scope that receives the tokens returned by Owned callbacks.
///
/// Each return is transferred without an additional retain and stays borrowed by
/// the native consumer until this scope closes. To escape, copy first. Reentrant
/// scopes are independent. Neither collects nor runs Dart.
///
/// # Panics
/// IDs exhausted; an internal failure aborts at the C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let e = dartforge_arc_native_scope_open_v1();
/// dartforge_arc_native_scope_close_v1(e);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_native_scope_open_v1() -> i64 {
    ARC_NATIVE_PORTS.with(|p| {
        let mut p = p.borrow_mut();
        let id = new_arc_port_id();
        p.scopes.push(ArcReturnScope {
            id,
            calls: 0,
            owners: Vec::new(),
        });
        id
    })
}

/// Closes the top scope and releases exactly the tokens it received.
///
/// Invalidates the borrows of the returns. Neither collects, runs Dart nor clears
/// the pending exception. The emitter will have to close it on every exit, with
/// exception cleanup; this API does not install that cleanup automatically.
///
/// # Panics
/// A missing scope, one not at the top or with a callback in progress; aborts at the
/// C boundary.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let e = dartforge_arc_native_scope_open_v1();
/// dartforge_arc_native_scope_close_v1(e);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_native_scope_close_v1(id: i64) {
    let owners = ARC_NATIVE_PORTS.with(|p| {
        p.borrow_mut()
            .do_close(id)
            .expect("invalid ARC native scope closing")
    });
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        for owner in owners {
            h.drop_held_code_token(owner);
        }
        h.arc_settle();
    });
}

struct ArcPortGuard {
    code: usize,
    arity: usize,
    version: i64,
    scope: i64,
}

fn arc_prepare_port(
    code: usize,
    arity: usize,
) -> Result<Option<ArcPortGuard>, &'static str> {
    arc_prepare_contract_port(code, arity, false)
}

fn arc_prepare_contract_port(
    code: usize,
    arity: usize,
    uniform: bool,
) -> Result<Option<ArcPortGuard>, &'static str> {
    // The old convention neither takes a lock nor creates a scope when no image
    // declared Owned callbacks. This does not select ARC implicitly.
    if !HAS_ARC_RETURN_PORTS.load(std::sync::atomic::Ordering::Acquire) {
        return Ok(None);
    }
    let mut p = arc_port_records()
        .lock()
        .expect("ARC port registry poisoned");
    let Some(r) = p.entries.get(&(code, arity)).copied() else {
        return if p.entries.keys().any(|k| k.0 == code) {
            Err("ARC callback signature not registered")
        } else {
            Ok(None)
        };
    };
    if uniform && !r.uniform {
        return Err("entry without a uniform contract");
    }
    let active_list = r
        .calls
        .checked_add(1)
        .ok_or("active callbacks exhausted")?;
    let scope = ARC_NATIVE_PORTS.with(|s| {
        let mut s = s.borrow_mut();
        let e = s
            .scopes
            .last_mut()
            .ok_or("an Owned callback requires a native scope")?;
        e.calls = e
            .calls
            .checked_add(1)
            .ok_or("active callbacks exhausted")?;
        Ok::<_, &'static str>(e.id)
    })?;
    p.entries.get_mut(&(code, arity)).unwrap().calls = active_list;
    Ok(Some(ArcPortGuard {
        code,
        arity,
        version: r.version,
        scope,
    }))
}

impl Drop for ArcPortGuard {
    fn drop(&mut self) {
        let mut p = arc_port_records()
            .lock()
            .expect("ARC port registry poisoned");
        let r = p
            .entries
            .get_mut(&(self.code, self.arity))
            .expect("active callback removed");
        assert_eq!(r.version, self.version, "active callback replaced");
        r.calls -= 1;
        ARC_NATIVE_PORTS.with(|s| {
            s.borrow_mut()
                .scopes
                .iter_mut()
                .find(|e| e.id == self.scope)
                .expect("callback scope closed")
                .calls -= 1
        });
    }
}

fn arc_validate_owned_return(result: i64) -> bool {
    if dartforge_exception_pending() != 0 {
        assert_eq!(
            result, 0,
            "an Owned callback must return null on the pending exit"
        );
        return false;
    }
    if smi::e_handle(result) {
        assert!(
            HEAP.with(|h| h.borrow().has_code_owner(result)),
            "Owned callback returned a reference without a token"
        );
    }
    true
}

fn arc_finish_port(guard: Option<ArcPortGuard>, result: i64) -> i64 {
    if let Some(g) = &guard {
        arc_check_port_scope(g);
        if !arc_validate_owned_return(result) {
            return result;
        }
        ARC_NATIVE_PORTS.with(|p| {
            let mut p = p.borrow_mut();
            let e = p.scopes.last_mut().expect("missing callback scope");
            assert_eq!(e.id, g.scope, "reentrant scope was not closed");
            e.owners.push(result);
        });
        // The scope holds the result until it closes: a virtual token becomes a root.
        HEAP.with(|h| h.borrow_mut().hold_code_token(result));
    }
    result
}

fn arc_check_port_scope(guard: &ArcPortGuard) {
    ARC_NATIVE_PORTS.with(|p| {
        assert_eq!(
            p.borrow().scopes.last().map(|e| e.id),
            Some(guard.scope),
            "reentrant scope was not closed"
        )
    });
}

fn arc_discard_port(guard: ArcPortGuard, result: i64) {
    arc_check_port_scope(&guard);
    // The guard keeps pinning the version and the scope during the release.
    if arc_validate_owned_return(result) {
        HEAP.with(|h| h.borrow_mut().release_code_owner(result));
    }
}

#[cfg(test)]
mod arc_port_tests {
    use super::*;
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn typed_target_checks_abi_map_and_version_without_mistaking_word_record() {
        let mut heap = Heap::new(false);
        let mut ports = ArcNativePorts::default();
        let c = heap.new_closure(3, (0, false), 3, 31);
        ports.register(3, 2).unwrap();
        assert!(arc_typed_target(&heap, &ports, c, 31).is_err());
        let v = ports.register_typed(3, 31).unwrap();
        assert_eq!(ports.register_typed(3, 31).unwrap(), v);
        let other = ports.register_typed(3, 41).unwrap();
        assert_ne!(other, v);
        let stats = heap.stats();
        assert_eq!(arc_typed_target(&heap, &ports, c, 31), Ok(3));
        for abi in [-1, 0, 1, 5, 7, 11, 26, i64::MAX, 41] {
            assert!(arc_typed_target(&heap, &ports, c, abi).is_err(), "{abi}");
        }
        let limit = (0..13).fold(1_i64, |a, _| a * 5 + 1);
        for abi in [6, 7, 8, 9, 31, 32, 33, 34] { assert!(check_arc_typed_abi(abi).is_ok()); }
        assert!(check_arc_typed_abi(30).is_err());
        assert!(check_arc_typed_abi(limit).is_ok());
        assert!(check_arc_typed_abi(limit * 5 + 1).is_err());
        for (n, field) in [(2, (3, true)), (3, (31, true)), (2, (0, false)), (3, (41, false))] {
            heap.set_field(c, n, field.0, field.1);
            assert!(arc_typed_target(&heap, &ports, c, 31).is_err());
            heap.set_field(c, 2, 3, false); heap.set_field(c, 3, 31, false);
        }
        for (cid, n) in [(32000, 4), (crate::layout::cid::CLOSURE, 3), (crate::layout::cid::CLOSURE, 5)] {
            let false_value = heap.new_object(i64::from(cid), &vec![(0, false); n]);
            assert!(arc_typed_target(&heap, &ports, false_value, 31).is_err());
        }
        assert!(ports.remove_typed(3, 31, other).is_err());
        ports.remove_typed(3, 31, v).unwrap();
        assert!(arc_typed_target(&heap, &ports, c, 31).is_err());
        let new = ports.register_typed(3, 31).unwrap();
        assert_ne!(new, v);
        assert!(ports.remove_typed(3, 31, v).is_err());
        assert_eq!(heap.stats().collections, stats.collections);
    }

    #[test]
    fn checked_typed_port_returns_independent_owner_in_both_modes() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        extern "C" fn body(_: i64, x: i64) -> i64 { dartforge_arc_retain(x); x }
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            let pc = body as *const () as usize as i64;
            let v = unsafe { dartforge_arc_register_typed_v1(pc, 31) };
            let child = dartforge_arc_box_int_owned_v1(i64::MAX);
            let env = dartforge_arc_object_owned_v1(i64::from(crate::layout::cid::CONTEXT), 1);
            dartforge_arc_write_field_ref_v1(env, 0, child);
            let c = dartforge_arc_closure_owned_v1(0, env, pc, 31);
            dartforge_arc_release(env);
            let stats = HEAP.with(|h| h.borrow().stats());
            let target = dartforge_arc_typed_target_v1(c, 31);
            assert_eq!(target, pc);
            assert_eq!(HEAP.with(|h| h.borrow().stats().allocations), stats.allocations);
            assert_eq!(HEAP.with(|h| h.borrow().stats().collections), stats.collections);
            let test_body: extern "C" fn(i64, i64) -> i64 = unsafe { std::mem::transmute(target as usize) };
            let r = test_body(dartforge_arc_closure_context_v1(c), child);
            dartforge_arc_release(child); dartforge_arc_release(c); dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(r), 3);
            assert_eq!(dartforge_unbox_int(r), i64::MAX);
            dartforge_arc_release(r); dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(r), 0);
            unsafe { dartforge_arc_take_typed_v1(pc, 31, v); }
            HEAP.with(|h| h.replace(previous));
        }
    }

    thread_local! { static LAST: std::cell::Cell<i64> = const { std::cell::Cell::new(0) }; }

    #[test]
    fn owned_datagram_escapes_scope_and_pending_creates_no_owner() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        extern "C" fn datagram(data: i64, text: i64, ip: i64, port: i64, ty: i64) -> i64 {
            assert_eq!(typed_list_bytes(data).unwrap(), [1, 2, 3]);
            let r = dartforge_arc_object_owned_v1(500, 5);
            for (i, v) in [(0, data), (1, text), (2, ip)] { dartforge_arc_write_field_ref_v1(r, i, v); }
            dartforge_arc_write_field_scalar_v1(r, 3, port);
            dartforge_arc_write_field_scalar_v1(r, 4, ty);
            r
        }
        extern "C" fn error(_: i64, _: i64, _: i64, _: i64, _: i64) -> i64 {
            let e = dartforge_arc_box_int_owned_v1(i64::MAX);
            dartforge_arc_throw_ref_v1(e);
            dartforge_arc_release(e);
            0
        }
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            // Uses the runtime layer's descriptor: on Windows it identifies a
            // Handle, not the raw SOCKET that std::net returns.
            let e = OsAddress::from_ip(&[127, 0, 0, 1]).unwrap();
            let fd = create_datagram(&e, false, false, 1).unwrap();
            let dest = std::net::SocketAddr::from(([127, 0, 0, 1], local_port(fd) as u16));
            let socket = dartforge_arc_object_owned_v1(123, 1);
            set_socket_on_object(socket, fd, SocketFinalizer::Normal);
            let scopes = ARC_NATIVE_PORTS.with(|p| p.borrow().scopes.len());
            assert_eq!(dartforge_arc_socket_recvfrom_owned_v1(socket), 0);
            assert_eq!(dartforge_exception_pending(), 0);
            let p = native_field(socket);
            NativeSocket::retain_ref(p);
            event_handler().send(EventCommand { sock: p, port: 0,
                data: (1 << COMMAND_MASK) | (1 << EVENT_IN) });
            let emitter = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
            for (pc, pending) in [(datagram as *const () as usize, false), (error as *const () as usize, true)] {
                let version = register(pc, 5);
                HELPERS.with(|a| a.borrow_mut().insert("_dartforgeDatagram".into(), pc));
                emitter.send_to(&[1, 2, 3], dest).unwrap();
                let mut r = 0;
                let limit = crate::clock::Instant::now() + std::time::Duration::from_secs(2);
                while crate::clock::Instant::now() < limit {
                    r = dartforge_arc_socket_recvfrom_owned_v1(socket);
                    if r != 0 || dartforge_exception_pending() != 0 { break; }
                    std::thread::yield_now();
                }
                assert_eq!(ARC_NATIVE_PORTS.with(|p| p.borrow().scopes.len()), scopes);
                if pending {
                    assert_eq!(r, 0); assert_ne!(dartforge_exception_pending(), 0);
                    dartforge_exception_clear();
                } else {
                    assert_eq!(dartforge_exception_pending(), 0); assert_ne!(r, 0);
                    let data = dartforge_arc_read_field_ref_v1(r, 0);
                    dartforge_arc_collect();
                    assert_eq!(dartforge_arc_observe_heap_v1(r), 3);
                    assert_eq!(typed_list_bytes(data).unwrap(), [1, 2, 3]);
                    dartforge_arc_release(r); dartforge_arc_collect();
                    assert_eq!(dartforge_arc_observe_heap_v1(r), 0);
                    assert_eq!(dartforge_arc_observe_heap_v1(data), 0);
                }
                dartforge_arc_take_return_owned_v1(pc as i64, version);
            }
            HELPERS.with(|a| a.borrow_mut().remove("_dartforgeDatagram"));
            dartforge_arc_release(socket); dartforge_arc_collect();
            HEAP.with(|h| { h.replace(previous); });
        }
    }

    extern "C" fn new() -> i64 {
        let x = dartforge_arc_box_int_owned_v1(i64::MAX);
        LAST.with(|u| u.set(x));
        x
    }
    extern "C" fn one(x: i64) -> i64 {
        dartforge_arc_retain(x);
        x
    }
    extern "C" fn two(x: i64, _: i64) -> i64 {
        one(x)
    }
    extern "C" fn three(x: i64, _: i64, _: i64) -> i64 {
        one(x)
    }
    extern "C" fn four(x: i64, _: i64, _: i64, _: i64) -> i64 {
        one(x)
    }
    extern "C" fn five(x: i64, _: i64, _: i64, _: i64, _: i64) -> i64 {
        one(x)
    }
    extern "C" fn seven(x: i64, _: i64, _: i64, _: i64, _: i64, _: i64, _: i64) -> i64 {
        one(x)
    }
    extern "C" fn scalar() -> i64 {
        i64::MAX - 1
    }

    fn register(f: usize, n: i64) -> i64 {
        // SAFETY: only the word/Owned Ref fixtures above are used.
        unsafe { dartforge_arc_register_return_owned_v1(f as i64, n) }
    }

    #[test]
    fn all_ports_transfer_one_token_without_retaining_twice() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memory_arc_v1();
            let fs = [
                (new as *const () as usize, 0),
                (one as *const () as usize, 1),
                (two as *const () as usize, 2),
                (three as *const () as usize, 3),
                (four as *const () as usize, 4),
                (five as *const () as usize, 5),
                (seven as *const () as usize, 7),
            ];
            let entries: Vec<_> = fs.iter().map(|&(f, n)| (f, register(f, n))).collect();
            let e = dartforge_arc_native_scope_open_v1();
            let x = new();
            let y = dart_r0(fs[0].0);
            assert_eq!(dart_r1(fs[1].0, x), x);
            assert_eq!(dart_r2(fs[2].0, x, 0), x);
            assert_eq!(dart_r3(fs[3].0, x, 0, 0), x);
            assert_eq!(dart_r4(fs[4].0, x, 0, 0, 0), x);
            assert_eq!(dart_r5(fs[5].0, x, 0, 0, 0, 0), x);
            assert_eq!(dart_r7(fs[6].0, x, 0, 0, 0, 0, 0, 0), x);
            dartforge_arc_release(x);
            dartforge_arc_collect();
            for h in [x, y] {
                assert_eq!(dartforge_arc_observe_heap_v1(h), 3);
            }
            // The scope is an owner, but its result is a native borrow.
            dartforge_arc_retain(y);
            dartforge_arc_native_scope_close_v1(e);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(x), 0);
            assert_eq!(dartforge_arc_observe_heap_v1(y), 3);
            dartforge_arc_release(y);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(y), 0);
            for (f, v) in entries {
                dartforge_arc_take_return_owned_v1(f as i64, v);
            }
            // Without an explicit registration, a word gains neither an owner nor a scope.
            assert_eq!(dart_r0(scalar as *const () as usize), i64::MAX - 1);
        })
        .join()
        .unwrap();
    }

    extern "C" fn reentrant() -> i64 {
        let e = dartforge_arc_native_scope_open_v1();
        let x = dart_r0(new as *const () as usize);
        dartforge_arc_retain(x);
        dartforge_arc_native_scope_close_v1(e);
        dartforge_arc_collect();
        x
    }

    #[test]
    fn reentrant_scopes_and_observational_frames_stay_distinct() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memory_arc_v1();
            let f = reentrant as *const () as usize;
            let n = new as *const () as usize;
            let vf = register(f, 0);
            let vn = register(n, 0);
            let frame = HEAP.with(|h| h.borrow_mut().push_frame_with_slots(0));
            let e = dartforge_arc_native_scope_open_v1();
            let x = dart_r0(f);
            // The boundary's owners do not push frames onto the heap: the runtime
            // may close the observational frame it opened before the callback.
            HEAP.with(|h| h.borrow_mut().pop_frame(frame));
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(x), 3);
            dartforge_arc_native_scope_close_v1(e);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(x), 0);
            dartforge_arc_take_return_owned_v1(f as i64, vf);
            dartforge_arc_take_return_owned_v1(n as i64, vn);
        })
        .join()
        .unwrap();
    }

    #[test]
    fn records_and_scopes_reject_stale_version_wrong_abi_and_active_mutation() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            let f = new as *const () as usize;
            let v = register(f, 0);
            assert_eq!(register(f, 0), v);
            assert!(arc_prepare_port(f, 0).is_err());
            let e = dartforge_arc_native_scope_open_v1();
            assert!(arc_prepare_port(f, 1).is_err());
            let g = arc_prepare_port(f, 0).unwrap();
            {
                let mut p = arc_port_records().lock().unwrap();
                for n in [6, 8] {
                    assert!(p.register(f, n).is_err());
                }
                assert!(p.register(0, 0).is_err());
                assert!(p.deregister(f, v).is_err());
                assert!(p.deregister(f, v + 1).is_err());
                assert_eq!(p.entries[&(f, 0)].calls, 1);
            }
            ARC_NATIVE_PORTS.with(|p| {
                let mut p = p.borrow_mut();
                assert!(p.do_close(e).is_err());
                assert_eq!(p.scopes.last().unwrap().calls, 1);
            });
            drop(g);
            dartforge_arc_take_return_owned_v1(f as i64, v);
            let new_area = register(f, 0);
            assert_ne!(new_area, v);
            {
                let mut p = arc_port_records().lock().unwrap();
                assert!(p.deregister(f, v).is_err());
                assert_eq!(p.entries[&(f, 0)].version, new_area);
            }
            dartforge_arc_take_return_owned_v1(f as i64, new_area);
            let internal = dartforge_arc_native_scope_open_v1();
            ARC_NATIVE_PORTS.with(|p| assert!(p.borrow_mut().do_close(e).is_err()));
            dartforge_arc_native_scope_close_v1(internal);
            dartforge_arc_native_scope_close_v1(e);
        })
        .join()
        .unwrap();
    }

    extern "C" fn fail() -> i64 {
        dartforge_exception_throw(42, 1);
        0
    }

    #[test]
    fn pending_exit_neither_publishes_result_nor_clears_exception() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memory_arc_v1();
            let f = fail as *const () as usize;
            let v = register(f, 0);
            let e = dartforge_arc_native_scope_open_v1();
            assert_eq!(dart_r0(f), 0);
            assert_ne!(dartforge_exception_pending(), 0);
            ARC_NATIVE_PORTS
                .with(|p| assert!(p.borrow().scopes.last().unwrap().owners.is_empty()));
            dartforge_arc_native_scope_close_v1(e);
            assert_ne!(dartforge_exception_pending(), 0);
            dartforge_exception_clear();
            dartforge_arc_take_return_owned_v1(f as i64, v);
        })
        .join()
        .unwrap();
    }

    extern "C" fn nsm_owned(_: i64, _: i64, _: i64, _: i64, _: i64, _: i64, _: i64) -> i64 {
        new()
    }

    extern "C" fn uniform_reentrant(_: i64, _: *const i64, _: *const i64) -> i64 {
        let x = dart_r0(new as *const () as usize);
        dartforge_arc_retain(x);
        x
    }

    extern "C" fn uniform_pending(_: i64, _: *const i64, _: *const i64) -> i64 {
        let _x = dart_r0(new as *const () as usize);
        fail()
    }

    fn register_uniform(f: usize) -> i64 {
        // SAFETY: the fixtures neither read nor publish vectors; they return Owned or pending.
        unsafe { dartforge_arc_register_uniform_owned_v1(f as i64) }
    }

    #[test]
    fn uniform_closes_callbacks_without_consuming_return_token() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memory_arc_v1();
            let f = uniform_reentrant as *const () as usize;
            let n = new as *const () as usize;
            let v = register_uniform(f);
            let vn = register(n, 0);
            let x = arc_invoke_uniform(f, 0, 0, 0).unwrap();
            ARC_NATIVE_PORTS.with(|p| assert!(p.borrow().scopes.is_empty()));
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(x), 3);
            dartforge_arc_release(x);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(x), 0);
            dartforge_arc_take_return_owned_v1(f as i64, v);
            dartforge_arc_take_return_owned_v1(n as i64, vn);
        }).join().unwrap();
    }

    #[test]
    fn uniform_pending_closes_internal_owners_and_preserves_exception() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memory_arc_v1();
            let f = uniform_pending as *const () as usize;
            let n = new as *const () as usize;
            let v = register_uniform(f);
            let vn = register(n, 0);
            assert_eq!(arc_invoke_uniform(f, 0, 0, 0), Ok(0));
            assert_ne!(dartforge_exception_pending(), 0);
            ARC_NATIVE_PORTS.with(|p| assert!(p.borrow().scopes.is_empty()));
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(LAST.with(|u| u.get())), 0);
            assert_ne!(dartforge_exception_pending(), 0);
            dartforge_exception_clear();
            dartforge_arc_take_return_owned_v1(f as i64, v);
            dartforge_arc_take_return_owned_v1(n as i64, vn);
        }).join().unwrap();
    }

    #[test]
    fn uniform_does_not_infer_generic_callback_contract_and_rejects_upgrade() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            let f = three as *const () as usize;
            assert!(arc_invoke_uniform(f, 0, 0, 0).is_err());
            let v = register(f, 3);
            assert!(arc_invoke_uniform(f, 0, 0, 0).is_err());
            {
                let mut p = arc_port_records().lock().unwrap();
                assert!(p.register_contract(f, 0, true).is_err());
                assert!(p.register_contract(f, 3, true).is_err());
                assert_eq!(p.entries[&(f, 3)].version, v);
                assert_eq!(p.entries[&(f, 3)].calls, 0);
            }
            ARC_NATIVE_PORTS.with(|p| assert!(p.borrow().scopes.is_empty()));
            dartforge_arc_take_return_owned_v1(f as i64, v);
            let f = uniform_reentrant as *const () as usize;
            let v = register_uniform(f);
            assert_eq!(register_uniform(f), v);
            assert!(arc_port_records().lock().unwrap().register(f, 3).is_err());
            dartforge_arc_take_return_owned_v1(f as i64, v);
            assert!(arc_invoke_uniform(f, 0, 0, 0).is_err());
        }).join().unwrap();
    }

    #[test]
    fn uniform_adapts_native_fallback_to_single_owned() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memory_arc_v1();
            let f = nsm_owned as *const () as usize;
            let v = register(f, 7);
            let name = b"_dartforgeNoSuchMethod";
            // SAFETY: a static seven-word helper with an Owned return.
            unsafe { dartforge_register_helper(name.as_ptr(), name.len() as i64, f); }
            let recv = new();
            let args = [0_i64]; let desc = [0_i64, 0];
            let x = arc_invoke_uniform(dartforge_nsm_selector as *const () as usize,
                recv, args.as_ptr() as i64, desc.as_ptr() as i64).unwrap();
            ARC_NATIVE_PORTS.with(|p| assert!(p.borrow().scopes.is_empty()));
            assert_eq!(args, [0]); assert_eq!(desc, [0, 0]);
            dartforge_arc_release(recv);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(x), 3);
            dartforge_arc_release(x);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(x), 0);
            dartforge_arc_take_return_owned_v1(f as i64, v);
        }).join().unwrap();
    }

    #[test]
    fn owned_no_such_method_survives_legacy_frame_pop_without_leaking() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memory_arc_v1();
            let f = nsm_owned as *const () as usize;
            let v = register(f, 7);
            let name = b"_dartforgeNoSuchMethod";
            // SAFETY: a static name and a word fixture with the helper's signature.
            unsafe {
                dartforge_register_helper(name.as_ptr(), name.len() as i64, f);
            }
            let e = dartforge_arc_native_scope_open_v1();
            let recv = dartforge_arc_box_int_owned_v1(i64::MAX - 1);
            let name = HEAP.with(|h| h.borrow_mut().allocate_str("missing"));
            let x = invoke_no_such_method(recv, 0, name, &[], 0, &[], 0).unwrap();
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(x), 3);
            assert_eq!(dartforge_unbox_int(x), i64::MAX);
            dartforge_arc_native_scope_close_v1(e);
            dartforge_arc_release(recv);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(x), 0);
            dartforge_arc_take_return_owned_v1(f as i64, v);
        })
        .join()
        .unwrap();
    }
    #[test]
    fn image_registration_is_global_but_owners_and_scopes_are_per_thread() {
        let _serial = SERIAL.lock().unwrap();
        let f = new as *const () as usize;
        let v = register(f, 0);
        let parent_node = dartforge_arc_native_scope_open_v1();
        std::thread::spawn(move || {
            dartforge_memory_arc_v1();
            let e = dartforge_arc_native_scope_open_v1();
            assert_ne!(e, parent_node);
            ARC_NATIVE_PORTS.with(|p| assert!(p.borrow_mut().do_close(parent_node).is_err()));
            let x = dart_r0(f);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(x), 3);
            let g = arc_prepare_port(f, 0).unwrap();
            // The version also stays pinned while a call from another thread is in
            // progress; the registration does not depend on the publisher's heap.
            assert!(
                arc_port_records()
                    .lock()
                    .unwrap()
                    .deregister(f, v)
                    .is_err()
            );
            drop(g);
            dartforge_arc_native_scope_close_v1(e);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(x), 0);
        })
        .join()
        .unwrap();
        dartforge_arc_native_scope_close_v1(parent_node);
        dartforge_arc_take_return_owned_v1(f as i64, v);
    }
    #[test]
    fn owned_return_ignored_by_void_port_is_released_immediately() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memory_arc_v1();
            let fs = [
                (new as *const () as usize, 0),
                (one as *const () as usize, 1),
                (two as *const () as usize, 2),
                (three as *const () as usize, 3),
            ];
            let entries: Vec<_> = fs.iter().map(|&(f, n)| (f, register(f, n))).collect();
            let e = dartforge_arc_native_scope_open_v1();
            dart_v0(fs[0].0);
            let y = LAST.with(|u| u.get());
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(y), 0);
            let x = new();
            dart_v1(fs[1].0, x);
            dart_v2(fs[2].0, x, 0);
            dart_v3(fs[3].0, x, 0, 0);
            ARC_NATIVE_PORTS
                .with(|p| assert!(p.borrow().scopes.last().unwrap().owners.is_empty()));
            dartforge_arc_release(x);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observe_heap_v1(x), 0);
            dartforge_arc_native_scope_close_v1(e);
            for (f, v) in entries {
                dartforge_arc_take_return_owned_v1(f as i64, v);
            }
        })
        .join()
        .unwrap();
    }
}
