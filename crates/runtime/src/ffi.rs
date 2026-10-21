// Native runtime: `dart:ffi` (the VM's `runtime/lib/ffi.cc` and
// `ffi_dynamic_library.cc`, and what its compiler generates for the recognized
// methods).
//
// * `DynamicLibrary`: `dlopen`/`dlsym` (Unix) and `LoadLibraryW`/`GetProcAddress`
//   (Windows); the process library searches everything that is loaded.
// * The native types and the signature trampolines come from the compiler
//   (`lower/ffi.rs`), registered during each isolate's preparation; the class ids
//   and the addresses belong to the program, so the tables are global.
// * `asFunction` creates a closure with the native signature's trampoline (the key
//   comes from the signature's RTI) and the address in the environment.
// * Loads and stores (`_loadInt32`…) serve a `Pointer` base (native memory: no
//   bounds checking, as in C) and a heap `TypedData` base (that of a `Struct`
//   created in Dart: with checking). Native access is always unaligned-safe
//   (`read_unaligned`).

/// The `handle` of the process library (`DynamicLibrary.process()`): no system
/// handle uses it.
const PROCESS_HANDLE: i64 = i64::MIN + 1;

/// RTI class → C type letter (the compiler's `TipoC::letter`).
fn native_types() -> &'static std::sync::RwLock<crate::hash::HashMap<i64, char>> {
    static T: std::sync::OnceLock<std::sync::RwLock<crate::hash::HashMap<i64, char>>> = std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

/// Signature key → trampoline entry.
fn trampolines() -> &'static std::sync::RwLock<crate::hash::HashMap<String, usize>> {
    static T: std::sync::OnceLock<std::sync::RwLock<crate::hash::HashMap<String, usize>>> = std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_register_type(class: i64, letter: i64) {
    let Some(c) = u32::try_from(letter).ok().and_then(char::from_u32) else { return };
    let mut t = native_types().write().unwrap_or_else(|e| e.into_inner());
    t.entry(class).or_insert(c);
}

/// # Safety
/// `key` points to `n` ASCII bytes; `input` is the trampoline's uniform entry.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_register_trampoline(key: *const u8, n: i64, input: usize) {
    // SAFETY: guaranteed by the caller (a module constant).
    let b = unsafe { std::slice::from_raw_parts(key, n as usize) };
    let key = String::from_utf8_lossy(b).into_owned();
    let mut t = trampolines().write().unwrap_or_else(|e| e.into_inner());
    t.entry(key).or_insert(input);
}

/// The size of a C type by its letter.
fn letter_size(l: char) -> Option<i64> {
    Some(match l {
        'a' | 'h' | 'b' => 1,
        's' | 't' => 2,
        'i' | 'j' | 'f' => 4,
        'l' | 'm' | 'd' | 'p' => 8,
        _ => return None,
    })
}

/// The RTI id of the type of a `Type` object.
fn object_type_type(h: i64) -> Option<i64> {
    HEAP.with(|heap| match heap.borrow().object(h) {
        Some(o) if o.class_id == TYPE_CLASS => o.first().map(|f| f.0),
        _ => None,
    })
}

/// The C type letter of an RTI type (a native type class).
fn type_letter(t: i64) -> Result<char, String> {
    let class = RTI.with(|u| match u.borrow().ty(t) {
        Type::Interface(c, _) => Some(*c),
        _ => None,
    });
    let letter = class.and_then(|c| native_types().read().unwrap_or_else(|e| e.into_inner()).get(&c).copied());
    letter.ok_or_else(|| {
        let text = RTI.with(|u| u.borrow().text(t));
        format!("`{text}` is not a native type supported by the DartForge native backend")
    })
}

/// The part of a type in the key: the C type letter, or `S<rti>.` of a struct/union
/// by value (registered by the program).
fn key_part(t: i64, s: &mut String) -> Result<(), String> {
    if let Some(rti) = type_rti_class(t).filter(|&r| compound_from_rti(r).is_some()) {
        s.push_str(&format!("S{rti}."));
        return Ok(());
    }
    s.push(type_letter(t)?);
    Ok(())
}

/// The key (`lower/ffi.rs::chave_da_assinatura`) of the native signature `ns`.
fn signature_key(ns: i64) -> Result<String, String> {
    let (ret, pos) = RTI.with(|u| match u.borrow().ty(ns) {
        Type::Function { generics: 0, ret, pos, n_required, named } if named.is_empty() && *n_required == pos.len() => Ok((*ret, pos.clone())),
        _ => Err(format!("`{}` is not a native function signature", u.borrow().text(ns))),
    })?;
    let mut s = String::with_capacity(pos.len() + 2);
    key_part(ret, &mut s)?;
    s.push('_');
    for p in pos {
        // `VarArgs<(T1, T2…)>`/`VarArgs<T>`: the marker and the variadic types.
        let variadic = RTI.with(|u| {
            let u = u.borrow();
            let Type::Interface(c, args) = u.ty(p) else { return None };
            if native_types().read().unwrap_or_else(|e| e.into_inner()).get(c) != Some(&'*') {
                return None;
            }
            let a = *args.first()?;
            Some(match u.ty(a) {
                Type::Record { pos, named } if named.is_empty() => pos.clone(),
                _ => vec![a],
            })
        });
        match variadic {
            Some(vs) => {
                s.push('*');
                for v in vs {
                    key_part(v, &mut s)?;
                }
            }
            None => key_part(p, &mut s)?,
        }
    }
    Ok(s)
}

fn throw_unsupported(msg: &str) {
    let m = allocate_str(msg);
    let e = with_roots(&[m], || dartforge_unsupported_error_new(m));
    with_roots(&[e], || dartforge_exception_throw(e, 3));
}

/// `sizeOf<T>()`: the size in bytes of native type `T` in the target ABI.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_size_of(ty: i64) -> i64 {
    let Some(t) = object_type_type(ty) else {
        throw_argument_error("sizeOf: type argument expected");
        return 0;
    };
    if let Some(c) = type_rti_class(t).and_then(compound_from_rti) {
        return c.size;
    }
    match type_letter(t).map(letter_size) {
        Ok(Some(n)) => n,
        Ok(None) => {
            throw_unsupported("sizeOf<Void>() is not defined");
            0
        }
        Err(m) => {
            throw_unsupported(&m);
            0
        }
    }
}

/// `asFunction`: the closure that calls the native function at `address` through
/// signature `ns`, with the Dart type `df`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_function(address: i64, ns: i64, df: i64) -> i64 {
    let (Some(ns), Some(df)) = (object_type_type(ns), object_type_type(df)) else {
        throw_argument_error("asFunction: type arguments expected");
        return 0;
    };
    let key = match signature_key(ns) {
        Ok(c) => c,
        Err(m) => {
            throw_unsupported(&m);
            return 0;
        }
    };
    let Some(input) = trampolines().read().unwrap_or_else(|e| e.into_inner()).get(&key).copied() else {
        let text = RTI.with(|u| u.borrow().text(ns));
        throw_unsupported(&format!("the native signature `{text}` was not compiled into this program (it must be written as a constant type argument)"));
        return 0;
    };
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        // The context keeps both scalars (without references); the closure keeps
        // it as a `Ref`. The temporary owner is counted during the allocation and
        // released after the closure starts holding the strong edge.
        let env = h.new_context(&[(address, false), (ns, false)]);
        let frame = h.push_owning_frame(1);
        h.set_root(frame, 0, env);
        let clo = h.new_closure(input as i64, (env, true), 0, 0);
        h.set_metadata(clo, df + 1);
        h.pop_frame(frame);
        clo
    })
}

/// The environment `[address, fn_signature]` of a trampoline's closure.
fn closure_env(clo: i64) -> (i64, i64) {
    HEAP.with(|h| {
        let h = h.borrow();
        let Some(c) = h.closure(clo) else {
            panic!("compiler bug: FFI trampoline without a closure");
        };
        let env = c.context.0;
        (h.capture(env, 0).0, h.capture(env, 1).0)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_closure_address(clo: i64) -> i64 {
    closure_env(clo).0
}

/// The address of a `Pointer` (the `_address` field).
fn address_of(pointer: i64) -> Option<i64> {
    HEAP.with(|h| {
        let h = h.borrow();
        match h.object(pointer) {
            Some(fields) => {
                let (bits, is_ref) = fields.first()?;
                if is_ref { h.int_of(bits) } else { Some(bits) }
            }
            _ => None,
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_pointer_address(pointer: i64) -> i64 {
    match address_of(pointer) {
        Some(e) => e,
        None => {
            throw_argument_error("a Pointer was expected in a native call");
            0
        }
    }
}

/// The `Pointer<X>` that a native call returns (`X` from the signature).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_return_pointer(address: i64, clo: i64) -> i64 {
    let (_, ns) = closure_env(clo);
    new_pointer(address, RTI.with(|u| match u.borrow().ty(ns) {
        Type::Function { ret, .. } => Some(*ret),
        _ => None,
    }))
}

/// The `Pointer<X>` that a native call returns, under ARC, with a code owner.
///
/// `clo` is borrowed by the caller. The Dart helper `_dartforgePointer` runs
/// inside a native scope: the token of the Owned port's return stays in it and is
/// copied into the caller's owner before closing. An exception from the helper
/// stays pending and the result is null, without an owner. May allocate, collect
/// and run Dart.
///
/// # Panics
/// Missing helper, an incompatible Owned registration or an unclosed reentrant
/// scope; aborts at the C boundary.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_ffi_return_pointer_owned_v1(address: i64, clo: i64) -> i64 {
    owner_in_native_scope(|| dartforge_ffi_return_pointer(address, clo))
}

/// A Dart `Pointer` with the address and, if known, the type `Pointer<X>`.
fn new_pointer(address: i64, ty: Option<i64>) -> i64 {
    let Some(f) = helper("_dartforgePointer") else {
        panic!("compiler bug: dart:ffi without a registered `_dartforgePointer`");
    };
    // SAFETY: registered by the `dart:ffi` overlay with the signature (`int`) →
    // `Pointer`.
    let f = { let dart_target: usize = f; move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
    let p = f(address);
    if let Some(t) = ty {
        HEAP.with(|h| h.borrow_mut().set_metadata(p, t + 1));
    }
    p
}

// ---------------------------------------------------------------------------
// Memory: a `Pointer` or `TypedData` base.

/// Calls `f` with the address of the `n` bytes at `base + offset`. On a `TypedData`
/// base, out of bounds throws `RangeError`.
fn with_memory<R>(base: i64, offset: i64, n: usize, f: impl FnOnce(*mut u8) -> R) -> Option<R> {
    let list = HEAP.with(|h| resolver(&h.borrow(), base));
    if let Some(t) = list {
        let total = t.bytes;
        let begin = usize::try_from(offset).ok().filter(|&d| d.checked_add(n).is_some_and(|end| end <= total));
        let Some(begin) = begin else {
            throw_range_error(offset, total as i64);
            return None;
        };
        // The bytes do not move (§2.14): the address is valid while the base lives,
        // and `f` does not allocate on the heap.
        return Some(f(t.data.wrapping_add(begin)));
    }
    let Some(address) = address_of(base) else {
        throw_argument_error("a Pointer or TypedData was expected");
        return None;
    };
    Some(f(address.wrapping_add(offset) as usize as *mut u8))
}

fn throw_range_error(ix: i64, total: i64) {
    let m = allocate_str(&format!("Invalid value: Not in inclusive range 0..{}: {ix}", total - 1));
    let e = with_roots(&[m], || dartforge_range_error_new(m));
    with_roots(&[e], || dartforge_exception_throw(e, 3));
}

// The loads and stores are explicit functions (not generated by a macro):
// `build.rs` reads the exported symbols from the fragments' text.
fn load_native<T: Copy + Into<i128>>(base: i64, offset: i64) -> i64 {
    // SAFETY: native memory is what the program indicated (as in C); heap memory
    // was checked against the bounds.
    with_memory(base, offset, std::mem::size_of::<T>(), |p| unsafe { std::ptr::read_unaligned(p.cast::<T>()) }.into() as i64).unwrap_or(0)
}

fn write_native<T: Copy>(base: i64, offset: i64, value: T) {
    // SAFETY: as in `load_native`.
    with_memory(base, offset, std::mem::size_of::<T>(), |p| unsafe { std::ptr::write_unaligned(p.cast::<T>(), value) });
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_load_i8(base: i64, offset: i64) -> i64 {
    load_native::<i8>(base, offset)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_store_i8(base: i64, offset: i64, value: i64) {
    write_native::<i8>(base, offset, value as i8);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_load_u8(base: i64, offset: i64) -> i64 {
    load_native::<u8>(base, offset)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_store_u8(base: i64, offset: i64, value: i64) {
    write_native::<u8>(base, offset, value as u8);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_load_i16(base: i64, offset: i64) -> i64 {
    load_native::<i16>(base, offset)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_store_i16(base: i64, offset: i64, value: i64) {
    write_native::<i16>(base, offset, value as i16);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_load_u16(base: i64, offset: i64) -> i64 {
    load_native::<u16>(base, offset)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_store_u16(base: i64, offset: i64, value: i64) {
    write_native::<u16>(base, offset, value as u16);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_load_i32(base: i64, offset: i64) -> i64 {
    load_native::<i32>(base, offset)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_store_i32(base: i64, offset: i64, value: i64) {
    write_native::<i32>(base, offset, value as i32);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_load_u32(base: i64, offset: i64) -> i64 {
    load_native::<u32>(base, offset)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_store_u32(base: i64, offset: i64, value: i64) {
    write_native::<u32>(base, offset, value as u32);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_load_i64(base: i64, offset: i64) -> i64 {
    load_native::<i64>(base, offset)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_store_i64(base: i64, offset: i64, value: i64) {
    write_native::<i64>(base, offset, value as i64);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_load_u64(base: i64, offset: i64) -> i64 {
    load_native::<u64>(base, offset)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_store_u64(base: i64, offset: i64, value: i64) {
    write_native::<u64>(base, offset, value as u64);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_load_f32(base: i64, offset: i64) -> f64 {
    // SAFETY: as in `carga_e_gravacao!`.
    with_memory(base, offset, 4, |p| f64::from(unsafe { std::ptr::read_unaligned(p.cast::<f32>()) })).unwrap_or(0.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_store_f32(base: i64, offset: i64, value: f64) {
    // SAFETY: as in `carga_e_gravacao!`.
    with_memory(base, offset, 4, |p| unsafe { std::ptr::write_unaligned(p.cast::<f32>(), value as f32) });
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_load_f64(base: i64, offset: i64) -> f64 {
    // SAFETY: as in `carga_e_gravacao!`.
    with_memory(base, offset, 8, |p| unsafe { std::ptr::read_unaligned(p.cast::<f64>()) }).unwrap_or(0.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_store_f64(base: i64, offset: i64, value: f64) {
    // SAFETY: as in `carga_e_gravacao!`.
    with_memory(base, offset, 8, |p| unsafe { std::ptr::write_unaligned(p.cast::<f64>(), value) });
}

/// The load of an ABI-specific integer (`Long`, `Size`…): the C type that the
/// compiler resolved for the target.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_load_abi(ty: i64, base: i64, offset: i64) -> i64 {
    match object_type_type(ty).map(type_letter) {
        Some(Ok('a')) => dartforge_native_DartForge_ffi_load_i8(base, offset),
        Some(Ok('h')) => dartforge_native_DartForge_ffi_load_u8(base, offset),
        Some(Ok('s')) => dartforge_native_DartForge_ffi_load_i16(base, offset),
        Some(Ok('t')) => dartforge_native_DartForge_ffi_load_u16(base, offset),
        Some(Ok('i')) => dartforge_native_DartForge_ffi_load_i32(base, offset),
        Some(Ok('j')) => dartforge_native_DartForge_ffi_load_u32(base, offset),
        Some(Ok('l')) => dartforge_native_DartForge_ffi_load_i64(base, offset),
        Some(Ok('m')) => dartforge_native_DartForge_ffi_load_u64(base, offset),
        Some(Err(m)) => {
            throw_unsupported(&m);
            0
        }
        _ => {
            throw_unsupported("AbiSpecificInteger without an integer mapping for this ABI");
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_store_abi(ty: i64, base: i64, offset: i64, value: i64) {
    match object_type_type(ty).map(type_letter) {
        Some(Ok('a' | 'h')) => dartforge_native_DartForge_ffi_store_u8(base, offset, value),
        Some(Ok('s' | 't')) => dartforge_native_DartForge_ffi_store_u16(base, offset, value),
        Some(Ok('i' | 'j')) => dartforge_native_DartForge_ffi_store_u32(base, offset, value),
        Some(Ok('l' | 'm')) => dartforge_native_DartForge_ffi_store_u64(base, offset, value),
        Some(Err(m)) => throw_unsupported(&m),
        _ => throw_unsupported("AbiSpecificInteger without an integer mapping for this ABI"),
    }
}

/// `_memCopy(target, targetOffset, source, sourceOffset, length)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_copy(dest: i64, dest_offset: i64, origin: i64, source_offset: i64, n: i64) {
    let Ok(n) = usize::try_from(n) else { return };
    let mut buf = vec![0u8; n];
    // SAFETY: as in `carga_e_gravacao!`; the intermediate buffer separates the two
    // bases (which may be the same heap list).
    if with_memory(origin, source_offset, n, |p| unsafe { std::ptr::copy_nonoverlapping(p, buf.as_mut_ptr(), n) }).is_none() {
        return;
    }
    with_memory(dest, dest_offset, n, |p| unsafe { std::ptr::copy_nonoverlapping(buf.as_ptr(), p, n) });
}

/// `_abi()`: the target's index in `Abi.values`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_abi() -> i64 {
    // The order of `Abi.values` (`abi.dart`): androidArm … windowsX64.
    let arm = cfg!(target_arch = "aarch64");
    if cfg!(windows) {
        if arm { 19 } else { 21 }
    } else if cfg!(target_os = "macos") {
        if arm { 17 } else { 18 }
    } else if arm {
        12
    } else {
        14
    }
}

// ---------------------------------------------------------------------------
// `DynamicLibrary`.

fn throw_ffi_argument_error(msg: String) -> i64 {
    throw_argument_error(&msg);
    0
}

#[cfg(unix)]
mod dl {
    unsafe extern "C" {
        pub fn dlopen(file_path: *const std::ffi::c_char, mode: i32) -> *mut std::ffi::c_void;
        pub fn dlsym(h: *mut std::ffi::c_void, name: *const std::ffi::c_char) -> *mut std::ffi::c_void;
        pub fn dlclose(h: *mut std::ffi::c_void) -> i32;
        pub fn dlerror() -> *const std::ffi::c_char;
    }
    pub const RTLD_LAZY: i32 = 1;
    /// `RTLD_DEFAULT`: `NULL` on Linux, `-2` on macOS.
    #[cfg(target_os = "linux")]
    pub const RTLD_DEFAULT: *mut std::ffi::c_void = std::ptr::null_mut();
    #[cfg(not(target_os = "linux"))]
    pub const RTLD_DEFAULT: *mut std::ffi::c_void = -2isize as *mut std::ffi::c_void;

    /// The last `dl*` message.
    pub fn error() -> String {
        // SAFETY: `dlerror` returns null or a string valid until the next call.
        let e = unsafe { dlerror() };
        if e.is_null() {
            return String::new();
        }
        // SAFETY: NUL-terminated text.
        unsafe { std::ffi::CStr::from_ptr(e) }.to_string_lossy().into_owned()
    }
}

#[cfg(windows)]
mod dl {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn LoadLibraryW(name: *const u16) -> usize;
        pub fn GetModuleHandleW(name: *const u16) -> usize;
        pub fn GetProcAddress(h: usize, name: *const std::ffi::c_char) -> usize;
        pub fn FreeLibrary(h: usize) -> i32;
        pub fn GetLastError() -> u32;
        pub fn GetCurrentProcess() -> usize;
        pub fn K32EnumProcessModules(p: usize, m: *mut usize, n: u32, needs: *mut u32) -> i32;
    }

    /// The modules loaded in the process (`DynamicLibrary.process()`).
    pub fn modules() -> Vec<usize> {
        let mut n = 256u32;
        loop {
            let mut v = vec![0usize; n as usize];
            let mut needs = 0u32;
            // SAFETY: the buffer has `n` handles.
            let ok = unsafe { K32EnumProcessModules(GetCurrentProcess(), v.as_mut_ptr(), n * std::mem::size_of::<usize>() as u32, &mut needs) } != 0;
            if !ok {
                return Vec::new();
            }
            let used_count = needs as usize / std::mem::size_of::<usize>();
            if used_count <= v.len() {
                v.truncate(used_count);
                return v;
            }
            n = used_count as u32;
        }
    }
}

/// `Ffi_dl_open(path)`: the handle, or `ArgumentError` with the system error.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Ffi_dl_open(file_path: i64) -> i64 {
    let text = HEAP.with(|h| h.borrow().text(file_path).map(|t| t.to_dart_string()).unwrap_or_default());
    #[cfg(unix)]
    {
        let Ok(c) = std::ffi::CString::new(text.clone()) else {
            return throw_ffi_argument_error(format!("Failed to load dynamic library '{text}': invalid path"));
        };
        // SAFETY: opens the library with a valid C path.
        let h = unsafe { dl::dlopen(c.as_ptr(), dl::RTLD_LAZY) };
        if h.is_null() {
            return throw_ffi_argument_error(format!("Failed to load dynamic library '{text}': {}", dl::error()));
        }
        h as i64
    }
    #[cfg(windows)]
    {
        let w: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: NUL-terminated UTF-16 name.
        let h = unsafe { dl::LoadLibraryW(w.as_ptr()) };
        if h == 0 {
            let code = unsafe { dl::GetLastError() };
            return throw_ffi_argument_error(format!("Failed to load dynamic library '{text}': error code {code}"));
        }
        h as i64
    }
    // A module links its native code statically (`@Native`); it has no loader.
    #[cfg(target_arch = "wasm32")]
    {
        throw_ffi_argument_error(format!(
            "Failed to load dynamic library '{text}': a WebAssembly module has no dynamic libraries"
        ))
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Ffi_dl_processLibrary() -> i64 {
    PROCESS_HANDLE
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Ffi_dl_executableLibrary() -> i64 {
    #[cfg(unix)]
    {
        // SAFETY: `dlopen(NULL)` is the executable.
        unsafe { dl::dlopen(std::ptr::null(), dl::RTLD_LAZY) as i64 }
    }
    #[cfg(windows)]
    {
        // SAFETY: the executable's module.
        unsafe { dl::GetModuleHandleW(std::ptr::null()) as i64 }
    }
    // The module itself, like the process: no symbol table to search at run time.
    #[cfg(target_arch = "wasm32")]
    {
        PROCESS_HANDLE
    }
}

/// The address of `name` in `handle` (0 if there is none).
fn search_for(handle: i64, name: &std::ffi::CStr) -> usize {
    #[cfg(unix)]
    {
        let h = if handle == PROCESS_HANDLE { dl::RTLD_DEFAULT } else { handle as *mut std::ffi::c_void };
        // SAFETY: a handle from `dlopen` (or `RTLD_DEFAULT`) and a valid C name.
        unsafe { dl::dlsym(h, name.as_ptr()) as usize }
    }
    #[cfg(windows)]
    {
        if handle == PROCESS_HANDLE {
            return dl::modules()
                .into_iter()
                // SAFETY: a loaded module and a valid C name.
                .map(|m| unsafe { dl::GetProcAddress(m, name.as_ptr()) })
                .find(|&p| p != 0)
                .unwrap_or(0);
        }
        // SAFETY: a handle from `LoadLibraryW` and a valid C name.
        unsafe { dl::GetProcAddress(handle as usize, name.as_ptr()) }
    }
    // Native symbols of a module are resolved at link time, never by name.
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (handle, name);
        0
    }
}

/// `Ffi_dl_lookup(handle, symbolName)`: the address, or `ArgumentError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Ffi_dl_lookup(handle: i64, name: i64) -> i64 {
    let text = HEAP.with(|h| h.borrow().text(name).map(|t| t.to_dart_string()).unwrap_or_default());
    let Ok(c) = std::ffi::CString::new(text.clone()) else {
        return throw_ffi_argument_error(format!("Failed to lookup symbol '{text}'"));
    };
    let p = search_for(handle, &c);
    if p == 0 {
        #[cfg(unix)]
        let detail = dl::error();
        #[cfg(windows)]
        let detail = format!("error code {}", unsafe { dl::GetLastError() });
        #[cfg(target_arch = "wasm32")]
        let detail = "a WebAssembly module resolves native symbols at link time (@Native)";
        return throw_ffi_argument_error(format!("Failed to lookup symbol '{text}': {detail}"));
    }
    p as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Ffi_dl_providesSymbol(handle: i64, name: i64) -> u8 {
    let text = HEAP.with(|h| h.borrow().text(name).map(|t| t.to_dart_string()).unwrap_or_default());
    std::ffi::CString::new(text).map_or(0, |c| u8::from(search_for(handle, &c) != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Ffi_dl_close(handle: i64) {
    if handle == PROCESS_HANDLE {
        throw_argument_error("Cannot close the process library.");
        return;
    }
    #[cfg(unix)]
    // SAFETY: a handle from `dlopen`.
    unsafe {
        dl::dlclose(handle as *mut std::ffi::c_void);
    }
    #[cfg(windows)]
    // SAFETY: a handle from `LoadLibraryW`.
    unsafe {
        dl::FreeLibrary(handle as usize);
    }
}

// ---------------------------------------------------------------------------
// Structs and unions (`lower/ffi.rs`, «Structs e unions»).

/// A program struct/union: the heap class, where the `_Compound` fields live in the
/// object, and its measure in the target ABI.
#[derive(Clone, Copy)]
struct FfiCompound {
    class: i64,
    fields: i64,
    base_index: i64,
    offset_index: i64,
    size: i64,
}

/// RTI class → compound (the class ids belong to the program, the same in every
/// isolate).
fn ffi_compounds() -> &'static std::sync::RwLock<crate::hash::HashMap<i64, FfiCompound>> {
    static C: std::sync::OnceLock<std::sync::RwLock<crate::hash::HashMap<i64, FfiCompound>>> = std::sync::OnceLock::new();
    C.get_or_init(Default::default)
}

/// Registers a program struct/union (the isolate's preparation).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_register_compound(
    rti: i64,
    class: i64,
    fields: i64,
    base_index: i64,
    offset_index: i64,
    size: i64,
    _alignment: i64,
) {
    ffi_compounds()
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .insert(rti, FfiCompound { class, fields, base_index, offset_index, size });
}

fn compound_from_rti(rti: i64) -> Option<FfiCompound> {
    ffi_compounds().read().unwrap_or_else(|e| e.into_inner()).get(&rti).copied()
}

/// The RTI class of an `Interface` RTI type.
fn type_rti_class(t: i64) -> Option<i64> {
    RTI.with(|u| match u.borrow().ty(t) {
        Type::Interface(c, _) => Some(*c),
        _ => None,
    })
}

/// An instance of struct/union `rti` over the memory at `base` + `offset` (the VM
/// transformer's `S#fromTypedDataBase`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_compound(rti: i64, base: i64, offset: i64) -> i64 {
    let Some(c) = compound_from_rti(rti) else {
        throw_unsupported("struct or union not registered in the DartForge native backend");
        return 0;
    };
    let obj = with_roots(&[base], || dartforge_object_new(c.class, c.fields));
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        let n = h.object(obj).map_or(0, |o| o.len());
        if let Some(i) = usize::try_from(c.base_index).ok().filter(|&i| i < n) {
            h.set_field(obj, i, base, true);
        }
        if let Some(i) = usize::try_from(c.offset_index).ok().filter(|&i| i < n) {
            h.set_field(obj, i, offset, false);
        }
    });
    obj
}

/// `Pointer<S>.ref`/`[i]` (the overlay): the instance of `S` over `base` + `offset`,
/// through the `Type` object of `S`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_ffi_compound_of_type(ty: i64, base: i64, offset: i64) -> i64 {
    let Some(rti) = object_type_type(ty).and_then(type_rti_class) else {
        throw_argument_error("struct type argument expected");
        return 0;
    };
    dartforge_ffi_compound(rti, base, offset)
}

/// A `Pointer` (type `Pointer<Never>`) with the address (a struct's `Pointer` field).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_pointer_new(address: i64) -> i64 {
    new_pointer(address, None)
}

/// The address of the native symbol `name` (UTF-8), for an `external` with
/// `@Native` (cached: the VM's `Ffi_GetFfiNativeResolver` resolves once per
/// function). Missing symbol: `ArgumentError`, like the VM. The reserved DartForge
/// ABI comes from this runtime, including in the RTLD_LOCAL SDK; foreign symbols
/// are still looked up in the process.
///
/// # Safety
/// `name` points to `n` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_native_symbol(name: *const u8, n: i64) -> i64 {
    // SAFETY: guaranteed by the caller (a module constant).
    let name = unsafe { std::slice::from_raw_parts(name, usize::try_from(n).unwrap_or(0)) };
    let name = String::from_utf8_lossy(name).into_owned();
    static CACHE: std::sync::OnceLock<std::sync::Mutex<crate::hash::HashMap<String, i64>>> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(&e) = cache.lock().unwrap_or_else(|e| e.into_inner()).get(&name) {
        return e;
    }
    let e = resolve_native_address(&name, |c| search_for(PROCESS_HANDLE, c)) as i64;
    match e {
        e if e != 0 => {
            cache.lock().unwrap_or_else(|e| e.into_inner()).insert(name, e);
            e
        }
        _ => {
            throw_argument_error(&format!("Couldn't resolve native function '{name}' in the process: symbol not found"));
            0
        }
    }
}

/// Local selection comes before the foreign lookup so as not to cross heaps/TLS.
fn resolve_native_address(name: &str, process: impl FnOnce(&std::ffi::CStr) -> usize) -> usize {
    runtime_native_address(name).unwrap_or_else(|| {
        std::ffi::CString::new(name).map_or(0, |c| process(&c))
    })
}

#[cfg(test)]
mod tests_resolve_native {
    use super::*;

    #[test]
    fn reserved_abi_does_not_query_same_named_process_runtime() {
        let looked_up = std::cell::Cell::new(false);
        let address = resolve_native_address("dartforge_arc_verify_abi", |_| {
            looked_up.set(true);
            0xdead
        });
        assert!(!looked_up.get());
        let lookup: extern "C" fn(i64) -> i8 = dartforge_arc_verify_abi;
        assert_eq!(address, lookup as *const () as usize);
        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
        assert_eq!(lookup(1), 0);
        HEAP.with(|h| h.borrow_mut().enable_arc());
        assert_eq!(lookup(1), 1);
        HEAP.with(|h| h.replace(previous));
    }

    #[test]
    fn foreign_symbol_keeps_lookup_and_invalid_name_does_not_call_it() {
        assert_eq!(resolve_native_address("user_library_symbol", |c| {
            assert_eq!(c.to_bytes(), b"user_library_symbol");
            0xbeef
        }), 0xbeef);
        assert_eq!(resolve_native_address("missing", |_| 0), 0);
        assert_eq!(resolve_native_address("invalid\0name", |_| panic!("an invalid name reached the lookup")), 0);
    }
}

/// `Pointer<X>.asTypedList(n)`: the internal typed list `class_id` (elements
/// `ty`) over the pointer's native memory, without copying — writing into one is
/// writing into the other.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_typed_external(class_id: i64, ty: i64, pointer: i64, n: i64) -> i64 {
    let Some(address) = address_of(pointer) else {
        throw_argument_error("a Pointer was expected");
        return 0;
    };
    let ty = ty as u8;
    if n < 0 {
        throw_argument_error("length must be non-negative");
        return 0;
    }
    if address == 0 && n > 0 {
        throw_argument_error("asTypedList on nullptr");
        return 0;
    }
    debug_assert_eq!(class_id, i64::from(crate::layout::cid::typed(ty)), "typed list cid outside the contract");
    // The internal list with `EXTERNAL` (§2.5): the length and the native address.
    HEAP.with(|h| h.borrow_mut().external_typed(ty, address as usize as *mut u8, n as usize))
}

/// [`dartforge_typed_external`] that registers the class's method table on the first
/// allocation (like `dartforge_typed_new_t`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_typed_external_t(class_id: i64, ty: i64, pointer: i64, n: i64, f: extern "C" fn() -> *const i64) -> i64 {
    dartforge_register_table(class_id, f);
    dartforge_typed_external(class_id, ty, pointer, n)
}

/// The address of the bytes of a struct/union (an argument by value): the base
/// (`Pointer` or `TypedData`) plus the offset, checked against the size on a
/// `TypedData` base.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_compound_address(obj: i64) -> i64 {
    let data = HEAP.with(|h| match h.borrow().object(obj) {
        Some(fields) => {
            let c = ffi_compounds().read().unwrap_or_else(|e| e.into_inner()).values().find(|c| c.class == fields.class_id).copied()?;
            let field = |i: i64| usize::try_from(i).ok().and_then(|i| fields.get(i));
            Some((c, field(c.base_index)?, field(c.offset_index)?))
        }
        _ => None,
    });
    let Some((c, (base, _), (offset, is_ref_kind))) = data else {
        throw_argument_error("a Struct or Union was expected in a native call");
        return 0;
    };
    let offset = if is_ref_kind { HEAP.with(|h| h.borrow().int_of(offset)).unwrap_or(0) } else { offset };
    with_memory(base, offset, c.size as usize, |p| p as i64).unwrap_or(0)
}

/// A new struct/union, over a `Uint8List` of its size (the value a native call
/// returns by value, as in the VM).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_compound_new(rti: i64) -> i64 {
    let Some(c) = compound_from_rti(rti) else {
        throw_unsupported("struct or union not registered in the DartForge native backend");
        return 0;
    };
    let bytes = HEAP.with(|h| h.borrow_mut().new_typed(TYPE_UINT8, c.size as usize));
    with_roots(&[bytes], || dartforge_ffi_compound(rti, bytes, 0))
}

/// A new struct/union (over a `Uint8List`) with the bytes at `address` — a
/// callback's by-value argument, copied as in the VM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_compound_copy(rti: i64, address: i64) -> i64 {
    let new = dartforge_ffi_compound_new(rti);
    if new == 0 {
        return 0;
    }
    let n = compound_from_rti(rti).map_or(0, |c| c.size as usize);
    let dest = with_roots(&[new], || dartforge_ffi_compound_address(new));
    if dest != 0 && address != 0 {
        // SAFETY: `address` has the struct's `n` bytes (the C entry or a message);
        // `dest`, those of the new struct.
        unsafe { std::ptr::copy_nonoverlapping(address as usize as *const u8, dest as usize as *mut u8, n) };
    }
    new
}

/// Copies the `n` bytes of the struct returned by a callback (at `origin`; 0 if the
/// closure threw) to the C entry's return.
///
/// # Safety
/// `dest` has `n` writable bytes; `origin` is 0 or has `n` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_copy_compound(dest: *mut u8, origin: i64, n: i64) {
    if origin != 0 && n > 0 {
        // SAFETY: guaranteed by the caller.
        unsafe { std::ptr::copy_nonoverlapping(origin as usize as *const u8, dest, n as usize) };
    }
}

// ---------------------------------------------------------------------------
// `Handle`: Dart objects at the native boundary (the VM's local handles).
//
// Each object passed becomes a cell (a `Box<i64>` with the object's handle, a
// collector root through its address) and C receives the cell's address. The
// cells belong to the scope opened around the native call and are released when
// it closes; a `Handle` that comes back from C is checked against this isolate's
// live cells (any other pointer is an `ArgumentError`).

thread_local! {
    static HANDLE_CELLS: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_handles_open() -> i64 {
    HANDLE_CELLS.with(|c| c.borrow().len() as i64)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_handles_close(scope: i64) {
    let released_list: Vec<usize> = HANDLE_CELLS.with(|c| {
        let mut c = c.borrow_mut();
        let of = (scope.max(0) as usize).min(c.len());
        c.split_off(of)
    });
    for cell in released_list {
        HEAP.with(|h| h.borrow_mut().release_global_root(cell as i64));
        // SAFETY: the cell was created by `dartforge_ffi_handle_new` and leaves the
        // registry once.
        unsafe { drop(Box::from_raw(cell as *mut i64)) };
    }
}

/// The `Handle` of `obj`: the address of a new cell of the current scope.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_handle_new(obj: i64) -> i64 {
    let cell = Box::into_raw(Box::new(obj)) as usize;
    HEAP.with(|h| h.borrow_mut().set_global_root(cell as i64, obj));
    HANDLE_CELLS.with(|c| c.borrow_mut().push(cell));
    cell as i64
}

/// The object of a `Handle` returned by C.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_handle_object(handle: i64) -> i64 {
    match live_object_of_handle(handle) {
        Some(obj) => obj,
        None => {
            throw_invalid_handle();
            0
        }
    }
}

/// The object of `handle` if the cell is alive in this isolate; without effects.
fn live_object_of_handle(handle: i64) -> Option<i64> {
    let alive = HANDLE_CELLS.with(|c| c.borrow().iter().rev().any(|&x| x as i64 == handle));
    // SAFETY: the cell is alive (registered in this isolate).
    alive.then(|| unsafe { *(handle as usize as *const i64) })
}

fn throw_invalid_handle() {
    throw_argument_error("the native code returned a Handle that is not alive in this isolate");
}

/// The object of the `Handle` returned by a native call, then closing its handle
/// scope `scope` — also when the `Handle` is invalid, in which case the
/// `ArgumentError` stays pending and the result is null. This way the exceptional
/// exit leaves no cells (and roots) behind.
///
/// Under tracing, nothing collects between reading the cell and returning the object.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_handles_close_with_object(scope: i64, handle: i64) -> i64 {
    let obj = live_object_of_handle(handle);
    dartforge_ffi_handles_close(scope);
    obj.unwrap_or_else(|| {
        throw_invalid_handle();
        0
    })
}

/// [`dartforge_ffi_handles_close_with_object`] under ARC: the object gains a code
/// owner before the scope's cells release their roots. Invalid handle: a pending
/// `ArgumentError`, null and no owner.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_ffi_handles_close_with_object_owned_v1(scope: i64, handle: i64) -> i64 {
    // The owner before closing the handles; the error runs in a native scope.
    in_native_scope(|| {
        let obj = live_object_of_handle(handle);
        if let Some(o) = obj {
            dartforge_arc_retain(o);
        }
        dartforge_ffi_handles_close(scope);
        obj.unwrap_or_else(|| {
            throw_invalid_handle();
            0
        })
    })
}

/// [`dartforge_ffi_handle_object`] under ARC, with a code owner (the `Handle`
/// argument of a callback, whose cell belongs to the outer scope).
/// Invalid handle: a pending `ArgumentError`, null and no owner.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_ffi_handle_object_owned_v1(handle: i64) -> i64 {
    in_native_scope(|| match live_object_of_handle(handle) {
        Some(obj) => {
            dartforge_arc_retain(obj);
            obj
        }
        None => {
            throw_invalid_handle();
            0
        }
    })
}
